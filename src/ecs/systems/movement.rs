use glam::Vec2;

use crate::ecs::components::{BehaviorState, CatState, Position, PrevPosition, Velocity};
use crate::heatmap::Heatmap;

/// Friction coefficient — multiplied per tick to slow cats down.
const FRICTION: f32 = 0.92;
/// Minimum velocity magnitude before snapping to zero.
const MIN_VELOCITY: f32 = 0.5;
/// Heatmap avoidance strength.
const HEAT_AVOIDANCE: f32 = 40.0;
/// Heatmap avoidance threshold — only avoid areas hotter than this.
const HEAT_THRESHOLD: f32 = 0.3;
/// Edge affinity pull strength.
const EDGE_PULL: f32 = 12.0;
/// Edge repulsion zone — cats within this distance of screen edge get pushed inward.
const EDGE_REPULSION_ZONE: f32 = 80.0;
/// Edge repulsion strength (scales linearly as cat approaches edge).
const EDGE_REPULSION_STRENGTH: f32 = 60.0;

/// Integrate velocity into position. Apply friction/damping.
/// Screen bounds clamping keeps cats on-screen.
/// Heatmap avoidance biases mobile cats away from hot zones.
/// Edge affinity pulls walking cats toward screen edges (Work mode).
pub fn integrate(
    world: &mut hecs::World,
    dt: f32,
    screen_w: f32,
    screen_h: f32,
    heatmap: &Heatmap,
    edge_affinity: f32,
) {
    for (_, (pos, prev_pos, vel, cat_state)) in world
        .query_mut::<(&mut Position, &mut PrevPosition, &mut Velocity, &CatState)>()
    {
        // Store previous position for render interpolation
        prev_pos.0 = pos.0;

        let mobile = matches!(
            cat_state.state,
            BehaviorState::Idle
                | BehaviorState::Walking
                | BehaviorState::Running
                | BehaviorState::Parading
        );

        // Heatmap avoidance: sample gradient and push away from hot zones
        if mobile && heatmap.enabled {
            let heat = heatmap.sample(pos.0.x, pos.0.y);
            if heat > HEAT_THRESHOLD {
                let dx = heatmap.sample(pos.0.x + 20.0, pos.0.y)
                    - heatmap.sample(pos.0.x - 20.0, pos.0.y);
                let dy = heatmap.sample(pos.0.x, pos.0.y + 20.0)
                    - heatmap.sample(pos.0.x, pos.0.y - 20.0);
                let gradient = Vec2::new(dx, dy);
                if gradient.length_squared() > 0.0001 {
                    vel.0 -= gradient.normalize() * (heat - HEAT_THRESHOLD) * HEAT_AVOIDANCE;
                }
            }
        }

        // Edge affinity: pull walking cats toward screen edges (Work mode)
        if edge_affinity > 0.01 && cat_state.state == BehaviorState::Walking {
            let center = Vec2::new(screen_w * 0.5, screen_h * 0.5);
            let to_edge = pos.0 - center;
            if to_edge.length_squared() > 1.0 {
                vel.0 += to_edge.normalize() * edge_affinity * EDGE_PULL;
            }
        }

        // Soft edge repulsion: push cats away from screen edges to prevent bunching.
        // Strength ramps linearly from 0 at EDGE_REPULSION_ZONE to full at margin.
        {
            let margin = 8.0;
            let zone = EDGE_REPULSION_ZONE;
            // Left edge
            if pos.0.x < margin + zone {
                let t = 1.0 - ((pos.0.x - margin) / zone).clamp(0.0, 1.0);
                vel.0.x += t * EDGE_REPULSION_STRENGTH;
            }
            // Right edge
            if pos.0.x > screen_w - margin - zone {
                let t = 1.0 - ((screen_w - margin - pos.0.x) / zone).clamp(0.0, 1.0);
                vel.0.x -= t * EDGE_REPULSION_STRENGTH;
            }
            // Top edge
            if pos.0.y < margin + zone {
                let t = 1.0 - ((pos.0.y - margin) / zone).clamp(0.0, 1.0);
                vel.0.y += t * EDGE_REPULSION_STRENGTH;
            }
            // Bottom edge
            if pos.0.y > screen_h - margin - zone {
                let t = 1.0 - ((screen_h - margin - pos.0.y) / zone).clamp(0.0, 1.0);
                vel.0.y -= t * EDGE_REPULSION_STRENGTH;
            }
        }

        // Integrate velocity
        pos.0 += vel.0 * dt;

        // Apply friction
        vel.0 *= FRICTION;

        // Snap tiny velocities to zero
        if vel.0.length_squared() < MIN_VELOCITY * MIN_VELOCITY {
            vel.0 = Vec2::ZERO;
        }

        // Hard clamp to screen bounds (safety net, should rarely trigger now)
        let margin = 8.0;
        pos.0.x = pos.0.x.clamp(margin, screen_w - margin);
        pos.0.y = pos.0.y.clamp(margin, screen_h - margin);
    }
}
