use glam::Vec2;

use crate::ecs::components::{BehaviorState, CatState, Personality, Position, Velocity};

/// Distance within which cats notice the mouse.
const MOUSE_NOTICE_RADIUS: f32 = 200.0;
/// Speed at which cats chase the mouse.
const CHASE_SPEED: f32 = 100.0;
/// Chance per tick that a curious cat starts chasing (if in idle-ish state).
const CHASE_CHANCE_PER_TICK: f32 = 0.02;

/// Track global mouse position and update chase targets.
pub fn update_mouse_pos(
    world: &mut hecs::World,
    mouse_x: f32,
    mouse_y: f32,
    rng: &mut fastrand::Rng,
) {
    let mouse = Vec2::new(mouse_x, mouse_y);

    for (_, (pos, vel, state, personality)) in world
        .query_mut::<(&Position, &mut Velocity, &mut CatState, &Personality)>()
    {
        let to_mouse = mouse - pos.0;
        let dist = to_mouse.length();

        if state.state == BehaviorState::ChasingMouse {
            // Already chasing — steer toward mouse
            if dist > 10.0 {
                let dir = to_mouse / dist;
                let speed = CHASE_SPEED * (0.7 + personality.curiosity * 0.6);
                vel.0 = dir * speed;
            } else {
                // Reached mouse, go idle
                state.state = BehaviorState::Idle;
                state.timer = 0.5 + rng.f32() * 1.0;
            }
            continue;
        }

        // Only idle/walking cats consider chasing
        if !matches!(
            state.state,
            BehaviorState::Idle | BehaviorState::Walking
        ) {
            continue;
        }

        // Check if mouse is close enough and cat is curious enough
        if dist < MOUSE_NOTICE_RADIUS {
            let chance = CHASE_CHANCE_PER_TICK * (0.5 + personality.curiosity);
            if rng.f32() < chance {
                state.state = BehaviorState::ChasingMouse;
                state.timer = 2.0 + rng.f32() * 3.0;
                let dir = to_mouse / dist;
                let speed = CHASE_SPEED * (0.7 + personality.curiosity * 0.6);
                vel.0 = dir * speed;
            }
        }
    }
}
