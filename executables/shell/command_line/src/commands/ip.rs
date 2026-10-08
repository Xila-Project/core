use crate::error::{Error, Result};
use getargs_derive::GetArgs;
use xila::{
    file_system::{AccessFlags, Path},
    log,
    network::{GET_IP_ADDRESS, GET_IP_ADDRESS_COUNT, GET_ROUTE, GET_ROUTE_COUNT, GET_STATE},
    virtual_file_system::{Directory, File, VirtualFileSystem},
};

use super::{CommandContext, UserCommand};

pub struct IpCommand;

impl UserCommand for IpCommand {
    async fn execute<'a, I, C>(
        &self,
        context: &mut C,
        options: &mut getargs::Options<&'a str, I>,
        _paths: &[&Path],
    ) -> crate::Result<()>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext,
    {
        execute_ip(context, options).await
    }
}

#[derive(GetArgs)]
struct IpArguments<'a> {
    command: &'a str,
}

async fn open_interface<C: CommandContext>(context: &C, interface: &str) -> Result<File<'static>> {
    let executable_context = context.executable_context();
    let virtual_file_system = &executable_context.virtual_file_system;
    let task = executable_context
        .task_manager
        .get_current_task_identifier()
        .await;
    let path = Path::NETWORK_DEVICES
        .join(interface)
        .ok_or(Error::FailedToJoinPath)?;

    File::open(virtual_file_system, task, &path, AccessFlags::Read.into())
        .await
        .map_err(Error::FailedToOpenFile)
}

async fn show_routes_interface<C: CommandContext>(
    context: &mut C,
    interface: &str,
    file: &mut File<'_>,
) -> crate::Result<()> {
    let count = file
        .as_synchronous_file_mut()
        .control(GET_ROUTE_COUNT, &())
        .map_err(Error::FailedToOpenFile)?;

    for index in 0..count {
        let route = file
            .as_synchronous_file_mut()
            .control(GET_ROUTE, &index)
            .map_err(Error::FailedToOpenFile)?;
        context.write_out_fmt(format_args!(
            "{} via {} device {}\n",
            route.cidr, route.via_router, interface
        ))?;
    }

    Ok(())
}

async fn show_routes<C: CommandContext>(
    context: &mut C,
    virtual_file_system: &VirtualFileSystem,
    task: xila::task::TaskIdentifier,
) -> crate::Result<()> {
    let mut directory = Directory::open(virtual_file_system, task, Path::NETWORK_DEVICES)
        .await
        .map_err(Error::FailedToOpenDirectory)?;

    while let Some(entry) = directory
        .read()
        .await
        .map_err(Error::FailedToReadDirectoryEntry)?
    {
        if entry.name == "." || entry.name == ".." {
            continue;
        }

        let mut file = open_interface(context, &entry.name).await?;
        show_routes_interface(context, &entry.name, &mut file).await?;
    }

    Ok(())
}

async fn show_address_interface<C: CommandContext>(
    context: &mut C,
    file: &mut File<'_>,
) -> crate::Result<()> {
    let count = file
        .as_synchronous_file_mut()
        .control(GET_IP_ADDRESS_COUNT, &())
        .map_err(Error::FailedToOpenFile)?;

    for index in 0..count {
        let address = file
            .as_synchronous_file_mut()
            .control(GET_IP_ADDRESS, &index)
            .map_err(Error::FailedToOpenFile)?;
        context.write_out_fmt(format_args!("   {} \n", address))?;
    }

    Ok(())
}

async fn show_address<C: CommandContext>(
    context: &mut C,
    virtual_file_system: &VirtualFileSystem,
    task: xila::task::TaskIdentifier,
) -> crate::Result<()> {
    let mut directory = Directory::open(virtual_file_system, task, Path::NETWORK_DEVICES)
        .await
        .map_err(Error::FailedToOpenDirectory)?;

    let mut index = 1;

    while let Some(entry) = directory
        .read()
        .await
        .map_err(Error::FailedToReadDirectoryEntry)?
    {
        if entry.name == "." || entry.name == ".." {
            continue;
        }

        log::information!("Showing address for interface {}", entry.name);

        let mut file = open_interface(context, &entry.name).await?;

        let state = file
            .control(GET_STATE, &())
            .await
            .map_err(Error::FailedToOpenFile)?;
        let state = if state { "Enabled" } else { "Disabled" };

        let status = file
            .control(xila::network::IS_LINK_UP, &())
            .await
            .map_err(Error::FailedToOpenFile)?;
        let status: &str = if status { "Up" } else { "Down" };

        let hardware_address = file
            .control(xila::network::GET_HARDWARE_ADDRESS, &())
            .await
            .map_err(Error::FailedToOpenFile)?;

        let maximum_transmission_unit = file
            .control(xila::network::GET_MAXIMUM_TRANSMISSION_UNIT, &())
            .await
            .map_err(Error::FailedToOpenFile)?;

        context.write_out_fmt(format_args!(
            "{}. {} [{}, {}, MTU: {}]\n",
            index, entry.name, state, status, maximum_transmission_unit
        ))?;
        context.write_out_fmt(format_args!(
            "   Hardware Address: {:x}:{:x}:{:x}:{:x}:{:x}:{:x}\n",
            hardware_address[0],
            hardware_address[1],
            hardware_address[2],
            hardware_address[3],
            hardware_address[4],
            hardware_address[5]
        ))?;

        show_address_interface(context, &mut file).await?;

        index += 1;
    }

    Ok(())
}

async fn execute_ip<'a, I, C>(
    context: &mut C,
    options: &mut getargs::Options<&'a str, I>,
) -> crate::Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let IpArguments { command } = IpArguments::parse(options)?;

    let executable_context = context.executable_context();
    let virtual_file_system = &executable_context.virtual_file_system;
    let task = executable_context
        .task_manager
        .get_current_task_identifier()
        .await;

    match command {
        "address" | "a" => show_address(context, virtual_file_system, task).await?,
        "route" | "r" => show_routes(context, virtual_file_system, task).await?,
        _ => return Err(crate::Error::InvalidOption),
    }

    Ok(())
}
