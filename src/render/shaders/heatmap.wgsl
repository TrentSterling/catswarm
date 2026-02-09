// Heatmap overlay shader — fullscreen quad, samples R8 texture,
// outputs warm color ramp with low opacity.

@group(0) @binding(0)
var heatmap_tex: texture_2d<f32>;
@group(0) @binding(1)
var heatmap_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Fullscreen triangle trick — 3 vertices, no vertex buffer needed.
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Generate a fullscreen triangle:
    // v0 = (-1, -1), v1 = (3, -1), v2 = (-1, 3)
    let x = f32(i32(vertex_index) / 2) * 4.0 - 1.0;
    let y = f32(i32(vertex_index) % 2) * 4.0 - 1.0;
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    // UV: map clip space to [0, 1]
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let heat = textureSample(heatmap_tex, heatmap_sampler, in.uv).r;

    if (heat < 0.01) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Color ramp: transparent -> orange -> red
    let r = smoothstep(0.0, 0.5, heat);
    let g = smoothstep(0.0, 0.3, heat) * (1.0 - smoothstep(0.5, 1.0, heat)) * 0.6;
    let b = 0.0;

    // Low opacity: 0.05 to 0.15
    let alpha = heat * 0.15;

    // Premultiplied alpha output
    return vec4<f32>(r * alpha, g * alpha, b * alpha, alpha);
}
