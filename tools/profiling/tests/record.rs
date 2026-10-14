#![cfg(target_os = "linux")]

use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Duration,
};
use xila_profiling::Summary;

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "xila-profile-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn output(&self) -> PathBuf {
        self.0.join("results with spaces")
    }
    fn run(
        &self,
        mode: &str,
        options: &[&str],
        applications: Option<&Path>,
    ) -> (Output, Option<Summary>) {
        self.launch("measure", mode, options, applications)
    }
    fn launch(
        &self,
        recording_mode: &str,
        fixture_mode: &str,
        options: &[&str],
        applications: Option<&Path>,
    ) -> (Output, Option<Summary>) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xila-profiling"));
        command
            .arg(recording_mode)
            .arg("--output")
            .arg(self.output())
            .args(["--scenario", "test", "--runs", "1"])
            .args(options)
            .arg("--")
            .arg(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "fixture", "--nocapture"])
            .env("XILA_TEST_FIXTURE", fixture_mode)
            .env_remove("XILA_PROFILE_APP_DIR");
        if let Some(path) = applications {
            command.env("XILA_PROFILE_APP_DIR", path);
        }
        let output = command.output().unwrap();
        let summary = fs::read_to_string(self.output().join("summary.json"))
            .ok()
            .map(|json| miniserde::json::from_str(&json).unwrap());
        (output, summary)
    }
    fn log(&self, run: usize) -> String {
        fs::read_to_string(self.output().join(format!("run-{run}/stdout.log"))).unwrap()
    }
    fn apps(&self) -> PathBuf {
        let path = self.0.join("apps with spaces");
        fs::create_dir(&path).unwrap();
        for name in ["calculator", "weather"] {
            fs::write(path.join(format!("{name}.wasm")), b"\0asm").unwrap();
        }
        path
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

extern "C" fn fail_on_interrupt(_: libc::c_int) {
    // SAFETY: _exit is async-signal-safe and ends only this fixture process.
    unsafe {
        libc::_exit(7);
    }
}

#[test]
#[ignore = "subprocess fixture, launched by integration tests"]
fn fixture() {
    let mode = std::env::var("XILA_TEST_FIXTURE").unwrap();
    match mode.as_str() {
        "allocate" => {
            let mut data = vec![0u8; 16 * 1024 * 1024];
            for byte in &mut data {
                *byte = std::hint::black_box(42);
            }
            std::hint::black_box(&data);
            println!("literal $HOME with spaces");
        }
        "fail" => std::process::exit(7),
        "stdin" => {
            let mut text = String::new();
            std::io::stdin().read_to_string(&mut text).unwrap();
            print!("input-start={text}input-end");
        }
        "apps" => println!("{}", std::env::var("XILA_PROFILE_APP_DIR").unwrap()),
        "background" => {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", "fixture", "--nocapture"])
                .env("XILA_TEST_FIXTURE", "sleep")
                .spawn()
                .unwrap();
            println!("child-pid={}", child.id());
            std::io::stdout().flush().unwrap();
            thread::sleep(Duration::from_secs(20));
            child.wait().unwrap();
        }
        "fail-on-stop" => {
            // SAFETY: handler exits using the async-signal-safe POSIX primitive.
            unsafe {
                libc::signal(libc::SIGINT, fail_on_interrupt as *const () as usize);
            }
            thread::sleep(Duration::from_secs(20));
        }
        "sleep" => thread::sleep(Duration::from_secs(20)),
        _ => panic!("unknown fixture"),
    }
}

#[test]
fn repeats_capture_cpu_rss_and_literal_arguments() {
    let scratch = Scratch::new();
    let (result, summary) = scratch.run("allocate", &["--runs", "2", "--duration", "5"], None);
    assert!(result.status.success(), "{:?}", result);
    let summary = summary.unwrap();
    assert_eq!(summary.runs.len(), 2);
    assert!(summary.runs.iter().all(|run| run.termination == "exited"));
    let medians = summary.medians.unwrap();
    assert!(medians["peak_rss_bytes"] > 16. * 1024. * 1024.);
    assert!(medians["cpu_seconds"] > 0.);
    assert!(scratch.log(1).contains("literal $HOME with spaces"));
}

#[test]
fn failure_is_not_a_successful_measurement() {
    let scratch = Scratch::new();
    let (result, summary) = scratch.run("fail", &[], None);
    assert!(!result.status.success());
    let summary = summary.unwrap();
    assert_eq!(summary.runs[0].exit_code, 7);
    assert!(summary.medians.is_none());
}

#[test]
fn duration_stops_background_children() {
    let scratch = Scratch::new();
    let (result, summary) = scratch.run("background", &["--duration", "0.4"], None);
    assert!(result.status.success(), "{result:?}");
    assert_eq!(summary.unwrap().runs[0].termination, "duration_limit");
    let log = scratch.log(1);
    let pid = log
        .lines()
        .find_map(|line| line.strip_prefix("child-pid="))
        .unwrap();
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"));
    assert!(stat.is_err() || stat.unwrap().split_whitespace().nth(2) == Some("Z"));
}

#[test]
fn failure_during_stop_has_no_successful_medians() {
    let scratch = Scratch::new();
    let (result, summary) = scratch.run("fail-on-stop", &["--duration", "0.4"], None);
    assert!(!result.status.success());
    let summary = summary.unwrap();
    assert_eq!(summary.runs[0].exit_code, 7);
    assert!(summary.medians.is_none());
}

#[test]
fn existing_output_is_not_overwritten() {
    let scratch = Scratch::new();
    assert!(scratch.run("allocate", &[], None).0.status.success());
    let first = scratch.log(1);
    assert!(!scratch.run("fail", &[], None).0.status.success());
    assert_eq!(scratch.log(1), first);
}

#[test]
fn input_is_replayed_each_run() {
    let scratch = Scratch::new();
    let input = scratch.0.join("input.txt");
    fs::write(&input, "repeat me\n").unwrap();
    let (result, _) = scratch.run(
        "stdin",
        &["--runs", "2", "--stdin", input.to_str().unwrap()],
        None,
    );
    assert!(result.status.success());
    for run in [1, 2] {
        let log = scratch.log(run);
        let replay = log
            .split_once("input-start=")
            .unwrap()
            .1
            .split_once("input-end")
            .unwrap()
            .0;
        assert_eq!(replay, "repeat me\n");
    }
}

#[test]
fn explicit_applications_are_hashed_and_forwarded() {
    let scratch = Scratch::new();
    let apps = scratch.apps();
    let (result, summary) = scratch.run(
        "apps",
        &["--applications-dir", apps.to_str().unwrap()],
        None,
    );
    assert!(result.status.success());
    let metadata = summary.unwrap().metadata;
    assert_eq!(metadata.applications_directory.as_deref(), apps.to_str());
    assert_eq!(metadata.application_sha256.len(), 2);
    assert_eq!(
        metadata.application_sha256["calculator"],
        "cd5d4935a48c0672cb06407bb443bc0087aff947c6b864bac886982c73b3027f"
    );
    assert!(scratch.log(1).contains(apps.to_str().unwrap()));
}

#[test]
fn inherited_applications_are_hashed_too() {
    let scratch = Scratch::new();
    let apps = scratch.apps();
    let (result, summary) = scratch.run("apps", &[], Some(&apps));
    assert!(result.status.success());
    assert_eq!(
        summary.unwrap().metadata.applications_directory.as_deref(),
        apps.to_str()
    );
}

#[test]
fn invalid_durations_are_rejected_before_creating_output() {
    for duration in ["nan", "inf", "-1", "0", "1e100"] {
        let scratch = Scratch::new();
        let (result, summary) = scratch.run("allocate", &["--duration", duration], None);
        assert!(!result.status.success());
        assert!(summary.is_none());
        assert!(!scratch.output().exists());
    }
}

#[test]
fn missing_profiler_is_rejected_before_creating_output() {
    let scratch = Scratch::new();
    let result = Command::new(env!("CARGO_BIN_EXE_xila-profiling"))
        .args(["cpu", "--output"])
        .arg(scratch.output())
        .args(["--scenario", "test", "--", "/bin/true"])
        .env("PATH", "")
        .env_remove("XILA_PROFILE_APP_DIR")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!scratch.output().exists());
}

