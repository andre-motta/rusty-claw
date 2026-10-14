// Composition of one frame into palette indices plus a replacement layer.
// Every function mirrors the CPU reference in cpu.rs exactly.

struct FrameU {
    target_size: vec2<f32>,
    pad: vec2<f32>,
};

struct PlaneP {
    origin: vec2<i32>,
    size: vec2<i32>,
    tile: vec2<i32>,
    grid: vec2<i32>,
    slots: u32,
    flags: u32,
    fill: u32,
    scale: u32,
};

struct SpriteP {
    pos: vec2<i32>,
    size: vec2<u32>,
    image: u32,
    flags: u32,
    effect: u32,
    color: u32,
    table: u32,
    scale: u32,
    pad: vec2<u32>,
};

struct ImageR {
    width: u32,
    height: u32,
    flags: u32,
    index_offset: u32,
    mask_offset: u32,
    hd_offset: u32,
    hd_scale: u32,
    pad: u32,
};

@group(0) @binding(0) var<uniform> frame: FrameU;
@group(0) @binding(1) var<storage, read> image_bytes: array<u32>;
@group(0) @binding(2) var<storage, read> hd_texels: array<u32>;
@group(0) @binding(3) var<storage, read> images: array<ImageR>;
@group(0) @binding(4) var<storage, read> slots: array<i32>;
@group(0) @binding(5) var<storage, read> tables: array<u32>;
@group(0) @binding(6) var<storage, read> planes: array<PlaneP>;
@group(0) @binding(7) var<storage, read> sprites: array<SpriteP>;
@group(0) @binding(8) var scratch: texture_2d<u32>;

const IMAGE_KEYED: u32 = 1u;
const IMAGE_MASKED: u32 = 2u;
const PLANE_X_WRAP: u32 = 4u;
const PLANE_Y_WRAP: u32 = 8u;
const SPRITE_MIRROR: u32 = 1u;
const SPRITE_INVERT: u32 = 2u;
const SLOT_FILL: i32 = -2;

struct Out {
    @location(0) index: u32,
    @location(1) hd: vec4<f32>,
};

fn image_byte(o: u32) -> u32 {
    return (image_bytes[o >> 2u] >> ((o & 3u) * 8u)) & 0xffu;
}

fn table_byte(o: u32) -> u32 {
    return (tables[o >> 2u] >> ((o & 3u) * 8u)) & 0xffu;
}

fn covered(img: ImageR, x: u32, y: u32, index: u32) -> bool {
    if (img.flags & IMAGE_MASKED) != 0u && image_byte(img.mask_offset + y * img.width + x) == 0u {
        return false;
    }
    if (img.flags & IMAGE_KEYED) != 0u && index == 0u {
        return false;
    }
    return true;
}

fn unpack_hd(t: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(t & 0xffu) / 255.0,
        f32((t >> 8u) & 0xffu) / 255.0,
        f32((t >> 16u) & 0xffu) / 255.0,
        1.0,
    );
}

fn floor_div(a: i32, b: i32) -> i32 {
    let q = a / b;
    if (a % b) != 0 && ((a < 0) != (b < 0)) {
        return q - 1;
    }
    return q;
}

fn rem_euclid(a: i32, b: i32) -> i32 {
    let r = a % b;
    if r < 0 {
        return r + b;
    }
    return r;
}

fn hd_coord(local: i32, k: i32, len: u32, scale: u32, flip: bool) -> u32 {
    let s = i32(scale);
    var v: i32;
    if flip {
        v = (2 * k * i32(len) - 2 * local - 1) * s / (2 * k);
    } else {
        v = (2 * local + 1) * s / (2 * k);
    }
    return u32(clamp(v, 0, i32(len) * s - 1));
}

struct PlaneVs {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) plane: u32,
};

