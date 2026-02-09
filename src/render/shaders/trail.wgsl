// Trail line shader — renders line segments with per-vertex color + alpha fade.
// Premultiplied alpha blending (same as cats).

struct ScreenUniform {
    size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> screen: ScreenUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Convert pixel coords to NDC: x in [0, screen_w] -> [-1, 1], y in [0, screen_h] -> [1, -1]
    let ndc_x = (in.position.x / screen.size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (in.position.y / screen.size.y) * 2.0;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color; // already premultiplied
}
