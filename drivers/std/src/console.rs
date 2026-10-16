use crate::io::map_error;
use file_system::{
    ControlCommand, ControlCommandIdentifier, DirectBaseOperations, DirectCharacterDevice,
    MountOperations, Size, character_device::IS_A_TERMINAL,
};
use shared::AnyByLayout;
use std::io::{Read, Write, stderr, stdin, stdout};

pub struct StandardInDevice;

impl DirectBaseOperations for StandardInDevice {
    fn read(&self, buffer: &mut [u8], _: Size) -> file_system::Result<usize> {
        stdin().read(buffer).map_err(map_error)
    }

    fn write(&self, _: &[u8], _: Size) -> file_system::Result<usize> {
        Err(file_system::Error::UnsupportedOperation)
    }

    fn flush(&self) -> file_system::Result<()> {
        Ok(())
    }

    fn control(
        &self,
        command: ControlCommandIdentifier,
        input: &AnyByLayout,
        output: &mut AnyByLayout,
    ) -> file_system::Result<()> {
        control(command, input, output)
    }
}

impl MountOperations for StandardInDevice {}

impl DirectCharacterDevice for StandardInDevice {}

pub struct StandardOutDevice;

impl DirectBaseOperations for StandardOutDevice {
    fn read(&self, _: &mut [u8], _: Size) -> file_system::Result<usize> {
        Err(file_system::Error::UnsupportedOperation)
    }

    fn write(&self, buffer: &[u8], _: Size) -> file_system::Result<usize> {
        Ok(stdout().write(buffer).map_err(map_error)? as _)
    }

    fn flush(&self) -> file_system::Result<()> {
        stdout().flush().map_err(map_error)
    }

    fn control(
        &self,
        command: ControlCommandIdentifier,
        input: &AnyByLayout,
        output: &mut AnyByLayout,
    ) -> file_system::Result<()> {
        control(command, input, output)
    }
}

impl MountOperations for StandardOutDevice {}

impl DirectCharacterDevice for StandardOutDevice {}

pub struct StandardErrorDevice;

impl DirectBaseOperations for StandardErrorDevice {
    fn read(&self, _: &mut [u8], _: Size) -> file_system::Result<usize> {
        Err(file_system::Error::UnsupportedOperation)
    }

    fn write(&self, buffer: &[u8], _: Size) -> file_system::Result<usize> {
        Ok(stderr().write(buffer).map_err(map_error)? as _)
    }

    fn flush(&self) -> file_system::Result<()> {
        stderr().flush().map_err(map_error)
    }

    fn control(
        &self,
        command: ControlCommandIdentifier,
        input: &AnyByLayout,
        output: &mut AnyByLayout,
    ) -> file_system::Result<()> {
        control(command, input, output)
    }
}

impl MountOperations for StandardErrorDevice {}

impl DirectCharacterDevice for StandardErrorDevice {}

fn control(
    command: ControlCommandIdentifier,
    _: &AnyByLayout,
    output: &mut AnyByLayout,
) -> file_system::Result<()> {
    match command {
        IS_A_TERMINAL::IDENTIFIER => {
            let output: &mut bool = IS_A_TERMINAL::cast_output(output)?;
            *output = true;

            Ok(())
        }
        _ => Err(file_system::Error::UnsupportedOperation),
    }
}
