pub mod behavior;
pub mod click;
pub mod interaction;
pub mod mouse;
pub mod movement;
pub mod spatial;
pub mod window_aware;

use crate::debug::timer::{SystemPhase, SystemTimers};
use crate::heatmap::Heatmap;
use crate::spatial::{CatSnapshot, SpatialHash};
use interaction::InteractionBuffers;
use mouse::CursorState;

/// Run all simulation systems for one fixed tick.
pub fn tick(
    world: &mut hecs::World,
    dt: f32,
    screen_w: f32,
    screen_h: f32,
    mouse_x: f32,
    mouse_y: f32,
    cursor: &mut CursorState,
    rng: &mut fastrand::Rng,
    grid: &mut SpatialHash,
    snapshots: &mut Vec<CatSnapshot>,
    interaction_bufs: &mut InteractionBuffers,
    timers: &mut SystemTimers,
    heatmap: &Heatmap,
    edge_affinity: f32,
    platforms: &[window_aware::DesktopWindow],
    energy_scale: f32,
) {
    // 0. Update cursor tracking
    cursor.update(mouse_x, mouse_y, dt);

    // 1. Mouse tracking + chase/flee behavior
    timers.begin();
    mouse::update_mouse_pos(world, mouse_x, mouse_y, cursor, rng);
    timers.end(SystemPhase::Mouse);

    // 2. Behavior state machine transitions
    timers.begin();
    behavior::update(world, dt, rng, energy_scale);
    timers.end(SystemPhase::Behavior);

    // 3. Movement integration (apply velocity, friction, bounds, heatmap avoidance, edge affinity)
    timers.begin();
    movement::integrate(world, dt, screen_w, screen_h, heatmap, edge_affinity);
    timers.end(SystemPhase::Movement);

    // 4. Rebuild spatial hash + snapshot cache
    timers.begin();
    spatial::rebuild(world, grid, snapshots);
    timers.end(SystemPhase::SpatialRebuild);

    // 5. Cat-to-cat interactions
    timers.begin();
    interaction::update(world, snapshots, grid, interaction_bufs, rng, dt);
    timers.end(SystemPhase::Interaction);

    // 6. Window awareness (cats perch on titlebars)
    window_aware::update(world, platforms, rng);
}
