//! WWD level files.
//!
//! A level is a fixed 0x5F4-byte header followed by a body, normally one zlib
//! stream (header flag 0x2). Every body offset counts from the start of a
//! buffer that holds the header bytes followed by the inflated body. The body
//! holds:
//!
//! - plane headers (0xA0 bytes each), each plane with a grid of `i32` tile
//!   numbers (-1 empty, `0xEEEEEEEE` filled with the plane's fill colour),
//!   its image-set names and, for the main plane, the object records;
//! - a tile descriptor block giving each tile number its collision
//!   attributes (types 1, 2 and 3).
//!
//! Levels, community custom levels included, are untrusted input: inflate is
//! bounded by the declared size and a hard cap, every offset and count is
//! checked, and malformed files return an error. Planes, objects and tile
//! descriptors may point at any bytes, so the parser also keeps one decode
//! budget for the whole file: together they may not decode more bytes than
//! the buffer holds, which keeps memory use linear in the file size even when
//! many planes point at the same region.

use flate2::{Decompress, FlushDecompress, Status};

use crate::bytes::{cstr_field, i32_at, latin1, slice, to_usize, u32_at};
use crate::error::{Result, format_error};
use crate::name::first_number;

/// Size of the header.
pub const HEADER_SIZE: usize = 0x5F4;
/// Default limit on the inflated body size.
pub const DEFAULT_MAX_INFLATE: usize = 64 << 20;
/// Hard limit on the inflated body size, whatever the caller asks for.
pub const HARD_INFLATE_CAP: usize = 256 << 20;
/// Most planes accepted.
pub const MAX_PLANES: u32 = 64;
/// Size of a plane header.
pub const PLANE_HEADER_SIZE: usize = 0xA0;
/// Size of the fixed part of an object record.
pub const OBJECT_FIXED_SIZE: usize = 0x11C;
/// Number of integers in the fixed part of an object record.
pub const OBJECT_INTS: usize = 71;
/// Size of the tile descriptor block header.
pub const TILE_DESC_BLOCK_HEADER: usize = 0x20;

/// Header flag: the body is zlib-compressed.
pub const FLAG_COMPRESSED: u32 = 0x2;
/// Plane flag: the main plane, which carries the objects and drives the
/// camera.
pub const PLANE_FLAG_MAIN: u32 = 0x1;
/// Tile value of an empty cell.
pub const TILE_EMPTY: i32 = -1;
/// Tile value of a cell filled with the plane's fill colour.
pub const TILE_FILLED: i32 = 0xEEEE_EEEE_u32 as i32;

const FORMAT: &str = "WWD level";

/// The level header.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WwdHeader {
    /// Stored header size, at most [`HEADER_SIZE`].
    pub header_size: u32,
    /// Level flags.
    pub flags: u32,
    /// Level name.
    pub name: String,
    /// Author.
    pub author: String,
    /// Creation date as text.
    pub date: String,
    /// Archive path the level was built against.
    pub rez_path: String,
    /// Folder of the tile image sets.
    pub tiles_path: String,
    /// Path of the level palette.
    pub palette_path: String,
    /// Player start x in main-plane pixels.
    pub start_x: i32,
    /// Player start y in main-plane pixels.
    pub start_y: i32,
    /// Number of planes.
    pub plane_count: u32,
    /// Offset of the first plane header.
    pub planes_offset: u32,
    /// Offset of the tile descriptor block; 0 means none.
    pub tile_descs_offset: u32,
    /// Size of the inflated body.
    pub inflated_size: u32,
    /// Stored check value.
    pub checksum: u32,
    /// Program the level editor launched.
    pub launch_app: String,
    /// Up to four image-set folders.
    pub image_set_paths: [String; 4],
    /// Prefix of each image-set folder.
    pub image_set_prefixes: [String; 4],
}

impl WwdHeader {
    /// True when the body is zlib-compressed.
    pub fn is_compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSED != 0
    }

    /// The retail level a level builds on: the first number in its name.
    pub fn base_level(&self) -> Option<i32> {
        first_number(&self.name)
    }
}

/// One plane: a grid of tile numbers drawn as a layer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plane {
    /// Plane name.
    pub name: String,
    /// Plane flags; [`PLANE_FLAG_MAIN`] marks the main plane.
    pub flags: u32,
    /// Stored width in pixels.
    pub px_w: i32,
    /// Stored height in pixels.
    pub px_h: i32,
    /// Tile width in pixels.
    pub tile_w: i32,
    /// Tile height in pixels.
    pub tile_h: i32,
    /// Width in tiles.
    pub w_tiles: i32,
    /// Height in tiles.
    pub h_tiles: i32,
    /// Initial plane x.
    pub origin_x: i32,
    /// Initial plane y.
    pub origin_y: i32,
    /// Horizontal scroll rate in percent.
    pub move_x_pct: i32,
    /// Vertical scroll rate in percent.
    pub move_y_pct: i32,
    /// Palette index for filled cells.
    pub fill_color: u32,
    /// Draw order key.
    pub z: i32,
    /// Image-set names; the first one supplies the tiles.
    pub image_sets: Vec<String>,
    /// Tile numbers, row-major, top row first.
    pub tiles: Vec<i32>,
}

impl Plane {
    /// True for the main plane.
    pub fn is_main(&self) -> bool {
        self.flags & PLANE_FLAG_MAIN != 0
    }

