use crate::Standard;
use alloc::{boxed::Box, string::String, vec::Vec};
use core::{num::NonZeroUsize, pin::Pin};
use file_system::{
    ControlCommand, ControlCommandIdentifier, DirectBaseOperations, DirectCharacterDevice,
    MountOperations, define_command,
};
use shared::AnyByLayout;
use synchronization::Arc;

define_command!(GET_MAIN_FUNCTION, Read, b'E', 1, (), MainFunction);

pub trait ExecutableTrait: 'static + Send + Sync {
    fn main(standard: Standard<'static>, arguments: Vec<String>) -> MainFuture;
}

pub struct ExecutableContext {
    pub task_manager: Arc<task::Manager>,
    pub users_manager: Arc<users::Manager>,
    pub virtual_file_system: Arc<virtual_file_system::VirtualFileSystem>,
    #[cfg(feature = "graphics")]
    pub graphics_manager: Option<Arc<graphics::Manager>>,
    pub network_manager: Option<Arc<network::Manager>>,
    pub time_manager: Arc<time::Manager<'static>>,
}

pub type MainFuture =
    Pin<Box<dyn Future<Output = core::result::Result<(), NonZeroUsize>> + 'static>>;

pub type MainFunction = Option<
    Box<dyn Fn(&'static ExecutableContext, Standard<'static>, Vec<String>) -> MainFuture + 'static>,
>;

pub struct ExecutableWrapper<T: ExecutableTrait>(T, &'static ExecutableContext);

impl<T: ExecutableTrait> ExecutableWrapper<T> {
    pub fn new(executable: T, context: &'static ExecutableContext) -> Self {
        Self(executable, context)
    }
}

impl<T: ExecutableTrait> MountOperations for ExecutableWrapper<T> {}

impl<T: ExecutableTrait> DirectBaseOperations for ExecutableWrapper<T> {
    fn read(
        &self,
        _buffer: &mut [u8],
        _absolute_position: file_system::Size,
    ) -> file_system::Result<usize> {
        Err(file_system::Error::UnsupportedOperation)
    }

    fn write(
        &self,
        _buffer: &[u8],
        _absolute_position: file_system::Size,
    ) -> file_system::Result<usize> {
        Err(file_system::Error::UnsupportedOperation)
    }

    fn control(
        &self,
        command: ControlCommandIdentifier,
        _: &AnyByLayout,
        output: &mut AnyByLayout,
    ) -> file_system::Result<()> {
        log::debug!("ExecutableWrapper control command: {:?}", command);

        match command {
            GET_MAIN_FUNCTION::IDENTIFIER => {
                let output = GET_MAIN_FUNCTION::cast_output(output)?;
                let executable_context = self.1;
                *output = Some(Box::new(move |context, standard, arguments| {
                    let mut standard = standard;
                    standard.context = context;
                    debug_assert!(core::ptr::eq(context, executable_context));
                    T::main(standard, arguments)
                }));
                Ok(())
            }
            _ => Err(file_system::Error::UnsupportedOperation),
        }
    }
}

impl<T: ExecutableTrait> DirectCharacterDevice for ExecutableWrapper<T> {}

#[macro_export]
macro_rules! mount_executables {

    ( $context:expr, $virtual_file_system:expr, $task:expr, &[ $( ($path:expr, $executable:expr) ),* $(,)? ] ) => {

    async || -> $crate::exported_virtual_file_system::Result<()>
    {
        use $crate::exported_file_system::{Permissions};
        use $crate::exported_virtual_file_system::ItemStatic;

        $(
            let __executable = $crate::ExecutableWrapper::new($executable, $context);

            let _ = $virtual_file_system.remove($task, $path).await;
            $virtual_file_system.mount_character_device($task, $path, __executable).await?;
            $virtual_file_system.set_permissions($task, $path, Permissions::EXECUTABLE).await?;
        )*

        Ok(())
    }()
};

}