@vertex
fn vs_plane(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> PlaneVs {
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: PlaneVs;
    out.pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.plane = ii;
    return out;
}

@fragment
fn fs_plane(in: PlaneVs) -> Out {
    let p = planes[in.plane];
    let px = vec2<i32>(floor(in.pos.xy));
    let k = i32(max(p.scale, 1u));
    let q = px + p.origin;
    var x = floor_div(q.x, k);
    var y = floor_div(q.y, k);
    let sub = q - vec2<i32>(x, y) * k;
    if (p.flags & PLANE_X_WRAP) != 0u && p.size.x > 0 {
        x = rem_euclid(x, p.size.x);
    }
    if (p.flags & PLANE_Y_WRAP) != 0u && p.size.y > 0 {
        y = rem_euclid(y, p.size.y);
    }
    if x < 0 || y < 0 || x >= p.size.x || y >= p.size.y {
        discard;
    }
    let col = x / p.tile.x;
    let row = y / p.tile.y;
    let tx = u32(x - col * p.tile.x);
    let ty = u32(y - row * p.tile.y);
    let slot = slots[p.slots + u32(row * p.grid.x + col)];
    var out: Out;
    out.hd = vec4<f32>(0.0);
    if slot == SLOT_FILL {
        out.index = p.fill;
        return out;
    }
    if slot < 0 {
        discard;
    }
    let img = images[u32(slot)];
    let index = image_byte(img.index_offset + ty * img.width + tx);
    if !covered(img, tx, ty, index) {
        discard;
    }
    out.index = index;
    if img.hd_scale > 0u {
        let s = i32(img.hd_scale);
        let hx = i32(tx) * s + (2 * sub.x + 1) * s / (2 * k);
        let hy = i32(ty) * s + (2 * sub.y + 1) * s / (2 * k);
        out.hd = unpack_hd(hd_texels[img.hd_offset + u32(hy) * img.width * img.hd_scale + u32(hx)]);
    }
    return out;
}

struct SpriteVs {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) sprite: u32,
};

@vertex
fn vs_sprite(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> SpriteVs {
    let s = sprites[ii];
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let size = vec2<f32>(s.size * s.scale);
    let p = vec2<f32>(s.pos) + corners[vi] * size;
    var out: SpriteVs;
    out.pos = vec4<f32>(
        p.x / frame.target_size.x * 2.0 - 1.0,
        1.0 - p.y / frame.target_size.y * 2.0,
        0.0,
        1.0,
    );
    out.sprite = ii;
    return out;
}

@fragment
fn fs_sprite(in: SpriteVs) -> Out {
    let s = sprites[in.sprite];
    let px = vec2<i32>(floor(in.pos.xy));
    let k = i32(max(s.scale, 1u));
    let l = px - s.pos;
    if l.x < 0 || l.y < 0 {
        discard;
    }
    let c = u32(l.x / k);
    let r = u32(l.y / k);
    if c >= s.size.x || r >= s.size.y {
        discard;
    }
    let mirror = (s.flags & SPRITE_MIRROR) != 0u;
    let invert = (s.flags & SPRITE_INVERT) != 0u;
    let sx = select(c, s.size.x - 1u - c, mirror);
    let sy = select(r, s.size.y - 1u - r, invert);
    let img = images[s.image];
    let src = image_byte(img.index_offset + sy * img.width + sx);
    if !covered(img, sx, sy, src) {
        discard;
    }
    var out: Out;
    out.hd = vec4<f32>(0.0);
    switch s.effect {
        case 1u: {
            out.index = src;
            if img.hd_scale > 0u {
                let hx = hd_coord(l.x, k, s.size.x, img.hd_scale, mirror);
                let hy = hd_coord(l.y, k, s.size.y, img.hd_scale, invert);
                out.hd = unpack_hd(hd_texels[img.hd_offset + hy * img.width * img.hd_scale + hx]);
            }
        }
        case 2u: {
            let d = textureLoad(scratch, px, 0).r;
            out.index = table_byte(s.table + d * 256u + src);
        }
        case 3u: {
            let d = textureLoad(scratch, px, 0).r;
            out.index = table_byte(s.table + d * 256u + (s.color & 0xffu));
        }
        case 4u: {
            out.index = table_byte(s.table + src * 256u + (s.color & 0xffu));
        }
        case 5u: {
            out.index = s.color & 0xffu;
        }
        case 6u: {
            let d = textureLoad(scratch, px, 0).r;
            let rs = i32(table_byte(s.table + 256u + src));
            let rd = i32(table_byte(s.table + 256u + d));
            let at = rd + (rs - rd) * i32(s.color & 0xffu) / 255;
            out.index = table_byte(s.table + u32(at));
        }
        default: {
            discard;
        }
    }
    return out;
}