    /// Tile number at a cell, if the cell exists.
    pub fn tile(&self, col: i32, row: i32) -> Option<i32> {
        if col < 0 || row < 0 || col >= self.w_tiles || row >= self.h_tiles {
            return None;
        }
        let idx = row as usize * self.w_tiles as usize + col as usize;
        self.tiles.get(idx).copied()
    }
}

/// One object placed in a level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectRecord {
    /// Index of the plane that holds the object.
    pub plane: usize,
    /// The 71 integers of the fixed part (indices 1 to 4 are the lengths of
    /// the four strings).
    pub ints: [i32; OBJECT_INTS],
    /// Object name.
    pub name: String,
    /// Logic name.
    pub logic: String,
    /// Image-set registry key.
    pub image_set: String,
    /// Animation or sound key.
    pub anim: String,
}

/// Collision attributes of one tile number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TileDescriptor {
    /// One attribute for the whole tile.
    Single {
        /// Tile width.
        width: i32,
        /// Tile height.
        height: i32,
        /// The attribute.
        attribute: u32,
    },
    /// Attribute `inside` within a rectangle and `outside` elsewhere.
    Double {
        /// Tile width.
        width: i32,
        /// Tile height.
        height: i32,
        /// Attribute outside the rectangle.
        outside: u32,
        /// Attribute inside the rectangle.
        inside: u32,
        /// Rectangle as left, top, right, bottom.
        rect: [i32; 4],
    },
    /// One attribute byte per pixel.
    Mask {
        /// Mask width.
        width: i32,
        /// Mask height.
        height: i32,
        /// `width * height` attribute bytes, row-major.
        mask: Vec<u8>,
    },
}

/// A parsed level.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wwd {
    /// The header.
    pub header: WwdHeader,
    /// Planes in file order.
    pub planes: Vec<Plane>,
    /// Objects of every plane.
    pub objects: Vec<ObjectRecord>,
    /// Tile descriptors, indexed by tile number.
    pub tile_descriptors: Vec<TileDescriptor>,
    /// Input bytes after the end of the zlib stream (ignored).
    pub unused_input: usize,
}

impl Wwd {
    /// Index of the main plane.
    pub fn main_plane(&self) -> Option<usize> {
        self.planes.iter().position(Plane::is_main)
    }

    /// Parses a level with the default inflate limit.
    pub fn parse(data: &[u8]) -> Result<Wwd> {
        Self::parse_with_limit(data, DEFAULT_MAX_INFLATE)
    }

    /// Parses a level, inflating at most `max_inflate` bytes (never more than
    /// [`HARD_INFLATE_CAP`]).
    pub fn parse_with_limit(data: &[u8], max_inflate: usize) -> Result<Wwd> {
        let header = parse_header(data)?;
        let limit = max_inflate.min(HARD_INFLATE_CAP);
        let mut unused_input = 0;
        let owned;
        let buf: &[u8] = if header.is_compressed() {
            let (body, unused) = inflate_body(data, &header, limit)?;
            unused_input = unused;
            owned = body;
            &owned
        } else {
            if data.len() > limit + HEADER_SIZE {
                return Err(format_error(
                    FORMAT,
                    "uncompressed level larger than the limit",
                ));
            }
            data
        };
        if header.plane_count > MAX_PLANES {
            return Err(format_error(
                FORMAT,
                format!("{} planes, at most {MAX_PLANES}", header.plane_count),
            ));
        }
        let mut budget = Budget::new(buf.len());
        let mut planes = Vec::new();
        let mut objects = Vec::new();
        for i in 0..to_usize(header.plane_count) {
            let off = to_usize(header.planes_offset)
                .checked_add(i * PLANE_HEADER_SIZE)
                .ok_or_else(|| format_error(FORMAT, "plane offset overflows"))?;
            let (plane, objs) = parse_plane(buf, off, i, &mut budget)?;
            planes.push(plane);
            objects.extend(objs);
        }
        let tile_descriptors = if header.tile_descs_offset != 0 {
            parse_tile_descriptors(buf, to_usize(header.tile_descs_offset), &mut budget)?
        } else {
            Vec::new()
        };
        Ok(Wwd {
            header,
            planes,
            objects,
            tile_descriptors,
            unused_input,
        })
    }
}

/// Bytes the parser may still decode from the body. Every tile grid,
/// image-set name scan, object record and tile descriptor is charged here
/// before it is copied, so the total decoded never exceeds the buffer size.
#[derive(Debug)]
struct Budget {
    remaining: usize,
}

impl Budget {
    fn new(total: usize) -> Self {
        Budget { remaining: total }
    }

    /// Fails when `len` more bytes do not fit, without charging them.
    fn check(&self, len: usize, what: &str) -> Result<()> {
        if len > self.remaining {
            return Err(format_error(
                FORMAT,
                format!(
                    "{what} of {len} bytes exceeds the remaining decode budget of {} bytes",
                    self.remaining
                ),
            ));
        }
        Ok(())
    }

    /// Charges `len` bytes, or fails when they do not fit.
    fn charge(&mut self, len: usize, what: &str) -> Result<()> {
        self.check(len, what)?;
        self.remaining -= len;
        Ok(())
    }
}

fn need<'a>(buf: &'a [u8], off: usize, len: usize, what: &str) -> Result<&'a [u8]> {
    slice(buf, off, len).ok_or_else(|| {
        format_error(
            FORMAT,
            format!(
                "{what} out of bounds (offset {off}, size {len}, buffer {})",
                buf.len()
            ),
        )
    })
}

