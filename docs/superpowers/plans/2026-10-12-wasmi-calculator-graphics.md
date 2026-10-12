# Wasmi Calculator Graphics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run Calculator's existing graphics ABI on Wasmi using context-provided capabilities and VM-independent, testable bindings.

**Architecture:** Share LVGL ABI discovery and call ordering in a build-only generator crate. Keep graphics dispatch, object ownership, and checked marshalling separate from the thin Wasmi adapter. Add cooperative clock polling only to the extent required by Calculator, then exercise the actual application.

**Tech Stack:** Rust 2024, `no_std` + `alloc` runtime modules, Wasmi 1.1, LVGL, `syn`/`quote` build-time generation, Cargo tests, LLVM coverage, Miri.

**Spec:** `docs/superpowers/specs/2026-10-12-wasmi-calculator-graphics-design.md`

## Global Constraints

- Keep existing guest wrappers and `host::xila_graphics_call` compatible; preserve full-catalog IDs, not subset indices.
- No WAMR dependencies in Wasmi's runtime or build path; no generated-source edits.
- Resolve graphics/task/time capabilities through the supplied context, never graphics globals.
- VM-independent graphics code must not reference `Caller`, `Store`, or `Linker`.
- Use checked borrowed bytes and fixed-width little-endian encoding, not typed guest-pointer casts.
- Limits: 256 issued object handles per execution; 64 map entries; 1024 bytes including NUL per string; 8192 retained map-string bytes per widget.
- Do not reuse dead handles within an execution. Invalidate handles on parent and external deletion.
- Validate complete inputs/outputs before side effects; unsupported calls return nonzero status.
- No allocations/copies/dynamic dispatch introduced merely for VM separation. Retained maps and lifetime bookkeeping are explicit bounded allocations.
- Preserve 100% line/function coverage for existing translation and new separable marshalling; report native/hardware validation separately.
- Keep user-owned documentation untouched. Do not push, rewrite history, or change unrelated applications.

## Review Focus

- A retained map remains valid after guest memory grows or the guest modifies its original bytes: Task 2 tests stable host ownership.
- Parent/external deletion followed by native address reuse cannot revive an old handle: Task 3 tests callback invalidation and non-reuse.
- Bad/overlapping event outputs must not pop an event or create aliased references: Task 3 tests sequential writes and no mutation on failure.
- A sleeping/instruction-heavy guest must not starve the graphics task: Task 4 tests scheduler progress during both suspension kinds.
- Generator refactoring must not renumber unsupported calls and therefore shift supported IDs: Task 1 compares the entire legacy catalog before/after.

## File Structure

- `modules/graphics/bindings_generator/`: new build-only `graphics_bindgen` crate; discovery, filtering, names, ordered call metadata, generator tests.
- `executables/wasm/build/{host.rs,guest.rs,utilities/}`: reuse shared catalog/naming; retain runtime-specific legacy emitters.
- `executables/wasm_2/build/`: generate the compatible catalog and explicit supported-call metadata, independent of Wasmi types.
- `executables/wasm_2/src/host/graphics/{mod.rs,error.rs,marshal.rs,objects.rs,dispatch.rs,wasmi.rs}`: VM-independent core plus a separate VM adapter.
- `executables/wasm_2/src/host/translation/{memory.rs,vm.rs,mod.rs}`: separate the existing VM lookup helpers from the portable borrowed memory view; public API unchanged.
- `executables/wasm_2/tests/translation/`: extend the source-including harness for pure graphics marshalling coverage.
- `executables/wasm_2/src/host/{store.rs,main.rs,wasi/poll.rs,wasi/register.rs,wasi/scheduling.rs}`: per-store suspension and cooperative execution.
- `executables/wasm_2/tests/{graphics_core.rs,graphics_adapter.rs}`: direct native-core checks and Wasmi adapter integration checks respectively.
- `executables/calculator/{Cargo.toml,src/interface.rs,tests/test.rs}`: Wasmi host test and an explicit close path, preserving guest graphics imports.
- `executables/wasm_2/README.md`: supported calls, limits, automated commands, interactive calculator procedure, known limitations.

---

### Task 1: Shared, readable ABI catalog

**Files:** Create `modules/graphics/bindings_generator/{Cargo.toml,src/lib.rs,src/catalog.rs,src/naming.rs,tests/catalog.rs}`; modify root workspace manifest, `executables/wasm/Cargo.toml`, and legacy build utilities/emitters.

