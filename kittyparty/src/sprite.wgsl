// One textured, rotated quad per instance. Every sprite (kitty, hearts, paper
// confetti, cat cards) lives in one texture array, so the whole scene is a
// single instanced draw call.

struct Screen {
    size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> screen: Screen;
@group(0) @binding(1) var tex: texture_2d_array<f32>;
@group(0) @binding(2) var samp: sampler;

struct InstanceIn {
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) rot: f32,
    @location(3) layer: u32,
    @location(4) tint: vec4<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) layer: u32,
    @location(2) tint: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: InstanceIn) -> VOut {
    var corners = array<vec2<f32>, 6>(
        vec2(-0.5, -0.5), vec2(0.5, -0.5), vec2(0.5, 0.5),
        vec2(-0.5, -0.5), vec2(0.5, 0.5), vec2(-0.5, 0.5),
    );
    let corner = corners[vi];
    // A negative width mirrors the quad, which flips the sprite for free.
    let local = corner * inst.size;
    let c = cos(inst.rot);
    let s = sin(inst.rot);
    let world = inst.pos + vec2(local.x * c - local.y * s, local.x * s + local.y * c);
    let ndc = world / screen.size * 2.0 - 1.0;

    var out: VOut;
    out.clip = vec4(ndc.x, -ndc.y, 0.0, 1.0);
    out.uv = corner + 0.5;
    out.layer = inst.layer;
    out.tint = inst.tint;
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    // Layers hold linear, premultiplied colour already (see upload_layer).
    let c = textureSample(tex, samp, in.uv, in.layer);
    return vec4(c.rgb * in.tint.rgb, c.a) * in.tint.a;
}
