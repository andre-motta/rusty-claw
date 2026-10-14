//! PAL palettes: 256 colours of three bytes (red, green, blue), full 8-bit
//! intensities, 768 bytes in all. Bytes after the first 768 are ignored.

use crate::error::{Result, format_error};

/// Size of a palette in bytes.
pub const PALETTE_SIZE: usize = 768;

/// A 256-colour palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette(pub [[u8; 3]; 256]);

impl Default for Palette {
    fn default() -> Self {
        Palette([[0; 3]; 256])
    }
}

impl Palette {
    /// Reads a palette from the first 768 bytes of `data`.
    pub fn parse(data: &[u8]) -> Result<Palette> {
        let raw = data.get(..PALETTE_SIZE).ok_or_else(|| {
            format_error(
                "palette",
                format!("{} bytes, need {PALETTE_SIZE}", data.len()),
            )
        })?;
        let mut colors = [[0u8; 3]; 256];
        for (color, rgb) in colors.iter_mut().zip(raw.chunks_exact(3)) {
            color.copy_from_slice(rgb);
        }
        Ok(Palette(colors))
    }

    /// Colour of one index as (red, green, blue).
    pub fn color(&self, index: u8) -> [u8; 3] {
        self.0[usize::from(index)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_ignores_trailing_bytes() {
        let mut data: Vec<u8> = (0..PALETTE_SIZE).map(|i| (i % 256) as u8).collect();
        data.extend_from_slice(b"extra");
        let pal = Palette::parse(&data).unwrap();
        assert_eq!(pal.color(0), [0, 1, 2]);
        assert_eq!(pal.color(255), [253, 254, 255]);
    }

    #[test]
    fn rejects_short_files() {
        assert!(Palette::parse(&[0u8; PALETTE_SIZE - 1]).is_err());
        assert!(Palette::parse(&[]).is_err());
    }
}
