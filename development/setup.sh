#!/usr/bin/env bash
set -euo pipefail

development_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "${XILA_DEV_ROOT:-}" != "$development_root" ||
      "${CARGO_HOME:-}" != "$development_root/.dev-environment/cargo" ||
      "${RUSTUP_HOME:-}" != "$development_root/.dev-environment/rustup" ]]; then
  echo "Enter devbox shell from the repository root before running this script." >&2
  exit 1
fi
cd "$development_root"
. "$development_root/development/toolchains.env"
export RUSTUP_TOOLCHAIN="$XILA_RUST_TOOLCHAIN"

# The shell's isolated Rust homes keep installation out of the user's setup.
rustup toolchain install "$XILA_RUST_TOOLCHAIN" --profile minimal --component rustfmt,clippy
rustup target add --toolchain "$XILA_RUST_TOOLCHAIN" wasm32-unknown-unknown wasm32-wasip1
rustup toolchain install "$XILA_NIGHTLY_TOOLCHAIN" --profile minimal --component rust-src
rustup target add --toolchain "$XILA_NIGHTLY_TOOLCHAIN" wasm32-unknown-unknown
npm ci

echo "Setup complete. Run cargo make check, or see readme.md."
