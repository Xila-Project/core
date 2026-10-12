# Calculator graphics on Wasmi

Status: approved by the user; implementation planning in progress.

## Intent

Port the LVGL/graphics path from `executables/wasm` to `executables/wasm_2`,
using the context available through Wasmi rather than resolving graphics through
global state. The user chose Calculator as the first application to test.
The user also requested improving the LVGL binding generator's readability and
simplicity alongside the port.
The user prefers bindings decoupled from the VM for testing, provided this does
not impose substantial resource costs.

Success for this milestone means the calculator opens, renders its display and
button matrix, accepts button clicks, evaluates an expression, and updates its
display under Wasmi. The graphics task must remain responsive while the guest
is running or sleeping. Windows must be cleaned up when execution ends or traps.

## Scope and compatibility

Builder's proposed choice: retain the existing calculator guest graphics API and
`host::xila_graphics_call` import for this milestone. The calculator can continue
to use `wasm`'s guest wrappers; its host test uses `wasm_2::WasmExecutable` instead
of WAMR. This avoids mixing a guest API migration with the host-runtime port.
The old runtime and guest generator remain available for reference.

Preserve the old call IDs by using the same filtered LVGL signatures, additional
helper signatures, and sorting rules as the existing generator. Do not silently
renumber a supported subset. Compatibility is with guest bindings generated from
this repository's same LVGL revision/configuration, not an independently stable
cross-version ABI. Do not edit generated source or generated assets.

The calculator-required calls are:

- `window_create`, `window_set_icon`, `window_pop_event`, `percentage`;
- `object_create`, `object_set_flex_flow`, `object_set_flex_grow`,
  `object_set_height`, `object_set_width`, `object_add_flag`;
- `label_create`, `label_set_text`;
- `buttonmatrix_create`, `buttonmatrix_set_map`,
  `buttonmatrix_get_selected_button`.

Also provide explicit deletion/teardown needed to manage object lifetimes safely.
Calls outside the supported set return a documented nonzero unsupported status.
Malformed IDs are integers, never transmuted to Rust enums. Success is zero;
every error is nonzero (the old error enum accidentally assigns zero to its
first error and must not be copied as-is).

## Context and capability boundary

Separate the VM adapter from the graphics binding implementation. The Wasmi
adapter alone handles `Caller`, `Store`, `Linker`, export lookup, and suspension.
It extracts the context's graphics capability, per-execution graphics state,
borrowed linear memory, and integer call arguments, then invokes a dispatcher.
The dispatcher, object registry, and marshalling adapters must not use Wasmi
APIs or resolve capabilities from globals. Their inputs are explicit Rust
references, borrowed bytes, and fixed-width ABI values, allowing tests without
constructing a VM. Wasmi-specific registration is generated/emitted separately.

Builder's proposed choice: begin with separate modules rather than adding a
runtime framework or requiring all runtimes to migrate. Extract a small shared
`no_std` core crate only if that makes source reuse or isolated testing simpler.
Reuse the existing checked-memory implementation where practical; avoid copying
its logic or reintroducing unsafe conversions to achieve separation. A separate
implementation module is acceptable for this milestone if its public boundary
is VM-independent and its sources can be tested independently.

The separation itself must introduce no heap allocations, guest-buffer copies,
dynamic dispatch, or extra per-object storage. Retained button maps and existing
object-lifetime bookkeeping are inherent graphics costs, not an abstraction
cost. Do not add a generic VM trait hierarchy or bring WAMR into Wasmi's path.

Graphics registration is feature-gated. The feature enables the corresponding
Xila/executable graphics capability without making graphics mandatory for
non-graphical Wasmi use.

In the thin Wasmi adapter, resolve the manager from `Caller<GlobalStore>` -> `GlobalStore.context` ->
`ExecutableContext.graphics_manager`. An absent manager returns a controlled
error. Never call `graphics::ffi_manager()` from the new bindings or cleanup.
The context also supplies task/time dependencies needed for yielding and sleep.

Maintain an object registry per VM/store. Guests receive opaque numeric handles,
not native LVGL addresses, and cannot use another VM's registry. Only handles
issued by that registry may resolve to native pointers. Null handles are not an
implicit grant of access to the global screen. Widget-specific operations check
the object's LVGL class before use.

All LVGL operations and teardown use the context manager's lock. Invalidation
must handle parent deletion and external host deletion, not just guest deletion.
Use LVGL deletion notifications or an equivalent lifetime-safe mechanism;
address-validity checks alone do not protect against pointer reuse. Do not reuse
dead numeric handles during a VM execution. Cleanup deletes owned roots before
releasing callback data and retained buffers. Do not use `OwnedWindow`'s current
global-manager-based destructor for VM-owned cleanup.

## Binding-generator cleanup

Improve the source generator rather than editing its output. Separate these
concerns with small, named types/functions:

1. Discover/filter the LVGL signatures and append Xila helper signatures.
2. Order calls and assign ABI IDs once, shared by host and guest emitters.
3. Describe each call's public guest arguments, host-only context arguments,
   result encoding, and pointer ownership policy.
4. Emit runtime-specific host dispatch and guest wrappers from that description.

Generate VM-independent graphics dispatch separately from import registration;
the core generator model must not embed WAMR/Wasmi environment types.

