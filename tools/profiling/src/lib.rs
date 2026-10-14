#![cfg(target_os = "linux")]

mod heap_smoke;
mod process;

use miniserde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const HELP: &str = "Usage: cargo run --manifest-path tools/profiling/Cargo.toml -- MODE OPTIONS -- EXECUTABLE ARGS...
Modes: measure, cpu, heap; heap-smoke runs the allocator integration check.
Options: --output NEW_DIRECTORY --scenario NAME [--duration SECONDS] [--runs COUNT]
         [--stdin FILE] [--applications-dir DIRECTORY] [--tool samply|perf]
Defaults: duration 30, runs 3, CPU tool samply. Linux only. No tools are installed.";

struct Options {
    mode: String,
    output: PathBuf,
    scenario: String,
    duration: f64,
    runs: usize,
    stdin: Option<PathBuf>,
    applications_dir: Option<PathBuf>,
    tool: String,
    command: Vec<String>,
}

impl Options {
    fn parse(args: Vec<String>) -> Result<Self> {
        let boundary = args
            .iter()
            .position(|s| s == "--")
            .ok_or("supply the workload after --")?;
        let command = args[boundary + 1..].to_vec();
        if command.is_empty() {
            return Err("the workload command is empty".into());
        }
        let mode = args.first().ok_or(HELP)?.clone();
        if !matches!(mode.as_str(), "measure" | "cpu" | "heap") {
            return Err(format!("unknown mode: {mode}\n{HELP}").into());
        }
        let mut options = Self {
            mode,
            output: PathBuf::new(),
            scenario: String::new(),
            duration: 30.,
            runs: 3,
            stdin: None,
            applications_dir: env::var_os("XILA_PROFILE_APP_DIR").map(PathBuf::from),
            tool: "samply".into(),
            command,
        };
        let mut arguments = args[1..boundary].iter();
        while let Some(key) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {key}"))?;
            match key.as_str() {
                "--output" => options.output = value.into(),
                "--scenario" => options.scenario = value.clone(),
                "--duration" => options.duration = value.parse()?,
                "--runs" => options.runs = value.parse()?,
                "--stdin" => options.stdin = Some(value.into()),
                "--applications-dir" => options.applications_dir = Some(value.into()),
                "--tool" => options.tool = value.clone(),
                _ => return Err(format!("unknown option: {key}").into()),
            }
        }
        if options.output.as_os_str().is_empty() || options.scenario.is_empty() {
            return Err("--output and --scenario are required".into());
        }
        if options.duration <= 0.
            || std::time::Duration::try_from_secs_f64(options.duration).is_err()
        {
            return Err("duration must be finite, greater than zero and representable".into());
        }
        if options.runs == 0 {
            return Err("runs must be at least one".into());
        }
        if !matches!(options.tool.as_str(), "samply" | "perf") {
            return Err("CPU tool must be samply or perf".into());
        }
        Ok(options)
    }

    fn recording_command(&self, directory: &Path) -> Result<Vec<String>> {
        let mut args = match self.mode.as_str() {
            "measure" => return Ok(self.command.clone()),
            "cpu" if self.tool == "samply" => vec![
                text(&executable("samply")?)?,
                "record".into(),
                "--save-only".into(),
                "-o".into(),
                text(&directory.join("cpu.json.gz"))?,
                "--".into(),
            ],
            "cpu" => vec![
                text(&executable("perf")?)?,
                "record".into(),
                "-o".into(),
                text(&directory.join("cpu.perf.data"))?,
                "--call-graph".into(),
                "dwarf".into(),
                "--".into(),
            ],
            _ => vec![
                text(&executable("heaptrack")?)?,
                "-o".into(),
                text(&directory.join("heaptrack"))?,
            ],
        };
        args.extend(self.command.clone());
        Ok(args)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Metadata {
    pub commit: Option<String>,
    pub dirty: bool,
    pub cwd: String,
    pub platform: Option<String>,
    pub machine: String,
    pub cpu: Option<String>,
    pub recorder_version: String,
    pub command: Vec<String>,
    pub executable: String,
    pub executable_sha256: String,
    pub stdin_sha256: Option<String>,
    pub build_environment: BTreeMap<String, Option<String>>,
    pub applications_directory: Option<String>,
    pub application_sha256: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize)]
