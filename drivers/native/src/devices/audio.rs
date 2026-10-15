//! Native PCM output. The runner owns SDL and must stay on its creating thread.

use ::audio::{Configuration, FailureReporter, OutputDevice, PlaybackBuffer, Renderer};
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired, AudioStatus};

#[derive(Debug)]
pub enum Error {
    InvalidConfiguration(file_system::Error),
    Initialize(String),
    Open(String),
    UnsupportedConfiguration,
    Stopped,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfiguration(error) => {
                write!(formatter, "Invalid PCM configuration: {error}")
            }
            Self::Initialize(error) => {
                write!(formatter, "Failed to initialize native audio: {error}")
            }
            Self::Open(error) => write!(formatter, "Failed to open native audio: {error}"),
            Self::UnsupportedConfiguration => {
                write!(formatter, "Unsupported native audio configuration")
            }
            Self::Stopped => write!(formatter, "Native audio stream stopped"),
        }
    }
}

impl std::error::Error for Error {}

struct Callback<const N: usize>(Renderer<'static, N>);

impl<const N: usize> AudioCallback for Callback<N> {
    type Channel = f32;

    fn callback(&mut self, output: &mut [f32]) {
        self.0.render_f32(output);
    }
}

/// Owns the native stream. Dropping it invalidates the output device.
/// SDL initialization and teardown must happen on the same thread.
pub struct Runner<const N: usize> {
    device: AudioDevice<Callback<N>>,
    failure: FailureReporter<'static>,
    // Keep the SDL context alive until after the device has been closed.
    _context: sdl2::Sdl,
}

impl<const N: usize> Runner<N> {
    /// Poll host health while feeding audio. This does not allocate on success.
    pub fn check(&self) -> Result<(), Error> {
        if self.device.status() != AudioStatus::Playing {
            self.failure.fail();
            return Err(Error::Stopped);
        }
        Ok(())
    }

    /// Host callback frames, useful when allowing for host latency at shutdown.
    pub fn callback_frames(&self) -> u16 {
        self.device.spec().samples
    }
}

impl<const N: usize> Drop for Runner<N> {
    fn drop(&mut self) {
        self.failure.fail();
    }
}

fn validate_spec(configuration: Configuration, rate: i32, channels: u8) -> Result<(), Error> {
    configuration
        .validate()
        .map_err(Error::InvalidConfiguration)?;
    if rate <= 0 || rate as u32 != configuration.sample_rate || channels != configuration.channels {
        return Err(Error::UnsupportedConfiguration);
    }
    Ok(())
}

/// Opens default host output with the exact input rate and channel count.
///
/// PCM sample format conversion and volume run through nanoaudio directly into
/// SDL's F32 callback buffer. SDL may perform further hardware adaptation.
/// Storage must be static because SDL invokes callbacks on a background thread,
/// including if a caller forgets the returned runner. The device starts paused;
/// use the `audio::START` control command after optionally prefilling it.
pub fn new<const N: usize>(
    buffer: &'static mut PlaybackBuffer<N>,
    configuration: Configuration,
) -> Result<(OutputDevice<'static, N>, Runner<N>), Error> {
    let rate =
        i32::try_from(configuration.sample_rate).map_err(|_| Error::UnsupportedConfiguration)?;
    validate_spec(configuration, rate, configuration.channels)?;
    let context = sdl2::init().map_err(Error::Initialize)?;
    let subsystem = context.audio().map_err(Error::Initialize)?;
    let (output, renderer) = buffer
        .split(configuration)
        .map_err(Error::InvalidConfiguration)?;
    let failure = renderer.failure_reporter();
    let desired = AudioSpecDesired {
        freq: Some(rate),
        channels: Some(configuration.channels),
        samples: Some(512),
    };
    let device = subsystem
        .open_playback(None, &desired, |_| Callback(renderer))
        .map_err(Error::Open)?;
    validate_spec(configuration, device.spec().freq, device.spec().channels)?;
    // Callbacks stay active; START/STOP controls consumption of the PCM queue.
    device.resume();
    let runner = Runner {
        device,
        failure,
        _context: context,
    };
    runner.check()?;
    Ok((output, runner))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_host_spec_without_changing_rate_or_channels() {
        let configuration = audio::Configuration::new(48_000, 2, audio::SampleFormat::I16);
        assert!(validate_spec(configuration, 48_000, 2).is_ok());
        assert!(validate_spec(configuration, 44_100, 2).is_err());
        assert!(validate_spec(configuration, 48_000, 1).is_err());
        assert!(validate_spec(configuration, -1, 2).is_err());
    }

    #[test]
    #[ignore = "run with SDL_AUDIODRIVER=dummy; exercises the native callback thread"]
    fn dummy_playback_and_teardown() {
        use file_system::{ControlCommand, DirectBaseOperations};
        use shared::AnyByLayout;
        use std::time::{Duration, Instant};

        let buffer = Box::leak(Box::new(PlaybackBuffer::<4096>::new()));
        let configuration = Configuration::new(48_000, 2, audio::SampleFormat::I16);
        let (device, runner) = new(buffer, configuration).unwrap();
        assert_eq!(device.write(&[0; 4096], 0), Ok(4096));
        assert_eq!(device.flush(), Err(file_system::Error::RessourceBusy));
        device
            .control(
                audio::START::IDENTIFIER,
                AnyByLayout::from(&()),
                AnyByLayout::NONE,
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while device.flush() == Err(file_system::Error::RessourceBusy) {
            runner.check().unwrap();
            assert!(
                Instant::now() < deadline,
                "native callback did not drain the queue"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(device.flush(), Ok(()));
        drop(runner);
        assert_eq!(
            device.write(&[0; 4], 0),
            Err(file_system::Error::InputOutput)
        );

        // An unexpectedly paused/stopped host stream must fail the device,
        // rather than leave writers indefinitely waiting for a callback.
        let buffer = Box::leak(Box::new(PlaybackBuffer::<16>::new()));
        let (device, runner) = new(buffer, configuration).unwrap();
        runner.device.pause();
        assert!(matches!(runner.check(), Err(Error::Stopped)));
        assert_eq!(device.flush(), Err(file_system::Error::InputOutput));
    }
}