**Interfaces:**
- Consume raw LVGL Rust bindings as `&str`; no runtime or LVGL native library dependency is necessary for the catalog unit tests.
- Produce `CallCatalog::parse(lvgl_source: &str) -> Result<CallCatalog, GeneratorError>` and `CallCatalog::calls(&self) -> &[Call]`.
- `Call` records `id: u16`, `signature: syn::Signature`, normalized guest name/variant, public argument count, and host-only argument names. Expose legacy-compatible naming/filtering helpers from this crate.
- `MAX_ARGUMENTS: usize = 7`; deterministic lowercase-name ordering includes unsupported calls.

- [ ] **Step 1: Write catalog regression tests.** Capture the pre-refactor filtered signatures/order using the current generator logic and compare every `(name, id)` against `CallCatalog`. Assert Calculator's 15 required calls remain present; `object_delete` has one public argument despite its host-only translator; duplicate normalized names, argument counts over seven, callbacks, and variadic unsafe-emitter candidates receive explicit diagnostics/policies.
- [ ] **Step 2: Run the new tests and confirm failure before implementation.** Run `cargo +stable test -p graphics_bindgen --target x86_64-unknown-linux-gnu`; a missing crate/API is the expected initial failure.
- [ ] **Step 3: Implement the catalog and adapt legacy consumers.** Move existing naming/filtering/helper rules rather than copy them; maintain enum ordering and guest import types. Separate discovery/order from code emission. Keep legacy translator-specific behavior in its emitter. Check formatter subprocess success instead of ignoring a nonzero exit.
- [ ] **Step 4: Verify compatibility.** Run the catalog tests, build Calculator for `wasm32-wasip1`, and compare generated enum/guest call signatures to the captured baseline. Run a focused WAMR library check where its toolchain permits; report any WAMR-specific blocker.
- [ ] **Step 5: Commit only Task 1 files.** Message: `Share LVGL ABI discovery and simplify generator metadata`.

### Task 2: Portable checked graphics marshalling

**Files:** Create graphics `error.rs` and `marshal.rs`, separate translation VM helpers into `vm.rs`, extend `tests/translation/src/lib.rs` and its README.

**Interfaces:**
- Keep `GuestMemory::new(&mut [u8])`, `read`, `write`, `bytes`, `validate_pointer` unchanged; move only `get_memory`/`borrow_memory` to the VM-specific module and re-export them.
- `GraphicsError` is `#[repr(i32)]`; statuses: invalid count = 1, invalid memory = 2, unavailable capability = 3, unsupported call = 4, invalid handle = 5, resource limit = 6, invalid value = 7, native failure = 8.
- Produce `read_c_string(memory: &GuestMemory<'_>, offset: WasmUsize) -> Result<&core::ffi::CStr, GraphicsError>` with the explicit 1024-byte limit (attach the return lifetime to the view borrow).
- Produce `ButtonMap::decode(memory: &GuestMemory<'_>, table: WasmUsize) -> Result<ButtonMap, GraphicsError>` and `ButtonMap::as_ptr(&self) -> *const *const core::ffi::c_char`.
- `ButtonMap` owns stable string storage and a native pointer table; null or empty-string guest terminators become an empty-string native terminator. No pointers refer into guest memory or movable inline storage.

- [ ] **Step 1: Add failing source-including tests.** Verify bounded C-string success/failure at null/end/1024 bytes, unaligned pointer-table entries, missing table terminators, out-of-range strings, checked arithmetic overflow, 64-entry and 8192-byte limits, row separators, both supported terminators, and pointer stability after moving `ButtonMap` and mutating/dropping backing guest bytes. Run both guest widths.
- [ ] **Step 2: Run focused tests and confirm failures.** Use `cargo +stable test --manifest-path executables/wasm_2/tests/translation/Cargo.toml` and the `memory_64` variant.
- [ ] **Step 3: Implement bounded adapters.** Decode guest words by value. Validate before installing maps. Allocate retained storage only for a valid map, without repeated unbounded string scans. Do not add a second guest-memory range implementation.
- [ ] **Step 4: Verify coverage, Miri, and portability.** Run the existing 100% line/function gate for both widths with `marshal.rs` included; run existing borrowing doctests and Miri commands. Preserve `no_std` checking of the standalone harness.
- [ ] **Step 5: Commit only Task 2 files.** Message: `Add bounded VM-independent graphics memory adapters`.

### Task 3: Context-owned graphics dispatch and Wasmi registration

**Files:** Add Wasmi build generation, graphics `mod.rs`, `objects.rs`, `dispatch.rs`, `wasmi.rs`, core/adapter tests; modify Wasmi manifest, host module, store and linker setup.

