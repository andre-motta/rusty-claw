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
    /// # Safety
    /// `frame` is a valid decoded audio frame.
    pub(super) unsafe fn convert(
        &mut self,
        frame: *const ffi::AVFrame,
    ) -> Result<Vec<f32>, MovieError> {
        // SAFETY: the caller hands a valid frame.
        let f = unsafe { &*frame };
        let key = (f.sample_rate, f.format, f.ch_layout.nb_channels);
        if self.key != Some(key) {
            // SAFETY: the frame's layout is valid for reading.
            unsafe { self.reopen(f)? };
            self.key = Some(key);
        }
        // SAFETY: the context is initialised; the input planes are the
        // frame's own.
        unsafe { self.run(f.extended_data as *const *const u8, f.nb_samples) }
    }

    /// The samples still held inside the resampler, at the end.
    pub(super) fn flush(&mut self) -> Result<Vec<f32>, MovieError> {
        if self.swr.is_null() {
            return Ok(Vec::new());
        }
        // SAFETY: a NULL input drains the context.
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

    /// # Safety
    /// `f` is a valid audio frame.
    unsafe fn reopen(&mut self, f: &ffi::AVFrame) -> Result<(), MovieError> {
        // SAFETY: frees the previous context, if any, and NULLs the field.
        unsafe { ffi::swr_free(&mut self.swr) };
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
