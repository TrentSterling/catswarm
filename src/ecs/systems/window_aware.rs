use glam::Vec2;

use crate::ecs::components::{BehaviorState, CatState, Position, Velocity};

/// Platform-agnostic desktop window rectangle.
#[derive(Debug, Clone, Copy)]
pub struct DesktopWindow {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// Distance from titlebar top where cats can snap onto it.
const SNAP_DIST: f32 = 25.0;
/// Per-tick chance an eligible cat near a titlebar will perch.
const PERCH_CHANCE: f32 = 0.015;
/// Walk speed along a titlebar.
const TITLEBAR_WALK_SPEED: f32 = 25.0;

/// Detect cats near window titlebars and snap them onto platforms.
pub fn update(
    world: &mut hecs::World,
    platforms: &[DesktopWindow],
    rng: &mut fastrand::Rng,
) {
    if platforms.is_empty() {
        return;
    }

    for (_, (pos, vel, state)) in
        world.query_mut::<(&mut Position, &mut Velocity, &mut CatState)>()
    {
        // Only affect mobile cats that aren't busy
        if !matches!(
            state.state,
            BehaviorState::Walking | BehaviorState::Idle | BehaviorState::Parading
        ) {
            continue;
        }

        // Find nearest platform top edge
        for plat in platforms {
            let top_y = plat.top;
            let dy = pos.0.y - top_y;

            // Cat must be near the top edge (slightly above or below)
            if dy < -10.0 || dy > SNAP_DIST {
                continue;
            }
            // Cat must be within the horizontal bounds
            if pos.0.x < plat.left - 10.0 || pos.0.x > plat.right + 10.0 {
                continue;
            }

            // Chance to snap to this titlebar
            if rng.f32() > PERCH_CHANCE {
                continue;
            }

            // Snap cat to sit on top of the titlebar
            pos.0.y = top_y - 8.0;
            vel.0.y = 0.0;

            // Walk along the titlebar
            if vel.0.x.abs() < 5.0 {
                vel.0.x = if rng.bool() {
                    TITLEBAR_WALK_SPEED
                } else {
                    -TITLEBAR_WALK_SPEED
                };
            }

            // Clamp to titlebar bounds
            pos.0.x = pos.0.x.clamp(plat.left + 5.0, plat.right - 5.0);

            // Refresh walking timer so they stay on the bar
            if state.state == BehaviorState::Idle {
                state.state = BehaviorState::Walking;
            }
            state.timer = state.timer.max(2.0 + rng.f32() * 3.0);

            break; // One platform per cat per tick
        }
    }
}