**Interfaces:**
- `GraphicsCall { function: i32, arguments: [WasmUsize; 7], count: i32, result: WasmUsize }` contains only ABI values.
- `GraphicsState::new(manager: Option<&'static xila::graphics::Manager>) -> GraphicsState`; owns issued handles, deletion notification storage, root ownership, and per-widget maps. Its destructor locks that same manager and cleans owned roots before freeing callback data.
- `dispatch(state: &mut GraphicsState, memory: &mut GuestMemory<'_>, call: GraphicsCall) -> Result<(), GraphicsError>` is VM-independent and locks the state's manager for all native operations.
- `graphics::wasmi::add_to_linker(linker: &mut wasmi::Linker<GlobalStore>) -> crate::host::error::Result<()>` registers only the compatible `host::xila_graphics_call` import.
- Adapter arguments: `Caller<GlobalStore>`, integer function/count, seven guest words, and guest-word result pointer; return integer status. Split Wasmi memory/store borrowing once, then call `dispatch` without retaining the borrow.
- Feature `graphics = ["xila?/graphics"]`; build-only parsing dependencies remain optional/feature-gated. Enable graphics explicitly in the calculator host test, not in non-graphical runtime defaults.

- [ ] **Step 1: Write failing direct-core tests.** With a headless test graphics manager, exercise Calculator's exact call set; verify invalid counts/IDs, absent manager, forged/stale handles, widget-class checks, 256-handle exhaustion, parent/external deletion, non-reuse, two-state isolation, empty events, invalid outputs without popping, sequential overlapping outputs, map replacement, and teardown on failure.
- [ ] **Step 2: Write failing adapter tests.** A small WAT guest imports the actual call signature and exports memory. Verify registration, missing/wrong-kind memory, context capability extraction, create/update calls, and guest memory growth after map installation. Use the generated full-catalog IDs, not hardcoded subset indices.
- [ ] **Step 3: Run tests to confirm failure.** Run `cargo +stable test -p wasm_2 --features graphics --target x86_64-unknown-linux-gnu --test graphics_core` and `--test graphics_adapter`.
- [ ] **Step 4: Implement core dispatch and thin registration.** Use an explicit supported-call/adapter table validated against catalog signatures. Validate result sizes before native creation. Store stable deletion callback data without aliasing mutable Rust references across LVGL callbacks. Resolve event targets by pointer comparison with live issued entries, never dereferencing queued target pointers. Do not expose the global screen through a null handle. Validate FlexFlow encodings and UTF-8 window icons; retain the existing packed color ABI.
- [ ] **Step 5: Verify core and adapter tests, feature checks, and source boundaries.** Check non-graphics host, graphics host, and `memory_64` graphics compilation. Search the core for Wasmi types/global lookups; native calls belong behind checked handles and the context manager lock. Re-run library tests.
- [ ] **Step 6: Commit only Task 3 files.** Message: `Add context-owned calculator graphics bindings for Wasmi`.

### Task 4: Cooperative clock polling and guest execution

**Files:** Add `wasi/poll.rs`, update register, scheduling, store, runner; add polling codec tests to the standalone harness and resumable execution tests to the adapter suite.

**Interfaces:**
- Produce `PendingCall::{Yield, ClockPoll(ClockPoll)}` stored as `Option<PendingCall>` in `GlobalStore`.
- `ClockPoll` owns only decoded bounded clock subscriptions and guest output handles, not guest borrows. Limit this initial adapter to 64 subscriptions.
- `poll_oneoff(caller: Caller<GlobalStore>, subscriptions: WasmUsize, events: WasmUsize, count: WasmUsize, written: WasmUsize) -> Result<i32, wasmi::Error>` validates outputs and decodes the WASI layouts before recording suspension. Support realtime/monotonic clock subscriptions with relative and absolute deadlines; return controlled errors for FD polling, invalid flags/clock IDs, zero/excessive counts, and arithmetic overflow.
- Subscription ABI: 48 bytes; userdata offset 0, tag 8, clock ID 16, timeout 24, precision 32, flags 40. Event ABI: 32 bytes; userdata 0, errno 8, tag 10, zeroed remaining padding/FD fields.
- VM-specific host errors identify intentional suspension, not guest exit or ordinary failure. Resume a successful suspended WASI call with `[wasmi::Val::I32(0)]` after writing validated events. Reacquire memory only after the async wait; never carry a guest-memory borrow across an await.

