pub mod ring;
pub mod timer;

use winit::window::Window;

use self::ring::RingBuffer;
use self::timer::{SystemPhase, SystemTimers};
use glam::Vec2;

use crate::ecs::components::{BehaviorState, Personality};
use crate::mode::AppMode;
use crate::render::GpuState;

/// Info about the cat currently under the mouse cursor.
pub struct HoveredCatInfo {
    pub screen_pos: Vec2,
    pub name: String,
    pub state: BehaviorState,
    pub personality: Personality,
}

/// Number of frame times to keep in the histogram.
const FRAME_HISTORY_LEN: usize = 300;

/// Debug overlay powered by egui.
pub struct DebugOverlay {
    pub egui_ctx: egui::Context,
    pub egui_state: egui_winit::State,
    pub egui_renderer: egui_wgpu::Renderer,

    pub visible: bool,
    f12_was_down: bool,

    /// Rolling window of frame times (seconds).
    pub frame_times: RingBuffer<f64>,

    /// Computed stats.
    pub fps: f64,
    pub frame_time_avg: f64,
    pub frame_time_min: f64,
    pub frame_time_max: f64,

    /// Per-system timers (updated externally via systems::tick).
    pub system_timers: SystemTimers,

    /// UI controls.
    pub paused: bool,
    pub target_cat_count: usize,
    pub cat_count_changed: bool,
    pub present_mode_index: usize,
    pub present_mode_changed: bool,

    /// Total entity count (updated each frame).
    pub entity_count: usize,
    pub tick_count: u64,

    /// Mode system info (updated from app each frame).
    pub current_mode: AppMode,
    pub idle_seconds: f64,
    pub edge_affinity: f32,
    pub energy_scale: f32,
    pub mode_changed: bool,
    pub selected_mode_index: usize,

    /// Visual toggle controls.
    pub show_trails: bool,
    pub show_heatmap: bool,

    /// Hovered cat tooltip info (updated by app each frame).
    pub hovered_cat: Option<HoveredCatInfo>,

    // Stats accumulator (replaces FrameStats).
    frame_count: u64,
    log_timer: f64,
    log_frame_count: u32,
    log_frame_sum: f64,
    log_frame_min: f64,
    log_frame_max: f64,
}

const PRESENT_MODES: [wgpu::PresentMode; 3] = [
    wgpu::PresentMode::Mailbox,
    wgpu::PresentMode::Fifo,
    wgpu::PresentMode::Immediate,
];

const PRESENT_MODE_LABELS: [&str; 3] = ["Mailbox", "Fifo (vsync)", "Immediate"];

impl DebugOverlay {
    pub fn new(window: &Window, gpu: &GpuState) -> Self {
        let egui_ctx = egui::Context::default();

        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            Some(gpu.device.limits().max_texture_dimension_2d as usize),
        );

        let egui_renderer = egui_wgpu::Renderer::new(
            &gpu.device,
            gpu.surface_config.format,
            egui_wgpu::RendererOptions {
                depth_stencil_format: None,
                msaa_samples: 1,
                dithering: true,
                predictable_texture_filtering: false,
            },
        );