pub struct Run {
    pub exit_code: i32,
    pub termination: String,
    pub forced_kill: bool,
    pub elapsed_seconds: f64,
    pub user_seconds: f64,
    pub system_seconds: f64,
    pub cpu_seconds: f64,
    pub cpu_percent: f64,
    pub peak_rss_bytes: u64,
    pub recording_command: Vec<String>,
    pub artifacts: Vec<String>,
    pub profile_present: bool,
}

impl Run {
    fn valid(&self) -> bool {
        let expected_stop = self.termination == "duration_limit"
            && [
                -libc::SIGINT,
                -libc::SIGTERM,
                128 + libc::SIGINT,
                128 + libc::SIGTERM,
            ]
            .contains(&self.exit_code);
        self.profile_present && !self.forced_kill && (self.exit_code == 0 || expected_stop)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Summary {
    pub schema_version: u32,
    pub mode: String,
    pub scenario: String,
    pub duration_limit_seconds: f64,
    pub metadata: Metadata,
    pub metrics_scope: String,
    pub runs: Vec<Run>,
    pub medians: Option<BTreeMap<String, f64>>,
    pub tool_version: Option<String>,
    pub error: Option<String>,
}

fn text(path: &Path) -> Result<String> {
    Ok(path
        .to_str()
        .ok_or("paths must be valid UTF-8 for the JSON report")?
        .into())
}

fn executable(name: &str) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let candidates: Vec<PathBuf> = if name.contains('/') {
        vec![PathBuf::from(name)]
    } else {
        env::split_paths(&env::var_os("PATH").unwrap_or_default())
            .map(|dir| dir.join(name))
            .collect()
    };
    candidates
        .into_iter()
        .find(|path| {
            fs::metadata(path)
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
        .ok_or_else(|| {
            format!("executable not found: {name}; install external profilers separately").into()
        })
        .and_then(|path| Ok(path.canonicalize()?))
}

fn digest(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 16384];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    let result = Command::new(program).args(args).output().ok()?;
    result
        .status
        .success()
        .then(|| String::from_utf8_lossy(&result.stdout).trim().into())
}

fn metadata(options: &Options) -> Result<Metadata> {
    let path = executable(&options.command[0])?;
    let mut applications = BTreeMap::new();
    if let Some(directory) = &options.applications_dir {
        for name in ["calculator", "weather"] {
            applications.insert(
                name.into(),
                digest(&directory.join(format!("{name}.wasm")))?,
            );
        }
    }
    let cpu = fs::read_to_string("/proc/cpuinfo")?
        .lines()
        .find_map(|line| {
            line.strip_prefix("model name")
                .and_then(|rest| rest.split_once(':'))
                .map(|(_, value)| value.trim().into())
        });
    Ok(Metadata {
        commit: output("git", &["rev-parse", "HEAD"]),
        dirty: output("git", &["status", "--porcelain"]).is_some_and(|status| !status.is_empty()),
        cwd: text(&env::current_dir()?)?,
        platform: output("uname", &["-sr"]),
        machine: env::consts::ARCH.into(),
        cpu,
        recorder_version: env!("CARGO_PKG_VERSION").into(),
        command: options.command.clone(),
        executable: text(&path)?,
        executable_sha256: digest(&path)?,
        stdin_sha256: options.stdin.as_deref().map(digest).transpose()?,
        build_environment: [
            "RUSTUP_TOOLCHAIN",
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_TARGET_DIR",
            "EMBEDDED_I18N_LOCALE",
        ]
        .into_iter()
        .map(|key| (key.into(), env::var(key).ok()))
        .collect(),
        applications_directory: options.applications_dir.as_deref().map(text).transpose()?,
        application_sha256: applications,
    })
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.
    } else {
        values[middle]
    }
}

fn write_summary(directory: &Path, summary: &Summary) -> Result<()> {
    fs::write(
        directory.join("summary.json"),
        format!("{}\n", miniserde::json::to_string(summary)),
    )?;
    Ok(())
}

/// Run the CLI without invoking a shell or installing external tools.
pub fn main(args: Vec<String>) -> Result<()> {
    if args.as_slice() == ["--help"] || args.is_empty() {
        println!("{HELP}");
        return Ok(());
    }
    if args.as_slice() == ["heap-smoke"] {
        return heap_smoke::run();
    }
    let mut options = Options::parse(args)?;
    options.output = env::current_dir()?.join(&options.output);
    options.applications_dir = options
        .applications_dir
        .map(|path| path.canonicalize())
        .transpose()?;
    let metadata = metadata(&options)?;
    options.recording_command(&options.output)?;
    process::install_handlers()?;
    if let Some(parent) = options.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&options.output)?;
    let mut summary = Summary {
        schema_version: 1,
        mode: options.mode.clone(),
        scenario: options.scenario.clone(),
        duration_limit_seconds: options.duration,
        metadata,
        metrics_scope: if options.mode == "measure" {
            "workload and its waited-for children"
        } else {
            "recorder and its waited-for children; includes profiler overhead"
        }
        .into(),
        runs: vec![],
        medians: None,
        tool_version: None,
        error: None,
    };
    let result = (|| -> Result<()> {
        if options.mode != "measure" {
            let tool = if options.mode == "cpu" {
                &options.tool
            } else {
                "heaptrack"
            };
            let version = Command::new(tool).arg("--version").output()?;
            summary.tool_version = Some(
                format!(
                    "{}{}",
                    String::from_utf8_lossy(&version.stdout),
                    String::from_utf8_lossy(&version.stderr)
                )
                .trim()
                .into(),
            );
        }
        for index in 1..=options.runs {
            let directory = options.output.join(format!("run-{index}"));
            fs::create_dir(&directory)?;
            let command = options.recording_command(&directory)?;
            let mut run = process::record(
                &command,
                &directory,
                options.duration,
                options.stdin.as_deref(),
                options.applications_dir.as_deref(),
                &[],
            )?;
            run.profile_present = options.mode == "measure";
            for entry in fs::read_dir(&directory)? {
                let entry = entry?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if entry.metadata()?.is_file()
                    && entry.metadata()?.len() > 0
                    && ((options.mode == "cpu" && name.starts_with("cpu."))
                        || (options.mode == "heap" && name.starts_with("heaptrack")))
                {
                    run.profile_present = true;
                }
                run.artifacts
                    .push(text(entry.path().strip_prefix(&options.output)?)?);
            }
            run.artifacts.sort();
            let valid = run.valid();
            summary.runs.push(run);
            write_summary(&options.output, &summary)?;
            if !valid {
                return Err(
                    "run failed, was forcibly killed, or produced no profile; inspect the logs"
                        .into(),
                );
            }
        }
        if options.mode == "measure" {
            summary.medians = Some(
                [
                    (
                        "elapsed_seconds",
                        summary.runs.iter().map(|r| r.elapsed_seconds).collect(),
                    ),
                    (
                        "cpu_seconds",
                        summary.runs.iter().map(|r| r.cpu_seconds).collect(),
                    ),
                    (
                        "cpu_percent",
                        summary.runs.iter().map(|r| r.cpu_percent).collect(),
                    ),
                    (
                        "peak_rss_bytes",
                        summary
                            .runs
                            .iter()
                            .map(|r| r.peak_rss_bytes as f64)
                            .collect(),
                    ),
                ]
                .into_iter()
                .map(|(key, values)| (key.into(), median(values)))
                .collect(),
            );
        }
        Ok(())
    })();
    if let Err(error) = &result {
        summary.error = Some(error.to_string());
    }
    write_summary(&options.output, &summary)?;
    println!("{}", options.output.join("summary.json").display());
    result
}