#[test]
fn workload_arguments_are_preserved_without_shell_expansion() {
    let scratch = Scratch::new();
    let result = Command::new(env!("CARGO_BIN_EXE_xila-profiling"))
        .args(["measure", "--output"])
        .arg(scratch.output())
        .args([
            "--scenario",
            "test",
            "--runs",
            "1",
            "--",
            "/bin/echo",
            "literal $HOME with spaces",
        ])
        .env_remove("XILA_PROFILE_APP_DIR")
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(scratch.log(1), "literal $HOME with spaces\n");
    let summary: Summary = miniserde::json::from_str(
        &fs::read_to_string(scratch.output().join("summary.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        summary.metadata.command,
        ["/bin/echo", "literal $HOME with spaces"]
    );
}

#[test]
fn interrupting_recorder_cleans_up_workload_and_descendants() {
    let scratch = Scratch::new();
    let mut recorder = Command::new(env!("CARGO_BIN_EXE_xila-profiling"))
        .args(["measure", "--output"])
        .arg(scratch.output())
        .args([
            "--scenario",
            "test",
            "--runs",
            "1",
            "--duration",
            "20",
            "--",
        ])
        .arg(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "fixture", "--nocapture"])
        .env("XILA_TEST_FIXTURE", "background")
        .env_remove("XILA_PROFILE_APP_DIR")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let log_path = scratch.output().join("run-1/stdout.log");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let pid = loop {
        let log = fs::read_to_string(&log_path).unwrap_or_default();
        if let Some(pid) = log.lines().find_map(|line| line.strip_prefix("child-pid=")) {
            break pid.to_string();
        }
        if std::time::Instant::now() > deadline {
            recorder.kill().unwrap();
            recorder.wait().unwrap();
            panic!("fixture did not start");
        }
        thread::sleep(Duration::from_millis(20));
    };
    // SAFETY: signal only the recorder subprocess started by this test.
    assert_eq!(unsafe { libc::kill(recorder.id() as i32, libc::SIGINT) }, 0);
    assert!(!recorder.wait().unwrap().success());
    let summary: Summary = miniserde::json::from_str(
        &fs::read_to_string(scratch.output().join("summary.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(summary.error.as_deref(), Some("interrupted"));
    assert!(summary.medians.is_none());
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"));
    assert!(stat.is_err() || stat.unwrap().split_whitespace().nth(2) == Some("Z"));
}
