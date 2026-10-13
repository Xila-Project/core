<h1 align="center">Xila core</h1>

[![codecov](https://codecov.io/github/Xila-Project/Core/graph/badge.svg?token=01M6GQFBEC)](https://codecov.io/github/Xila-Project/Core)
[![État de la traduction](https://translate.anneraud.fr/widget/xila-core/svg-badge.svg)](https://translate.anneraud.fr/engage/xila-core/)
[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-yellow.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)

## 📝 Description

This repository contains the core of Xila. It contains the code for xila's core functionalities like task management, file system, virtual machine, graphics, drivers, etc.

## 🛠️ Build

For an optional Devbox environment with pinned tools and host display access, see
[the local development guide](development/README.md). Manual setup below remains
supported. CI uses the same pinned Devbox tools and setup script.

### ✅ Requirements

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
cargo make generate-fonts
cd examples/wasm && trunk serve
```

## ℹ️ About

This project is under the [GNU General Public License v2.0](license.md).
