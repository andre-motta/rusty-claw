// Final pass: palette lookup (or replacement colour) and scaling to the
// output. Mirrors present() in cpu.rs.

struct PresentU {
    rect: vec4<f32>,
    ratio: f32,
    brightness: f32,
    srgb: u32,
    pad: u32,
    target_size: vec2<i32>,
    pad2: vec2<i32>,
};

@group(0) @binding(0) var<uniform> pu: PresentU;
@group(0) @binding(1) var index_tex: texture_2d<u32>;
@group(0) @binding(2) var hd_tex: texture_2d<f32>;
@group(0) @binding(3) var palette_tex: texture_2d<f32>;

@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn texel(x: i32, y: i32) -> vec3<f32> {
    let p = vec2<i32>(clamp(x, 0, pu.target_size.x - 1), clamp(y, 0, pu.target_size.y - 1));
    let h = textureLoad(hd_tex, p, 0);
    if h.a > 0.0 {
        return h.rgb * pu.brightness;
    }
    let i = textureLoad(index_tex, p, 0).r;
    return textureLoad(palette_tex, vec2<i32>(i32(i), 0), 0).rgb;
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_present(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let r = pu.rect;
    if pos.x < r.x || pos.y < r.y || pos.x >= r.x + r.z || pos.y >= r.y + r.w {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let u = (pos.x - r.x) / pu.ratio - 0.5;
    let v = (pos.y - r.y) / pu.ratio - 0.5;
    let x0 = floor(u);
    let y0 = floor(v);
    let fx = u - x0;
    let fy = v - y0;
    let ix = i32(x0);
    let iy = i32(y0);
    let c00 = texel(ix, iy);
    let c10 = texel(ix + 1, iy);
    let c01 = texel(ix, iy + 1);
    let c11 = texel(ix + 1, iy + 1);
    var c = mix(mix(c00, c10, fx), mix(c01, c11, fx), fy);
    if pu.srgb != 0u {
        c = to_linear(c);
    }
    return vec4<f32>(c, 1.0);
}
