use super::{Result, Run};
use std::{
    fs::File,
    io,
    os::unix::process::CommandExt,
    path::Path,
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn interrupt(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::Relaxed);
}

pub(crate) fn install_handlers() -> Result<()> {
    // SAFETY: zero initializes the POSIX signal-action structure. The handler
    // performs only a lock-free atomic store, with no allocation or I/O.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = interrupt as *const () as usize;
        libc::sigemptyset(&mut action.sa_mask);
        for signal in [libc::SIGINT, libc::SIGTERM] {
            if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                return Err(io::Error::last_os_error().into());
            }
        }
    }
    Ok(())
}

struct ProcessGroup {
    pid: libc::pid_t,
    reaped: bool,
}

impl ProcessGroup {
    fn signal(&self, signal: libc::c_int) {
        // SAFETY: a negative PID addresses this child's session/process group.
        unsafe {
            libc::kill(-self.pid, signal);
        }
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        // Remove descendants even when their parent already exited.
        self.signal(libc::SIGKILL);
        if !self.reaped {
            loop {
                // SAFETY: waiting for our own child, with no status storage.
                if unsafe { libc::waitpid(self.pid, std::ptr::null_mut(), 0) } >= 0
                    || io::Error::last_os_error().kind() != io::ErrorKind::Interrupted
                {
                    break;
                }
            }
        }
    }
}

pub(crate) fn record(
    command: &[String],
    directory: &Path,
    seconds: f64,
    stdin: Option<&Path>,
    applications: Option<&Path>,
    extra_environment: &[(&str, &Path)],
) -> Result<Run> {
    if INTERRUPTED.load(Ordering::Relaxed) {
        return Err("interrupted".into());
    }
    let mut child = Command::new(&command[0]);
    child
        .args(&command[1..])
        .stdin(File::open(stdin.unwrap_or(Path::new("/dev/null")))?)
        .stdout(File::create(directory.join("stdout.log"))?)
        .stderr(File::create(directory.join("stderr.log"))?)
        .env("XILA_PROFILE_OUTPUT", directory.join("runtime.json"));
    if let Some(applications) = applications {
        child.env("XILA_PROFILE_APP_DIR", applications);
    }
    child.envs(extra_environment.iter().copied());
    // SAFETY: setsid is async-signal-safe; do no allocation between fork/exec.
    unsafe {
        child.pre_exec(|| {
            if libc::setsid() < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let started = Instant::now();
    let child = child.spawn()?;
    let mut group = ProcessGroup {
        pid: child.id() as libc::pid_t,
        reaped: false,
    };
    let mut limited = false;
    let mut forced = false;
    let mut stage = 0;
    let mut deadline = Duration::from_secs_f64(seconds);
    let mut status = 0;
    // SAFETY: rusage is a plain C structure, initialized before wait4 fills it.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    loop {
        if INTERRUPTED.load(Ordering::Relaxed) {
            return Err("interrupted".into());
        }
        // SAFETY: pointers refer to valid writable structures for our child.
        let pid = unsafe { libc::wait4(group.pid, &mut status, libc::WNOHANG, &mut usage) };
        if pid == group.pid {
            group.reaped = true;
            break;
        }
        if pid < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return Err(io::Error::last_os_error().into());
        }
        let elapsed = started.elapsed();
        if elapsed >= deadline {
            limited = true;
            group.signal([libc::SIGINT, libc::SIGTERM, libc::SIGKILL][stage]);
            forced = stage == 2;
            deadline = elapsed + Duration::from_secs(2);
            stage = (stage + 1).min(2);
        }
        thread::sleep(Duration::from_millis(20));
    }
    let elapsed = started.elapsed().as_secs_f64();
    let seconds = |time: libc::timeval| time.tv_sec as f64 + time.tv_usec as f64 / 1_000_000.;
    let user = seconds(usage.ru_utime);
    let system = seconds(usage.ru_stime);
    Ok(Run {
        exit_code: if libc::WIFEXITED(status) {
            libc::WEXITSTATUS(status)
        } else {
            -libc::WTERMSIG(status)
        },
        termination: if limited { "duration_limit" } else { "exited" }.into(),
        forced_kill: forced,
        elapsed_seconds: elapsed,
        user_seconds: user,
        system_seconds: system,
        cpu_seconds: user + system,
        cpu_percent: 100. * (user + system) / elapsed,
        peak_rss_bytes: usage.ru_maxrss as u64 * 1024,
        recording_command: command.to_vec(),
        artifacts: vec![],
        profile_present: true,
    })
}
