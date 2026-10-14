//! Sprite draw modes and the colour lookup tables they read.
//!
//! In an 8-bit picture there is no alpha channel, so translucency, shadows
//! and blends work by table lookup on palette indices. For a sprite pixel `s`
//! drawn over destination index `d`, with table `T`, colour value `c`, and
//! two-index lookups `T[row * 256 + column]`:
//!
//! | Mode | Result | Reads the destination |
//! | --- | --- | --- |
//! | normal | `s` | no |
//! | ghost | `T[d * 256 + s]` (default table: the level's averaging table) | yes |
//! | shadow | `T[d * 256 + c]`; `s` gives only the shape (default: averaging) | yes |
//! | remap | `T[s * 256 + c]` (no default table: nothing is drawn without one) | no |
//! | colour fill | `c`; `s` gives only the shape | no |
//! | blend | `R0[R1[d] + trunc((R1[s] - R1[d]) * c / 255)]` with the rank table | yes |
//!
//! The rank table lists the 256 indices by ascending brightness (`R0`,
//! bytes 0..256) and, for each index, its position in that list (`R1`, bytes
//! 256..512), so the blend moves from the destination to the sprite in
//! brightness order: `c = 0` keeps the destination, `c = 255` draws the
//! sprite.
//!
//! The functions here are the reference the shaders are tested against.

use claw_assets::{AssetStore, Clt, CltKind, Palette, name::TYPE_CLT};

/// Size of a pair table.
pub const PAIR_TABLE: usize = 256 * 256;
/// Size of the rank table.
pub const RANK_TABLE: usize = 512;

/// Byte offset of the averaging table in [`LevelTables::packed`].
pub const AVERAGE_OFFSET: u32 = 0;
/// Byte offset of the light table in [`LevelTables::packed`].
pub const LIGHT_OFFSET: u32 = PAIR_TABLE as u32;
/// Byte offset of the rank table in [`LevelTables::packed`].
pub const RANK_OFFSET: u32 = 2 * PAIR_TABLE as u32;

/// How a sprite pixel combines with the destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Effect {
    /// The sprite's own index.
    Normal = 1,
    /// Table lookup of (destination, sprite).
    Ghost = 2,
    /// Table lookup of (destination, colour).
    Shadow = 3,
    /// Table lookup of (sprite, colour).
    Remap = 4,
    /// The colour value.
    ColorFill = 5,
    /// Brightness-order blend of destination and sprite by the colour value.
    Blend = 6,
}

impl Effect {
    /// The effect of a draw-mode number; `None` for modes that draw nothing.
    pub fn from_mode(mode: u32) -> Option<Effect> {
        match mode {
            0 | 1 => Some(Effect::Normal),
            2 => Some(Effect::Ghost),
            3 => Some(Effect::Shadow),
            4 => Some(Effect::Remap),
            5 => Some(Effect::ColorFill),
            6 => Some(Effect::Blend),
            _ => None,
        }
    }

    /// True when the result depends on the index already drawn.
    pub fn reads_destination(self) -> bool {
        matches!(self, Effect::Ghost | Effect::Shadow | Effect::Blend)
    }

    /// The table the effect uses when none is given.
    pub fn default_table(self) -> Table {
        match self {
            Effect::Ghost | Effect::Shadow => Table::Average,
            Effect::Blend => Table::Rank,
            Effect::Normal | Effect::Remap | Effect::ColorFill => Table::None,
        }
    }
}

/// Which level table a sprite reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum Table {
    /// No table.
    #[default]
    None = 0,
    /// The averaging table.
    Average = 1,
    /// The light table.
    Light = 2,
    /// The brightness rank table.
    Rank = 3,
}

impl Table {
    /// Byte offset of the table in [`LevelTables::packed`].
    pub fn offset(self) -> u32 {
        match self {
            Table::None | Table::Average => AVERAGE_OFFSET,
            Table::Light => LIGHT_OFFSET,
            Table::Rank => RANK_OFFSET,
        }
    }
}

/// The result of one sprite pixel, or `None` when nothing is drawn.
pub fn apply(effect: Effect, s: u8, d: u8, color: u8, table: Option<&[u8]>) -> Option<u8> {
    let pair = |row: u8, col: u8| -> Option<u8> {
        table.and_then(|t| t.get(usize::from(row) * 256 + usize::from(col)).copied())
    };
    match effect {
        Effect::Normal => Some(s),
        Effect::Ghost => pair(d, s),
        Effect::Shadow => pair(d, color),
        Effect::Remap => pair(s, color),
        Effect::ColorFill => Some(color),
        Effect::Blend => {
            let t = table.filter(|t| t.len() >= RANK_TABLE)?;
            let rs = i32::from(t[256 + usize::from(s)]);
            let rd = i32::from(t[256 + usize::from(d)]);
            let pos = rd + (rs - rd) * i32::from(color) / 255;
            t.get(usize::try_from(pos).ok()?).copied()
        }
    }
}

