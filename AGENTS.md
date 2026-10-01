# Project Overview

Xila is an embedded operating system written in Rust for low-power microcontrollers, including ESP32, nRF, and STM32 targets. This Cargo workspace separates reusable functionality into `modules/`, platform support into `drivers/`, and applications/runtimes into `executables/`; code must account for constrained RAM, flash, and target capabilities.

# Build & Test Commands

- Format Rust: `cargo make format-rust -- --check`
- Format TOML: `cargo make format-toml --check`
- Format JSON: `npm exec prettier -- --check "**/*.json"`
- Run host and guest checks: `cargo make check`
- Run host and guest Clippy: `cargo make clippy`
- Test the workspace: `cargo make test`
- For focused validation, use `cargo make check-host`, `cargo make check-guest`, `cargo make clippy-host`, or `cargo make clippy-guest` as appropriate.

These tasks are defined in `Makefile.toml`; CI is the source of truth for target-specific validation.

# Architecture & Implementation Guidelines

- Put changes in the crate that owns the behavior and follow its existing APIs, error handling, feature flags, and tests.
- Keep reusable modules portable: preserve `no_std` operation and isolate host-only code and dependencies behind target-specific Cargo sections or conditional compilation.
- Check a crate's `Cargo.toml`, features, and consumers before changing dependencies or enabling features. Preserve intentional `default-features = false`, optional dependencies, and target-specific dependency boundaries.
- Treat every byte as important: consider RAM, flash, stack, and peak allocation costs. Prefer bounded or caller-provided storage where appropriate; account for capacity, lifetime, and allocation behavior when using `Vec`, `String`, or `Box`. Avoid unnecessary allocations, copies, buffers, large static data, and dependencies.
- Keep target support explicit. Put hardware-specific adaptation in the relevant driver and keep reusable OS behavior in modules.
- Identify the generator or build script for generated sources and assets before changing them. Keep generated files and build output out of source changes unless the task explicitly requires updating generated assets.

# Non-Negotiables

- Make the smallest focused change that follows the existing crate structure and conventions.
- Validate changes with the narrowest relevant checks, using CI's host/guest checks as the source of truth for those targets. Host and guest checks do not establish hardware-target correctness; report what was actually checked and any target/toolchain limitations.
- Do not assume documentation in adjacent repositories is current; prefer this repository's source, manifests, and CI configuration as ground truth.
