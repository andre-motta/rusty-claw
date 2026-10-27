//! Decoded pictures to the player's frame layout: RGBA through swscale,
//! with the YUV matrix and range the stream names (BT.601 for untagged
//! pictures of 576 lines or fewer, BT.709 above).

use std::ffi::c_int;
use std::ptr;
use std::time::Duration;

use rusty_ffmpeg::ffi;

use crate::MovieError;
use crate::frame::{FrameLayout, Matrix, VideoFrame};

/// What a scaler was made for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Key {
    width: c_int,
    height: c_int,
    format: ffi::AVPixelFormat,
    yuv: Option<(Matrix, bool)>,
}

/// Converts decoded pictures to [`VideoFrame`]s.
pub(super) struct Converter {
    sws: *mut ffi::SwsContext,
    key: Option<Key>,
}

impl Converter {
    pub(super) fn new() -> Converter {
        Converter {
            sws: ptr::null_mut(),
            key: None,
        }
    }

    /// The RGBA picture of `frame`, shown at `pts`.
    ///
    /// # Safety
    /// `frame` is a valid decoded video frame.
    pub(super) unsafe fn convert(
        &mut self,
        frame: *const ffi::AVFrame,
        pts: Duration,
    ) -> Result<VideoFrame, MovieError> {
        // SAFETY: the caller hands a valid frame.
        let f = unsafe { &*frame };
        let (width, height) = (f.width, f.height);
        if width <= 0 || height <= 0 {
            return Err(MovieError::Decode("a picture without a size".into()));
        }
        let format = f.format as ffi::AVPixelFormat;
        // SAFETY: plain lookups of static tables.
        let yuv = unsafe { yuv_details(f) };
        let key = Key {
            width,
            height,
            format,
            yuv,
        };
        if self.key != Some(key) {
            // SAFETY: sws_getCachedContext frees or reuses the old context.
            self.sws = unsafe {
                ffi::sws_getCachedContext(
                    self.sws,
                    width,
                    height,
                    format,
                    width,
                    height,
                    ffi::AV_PIX_FMT_RGBA,
                    ffi::SWS_BILINEAR as c_int,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null(),
                )
            };
            if self.sws.is_null() {
                self.key = None;
                return Err(MovieError::Unsupported(format!(
                    "cannot convert pictures of pixel format {format}"
                )));
            }
            if let Some((matrix, full)) = yuv {
                let space = match matrix {
                    Matrix::Bt601 => ffi::SWS_CS_ITU601,
                    Matrix::Bt709 => ffi::SWS_CS_ITU709,
                } as c_int;
                // SAFETY: the coefficient tables are static; the context is
                // valid.
                unsafe {
                    ffi::sws_setColorspaceDetails(
                        self.sws,
                        ffi::sws_getCoefficients(space),
                        c_int::from(full),
                        ffi::sws_getCoefficients(ffi::SWS_CS_DEFAULT as c_int),
                        1,
                        0,
                        1 << 16,
                        1 << 16,
                    );
                }
            }
            self.key = Some(key);
        }
        let stride = width as usize * 4;
        let mut rgba = vec![0u8; stride * height as usize];
        let dst = [
            rgba.as_mut_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        ];
        let dst_stride = [stride as c_int, 0, 0, 0];
        // SAFETY: the source planes are the frame's; the destination is a
        // buffer of stride * height bytes.
        let rows = unsafe {
            ffi::sws_scale(
                self.sws,
                f.data.as_ptr() as *const *const u8,
                f.linesize.as_ptr(),
                0,
                height,
                dst.as_ptr(),
                dst_stride.as_ptr(),
            )
        };
        if rows != height {
            return Err(MovieError::Decode(format!(
                "converted {rows} of {height} rows"
            )));
        }
        Ok(VideoFrame {
            width: width as u32,
            height: height as u32,
            pts,
            layout: FrameLayout::Rgba,
            planes: [rgba, Vec::new(), Vec::new()],
            strides: [stride, 0, 0],
        })
    }
}

impl Drop for Converter {
    fn drop(&mut self) {
        // SAFETY: the context is ours or NULL.
        unsafe { ffi::sws_freeContext(self.sws) };
    }
}

/// The matrix and range of a YUV picture, or `None` for RGB and palette
/// pictures.
///
/// # Safety
/// `f` is a valid frame.
unsafe fn yuv_details(f: &ffi::AVFrame) -> Option<(Matrix, bool)> {
    let format = f.format as ffi::AVPixelFormat;
    // SAFETY: a lookup in a static table; NULL for unknown formats.
    let desc = unsafe { ffi::av_pix_fmt_desc_get(format) };
    if desc.is_null() {
        return None;
    }
    // SAFETY: checked for NULL above.
    let desc = unsafe { &*desc };
    let flags = desc.flags;
    if flags & u64::from(ffi::AV_PIX_FMT_FLAG_RGB | ffi::AV_PIX_FMT_FLAG_PAL) != 0
        || desc.nb_components < 3
    {
        return None;
    }
    let matrix = match f.colorspace {
        ffi::AVCOL_SPC_BT709 => Matrix::Bt709,
        ffi::AVCOL_SPC_BT470BG | ffi::AVCOL_SPC_SMPTE170M | ffi::AVCOL_SPC_FCC => Matrix::Bt601,
        _ => Matrix::for_untagged(f.height.max(0) as u32),
    };
    let full = f.color_range == ffi::AVCOL_RANGE_JPEG
        || matches!(
            format,
            ffi::AV_PIX_FMT_YUVJ420P | ffi::AV_PIX_FMT_YUVJ422P | ffi::AV_PIX_FMT_YUVJ444P
        );
    Some((matrix, full))
}
