# Optional local development environment

The host-native Devbox shell supplies the common Linux and WASM development tools
without replacing your normal setup. Start in the repository root on **x86_64
Linux** with [Devbox installed](https://www.jetify.com/docs/devbox/installing_devbox/).
Use the version in `.devbox-version` (also used by CI). Devbox installs Nix if
needed. Then run:

```sh
devbox shell
devbox run setup
cargo make check
```

The first invocation downloads the pinned native packages. Setup installs the
pinned Rust toolchains and WASM targets in `.dev-environment/`, then
uses `npm ci` with the existing `package-lock.json`. It can be rerun. Run `exit`
to leave the shell. Your normal Rust installation and shell configuration are
not changed. To reclaim its local caches, leave the shell and remove
`.dev-environment/` and `.devbox/`; Nix packages remain in the shared Nix store until garbage
collection. Internet access is needed initially, including GitHub, api.github.com, artifacts.nixos.org,
cache.nixos.org, static.rust-lang.org, crates.io and registry.npmjs.org.

Run `devbox shell` from the repository root, without `--pure`. Do not set persistent
shell exports for this environment or commit generated caches or assets.

## Why this approach

The repository's host checks explicitly target `x86_64-unknown-linux-gnu`; its
native event loop currently imports Wayland-specific winit APIs. Linux is the
initial supported contributor platform, rather than promising native macOS or
Windows builds that the source and CI do not validate.

| Approach | Platforms and display | Reproducibility and maintenance | Embedded tools |
| --- | --- | --- | --- |
| Devbox | Linux/macOS; Windows through WSL. Host session retained. | Convenient shell and lockfile, but adds Devbox and its package index on top of Nix. | ESP-IDF still needs separate setup. |
| Dev Containers | Linux; Docker VM on macOS/Windows. GUI requires explicit display/GPU forwarding. | Pinning an image works well for CI, but container and GUI configuration adds maintenance. | Serial devices and hardware access need forwarding. |
| Nix shell / flakes / devenv | Linux/macOS; Windows through WSL. Host session retained. | A small shell with one immutable nixpkgs revision pins native packages. Flakes or devenv would add structure not needed for this single environment. | Vendor toolchains remain separate. |
| A version manager such as mise | Broad host support and native display access. | Good for Node/Rust, but compiler, multilib, linker and GUI libraries still depend on separate OS packages. | Does not reproduce ESP-IDF by itself. |

We select Devbox for the common Linux workflow: it provides a familiar shell
entry point over Nix packages while retaining direct access to the host desktop.
It needs no container, flakes authored in this repository, or devenv services.
Adoption is optional.

## Version sources and CI

- `devbox.json` uses explicit Nix flake references to one immutable nixpkgs
  commit for Node 24, Clang/libclang,
  GCC/multilib, Mold, cargo-make, Trunk, CMake, Ninja, pkg-config and display
  libraries. The `nixpkgs.commit` setting pins the shell’s base packages to
  the same revision. No global channel or rolling package search determines
  these versions. Devbox maintains `devbox.lock`; retain generated resolutions
  when updating packages.
- `development/toolchains.env` pins Devbox stable and nightly Rust
  snapshots for both local work and CI.
- `package-lock.json` owns npm dependencies, including Prettier and the font
  converter; do not install another global copy.
- `Makefile.toml` owns the host/guest commands and font generation. Its
  `XILA_RUST_TOOLCHAIN` defaults to `stable` outside this shell, preserving manual
  setup. Devbox selects the shared pinned toolchain for both local work and CI.
- `.github/actions/setup/action.yml` installs the pinned Devbox CLI, restores
  the Nix/toolchain/build caches and runs the same setup script. Check and
  deployment jobs execute shell steps through `devbox run`, preserving their
  existing targets, features, formatting, coverage, documentation and Miri
  checks. Manual workflows do not require Devbox.
- Taplo stays at CI’s existing 0.9.3 version using a separate immutable nixpkgs
  reference in `devbox.json`, avoiding a formatter upgrade as part of this
  change. Coverage tooling is also supplied by Devbox.

To update the environment, review a new nixpkgs commit and Rust snapshots,
update the corresponding source files, then repeat the checks and GUI smoke
test below. Updating Taplo also updates CI. Rust remains subject to the
workspace's `rust-version` minimum; the browser example remains a nightly
workflow. The repository does not track Cargo lockfiles and has moving Git
dependencies, so this pins developer tools, not every application dependency
or a byte-for-byte build.

## Common validation commands

Inside the shell, after setup, run from the repository root:

```sh
cargo make check                 # Linux x86_64 host + wasm32-unknown-unknown guest
cargo make clippy               # Same targets; warnings denied
cargo make format-rust -- --check
cargo make format-toml --check
npm exec prettier -- --check "**/*.json"
cargo make build -p native_example
cargo build -p calculator --target wasm32-wasip1 --release
cargo make generate-fonts
(cd examples/wasm && RUSTUP_TOOLCHAIN="$XILA_NIGHTLY_TOOLCHAIN" trunk build)
```

Use `cargo make test` for workspace tests. For coverage, add
`rustup component add --toolchain "$XILA_RUST_TOOLCHAIN" llvm-tools-preview`.
For translation Miri and cross-endian checks, add
`rustup component add --toolchain "$XILA_NIGHTLY_TOOLCHAIN" miri rust-src`, then
use the commands in `.github/workflows/check.yml`. CI adds those components
where needed; they are not installed by the common setup.

## Native GUI and browser smoke tests

On a Linux Wayland desktop, enter the shell from a terminal in the logged-in
session. Devbox does not create a container or replace `WAYLAND_DISPLAY`,
`DISPLAY`, `XDG_RUNTIME_DIR`, the session bus or GPU device access. The shell
adds the runtime Wayland, xkbcommon, OpenGL and Vulkan loader libraries.
The host must already have a working compositor and GPU drivers; these are not
replaced by Nix.

```sh
test -n "$WAYLAND_DISPLAY"
test -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY"
cargo make run -p native_example
```

Confirm that a window appears, renders, accepts keyboard/mouse input and can be
closed. This is a manual acceptance check requiring a real display. Compilation
alone does not validate it. If Vulkan/OpenGL fails, check the host's GPU driver
installation and ICD paths; do not disable display access controls or use
`xhost +`. X11-only sessions are not claimed supported by the current native
event loop. NixOS may additionally need its normal nix-ld/graphics configuration
for rustup binaries and vendor GPU drivers; it is not yet a validated platform.

To test in the host browser:

```sh
cargo build -p calculator --target wasm32-wasip1 --release
cargo make generate-fonts
(cd examples/wasm && RUSTUP_TOOLCHAIN="$XILA_NIGHTLY_TOOLCHAIN" trunk serve)
```

Open the URL printed by Trunk and check rendering and input. The explicit
nightly selection overrides the shell's stable `RUSTUP_TOOLCHAIN` for that
command only.

## Platform and hardware boundaries

| Contributor platform | Expectation |
| --- | --- |
| x86_64 Linux with a Wayland desktop | Initial Devbox target; run the commands and GUI smoke test above. |
| Windows x86_64 | Use WSL2 Linux for the same commands. WSLg and compatible GPU drivers are required for GUI access; this needs separate validation. Native Windows Nix/GUI builds are not supported here. |
| macOS, including Apple Silicon | Nix itself supports macOS, but this environment only supports Linux: the Linux host target, multilib setup and Wayland APIs are not a validated macOS workflow. Use existing manual WASM setup or a Linux machine. |
| ARM Linux | Not supported by this initial x86_64 shell; adapting the host target and multilib setup needs separate validation. |

ESP-IDF, Xtensa/ESP32 and RISC-V vendor toolchains, flashing utilities, USB/serial
permissions, SDK versions and hardware execution remain outside this shell.
Use the esp-rs installation guides linked in the root README and the repository's
target-specific manifests/configuration. Leave this shell before entering a
vendor environment so its isolated Rust homes and toolchain selection do not
override that setup. Host and WASM success does not establish ESP32, nRF,
STM32 or any other hardware-target correctness.