/// Parses the header of a level file (compressed or not).
pub fn parse_header(data: &[u8]) -> Result<WwdHeader> {
    let head = need(data, 0, HEADER_SIZE, "header")?;
    let u = |off| u32_at(head, off).unwrap_or(0);
    let i = |off| i32_at(head, off).unwrap_or(0);
    let text = |off: usize, len: usize| cstr_field(&head[off..off + len]);
    let header_size = u(0x00);
    if to_usize(header_size) > HEADER_SIZE {
        return Err(format_error(
            FORMAT,
            format!("header size {header_size} exceeds {HEADER_SIZE}"),
        ));
    }
    Ok(WwdHeader {
        header_size,
        flags: u(0x08),
        name: text(0x10, 64),
        author: text(0x50, 64),
        date: text(0x90, 64),
        rez_path: text(0xD0, 256),
        tiles_path: text(0x1D0, 128),
        palette_path: text(0x250, 128),
        start_x: i(0x2D0),
        start_y: i(0x2D4),
        plane_count: u(0x2DC),
        planes_offset: u(0x2E0),
        tile_descs_offset: u(0x2E4),
        inflated_size: u(0x2E8),
        checksum: u(0x2EC),
        launch_app: text(0x2F4, 128),
        image_set_paths: std::array::from_fn(|k| text(0x374 + k * 128, 128)),
        image_set_prefixes: std::array::from_fn(|k| text(0x574 + k * 32, 32)),
    })
}

/// The retail level a level file builds on: the first number in its header
/// name. `None` when the header is unreadable or the name has no number.
pub fn base_level(data: &[u8]) -> Option<i32> {
    parse_header(data).ok()?.base_level()
}

/// Inflates the body: returns the header bytes followed by the inflated body,
/// and the number of input bytes after the end of the stream.
fn inflate_body(data: &[u8], header: &WwdHeader, limit: usize) -> Result<(Vec<u8>, usize)> {
    let expected = to_usize(header.inflated_size);
    if expected == 0 {
        return Err(format_error(
            FORMAT,
            "compressed level with inflated size 0",
        ));
    }
    if expected > limit {
        return Err(format_error(
            FORMAT,
            format!("inflated size {expected} exceeds the limit {limit}"),
        ));
    }
    let start = to_usize(header.header_size);
    let input = &data[start..];
    let mut out = Vec::with_capacity(start + expected.min(1 << 20));
    out.extend_from_slice(&data[..start]);
    let mut stream = Decompress::new(true);
    let mut chunk = vec![0u8; 64 * 1024];
    let mut produced = 0usize;
    loop {
        let consumed = to_usize_u64(stream.total_in());
        let before_out = stream.total_out();
        let status = stream
            .decompress(
                &input[consumed.min(input.len())..],
                &mut chunk,
                FlushDecompress::None,
            )
            .map_err(|e| format_error(FORMAT, format!("zlib error: {e}")))?;
        let n = to_usize_u64(stream.total_out() - before_out);
        produced += n;
        if produced > expected {
            return Err(format_error(
                FORMAT,
                format!("body inflates past the declared {expected} bytes"),
            ));
        }
        out.extend_from_slice(&chunk[..n]);
        match status {
            Status::StreamEnd => break,
            Status::Ok | Status::BufError => {
                let progressed = n > 0 || to_usize_u64(stream.total_in()) > consumed;
                if !progressed {
                    return Err(format_error(
                        FORMAT,
                        format!("zlib stream ends early after {produced} of {expected} bytes"),
                    ));
                }
            }
        }
    }
    if produced != expected {
        return Err(format_error(
            FORMAT,
            format!("inflated {produced} bytes, header declares {expected}"),
        ));
    }
    let unused = input.len().saturating_sub(to_usize_u64(stream.total_in()));
    Ok((out, unused))
}

fn to_usize_u64(v: u64) -> usize {
    usize::try_from(v).unwrap_or(usize::MAX)
}

/// Reads image-set names: per name, skip bytes that are not NUL and below
/// 0x30 (as signed bytes, so 0x80 and up too), then collect bytes from 0x30
/// to 0x7F. The NUL that ends a name is not stepped over, so after a NUL
/// every further pass finds nothing; the scan stops at the first pass that
/// consumes no bytes, which bounds the work by the buffer size instead of
/// the untrusted count.
fn parse_image_set_names(buf: &[u8], off: usize, count: u32) -> Result<(Vec<String>, usize)> {
    let mut names = Vec::new();
    let mut pos = off;
    let n = buf.len();
    let is_name = |b: u8| (0x30..0x80).contains(&b);
    for _ in 0..count {
        let pass_start = pos;
        while pos < n && buf[pos] != 0 && !is_name(buf[pos]) {
            pos += 1;
        }
        let start = pos;
        while pos < n && is_name(buf[pos]) {
            pos += 1;
        }
        if pos >= n && start == pos {
            return Err(format_error(FORMAT, "image-set names run past the end"));
        }
        if pos > start {
            names.push(latin1(&buf[start..pos]));
        }
        if pos == pass_start {
            break;
        }
    }
    Ok((names, pos - off))
}

