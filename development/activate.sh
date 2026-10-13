# Sourced by Devbox; keep display/session variables inherited from the host.
if [ "$(uname -s)" != Linux ] || [ "$(uname -m)" != x86_64 ]; then
  echo "Xila's Devbox environment currently requires x86_64 Linux (or WSL2)." >&2
  return 1
fi

export XILA_DEV_ROOT="$DEVBOX_PROJECT_ROOT"
export CARGO_HOME="$XILA_DEV_ROOT/.dev-environment/cargo"
export RUSTUP_HOME="$XILA_DEV_ROOT/.dev-environment/rustup"
export npm_config_cache="$XILA_DEV_ROOT/.dev-environment/npm-cache"
. "$XILA_DEV_ROOT/development/toolchains.env"
export XILA_RUST_TOOLCHAIN XILA_NIGHTLY_TOOLCHAIN
export RUSTUP_TOOLCHAIN="$XILA_RUST_TOOLCHAIN"
export PATH="$CARGO_HOME/bin:$PATH"

xila_profile="$XILA_DEV_ROOT/.devbox/nix/profile/default"
export LIBCLANG_PATH="$xila_profile/lib"
export LD_LIBRARY_PATH="$xila_profile/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
unset xila_profile
