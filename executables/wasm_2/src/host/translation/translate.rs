pub trait FromGuest<I>: Sized {
    fn from_guest(&self, memory: &mut [u8]) -> Option<I>;
}

pub trait IntoGuest<O>: Sized {
    fn into_guest(self, memory: &mut [u8]) -> Option<O>;
}