fn parse_plane(
    buf: &[u8],
    off: usize,
    index: usize,
    budget: &mut Budget,
) -> Result<(Plane, Vec<ObjectRecord>)> {
    let head = need(buf, off, PLANE_HEADER_SIZE, "plane header")?;
    let u = |o| u32_at(head, o).unwrap_or(0);
    let i = |o| i32_at(head, o).unwrap_or(0);
    if to_usize(u(0x00)) != PLANE_HEADER_SIZE {
        return Err(format_error(
            FORMAT,
            format!(
                "plane {index} header size {} is not {PLANE_HEADER_SIZE}",
                u(0x00)
            ),
        ));
    }
    let mut plane = Plane {
        name: cstr_field(&head[0x10..0x50]),
        flags: u(0x08),
        px_w: i(0x50),
        px_h: i(0x54),
        tile_w: i(0x58),
        tile_h: i(0x5C),
        w_tiles: i(0x60),
        h_tiles: i(0x64),
        origin_x: i(0x68),
        origin_y: i(0x6C),
        move_x_pct: i(0x70),
        move_y_pct: i(0x74),
        fill_color: u(0x78),
        z: i(0x90),
        ..Plane::default()
    };
    let image_set_count = u(0x7C);
    let object_count = to_usize(u(0x80));
    let tiles_offset = to_usize(u(0x84));
    let image_sets_offset = to_usize(u(0x88));
    let objects_offset = to_usize(u(0x8C));
    let (w, h) = (plane.w_tiles, plane.h_tiles);
    if w < 0 || h < 0 {
        return Err(format_error(
            FORMAT,
            format!("plane {index} has a negative size"),
        ));
    }
    let cells = (w as usize)
        .checked_mul(h as usize)
        .and_then(|c| c.checked_mul(4))
        .ok_or_else(|| format_error(FORMAT, format!("plane {index} size overflows")))?;
    let raw = need(buf, tiles_offset, cells, "tile grid")?;
    budget.charge(cells, &format!("plane {index} tile grid"))?;
    plane.tiles = raw
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    if image_set_count != 0 {
        need(buf, image_sets_offset, 1, "image-set names")?;
        // Scan no further than the budget allows, then charge what was read.
        let end = image_sets_offset.saturating_add(budget.remaining);
        let limited = end < buf.len();
        let window = &buf[..end.min(buf.len())];
        let (names, consumed) = parse_image_set_names(window, image_sets_offset, image_set_count)
            .map_err(|e| {
            if limited {
                format_error(
                    FORMAT,
                    format!("plane {index} image-set names exceed the decode budget"),
                )
            } else {
                e
            }
        })?;
        budget.charge(consumed, &format!("plane {index} image-set names"))?;
        plane.image_sets = names;
    }
    let mut objects = Vec::new();
    if objects_offset != 0 && object_count != 0 {
        let total = object_count.checked_mul(OBJECT_FIXED_SIZE).ok_or_else(|| {
            format_error(
                FORMAT,
                format!("plane {index} object count {object_count} cannot fit"),
            )
        })?;
        budget.check(total, &format!("plane {index} object records"))?;
        let mut pos = objects_offset;
        for _ in 0..object_count {
            let (record, size) = parse_object(buf, pos, index, budget)?;
            objects.push(record);
            pos += size;
        }
    }
    Ok((plane, objects))
}

fn parse_object(
    buf: &[u8],
    off: usize,
    plane: usize,
    budget: &mut Budget,
) -> Result<(ObjectRecord, usize)> {
    let fixed = need(buf, off, OBJECT_FIXED_SIZE, "object record")?;
    budget.charge(OBJECT_FIXED_SIZE, "object record")?;
    let ints: [i32; OBJECT_INTS] = std::array::from_fn(|k| i32_at(fixed, k * 4).unwrap_or(0));
    let mut pos = off + OBJECT_FIXED_SIZE;
    let mut strings: [String; 4] = Default::default();
    for (k, s) in strings.iter_mut().enumerate() {
        let len = to_usize(u32_at(fixed, 4 + k * 4).unwrap_or(0));
        let raw = need(buf, pos, len, "object string")?;
        budget.charge(len, "object string")?;
        *s = latin1(raw);
        pos += len;
    }
    let [name, logic, image_set, anim] = strings;
    Ok((
        ObjectRecord {
            plane,
            ints,
            name,
            logic,
            image_set,
            anim,
        },
        pos - off,
    ))
}

