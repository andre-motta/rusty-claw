// A movie picture drawn into the viewport the presenter sets (the picture's
// rectangle at its display aspect; the rest of the target is cleared to
// black), with bilinear filtering. RGBA pictures are one texture; I420
// pictures are three one-byte planes (Y, then U and V at half size) turned
// into colour here. Mirrors movie_pixel() in cpu.rs.

struct MovieU {
    srgb: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
    // Subtracted from Y, U and V (w unused).
    offset: vec4<f32>,
    // Red, green and blue rows applied to the offset Y, U, V (w unused).
    red: vec4<f32>,
    green: vec4<f32>,
    blue: vec4<f32>,
};

@group(0) @binding(0) var<uniform> mu: MovieU;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var bilinear: sampler;
@group(0) @binding(3) var plane_y: texture_2d<f32>;
@group(0) @binding(4) var plane_u: texture_2d<f32>;
@group(0) @binding(5) var plane_v: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_movie(@builtin(vertex_index) vi: u32) -> VsOut {
    let p = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: VsOut;
    out.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn finish(rgb: vec3<f32>) -> vec4<f32> {
    var c = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    if mu.srgb != 0u {
        c = to_linear(c);
    }
    return vec4<f32>(c, 1.0);
}

@fragment
fn fs_rgba(in: VsOut) -> @location(0) vec4<f32> {
    return finish(textureSample(picture, bilinear, in.uv).rgb);
}

@fragment
fn fs_i420(in: VsOut) -> @location(0) vec4<f32> {
    let yuv = vec3<f32>(
        textureSample(plane_y, bilinear, in.uv).r,
        textureSample(plane_u, bilinear, in.uv).r,
        textureSample(plane_v, bilinear, in.uv).r,
    ) - mu.offset.xyz;
    return finish(vec3<f32>(dot(mu.red.xyz, yuv), dot(mu.green.xyz, yuv), dot(mu.blue.xyz, yuv)));
}
