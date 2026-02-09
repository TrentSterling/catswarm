pub mod heatmap_pipeline;
pub mod instance;
pub mod pipeline;
pub mod trail;

use std::sync::Arc;
use winit::window::Window;

use self::heatmap_pipeline::HeatmapPipeline;
use self::instance::CatInstance;
use self::pipeline::CatPipeline;
use self::trail::{TrailPipeline, TrailVertex};

/// Core GPU state — device, queue, surface, pipeline.
pub struct GpuState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub cat_pipeline: CatPipeline,
    pub trail_pipeline: TrailPipeline,
    pub heatmap_pipeline: HeatmapPipeline,
}

/// Intermediate frame state returned by `begin_frame`.
pub struct FrameContext {
    pub output: wgpu::SurfaceTexture,
    pub view: wgpu::TextureView,
    pub encoder: wgpu::CommandEncoder,
}

impl GpuState {
    /// Initialize wgpu and the cat rendering pipeline.
    pub fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        // DX12 only — Vulkan WSI on Windows doesn't support transparent composition.
        // Use DirectComposition presentation for per-pixel alpha transparency.
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            backend_options: wgpu::BackendOptions {
                dx12: wgpu::Dx12BackendOptions {
                    presentation_system: wgpu_types::Dx12SwapchainKind::DxgiFromVisual,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        });

        let surface = instance
            .create_surface(window)
            .expect("failed to create wgpu surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("no suitable GPU adapter found");

        log::info!(
            "GPU adapter: {:?} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("pettoy_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
        ))
        .expect("failed to create wgpu device");

        let surface_caps = surface.get_capabilities(&adapter);

        let format = surface_caps
            .formats
            .iter()
            .find(|f| **f == wgpu::TextureFormat::Bgra8UnormSrgb)
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        log::info!("Available alpha modes: {:?}", surface_caps.alpha_modes);

        let alpha_mode = if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else {
            wgpu::CompositeAlphaMode::Auto
        };

        // Prefer Mailbox (no CPU-blocking on missed deadlines) with Fifo fallback.
        let present_mode = if surface_caps
            .present_modes
            .contains(&wgpu::PresentMode::Mailbox)
        {
            log::info!("Using PresentMode::Mailbox");
            wgpu::PresentMode::Mailbox
        } else {
            log::info!("Mailbox unavailable, falling back to PresentMode::Fifo");
            wgpu::PresentMode::Fifo
        };

        log::info!(
            "Surface: format={:?}, alpha_mode={:?}",
            format,
            alpha_mode,
        );

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create the cat rendering pipeline
        let cat_pipeline = CatPipeline::new(&device, format);

        // Create the trail rendering pipeline (shares screen_size uniform)
        let trail_pipeline = TrailPipeline::new(
            &device,
            format,
            &cat_pipeline.screen_uniform_buffer,
        );

        // Create the heatmap rendering pipeline
        let heatmap_pipeline = HeatmapPipeline::new(&device, format);

        // Set initial screen size uniform
        cat_pipeline.update_screen_size(
            &queue,
            surface_config.width as f32,
            surface_config.height as f32,
        );

        Self {
            device,
            queue,
            surface,
            surface_config,
            cat_pipeline,
            trail_pipeline,
            heatmap_pipeline,
        }
    }

    /// Resize the surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.cat_pipeline
            .update_screen_size(&self.queue, width as f32, height as f32);
    }

    /// Upload instance data for this frame.
    pub fn update_instances(&mut self, instances: &[CatInstance]) {
        self.cat_pipeline
            .update_instances(&self.queue, instances);
    }

    /// Upload trail vertex data for this frame.
    pub fn update_trails(&mut self, vertices: &[TrailVertex]) {
        self.trail_pipeline.update_vertices(&self.queue, vertices);
    }

    /// Upload heatmap texture data for this frame.
    pub fn update_heatmap(&mut self, data: &[u8]) {
        self.heatmap_pipeline.update_texture(&self.queue, data);
    }

    /// Change the present mode at runtime.
    pub fn set_present_mode(&mut self, mode: wgpu::PresentMode) {
        self.surface_config.present_mode = mode;
        self.surface
            .configure(&self.device, &self.surface_config);
        log::info!("Present mode changed to {:?}", mode);
    }

    /// Acquire the next surface texture and create a command encoder.
    /// Returns None if the surface is lost/outdated (caller should skip this frame).
    pub fn begin_frame(&self) -> Option<FrameContext> {
        let output = match self.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface
                    .configure(&self.device, &self.surface_config);
                return None;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("GPU out of memory");
                return None;
            }
            Err(e) => {
                log::warn!("Surface error: {e:?}");
                return None;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });

        Some(FrameContext {
            output,
            view,
            encoder,
        })
    }

    /// Draw heatmap overlay (before trails and cats so it's behind everything).
    pub fn draw_heatmap(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("heatmap_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.heatmap_pipeline.pipeline);
        render_pass.set_bind_group(0, &self.heatmap_pipeline.bind_group, &[]);
        render_pass.draw(0..3, 0..1); // fullscreen triangle
    }

    /// Draw cat trails (between clear and cats, so trails are behind cats).
    pub fn draw_trails(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let p = &self.trail_pipeline;
        if p.num_vertices == 0 {
            return;
        }

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("trail_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&p.pipeline);
        render_pass.set_bind_group(0, &p.screen_bind_group, &[]);
        render_pass.set_vertex_buffer(0, p.vertex_buffer.slice(..));
        render_pass.draw(0..p.num_vertices, 0..1);
    }

    /// Run the cat render pass (clear to transparent + draw instanced cats).
    pub fn draw_cats(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("cat_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        let p = &self.cat_pipeline;
        if p.num_instances > 0 {
            render_pass.set_pipeline(&p.pipeline);
            render_pass.set_bind_group(0, &p.screen_bind_group, &[]);
            render_pass.set_vertex_buffer(0, p.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, p.instance_buffer.slice(..));
            render_pass.set_index_buffer(p.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..6, 0, 0..p.num_instances);
        }
    }

    /// Create an egui render pass that preserves existing content (LoadOp::Load).
    /// Returns a 'static render pass suitable for egui_wgpu::Renderer::render().
    pub fn begin_egui_pass<'a>(
        encoder: &'a mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) -> wgpu::RenderPass<'static> {
        let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("egui_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.forget_lifetime()
    }

    /// Submit the command encoder and present.
    pub fn finish_frame(
        &self,
        encoder: wgpu::CommandEncoder,
        output: wgpu::SurfaceTexture,
        extra_cmd_bufs: Vec<wgpu::CommandBuffer>,
    ) {
        self.queue.submit(
            extra_cmd_bufs
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );
        output.present();
    }
}
