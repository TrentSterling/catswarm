// Procedural cat instanced sprite shader
// Draws a cat silhouette using SDF (signed distance field) shapes

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct InstanceInput {
    @location(2) offset: vec2<f32>,
    @location(3) size: f32,
    @location(4) color: u32,
    @location(5) frame: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) frame: u32,
};

@group(0) @binding(0)
var<uniform> screen_size: vec2<f32>;

@vertex
fn vs_main(vert: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = vert.position * inst.size + inst.offset;
    // Convert from screen pixels to clip space [-1, 1]
    let clip = vec2<f32>(
        (world_pos.x / screen_size.x) * 2.0 - 1.0,
        1.0 - (world_pos.y / screen_size.y) * 2.0,
    );

    out.clip_position = vec4<f32>(clip, 0.0, 1.0);
    out.uv = vert.uv;
    out.frame = inst.frame;

    // Unpack RGBA from u32
    let r = f32((inst.color >> 24u) & 0xFFu) / 255.0;
    let g = f32((inst.color >> 16u) & 0xFFu) / 255.0;
    let b = f32((inst.color >> 8u) & 0xFFu) / 255.0;
    let a = f32(inst.color & 0xFFu) / 255.0;
    out.color = vec4<f32>(r, g, b, a);

    return out;
}

// SDF circle
fn sd_circle(p: vec2<f32>, center: vec2<f32>, radius: f32) -> f32 {
    return length(p - center) - radius;
}

// SDF ellipse (approximate)
fn sd_ellipse(p: vec2<f32>, center: vec2<f32>, radii: vec2<f32>) -> f32 {
    let q = (p - center) / radii;
    return (length(q) - 1.0) * min(radii.x, radii.y);
}

// SDF triangle (for ears)
fn sd_triangle(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> f32 {
    let e0 = b - a;
    let e1 = c - b;
    let e2 = a - c;
    let v0 = p - a;
    let v1 = p - b;
    let v2 = p - c;

    let pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
    let pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
    let pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);

    let s = sign(e0.x * e2.y - e0.y * e2.x);
    let d = min(min(
        vec2<f32>(dot(pq0, pq0), s * (v0.x * e0.y - v0.y * e0.x)),
        vec2<f32>(dot(pq1, pq1), s * (v1.x * e1.y - v1.y * e1.x))),
        vec2<f32>(dot(pq2, pq2), s * (v2.x * e2.y - v2.y * e2.x))
    );

    return -sqrt(d.x) * sign(d.y);
}

// Smooth minimum for organic shape blending
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

// Procedural cat shape - sitting pose (frame 0 / idle / sleeping / grooming)
fn cat_sitting(uv: vec2<f32>) -> f32 {
    // Work in centered coordinates, UV is [0,1]
    let p = uv - vec2<f32>(0.5, 0.5);

    // Body — fat ellipse, lower portion
    let body = sd_ellipse(p, vec2<f32>(0.0, 0.1), vec2<f32>(0.2, 0.18));

    // Head — circle, upper
    let head = sd_circle(p, vec2<f32>(0.0, -0.12), 0.14);

    // Left ear — triangle
    let ear_l = sd_triangle(p,
        vec2<f32>(-0.12, -0.22),
        vec2<f32>(-0.06, -0.35),
        vec2<f32>(-0.01, -0.22)
    );

    // Right ear — triangle
    let ear_r = sd_triangle(p,
        vec2<f32>(0.01, -0.22),
        vec2<f32>(0.06, -0.35),
        vec2<f32>(0.12, -0.22)
    );

    // Tail — curving to the right
    let tail1 = sd_ellipse(p, vec2<f32>(0.2, 0.15), vec2<f32>(0.12, 0.04));
    let tail2 = sd_circle(p, vec2<f32>(0.28, 0.1), 0.04);

    // Combine with smooth min for organic blending
    var d = smin(body, head, 0.06);
    d = min(d, ear_l);
    d = min(d, ear_r);
    d = smin(d, tail1, 0.04);
    d = smin(d, tail2, 0.03);

    return d;
}

