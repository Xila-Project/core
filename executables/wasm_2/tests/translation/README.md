# Translation-layer validation

This standalone test crate compiles the actual production translation sources
and WASI record codecs using `#[path]`. It does not copy the implementation or
require the OS integration. Its Wasmi dependency uses the same version requirement
and features as `wasm_2`.

## Borrowed-memory API

```rust
use wasm_translation_tests::translation::{GuestMemory, GuestPointer, GuestSlice};

let mut backing = [0; 32];
let mut memory = GuestMemory::new(&mut backing);

// Unaligned addresses are safe: values are decoded/encoded, never cast to host references.
memory.write(GuestPointer::new(3), 0x1122_3344_u32).unwrap();
assert_eq!(memory.read(GuestPointer::<u32>::new(3)), Some(0x1122_3344));

// Bulk byte access is zero-copy and tied to the view's borrow.
let range = GuestSlice::<u8>::new(8, 4);
memory.bytes_mut(range).unwrap().copy_from_slice(b"test");
assert_eq!(memory.offset_of(memory.bytes(range).unwrap()), Some(range));
```

The compact handles retain guest offsets and element counts. Resolve them with:

- `read` / `write`: logical values using their explicit `GuestValue` codec.
- `values`: a lazy, exact-size iterator over encoded table elements.
- `bytes` / `bytes_mut`: shared/exclusive byte slices borrowing the view.
- `into_bytes_mut`: consume a view and borrow its original backing buffer.
- `validate_pointer` / `validate_slice`: validate outputs before side effects.
- `GuestSlice::pointer_at`: retain table bounds when calculating an element offset.

Nonempty ranges at guest offset zero remain invalid, matching the existing null
pointer convention. Empty ranges at zero and at the end of memory are valid.
No operation relies on native alignment, native structure padding, or native byte
order. There are no raw-pointer outputs, `FromGuest`/`IntoGuest` overloads, or
reference-to-pointer reverse conversions. `offset_of` borrows memory immutably,
preserves the entire byte-range length, and checks numeric bounds; it does not
establish pointer provenance. Usually retain the original handle instead.

`GuestPointer<T>` stores one guest word; `GuestSlice<T>` stores two. The memory
view and table iterator each store two host words. None allocates or stages a
guest buffer. ABI scalars are fixed-width, so `usize`/`isize` are not codecs.
WASI `Fdstat`, `Prestat`, and `Filestat` have explicit 24-, 8-, and 64-byte
encodings with zeroed padding, independently of their native Rust layouts.

## Tests

From the repository root:

```sh
cargo +stable test --manifest-path executables/wasm_2/tests/translation/Cargo.toml
cargo +stable test --manifest-path executables/wasm_2/tests/translation/Cargo.toml --no-default-features --features memory_64
```

Tests cover independent little-endian scalar and WASI-record fixtures, padding,
unaligned accesses, all codec families, a wide-integer range model, nulls, empty
ranges, overflow, failed writes without side effects, overlapping sequential
outputs, full-range reverse mapping, zero-copy byte access, and real Wasmi
callbacks with present/missing/wrong-kind memory exports.

Compile-fail doctests verify that mutable byte ranges and table iterators prevent
conflicting writes, and that borrowed bytes cannot outlive their backing buffer.

## Coverage gate

Requires `cargo-llvm-cov` and the stable `llvm-tools-preview` component. Run for
both `memory_32` and `memory_64`:

```sh
cargo +stable llvm-cov \
  --manifest-path executables/wasm_2/tests/translation/Cargo.toml \
  --no-default-features --features memory_32 \
  --ignore-filename-regex '(tests\.rs|tests/translation/src/lib\.rs)$' \
  --fail-under-lines 100 --fail-under-functions 100
```

Only tests and the harness module are excluded. The production translation source
and WASI record codecs are measured. CI enforces 100% line and function coverage
for each guest width independently. LLVM regions and branch coverage are separate
metrics: some conversion/error paths are unreachable on a given host width or
generic instantiation. The gate is not a proof of correctness for arbitrary inputs.

## Miri and portability

```sh
rustup component add --toolchain nightly miri rust-src
cargo +nightly miri test --manifest-path executables/wasm_2/tests/translation/Cargo.toml
cargo +nightly miri test --manifest-path executables/wasm_2/tests/translation/Cargo.toml --no-default-features --features memory_64
cargo +nightly miri test --manifest-path executables/wasm_2/tests/translation/Cargo.toml --no-default-features --features memory_64 --target i686-unknown-linux-gnu
cargo +nightly miri test --manifest-path executables/wasm_2/tests/translation/Cargo.toml --target powerpc-unknown-linux-gnu
```

The 32-bit simulated host exercises guest-to-host narrowing conversions; the
big-endian simulated host checks the independent little-endian ABI fixtures.
These checks do not establish hardware-target correctness or measure firmware RAM/flash.