Builder chooses the smallest shared build-only location that fits the existing
crate structure. Avoid depending on WAMR in Wasmi's generator/runtime, and avoid
making build-only `syn`/`quote`/LVGL parsing dependencies runtime dependencies.
Retain the legacy generator's outputs/ABI where possible; a broad rewrite of all
guest wrappers or WAMR pointer translation is not part of this milestone.

Replace unexplained argument-count literals and implicit pointer conversions
with named limits and explicit policies. Prefer a short supported-call table
and dedicated adapters for retained maps/events over generating unsafe generic
pointer casts. Unsupported signatures should produce understandable diagnostics
or explicit unsupported dispatch, rather than accidental compiler errors or
unchecked casts. Reuse common filtering/naming/ordering rules instead of copying
them into a separate Wasmi implementation.

Regression tests must prove that the cleanup preserves call names/IDs and guest
import signatures for the calculator-required calls, and that unsupported,
variadic, callback, and retained-pointer signatures cannot silently enter the
generic safe emitter. Any intentional ABI change must be called out rather than
hidden as a readability refactor.

## Guest-memory adaptation

Use the existing borrowed `GuestMemory` and fixed-width little-endian codecs.
Do not revive typed guest-pointer casts or native-layout struct access. Validate
argument counts, handles, and all output extents before observable side effects.
Honor guest-word width for pointer-sized results; event codes are `u32` and event
target outputs are guest words. Existing helper signatures with `u16` outputs
retain that encoding when implemented.

Borrowed strings are bounded, checked for a terminator, and usable only for
calls where LVGL copies them synchronously. Window icons must also be valid UTF-8.
No pointers into guest linear memory survive a host callback or memory growth.

`buttonmatrix_set_map` needs a dedicated adapter: decode its guest pointer table
by value, validate the complete map, and copy strings and the native pointer
table into VM-owned storage retained for that widget's lifetime. Preserve the
row separators. Accept the calculator's existing null table sentinel and adapt
it to LVGL's empty-string terminator; never hand an unterminated native map to
LVGL. Replacing a map releases the old storage only after LVGL has switched to
the new storage. Failed validation leaves the old map untouched.

Builder's initial limits: 256 issued object handles per execution, 64 map
entries, 1024 bytes including NUL per string, and 8192 total retained map-string
bytes per widget. Keep limits explicit and tested; exceedance returns an error,
not an unbounded scan. The map adapter necessarily allocates bounded retained
storage. Other translation access stays allocation-free where possible.

Event delivery translates a live event target only to an already issued handle;
unknown/deleted targets become null, not new capabilities. An empty queue writes
the existing no-event code and null target. Validate both output ranges before
popping an event, then write them sequentially without creating aliased typed
references. Stale events must never cause a dereference of deleted objects.

## Calculator runtime needs

The calculator currently calls `std::thread::sleep` between event checks.
`wasm_2` does not register `poll_oneoff` yet, and its out-of-fuel resume loop does
not yield. Include the minimum clock-poll/sleep support needed by the compiled
calculator, using explicit WASI subscription/event layouts and context managers.
Unsupported poll kinds return a controlled WASI error, not fabricated readiness.
Sleep and fuel exhaustion must hand execution back to the host scheduler so
graphics/event tasks run; do not block the executor with a synchronous sleep
inside a Wasmi callback. VM suspension state belongs to the store, not a global.

Builder chooses the precise Wasmi resumable-host-call mechanism after checking
the installed Wasmi API. Verify the actual guest imports before finalizing that
adapter. This is not a general socket/file polling implementation.

The interactive calculator test currently waits forever, and the guest does not
handle a close request. Provide an explicit test exit path so the integration
test can finish and cleanup can be asserted. Prefer a minimal close-event adapter
in the calculator; avoid unrelated evaluator, parser, or installation changes.

## Validation

- Test the core graphics dispatcher and marshalling directly with explicit
  memory/state/capability inputs, without building a Wasmi store. Keep separate
  adapter tests for registration, context extraction, and real guest calls.
- Tests for invalid IDs/counts, missing graphics capability, forged/stale/wrong
  widget handles, parent deletion, and VM isolation.
- Guest-memory tests for unaligned outputs, missing/unterminated strings,
  pointer-table overflow, map limits/sentinels, replacement lifetime, memory
  growth, event output overlap, and failed validation without side effects.
- Preserve existing 100% line/function translation coverage for both widths.
  Apply the same gate to new separable memory-marshalling helpers; LVGL native
  integration checks are additional, not part of a claim of full ABI coverage.
- A Wasmi integration guest using the calculator's actual graphics ABI, with
  deterministic checks for creation, label update, button event delivery, and
  teardown where the available display backend permits.
- Update the calculator's ignored interactive host test to use Wasmi; document
  its command and exit procedure. Manually verify a simple expression such as
  `1 + 2 = 3`, if a graphical session is available; otherwise report that limit.
- Focused crate/feature checks, formatting, and existing runtime/translation
  tests. Check both guest widths for portable marshalling; the actual calculator
  is a wasm32 guest. Do not claim embedded hardware validation from host checks.

## Non-goals

Full LVGL API parity; callbacks into guest code; styles/unions or arbitrary
native struct pointers; chart/image/font pointer APIs; general WASI polling;
calculator installation; replacing the old guest API; removing WAMR; changes to
unrelated native applications; pushing or rewriting branch history.