// Procedural cat shape - walking pose (frame 1)
fn cat_walking(uv: vec2<f32>) -> f32 {
    let p = uv - vec2<f32>(0.5, 0.5);

    // Body — horizontal ellipse
    let body = sd_ellipse(p, vec2<f32>(0.0, 0.05), vec2<f32>(0.22, 0.12));

    // Head — circle, forward and up
    let head = sd_circle(p, vec2<f32>(0.15, -0.08), 0.11);

    // Left ear
    let ear_l = sd_triangle(p,
        vec2<f32>(0.08, -0.16),
        vec2<f32>(0.12, -0.28),
        vec2<f32>(0.16, -0.16)
    );

    // Right ear
    let ear_r = sd_triangle(p,
        vec2<f32>(0.14, -0.16),
        vec2<f32>(0.19, -0.28),
        vec2<f32>(0.23, -0.16)
    );

    // Front legs
    let leg_fl = sd_ellipse(p, vec2<f32>(0.1, 0.2), vec2<f32>(0.035, 0.1));
    let leg_fr = sd_ellipse(p, vec2<f32>(0.05, 0.22), vec2<f32>(0.035, 0.1));

    // Back legs
    let leg_bl = sd_ellipse(p, vec2<f32>(-0.12, 0.2), vec2<f32>(0.035, 0.1));
    let leg_br = sd_ellipse(p, vec2<f32>(-0.17, 0.18), vec2<f32>(0.035, 0.1));

    // Tail — raised up
    let tail = sd_ellipse(p, vec2<f32>(-0.25, -0.05), vec2<f32>(0.04, 0.14));

    var d = smin(body, head, 0.05);
    d = min(d, ear_l);
    d = min(d, ear_r);
    d = smin(d, leg_fl, 0.03);
    d = smin(d, leg_fr, 0.03);
    d = smin(d, leg_bl, 0.03);
    d = smin(d, leg_br, 0.03);
    d = smin(d, tail, 0.04);

    return d;
}

// Sleeping pose - curled up ball
fn cat_sleeping(uv: vec2<f32>) -> f32 {
    let p = uv - vec2<f32>(0.5, 0.5);

    // Curled body — big circle
    let body = sd_circle(p, vec2<f32>(0.0, 0.05), 0.2);

    // Head tucked into body
    let head = sd_circle(p, vec2<f32>(0.12, -0.05), 0.1);

    // Tiny ear tips
    let ear_l = sd_triangle(p,
        vec2<f32>(0.07, -0.12),
        vec2<f32>(0.1, -0.2),
        vec2<f32>(0.13, -0.12)
    );
    let ear_r = sd_triangle(p,
        vec2<f32>(0.12, -0.12),
        vec2<f32>(0.16, -0.2),
        vec2<f32>(0.19, -0.1)
    );

    // Tail wrapped around
    let tail = sd_ellipse(p, vec2<f32>(-0.15, 0.18), vec2<f32>(0.12, 0.04));

    var d = smin(body, head, 0.06);
    d = min(d, ear_l);
    d = min(d, ear_r);
    d = smin(d, tail, 0.04);

    return d;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Select cat shape based on frame/state
    // frame encoding: 0=idle/groom, 1=walk/run/chase, 2=sleep
    var d: f32;
    let state = in.frame;

    if state == 2u {
        d = cat_sleeping(in.uv);
    } else if state == 1u {
        d = cat_walking(in.uv);
    } else {
        d = cat_sitting(in.uv);
    }

    // Anti-aliased edge
    let alpha = 1.0 - smoothstep(-0.01, 0.01, d);

    if alpha < 0.01 {
        discard;
    }

    // Slight shading — darken edges for depth
    let shade = mix(0.85, 1.0, smoothstep(0.0, -0.08, d));
    let col = in.color.rgb * shade;

    // Premultiplied alpha output
    return vec4<f32>(col * alpha, in.color.a * alpha);
}
