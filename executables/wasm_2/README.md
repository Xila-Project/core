# Wasmi runtime

`wasm_2` is the Wasmi-based runtime. The original `wasm` runtime remains available;
Calculator still uses its guest wrappers, with compatible generated call IDs.

## Test Calculator

From the repository root on Linux:

```sh
rustup target add --toolchain stable wasm32-wasip1

# Headless acceptance: builds the real guest, enters 1 + 2 = through LVGL
# button events, checks the displayed result, requests close, checks cleanup.
cargo +stable test -p wasm_2 --features graphics \
  --target x86_64-unknown-linux-gnu --test calculator

# Interactive native display. Click calculator buttons; press Enter in the
# launching terminal to request close and let the test finish successfully.
RUSTUP_TOOLCHAIN=stable cargo test -p calculator \
  --target x86_64-unknown-linux-gnu --test calculator_test \
  -- --ignored --nocapture
```

The interactive test requires a working native graphical session. Its terminal
close path sends Xila's window-close event, rather than abruptly killing the VM.
The guest handles that event as a numeric code, not an invalid Rust enum variant.

Other focused tests:

```sh
cargo +stable test -p graphics_bindgen --target x86_64-unknown-linux-gnu
cargo +stable test -p wasm_2 --features graphics --target x86_64-unknown-linux-gnu
cargo +stable test -p wasm_2 --no-default-features \
  --features host,graphics,memory_64 --target x86_64-unknown-linux-gnu
```

Calculator is a wasm32 guest, so its acceptance test is not run in the memory64
configuration. Core/adapter tests cover both guest-word widths. Portable memory
tests, 100% line/function coverage commands, and Miri commands are documented in
[`tests/translation/README.md`](tests/translation/README.md).

## Graphics architecture

- `modules/graphics/bindings_generator` discovers and orders one shared LVGL ABI
  catalog; unsupported calls keep their IDs. Wasmi's supported-call table checks
  full C signatures, not just argument counts. The catalog is compatible with
  guest bindings built from the same LVGL revision/configuration, not a stable
  cross-version ABI.
- `src/host/graphics/{dispatch,objects,marshal}.rs` use explicit graphics state
  and checked borrowed guest memory. They do not use Wasmi types or global
  graphics-manager lookups. The thin `graphics/wasmi.rs` adapter alone handles
  imports, exports, memory/store borrowing, and context capability checks.
- Object handles are local to the VM. Deleted handles are never reused during
  execution. Native deletion notifications invalidate handles and remove stale
  queued event targets. Teardown deletes owned windows and their children using
  the same context-provided manager and lock, including on traps.
- LVGL copies label strings synchronously. Button-map strings and the native
  pointer table are copied into stable VM-owned storage because LVGL retains
  them. Guest memory growth/mutation does not invalidate the map. A replacement
  switches LVGL to the new map before releasing the old map.

Enable the optional `graphics` feature for host bindings. Select the Xila
rendering feature appropriate to the platform (e.g.
`xila/graphics_rendering_xrgb8888` for a standalone host library check). Normal
non-graphical runtime use does not enable graphics.

### Supported calls and limits

This first milestone supports windows (creation, icon, event pop), object
creation/deletion, flex flow/grow, width/height, flags, labels (creation/text),
button matrices (creation/map/selection), and percentage conversion.

- 256 issued object handles per execution, including subsequently deleted ones.
- 64 map entries including the sentinel; 1024 bytes per NUL-terminated string.
- 8192 retained map-string bytes per widget, including the native sentinel.
- Null and empty-string guest map sentinels both become LVGL's empty-string
  terminator. Row separators are preserved.

Return status zero means success. Errors are nonzero: invalid count 1, invalid
memory 2, unavailable capability 3, unsupported call 4, invalid handle 5,
resource limit 6, invalid value 7, native failure 8.

Calls requiring arbitrary native structure layouts, guest callbacks, retained
guest pointers, fonts/images/charts, or unsupported LVGL APIs return unsupported.
No typed host reference is formed from guest memory.

## Scheduling and remaining limitations

WASI `poll_oneoff` supports up to 64 realtime/startup-clock subscriptions,
relative and absolute deadlines. It validates explicit little-endian records
and outputs, suspends the VM, asynchronously waits, then reacquires memory and
resumes with ready events. FD polling and unsupported clocks return errors.
The startup-clock implementation follows the existing Xila time manager; it is
not an independent hardware monotonic-clock source.

Fuel exhaustion and `sched_yield` yield to the executor. Clock polling uses
resumable Wasmi host calls; suspension is VM-local. WebAssembly start-section
suspension is not supported; WASI command guests should use exported `_start`.
Installation and the command-line stack-size option remain unimplemented.

The existing stdin fixture passes with a line of input. Running it with closed
stdin (`/dev/null`) exposes an unrelated `drivers_std::StandardInDevice` bug:
it discards the actual read count and reports the buffer length even at EOF.

Host tests and Miri do not establish embedded hardware correctness or firmware
RAM/flash costs. Native LVGL integration tests are separate from the portable
translation coverage gate; no full LVGL/WASI compatibility is claimed.