fn parse_tile_descriptors(
    buf: &[u8],
    off: usize,
    budget: &mut Budget,
) -> Result<Vec<TileDescriptor>> {
    let block = need(buf, off, TILE_DESC_BLOCK_HEADER, "tile descriptor block")?;
    let count = to_usize(u32_at(block, 0x08).unwrap_or(0));
    let total = count
        .checked_mul(0x10)
        .ok_or_else(|| format_error(FORMAT, format!("tile descriptor count {count} cannot fit")))?;
    budget.check(total, "tile descriptors")?;
    let mut out = Vec::with_capacity(count);
    let mut pos = off + TILE_DESC_BLOCK_HEADER;
    for n in 0..count {
        let head = need(buf, pos, 0x10, "tile descriptor")?;
        let kind = u32_at(head, 0).unwrap_or(0);
        let width = i32_at(head, 8).unwrap_or(0);
        let height = i32_at(head, 12).unwrap_or(0);
        let (desc, size) = match kind {
            1 => {
                let d = need(buf, pos, 0x14, "tile descriptor")?;
                let attribute = u32_at(d, 0x10).unwrap_or(0);
                (
                    TileDescriptor::Single {
                        width,
                        height,
                        attribute,
                    },
                    0x14,
                )
            }
            2 => {
                let d = need(buf, pos, 0x28, "tile descriptor")?;
                let r = |o| i32_at(d, o).unwrap_or(0);
                (
                    TileDescriptor::Double {
                        width,
                        height,
                        outside: u32_at(d, 0x10).unwrap_or(0),
                        inside: u32_at(d, 0x14).unwrap_or(0),
                        rect: [r(0x18), r(0x1C), r(0x20), r(0x24)],
                    },
                    0x28,
                )
            }
            3 => {
                if width <= 0 || height <= 0 {
                    return Err(format_error(
                        FORMAT,
                        format!("mask tile descriptor {n} has size {width}x{height}"),
                    ));
                }
                // The width must be the largest power of two not above the height.
                let expected_width = 1i64 << (31 - (height as u32).leading_zeros());
                if i64::from(width) != expected_width {
                    return Err(format_error(
                        FORMAT,
                        format!("mask tile descriptor {n}: width {width} for height {height}"),
                    ));
                }
                let len = (width as usize)
                    .checked_mul(height as usize)
                    .ok_or_else(|| format_error(FORMAT, "mask size overflows"))?;
                let raw = need(buf, pos + 0x10, len, "tile mask")?;
                budget.charge(0x10 + len, "tile descriptor")?;
                let mask = raw.to_vec();
                (
                    TileDescriptor::Mask {
                        width,
                        height,
                        mask,
                    },
                    0x10 + len,
                )
            }
            other => {
                return Err(format_error(
                    FORMAT,
                    format!("tile descriptor {n} has unknown type {other}"),
                ));
            }
        };
        if !matches!(desc, TileDescriptor::Mask { .. }) {
            budget.charge(size, "tile descriptor")?;
        }
        out.push(desc);
        pos += size;
    }
    Ok(out)
}

/// Builders for synthetic level files, for tests and tools.
pub mod build {
    use super::*;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    /// A plane to write.
    #[derive(Debug, Clone, Default)]
    pub struct PlaneSpec {
        /// Plane name.
        pub name: String,
        /// Plane flags.
        pub flags: u32,
        /// Tile size in pixels.
        pub tile_size: (i32, i32),
        /// Size in tiles.
        pub size: (i32, i32),
        /// Scroll rates in percent.
        pub move_pct: (i32, i32),
        /// Draw order key.
        pub z: i32,
        /// Image-set names, written NUL-separated.
        pub image_sets: Vec<String>,
        /// Tile numbers.
        pub tiles: Vec<i32>,
        /// Objects as (name, logic, image set, animation).
        pub objects: Vec<[String; 4]>,
    }

    /// Writes a level: header, then (compressed when `compress`) plane
    /// headers, grids, names, objects and descriptors.
    pub fn level(
        name: &str,
        start: (i32, i32),
        planes: &[PlaneSpec],
        descriptors: &[TileDescriptor],
        compress: bool,
    ) -> Vec<u8> {
        let base = HEADER_SIZE;
        let headers_len = planes.len() * PLANE_HEADER_SIZE;
        let mut tail = Vec::new();
        let mut plane_heads = Vec::new();
        for p in planes {
            let tiles_off = base + headers_len + tail.len();
            for t in &p.tiles {
                tail.extend_from_slice(&t.to_le_bytes());
            }
            let names_off = base + headers_len + tail.len();
            for n in &p.image_sets {
                tail.extend_from_slice(n.as_bytes());
                tail.push(0);
            }
            let objects_off = if p.objects.is_empty() {
                0
            } else {
                base + headers_len + tail.len()
            };
            for o in &p.objects {
                let mut ints = [0i32; OBJECT_INTS];
                for (k, s) in o.iter().enumerate() {
                    ints[1 + k] = s.len() as i32;
                }
                for v in ints {
                    tail.extend_from_slice(&v.to_le_bytes());
                }
                for s in o {
                    tail.extend_from_slice(s.as_bytes());
                }
            }
            let mut head = vec![0u8; PLANE_HEADER_SIZE];
            let put = |h: &mut Vec<u8>, off: usize, v: u32| {
                h[off..off + 4].copy_from_slice(&v.to_le_bytes());
            };
            put(&mut head, 0x00, PLANE_HEADER_SIZE as u32);
            put(&mut head, 0x08, p.flags);
            head[0x10..0x10 + p.name.len()].copy_from_slice(p.name.as_bytes());
            put(&mut head, 0x50, (p.tile_size.0 * p.size.0) as u32);
            put(&mut head, 0x54, (p.tile_size.1 * p.size.1) as u32);
            put(&mut head, 0x58, p.tile_size.0 as u32);
            put(&mut head, 0x5C, p.tile_size.1 as u32);
            put(&mut head, 0x60, p.size.0 as u32);
            put(&mut head, 0x64, p.size.1 as u32);
            put(&mut head, 0x70, p.move_pct.0 as u32);
            put(&mut head, 0x74, p.move_pct.1 as u32);
            put(&mut head, 0x7C, p.image_sets.len() as u32);
            put(&mut head, 0x80, p.objects.len() as u32);
            put(&mut head, 0x84, tiles_off as u32);
            put(&mut head, 0x88, names_off as u32);
            put(&mut head, 0x8C, objects_off as u32);
            put(&mut head, 0x90, p.z as u32);
            plane_heads.extend_from_slice(&head);
        }
        let descs_off = if descriptors.is_empty() {
            0
        } else {
            base + headers_len + tail.len()
        };
        if !descriptors.is_empty() {
            let mut block = vec![0u8; TILE_DESC_BLOCK_HEADER];
            block[0..4].copy_from_slice(&(TILE_DESC_BLOCK_HEADER as u32).to_le_bytes());
            block[8..12].copy_from_slice(&(descriptors.len() as u32).to_le_bytes());
            tail.extend_from_slice(&block);
            for d in descriptors {
                let mut words: Vec<u32> = Vec::new();
                let mut mask: &[u8] = &[];
                match d {
                    TileDescriptor::Single {
                        width,
                        height,
                        attribute,
                    } => words.extend([1, 0, *width as u32, *height as u32, *attribute]),
                    TileDescriptor::Double {
                        width,
                        height,
                        outside,
                        inside,
                        rect,
                    } => {
                        words.extend([2, 0, *width as u32, *height as u32, *outside, *inside]);
                        words.extend(rect.iter().map(|&v| v as u32));
                    }
                    TileDescriptor::Mask {
                        width,
                        height,
                        mask: m,
                    } => {
                        words.extend([3, 0, *width as u32, *height as u32]);
                        mask = m;
                    }
                }
                for w in words {
                    tail.extend_from_slice(&w.to_le_bytes());
                }
                tail.extend_from_slice(mask);
            }
        }
        let mut body = plane_heads;
        body.extend_from_slice(&tail);

        let mut header = vec![0u8; HEADER_SIZE];
        let put = |h: &mut Vec<u8>, off: usize, v: u32| {
            h[off..off + 4].copy_from_slice(&v.to_le_bytes());
        };
        put(&mut header, 0x00, HEADER_SIZE as u32);
        put(
            &mut header,
            0x08,
            if compress { 0x1 | FLAG_COMPRESSED } else { 0x1 },
        );
        header[0x10..0x10 + name.len()].copy_from_slice(name.as_bytes());
        put(&mut header, 0x2D0, start.0 as u32);
        put(&mut header, 0x2D4, start.1 as u32);
        put(&mut header, 0x2DC, planes.len() as u32);
        put(
            &mut header,
            0x2E0,
            if planes.is_empty() { 0 } else { base as u32 },
        );
        put(&mut header, 0x2E4, descs_off as u32);
        put(&mut header, 0x2E8, body.len() as u32);
        let mut out = header;
        if compress {
            let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
            enc.write_all(&body).expect("writing to memory");
            out.extend_from_slice(&enc.finish().expect("writing to memory"));
        } else {
            out.extend_from_slice(&body);
        }
        out
    }

