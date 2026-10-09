use alloc::vec::Vec;
use core::mem::size_of;

use super::{
    GuestMemory, GuestPointer, GuestSlice, GuestValue, GuestValues, WasiVector, WasmUsize,
    borrow_memory, get_memory, sealed::Sealed,
};
use crate::host::wasi::types::{Fdstat, Filestat, Prestat, PrestatDir};

#[test]
fn handles_and_views_have_no_owned_buffers() {
    let pointer = GuestPointer::<u32>::new(3);
    let slice = pointer.as_slice(2);
    assert_eq!(slice.offset(), 3);
    assert_eq!(slice.len(), 2);
    assert!(!slice.is_empty());
    assert!(GuestSlice::<u8>::new(128, 0).is_empty());
    assert_eq!(GuestSlice::<u8>::from((3, 2)), GuestSlice::new(3, 2));
    assert_eq!(slice.pointer_at(1).unwrap().offset, 7);
    assert!(slice.pointer_at(2).is_none());
    assert!(GuestSlice::<u32>::new(3, 0).pointer_at(0).is_none());
    assert!(
        GuestSlice::<u64>::new(WasmUsize::MAX - 7, 2)
            .pointer_at(1)
            .is_none()
    );
    assert!(
        GuestSlice::<u64>::new(1, WasmUsize::MAX)
            .pointer_at(WasmUsize::MAX - 1)
            .is_none()
    );
    assert_eq!(size_of::<GuestPointer<u32>>(), size_of::<WasmUsize>());
    assert_eq!(size_of::<GuestSlice<u32>>(), 2 * size_of::<WasmUsize>());
    assert_eq!(size_of::<GuestMemory<'_>>(), 2 * size_of::<usize>());
    assert_eq!(size_of::<GuestValues<'_, u32>>(), 2 * size_of::<usize>());
}

fn check_codec<T: GuestValue>(value: T, expected: &[u8]) {
    let mut backing = [0xAA; 160];
    assert_eq!(expected.len(), T::SIZE);
    let mut memory = GuestMemory::new(&mut backing);
    // Deliberately unaligned: only the guest encoding, not native T alignment, matters.
    memory.write(GuestPointer::new(3), value).unwrap();
    assert_eq!(
        memory
            .bytes(GuestSlice::new(3, T::SIZE as WasmUsize))
            .unwrap(),
        expected
    );
    let decoded = memory.read(GuestPointer::<T>::new(3)).unwrap();
    let mut encoded = [0; 64];
    decoded.encode(&mut encoded[..T::SIZE]);
    assert_eq!(&encoded[..T::SIZE], expected);

    let table = GuestSlice::<T>::new(1, 2);
    memory.write(table.pointer_at(0).unwrap(), value).unwrap();
    memory.write(table.pointer_at(1).unwrap(), value).unwrap();
    let mut values = memory.values(table).unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values.size_hint(), (2, Some(2)));
    for _ in 0..2 {
        values.next().unwrap().encode(&mut encoded[..T::SIZE]);
        assert_eq!(&encoded[..T::SIZE], expected);
    }
    assert_eq!(values.len(), 0);
    assert!(values.next().is_none());
    assert!(values.next().is_none());
    assert_eq!(backing[0], 0xAA);
    assert!(
        backing[(3 + T::SIZE).max(1 + 2 * T::SIZE)..]
            .iter()
            .all(|&byte| byte == 0xAA)
    );
}

#[test]
fn fixed_width_scalars_have_exact_little_endian_encodings() {
    check_codec(0x12_u8, &[0x12]);
    check_codec(0x1234_u16, &[0x34, 0x12]);
    check_codec(0x1234_5678_u32, &[0x78, 0x56, 0x34, 0x12]);
    check_codec(0x0102_0304_0506_0708_u64, &[8, 7, 6, 5, 4, 3, 2, 1]);
    check_codec(-2_i8, &[0xFE]);
    check_codec(-2_i16, &[0xFE, 0xFF]);
    check_codec(-2_i32, &[0xFE, 0xFF, 0xFF, 0xFF]);
    check_codec(-2_i64, &[0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    check_codec(1.0_f32, &[0, 0, 0x80, 0x3F]);
    check_codec(1.0_f64, &[0, 0, 0, 0, 0, 0, 0xF0, 0x3F]);
    check_codec(f32::from_bits(0x7FC0_0001), &[1, 0, 0xC0, 0x7F]);
    check_codec(
        f64::from_bits(0x8000_0000_0000_0000),
        &[0, 0, 0, 0, 0, 0, 0, 0x80],
    );
}

#[test]
fn handles_and_iovecs_use_the_selected_guest_width() {
    let mut pointer = [0; 8];
    pointer[0] = 3;
    let mut pair = [0; 16];
    pair[0] = 3;
    pair[WasmUsize::SIZE] = 2;
    check_codec(GuestPointer::<u32>::new(3), &pointer[..WasmUsize::SIZE]);
    check_codec(GuestSlice::<u32>::new(3, 2), &pair[..2 * WasmUsize::SIZE]);
    check_codec(WasiVector::new(3, 2), &pair[..2 * WasmUsize::SIZE]);
}

#[test]
fn wasi_records_use_fixed_layouts_and_initialize_all_padding() {
    // These fixtures are written independently of the codecs and native host layouts.
    let fdstat = [
        4, 0, 0x34, 0x12, 0, 0, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13,
        0x12, 0x11,
    ];
    check_codec(
        Fdstat {
            filetype: 4,
            flags: 0x1234,
            rights_base: 0x0102_0304_0506_0708,
            rights_inheriting: 0x1112_1314_1516_1718,
        },
        &fdstat,
    );
    check_codec(
        Prestat {
            tag: 0,
            dir: PrestatDir {
                pr_name_len: 0x1234_5678,
            },
        },
        &[0, 0, 0, 0, 0x78, 0x56, 0x34, 0x12],
    );
    let filestat = [
        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0,
        0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0,
        0, 0, 0, 0,
    ];
    check_codec(
        Filestat {
            dev: 1,
            ino: 2,
            filetype: 4,
            nlink: 3,
            size: 4,
            atime: 5,
            mtime: 6,
            ctime: 7,
        },
        &filestat,
    );
}

fn check_ranges<T: GuestValue>() {
    let mut backing = [0xAA; 128];
    let mut memory = GuestMemory::new(&mut backing);
    for offset in (0..=132).chain([WasmUsize::MAX]) {
        let pointer = GuestPointer::<T>::new(offset);
        let expected = offset != 0 && offset as u128 + T::SIZE as u128 <= 128;
        assert_eq!(memory.validate_pointer(pointer).is_some(), expected);
        assert_eq!(memory.read(pointer).is_some(), expected);
        for count in (0..=18).chain([WasmUsize::MAX]) {
            // Wide arithmetic is independent of the implementation's usize overflow checks.
            let expected = !(offset == 0 && count != 0)
                && offset as u128 + count as u128 * T::SIZE as u128 <= 128;
            let slice = GuestSlice::<T>::new(offset, count);
            assert_eq!(
                memory.validate_slice(slice).is_some(),
                expected,
                "offset {offset}, count {count}"
            );
            let values = memory.values(slice);
            assert_eq!(values.is_some(), expected);
            if let Some(values) = values {
                assert_eq!(values.len(), count as usize);
            }
        }
    }
    // Failed writes must not modify any byte.
    let value = memory.read(GuestPointer::<T>::new(1)).unwrap();
    for offset in [0, 128, 129, WasmUsize::MAX] {
        assert!(memory.write(GuestPointer::new(offset), value).is_none());
    }
    assert_eq!(backing, [0xAA; 128]);
}

#[test]
fn range_model_covers_encoded_sizes_nulls_boundaries_overflow_and_unaligned_offsets() {
    check_ranges::<u8>();
    check_ranges::<u16>();
    check_ranges::<u32>();
    check_ranges::<u64>();
    check_ranges::<i8>();
    check_ranges::<i16>();
    check_ranges::<i32>();
    check_ranges::<i64>();
    check_ranges::<f32>();
    check_ranges::<f64>();
    check_ranges::<WasiVector>();
    check_ranges::<GuestPointer<u32>>();
    check_ranges::<GuestSlice<u32>>();
    check_ranges::<Fdstat>();
    check_ranges::<Prestat>();
    check_ranges::<Filestat>();
}

#[test]
fn borrowed_bytes_are_zero_copy_and_mutations_are_confined_to_the_range() {
    let mut backing = [0xAA; 128];
    let expected = backing.as_ptr().wrapping_add(3);
    let mut memory = GuestMemory::new(&mut backing);
    let range = GuestSlice::new(3, 4);
    let bytes = memory.bytes(range).unwrap();
    assert_eq!(bytes.as_ptr(), expected);
    assert_eq!(bytes, &[0xAA; 4]);
    memory.bytes_mut(range).unwrap().fill(7);
    assert_eq!(memory.bytes(range).unwrap(), &[7; 4]);
    for invalid in [
        GuestSlice::new(0, 1),
        GuestSlice::new(128, 1),
        GuestSlice::new(WasmUsize::MAX, 1),
    ] {
        assert!(memory.bytes(invalid).is_none());
        assert!(memory.bytes_mut(invalid).is_none());
    }
    for empty in [
        GuestSlice::new(0, 0),
        GuestSlice::new(1, 0),
        GuestSlice::new(128, 0),
    ] {
        assert!(memory.bytes(empty).unwrap().is_empty());
        assert!(memory.bytes_mut(empty).unwrap().is_empty());
    }
    assert!(memory.bytes(GuestSlice::new(129, 0)).is_none());
    assert_eq!(backing[..3], [0xAA; 3]);
    assert_eq!(backing[7..], [0xAA; 121]);
}

#[test]
fn consuming_a_view_preserves_the_original_buffer_borrow() {
    let mut backing = [0; 8];
    GuestMemory::new(&mut backing)
        .into_bytes_mut(GuestSlice::new(3, 2))
        .unwrap()
        .fill(9);
    assert_eq!(backing, [0, 0, 0, 9, 9, 0, 0, 0]);
    assert!(
        GuestMemory::new(&mut backing)
            .into_bytes_mut(GuestSlice::new(7, 2))
            .is_none()
    );
}

#[test]
fn reverse_mapping_is_read_only_and_preserves_the_whole_byte_range() {
    let mut backing = [0; 128];
    let (before, remainder) = backing.split_at_mut(16);
    let (middle, after) = remainder.split_at_mut(64);
    let memory = GuestMemory::new(middle);
    let range = GuestSlice::new(3, 17);
    assert_eq!(memory.offset_of(memory.bytes(range).unwrap()), Some(range));
    for offset in [0, 1, 64] {
        let range = GuestSlice::new(offset, 0);
        assert_eq!(memory.offset_of(memory.bytes(range).unwrap()), Some(range));
    }
    assert!(memory.offset_of(before).is_none());
    assert!(memory.offset_of(after).is_none());
}

#[derive(Clone, Copy)]
struct ZeroSize;
impl Sealed for ZeroSize {}
impl GuestValue for ZeroSize {
    const SIZE: usize = 0;
    fn decode(_: &[u8]) -> Self {
        Self
    }
    fn encode(self, _: &mut [u8]) {}
}

#[derive(Clone, Copy)]
struct HugeSize;
impl Sealed for HugeSize {}
impl GuestValue for HugeSize {
    const SIZE: usize = usize::MAX;
    fn decode(_: &[u8]) -> Self {
        Self
    }
    fn encode(self, _: &mut [u8]) {}
}

#[test]
fn invalid_codec_sizes_and_extent_overflow_are_rejected_without_allocating() {
    let mut backing = [0; 8];
    let mut memory = GuestMemory::new(&mut backing);
    assert!(
        memory
            .validate_pointer(GuestPointer::<ZeroSize>::new(1))
            .is_none()
    );
    assert!(memory.values(GuestSlice::<ZeroSize>::new(1, 0)).is_none());
    assert!(memory.read(GuestPointer::<ZeroSize>::new(1)).is_none());
    assert!(memory.write(GuestPointer::new(1), ZeroSize).is_none());
    assert!(
        memory
            .validate_pointer(GuestPointer::<HugeSize>::new(1))
            .is_none()
    );
    assert!(
        memory
            .validate_slice(GuestSlice::<HugeSize>::new(1, 2))
            .is_none()
    );
    let pointer = GuestSlice::<HugeSize>::new(1, 3).pointer_at(2);
    let offset = 1 + 2 * HugeSize::SIZE as u128;
    if offset > WasmUsize::MAX as u128 {
        assert!(pointer.is_none());
    } else {
        assert_eq!(pointer.unwrap().offset, offset as WasmUsize);
    }
}

#[test]
fn overlapping_guest_outputs_are_written_sequentially_without_aliasing() {
    let mut backing = [0; 16];
    let mut memory = GuestMemory::new(&mut backing);
    memory.write(GuestPointer::new(1), 0x1122_3344_u32).unwrap();
    memory.write(GuestPointer::new(2), 0x5566_7788_u32).unwrap();
    assert_eq!(
        memory.bytes(GuestSlice::new(1, 5)).unwrap(),
        &[0x44, 0x88, 0x77, 0x66, 0x55]
    );
}

#[derive(Clone, Copy)]
enum Export {
    Memory,
    Missing,
    Function,
}

fn name(text: &str, output: &mut Vec<u8>) {
    output.push(text.len() as u8);
    output.extend_from_slice(text.as_bytes());
}

fn section(id: u8, contents: &[u8], output: &mut Vec<u8>) {
    output.extend_from_slice(&[id, contents.len() as u8]);
    output.extend_from_slice(contents);
}

fn probe_module(export: Export) -> Vec<u8> {
    let mut wasm = b"\0asm\x01\0\0\0".to_vec();
    section(1, &[1, 0x60, 0, 0], &mut wasm);
    let mut imports = alloc::vec![1];
    name("test", &mut imports);
    name("probe", &mut imports);
    imports.extend_from_slice(&[0, 0]);
    section(2, &imports, &mut wasm);
    section(3, &[1, 0], &mut wasm);
    section(5, &[1, 0, 1], &mut wasm);
    let mut exports = alloc::vec![if matches!(export, Export::Missing) {
        1
    } else {
        2
    }];
    name("run", &mut exports);
    exports.extend_from_slice(&[0, 1]);
    match export {
        Export::Memory => {
            name("memory", &mut exports);
            exports.extend_from_slice(&[2, 0]);
        }
        Export::Function => {
            name("memory", &mut exports);
            exports.extend_from_slice(&[0, 1]);
        }
        Export::Missing => {}
    }
    section(7, &exports, &mut wasm);
    section(10, &[1, 4, 0, 0x10, 0, 0x0B], &mut wasm);
    wasm
}

#[test]
fn wasmi_helpers_find_only_memory_exports_and_borrow_live_bytes() {
    use wasmi::{Caller, Engine, Linker, Module, Store};
    for export in [Export::Memory, Export::Missing, Export::Function] {
        let engine = Engine::default();
        let module = Module::new(&engine, probe_module(export)).unwrap();
        let mut store = Store::new(&engine, false);
        let mut linker = Linker::new(&engine);
        linker
            .func_wrap("test", "probe", |mut caller: Caller<bool>| {
                if let Some(memory) = get_memory(&caller) {
                    let mut guest = borrow_memory(&memory, &mut caller);
                    assert_eq!(guest.read(GuestPointer::<u8>::new(1)), Some(0));
                    guest.write(GuestPointer::new(1), 42_u8).unwrap();
                    guest.write(GuestPointer::new(65_535), 99_u8).unwrap();
                    *caller.data_mut() = true;
                }
            })
            .unwrap();
        let instance = linker.instantiate_and_start(&mut store, &module).unwrap();
        instance
            .get_typed_func::<(), ()>(&store, "run")
            .unwrap()
            .call(&mut store, ())
            .unwrap();
        assert_eq!(*store.data(), matches!(export, Export::Memory));
        if let Some(memory) = instance.get_memory(&store, "memory") {
            assert_eq!(memory.data(&store)[1], 42);
            assert_eq!(memory.data(&store)[65_535], 99);
        }
    }
}
