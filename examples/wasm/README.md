# Browser example

From the repository root, build the Calculator guest first, generate the LVGL
fonts, and then build the browser example:

```sh
cargo +stable build -p calculator --target wasm32-wasip1 --release
cargo make generate-fonts
cargo +stable build -p wasm_example --target wasm32-unknown-unknown
```

To serve it in a browser, install [Trunk](https://trunkrs.dev/) and run:

```sh
cd examples/wasm
RUSTUP_TOOLCHAIN=stable trunk serve
```

Use localhost or HTTPS in a browser supporting OPFS,
`FileSystemFileHandle.createWritable`, and Web Locks.

## Running guest WebAssembly applications

Calculator is embedded from `target/wasm32-wasip1/release/calculator.wasm` at
browser build time. Startup copies it to `/binaries/calculator` and runs its
installer to create an application-drawer shortcut. Rebuild the guest before
rebuilding the browser example whenever Calculator changes. Guest builds must
place this artifact in the repository's default `target` directory.

On a fresh disk, log in as **administrator** with an empty password; otherwise
use your saved credentials. Open the application drawer using the Xila button
in the dock. Select
**Calculator**. It can also be launched from Xila's terminal:

```text
/binaries/wasm /binaries/calculator
```

The example mounts the Wasmi-backed runtime at `/binaries/wasm`, with the same
graphics bindings as the native example. For other guests that are already
present in Xila's virtual filesystem:

```text
/binaries/wasm /path/to/guest.wasm
```

WASI Preview 1 command guests should export `_start: () -> ()`. Use
`/binaries/wasm --install /path/to/guest.wasm` to call `__install: () -> ()`
instead. Graphical guests must use compatible Xila graphics bindings; see the
[runtime documentation](../../executables/wasm/README.md) for supported APIs and
limits.

## Browser limitations

- The example persists its filesystem in OPFS; see the storage and durability
  limits below. Startup reinstalls the bundled Calculator. Other guest files
  must be present before launching; there is no upload mechanism.
- Embedding Calculator increases the browser WASM download and retains the
  embedded bytes alongside the VFS copy. Running it also needs Wasmi's module
  and guest memory. This simple approach favors convenience over RAM usage.
- Guests run cooperatively in Wasmi on the browser's executor, not as separate
  native browser WebAssembly instances. Fuel exhaustion and supported WASI
  suspension calls yield to the executor.
- There is no Xila network manager in this example. The browser HTTP device
  uses Fetch and remains subject to browser security restrictions, including
  CORS; it does not provide raw sockets.
- Runtime limitations still apply, including the supported graphics subset,
  clock-only WASI polling, unsupported suspension in WebAssembly start sections,
  and the currently unused stack-size option.

Per-call WASI diagnostics are logged at Trace level, which the browser logger
does not display. Informational messages, warnings, and errors remain visible.

## Browser verification

Verified in headless Chromium 154 with a Trunk development build using Rust
stable. Temporary startup probes loaded guest files into the example's VFS and
launched them through the mounted `/binaries/wasm` executable:

- A minimal guest executed a host call, exhausted and replenished its fuel,
  resumed after WASI `sched_yield`, and handled success and nonzero `proc_exit`.
- The real `calculator` guest, built for `wasm32-wasip1`, evaluated `1 + 2 = 3`
  through LVGL button events, used WASI clock polling, and exited successfully
  with its window removed after a close request.

Calculator bundling was additionally checked with a temporary probe that read
the startup-created guest and shortcut without creating either, then ran the
bundled guest through the same evaluation and teardown checks.

The probes are not included in normal example startup; Calculator is. Generated
guest binaries are not committed. This is focused browser validation, not a
claim of full WASI/graphics compatibility or support across all browsers.

## Persistent storage

The example stores its 16 MiB MBR/LittleFS disk in the origin-private file
`xila_drive.img`. Files, configuration, and credentials survive reloads on the
same origin. The default administrator with an empty password is created only
on a fresh disk. Existing users/groups are loaded even if the administrator
account was removed. Invalid MBRs or filesystem mount errors stop boot without
reformatting an existing image. A Web Lock allows only one Xila tab to open
this disk at a time; close the first tab before opening another.

Synchronous OPFS handles are available only in workers, while this example's
UI and filesystem run on the browser main thread. The drive therefore keeps a
16 MiB memory cache and asynchronously commits an atomic disk snapshot at boot,
every second when dirty, and on normal shutdown. Saving temporarily needs
another 16 MiB snapshot, plus browser-internal buffering. There is no growing
queue of disk snapshots. Clean disks are not rewritten.

**Durability:** synchronous filesystem flushes update the cache, not OPFS.
`DriveDevice::sync().await` waits for an OPFS commit and reports storage errors;
autosave errors are logged and retried. Abrupt reloads, tab closure, or crashes
can lose changes since the last successful save. Background-tab timer throttling
can increase that window. OPFS is browser-managed storage: quota limits, private
browsing, clearing site data, or browser eviction can remove it. This is not a
substitute for backups. Changing the disk size does not silently resize an
existing image.

To reset the demo, clear its site data in browser settings. Alternatively, close
all Xila tabs and run this on a separate page on the same origin in the console:

```javascript
const root = await navigator.storage.getDirectory();
await root.removeEntry("xila_drive.img");
```

## Browser storage tests

Install `wasm-bindgen-cli` matching the resolved `wasm-bindgen` dependency and a
Chrome/Chromium WebDriver matching your browser, then run from the repository root:

```sh
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
CHROMEDRIVER=/path/to/chromedriver \
cargo test -p drivers_wasm --target wasm32-unknown-unknown --test opfs
```

For a non-default browser binary, set `WASM_BINDGEN_TEST_WEBDRIVER_JSON` to a JSON
file containing `{"goog:chromeOptions":{"binary":"/path/to/chromium"}}`.
These tests use real OPFS files, including MBR/LittleFS save/reopen coverage.
