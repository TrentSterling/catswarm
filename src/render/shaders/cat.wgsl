// Procedural cat instanced sprite shader
// Draws a cat silhouette using SDF (signed distance field) shapes

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct InstanceInput {
    @location(2) offset: vec2<f32>,
    @location(3) size: vec2<f32>,
    @location(4) color: u32,
    @location(5) frame: u32,
    @location(6) rotation: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) frame: u32,
    @location(3) rotation: f32,
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
    out.rotation = inst.rotation;

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

// Procedural cat shape - walking pose B (frame 7, legs swapped for animation)
fn cat_walking_b(uv: vec2<f32>) -> f32 {
    let p = uv - vec2<f32>(0.5, 0.5);

    // Body — same as walking A
    let body = sd_ellipse(p, vec2<f32>(0.0, 0.05), vec2<f32>(0.22, 0.12));

    // Head — same
    let head = sd_circle(p, vec2<f32>(0.15, -0.08), 0.11);

    // Ears — same
    let ear_l = sd_triangle(p,
        vec2<f32>(0.08, -0.16),
        vec2<f32>(0.12, -0.28),
        vec2<f32>(0.16, -0.16)
    );
    let ear_r = sd_triangle(p,
        vec2<f32>(0.14, -0.16),
        vec2<f32>(0.19, -0.28),
        vec2<f32>(0.23, -0.16)
    );

    // Front legs — swapped stride positions
    let leg_fl = sd_ellipse(p, vec2<f32>(0.05, 0.2), vec2<f32>(0.035, 0.1));
    let leg_fr = sd_ellipse(p, vec2<f32>(0.1, 0.22), vec2<f32>(0.035, 0.1));

    // Back legs — swapped stride positions
    let leg_bl = sd_ellipse(p, vec2<f32>(-0.17, 0.2), vec2<f32>(0.035, 0.1));
    let leg_br = sd_ellipse(p, vec2<f32>(-0.12, 0.18), vec2<f32>(0.035, 0.1));

    // Tail — same
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

// SDF heart shape (for love/chase particles)
fn sd_heart(uv: vec2<f32>) -> f32 {
    let p = (uv - vec2<f32>(0.5, 0.5)) * 2.5;
    let q = vec2<f32>(abs(p.x), -p.y + 0.5);
    let a = q - vec2<f32>(0.25, 0.75);
    let b = q - vec2<f32>(0.0, 0.25);
    let r = q - clamp(vec2<f32>(0.0, 0.75), vec2<f32>(0.0, 0.25), q);
    let d = min(dot(a, a), dot(b, b));
    let s = max(
        (q.x + q.y - 1.0) * 0.5,
        -length(q - vec2<f32>(0.25, 0.75)) + 0.5
    );
    // Simple approximation
    let heart_d = length(q - vec2<f32>(0.0, 0.4)) - 0.5;
    let top_l = sd_circle(uv, vec2<f32>(0.38, 0.38), 0.17);
    let top_r = sd_circle(uv, vec2<f32>(0.62, 0.38), 0.17);
    let bottom = sd_triangle(uv,
        vec2<f32>(0.22, 0.48),
        vec2<f32>(0.5, 0.82),
        vec2<f32>(0.78, 0.48)
    );
    return min(min(top_l, top_r), bottom);
}

// SDF 4-point star (for sparkle/excitement particles)
fn sd_star(uv: vec2<f32>) -> f32 {
    let p = (uv - vec2<f32>(0.5, 0.5)) * 3.0;
    // Diamond rotated 45 degrees combined with regular diamond
    let d1 = (abs(p.x) + abs(p.y)) - 0.6;           // diamond
    let d2 = (abs(p.x * 0.707 - p.y * 0.707) +
              abs(p.x * 0.707 + p.y * 0.707)) - 0.4; // rotated diamond
    return min(d1, d2) / 3.0;
}

// SDF Z-letter shape (for sleeping particles)
fn sd_z_letter(uv: vec2<f32>) -> f32 {
    let p = (uv - vec2<f32>(0.5, 0.5)) * 3.0;
    // Three horizontal/diagonal strokes forming Z
    let top = max(abs(p.x) - 0.5, abs(p.y - 0.5) - 0.1);       // top bar
    let bottom = max(abs(p.x) - 0.5, abs(p.y + 0.5) - 0.1);    // bottom bar
    let diag_d = abs(p.x * 0.707 + p.y * 0.707);                // diagonal
    let diag = max(diag_d - 0.12, max(abs(p.x) - 0.55, abs(p.y) - 0.55));
    return min(min(top, bottom), diag) / 3.0;
}

// Rotate a 2D point around (0.5, 0.5)
fn rotate_uv(uv: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    let p = uv - vec2<f32>(0.5, 0.5);
    return vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c) + vec2<f32>(0.5, 0.5);
}