/// Brightness of a colour as the tables measure it, 0 to 253.
pub fn luminance(c: [u8; 3]) -> u32 {
    (75 * u32::from(c[0]) + 38 * u32::from(c[1]) + 14 * u32::from(c[2])) / 128
}

/// The palette index nearest to (r, g, b) by squared distance; the lowest
/// index wins a tie.
pub fn nearest_index(palette: &Palette, r: i32, g: i32, b: i32) -> u8 {
    let mut best = 0u8;
    let mut best_d = i64::MAX;
    for (i, c) in palette.0.iter().enumerate() {
        let dr = i64::from(c[0]) - i64::from(r);
        let dg = i64::from(c[1]) - i64::from(g);
        let db = i64::from(c[2]) - i64::from(b);
        let d = dr * dr + dg * dg + db * db;
        if d < best_d {
            best_d = d;
            best = i as u8;
        }
    }
    best
}

/// The averaging table with weights `wa` (destination) and `wb` (sprite)
/// in percent: entry `d * 256 + s` is the index nearest to the weighted
/// average of the two colours.
pub fn average_table(palette: &Palette, wa: i32, wb: i32) -> Vec<u8> {
    let div = ((wa + wb) / 100).max(1);
    let mut out = vec![0u8; PAIR_TABLE];
    for d in 0..256 {
        let dc = palette.0[d];
        for s in 0..256 {
            let sc = palette.0[s];
            let ch = |i: usize| (i32::from(sc[i]) * wb / 100 + i32::from(dc[i]) * wa / 100) / div;
            out[d * 256 + s] = nearest_index(palette, ch(0), ch(1), ch(2));
        }
    }
    out
}

/// The light table: `n` brightened versions of each colour, entry
/// `c * n + k` growing brighter with `k`, from the parameters the level
/// set-up uses (`n` 256, `p` 400, `q` 1, `shift` 16).
pub fn light_table(palette: &Palette, n: usize, p: i32, q: i32, shift: i32) -> Vec<u8> {
    let inv255 = f64::from(1.0_f32 / 255.0);
    let pct = f64::from(0.01_f32);
    let mut out = vec![0u8; 256 * n];
    for (c, rgb) in palette.0.iter().enumerate() {
        let lum = f64::from(luminance(*rgb));
        let f = 1.0 / (lum * inv255 + 1.0);
        for k in 0..n {
            let m = 1.0 + f.powi(q) * f64::from(p - 100) * pct * k as f64 / n as f64;
            let ch = |i: usize| {
                let v = f64::from(i32::from(rgb[i]) + shift) * m;
                if v < 255.0 { v as i32 } else { 255 }
            };
            out[c * n + k] = nearest_index(palette, ch(0), ch(1), ch(2));
        }
    }
    out
}

/// The rank table: indices by ascending brightness, then each index's
/// position. Equal brightness keeps the lower index first.
pub fn rank_table(palette: &Palette) -> Vec<u8> {
    let mut order: Vec<u8> = (0..=255).collect();
    order.sort_by_key(|&i| luminance(palette.0[usize::from(i)]));
    let mut out = vec![0u8; RANK_TABLE];
    for (pos, &i) in order.iter().enumerate() {
        out[pos] = i;
        out[256 + usize::from(i)] = pos as u8;
    }
    out
}

/// The three tables of a level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelTables {
    /// Averaging table (65536 bytes).
    pub average: Vec<u8>,
    /// Light table (65536 bytes).
    pub light: Vec<u8>,
    /// Rank table (512 bytes).
    pub rank: Vec<u8>,
}

impl LevelTables {
    /// Generates every table from the palette.
    pub fn generate(palette: &Palette) -> LevelTables {
        LevelTables {
            average: average_table(palette, 100, 100),
            light: light_table(palette, 256, 400, 1, 16),
            rank: rank_table(palette),
        }
    }

    /// Tables of zeros with a generated rank table, for levels or tests that
    /// draw no table effects.
    pub fn blank(palette: &Palette) -> LevelTables {
        LevelTables {
            average: vec![0; PAIR_TABLE],
            light: vec![0; PAIR_TABLE],
            rank: rank_table(palette),
        }
    }

