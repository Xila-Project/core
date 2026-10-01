use super::{FromGuest, GuestPointer, GuestSlice, IntoGuest};

#[repr(align(8))]
struct AlignedMemory([u8; 16]);

#[test]
fn guest_pointer_translates_valid_null_and_rejects_invalid_offsets() {
    let mut backing = AlignedMemory([0u8; 16]);
    let memory = &mut backing.0;

    let valid = GuestPointer::<u32>::new(4)
        .from_guest(&mut memory)
        .expect("aligned in-bounds pointer");
    assert_eq!(valid as usize, memory.as_mut_ptr() as usize + 4);

    let null = GuestPointer::<u32>::new(0)
        .from_guest(&mut memory)
        .expect("null pointer offset is permitted");
    assert!(null.is_null());

    assert!(
        GuestPointer::<u32>::new(1)
            .from_guest(&mut memory)
            .is_none()
    );
    assert!(
        GuestPointer::<u32>::new(16)
            .from_guest(&mut memory)
            .is_none()
    );
}

#[test]
fn guest_slice_accepts_valid_empty_ranges_and_rejects_bad_ranges() {
    let mut backing = AlignedMemory([0u8; 16]);
    let memory = &mut backing.0;

    let values = GuestSlice::<u32>::new(4, 2)
        .from_guest(&mut memory)
        .expect("two aligned values fit");
    assert_eq!(
        values as *mut u32 as usize,
        memory.as_mut_ptr() as usize + 4
    );
    assert_eq!(unsafe { (&*values).len() }, 2);

    assert_eq!(
        unsafe {
            (&*GuestSlice::<u32>::new(0, 0)
                .from_guest(&mut memory)
                .unwrap())
                .len()
        },
        0
    );
    assert_eq!(
        unsafe {
            (&*GuestSlice::<u32>::new(16, 0)
                .from_guest(&mut memory)
                .unwrap())
                .len()
        },
        0
    );
    assert!(
        GuestSlice::<u32>::new(0, 1)
            .from_guest(&mut memory)
            .is_none()
    );
    assert!(
        GuestSlice::<u32>::new(1, 1)
            .from_guest(&mut memory)
            .is_none()
    );
    assert!(
        GuestSlice::<u32>::new(12, 2)
            .from_guest(&mut memory)
            .is_none()
    );
}

#[test]
fn guest_pointer_round_trips_host_pointer_to_guest_offset() {
    let mut backing = AlignedMemory([0u8; 16]);
    let memory = &mut backing.0;
    let host_pointer = unsafe { memory.as_mut_ptr().add(4) };

    let guest_pointer = host_pointer
        .into_guest(&mut memory)
        .expect("pointer into same guest memory");
    assert_eq!(guest_pointer.offset, 4);

    let translated = guest_pointer
        .from_guest(&mut memory)
        .expect("valid guest pointer");
    assert_eq!(translated as usize, host_pointer as usize);

    let outside = 1usize as *mut u32;
    assert!(outside.into_guest(&mut memory).is_none());
}
