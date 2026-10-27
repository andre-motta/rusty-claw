//! Decoded sound to the soundtrack's layout: interleaved stereo `f32` at
//! the output rate, through swresample.

use std::ffi::c_int;
use std::mem::MaybeUninit;
use std::ptr;

use rusty_ffmpeg::ffi;

use crate::MovieError;

/// What a resampler was made for: input rate, sample format and channels.
type Key = (c_int, c_int, c_int);

/// Converts decoded sound frames to stereo `f32` at one rate.
pub(super) struct Resampler {
    swr: *mut ffi::SwrContext,
    key: Option<Key>,
    out_rate: c_int,
}

impl Resampler {
    pub(super) fn new(out_rate: u32) -> Resampler {
        Resampler {
            swr: ptr::null_mut(),
            key: None,
            out_rate: c_int::try_from(out_rate.max(1)).unwrap_or(48000),
        }
    }

    /// The samples of `frame`, interleaved stereo at the output rate.
    ///
    /// When the sound's rate, sample format or channels change mid-stream,
    /// the samples the old context still holds come first, then the new
    /// context's. When no context can be made for the new sound, the
    /// resampler holds none (and [`Resampler::flush`] gives nothing) until
    /// a frame it can convert comes.
    ///
    /// # Safety
    /// `frame` is a valid decoded audio frame.
    pub(super) unsafe fn convert(
        &mut self,
        frame: *const ffi::AVFrame,
    ) -> Result<Vec<f32>, MovieError> {
        // SAFETY: the caller hands a valid frame.
        let f = unsafe { &*frame };
        let key = (f.sample_rate, f.format, f.ch_layout.nb_channels);
        let mut out = Vec::new();
        if self.key != Some(key) {
            // What the old context holds belongs before the new sound; a
            // context that cannot give it up is dropped all the same.
            out = self.flush().unwrap_or_default();
            self.key = None;
            // SAFETY: the frame's layout is valid for reading. On failure
            // the context is freed and NULL.
            if let Err(e) = unsafe { self.reopen(f) } {
                // The old context's sound still plays; this frame's is lost.
                return if out.is_empty() { Err(e) } else { Ok(out) };
            }
            self.key = Some(key);
        }
        // SAFETY: the context is initialised; the input planes are the
        // frame's own.
        match unsafe { self.run(f.extended_data as *const *const u8, f.nb_samples) } {
            Ok(samples) if out.is_empty() => Ok(samples),
            Ok(samples) => {
                out.extend_from_slice(&samples);
                Ok(out)
            }
            Err(_) if !out.is_empty() => Ok(out),
            Err(e) => Err(e),
        }
    }

    /// The samples still held inside the resampler, at the end (nothing
    /// when it holds no context).
    pub(super) fn flush(&mut self) -> Result<Vec<f32>, MovieError> {
        if self.swr.is_null() {
            return Ok(Vec::new());
        }
        // SAFETY: a non-NULL context is always an initialised one (reopen
        // frees it on any failure); a NULL input drains it.
        unsafe { self.run(ptr::null(), 0) }
    }

    /// # Safety
    /// `input` holds `count` samples in the context's input format, or is
    /// NULL with a count of 0.
    unsafe fn run(
        &mut self,
        input: *const *const u8,
        count: c_int,
    ) -> Result<Vec<f32>, MovieError> {
        // SAFETY: the context is initialised.
        let room = unsafe { ffi::swr_get_out_samples(self.swr, count) };
        if room < 0 {
            return Err(MovieError::Decode(ffi::av_err2str(room)));
        }
        let mut out = vec![0f32; room as usize * 2];
        let dst = out.as_mut_ptr() as *mut u8;
        // SAFETY: `out` has room for `room` stereo frames.
        let n = unsafe { ffi::swr_convert(self.swr, &dst, room, input, count) };
        if n < 0 {
            return Err(MovieError::Decode(ffi::av_err2str(n)));
        }
        out.truncate(n as usize * 2);
        Ok(out)
    }

    /// Replaces the context with one for `f`'s sound. On failure the
    /// context is freed and NULL, never left allocated but uninitialised.
    ///
    /// # Safety
    /// `f` is a valid audio frame.
    unsafe fn reopen(&mut self, f: &ffi::AVFrame) -> Result<(), MovieError> {
        // SAFETY: frees the previous context, if any, and NULLs the field.
        unsafe { ffi::swr_free(&mut self.swr) };
        // SAFETY: as above.
        let result = unsafe { self.open(f) };
        if result.is_err() {
            // SAFETY: frees what swr_alloc_set_opts2 made, if anything.
            unsafe { ffi::swr_free(&mut self.swr) };
        }
        result
    }

