//! Play a quiet 440 Hz tone through the PCM character-device interface.

use audio::{Configuration, PlaybackBuffer, SET_VOLUME, START, SampleFormat};
use file_system::{ControlCommand, DirectBaseOperations, Error};
use shared::AnyByLayout;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const RATE: u32 = 48_000;
    let configuration = Configuration::new(RATE, 2, SampleFormat::I16);
    // One setup allocation; the native background callback requires static storage.
    let buffer = Box::leak(Box::new(PlaybackBuffer::<4096>::new()));
    let (device, runner) = drivers_native::audio::new(buffer, configuration)?;
    device.control(
        SET_VOLUME::IDENTIFIER,
        AnyByLayout::from(&25u8),
        AnyByLayout::NONE,
    )?;
    device.control(START::IDENTIFIER, AnyByLayout::from(&()), AnyByLayout::NONE)?;

    println!("Playing a two-second 440 Hz tone through native PCM output.");
    let mut bytes = [0u8; 1024];
    let mut frame = 0u32;
    let deadline = Instant::now() + Duration::from_secs(10);
    while frame < RATE * 2 {
        let count = (RATE * 2 - frame).min(256);
        for offset in 0..count {
            let sample: i16 = if ((frame + offset) * 440) % RATE < RATE / 2 {
                4000
            } else {
                -4000
            };
            let start = offset as usize * 4;
            bytes[start..start + 2].copy_from_slice(&sample.to_le_bytes());
            bytes[start + 2..start + 4].copy_from_slice(&sample.to_le_bytes());
        }
        let mut remaining = &bytes[..count as usize * 4];
        while !remaining.is_empty() {
            runner.check()?;
            let written = device.write(remaining, 0)?;
            remaining = &remaining[written..];
            if written == 0 {
                if Instant::now() >= deadline {
                    return Err("native audio timed out".into());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        frame += count;
    }
    loop {
        runner.check()?;
        match device.flush() {
            Ok(()) => break,
            Err(Error::RessourceBusy) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(error.into()),
        }
    }
    // flush reports submission to the callback. Allow one host callback period
    // before closing; physical device latency can be longer.
    std::thread::sleep(Duration::from_secs_f64(
        runner.callback_frames() as f64 / RATE as f64,
    ));
    Ok(())
}
