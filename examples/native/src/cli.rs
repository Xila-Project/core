use getargs_derive::GetArgs;
use std::string::{String, ToString};

pub const MINIMUM_DISK_SIZE: u64 = (xila::file_system::mbr::Mbr::MINIMUM_START_BLOCK + 2)
    * xila::file_system::mbr::Mbr::SIZE as u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceMode {
    Graphical,
    Terminal,
}

pub struct Config {
    pub interface: InterfaceMode,
    pub networking: bool,
    pub resolution: (i16, i16),
    pub disk_path: String,
    pub disk_size: u64,
    pub interface_name: String,
}

#[derive(GetArgs)]
struct Arguments<'a> {
    #[arg(flag, short = 'g', long = "gui")]
    gui: bool,
    #[arg(flag, short = 't', long = "terminal")]
    terminal: bool,
    #[arg(flag, short = 'n', long = "network")]
    network: bool,
    #[arg(flag, short = 'N', long = "no-network")]
    no_network: bool,
    #[arg(short = 'r', long = "resolution", default = "800x600")]
    resolution: &'a str,
    #[arg(short = 'p', long = "disk-path", default = "./drive.img")]
    disk_path: &'a str,
    #[arg(short = 's', long = "disk-size", default = 16 * 1024 * 1024)]
    disk_size: u64,
    #[arg(short = 'i', long = "interface-name", default = "xila0")]
    interface_name: &'a str,
}

pub fn parse<'a, I>(arguments: I) -> Result<Config, String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut options = getargs::Options::new(arguments.into_iter());
    let arguments = Arguments::parse(&mut options).map_err(|error| error.to_string())?;

    if arguments.gui && arguments.terminal {
        return Err("--gui and --terminal cannot be used together".to_string());
    }

    if arguments.network && arguments.no_network {
        return Err("--network and --no-network cannot be used together".to_string());
    }

    let (width, height) = arguments.resolution.split_once('x').ok_or_else(|| {
        "--resolution must be in WIDTHxHEIGHT form (for example, 800x600)".to_string()
    })?;
    let width = width
        .parse::<i16>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "--resolution width must be between 1 and 32767".to_string())?;
    let height = height
        .parse::<i16>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "--resolution height must be between 1 and 32767".to_string())?;

    if arguments.disk_size < MINIMUM_DISK_SIZE {
        return Err(format!(
            "--disk-size must be at least {MINIMUM_DISK_SIZE} bytes for the MBR and filesystem"
        ));
    }

    if arguments.interface_name.is_empty() {
        return Err("--interface-name cannot be empty".to_string());
    }

    Ok(Config {
        interface: if arguments.terminal {
            InterfaceMode::Terminal
        } else {
            InterfaceMode::Graphical
        },
        networking: !arguments.no_network,
        resolution: (width, height),
        disk_path: arguments.disk_path.to_string(),
        disk_size: arguments.disk_size,
        interface_name: arguments.interface_name.to_string(),
    })
}

pub const HELP: &str = "Usage: native_example [OPTIONS]\n\
Run the Xila native example. Defaults preserve the current setup.\n\
\n\
Options:\n\
  -g, --gui                    Use the graphical interface (default)\n\
  -t, --terminal               Use the terminal interface\n\
  -n, --network                Enable networking (default)\n\
  -N, --no-network             Disable networking\n\
  -r, --resolution WIDTHxHEIGHT\n\
                               Display resolution (default: 800x600)\n\
  -p, --disk-path PATH         Backing disk image (default: ./drive.img)\n\
  -s, --disk-size BYTES        Capacity in bytes (default: 16777216; minimum: 1049600)\n\
  -i, --interface-name NAME    Host TUN/TAP interface (default: xila0)\n\
  -h, --help                   Show this help\n";

#[cfg(test)]
mod tests {
    use super::{InterfaceMode, MINIMUM_DISK_SIZE, parse};

    #[test]
    fn defaults_preserve_the_current_example_setup() {
        let config = parse([] as [&str; 0]).unwrap();

        assert_eq!(config.interface, InterfaceMode::Graphical);
        assert!(config.networking);
        assert_eq!(config.resolution, (800, 600));
        assert_eq!(config.disk_path, "./drive.img");
        assert_eq!(config.disk_size, 16 * 1024 * 1024);
        assert_eq!(config.interface_name, "xila0");
    }

    #[test]
    fn parses_runtime_options() {
        let config = parse([
            "--terminal",
            "--no-network",
            "--resolution",
            "1024x768",
            "--disk-path",
            "/tmp/xila.img",
            "--disk-size",
            "33554432",
            "--interface-name",
            "xila1",
        ])
        .unwrap();

        assert_eq!(config.interface, InterfaceMode::Terminal);
        assert!(!config.networking);
        assert_eq!(config.resolution, (1024, 768));
        assert_eq!(config.disk_path, "/tmp/xila.img");
        assert_eq!(config.disk_size, 33_554_432);
        assert_eq!(config.interface_name, "xila1");
    }

    #[test]
    fn rejects_conflicting_modes() {
        assert!(parse(["--terminal", "--gui"]).is_err());
        assert!(parse(["--network", "--no-network"]).is_err());
    }

    #[test]
    fn rejects_invalid_values() {
        assert!(parse(["--resolution", "0x600"]).is_err());
        assert!(parse(["--resolution", "800"]).is_err());
        assert!(parse(["--resolution", "32768x600"]).is_err());
        let below_minimum = (MINIMUM_DISK_SIZE - 1).to_string();
        assert!(parse(["--disk-size", &below_minimum]).is_err());
        let minimum = MINIMUM_DISK_SIZE.to_string();
        assert!(parse(["--disk-size", &minimum]).is_ok());
        assert!(parse(["--interface-name", ""]).is_err());
        assert!(parse(["--unknown"]).is_err());
    }
}
