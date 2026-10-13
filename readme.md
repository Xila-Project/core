<h1 align="center">Xila core</h1>

[![codecov](https://codecov.io/github/Xila-Project/Core/graph/badge.svg?token=01M6GQFBEC)](https://codecov.io/github/Xila-Project/Core)
[![État de la traduction](https://translate.anneraud.fr/widget/xila-core/svg-badge.svg)](https://translate.anneraud.fr/engage/xila-core/)
[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-yellow.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)

## 📝 Description

This repository contains the core of Xila. It contains the code for xila's core functionalities like task management, file system, virtual machine, graphics, drivers, etc.

## 🛠️ Build

### Optional Devbox environment

From a checkout on **x86_64 Linux**, install [Devbox](https://www.jetify.com/docs/devbox/installing_devbox/)
at the version in `.devbox-version`, then run:

```bash
devbox shell
devbox run setup
cargo make check                 # Linux x86_64 host + WASM guest
cargo make clippy
cargo make format-rust -- --check
cargo make format-toml --check
npm exec prettier -- --check "**/*.json"
```

Devbox installs Nix if needed. Setup installs the pinned Rust toolchains/WASM
targets and runs `npm ci`. Exit with `exit`; your normal setup stays unchanged.
Remove `.dev-environment/` and `.devbox/` after leaving to clear local caches.
Initial setup needs internet access, including GitHub, artifacts.nixos.org,
cache.nixos.org, static.rust-lang.org, crates.io and registry.npmjs.org.

Devbox provides a host-native shell with JSON configuration and pinned native
libraries. Containers need GUI/device forwarding; mise leaves native packages
to the OS; direct Nix/devenv requires maintaining Nix configuration. Devbox is
optional locally; manual setup below remains supported.

Version sources are `.devbox-version` (CLI), `devbox.json`/`devbox.lock` (native
tools, including Taplo 0.9.3), `development/toolchains.env` (Rust) and
`package-lock.json` (npm). CI uses the same setup and `devbox run` for the existing
formatting, host/WASM, coverage, Miri and deployment checks. For local coverage,
add `llvm-tools-preview` to the pinned stable toolchain; for Miri, add `miri` to
the pinned nightly. Tool pins do not freeze the untracked Cargo lockfiles or
moving Git dependencies. Update the version sources together and repeat the
checks and GUI smoke test.

Run `devbox run gui` from a **Wayland desktop** and verify rendering, input and
window closing. Enter without `--pure` to retain the host display, runtime
directory and session bus. Working host GPU drivers are required; compilation
does not validate GUI behavior. Windows needs x86_64 WSL2/WSLg, with GUI behavior
still requiring validation. Native Windows, macOS, ARM Linux and NixOS are not
validated by this environment; the current host target and native event loop
depend on Linux/Wayland.

For the browser example, inside the shell:

```bash
cargo build -p calculator --target wasm32-wasip1 --release
cargo make generate-fonts
(cd examples/wasm && RUSTUP_TOOLCHAIN="$XILA_NIGHTLY_TOOLCHAIN" trunk serve)
```

Use `trunk build` instead of `trunk serve` for a build check. Open the served URL
to check rendering and input. ESP-IDF/vendor toolchains, flashing and serial
permissions require separate setup using the guides below; leave Devbox first.
Host/WASM checks do not establish ESP32, nRF, STM32 or other hardware correctness.

### ✅ Manual requirements

Please install the following dependencies to build core:

- `gcc-multilib`: for cross-compilation purposes.
- `clang`, `libclang-dev`, and `mold`: for native linking and generated C bindings.
- [`cargo-make`](https://github.com/sagiegurari/cargo-make): to build the project with `cargo`.
- `nodejs`: to generate fonts for LVGL ([lv_font_conv](https://github.com/lvgl/lv_font_conv)).
- `wasm32-wasip1` Rust target: to compile xila virtual machine executables.
- (Optional) `wasm32-unknown-unknown` Rust target: to compile `wasm_example`.
- (Optional) `nightly` Rust toolchain: to compile the `wasm_example`.
- (Optional) [`trunk`](https://trunkrs.dev/): to build the `wasm_example`.
- (Optional) Rust [xtensa-esp32\*-espidf](https://docs.esp-rs.org/book/installation/riscv-and-xtensa.html) toolchain: to compile for ESP32 / ESP32-S series.
- (Optional) Rust [riscv\*-esp-espidf](https://docs.esp-rs.org/book/installation/riscv.html): to compile for ESP32-H / ESP32-C series.

### 🛠️ Instructions

1. Clone the repository:

```bash
git clone https://github.com/Xila-Project/Core.git
```

2. Navigate to the project directory:

```bash
cd Core
npm ci
```

3. Run one of the examples:

```bash
cargo make run -p native_example
```

or

```bash
cargo +stable build -p calculator --target wasm32-wasip1 --release
cargo make generate-fonts
cd examples/wasm && trunk serve
```

## ℹ️ About

This project is under the [GNU General Public License v2.0](license.md).