// Eye glow: two small bright circles on the cat's head.
// Returns glow intensity (0.0 = no glow, 1.0 = full glow).
fn eye_glow_sitting(uv: vec2<f32>) -> f32 {
    let p = uv - vec2<f32>(0.5, 0.5);
    let eye_l = sd_circle(p, vec2<f32>(-0.055, -0.14), 0.022);
    let eye_r = sd_circle(p, vec2<f32>(0.055, -0.14), 0.022);
    let d = min(eye_l, eye_r);
    return 1.0 - smoothstep(-0.005, 0.005, d);
}

fn eye_glow_walking(uv: vec2<f32>) -> f32 {
    let p = uv - vec2<f32>(0.5, 0.5);
    let eye_l = sd_circle(p, vec2<f32>(0.11, -0.1), 0.02);
    let eye_r = sd_circle(p, vec2<f32>(0.17, -0.1), 0.02);
    let d = min(eye_l, eye_r);
    return 1.0 - smoothstep(-0.005, 0.005, d);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Apply rotation to UVs (for spawn somersault etc.)
    var uv = in.uv;
    if abs(in.rotation) > 0.001 {
        uv = rotate_uv(uv, in.rotation);
    }

    // Frames 8+ = night glow variant (subtract 8 to get base frame)
    let has_glow = in.frame >= 8u;
    let state = select(in.frame, in.frame - 8u, has_glow);

    // Select shape based on frame
    // 0=sitting, 1=walking, 2=sleeping, 3=circle, 4=heart, 5=star, 6=Z-letter, 7=walking-B
    var d: f32;

    if state == 7u {
        d = cat_walking_b(uv);
    } else if state == 6u {
        d = sd_z_letter(uv);
    } else if state == 5u {
        d = sd_star(uv);
    } else if state == 4u {
        d = sd_heart(uv);
    } else if state == 3u {
        d = sd_circle(uv, vec2<f32>(0.5, 0.5), 0.25);
    } else if state == 2u {
        d = cat_sleeping(uv);
    } else if state == 1u {
        d = cat_walking(uv);
    } else {
        d = cat_sitting(uv);
    }

    // Anti-aliased edge
    let alpha = 1.0 - smoothstep(-0.01, 0.01, d);

    if alpha < 0.01 {
        discard;
    }

    // Slight shading — darken edges for depth
    let shade = mix(0.85, 1.0, smoothstep(0.0, -0.08, d));
    var col = in.color.rgb * shade;

    // Eye glow at night — bright yellow-green dots on head
    if has_glow && (state == 0u || state == 1u || state == 7u) {
        var glow: f32;
        if state == 0u {
            glow = eye_glow_sitting(uv);
        } else {
            glow = eye_glow_walking(uv);
        }
        if glow > 0.01 {
            // Bright yellow-green eyes, additive over cat color
            let eye_color = vec3<f32>(0.6, 1.0, 0.2);
            col = mix(col, eye_color, glow);
        }
    }

    // Premultiplied alpha output
    return vec4<f32>(col * alpha, in.color.a * alpha);
}