    /// # Safety
    /// `f` is a valid audio frame and the context is NULL.
    unsafe fn open(&mut self, f: &ffi::AVFrame) -> Result<(), MovieError> {
        let mut input = MaybeUninit::<ffi::AVChannelLayout>::zeroed();
        let mut output = MaybeUninit::<ffi::AVChannelLayout>::zeroed();
        // SAFETY: both layouts are zeroed, a valid empty state, and are
        // uninitialised again before returning; swr copies what it needs.
        unsafe {
            let ret = if f.ch_layout.order == ffi::AV_CHANNEL_ORDER_UNSPEC {
                ffi::av_channel_layout_default(input.as_mut_ptr(), f.ch_layout.nb_channels);
                0
            } else {
                ffi::av_channel_layout_copy(input.as_mut_ptr(), &f.ch_layout)
            };
            ffi::av_channel_layout_default(output.as_mut_ptr(), 2);
            let ret = if ret < 0 {
                ret
            } else {
                ffi::swr_alloc_set_opts2(
                    &mut self.swr,
                    output.as_ptr(),
                    ffi::AV_SAMPLE_FMT_FLT,
                    self.out_rate,
                    input.as_ptr(),
                    f.format,
                    f.sample_rate,
                    0,
                    ptr::null_mut(),
                )
            };
            ffi::av_channel_layout_uninit(input.as_mut_ptr());
            ffi::av_channel_layout_uninit(output.as_mut_ptr());
            if ret < 0 {
                return Err(MovieError::Unsupported(format!(
                    "sound of {} channels at {} Hz: {}",
                    f.ch_layout.nb_channels,
                    f.sample_rate,
                    ffi::av_err2str(ret)
                )));
            }
            let ret = ffi::swr_init(self.swr);
            if ret < 0 {
                return Err(MovieError::Unsupported(ffi::av_err2str(ret)));
            }
        }
        Ok(())
    }
}

impl Drop for Resampler {
    fn drop(&mut self) {
        // SAFETY: the context is ours or NULL.
        unsafe { ffi::swr_free(&mut self.swr) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame of `count` stereo 16-bit samples at `rate` Hz, all `value`.
    struct TestFrame(*mut ffi::AVFrame);

    impl TestFrame {
        fn new(rate: c_int, count: c_int, value: i16) -> TestFrame {
            // SAFETY: a fresh frame, given a layout and a buffer of its own
            // before the samples are written into it.
            unsafe {
                let f = ffi::av_frame_alloc();
                assert!(!f.is_null());
                (*f).format = ffi::AV_SAMPLE_FMT_S16;
                (*f).sample_rate = rate;
                (*f).nb_samples = count;
                ffi::av_channel_layout_default(&mut (*f).ch_layout, 2);
                assert!(ffi::av_frame_get_buffer(f, 0) >= 0);
                let data = (*f).data[0] as *mut i16;
                for i in 0..(count as usize * 2) {
                    *data.add(i) = value;
                }
                TestFrame(f)
            }
        }
    }

    impl Drop for TestFrame {
        fn drop(&mut self) {
            // SAFETY: allocated with av_frame_alloc.
            unsafe { ffi::av_frame_free(&mut self.0) };
        }
    }

    fn convert(r: &mut Resampler, f: &TestFrame) -> Result<Vec<f32>, MovieError> {
        // SAFETY: a valid audio frame.
        unsafe { r.convert(f.0) }
    }

    #[test]
    fn a_context_that_cannot_start_is_freed_and_flush_then_gives_nothing() {
        super::super::quiet_ffmpeg();
        let mut r = Resampler::new(48000);
        // No rate: swr_alloc_set_opts2 takes it, swr_init refuses it.
        let bad = TestFrame::new(0, 64, 1000);
        assert!(convert(&mut r, &bad).is_err());
        assert!(r.swr.is_null(), "the uninitialised context is freed");
        assert_eq!(r.key, None);
        assert!(r.flush().unwrap().is_empty());
        // The next frame it can convert opens a context again.
        let good = TestFrame::new(48000, 480, 1000);
        let out = convert(&mut r, &good).unwrap();
        assert!(!r.swr.is_null());
        assert_eq!(out.len() + r.flush().unwrap().len(), 2 * 480);
        // And a bad frame after a good one leaves no context behind either.
        assert!(convert(&mut r, &bad).is_err());
        assert!(r.swr.is_null());
        assert!(r.flush().unwrap().is_empty());
    }

    #[test]
    fn a_change_of_rate_mid_stream_keeps_the_old_contexts_last_samples() {
        let mut r = Resampler::new(48000);
        let mut out = Vec::new();
        // 0.2 s at 44.1 kHz, then 0.2 s at 32 kHz, then the end.
        for _ in 0..20 {
            out.extend(convert(&mut r, &TestFrame::new(44100, 441, 8000)).unwrap());
        }
        let first = out.len() / 2;
        for _ in 0..20 {
            out.extend(convert(&mut r, &TestFrame::new(32000, 320, -8000)).unwrap());
        }
        out.extend(r.flush().unwrap());
        let frames = out.len() / 2;
        assert!(
            frames.abs_diff(2 * 9600) <= 2,
            "{frames} frames for 0.4 s at 48 kHz"
        );
        // The old sound's tail comes before the new sound.
        assert!(first < 9600, "the old context still held some: {first}");
        let level = 8000.0 / 32768.0;
        let tail = &out[2 * first..2 * 9590];
        assert!(
            tail.iter().all(|s| (s - level).abs() < 0.02),
            "the first sound's last samples"
        );
        let new = &out[2 * 9610..2 * 19000];
        assert!(
            new.iter().all(|s| (s + level).abs() < 0.02),
            "the new sound"
        );
    }
}
