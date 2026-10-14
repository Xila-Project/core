//! Opt-in Linux executor/task/heap snapshots for native profiling recordings.
use miniserde::Serialize;
use std::{
    fs,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

static EXECUTORS: Mutex<Vec<Arc<ExecutorCounters>>> = Mutex::new(Vec::new());

pub(crate) struct ExecutorCounters {
    polls: AtomicU64,
    cpu: AtomicU64,
    wall: AtomicU64,
    waits: AtomicU64,
    wait_wall: AtomicU64,
    thread_id: AtomicU64,
    clock_errors: AtomicU64,
    poll_active: AtomicBool,
    wait_active: AtomicBool,
}

#[derive(Serialize)]
struct ExecutorSample {
    executor: usize,
    thread_id: u64,
    polls: u64,
    poll_cpu_nanoseconds: u64,
    poll_wall_nanoseconds: u64,
    waits: u64,
    wait_wall_nanoseconds: u64,
    clock_errors: u64,
    poll_active: bool,
    wait_active: bool,
}

struct ActivityGuard<'a>(&'a AtomicBool);
impl Drop for ActivityGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

impl ExecutorCounters {
    pub(crate) fn new() -> Arc<Self> {
        let value = Arc::new(Self {
            polls: AtomicU64::new(0),
            cpu: AtomicU64::new(0),
            wall: AtomicU64::new(0),
            waits: AtomicU64::new(0),
            wait_wall: AtomicU64::new(0),
            thread_id: AtomicU64::new(0),
            clock_errors: AtomicU64::new(0),
            poll_active: AtomicBool::new(false),
            wait_active: AtomicBool::new(false),
        });
        EXECUTORS.lock().unwrap().push(value.clone());
        value
    }
    pub(crate) fn poll<R>(&self, poll: impl FnOnce() -> R) -> R {
        self.thread_id.store(
            unsafe { libc::syscall(libc::SYS_gettid) as u64 },
            Ordering::Relaxed,
        );
        let start_cpu = task::profiling::thread_cpu_nanoseconds();
        let started = Instant::now();
        self.poll_active.store(true, Ordering::Relaxed);
        let active = ActivityGuard(&self.poll_active);
        let result = poll();
        drop(active);
        let elapsed = started.elapsed().as_nanos() as u64;
        let end_cpu = task::profiling::thread_cpu_nanoseconds();
        self.polls.fetch_add(1, Ordering::Relaxed);
        self.wall.fetch_add(elapsed, Ordering::Relaxed);
        if let (Some(start), Some(end)) = (start_cpu, end_cpu) {
            self.cpu
                .fetch_add(end.saturating_sub(start), Ordering::Relaxed);
        } else {
            self.clock_errors.fetch_add(1, Ordering::Relaxed);
        }
        result
    }
    pub(crate) fn wait<R>(&self, wait: impl FnOnce() -> R) -> R {
        let started = Instant::now();
        self.wait_active.store(true, Ordering::Relaxed);
        let active = ActivityGuard(&self.wait_active);
        let result = wait();
        drop(active);
        self.waits.fetch_add(1, Ordering::Relaxed);
        self.wait_wall
            .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
        result
    }
    fn sample(&self) -> ExecutorSample {
        ExecutorSample {
            executor: self as *const Self as usize,
            thread_id: self.thread_id.load(Ordering::Relaxed),
            polls: self.polls.load(Ordering::Relaxed),
            poll_cpu_nanoseconds: self.cpu.load(Ordering::Relaxed),
            poll_wall_nanoseconds: self.wall.load(Ordering::Relaxed),
            waits: self.waits.load(Ordering::Relaxed),
            wait_wall_nanoseconds: self.wait_wall.load(Ordering::Relaxed),
            clock_errors: self.clock_errors.load(Ordering::Relaxed),
            poll_active: self.poll_active.load(Ordering::Relaxed),
            wait_active: self.wait_active.load(Ordering::Relaxed),
        }
    }
}

#[derive(Serialize)]
struct TaskSample {
    generation: usize,
    manager: usize,
    task: usize,
    name: String,
    spawner: usize,
    thread_id: u64,
    polls: u64,
    cpu_nanoseconds: u64,
    poll_wall_nanoseconds: u64,
    max_poll_wall_nanoseconds: u64,
    clock_errors: u64,
    completed: bool,
    poll_active: bool,
    allocation_requests: u64,
    allocation_request_bytes: u64,
}

#[derive(Serialize)]
struct Report {
    schema_version: u32,
    snapshot_unix_milliseconds: u64,
    heap_used_bytes: usize,
    heap_reserved_bytes: usize,
    executors: Vec<ExecutorSample>,
    tasks: Vec<TaskSample>,
}

/// Atomically replace a cumulative snapshot. Heap usage includes allocator overhead.
pub fn write_report(path: &Path) -> std::io::Result<()> {
    let executors = EXECUTORS
        .lock()
        .unwrap()
        .iter()
        .map(|c| c.sample())
        .collect();
    let tasks = task::profiling::snapshot()
        .into_iter()
        .map(|t| TaskSample {
            generation: t.generation,
            manager: t.manager,
            task: t.task,
            name: t.name,
            spawner: t.spawner,
            thread_id: t.thread_id,
            polls: t.polls,
            cpu_nanoseconds: t.cpu_nanoseconds,
            poll_wall_nanoseconds: t.poll_wall_nanoseconds,
            max_poll_wall_nanoseconds: t.max_poll_wall_nanoseconds,
            clock_errors: t.clock_errors,
            completed: t.completed,
            poll_active: t.poll_active,
            allocation_requests: t.allocation_requests,
            allocation_request_bytes: t.allocation_request_bytes,
        })
        .collect();
    let manager = memory::global_manager();
    let report = Report {
        schema_version: 1,
        snapshot_unix_milliseconds: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        heap_used_bytes: manager.get_used(),
        heap_reserved_bytes: manager.get_total_size(),
        executors,
        tasks,
    };
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, miniserde::json::to_string(&report))?;
    fs::rename(temporary, path)
}

/// Start periodic snapshots only when the recording runner supplies a destination.
/// The last snapshot can precede termination by up to one second; it is not final.
pub fn start_from_environment() -> std::io::Result<()> {
    let Some(path) = std::env::var_os("XILA_PROFILE_OUTPUT") else {
        return Ok(());
    };
    let path = std::path::PathBuf::from(path);
    write_report(&path)?;
    std::thread::Builder::new()
        .name("xila-profiler".into())
        .spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if let Err(error) = write_report(&path) {
                    eprintln!("profiling snapshot failed: {error}");
                    break;
                }
            }
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_executor_poll_and_wait_are_visible() {
        let counters = ExecutorCounters::new();
        counters.poll(|| assert!(counters.sample().poll_active));
        counters.wait(|| assert!(counters.sample().wait_active));
        assert!(!counters.sample().poll_active);
        assert!(!counters.sample().wait_active);
    }

    #[test]
    fn executor_wait_is_separate_from_poll_work() {
        let counters = ExecutorCounters::new();
        counters.poll(|| std::hint::black_box(42));
        counters.wait(|| std::thread::sleep(std::time::Duration::from_millis(20)));
        let sample = counters.sample();
        assert_eq!(sample.polls, 1);
        assert_eq!(sample.waits, 1);
        assert!(sample.wait_wall_nanoseconds >= 20_000_000);
        assert!(sample.poll_wall_nanoseconds < sample.wait_wall_nanoseconds);
    }
}
