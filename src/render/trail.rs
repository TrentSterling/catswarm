use bytemuck::{Pod, Zeroable};

/// Per-vertex data for trail line segments.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TrailVertex {
    pub position: [f32; 2],
    pub color: [f32; 4], // premultiplied RGBA
}

/// Number of trail points stored per cat.
const TRAIL_POINTS_PER_CAT: usize = 30;
/// Maximum cats tracked.
const MAX_TRAIL_CATS: usize = 4096;
/// Max trail vertices (2 per line segment, segments = points-1 per cat).
const MAX_TRAIL_VERTICES: usize = MAX_TRAIL_CATS * (TRAIL_POINTS_PER_CAT - 1) * 2;
/// Sample trail position every N frames.
const TRAIL_SAMPLE_INTERVAL: u32 = 3;

/// Per-cat ring buffer of trail positions.
struct CatTrail {
    points: [(f32, f32); TRAIL_POINTS_PER_CAT],
    head: usize,
    len: usize,
    color: [f32; 3],
}

impl CatTrail {
    fn new(r: f32, g: f32, b: f32) -> Self {
        Self {
            points: [(0.0, 0.0); TRAIL_POINTS_PER_CAT],
            head: 0,
            len: 0,
            color: [r, g, b],
        }
    }

    fn push(&mut self, x: f32, y: f32) {
        self.points[self.head] = (x, y);
        self.head = (self.head + 1) % TRAIL_POINTS_PER_CAT;
        if self.len < TRAIL_POINTS_PER_CAT {
            self.len += 1;
        }
    }
}

/// Manages trail data for all cats and produces GPU vertices.
pub struct TrailSystem {
    trails: Vec<CatTrail>,
    frame_counter: u32,
    vertex_buf: Vec<TrailVertex>,
    pub enabled: bool,
}

impl TrailSystem {
    pub fn new() -> Self {
        Self {
            trails: Vec::new(),
            frame_counter: 0,
            vertex_buf: Vec::with_capacity(MAX_TRAIL_VERTICES),
            enabled: false,
        }
    }

    /// Update trail positions from cat positions. Call once per frame.
    /// `positions` is (x, y, r, g, b) for each cat.
    pub fn update(&mut self, positions: &[(f32, f32, f32, f32, f32)]) {
        self.frame_counter += 1;

        // Resize trail storage if needed
        while self.trails.len() < positions.len() {
            self.trails.push(CatTrail::new(0.5, 0.5, 0.5));
        }
        // Truncate if cats despawned
        self.trails.truncate(positions.len());

        // Only sample every N frames
        if self.frame_counter % TRAIL_SAMPLE_INTERVAL != 0 {
            return;
        }

        for (i, &(x, y, r, g, b)) in positions.iter().enumerate() {
            self.trails[i].color = [r, g, b];
            self.trails[i].push(x, y);
        }
    }

    /// Build the vertex buffer for rendering. Returns number of vertices.
    pub fn build_vertices(&mut self) -> &[TrailVertex] {
        self.vertex_buf.clear();

        for trail in &self.trails {
            if trail.len < 2 {
                continue;
            }

            // Iterate points oldest to newest
            for i in 0..(trail.len - 1) {
                let idx_a = (trail.head + TRAIL_POINTS_PER_CAT - trail.len + i)
                    % TRAIL_POINTS_PER_CAT;
                let idx_b = (idx_a + 1) % TRAIL_POINTS_PER_CAT;

                let (ax, ay) = trail.points[idx_a];
                let (bx, by) = trail.points[idx_b];

                // Alpha fades from 0 (oldest) to 0.5 (newest)
                let t = i as f32 / (trail.len - 1) as f32;
                let alpha = t * 0.5;

                // Premultiplied alpha
                let r = trail.color[0] * alpha;
                let g = trail.color[1] * alpha;
                let b = trail.color[2] * alpha;

                self.vertex_buf.push(TrailVertex {
                    position: [ax, ay],
                    color: [r, g, b, alpha],
                });
                self.vertex_buf.push(TrailVertex {
                    position: [bx, by],
                    color: [r, g, b, alpha],
                });
            }
        }

        &self.vertex_buf
    }
}

/// GPU pipeline for rendering trails as lines.
pub struct TrailPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
    pub screen_bind_group: wgpu::BindGroup,
    pub num_vertices: u32,
}

impl TrailPipeline {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        screen_uniform_buffer: &wgpu::Buffer,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("trail_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/trail.wgsl").into()),
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("trail_screen_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("trail_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TrailVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 8,
                    shader_location: 1,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("trail_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("trail_vertex_buffer"),
            size: (MAX_TRAIL_VERTICES * std::mem::size_of::<TrailVertex>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let screen_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("trail_screen_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_uniform_buffer.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            vertex_buffer,
            screen_bind_group,
            num_vertices: 0,
        }
    }

    pub fn update_vertices(&mut self, queue: &wgpu::Queue, vertices: &[TrailVertex]) {
        let count = vertices.len().min(MAX_TRAIL_VERTICES);
        self.num_vertices = count as u32;
        if count > 0 {
            queue.write_buffer(
                &self.vertex_buffer,
                0,
                bytemuck::cast_slice(&vertices[..count]),
            );
        }
    }
}
