use glam::Vec2;

use crate::ecs::components::{Position, PrevPosition, Velocity};

/// Friction coefficient — multiplied per tick to slow cats down.
const FRICTION: f32 = 0.92;
/// Minimum velocity magnitude before snapping to zero.
const MIN_VELOCITY: f32 = 0.5;

/// Integrate velocity into position. Apply friction/damping.
/// Screen bounds clamping keeps cats on-screen.
pub fn integrate(world: &mut hecs::World, dt: f32, screen_w: f32, screen_h: f32) {
    for (_, (pos, prev_pos, vel)) in world
        .query_mut::<(&mut Position, &mut PrevPosition, &mut Velocity)>()
    {
        // Store previous position for render interpolation
        prev_pos.0 = pos.0;

        // Integrate velocity
        pos.0 += vel.0 * dt;

        // Apply friction
        vel.0 *= FRICTION;

        // Snap tiny velocities to zero
        if vel.0.length_squared() < MIN_VELOCITY * MIN_VELOCITY {
            vel.0 = Vec2::ZERO;
        }

        // Clamp to screen bounds (with small margin so cats stay visible)
        let margin = 8.0;
        pos.0.x = pos.0.x.clamp(margin, screen_w - margin);
        pos.0.y = pos.0.y.clamp(margin, screen_h - margin);
    }
}
