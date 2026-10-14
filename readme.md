<h1 align="center">Xila core</h1>

[![codecov](https://codecov.io/github/Xila-Project/Core/graph/badge.svg?token=01M6GQFBEC)](https://codecov.io/github/Xila-Project/Core)
[![État de la traduction](https://translate.anneraud.fr/widget/xila-core/svg-badge.svg)](https://translate.anneraud.fr/engage/xila-core/)
[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-yellow.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)

## 📝 Description

This repository contains the core of Xila. It contains the code for xila's core functionalities like task management, file system, virtual machine, graphics, drivers, etc.

## 🛠️ Build

**Devbox is the standard setup for local development and CI on x86_64 Linux.**
Install [Devbox](https://www.jetify.com/docs/devbox/installing_devbox/) at the
version recorded in `.devbox-version`, then:

```bash
git clone https://github.com/Xila-Project/Core.git
cd Core
devbox shell
devbox run setup
devbox run check                # Linux host + WASM guest checks
```

Devbox installs Nix if needed; setup installs Rust/WASM targets and runs `npm ci`.
Initial setup requires internet access. Leave with `exit`; `.dev-environment/`
and `.devbox/` hold removable local caches.

Other checks, inside the shell:

```bash
cargo make clippy
cargo make format-rust -- --check
cargo make format-toml --check
npm exec prettier -- --check "**/*.json"
```

Run `devbox run gui` from a **Wayland desktop** and check rendering, input and
window closing. Keep the host GPU drivers and enter without `--pure` to retain
the display, runtime directory and session bus. Windows requires x86_64 WSL2/WSLg
and a GUI smoke test. Native Windows, macOS, ARM Linux and NixOS are unvalidated;
the current host checks and native event loop depend on Linux/Wayland.

For the browser example, inside the shell:

```bash
cargo build -p calculator --target wasm32-wasip1 --release
cargo make generate-fonts
(cd examples/wasm && RUSTUP_TOOLCHAIN="$XILA_NIGHTLY_TOOLCHAIN" trunk serve)
```

Use `trunk build` for a build check; open the served URL to test rendering/input.

Devbox provides a native shell and JSON configuration. Containers require GUI
and device forwarding; mise leaves native dependencies to the OS; direct
Nix/devenv adds Nix configuration to maintain. Versions live in `.devbox-version`
(CLI), `devbox.json`/`devbox.lock` (native tools), `development/toolchains.env`
(Rust versions/targets) and `package-lock.json` (npm). CI uses these same sources and setup for
formatting, host/WASM, coverage, Miri and deployment. For local coverage/Miri,
add `llvm-tools-preview` to the pinned stable toolchain and `miri` to the pinned
nightly. Update the version sources together and repeat checks and GUI tests;
tool pins do not freeze untracked Cargo lockfiles or moving Git dependencies.

Manual workflows still work with the tools/targets from `devbox.json` installed separately, followed by `npm ci`; cargo-make
defaults to stable outside Devbox.

ESP-IDF, flashing and serial permissions need separate setup: follow the esp-rs
[Xtensa](https://docs.esp-rs.org/book/installation/riscv-and-xtensa.html) or
[RISC-V](https://docs.esp-rs.org/book/installation/riscv.html) guides after leaving
Devbox. Host/WASM checks do not prove ESP32, nRF, STM32 or other hardware correctness.

## Native profiling

Profiling is intended for developers and optimization agents on Linux. It is
opt-in and adds no hooks/counters to default builds. The recorder and its checks
use the existing Rust toolchain; install [Samply](https://github.com/mstange/samply), Linux `perf`,
or [heaptrack](https://github.com/KDE/heaptrack) separately when needed. The
runner never installs tools or changes kernel permissions. Linux CPU recording
requires access to perf events; a permission error is reported in the run log.

Build before recording, inside the development shell:

```bash
cargo make prepare-profiling
cargo make build-profiling --features prebuilt-applications
cargo run --manifest-path tools/profiling/Cargo.toml -- measure \
  --scenario terminal-idle --output target/profiles/before --runs 3 --duration 30 \
  --applications-dir target/wasm32-wasip1/release-wasm \
  -- target/profiling/native_example --terminal --no-network
```

The `profiling` Cargo profile keeps release optimizations and full debug symbols.
If using an explicit Cargo target or custom target directory, adjust the binary
path. The `prebuilt-applications` feature and `--applications-dir` load prepared
Calculator/Weather files instead of invoking Cargo during startup. A missing
file fails explicitly. The recorder also recognizes and hashes application files
selected by `XILA_PROFILE_APP_DIR`. Default native builds keep their existing startup builds.
Measurements still include OS startup; they are not steady-state-only data.
Use the same disk fixture and settings on each run. A mutable disk can change
between runs; restore its baseline if the workload writes to it.

For allocation and task attribution, build with the optional features:

```bash
cargo make build-profiling --features profiling,heaptrack
cargo run --manifest-path tools/profiling/Cargo.toml -- cpu \
  --scenario gui-navigation --output target/profiles/cpu --runs 1 --duration 60 \
  --applications-dir target/wasm32-wasip1/release-wasm \
  -- target/profiling/native_example --gui --no-network --resolution 800x600
cargo run --manifest-path tools/profiling/Cargo.toml -- heap \
  --scenario gui-navigation --output target/profiles/heap --runs 1 --duration 60 \
  --applications-dir target/wasm32-wasip1/release-wasm \
  -- target/profiling/native_example --gui --no-network --resolution 800x600
```

Replay the same manual GUI interactions for exploratory recordings. For repeatable
comparisons, use a bounded scripted workload and pass `--stdin INPUT_FILE` when
it accepts stdin; the file is replayed for every run. The default stdin is empty.
The runner also works with an already-built benchmark or test harness, so agents
can isolate a hot path without starting the whole graphical OS.

Each new output directory contains `summary.json`, per-run stdout/stderr logs,
and any profiles. Existing directories are refused to protect previous results.
The summary retains exact argv, executable hash, Git revision/dirty status,
selected build/locale settings, prebuilt application hashes, CPU model, duration, exit status and median
CPU/RSS measurements. It does not copy the full environment or credentials.
A duration limit stops the process group; forced kills and failed runs have no
successful median. Inspect the logs to establish that the intended scenario
actually ran. Profiler recordings must produce a nonempty artifact.

With the `profiling` feature, `runtime.json` is replaced atomically about once a
second. It records allocator-used/reserved bytes, task names/IDs/generations,
spawners/OS threads, poll counts and CPU time, maximum poll wall time, executor
poll/wait totals, and successful allocation request count/volume per task.
Completed task generations remain in memory until exit. Snapshots can lag exit
and are not final or peak heap measurements; counter updates during a snapshot
are not a transaction. In-progress polls/waits are counted only when they return. Tasks not spawned through the Xila manager are absent.
Executor CPU includes task CPU; do not sum both. Poll CPU uses OS-thread CPU
time, while wall time can include blocking/descheduling. Allocation volume
counts requested bytes, not retained memory or ownership of later frees.
Profiling metadata and the reporter itself add overhead in feature-enabled runs.

- Samply profiles: `samply load target/profiles/cpu/run-1/cpu.json.gz`.
- Perf alternative: add `--tool perf` to CPU recording; inspect
  `cpu.perf.data` with `perf report` or Hotspot.
- Heap reports: open the generated `heaptrack*` data file with `heaptrack_gui`
  or `heaptrack_print`. Xila's custom `mmap` allocator needs the `heaptrack`
  feature; plain malloc interception misses its suballocations. Reallocation is
  reported as the allocator's actual allocate/copy/free operations. Requested
  sizes exclude allocator padding and Xila ABI allocation headers are included
  when the manager receives them. Heaptrack also captures ordinary libc calls.

For before/after consumption comparisons, rebuild **without** `profiling` and
`heaptrack` (retain only `prebuilt-applications` for the OS workload), repeat `measure` with the same scenario/inputs, and compare medians
and individual runs. Linux `wait4` CPU accounting includes waited-for children;
peak RSS is a high-water mark, not a sum of simultaneous process-tree memory.
Instrumented CPU/heap runs explain costs; their timings are not uninstrumented
benchmarks. No heavy profiler is run automatically on PRs or merge groups.
Native findings guide shared-code optimization, but validate gains on the actual
Wasm/embedded target before claiming target-specific improvements.

Local checks:

```bash
cargo test --manifest-path tools/profiling/Cargo.toml
cargo test -p task -p drivers_std --lib --features task/test_harness,drivers_std/profiling,drivers_std/heaptrack
cargo test -p native_example --bin native_example --features profiling,heaptrack
cargo run --manifest-path tools/profiling/Cargo.toml -- heap-smoke
```

The heaptrack smoke test compiles a local C API stand-in and verifies real native
allocation events, failed requests, realloc preservation and silence without the
feature. It does not require installing heaptrack or access to perf events.

## ℹ️ About

This project is under the [GNU General Public License v2.0](license.md).
