use std::sync::Arc;

use instant::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId, WindowLevel};

use crate::cat;
use crate::ecs::components::{Appearance, CatState, Position, PrevPosition};
use crate::ecs::systems;
use crate::ecs::systems::interaction::InteractionBuffers;
use crate::platform;
use crate::render::instance::CatInstance;
use crate::render::GpuState;
use crate::spatial::{CatSnapshot, SpatialHash};

/// Target simulation tick rate (seconds per tick).
const TICK_RATE: f64 = 1.0 / 60.0;
/// Max accumulated time before we clamp (prevents spiral of death).
const MAX_ACCUMULATOR: f64 = 0.25;
/// How many cats to spawn on startup.
const INITIAL_CAT_COUNT: usize = 1000;
/// How often to log FPS (seconds).
const FPS_LOG_INTERVAL: f64 = 5.0;
/// Spatial hash cell size — 2x interaction radius.
const SPATIAL_CELL_SIZE: f32 = 128.0;
/// Spatial hash table size (prime-ish for good distribution).
const SPATIAL_TABLE_SIZE: usize = 1024;

// ---------------------------------------------------------------------------
// Frame timing (#9)
// ---------------------------------------------------------------------------

struct FrameStats {
    frame_count: u64,
    last_log_time: Instant,
    frame_time_sum: f64,
    frame_time_min: f64,
    frame_time_max: f64,
    frames_since_log: u32,
}

impl FrameStats {
    fn new() -> Self {
        Self {
            frame_count: 0,
            last_log_time: Instant::now(),
            frame_time_sum: 0.0,
            frame_time_min: f64::MAX,
            frame_time_max: 0.0,
            frames_since_log: 0,
        }
    }

    fn record_frame(&mut self, dt: f64) {
        self.frame_count += 1;
        self.frames_since_log += 1;
        self.frame_time_sum += dt;
        self.frame_time_min = self.frame_time_min.min(dt);
        self.frame_time_max = self.frame_time_max.max(dt);

        let elapsed = self.last_log_time.elapsed().as_secs_f64();
        if elapsed >= FPS_LOG_INTERVAL {
            let avg_ms = (self.frame_time_sum / self.frames_since_log as f64) * 1000.0;
            let fps = self.frames_since_log as f64 / elapsed;
            log::info!(
                "FPS: {:.0} | avg: {:.2}ms | min: {:.2}ms | max: {:.2}ms | total frames: {}",
                fps,
                avg_ms,
                self.frame_time_min * 1000.0,
                self.frame_time_max * 1000.0,
                self.frame_count,
            );
            self.last_log_time = Instant::now();
            self.frame_time_sum = 0.0;
            self.frame_time_min = f64::MAX;
            self.frame_time_max = 0.0;
            self.frames_since_log = 0;
        }
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

/// Top-level application state.
struct App {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,

    // ECS
    world: hecs::World,

    // Spatial hash
    spatial_grid: SpatialHash,

    // Snapshot cache for interaction queries
    snapshots: Vec<CatSnapshot>,

    // Interaction buffers (pre-allocated, reused each tick)
    interaction_bufs: InteractionBuffers,

    // RNG (shared, deterministic per session)
    rng: fastrand::Rng,

    // Fixed timestep
    last_frame_time: Option<Instant>,
    accumulator: f64,
    tick_count: u64,

    // Frame timing
    frame_stats: FrameStats,

    // Screen dimensions
    screen_w: u32,
    screen_h: u32,

    // Reusable instance buffer (avoid per-frame allocation)
    instance_buf: Vec<CatInstance>,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            gpu: None,
            world: hecs::World::new(),
            spatial_grid: SpatialHash::new(SPATIAL_CELL_SIZE, SPATIAL_TABLE_SIZE),
            snapshots: Vec::with_capacity(INITIAL_CAT_COUNT),
            interaction_bufs: InteractionBuffers::new(INITIAL_CAT_COUNT),
            rng: fastrand::Rng::new(),
            last_frame_time: None,
            accumulator: 0.0,
            tick_count: 0,
            frame_stats: FrameStats::new(),
            screen_w: 0,
            screen_h: 0,
            instance_buf: Vec::with_capacity(INITIAL_CAT_COUNT),
        }
    }