- [ ] **Step 1: Inspect compiled Calculator imports and add failing polling/scheduling tests.** Confirm `poll_oneoff` usage. Test the explicit LE record fixtures, unaligned records, limit/error cases, no writes before validation, two clock subscriptions, deadline overflow, and distinction between sleep/yield/exit/unhandled traps. Spawn a competing task and assert it makes progress while the guest sleeps and while repeatedly exhausting fuel.
- [ ] **Step 2: Run focused tests and confirm failures.** Use the standalone harness for pure codecs and the Wasmi adapter test binary for resumable execution.
- [ ] **Step 3: Implement checked clock polling and asynchronous resume.** Use the context time manager for deadlines and the task scheduler for waits. Set fuel before executing guest start code where applicable. Yield between fuel replenishments; make `sched_yield` suspend instead of using synchronous `block_on(yield_now())`. Preserve existing `proc_exit` behavior and ordinary traps.
- [ ] **Step 4: Verify tests, coverage for separable codecs, and existing WASI fixture.** Run focused tests plus existing runtime library tests. Confirm no executor-blocking sleep or graphics lock spans an await. Report clock-poll-only support explicitly.
- [ ] **Step 5: Commit only Task 4 files.** Message: `Resume Wasmi clock polling and fuel exhaustion cooperatively`.

### Task 5: Calculator acceptance and documentation

**Files:** Update Calculator manifest, interactive test, interface close path; add Wasmi README and deterministic calculator acceptance fixture in the graphics adapter test suite.

**Interfaces:**
- Preserve Calculator's target-specific guest dependency and graphics wrapper calls. Change its host-test runtime dependency/registration to `wasm_2` with `graphics` enabled; keep mounted executable path `/binaries/wasm` compatible with existing launch conventions.
- Emit `EVENT_CLOSE_REQUESTED: u32` through the guest generator from this build's LVGL `LV_EVENT_LAST` plus the existing Xila offset of three. Do not add a new call to the sorted catalog or hardcode a version-dependent event number in Calculator.
- Use a `u32` event-code output variable in Calculator and compare to `EventCode::Clicked as u32`; writing a custom event discriminant into the existing Rust `EventCode` enum would be invalid.
- The calculator event loop returns on close request; normal VM teardown owns deletion. Return/propagate graphics failures rather than silently looping forever on failed binding calls.

- [ ] **Step 1: Add failing acceptance checks.** Exercise the real calculator guest by injecting button events for `1 + 2 =`, observing display text containing `3`, then a close request. Assert a successful exit and restored window count. Verify the calculator window remains responsive without console input.
- [ ] **Step 2: Run acceptance test and confirm failure.** Build the actual guest with `RUSTUP_TOOLCHAIN=stable cargo build -p calculator --profile release-wasm --target wasm32-wasip1`; run the graphics adapter acceptance test.
- [ ] **Step 3: Wire the interactive test and minimal close handling.** Keep parser/evaluator/installation changes out of scope. Document retained-map limits, unsupported graphics calls, and unsupported FD polling.
- [ ] **Step 4: Final verification.** Run catalog tests, standalone tests/coverage for both widths, Miri, Wasmi library/core/adapter tests, focused host and guest checks, Rust/TOML formatting, and `git diff --check`. Run `RUSTUP_TOOLCHAIN=stable cargo test -p calculator --target x86_64-unknown-linux-gnu --test calculator_test -- --ignored --nocapture` in a graphical session if available; verify arithmetic and close. If no graphical session is available, report that rather than claim manual acceptance.
- [ ] **Step 5: Commit only Task 5 files.** Message: `Test Calculator on Wasmi and document graphics support`.

## Self-review

The five tasks cover catalog compatibility, VM-independent boundaries, bounded
marshalling, object/map lifetime, event semantics, context resolution, cooperative
sleep/fuel, close/cleanup, and Calculator acceptance. Test interfaces use the
same types as the consuming tasks. Review-focus cases are assigned above.
No full LVGL migration, general polling, native struct pointer ABI, or unrelated
application work is included.

## Execution progress

- [x] Task 1: shared ABI generator (`ce6c554`); guest outputs preserved, with
  an independent frozen full-catalog compatibility check added during acceptance.
- [x] Task 2: portable graphics marshalling (`63cd17f`); 100% line/function
  coverage and Miri on both widths and big-endian host simulation.
- [x] Task 3: graphics core and thin Wasmi adapter (`69394c0`); headless native
  tests cover ownership, event targets, deletion, capability isolation, and maps.
- [x] Task 4: cooperative polling/execution (`0e8e3a9`); scheduler-progress,
  trap/exit, clock-ID, and portable record tests pass.
- [x] Task 5: real Calculator acceptance; automated `1 + 2 = 3`, close/cleanup,
  and native graphical-session startup/console-close smoke test pass. Arithmetic
  was automated headlessly, not manually clicked in the native display.
- [ ] Final independent review and any important fixes.

Implementation choices: retain the calling instance's Wasmi Memory handle in
pending clock calls (never a guest borrow); do not address the unrelated console
EOF read-count bug in this graphics port. The closed-stdin integration variant
failed; the same fixture with a line of input passes. Unsupported polling kinds
and graphics calls remain explicit errors, as designed.