        Self {
            egui_ctx,
            egui_state,
            egui_renderer,
            visible: false,
            f12_was_down: false,
            frame_times: RingBuffer::new(FRAME_HISTORY_LEN),
            fps: 0.0,
            frame_time_avg: 0.0,
            frame_time_min: 0.0,
            frame_time_max: 0.0,
            system_timers: SystemTimers::new(),
            paused: false,
            target_cat_count: 1000,
            cat_count_changed: false,
            present_mode_index: 0, // Mailbox
            present_mode_changed: false,
            entity_count: 0,
            tick_count: 0,
            current_mode: AppMode::Play,
            idle_seconds: 0.0,
            edge_affinity: 0.0,
            energy_scale: 1.0,
            mode_changed: false,
            selected_mode_index: 1, // Play
            show_trails: false,
            show_heatmap: false,
            hovered_cat: None,
            frame_count: 0,
            log_timer: 0.0,
            log_frame_count: 0,
            log_frame_sum: 0.0,
            log_frame_min: f64::MAX,
            log_frame_max: 0.0,
        }
    }

    /// Record a frame time, update rolling stats, and periodically log.
    pub fn record_frame(&mut self, dt: f64) {
        self.frame_count += 1;
        self.frame_times.push(dt);

        // Compute stats from ring buffer.
        let len = self.frame_times.len();
        if len > 0 {
            let mut sum = 0.0;
            let mut min = f64::MAX;
            let mut max = 0.0f64;
            for &t in self.frame_times.iter() {
                sum += t;
                min = min.min(t);
                max = max.max(t);
            }
            self.frame_time_avg = sum / len as f64;
            self.frame_time_min = min;
            self.frame_time_max = max;
            self.fps = 1.0 / self.frame_time_avg;
        }

        // Periodic log (every 5s).
        self.log_frame_count += 1;
        self.log_frame_sum += dt;
        self.log_frame_min = self.log_frame_min.min(dt);
        self.log_frame_max = self.log_frame_max.max(dt);
        self.log_timer += dt;

        if self.log_timer >= 5.0 {
            let avg_ms = (self.log_frame_sum / self.log_frame_count as f64) * 1000.0;
            let fps = self.log_frame_count as f64 / self.log_timer;
            log::info!(
                "FPS: {:.0} | avg: {:.2}ms | min: {:.2}ms | max: {:.2}ms | total frames: {}",
                fps,
                avg_ms,
                self.log_frame_min * 1000.0,
                self.log_frame_max * 1000.0,
                self.frame_count,
            );
            self.log_timer = 0.0;
            self.log_frame_count = 0;
            self.log_frame_sum = 0.0;
            self.log_frame_min = f64::MAX;
            self.log_frame_max = 0.0;
        }
    }

    /// Handle F12 toggle. Returns true if visibility changed.
    pub fn poll_toggle(&mut self, f12_down: bool) -> bool {
        // Edge-detect: trigger on press, not hold.
        if f12_down && !self.f12_was_down {
            self.f12_was_down = true;
            self.visible = !self.visible;
            return true;
        }
        if !f12_down {
            self.f12_was_down = false;
        }
        false
    }

    /// Forward a winit event to egui. Returns true if egui consumed it.
    pub fn on_window_event(
        &mut self,
        window: &Window,
        event: &winit::event::WindowEvent,
    ) -> bool {
        let response = self.egui_state.on_window_event(window, event);
        response.consumed
    }

    /// The selected present mode.
    pub fn selected_present_mode(&self) -> wgpu::PresentMode {
        PRESENT_MODES[self.present_mode_index]
    }

    /// Run the egui frame and produce paint output.
    /// Returns (clipped_primitives, textures_delta, screen_descriptor).
    pub fn run_frame(
        &mut self,
        window: &Window,
        screen_w: u32,
        screen_h: u32,
    ) -> (
        Vec<egui::epaint::ClippedPrimitive>,
        egui::TexturesDelta,
        egui_wgpu::ScreenDescriptor,
    ) {
        let raw_input = self.egui_state.take_egui_input(window);

        // Snapshot read-only state for UI drawing (avoids borrow conflict
        // between egui_ctx.run() borrowing self and the closure borrowing self).
        let (hovered_name, hovered_state, hovered_personality) =
            if let Some(ref info) = self.hovered_cat {
                (
                    Some(info.name.clone()),
                    Some(format!("{:?}", info.state)),
                    Some([
                        info.personality.laziness,
                        info.personality.energy,
                        info.personality.curiosity,
                        info.personality.skittishness,
                    ]),
                )
            } else {
                (None, None, None)
            };

        let ui_state = UiSnapshot {
            visible: self.visible,
            fps: self.fps,
            frame_time_avg: self.frame_time_avg,
            frame_time_min: self.frame_time_min,
            frame_time_max: self.frame_time_max,
            frame_times: self.frame_times.iter().copied().collect(),
            system_durations: self.system_timers.durations_us,
            entity_count: self.entity_count,
            tick_count: self.tick_count,
            current_mode: self.current_mode,
            idle_seconds: self.idle_seconds,
            edge_affinity: self.edge_affinity,
            energy_scale: self.energy_scale,
            hovered_cat_name: hovered_name,
            hovered_cat_state: hovered_state,
            hovered_cat_personality: hovered_personality,
        };

        // Mutable controls — read from self, written back after run().
        let mut paused = self.paused;
        let mut target_cat_count = self.target_cat_count;
        let mut present_mode_index = self.present_mode_index;
        let mut selected_mode_index = self.selected_mode_index;
        let mut show_trails = self.show_trails;
        let mut show_heatmap = self.show_heatmap;

        let ctx = self.egui_ctx.clone();
        let full_output = ctx.run(raw_input, |ctx| {
            draw_ui(
                ctx, &ui_state,
                &mut paused, &mut target_cat_count, &mut present_mode_index,
                &mut selected_mode_index, &mut show_trails, &mut show_heatmap,
            );
        });

        // Write back mutable controls.
        self.paused = paused;
        if target_cat_count != self.target_cat_count {
            self.cat_count_changed = true;
        }
        self.target_cat_count = target_cat_count;
        if present_mode_index != self.present_mode_index {
            self.present_mode_changed = true;
        }
        self.present_mode_index = present_mode_index;
        if selected_mode_index != self.selected_mode_index {
            self.mode_changed = true;
        }
        self.selected_mode_index = selected_mode_index;
        self.show_trails = show_trails;
        self.show_heatmap = show_heatmap;

        self.egui_state
            .handle_platform_output(window, full_output.platform_output);

        let pixels_per_point = full_output.pixels_per_point;
        let clipped_primitives = self.egui_ctx.tessellate(full_output.shapes, pixels_per_point);

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [screen_w, screen_h],
            pixels_per_point,
        };

        (clipped_primitives, full_output.textures_delta, screen_descriptor)
    }

    /// Upload egui textures and buffers. Call before draw_egui render pass.
    pub fn prepare_egui(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        primitives: &[egui::epaint::ClippedPrimitive],
        textures_delta: &egui::TexturesDelta,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
    ) -> Vec<wgpu::CommandBuffer> {
        for (id, image_delta) in &textures_delta.set {
            self.egui_renderer
                .update_texture(device, queue, *id, image_delta);
        }

        self.egui_renderer
            .update_buffers(device, queue, encoder, primitives, screen_descriptor)
    }

    /// Render egui into the given render pass.
    pub fn render_egui(
        &self,
        render_pass: &mut wgpu::RenderPass<'static>,
        primitives: &[egui::epaint::ClippedPrimitive],
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
    ) {
        self.egui_renderer
            .render(render_pass, primitives, screen_descriptor);
    }

    /// Free textures after present.
    pub fn free_textures(&mut self, textures_delta: &egui::TexturesDelta) {
        for &id in &textures_delta.free {
            self.egui_renderer.free_texture(&id);
        }
    }

}