    /// Compresses arbitrary bytes as a zlib stream.
    pub fn zlib(data: &[u8]) -> Vec<u8> {
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        enc.write_all(data).expect("writing to memory");
        enc.finish().expect("writing to memory")
    }
}

#[cfg(test)]
mod tests {
    use super::build::{PlaneSpec, level, zlib};
    use super::*;

    fn sample_planes() -> Vec<PlaneSpec> {
        vec![
            PlaneSpec {
                name: "Background".into(),
                flags: 0x4,
                tile_size: (64, 64),
                size: (2, 1),
                move_pct: (50, 50),
                z: -999,
                image_sets: vec!["BACK".into()],
                tiles: vec![3, TILE_EMPTY],
                objects: vec![],
            },
            PlaneSpec {
                name: "Action".into(),
                flags: PLANE_FLAG_MAIN,
                tile_size: (64, 64),
                size: (3, 2),
                move_pct: (100, 100),
                z: 0,
                image_sets: vec!["ACTION".into(), "EXTRA".into()],
                tiles: vec![1, 2, TILE_FILLED, 4, 5, 6],
                objects: vec![[
                    "Rat".into(),
                    "Rat".into(),
                    "LEVEL_RAT".into(),
                    "LEVEL_RAT_WALK".into(),
                ]],
            },
        ]
    }

    fn sample_descs() -> Vec<TileDescriptor> {
        vec![
            TileDescriptor::Single {
                width: 64,
                height: 64,
                attribute: 1,
            },
            TileDescriptor::Double {
                width: 64,
                height: 64,
                outside: 0,
                inside: 2,
                rect: [0, 0, 63, 31],
            },
            TileDescriptor::Mask {
                width: 4,
                height: 5,
                mask: (0..20).collect(),
            },
        ]
    }

    #[test]
    fn tiny_compressed_level() {
        let data = level(
            "Level 3 remix",
            (120, 340),
            &sample_planes(),
            &sample_descs(),
            true,
        );
        let wwd = Wwd::parse(&data).unwrap();
        assert_eq!(wwd.header.name, "Level 3 remix");
        assert_eq!(wwd.header.base_level(), Some(3));
        assert_eq!((wwd.header.start_x, wwd.header.start_y), (120, 340));
        assert_eq!(wwd.planes.len(), 2);
        assert_eq!(wwd.main_plane(), Some(1));
        let action = &wwd.planes[1];
        assert_eq!(action.tiles, vec![1, 2, TILE_FILLED, 4, 5, 6]);
        assert_eq!(action.tile(2, 0), Some(TILE_FILLED));
        assert_eq!(action.tile(3, 0), None);
        // Names are NUL-separated, so only the first one is found.
        assert_eq!(action.image_sets, vec!["ACTION"]);
        assert_eq!(wwd.objects.len(), 1);
        assert_eq!(wwd.objects[0].plane, 1);
        assert_eq!(wwd.objects[0].image_set, "LEVEL_RAT");
        assert_eq!(wwd.objects[0].anim, "LEVEL_RAT_WALK");
        assert_eq!(wwd.tile_descriptors, sample_descs());
        assert_eq!(wwd.unused_input, 0);
    }

