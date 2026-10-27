//! Decoded pictures to the player's frame layout.
//!
//! - YUV pictures (MPEG-4, H.264, HEVC, MPEG-2, any chroma layout or bit
//!   depth) become I420: 8-bit Y, U and V planes, U and V at half width
//!   and half height, keeping the stream's matrix and range (BT.601 for
//!   untagged pictures of 576 lines or fewer, BT.709 above). The renderer
//!   turns them into colours. 8-bit 4:2:0 pictures are copied as they are;
//!   others (10-bit HEVC Main10, 4:2:2, 4:4:4, NV12) go through swscale.
//!   Each picture also says where its colour samples sit across the row
//!   ([`ChromaSiting`]): as the stream tags them, else as its codec places
//!   them (left for the MPEG family, which the decoders also report for
//!   untagged H.264, H.265 and MPEG-4 streams; centred for JPEG-range
//!   formats). swscale keeps the samples where they are when the source
//!   has half-width colour already and centres them when it halves full
//!   width colour (4:4:4).
//! - Palette and RGB pictures (Smacker) become RGBA through swscale, so
//!   their palette colours stay exact.

use std::ffi::c_int;
use std::ptr;
use std::time::Duration;

use rusty_ffmpeg::ffi;

use crate::MovieError;
use crate::frame::{ChromaSiting, FrameLayout, MAX_PICTURE_SIDE, Matrix, VideoFrame};

/// Refuses pictures of `width` x `height` with a side over
/// [`MAX_PICTURE_SIDE`], before any buffer is made for them.
pub(super) fn check_size(width: c_int, height: c_int) -> Result<(), MovieError> {
    let max = i64::from(MAX_PICTURE_SIDE);
    if i64::from(width) > max || i64::from(height) > max {
        return Err(MovieError::Unsupported(format!(
            "pictures of {width}x{height}, over {max} pixels on a side"
        )));
    }
    Ok(())
}

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

    /// The picture of `frame`, shown at `pts`: I420 for YUV pictures,
    /// RGBA for the others.
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
        if f.width <= 0 || f.height <= 0 {
            return Err(MovieError::Decode("a picture without a size".into()));
        }
        // The size can change in the middle of a stream.
        check_size(f.width, f.height)?;
        // SAFETY: plain lookups of static tables.
        let yuv = unsafe { yuv_details(f) };
        match yuv {
            Some((matrix, full_range)) => {
                let layout = FrameLayout::I420 { matrix, full_range };
                // SAFETY: a valid frame; its planes hold 4:2:0 8-bit data
                // when copy_i420 says so.
                if let Some(planes) = unsafe { copy_i420(f) } {
                    return Ok(i420_frame(f, pts, layout, planes));
                }
                let mut planes = i420_planes(f.width as usize, f.height as usize);
                // SAFETY: a valid frame and planes of the picture's size.
                unsafe { self.scale(f, ffi::AV_PIX_FMT_YUV420P, yuv, &mut planes) }?;
                Ok(i420_frame(f, pts, layout, planes))
            }
            None => {
                let stride = f.width as usize * 4;
                let mut planes = Planes {
                    data: [
                        vec![0u8; stride * f.height as usize],
                        Vec::new(),
                        Vec::new(),
                    ],
                    strides: [stride, 0, 0],
                };
                // SAFETY: a valid frame and a plane of the picture's size.
                unsafe { self.scale(f, ffi::AV_PIX_FMT_RGBA, None, &mut planes) }?;
                Ok(VideoFrame {
                    width: f.width as u32,
                    height: f.height as u32,
                    pts,
                    layout: FrameLayout::Rgba,
                    planes: planes.data,
                    strides: planes.strides,
                    chroma: ChromaSiting::Centre,
                })
            }
        }
    }

    /// Converts `f` into `out`, laid out as `target` (I420 or RGBA) at the
    /// same size. For YUV to I420 the matrix and range stay as they are;
    /// for YUV to RGBA they are `yuv`.
    ///
    /// # Safety
    /// `f` is a valid frame; `out` holds planes of `target` at its size.
    unsafe fn scale(
        &mut self,
        f: &ffi::AVFrame,
        target: ffi::AVPixelFormat,
        yuv: Option<(Matrix, bool)>,
        out: &mut Planes,
    ) -> Result<(), MovieError> {
        let (width, height) = (f.width, f.height);
        let format = f.format as ffi::AVPixelFormat;
        let key = Key {
            width,
            height,
            format,
            yuv,
        };
        if self.key != Some(key) {
            // The context keeps swscale's default chroma positions (centred
            // samples in what it makes): chroma_siting relies on that.
            // SAFETY: sws_getCachedContext frees or reuses the old context.
            self.sws = unsafe {
                ffi::sws_getCachedContext(
                    self.sws,
                    width,
                    height,
                    format,
                    width,
                    height,
                    target,
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
                // To I420 the destination keeps the source's matrix and
                // range, so only the bit depth and the chroma layout
                // change; to RGBA the destination is full range.
                let (dst_space, dst_full) = if target == ffi::AV_PIX_FMT_RGBA {
                    (ffi::SWS_CS_DEFAULT as c_int, 1)
                } else {
                    (space, c_int::from(full))
                };
                // SAFETY: the coefficient tables are static; the context is
                // valid.
                unsafe {
                    ffi::sws_setColorspaceDetails(
                        self.sws,
                        ffi::sws_getCoefficients(space),
                        c_int::from(full),
                        ffi::sws_getCoefficients(dst_space),
                        dst_full,
                        0,
                        1 << 16,
                        1 << 16,
                    );
                }
            }
            self.key = Some(key);
        }
        let mut dst = [ptr::null_mut::<u8>(); 4];
        let mut dst_stride = [0 as c_int; 4];
        for (i, plane) in out.data.iter_mut().enumerate() {
            if !plane.is_empty() {
                dst[i] = plane.as_mut_ptr();
                dst_stride[i] = out.strides[i] as c_int;
            }
        }
        // SAFETY: the source planes are the frame's; each destination plane
        // holds stride * rows bytes for the target layout at this size.
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
        Ok(())
    }
}

