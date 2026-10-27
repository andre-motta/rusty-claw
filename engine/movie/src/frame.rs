//! A decoded picture.

use std::time::Duration;

/// The colour matrix of a YUV picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Matrix {
    /// ITU-R BT.601 (standard definition).
    Bt601,
    /// ITU-R BT.709 (high definition).
    Bt709,
}

impl Matrix {
    /// The matrix to assume for a picture `height` lines high that names
    /// none: BT.601 up to 576 lines, BT.709 above.
    pub fn for_untagged(height: u32) -> Matrix {
        if height <= 576 {
            Matrix::Bt601
        } else {
            Matrix::Bt709
        }
    }
}

/// Where the colour (U and V) samples of a picture with half-width colour
/// planes sit across each row, against the brightness (Y) samples.
/// Vertically they sit halfway between two rows either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChromaSiting {
    /// Level with the left sample of each pair of Y samples: MPEG-2,
    /// MPEG-4, H.264, H.265 and AV1 pictures, unless the stream says
    /// otherwise.
    Left,
    /// Halfway between each pair of Y samples (MPEG-1 and JPEG pictures,
    /// and streams tagged so).
    Centre,
}

/// How a picture's bytes are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameLayout {
    /// One plane of red, green, blue and alpha bytes per pixel, in sRGB.
    Rgba,
    /// Three 8-bit planes: Y at full size, U and V at half width and half
    /// height.
    I420 {
        /// The colour matrix.
        matrix: Matrix,
        /// Full range (0 to 255) rather than limited range (16 to 235).
        full_range: bool,
    },
}

/// The largest picture side the player decodes, in pixels: a movie or a
/// picture with a wider or taller one is refused as unsupported before any
/// picture is made for it. It is the usual largest texture side of the
/// renderer's GPUs, far above any upscaled movie.
pub const MAX_PICTURE_SIDE: u32 = 8192;

/// One decoded picture and the time it shows.
#[derive(Clone, PartialEq, Eq)]
pub struct VideoFrame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// When it shows, from the movie's start.
    pub pts: Duration,
    /// How the planes are laid out.
    pub layout: FrameLayout,
    /// The planes; [`FrameLayout::Rgba`] uses the first only.
    pub planes: [Vec<u8>; 3],
    /// Bytes from one row to the next in each plane.
    pub strides: [usize; 3],
    /// Where the colour samples of an [`FrameLayout::I420`] picture sit
    /// across each row ([`FrameLayout::Rgba`] pictures leave it unused).
    pub chroma: ChromaSiting,
}

impl std::fmt::Debug for VideoFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoFrame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pts", &self.pts)
            .field("layout", &self.layout)
            .field("strides", &self.strides)
            .field("chroma", &self.chroma)
            .finish_non_exhaustive()
    }
}

impl VideoFrame {
    /// The red, green, blue and alpha bytes of pixel (x, y) of an
    /// [`FrameLayout::Rgba`] picture.
    pub fn rgba(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if self.layout != FrameLayout::Rgba || x >= self.width || y >= self.height {
            return None;
        }
        let at = y as usize * self.strides[0] + x as usize * 4;
        let p = self.planes[0].get(at..at + 4)?;
        Some([p[0], p[1], p[2], p[3]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untagged_pictures_take_the_matrix_of_their_size() {
        assert_eq!(Matrix::for_untagged(240), Matrix::Bt601);
        assert_eq!(Matrix::for_untagged(480), Matrix::Bt601);
        assert_eq!(Matrix::for_untagged(576), Matrix::Bt601);
        assert_eq!(Matrix::for_untagged(720), Matrix::Bt709);
        assert_eq!(Matrix::for_untagged(1920), Matrix::Bt709);
    }

    #[test]
    fn pixels_are_read_through_the_stride() {
        let f = VideoFrame {
            width: 2,
            height: 2,
            pts: Duration::ZERO,
            layout: FrameLayout::Rgba,
            planes: [
                vec![
                    1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 9, 10, 11, 12, 13, 14, 15, 16, 0, 0,
                ],
                Vec::new(),
                Vec::new(),
            ],
            strides: [10, 0, 0],
            chroma: ChromaSiting::Centre,
        };
        assert_eq!(f.rgba(1, 0), Some([5, 6, 7, 8]));
        assert_eq!(f.rgba(0, 1), Some([9, 10, 11, 12]));
        assert_eq!(f.rgba(2, 0), None);
    }
}