    /// Run fixed-timestep simulation ticks.
    fn run_fixed_update(&mut self, dt: f64) {
        self.accumulator += dt;

        if self.accumulator > MAX_ACCUMULATOR {
            self.accumulator = MAX_ACCUMULATOR;
        }

        // Get mouse position once per frame (not per tick)
        #[cfg(windows)]
        let (mouse_x, mouse_y) = platform::win32::get_mouse_pos();
        #[cfg(not(windows))]
        let (mouse_x, mouse_y) = (0.0f32, 0.0f32);

        while self.accumulator >= TICK_RATE {
            systems::tick(
                &mut self.world,
                TICK_RATE as f32,
                self.screen_w as f32,
                self.screen_h as f32,
                mouse_x,
                mouse_y,
                &mut self.rng,
                &mut self.spatial_grid,
                &mut self.snapshots,
                &mut self.interaction_bufs,
            );

            self.accumulator -= TICK_RATE;
            self.tick_count += 1;
        }
    }

    /// Interpolation alpha for rendering between ticks.
    fn interpolation_alpha(&self) -> f32 {
        (self.accumulator / TICK_RATE) as f32
    }

    /// Build instance buffer from ECS world for rendering.
    fn build_instances(&mut self) {
        self.instance_buf.clear();
        let alpha = self.interpolation_alpha();

        for (_, (pos, prev_pos, appearance, cat_state)) in self
            .world
            .query::<(&Position, &PrevPosition, &Appearance, &CatState)>()
            .iter()
        {
            self.instance_buf
                .push(CatInstance::from_components(pos, prev_pos, appearance, cat_state, alpha));
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Create fullscreen-sized borderless transparent window
        let monitor = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
            .expect("no monitor found");
        let screen_size = monitor.size();

        // No with_transparent(true) — that sets WS_EX_LAYERED which creates
        // a GDI backing surface that conflicts with DirectComposition.
        // Transparency comes from wgpu's DxgiFromVisual + PreMultiplied alpha.
        // Start hidden so DWM doesn't cache stale frame state before our
        // overlay style changes take effect.
        let attrs = WindowAttributes::default()
            .with_title("PetToy")
            .with_decorations(false)
            .with_visible(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_inner_size(screen_size)
            .with_position(winit::dpi::PhysicalPosition::new(0, 0));

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );

        #[cfg(windows)]
        platform::win32::setup_overlay(&window);

        let size = window.inner_size();
        self.screen_w = size.width;
        self.screen_h = size.height;

        log::info!(
            "Overlay window created: {}x{} on {:?}",
            size.width,
            size.height,
            monitor.name().unwrap_or_default()
        );

        // Initialize wgpu + pipeline
        let gpu = GpuState::new(window.clone());
        self.gpu = Some(gpu);
        log::info!("wgpu + cat pipeline initialized");

        // Spawn cats
        cat::spawn_cats(
            &mut self.world,
            INITIAL_CAT_COUNT,
            self.screen_w as f32,
            self.screen_h as f32,
        );
        log::info!("Spawned {} cats", INITIAL_CAT_COUNT);

        // Continuous game loop
        event_loop.set_control_flow(ControlFlow::Poll);

        // Show window now that all styles and GPU resources are ready.
        // This prevents DWM from caching stale frame state (the "white box").
        window.set_visible(true);

        self.window = Some(window);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Poll ESC key (window is click-through so can't receive keyboard events)
        #[cfg(windows)]
        if platform::win32::is_escape_pressed() {
            log::info!("ESC pressed, exiting");
            event_loop.exit();
            return;
        }

        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                log::info!("Close requested, exiting");
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(new_size.width, new_size.height);
                    self.screen_w = new_size.width;
                    self.screen_h = new_size.height;
                }
            }
            WindowEvent::RedrawRequested => {
                // --- Timing ---
                let now = Instant::now();
                if let Some(last) = self.last_frame_time {
                    let dt = now.duration_since(last).as_secs_f64();

                    // Frame stats
                    self.frame_stats.record_frame(dt);

                    // Fixed timestep sim
                    self.run_fixed_update(dt);
                }
                self.last_frame_time = Some(now);

                // --- Build instance buffer from ECS ---
                self.build_instances();

                // --- Render ---
                if let Some(gpu) = &mut self.gpu {
                    gpu.update_instances(&self.instance_buf);
                    gpu.render_frame();
                }
            }
            _ => {}
        }
    }
}

/// Entry point — create event loop and run.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}