impl Drop for Converter {
    fn drop(&mut self) {
        // SAFETY: the context is ours or NULL.
        unsafe { ffi::sws_freeContext(self.sws) };
    }
}

/// A picture's planes and their row strides.
struct Planes {
    data: [Vec<u8>; 3],
    strides: [usize; 3],
}

/// Empty I420 planes for a `width` x `height` picture, rows packed: the
/// chroma planes are half size, rounded up.
fn i420_planes(width: usize, height: usize) -> Planes {
    let (cw, ch) = (width.div_ceil(2), height.div_ceil(2));
    Planes {
        data: [
            vec![0u8; width * height],
            vec![0u8; cw * ch],
            vec![0u8; cw * ch],
        ],
        strides: [width, cw, cw],
    }
}

fn i420_frame(f: &ffi::AVFrame, pts: Duration, layout: FrameLayout, planes: Planes) -> VideoFrame {
    VideoFrame {
        width: f.width as u32,
        height: f.height as u32,
        pts,
        layout,
        planes: planes.data,
        strides: planes.strides,
        // SAFETY: the caller's frame is a valid decoded frame.
        chroma: unsafe { chroma_siting(f) },
    }
}

/// Where the colour samples of the I420 picture made from `f` sit across
/// each row (see the module documentation).
///
/// # Safety
/// `f` is a valid frame.
unsafe fn chroma_siting(f: &ffi::AVFrame) -> ChromaSiting {
    let format = f.format as ffi::AVPixelFormat;
    // SAFETY: a lookup in a static table; NULL for unknown formats.
    let desc = unsafe { ffi::av_pix_fmt_desc_get(format) };
    // SAFETY: checked for NULL first.
    if !desc.is_null() && unsafe { (*desc).log2_chroma_w } != 1 {
        // swscale makes the half-width colour planes itself, centred.
        return ChromaSiting::Centre;
    }
    siting_of(f.chroma_location, format)
}

/// The colour sample siting a picture of `format` tagged `location` has
/// across each row.
fn siting_of(location: ffi::AVChromaLocation, format: ffi::AVPixelFormat) -> ChromaSiting {
    match location {
        ffi::AVCHROMA_LOC_LEFT | ffi::AVCHROMA_LOC_TOPLEFT | ffi::AVCHROMA_LOC_BOTTOMLEFT => {
            ChromaSiting::Left
        }
        ffi::AVCHROMA_LOC_CENTER | ffi::AVCHROMA_LOC_TOP | ffi::AVCHROMA_LOC_BOTTOM => {
            ChromaSiting::Centre
        }
        // Untagged: JPEG-range formats come from JPEG-style codecs, which
        // centre them; every other codec the player decodes is of the
        // MPEG family.
        _ if matches!(
            format,
            ffi::AV_PIX_FMT_YUVJ420P | ffi::AV_PIX_FMT_YUVJ422P | ffi::AV_PIX_FMT_YUVJ444P
        ) =>
        {
            ChromaSiting::Centre
        }
        _ => ChromaSiting::Left,
    }
}

