pub mod behavior;
pub mod interaction;
pub mod mouse;
pub mod movement;
pub mod spatial;
pub mod window_aware;

use crate::spatial::{CatSnapshot, SpatialHash};
use interaction::InteractionBuffers;

/// Run all simulation systems for one fixed tick.
pub fn tick(
    world: &mut hecs::World,
    dt: f32,
    screen_w: f32,
    screen_h: f32,
    mouse_x: f32,
    mouse_y: f32,
    rng: &mut fastrand::Rng,
    grid: &mut SpatialHash,
    snapshots: &mut Vec<CatSnapshot>,
    interaction_bufs: &mut InteractionBuffers,
) {
    // 1. Mouse tracking + chase behavior
    mouse::update_mouse_pos(world, mouse_x, mouse_y, rng);

    // 2. Behavior state machine transitions
    behavior::update(world, dt, rng);

    // 3. Movement integration (apply velocity, friction, bounds)
    movement::integrate(world, dt, screen_w, screen_h);

    // 4. Rebuild spatial hash + snapshot cache
    spatial::rebuild(world, grid, snapshots);

    // 5. Cat-to-cat interactions
    interaction::update(world, snapshots, grid, interaction_bufs, rng, dt);

    // 6. Window awareness (TODO: Milestone 6)
    // window_aware::update(world, dt);
}
