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
(Rust) and `package-lock.json` (npm). CI uses these same sources and setup for
formatting, host/WASM, coverage, Miri and deployment. For local coverage/Miri,
add `llvm-tools-preview` to the pinned stable toolchain and `miri` to the pinned
nightly. Update the version sources together and repeat checks and GUI tests;
tool pins do not freeze untracked Cargo lockfiles or moving Git dependencies.

Manual workflows still work with the tools/targets from `devbox.json` and
`development/setup.sh` installed separately, followed by `npm ci`; cargo-make
defaults to stable outside Devbox.

ESP-IDF, flashing and serial permissions need separate setup: follow the esp-rs
[Xtensa](https://docs.esp-rs.org/book/installation/riscv-and-xtensa.html) or
[RISC-V](https://docs.esp-rs.org/book/installation/riscv.html) guides after leaving
Devbox. Host/WASM checks do not prove ESP32, nRF, STM32 or other hardware correctness.

## ℹ️ About

This project is under the [GNU General Public License v2.0](license.md).
