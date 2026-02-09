use std::sync::Arc;

use instant::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId, WindowLevel};

use crate::cat;
use crate::click::ClickState;
use crate::debug::timer::{SystemPhase, SystemTimers};
use crate::debug::DebugOverlay;
use crate::ecs::components::{Appearance, CatState, Position, PrevPosition};
use crate::ecs::systems;
use crate::ecs::systems::interaction::InteractionBuffers;
use crate::ecs::systems::mouse::CursorState;
use crate::mode::{AtkAction, ModeState};
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
/// Spatial hash cell size — 2x interaction radius.
const SPATIAL_CELL_SIZE: f32 = 128.0;
/// Spatial hash table size (prime-ish for good distribution).
const SPATIAL_TABLE_SIZE: usize = 1024;

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

/// Top-level application state.
struct App {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,

    // Debug overlay (initialized after GPU)
    debug: Option<DebugOverlay>,

    // ECS
    world: hecs::World,

    // Spatial hash
    spatial_grid: SpatialHash,

    // Snapshot cache for interaction queries
    snapshots: Vec<CatSnapshot>,

    // Interaction buffers (pre-allocated, reused each tick)
    interaction_bufs: InteractionBuffers,

    // Cursor tracking (speed, still timer)
    cursor_state: CursorState,

    // Mode system (Work/Play/Zen/Chaos + AFK escalation)
    mode_state: ModeState,

    // Click interaction state (startle, treats, laser)
    click_state: ClickState,

    // RNG (shared, deterministic per session)
    rng: fastrand::Rng,

    // Fixed timestep
    last_frame_time: Option<Instant>,
    accumulator: f64,
    tick_count: u64,

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
            debug: None,
            world: hecs::World::new(),
            spatial_grid: SpatialHash::new(SPATIAL_CELL_SIZE, SPATIAL_TABLE_SIZE),
            snapshots: Vec::with_capacity(INITIAL_CAT_COUNT),
            interaction_bufs: InteractionBuffers::new(INITIAL_CAT_COUNT),
            cursor_state: CursorState::new(),
            mode_state: ModeState::new(),
            click_state: ClickState::new(),
            rng: fastrand::Rng::new(),
            last_frame_time: None,
            accumulator: 0.0,
            tick_count: 0,
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

        // Poll mouse buttons for click interactions
        #[cfg(windows)]
        let (left_down, right_down) = platform::win32::get_mouse_buttons();
        #[cfg(not(windows))]
        let (left_down, right_down) = (false, false);

        self.click_state.update(
            left_down,
            right_down,
            glam::Vec2::new(mouse_x, mouse_y),
            dt as f32,
        );

        // Borrow timers from debug overlay (or use a throwaway).
        let timers = match &mut self.debug {
            Some(d) => &mut d.system_timers,
            None => return,
        };

