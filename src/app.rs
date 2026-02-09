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
use crate::ecs::components::{
    Appearance, BehaviorState, CatName, CatState, Personality, Position, PrevPosition,
    SleepingPile, SpawnAnimation,
};
use crate::ecs::systems;
use crate::ecs::systems::interaction::InteractionBuffers;
use crate::ecs::systems::mouse::CursorState;
use crate::ecs::systems::window_aware::DesktopWindow;
use crate::heatmap::Heatmap;
use crate::mode::{AtkAction, ModeState};
use crate::platform;
use crate::render::instance::CatInstance;
use crate::render::trail::TrailSystem;
use crate::render::GpuState;
use crate::spatial::{CatSnapshot, SpatialHash};
use crate::daynight::DayNightState;
use crate::particles::ParticleSystem;
use crate::toy::YarnBalls;

/// Target simulation tick rate (seconds per tick).
const TICK_RATE: f64 = 1.0 / 60.0;
/// Max accumulated time before we clamp (prevents spiral of death).
const MAX_ACCUMULATOR: f64 = 0.25;
/// How many cats to spawn on startup.
const INITIAL_CAT_COUNT: usize = 20;
/// Target population that the colony grows toward.
const TARGET_CAT_COUNT: usize = 1000;
/// Cats spawned per second during growth phase.
const POPULATION_GROWTH_RATE: f64 = 2.0;
/// Seconds to wait before population starts growing.
const GROWTH_DELAY: f64 = 5.0;
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

    // Visual systems
    trail_system: TrailSystem,
    heatmap: Heatmap,

    // Toys
    yarn_balls: YarnBalls,

    // Emotion particles
    particles: ParticleSystem,

    // Day/night cycle
    daynight: DayNightState,

    // Window platforms (periodically refreshed)
    desktop_windows: Vec<DesktopWindow>,
    window_refresh_timer: f64,

    // RNG (shared, deterministic per session)
    rng: fastrand::Rng,

    // Fixed timestep
    last_frame_time: Option<Instant>,
    accumulator: f64,
    tick_count: u64,

    // Population growth
    elapsed_time: f64,
    spawn_accumulator: f64,

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
            trail_system: TrailSystem::new(),
            heatmap: Heatmap::new(1.0, 1.0),
            yarn_balls: YarnBalls::new(),
            particles: ParticleSystem::new(),
            daynight: DayNightState::new(),
            desktop_windows: Vec::new(),
            window_refresh_timer: 0.0,
            rng: fastrand::Rng::new(),
            last_frame_time: None,
            accumulator: 0.0,
            tick_count: 0,
            elapsed_time: 0.0,
            spawn_accumulator: 0.0,
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
        let (left_down, right_down, middle_down) = platform::win32::get_mouse_buttons();
        #[cfg(not(windows))]
        let (left_down, right_down, middle_down) = (false, false, false);

        self.click_state.update(
            left_down,
            right_down,
            middle_down,
            glam::Vec2::new(mouse_x, mouse_y),
            dt as f32,
        );

        // Update yarn ball physics (mouse pushes them when hovering near)
        let mouse_vec = glam::Vec2::new(mouse_x, mouse_y);
        self.yarn_balls
            .update(dt as f32, self.screen_w as f32, self.screen_h as f32, mouse_vec);

        // Borrow timers from debug overlay (or use a throwaway).
        let timers = match &mut self.debug {
            Some(d) => &mut d.system_timers,
            None => return,
        };

        // Effective energy scale: mode preset * day/night modifier
        let energy_scale = self.mode_state.behavior_energy_scale * self.daynight.energy_modifier;

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
                &self.heatmap,
                self.mode_state.edge_affinity,
                &self.desktop_windows,
                energy_scale,
            );

            // Click interactions (startle, treats, laser, yarn ball)
            systems::click::update(
                &mut self.world,
                &self.click_state,
                glam::Vec2::new(mouse_x, mouse_y),
                &mut self.rng,
                &mut self.yarn_balls,
            );

            // Advance spawn drop-in animations — collect bounce impacts for dust
            let bounces = update_spawn_animations(&mut self.world, TICK_RATE as f32);
            if self.particles.enabled {
                for bounce in &bounces {
                    // Dust poof: more particles for harder impacts
                    let count = (3.0 + bounce.intensity * 8.0) as usize;
                    self.particles.spawn_dust(bounce.pos, count, bounce.intensity, &mut self.rng);
                }
            }

            // Click feedback particles
            if self.particles.enabled {
                let mouse = glam::Vec2::new(mouse_x, mouse_y);
                if self.click_state.left_clicked {
                    self.particles.spawn_burst(mouse, 8, 0xFFFF66EE, 5, &mut self.rng);
                }
                if self.click_state.right_clicked {
                    self.particles.spawn_burst(mouse, 6, 0xDD3333DD, 3, &mut self.rng);
                }
                if self.click_state.double_clicked {
                    self.particles.spawn_burst(mouse, 12, 0xFF0000FF, 5, &mut self.rng);
                }
            }

            // Emotion particles: gather cat states, spawn particles, update physics
            if self.particles.enabled {
                let mut cat_states: Vec<(glam::Vec2, BehaviorState, f32)> =
                    Vec::with_capacity(self.world.len() as usize);
                for (_, (pos, cat_state, appearance)) in self
                    .world
                    .query::<(&Position, &CatState, &Appearance)>()
                    .iter()
                {
                    cat_states.push((pos.0, cat_state.state, appearance.size));
                }
                self.particles
                    .spawn_from_behaviors(&cat_states, &mut self.rng, TICK_RATE as f32);
                self.particles.update(TICK_RATE as f32);
            }

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
        let time = self.elapsed_time as f32;

        for (_, (pos, prev_pos, appearance, cat_state, pile, spawn_anim)) in self
            .world
            .query::<(
                &Position,
                &PrevPosition,
                &Appearance,
                &CatState,
                Option<&SleepingPile>,
                Option<&SpawnAnimation>,
            )>()
            .iter()
        {
            let mut inst = CatInstance::from_components(pos, prev_pos, appearance, cat_state, alpha);

            // Spawn animation: tumble during fall, land feet-first on impact.
            if let Some(anim) = spawn_anim {
                if !anim.has_landed {
                    // During fall: walking frame (asymmetric, rotation visible)
                    inst.frame = 1;

                    // Rotation eased to fall progress (position-based, not time-based).
                    // 0.0 at start_y, 1.0 at target_y — always completes at ground.
                    let fall_dist = anim.target_y - anim.start_y;
                    if fall_dist > 1.0 && anim.flips > 0 {
                        let t = ((pos.0.y - anim.start_y) / fall_dist).clamp(0.0, 1.0);
                        // Ease-in-out quadratic: slow start, fast middle, slow finish (lands upright)
                        let eased = if t < 0.5 {
                            2.0 * t * t
                        } else {
                            1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                        };
                        inst.rotation = eased * std::f32::consts::TAU * anim.flips as f32;
                    }
                }
                // After landing: rotation = 0.0 (default), cat is upright for bounces
            }

            // Breathing animation for sleeping pile members
            if let Some(pile) = pile {
                let breath = (time * 2.0 + pile.breathing_offset).sin() * 0.04;
                inst.size *= 1.0 + breath;
            }

            // Day/night color tint
            inst.color = apply_tint(inst.color, self.daynight.tint);

            self.instance_buf.push(inst);
        }
        // Render yarn balls
        for ball in &self.yarn_balls.balls {
            let fade = (ball.lifetime / 5.0).clamp(0.0, 1.0); // fade in last 5s
            let alpha = (fade * 255.0) as u32;
            self.instance_buf.push(CatInstance {
                position: ball.pos.into(),
                size: 1.0,
                color: (0xDD << 24) | (0x33 << 16) | (0x33 << 8) | alpha,
                frame: 3, // circle SDF
                rotation: 0.0,
            });
        }

        // Render treats as golden star shapes
        for treat in &self.click_state.treats {
            let fade = (treat.timer / 10.0).clamp(0.0, 1.0);
            let alpha = (fade * 200.0 + 55.0) as u32;
            // Pulsing size for visibility
            let pulse = (time * 3.0 + treat.pos.x * 0.01).sin() * 0.1 + 1.0;
            self.instance_buf.push(CatInstance {
                position: treat.pos.into(),
                size: 0.7 * pulse,
                color: (0xFF << 24) | (0xCC << 16) | (0x33 << 8) | alpha, // bright gold
                frame: 5,
                rotation: 0.0,
            });
        }

        // Add laser pointer dot when active
        if self.click_state.laser_active {
            #[cfg(windows)]
            let (mx, my) = platform::win32::get_mouse_pos();
            #[cfg(not(windows))]
            let (mx, my) = (0.0f32, 0.0f32);

            // Pulsing glow effect
            let pulse = (time * 12.0).sin() * 0.15 + 1.0;
            self.instance_buf.push(CatInstance {
                position: [mx, my],
                size: 0.5 * pulse,
                color: 0xFF0000FF, // bright red laser dot
                frame: 3,          // circle SDF
                rotation: 0.0,
            });
        }

        // Emotion particles
        if self.particles.enabled {
            self.particles.build_instances(&mut self.instance_buf);
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

        // Initialize heatmap with actual screen dimensions
        self.heatmap.resize(self.screen_w as f32, self.screen_h as f32);

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
        // Skip everything while minimized — prevents cats from getting
        // clamped to tiny dimensions and flinging apart on restore.
        if let Some(window) = &self.window {
            if window.is_minimized() == Some(true) {
                // Reset frame timer so we don't accumulate a huge dt
                self.last_frame_time = None;
                return;
            }
        }

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

        // Periodically refresh desktop window list for window awareness
        #[cfg(windows)]
        {
            let dt = self.last_frame_time
                .map(|t| instant::Instant::now().duration_since(t).as_secs_f64())
                .unwrap_or(0.016);
            self.window_refresh_timer += dt;
            if self.window_refresh_timer >= 2.0 {
                self.window_refresh_timer = 0.0;
                if let Some(window) = &self.window {
                    let hwnd = platform::win32::get_hwnd(window);
                    let rects = platform::win32::enumerate_windows(hwnd);
                    self.desktop_windows = rects
                        .into_iter()
                        .map(|r| DesktopWindow {
                            left: r.x as f32,
                            top: r.y as f32,
                            right: (r.x + r.w) as f32,
                            bottom: (r.y + r.h) as f32,
                        })
                        .collect();
                }
            }
        }

        // Update day/night cycle from system clock
        self.daynight.update();

        // Sync debug mode display + visual toggles
        if let Some(debug) = &mut self.debug {
            debug.current_mode = self.mode_state.mode;
            debug.idle_seconds = self.mode_state.idle_seconds;
            debug.edge_affinity = self.mode_state.edge_affinity;
            debug.energy_scale = self.mode_state.behavior_energy_scale;
            self.trail_system.enabled = debug.show_trails;
            self.heatmap.enabled = debug.show_heatmap;
            self.particles.enabled = debug.show_particles;

            // Tooltip hit-test: find nearest cat to mouse cursor
            if debug.visible {
                #[cfg(windows)]
                let (mx, my) = platform::win32::get_mouse_pos();
                #[cfg(not(windows))]
                let (mx, my) = (0.0f32, 0.0f32);
                let mouse = glam::Vec2::new(mx, my);

                let mut best: Option<(f32, crate::debug::HoveredCatInfo)> = None;
                for (_, (pos, name, state, personality)) in self
                    .world
                    .query::<(&Position, &CatName, &CatState, &Personality)>()
                    .iter()
                {
                    let dist_sq = (pos.0 - mouse).length_squared();
                    if dist_sq < 40.0 * 40.0 {
                        if best.is_none() || dist_sq < best.as_ref().expect("checked").0 {
                            best = Some((
                                dist_sq,
                                crate::debug::HoveredCatInfo {
                                    screen_pos: pos.0,
                                    name: name.0.clone(),
                                    state: state.state,
                                    personality: *personality,
                                },
                            ));
                        }
                    }
                }
                debug.hovered_cat = best.map(|(_, info)| info);
            } else {
                debug.hovered_cat = None;
            }
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
                // Ignore tiny resizes (happens during minimize animation) —
                // prevents cats from getting clamped to a tiny area.
                // A real desktop window is always larger than 200x200.
                if new_size.width >= 200 && new_size.height >= 200 {
                    if let Some(gpu) = &mut self.gpu {
                        gpu.resize(new_size.width, new_size.height);
                        self.screen_w = new_size.width;
                        self.screen_h = new_size.height;
                        self.heatmap.resize(new_size.width as f32, new_size.height as f32);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                // Skip everything while minimized — simulation would run
                // with stale (tiny) dimensions, crushing cats to a corner.
                if let Some(window) = &self.window {
                    if window.is_minimized() == Some(true) {
                        self.last_frame_time = None;
                        return;
                    }
                }

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

                    // Population growth: gradually spawn cats up to target
                    self.elapsed_time += dt;
                    if self.elapsed_time > GROWTH_DELAY {
                        let current = self.world.len() as usize;
                        if current < TARGET_CAT_COUNT {
                            self.spawn_accumulator += dt * POPULATION_GROWTH_RATE;
                            let to_spawn = self.spawn_accumulator as usize;
                            if to_spawn > 0 {
                                self.spawn_accumulator -= to_spawn as f64;
                                let actual = to_spawn.min(TARGET_CAT_COUNT - current);
                                cat::spawn_cats(
                                    &mut self.world,
                                    actual,
                                    self.screen_w as f32,
                                    self.screen_h as f32,
                                );
                            }
                        }
                    }
                }
                self.last_frame_time = Some(now);

                // Sync cat count only when slider is explicitly changed
                let slider_target = self.debug.as_mut().and_then(|d| {
                    if d.cat_count_changed {
                        d.cat_count_changed = false;
                        Some(d.target_cat_count)
                    } else {
                        None
                    }
                });
                if let Some(target) = slider_target {
                    self.sync_cat_count(target);
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
                    // Keep slider in sync with actual population
                    // (only when user isn't actively changing it)
                    if !debug.cat_count_changed {
                        debug.target_cat_count = self.world.len() as usize;
                    }
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

                // --- Update trail and heatmap ---
                {
                    // Build trail positions from ECS with mood colors
                    if self.trail_system.enabled {
                        let mut trail_positions = Vec::with_capacity(self.world.len() as usize);
                        for (_, (pos, appearance, cat_state)) in
                            self.world.query::<(&Position, &Appearance, &CatState)>().iter()
                        {
                            let color = appearance.color;
                            let base_r = ((color >> 24) & 0xFF) as f32 / 255.0;
                            let base_g = ((color >> 16) & 0xFF) as f32 / 255.0;
                            let base_b = ((color >> 8) & 0xFF) as f32 / 255.0;

                            // Mood color based on behavior state
                            let (mood_r, mood_g, mood_b) = mood_color(cat_state.state);

                            trail_positions.push((
                                pos.0.x, pos.0.y,
                                base_r, base_g, base_b,
                                mood_r, mood_g, mood_b,
                            ));
                        }
                        self.trail_system.update(&trail_positions);
                    }

                    // Update heatmap with cursor position
                    if self.heatmap.enabled {
                        #[cfg(windows)]
                        let (mx, my) = platform::win32::get_mouse_pos();
                        #[cfg(not(windows))]
                        let (mx, my) = (0.0f32, 0.0f32);
                        self.heatmap.update(mx, my, TICK_RATE as f32);
                    }
                }

                // --- GPU upload (timed) ---
                if let Some(timers) = &mut timers_temp {
                    timers.begin();
                }
                if let Some(gpu) = &mut self.gpu {
                    gpu.update_instances(&self.instance_buf);

                    // Upload trail vertices
                    if self.trail_system.enabled {
                        let trail_verts = self.trail_system.build_vertices();
                        gpu.update_trails(trail_verts);
                    }

                    // Upload heatmap texture
                    if self.heatmap.enabled {
                        let heatmap_data = self.heatmap.to_texture_data();
                        gpu.update_heatmap(&heatmap_data);
                    }
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

                    // Cat render pass (includes clear to transparent)
                    gpu.draw_cats(&mut frame.encoder, &frame.view);

                    // Heatmap overlay (behind trails/cats but after clear)
                    if self.heatmap.enabled {
                        gpu.draw_heatmap(&mut frame.encoder, &frame.view);
                    }

                    // Trail render pass (behind cats, after heatmap)
                    if self.trail_system.enabled {
                        gpu.draw_trails(&mut frame.encoder, &frame.view);
                    }

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

/// Apply a color tint to a packed RGBA u32 color.
fn apply_tint(color: u32, tint: [f32; 3]) -> u32 {
    let r = ((color >> 24) & 0xFF) as f32;
    let g = ((color >> 16) & 0xFF) as f32;
    let b = ((color >> 8) & 0xFF) as f32;
    let a = color & 0xFF;
    let tr = (r * tint[0]).min(255.0) as u32;
    let tg = (g * tint[1]).min(255.0) as u32;
    let tb = (b * tint[2]).min(255.0) as u32;
    (tr << 24) | (tg << 16) | (tb << 8) | a
}

/// Map behavior state to a mood color (r, g, b) for trail rendering.
fn mood_color(state: BehaviorState) -> (f32, f32, f32) {
    match state {
        BehaviorState::Idle => (0.7, 0.7, 0.7),                    // soft white/gray
        BehaviorState::Walking | BehaviorState::Parading => (0.4, 0.8, 0.4), // green
        BehaviorState::Running => (0.9, 0.3, 0.2),                 // red
        BehaviorState::Sleeping | BehaviorState::Grooming => (0.3, 0.5, 0.9), // blue
        BehaviorState::ChasingMouse | BehaviorState::ChasingCat => (0.9, 0.5, 0.8), // pink
        BehaviorState::FleeingCursor => (0.9, 0.7, 0.2),           // yellow/orange
        BehaviorState::Playing => (0.9, 0.4, 0.7),                 // pink/magenta
        BehaviorState::Zoomies => (1.0, 0.2, 0.1),                 // bright red
        BehaviorState::Startled => (1.0, 0.9, 0.2),                // bright yellow
        BehaviorState::Yawning => (0.5, 0.5, 0.8),                 // muted blue
    }
}

/// Gravity for spawn animation (pixels/second²). Tuned for ~1s fall time.
const SPAWN_GRAVITY: f32 = 1500.0;
/// Velocity multiplier on bounce (energy retained).
const BOUNCE_DAMPING: f32 = 0.4;
/// After this many bounces, animation is done.
const MAX_BOUNCES: u8 = 3;
/// If velocity is below this after a bounce, stop early.
const BOUNCE_VEL_THRESHOLD: f32 = 30.0;

/// A bounce impact event — position + intensity for dust particles.
struct BounceEvent {
    pos: glam::Vec2,
    intensity: f32, // 0.0–1.0, higher = bigger impact
}

/// Advance spawn drop-in animations with real physics.
/// Gravity accelerates cats downward. On impact at target_y, velocity reverses
/// with damping for a natural bounce. Returns bounce events for particle effects.
fn update_spawn_animations(world: &mut hecs::World, dt: f32) -> Vec<BounceEvent> {
    let mut done = Vec::new();
    let mut bounces = Vec::new();

    for (entity, (pos, prev_pos, anim)) in
        world.query_mut::<(&mut Position, &mut PrevPosition, &mut SpawnAnimation)>()
    {
        // Apply gravity (positive Y = downward on screen)
        anim.vel_y += SPAWN_GRAVITY * dt;
        prev_pos.0.y = pos.0.y;
        pos.0.y += anim.vel_y * dt;

        // Ground collision detection
        if pos.0.y >= anim.target_y {
            // Record bounce intensity from impact velocity (before damping)
            let impact_vel = anim.vel_y.abs();
            let intensity = (impact_vel / 800.0).clamp(0.0, 1.0);

            pos.0.y = anim.target_y;
            anim.has_landed = true;
            anim.bounce_count += 1;

            // Reverse velocity with damping
            anim.vel_y = -anim.vel_y.abs() * BOUNCE_DAMPING;

            // Emit bounce event for dust particles
            if intensity > 0.05 {
                bounces.push(BounceEvent {
                    pos: glam::Vec2::new(pos.0.x, anim.target_y),
                    intensity,
                });
            }

            // Check if animation is done (enough bounces or too little energy)
            if anim.bounce_count >= MAX_BOUNCES || anim.vel_y.abs() < BOUNCE_VEL_THRESHOLD {
                pos.0.y = anim.target_y;
                prev_pos.0.y = anim.target_y;
                done.push(entity);
            }
        }
    }

    for entity in done {
        let _ = world.remove_one::<SpawnAnimation>(entity);
    }

    bounces
}

/// Entry point — create event loop and run.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}
