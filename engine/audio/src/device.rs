//! Opening the default output device as a stream that renders an
//! [`AudioState`].
//!
//! The state moves into the stream's callback. When the stream is dropped the
//! callback is dropped with it, and a small guard sends the state back to the
//! host thread, so voices, the bank and the music survive a device change.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{ErrorKind, FromSample, SampleFormat, SizedSample, StreamConfig};

use crate::state::{AudioState, Shared};

/// How long the host waits for the state to come back from a dropped stream.
pub(crate) const RETURN_TIMEOUT: Duration = Duration::from_secs(2);

/// A running device stream.
pub(crate) struct Opened {
    pub stream: cpal::Stream,
    pub back: Receiver<AudioState>,
}

/// A failed open: the state, when it came back, and the reason.
pub(crate) type Failed = (Option<Box<AudioState>>, String);

/// Sends the state back to the host when the callback is dropped.
struct ReturnGuard {
    state: Option<AudioState>,
    back: Sender<AudioState>,
}

impl Drop for ReturnGuard {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            let _ = self.back.send(state);
        }
    }
}

/// Opens the default output device. On failure the state comes back with the
/// reason; it is missing only if the failed stream kept its callback.
pub(crate) fn open(mut state: AudioState) -> Result<Opened, Failed> {
    let host = cpal::default_host();
    let Some(device) = host.default_output_device() else {
        return Err((Some(Box::new(state)), "no output device".into()));
    };
    let supported = match device.default_output_config() {
        Ok(c) => c,
        Err(e) => {
            return Err((
                Some(Box::new(state)),
                format!("no usable output format: {e}"),
            ));
        }
    };
    let config = StreamConfig {
        channels: supported.channels(),
        sample_rate: supported.sample_rate(),
        buffer_size: cpal::BufferSize::Default,
    };
    let rate = config.sample_rate;
    state.mixer.set_out_rate(rate);
    state.shared.device_rate.store(rate, Ordering::Relaxed);
    match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, state),
        SampleFormat::F64 => build::<f64>(&device, config, state),
        SampleFormat::I16 => build::<i16>(&device, config, state),
        SampleFormat::I32 => build::<i32>(&device, config, state),
        SampleFormat::I8 => build::<i8>(&device, config, state),
        SampleFormat::U16 => build::<u16>(&device, config, state),
        SampleFormat::U32 => build::<u32>(&device, config, state),
        SampleFormat::U8 => build::<u8>(&device, config, state),
        other => Err((
            Some(Box::new(state)),
            format!("unsupported sample format {other}"),
        )),
    }
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    state: AudioState,
) -> Result<Opened, Failed>
where
    T: SizedSample + FromSample<f32> + Send + 'static,
{
    let channels = usize::from(config.channels);
    let shared: Arc<Shared> = Arc::clone(&state.shared);
    let (tx, back) = mpsc::channel();
    let mut guard = ReturnGuard {
        state: Some(state),
        back: tx,
    };
    let data = move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
        if let Some(state) = guard.state.as_mut() {
            state.fill(out, channels);
        }
    };
    let error = move |e: cpal::Error| {
        let kind = e.kind();
        if kind == ErrorKind::Xrun {
            shared.xruns.fetch_add(1, Ordering::Relaxed);
        }
        if ends_stream(kind) {
            shared.lost.store(true, Ordering::Release);
        }
    };
    let stream = match device.build_output_stream(config, data, error, None) {
        Ok(s) => s,
        Err(e) => {
            return Err(take_back(
                &back,
                format!("cannot open the output stream: {e}"),
            ));
        }
    };
    if let Err(e) = stream.play() {
        drop(stream);
        return Err(take_back(
            &back,
            format!("cannot start the output stream: {e}"),
        ));
    }
    Ok(Opened { stream, back })
}

/// Whether a stream error means the device or stream is gone. Underruns,
/// reroutes to a new default device and refused real-time priority are
/// glitches the stream recovers from by itself, so they keep the stream; so
/// does any kind not listed here.
pub(crate) fn ends_stream(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::DeviceNotAvailable
            | ErrorKind::StreamInvalidated
            | ErrorKind::HostUnavailable
            | ErrorKind::BackendError
            | ErrorKind::Other
    )
}

/// Collects the state after a failed open. The callback was dropped with the
/// failed stream, so the state is already waiting.
fn take_back(back: &Receiver<AudioState>, reason: String) -> Failed {
    (back.recv_timeout(RETURN_TIMEOUT).ok().map(Box::new), reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_loss_ends_the_stream() {
        for kind in [
            ErrorKind::DeviceNotAvailable,
            ErrorKind::StreamInvalidated,
            ErrorKind::HostUnavailable,
            ErrorKind::BackendError,
            ErrorKind::Other,
        ] {
            assert!(ends_stream(kind), "{kind:?}");
        }
    }

    #[test]
    fn recoverable_glitches_keep_the_stream() {
        for kind in [
            ErrorKind::Xrun,
            ErrorKind::DeviceChanged,
            ErrorKind::RealtimeDenied,
            ErrorKind::DeviceBusy,
            ErrorKind::ResourceExhausted,
        ] {
            assert!(!ends_stream(kind), "{kind:?}");
        }
    }
}