    /// Reads the level's `PALETTES/LIGHT` and `PALETTES/AVERAGE` tables from
    /// `LEVEL<level>`, generating whichever is missing or malformed; the rank
    /// table is always generated.
    pub fn for_level(store: &AssetStore, level: u32, palette: &Palette) -> LevelTables {
        let read = |name: &str| -> Option<Vec<u8>> {
            let id = store
                .lookup_typed(&format!("LEVEL{level}/PALETTES/{name}"), TYPE_CLT)
                .ok()
                .flatten()?;
            let clt = Clt::parse(&store.read(id).ok()?).ok()?;
            (clt.kind() == CltKind::Pair).then_some(clt.table)
        };
        LevelTables {
            average: read("AVERAGE").unwrap_or_else(|| average_table(palette, 100, 100)),
            light: read("LIGHT").unwrap_or_else(|| light_table(palette, 256, 400, 1, 16)),
            rank: rank_table(palette),
        }
    }

    /// The table bytes of `table`, if it has any.
    pub fn get(&self, table: Table) -> Option<&[u8]> {
        match table {
            Table::None => None,
            Table::Average => Some(&self.average),
            Table::Light => Some(&self.light),
            Table::Rank => Some(&self.rank),
        }
    }

    /// All three tables back to back (averaging, light, rank), padded to a
    /// multiple of four bytes, as the shaders read them.
    pub fn packed(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(2 * PAIR_TABLE + RANK_TABLE);
        let mut push = |t: &[u8], len: usize| {
            let start = out.len();
            out.extend_from_slice(&t[..t.len().min(len)]);
            out.resize(start + len, 0);
        };
        push(&self.average, PAIR_TABLE);
        push(&self.light, PAIR_TABLE);
        push(&self.rank, RANK_TABLE);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey_palette() -> Palette {
        let mut p = Palette::default();
        for (i, c) in p.0.iter_mut().enumerate() {
            *c = [i as u8, i as u8, i as u8];
        }
        p
    }

    #[test]
    fn modes_on_a_tiny_table() {
        let mut t = vec![0u8; PAIR_TABLE];
        for (i, v) in t.iter_mut().enumerate() {
            *v = (i % 251) as u8;
        }
        let at = |a: u8, b: u8| t[usize::from(a) * 256 + usize::from(b)];
        assert_eq!(apply(Effect::Normal, 9, 3, 0x80, None), Some(9));
        assert_eq!(apply(Effect::Ghost, 9, 3, 0x80, Some(&t)), Some(at(3, 9)));
        assert_eq!(
            apply(Effect::Shadow, 9, 3, 0x80, Some(&t)),
            Some(at(3, 0x80))
        );
        assert_eq!(
            apply(Effect::Remap, 9, 3, 0x80, Some(&t)),
            Some(at(9, 0x80))
        );
        assert_eq!(apply(Effect::Remap, 9, 3, 0x80, None), None);
        assert_eq!(apply(Effect::ColorFill, 9, 3, 0x42, None), Some(0x42));
    }

    #[test]
    fn blend_moves_in_brightness_order() {
        let pal = grey_palette();
        let rank = rank_table(&pal);
        assert_eq!(apply(Effect::Blend, 200, 10, 0, Some(&rank)), Some(10));
        assert_eq!(apply(Effect::Blend, 200, 10, 255, Some(&rank)), Some(200));
        let mid = apply(Effect::Blend, 200, 10, 128, Some(&rank)).unwrap();
        assert!(mid > 10 && mid < 200);
        // Downward blends truncate toward zero.
        let down = apply(Effect::Blend, 10, 200, 1, Some(&rank)).unwrap();
        assert_eq!(rank[256 + usize::from(down)], rank[256 + 200]);
    }

    #[test]
    fn average_of_greys() {
        let pal = grey_palette();
        let avg = average_table(&pal, 100, 100);
        assert_eq!(avg[100 * 256 + 50], 75);
        assert_eq!(avg[0], 0);
        assert_eq!(avg[255 * 256 + 255], 255);
        let rank = rank_table(&pal);
        assert_eq!(rank[0], 0);
        assert_eq!(rank[256 + 255], 255);
        assert_eq!(luminance([255, 255, 255]), 253);
        let light = light_table(&pal, 256, 400, 1, 16);
        assert!(light[100 * 256 + 255] > light[100 * 256]);
    }

    #[test]
    fn packed_layout() {
        let pal = grey_palette();
        let tables = LevelTables {
            average: vec![1; PAIR_TABLE],
            light: vec![2; 10],
            rank: rank_table(&pal),
        };
        let p = tables.packed();
        assert_eq!(p.len(), 2 * PAIR_TABLE + RANK_TABLE);
        assert_eq!(p[AVERAGE_OFFSET as usize], 1);
        assert_eq!(p[LIGHT_OFFSET as usize + 9], 2);
        assert_eq!(p[LIGHT_OFFSET as usize + 10], 0);
        assert_eq!(p[RANK_OFFSET as usize + 256 + 7], 7);
    }
}
