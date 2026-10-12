use core::fmt::Display;

use crate::host::wasi::WasiContext;

pub struct GlobalStore {
    pub context: &'static xila::executable::ExecutableContext,
    pub wasi: WasiContext,
    pub pending: Option<super::execution::PendingCall>,
    #[cfg(feature = "graphics")]
    pub graphics: super::graphics::GraphicsState,
}

impl Display for GlobalStore {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "GlobalStore")
    }
}