    #[test]
    fn uncompressed_level() {
        let data = level("Level 1", (0, 0), &sample_planes(), &sample_descs(), false);
        let wwd = Wwd::parse(&data).unwrap();
        assert_eq!(wwd.planes[0].tiles, vec![3, TILE_EMPTY]);
        assert_eq!(base_level(&data), Some(1));
    }

    #[test]
    fn bytes_after_the_stream_are_ignored() {
        let mut data = level("Level 2", (0, 0), &sample_planes(), &[], true);
        data.extend_from_slice(b"junk");
        let wwd = Wwd::parse(&data).unwrap();
        assert_eq!(wwd.unused_input, 4);
    }

    fn with_body(declared: u32, stream: &[u8]) -> Vec<u8> {
        let mut data = level("Level 1", (0, 0), &[], &[], true);
        data.truncate(HEADER_SIZE);
        data[0x2E8..0x2EC].copy_from_slice(&declared.to_le_bytes());
        data.extend_from_slice(stream);
        data
    }

    #[test]
    fn zip_bomb_like_body_is_refused() {
        // 32 MiB of zeros compresses to a few kilobytes; the header declares
        // only 1000 bytes, so the inflate must stop right after them.
        let stream = zlib(&vec![0u8; 32 << 20]);
        assert!(stream.len() < 64 * 1024);
        let err = Wwd::parse(&with_body(1000, &stream)).unwrap_err();
        assert!(err.to_string().contains("past the declared"), "{err}");
        // Declaring the true size is refused by the caller's limit.
        assert!(Wwd::parse_with_limit(&with_body(32 << 20, &stream), 1 << 20).is_err());
        // And never more than the hard cap, whatever the caller asks.
        assert!(Wwd::parse_with_limit(&with_body(u32::MAX, &stream), usize::MAX).is_err());
    }

    #[test]
    fn short_and_corrupt_bodies() {
        let body = vec![7u8; 5000];
        let stream = zlib(&body);
        // Declared size larger than the stream produces.
        assert!(Wwd::parse(&with_body(6000, &stream)).is_err());
        // Stream cut in half.
        assert!(Wwd::parse(&with_body(5000, &stream[..stream.len() / 2])).is_err());
        // Not zlib at all.
        assert!(Wwd::parse(&with_body(5000, &[0xFF; 100])).is_err());
        // Inflated size 0.
        assert!(Wwd::parse(&with_body(0, &stream)).is_err());
    }

    #[test]
    fn header_checks() {
        let data = level("Level 1", (0, 0), &sample_planes(), &[], true);
        assert!(Wwd::parse(&data[..HEADER_SIZE - 1]).is_err());
        let mut bad = data.clone();
        bad[0..4].copy_from_slice(&(HEADER_SIZE as u32 + 1).to_le_bytes());
        assert!(Wwd::parse(&bad).is_err());
        let mut bad = data.clone();
        bad[0x2DC..0x2E0].copy_from_slice(&(MAX_PLANES + 1).to_le_bytes());
        assert!(Wwd::parse(&bad).is_err());
    }

    #[test]
    fn offsets_and_counts_are_checked() {
        let base = level("Level 1", (0, 0), &sample_planes(), &sample_descs(), false);
        let plane1 = HEADER_SIZE + PLANE_HEADER_SIZE;
        let cases: Vec<(usize, u32)> = vec![
            (plane1, 0x9F),                    // plane header size
            (plane1 + 0x60, u32::MAX),         // negative width
            (plane1 + 0x60, 0x7FFF_FFFF),      // grid too large
            (plane1 + 0x84, u32::MAX - 2),     // grid offset
            (plane1 + 0x80, u32::MAX),         // object count
            (plane1 + 0x8C, u32::MAX - 0x100), // object offset
            (0x2E0, u32::MAX - 0x10),          // planes offset
            (0x2E4, u32::MAX - 0x10),          // descriptor offset
        ];
        for (off, value) in cases {
            let mut data = base.clone();
            data[off..off + 4].copy_from_slice(&value.to_le_bytes());
            assert!(
                Wwd::parse(&data).is_err(),
                "offset {off:#x} = {value:#x} accepted"
            );
        }
    }

    #[test]
    fn descriptor_checks() {
        let bad_mask = vec![TileDescriptor::Mask {
            width: 3,
            height: 5,
            mask: vec![0; 15],
        }];
        let data = level("Level 1", (0, 0), &[], &bad_mask, false);
        assert!(Wwd::parse(&data).is_err());

        let data = level("Level 1", (0, 0), &[], &sample_descs(), false);
        let first = HEADER_SIZE + TILE_DESC_BLOCK_HEADER;
        let mut bad = data.clone();
        bad[first..first + 4].copy_from_slice(&9u32.to_le_bytes());
        assert!(Wwd::parse(&bad).is_err());
        let mut bad = data.clone();
        let count = HEADER_SIZE + 8;
        bad[count..count + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Wwd::parse(&bad).is_err());
        let truncated = &data[..data.len() - 3];
        assert!(Wwd::parse(truncated).is_err());
    }

    #[test]
    fn image_set_names_scan() {
        let buf = b"\x01\x02ACTION\0BACK\0";
        assert_eq!(
            parse_image_set_names(buf, 0, 5).unwrap(),
            (vec!["ACTION".to_string()], 8)
        );
        let buf = b"ONE TWO\0";
        assert_eq!(
            parse_image_set_names(buf, 0, 2).unwrap().0,
            vec!["ONE", "TWO"]
        );
        let buf = b"\x01\x01";
        assert!(parse_image_set_names(buf, 0, 1).is_err());
        // A huge count stops as soon as a pass consumes nothing.
        let buf = b"A\0";
        assert_eq!(
            parse_image_set_names(buf, 0, u32::MAX).unwrap(),
            (vec!["A".to_string()], 1)
        );
    }