/// The planes of an 8-bit 4:2:0 picture, copied row by row; `None` for
/// any other layout (or one stored bottom-up), which goes through swscale.
///
/// # Safety
/// `f` is a valid frame.
unsafe fn copy_i420(f: &ffi::AVFrame) -> Option<Planes> {
    let format = f.format as ffi::AVPixelFormat;
    if format != ffi::AV_PIX_FMT_YUV420P && format != ffi::AV_PIX_FMT_YUVJ420P {
        return None;
    }
    if f.linesize[..3].iter().any(|&s| s <= 0) || f.data[..3].iter().any(|p| p.is_null()) {
        return None;
    }
    let (w, h) = (f.width as usize, f.height as usize);
    let mut planes = i420_planes(w, h);
    for i in 0..3 {
        let (pw, ph) = if i == 0 {
            (w, h)
        } else {
            (w.div_ceil(2), h.div_ceil(2))
        };
        let src_stride = f.linesize[i] as usize;
        if src_stride < pw {
            return None;
        }
        let dst = &mut planes.data[i];
        for y in 0..ph {
            // SAFETY: a decoded plane holds `ph` rows of `src_stride` bytes,
            // of which the first `pw` are the picture's.
            let row = unsafe { std::slice::from_raw_parts(f.data[i].add(y * src_stride), pw) };
            dst[y * pw..(y + 1) * pw].copy_from_slice(row);
        }
    }
    Some(planes)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tagged_pictures_keep_their_siting_and_untagged_ones_take_their_codecs() {
        let yuv = ffi::AV_PIX_FMT_YUV420P;
        for loc in [
            ffi::AVCHROMA_LOC_LEFT,
            ffi::AVCHROMA_LOC_TOPLEFT,
            ffi::AVCHROMA_LOC_BOTTOMLEFT,
        ] {
            assert_eq!(siting_of(loc, yuv), ChromaSiting::Left);
        }
        for loc in [
            ffi::AVCHROMA_LOC_CENTER,
            ffi::AVCHROMA_LOC_TOP,
            ffi::AVCHROMA_LOC_BOTTOM,
        ] {
            assert_eq!(siting_of(loc, yuv), ChromaSiting::Centre);
            assert_eq!(
                siting_of(loc, ffi::AV_PIX_FMT_YUVJ420P),
                ChromaSiting::Centre
            );
        }
        assert_eq!(
            siting_of(ffi::AVCHROMA_LOC_UNSPECIFIED, yuv),
            ChromaSiting::Left
        );
        assert_eq!(
            siting_of(ffi::AVCHROMA_LOC_UNSPECIFIED, ffi::AV_PIX_FMT_YUV420P10LE),
            ChromaSiting::Left
        );
        assert_eq!(
            siting_of(ffi::AVCHROMA_LOC_UNSPECIFIED, ffi::AV_PIX_FMT_YUVJ420P),
            ChromaSiting::Centre
        );
        assert_eq!(
            siting_of(ffi::AVCHROMA_LOC_LEFT, ffi::AV_PIX_FMT_YUVJ420P),
            ChromaSiting::Left
        );
    }

    /// A video frame of `width` x `height` in 8-bit 4:2:0, with buffers
    /// only when `buffers` is set.
    struct TestFrame(*mut ffi::AVFrame);

    impl TestFrame {
        fn new(width: c_int, height: c_int, buffers: bool) -> TestFrame {
            // SAFETY: a fresh frame, given a size and a format before its
            // buffers are made.
            unsafe {
                let f = ffi::av_frame_alloc();
                assert!(!f.is_null());
                (*f).width = width;
                (*f).height = height;
                (*f).format = ffi::AV_PIX_FMT_YUV420P;
                if buffers {
                    assert!(ffi::av_frame_get_buffer(f, 0) >= 0);
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

    #[test]
    fn pictures_over_the_largest_side_are_refused_before_any_buffer() {
        let max = MAX_PICTURE_SIDE as c_int;
        assert!(check_size(max, max).is_ok());
        assert!(check_size(1, 1).is_ok());
        for (w, h) in [
            (max + 1, 16),
            (16, max + 1),
            (16000, 16000),
            (c_int::MAX, 2),
        ] {
            assert!(
                matches!(check_size(w, h), Err(MovieError::Unsupported(_))),
                "{w}x{h}"
            );
            // The frame has no planes: had the converter gone on, it would
            // have read them.
            let f = TestFrame::new(w, h, false);
            let mut c = Converter::new();
            // SAFETY: a valid frame (with no buffers, which the size check
            // stops before).
            let got = unsafe { c.convert(f.0, Duration::ZERO) };
            assert!(matches!(got, Err(MovieError::Unsupported(_))), "{got:?}");
        }
        let f = TestFrame::new(32, 16, true);
        // SAFETY: a valid frame with its buffers.
        let got = unsafe { Converter::new().convert(f.0, Duration::ZERO) }.unwrap();
        assert_eq!((got.width, got.height), (32, 16));
    }
}
