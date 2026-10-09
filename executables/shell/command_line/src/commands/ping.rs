use getargs_derive::GetArgs;
use xila::{
    file_system::Path,
    internationalization::translate,
    network::{self, DnsQueryKind, Duration, IcmpEndpoint},
};

use crate::Error;

use super::{CommandContext, UserCommand};

pub struct PingCommand;

impl UserCommand for PingCommand {
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
        execute_ping(context, options).await
    }
}

const ICMP_IDENTIFIER: u16 = 0x22b;

#[derive(GetArgs)]
struct PingArguments<'a> {
    target: &'a str,
    #[arg(short = 'c', long = "count", default = 4)]
    count: u16,
    #[arg(short = 't', long = "timeout", default = 5)]
    timeout: u64,
    #[arg(short = 's', long = "size", default = 56)]
    size: usize,
}

async fn write_ping_line<C: CommandContext>(
    context: &mut C,
    target: &str,
    resolved_target: &xila::network::IpAddress,
) -> crate::Result<()> {
    context.write_out_fmt(format_args!(
        "{}\n",
        format_args!(
            translate!("PING {} ({}): {} data bytes"),
            target, resolved_target, 56
        )
    ))
}

async fn ping_loop<C: CommandContext>(
    context: &mut C,
    network_manager: &network::Manager,
    resolved_target: &xila::network::IpAddress,
    count: u16,
    timeout_seconds: u64,
    payload_size: usize,
) -> crate::Result<()> {
    let socket = network_manager
        .new_icmp_socket(256, 256, 1, 1, None)
        .await
        .map_err(Error::FailedToCreateSocket)?;

    socket
        .bind(IcmpEndpoint::Identifier(ICMP_IDENTIFIER))
        .await
        .map_err(Error::FailedToCreateSocket)?;

    for i in 0..count {
        match socket
            .ping(
                resolved_target,
                i,
                ICMP_IDENTIFIER,
                Duration::from_seconds(timeout_seconds),
                payload_size,
            )
            .await
        {
            Ok(duration) => {
                context.write_out_fmt(format_args!(
                    "{}\n",
                    format_args!(
                        translate!("{} bytes from {}: icmp_seq={} time={:.2} ms"),
                        payload_size,
                        resolved_target,
                        i,
                        duration.as_milliseconds()
                    )
                ))?;
            }
            Err(network::Error::TimedOut) => {
                context.write_out_fmt(format_args!(
                    "{}\n",
                    format_args!(translate!("Request timeout for icmp_seq {}"), i)
                ))?;
            }
            Err(e) => {
                context.write_out_fmt(format_args!(
                    "{}\n",
                    format_args!(translate!("Error: {}"), e)
                ))?;
            }
        }
    }

    Ok(())
}

async fn execute_ping<'a, I, C>(
    context: &mut C,
    options: &mut getargs::Options<&'a str, I>,
) -> crate::Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let PingArguments {
        target,
        count,
        timeout: timeout_seconds,
        size: payload_size,
    } = PingArguments::parse(options)?;

    let manager = context
        .executable_context()
        .network_manager
        .as_ref()
        .ok_or(crate::Error::InvalidOption)?;

    let resolved_target = manager
        .resolve(target, DnsQueryKind::A | DnsQueryKind::Aaaa, true, None)
        .await
        .ok()
        .and_then(|ips| ips.first().cloned());

    let resolved_target = match resolved_target {
        Some(ip) => ip,
        None => {
            context.write_out_fmt(format_args!(
                "{}\n",
                format_args!(translate!("Cannot resolve {}: Unknown host"), target)
            ))?;
            return Ok(());
        }
    };

    write_ping_line(context, target, &resolved_target).await?;
    ping_loop(
        context,
        manager,
        &resolved_target,
        count,
        timeout_seconds,
        payload_size,
    )
    .await
}
