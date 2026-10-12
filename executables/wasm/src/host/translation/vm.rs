use super::GuestMemory;
use wasmi::Caller;

pub fn get_memory<T>(caller: &Caller<'_, T>) -> Option<wasmi::Memory> {
    caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
}

pub fn borrow_memory<'a, T>(
    memory: &wasmi::Memory,
    caller: &'a mut Caller<'_, T>,
) -> GuestMemory<'a> {
    GuestMemory::new(memory.data_mut(caller))
}
