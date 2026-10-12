use super::sealed::Sealed;

/// A value with an explicit, fixed-size, little-endian guest ABI encoding.
///
/// Implementations are sealed inside this crate. `SIZE` must be nonzero and independent
/// of native Rust layout. Both codec methods receive exactly `SIZE` initialized bytes;
/// encoding must initialize every byte, including ABI padding.
pub trait GuestValue: Sealed + Copy {
    const SIZE: usize;
    fn decode(bytes: &[u8]) -> Self;
    fn encode(self, bytes: &mut [u8]);
}

macro_rules! primitive_guest_values {
    ($($ty:ty => $size:expr),* $(,)?) => {
        $(
            impl Sealed for $ty {}
            impl GuestValue for $ty {
                const SIZE: usize = $size;

                fn decode(bytes: &[u8]) -> Self {
                    Self::from_le_bytes(bytes.try_into().expect("exact ABI scalar size"))
                }

                fn encode(self, bytes: &mut [u8]) {
                    bytes.copy_from_slice(&self.to_le_bytes());
                }
            }
        )*
    };
}

primitive_guest_values!(
    u8 => 1, u16 => 2, u32 => 4, u64 => 8,
    i8 => 1, i16 => 2, i32 => 4, i64 => 8,
    f32 => 4, f64 => 8,
);