        while self.accumulator >= TICK_RATE {
            systems::tick(
                &mut self.world,
                TICK_RATE as f32,
                self.screen_w as f32,
                self.screen_h as f32,
                mouse_x,
                mouse_y,
                &mut self.cursor_state,
                &mut self.rng,
                &mut self.spatial_grid,
                &mut self.snapshots,
                &mut self.interaction_bufs,
                timers,
            );

            // Click interactions (startle, treats, laser)
            systems::click::update(
                &mut self.world,
                &self.click_state,
                glam::Vec2::new(mouse_x, mouse_y),
                &mut self.rng,
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
    fn build_instances(&mut self, timers: &mut SystemTimers) {
        timers.begin();
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
        timers.end(SystemPhase::BuildInstances);
    }

    /// Spawn or despawn cats to match target count.
    fn sync_cat_count(&mut self, target: usize) {
        let current = self.world.len() as usize;
        if target > current {
            cat::spawn_cats(
                &mut self.world,
                target - current,
                self.screen_w as f32,
                self.screen_h as f32,
            );
        } else if target < current {
            let to_remove = current - target;
            let entities: Vec<hecs::Entity> = self
                .world
                .iter()
                .take(to_remove)
                .map(|e| e.entity())
                .collect();
            for entity in entities {
                let _ = self.world.despawn(entity);
            }
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

        // Initialize debug overlay
        let debug = DebugOverlay::new(&window, &gpu);

        self.gpu = Some(gpu);
        self.debug = Some(debug);
        log::info!("wgpu + cat pipeline + debug overlay initialized");

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

        // Poll F12 for debug overlay toggle
        #[cfg(windows)]
        {
            let f12_down = platform::win32::is_f12_pressed();
            if let (Some(debug), Some(window)) = (&mut self.debug, &self.window) {
                if debug.poll_toggle(f12_down) {
                    let _ = window.set_cursor_hittest(debug.visible);
                    log::info!("Debug overlay: {}", if debug.visible { "shown" } else { "hidden" });
                }
            }
        }

        // Poll F11 for mode cycle
        #[cfg(windows)]
        {
            let f11_down = platform::win32::is_f11_pressed();
            if self.mode_state.poll_f11(f11_down) {
                log::info!("Mode changed to: {}", self.mode_state.mode.label());
            }
        }

        // AFK escalation
        #[cfg(windows)]
        {
            let idle_secs = platform::win32::get_idle_time();
            let dt = self.last_frame_time
                .map(|t| instant::Instant::now().duration_since(t).as_secs_f64())
                .unwrap_or(0.016);
            match self.mode_state.update_afk(idle_secs, dt) {
                AtkAction::SpawnCats(n) => {
                    cat::spawn_cats(
                        &mut self.world,
                        n,
                        self.screen_w as f32,
                        self.screen_h as f32,
                    );
                }
                AtkAction::ScatterAndDespawn(n) => {
                    log::info!("User returned — despawning {} bonus cats", n);
                    self.sync_cat_count((self.world.len() as usize).saturating_sub(n));
                }
                AtkAction::Scatter => {
                    log::info!("User returned — scattering cats");
                }
                AtkAction::None => {}
            }
        }

        // Sync debug mode display
        if let Some(debug) = &mut self.debug {
            debug.current_mode = self.mode_state.mode;
            debug.idle_seconds = self.mode_state.idle_seconds;
            debug.edge_affinity = self.mode_state.edge_affinity;
            debug.energy_scale = self.mode_state.behavior_energy_scale;
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
        // Forward events to egui when overlay is visible
        if let (Some(debug), Some(window)) = (&mut self.debug, &self.window) {
            if debug.visible {
                let consumed = debug.on_window_event(window, &event);
                if consumed {
                    return;
                }
            }
        }

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

                    // Record frame time in debug overlay
                    if let Some(debug) = &mut self.debug {
                        debug.record_frame(dt);
                    }

                    // Fixed timestep sim (unless paused)
                    let paused = self.debug.as_ref().map_or(false, |d| d.paused);
                    if !paused {
                        self.run_fixed_update(dt);
                    }
                }
                self.last_frame_time = Some(now);

                // Sync cat count if slider changed
                if let Some(debug) = &self.debug {
                    let target = debug.target_cat_count;
                    if target != self.world.len() as usize {
                        self.sync_cat_count(target);
                    }
                }

                // Handle present mode change
                if let (Some(debug), Some(gpu)) = (&mut self.debug, &mut self.gpu) {
                    if debug.present_mode_changed {
                        debug.present_mode_changed = false;
                        gpu.set_present_mode(debug.selected_present_mode());
                    }
                }

                // Handle app mode change from debug UI
                if let Some(debug) = &mut self.debug {
                    if debug.mode_changed {
                        debug.mode_changed = false;
                        let modes = crate::mode::ModeState::all_modes();
                        if debug.selected_mode_index < modes.len() {
                            self.mode_state.set_mode(modes[debug.selected_mode_index]);
                            log::info!("Mode set from UI: {}", self.mode_state.mode.label());
                        }
                    }
                }

                // Update entity count / tick count in overlay
                if let Some(debug) = &mut self.debug {
                    debug.entity_count = self.world.len() as usize;
                    debug.tick_count = self.tick_count;
                }

                // --- Build instance buffer from ECS (timed) ---
                // We need to borrow debug.system_timers mutably, but also need self.
                // Extract the timers temporarily.
                let mut timers_temp = self
                    .debug
                    .as_mut()
                    .map(|d| std::mem::replace(&mut d.system_timers, SystemTimers::new()));
                if let Some(timers) = &mut timers_temp {
                    self.build_instances(timers);
                } else {
                    self.instance_buf.clear();
                }

                // --- GPU upload (timed) ---
                if let Some(timers) = &mut timers_temp {
                    timers.begin();
                }
                if let Some(gpu) = &mut self.gpu {
                    gpu.update_instances(&self.instance_buf);
                }
                if let Some(timers) = &mut timers_temp {
                    timers.end(SystemPhase::GpuUpload);
                }

                // Put timers back
                if let (Some(debug), Some(timers)) = (&mut self.debug, timers_temp) {
                    debug.system_timers = timers;
                }

                // --- Run egui frame ---
                let egui_output = if let (Some(debug), Some(window)) =
                    (&mut self.debug, &self.window)
                {
                    if debug.visible {
                        Some(debug.run_frame(window, self.screen_w, self.screen_h))
                    } else {
                        None
                    }
                } else {
                    None
                };

                // --- Render ---
                if let Some(gpu) = &mut self.gpu {
                    let Some(mut frame) = gpu.begin_frame() else {
                        return;
                    };

                    // Time the render submit phase
                    if let Some(debug) = &mut self.debug {
                        debug.system_timers.begin();
                    }

                    // Cat render pass
                    gpu.draw_cats(&mut frame.encoder, &frame.view);

                    // Egui render pass (if visible)
                    let mut extra_cmd_bufs = Vec::new();
                    if let Some((ref primitives, ref textures_delta, ref screen_desc)) =
                        egui_output
                    {
                        if let Some(debug) = &mut self.debug {
                            let bufs = debug.prepare_egui(
                                &gpu.device,
                                &gpu.queue,
                                &mut frame.encoder,
                                primitives,
                                textures_delta,
                                screen_desc,
                            );
                            extra_cmd_bufs = bufs;

                            {
                                let mut egui_pass =
                                    GpuState::begin_egui_pass(&mut frame.encoder, &frame.view);
                                debug.render_egui(&mut egui_pass, primitives, screen_desc);
                            }
                        }
                    }

                    if let Some(debug) = &mut self.debug {
                        debug.system_timers.end(SystemPhase::RenderSubmit);
                    }

                    gpu.finish_frame(frame.encoder, frame.output, extra_cmd_bufs);

                    // Free egui textures after present
                    if let Some((_, ref textures_delta, _)) = egui_output {
                        if let Some(debug) = &mut self.debug {
                            debug.free_textures(textures_delta);
                        }
                    }
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
