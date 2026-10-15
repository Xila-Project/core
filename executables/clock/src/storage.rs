//! Two small alternating records retain the last valid settings after a torn write.
//! Durability still depends on the host filesystem; WASI has no sync/rename here.
use crate::{SETTINGS_SIZE, Settings};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::PathBuf,
};

const RECORD_SIZE: usize = 8 + SETTINGS_SIZE + 4;

pub struct Store {
    directory: PathBuf,
    generation: u64,
}

impl Store {
    pub fn load(directory: impl Into<PathBuf>) -> io::Result<(Self, Settings)> {
        let directory = directory.into();
        let mut latest = None;
        let mut corrupt = false;
        for index in 0..2 {
            let mut file = match File::open(directory.join(slot(index))) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            let mut bytes = [0; RECORD_SIZE];
            let complete = match file.read_exact(&mut bytes) {
                Ok(()) => true,
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => false,
                Err(error) => return Err(error),
            };
            let record = if complete && file.read(&mut [0])? == 0 {
                decode(&bytes)
            } else {
                None
            };
            if let Some((generation, settings)) = record {
                if latest.is_none_or(|(old, _)| generation > old) {
                    latest = Some((generation, settings));
                }
            } else {
                corrupt = true;
            }
        }
        match latest {
            Some((generation, settings)) => Ok((
                Self {
                    directory,
                    generation,
                },
                settings,
            )),
            None if corrupt => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Clock settings are damaged",
            )),
            None => Ok((
                Self {
                    directory,
                    generation: 0,
                },
                Settings::default(),
            )),
        }
    }
    pub fn save(&mut self, settings: &Settings) -> io::Result<()> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| io::Error::other("Clock settings generation overflow"))?;
        // A pre-provisioned directory can be writable even when its protected
        // parent is not. Do not ask WASI mkdir to recreate that existing path.
        match fs::metadata(&self.directory) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    "Clock settings path is not a directory",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir_all(&self.directory)?
            }
            Err(error) => return Err(error),
        }
        let mut bytes = [0; RECORD_SIZE];
        bytes[..8].copy_from_slice(&generation.to_le_bytes());
        bytes[8..8 + SETTINGS_SIZE].copy_from_slice(&settings.encode());
        let checksum = checksum(&bytes[..RECORD_SIZE - 4]);
        bytes[RECORD_SIZE - 4..].copy_from_slice(&checksum.to_le_bytes());
        let mut file = File::create(self.directory.join(slot((generation % 2) as usize)))?;
        file.write_all(&bytes)?;
        file.flush()?;
        self.generation = generation;
        Ok(())
    }
}

fn slot(index: usize) -> &'static str {
    if index == 0 {
        "settings.0"
    } else {
        "settings.1"
    }
}
fn checksum(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c9dc5u32, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(0x01000193)
    })
}
fn decode(bytes: &[u8]) -> Option<(u64, Settings)> {
    if bytes.len() != RECORD_SIZE {
        return None;
    }
    let expected = u32::from_le_bytes(bytes[RECORD_SIZE - 4..].try_into().ok()?);
    if expected != checksum(&bytes[..RECORD_SIZE - 4]) {
        return None;
    }
    Some((
        u64::from_le_bytes(bytes[..8].try_into().ok()?),
        Settings::decode(&bytes[8..8 + SETTINGS_SIZE])?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interrupted_latest_write_keeps_the_previous_settings() {
        let path = std::env::temp_dir().join(std::format!("xila-clock-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        let (mut store, initial) = Store::load(&path).unwrap();
        store.save(&initial).unwrap();
        let modified = Settings {
            twelve_hour: true,
            ..initial
        };
        store.save(&modified).unwrap();
        assert!(Store::load(&path).unwrap().1.twelve_hour);
        fs::write(path.join("settings.0"), b"torn write").unwrap();
        assert!(!Store::load(&path).unwrap().1.twelve_hour);
        fs::write(path.join("settings.1"), [0; RECORD_SIZE + 10]).unwrap();
        assert!(Store::load(&path).is_err());
        fs::remove_dir_all(path).unwrap();
    }
}