// ---------------------------------------------------------------------------
// UI snapshot + free-function draw (avoids borrow conflicts with egui_ctx)
// ---------------------------------------------------------------------------

struct UiSnapshot {
    visible: bool,
    fps: f64,
    frame_time_avg: f64,
    frame_time_min: f64,
    frame_time_max: f64,
    frame_times: Vec<f64>,
    system_durations: [f64; 8],
    entity_count: usize,
    tick_count: u64,
    current_mode: AppMode,
    idle_seconds: f64,
    edge_affinity: f32,
    energy_scale: f32,
    hovered_cat_name: Option<String>,
    hovered_cat_state: Option<String>,
    hovered_cat_personality: Option<[f32; 4]>,
}

fn draw_ui(
    ctx: &egui::Context,
    s: &UiSnapshot,
    paused: &mut bool,
    target_cat_count: &mut usize,
    present_mode_index: &mut usize,
    selected_mode_index: &mut usize,
    show_trails: &mut bool,
    show_heatmap: &mut bool,
) {
    if !s.visible {
        return;
    }

    let panel_frame = egui::Frame::NONE
        .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 20, 220))
        .corner_radius(6.0)
        .inner_margin(10.0);

    egui::Window::new("Debug")
        .default_pos([10.0, 10.0])
        .default_width(320.0)
        .resizable(true)
        .frame(panel_frame)
        .show(ctx, |ui| {
            ui.style_mut().visuals.override_text_color = Some(egui::Color32::from_gray(220));

            // --- Performance ---
            ui.heading("Performance");
            ui.label(format!("FPS: {:.1}", s.fps));
            ui.label(format!(
                "Frame: {:.2}ms avg | {:.2} min | {:.2} max",
                s.frame_time_avg * 1000.0,
                s.frame_time_min * 1000.0,
                s.frame_time_max * 1000.0,
            ));
            ui.add_space(4.0);

            // --- Frame time histogram ---
            ui.heading("Frame Time History");
            if !s.frame_times.is_empty() {
                let max_time = s
                    .frame_times
                    .iter()
                    .copied()
                    .fold(0.0f64, f64::max)
                    .max(0.020);

                let (response, painter) =
                    ui.allocate_painter(egui::vec2(300.0, 60.0), egui::Sense::hover());
                let rect = response.rect;

                let bar_width = rect.width() / s.frame_times.len() as f32;
                let target_y = rect.bottom() - (0.01667 / max_time as f32) * rect.height();

                for (i, &t) in s.frame_times.iter().enumerate() {
                    let h = (t / max_time) as f32 * rect.height();
                    let x = rect.left() + i as f32 * bar_width;
                    let color = if t > 0.01667 {
                        egui::Color32::from_rgb(255, 100, 80)
                    } else {
                        egui::Color32::from_rgb(80, 200, 120)
                    };
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(x, rect.bottom() - h),
                            egui::pos2(x + bar_width - 1.0, rect.bottom()),
                        ),
                        0.0,
                        color,
                    );
                }

                // 16.67ms target line
                painter.line_segment(
                    [
                        egui::pos2(rect.left(), target_y),
                        egui::pos2(rect.right(), target_y),
                    ],
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 255, 100)),
                );
            }
            ui.add_space(4.0);

            // --- System timers ---
            ui.heading("System Timers");
            let total: f64 = s.system_durations.iter().sum::<f64>().max(1.0);
            let max_us = s
                .system_durations
                .iter()
                .copied()
                .fold(0.0f64, f64::max)
                .max(1.0);

            for phase in SystemPhase::ALL {
                let us = s.system_durations[phase as usize];
                let pct = us / total * 100.0;
                let bar_frac = (us / max_us) as f32;

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{:<12}", phase.label())).monospace());
                    ui.label(
                        egui::RichText::new(format!("{:>5.0}us ({:>2.0}%)", us, pct)).monospace(),
                    );

                    let (response, painter) =
                        ui.allocate_painter(egui::vec2(80.0, 12.0), egui::Sense::hover());
                    let r = response.rect;
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            r.left_top(),
                            egui::pos2(r.left() + r.width() * bar_frac, r.bottom()),
                        ),
                        2.0,
                        egui::Color32::from_rgb(100, 180, 255),
                    );
                });
            }
            ui.label(
                egui::RichText::new(format!("Total: {:.0}us ({:.2}ms)", total, total / 1000.0))
                    .monospace(),
            );
            ui.add_space(4.0);

            // --- Controls ---
            ui.heading("Controls");
            ui.checkbox(paused, "Pause Simulation");

            ui.horizontal(|ui| {
                ui.label("Cat Count:");
                ui.add(egui::Slider::new(target_cat_count, 1..=4096));
            });

            ui.horizontal(|ui| {
                ui.label("Present:");
                egui::ComboBox::from_id_salt("present_mode")
                    .selected_text(PRESENT_MODE_LABELS[*present_mode_index])
                    .show_ui(ui, |ui| {
                        for (i, label) in PRESENT_MODE_LABELS.iter().enumerate() {
                            ui.selectable_value(present_mode_index, i, *label);
                        }
                    });
            });
            ui.add_space(4.0);

            // --- Visuals ---
            ui.heading("Visuals");
            ui.checkbox(show_trails, "Show Trails");
            ui.checkbox(show_heatmap, "Show Heatmap");
            ui.add_space(4.0);

            // --- Mode ---
            ui.heading("Mode");
            ui.horizontal(|ui| {
                ui.label("Mode:");
                let mode_labels = ["Work", "Play", "Zen", "Chaos"];
                egui::ComboBox::from_id_salt("app_mode")
                    .selected_text(mode_labels[*selected_mode_index])
                    .show_ui(ui, |ui| {
                        for (i, label) in mode_labels.iter().enumerate() {
                            ui.selectable_value(selected_mode_index, i, *label);
                        }
                    });
            });
            ui.label(format!("Idle: {:.1}s", s.idle_seconds));
            ui.label(format!(
                "Edge: {:.2} | Energy: {:.2}x",
                s.edge_affinity, s.energy_scale,
            ));
            ui.add_space(4.0);

            // --- Info ---
            ui.heading("Info");
            ui.label(format!(
                "Entities: {} | Ticks: {}",
                s.entity_count, s.tick_count
            ));
            ui.label("F11: Mode | F12: Toggle | ESC: Quit");
            ui.label("Middle-click: Yarn Ball");
        });

    // --- Cat tooltip (floating near cursor) ---
    if let Some(ref name) = s.hovered_cat_name {
        let tooltip_frame = egui::Frame::NONE
            .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 230))
            .corner_radius(4.0)
            .inner_margin(6.0);

        egui::Window::new("cat_tooltip")
            .title_bar(false)
            .fixed_pos(ctx.input(|i| {
                let pos = i.pointer.hover_pos().unwrap_or(egui::pos2(0.0, 0.0));
                [pos.x + 15.0, pos.y - 10.0]
            }))
            .resizable(false)
            .frame(tooltip_frame)
            .show(ctx, |ui| {
                ui.style_mut().visuals.override_text_color =
                    Some(egui::Color32::from_gray(230));
                ui.label(egui::RichText::new(name).strong().size(14.0));
                if let Some(ref state_str) = s.hovered_cat_state {
                    ui.label(format!("Mood: {}", state_str));
                }
                if let Some(p) = s.hovered_cat_personality {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "Lazy:{:.0}% Energy:{:.0}% Curious:{:.0}% Skittish:{:.0}%",
                            p[0] * 100.0,
                            p[1] * 100.0,
                            p[2] * 100.0,
                            p[3] * 100.0,
                        ));
                    });
                }
            });
    }
}