    /// Body of `planes` plane headers that all point at one shared region:
    /// a `w` by `h` tile grid at the start of the region, image-set names
    /// after it and, when `objects` is set, one object record after those.
    fn shared_region_body(planes: usize, w: u32, h: u32, objects: bool) -> Vec<u8> {
        let region = HEADER_SIZE + planes * PLANE_HEADER_SIZE;
        let grid = (w * h * 4) as usize;
        let names = region + grid;
        let object = names + 8;
        let mut body = vec![0u8; planes * PLANE_HEADER_SIZE];
        for p in 0..planes {
            let head = &mut body[p * PLANE_HEADER_SIZE..(p + 1) * PLANE_HEADER_SIZE];
            let mut put = |off: usize, v: u32| head[off..off + 4].copy_from_slice(&v.to_le_bytes());
            put(0x00, PLANE_HEADER_SIZE as u32);
            put(0x58, 64);
            put(0x5C, 64);
            put(0x60, w);
            put(0x64, h);
            put(0x7C, 1);
            put(0x80, u32::from(objects));
            put(0x84, region as u32);
            put(0x88, names as u32);
            put(0x8C, if objects { object as u32 } else { 0 });
        }
        body.resize(body.len() + grid, 0x11);
        body.extend_from_slice(b"SHARED\0\0");
        let mut record = vec![0u8; OBJECT_FIXED_SIZE];
        record[4..8].copy_from_slice(&4u32.to_le_bytes());
        body.extend_from_slice(&record);
        body.extend_from_slice(b"Name");
        body
    }

    fn level_with_body(planes: usize, body: &[u8], compress: bool) -> Vec<u8> {
        let mut data = level("Level 1", (0, 0), &[], &[], compress);
        data.truncate(HEADER_SIZE);
        data[0x2DC..0x2E0].copy_from_slice(&(planes as u32).to_le_bytes());
        data[0x2E0..0x2E4].copy_from_slice(&(HEADER_SIZE as u32).to_le_bytes());
        data[0x2E8..0x2EC].copy_from_slice(&(body.len() as u32).to_le_bytes());
        if compress {
            data.extend_from_slice(&zlib(body));
        } else {
            data.extend_from_slice(body);
        }
        data
    }

    #[test]
    fn planes_sharing_one_region_are_refused() {
        // One plane over the region is fine.
        let body = shared_region_body(1, 256, 256, true);
        let wwd = Wwd::parse(&level_with_body(1, &body, true)).unwrap();
        assert_eq!(wwd.planes[0].tiles.len(), 256 * 256);
        assert_eq!(wwd.planes[0].image_sets, vec!["SHARED"]);
        assert_eq!(wwd.objects.len(), 1);

        // Many planes over the same grid would decode it many times over.
        for compress in [true, false] {
            let body = shared_region_body(MAX_PLANES as usize, 256, 256, false);
            let data = level_with_body(MAX_PLANES as usize, &body, compress);
            let err = Wwd::parse(&data).unwrap_err();
            assert!(err.to_string().contains("decode budget"), "{err}");
        }

        // Small grids but one large object region shared by every plane.
        let body = shared_region_body(MAX_PLANES as usize, 1, 1, true);
        let mut big = body.clone();
        let object = HEADER_SIZE + MAX_PLANES as usize * PLANE_HEADER_SIZE + 4 + 8;
        let string_len = 64 * 1024u32;
        big[object - HEADER_SIZE + 4..object - HEADER_SIZE + 8]
            .copy_from_slice(&string_len.to_le_bytes());
        big.truncate(object - HEADER_SIZE + OBJECT_FIXED_SIZE);
        big.resize(big.len() + string_len as usize, b'x');
        let err = Wwd::parse(&level_with_body(MAX_PLANES as usize, &big, true)).unwrap_err();
        assert!(err.to_string().contains("decode budget"), "{err}");
    }

    #[test]
    fn descriptors_share_the_budget() {
        let descs = vec![TileDescriptor::Mask {
            width: 64,
            height: 64,
            mask: vec![1; 64 * 64],
        }];
        let alone = level("Level 1", (0, 0), &[], &descs, false);
        assert_eq!(Wwd::parse(&alone).unwrap().tile_descriptors, descs);

        // One plane whose grid spans the whole buffer, followed by the same
        // descriptor block: the grid uses up the budget the block needs.
        let mut body = shared_region_body(1, 1, 1, false);
        let descs_off = (HEADER_SIZE + body.len()) as u32;
        body.extend_from_slice(&alone[HEADER_SIZE..]);
        let cells = ((HEADER_SIZE + body.len()) / 4) as u32;
        body[0x60..0x64].copy_from_slice(&cells.to_le_bytes());
        body[0x7C..0x80].copy_from_slice(&0u32.to_le_bytes());
        body[0x84..0x88].copy_from_slice(&0u32.to_le_bytes());
        let mut data = level_with_body(1, &body, false);
        data[0x2E4..0x2E8].copy_from_slice(&descs_off.to_le_bytes());
        let err = Wwd::parse(&data).unwrap_err();
        assert!(err.to_string().contains("decode budget"), "{err}");
    }
}
