use glam::Vec2;

use crate::ecs::components::{
    Appearance, BehaviorState, CatState, Position, Stacked, Velocity,
};
use crate::spatial::{CatSnapshot, SpatialHash};

/// Cats within this distance can start stacking.
const STACK_RADIUS_SQ: f32 = 35.0 * 35.0;
/// Per-tick chance to climb on a nearby cat (scaled by energy).
const STACK_CHANCE: f32 = 0.002;
/// Max cats sitting on a single base cat.
const MAX_ON_ONE_BASE: u8 = 3;
/// Y offset per stacking level (pixels * base_size). Negative = upward.
const STACK_Y_OFFSET: f32 = 30.0;

/// Update cat towers: maintain position constraints, collapse when
/// base cats move, detect new stacking opportunities.
pub fn update(
    world: &mut hecs::World,
    snapshots: &[CatSnapshot],
    grid: &SpatialHash,
    rng: &mut fastrand::Rng,
) {
    // Phase 1: Collect all stacking relationships
    let stacks: Vec<(hecs::Entity, hecs::Entity)> = world
        .query::<&Stacked>()
        .iter()
        .map(|(entity, stacked)| (entity, stacked.base))
        .collect();

    // Phase 2: Check for collapse (base moved or entity gone)
    let mut collapses: Vec<hecs::Entity> = Vec::new();
    for &(entity, base) in &stacks {
        let base_ok = world
            .get::<&CatState>(base)
            .map(|s| {
                matches!(
                    s.state,
                    BehaviorState::Idle
                        | BehaviorState::Sleeping
                        | BehaviorState::Grooming
                        | BehaviorState::Yawning
                )
            })
            .unwrap_or(false);

        // Also collapse if the stacked cat itself changed to a mobile state
        let self_ok = world
            .get::<&CatState>(entity)
            .map(|s| {
                matches!(
                    s.state,
                    BehaviorState::Idle
                        | BehaviorState::Sleeping
                        | BehaviorState::Grooming
                        | BehaviorState::Yawning
                )
            })
            .unwrap_or(false);

        if !base_ok || !self_ok {
            collapses.push(entity);
        }
    }

    // Apply collapses: startled hop off
    for &entity in &collapses {
        let _ = world.remove_one::<Stacked>(entity);
        if let Ok(mut state) = world.get::<&mut CatState>(entity) {
            state.state = BehaviorState::Startled;
            state.timer = 0.3;
        }
        if let Ok(mut vel) = world.get::<&mut Velocity>(entity) {
            vel.0.y = -120.0; // small upward hop
            vel.0.x = (rng.f32() - 0.5) * 100.0; // random sideways
        }
    }

    // Phase 3: Maintain position constraints (snap stacked cats on top of base)
    // Re-collect stacking since collapses may have removed some
    let remaining: Vec<(hecs::Entity, hecs::Entity)> = world
        .query::<&Stacked>()
        .iter()
        .map(|(entity, stacked)| (entity, stacked.base))
        .collect();

    for &(entity, base) in &remaining {
        let base_info = world
            .get::<&Position>(base)
            .ok()
            .map(|p| p.0)
            .and_then(|pos| {
                world
                    .get::<&Appearance>(base)
                    .ok()
                    .map(|a| (pos, a.size))
            });

        if let Some((base_pos, base_size)) = base_info {
            let target = Vec2::new(base_pos.x, base_pos.y - base_size * STACK_Y_OFFSET);
            if let Ok(mut pos) = world.get::<&mut Position>(entity) {
                pos.0 = target;
            }
            if let Ok(mut vel) = world.get::<&mut Velocity>(entity) {
                vel.0 = Vec2::ZERO;
            }
        }
    }

    // Phase 4: Detect new stacking opportunities
    // Count how many cats are on each base
    let mut base_counts: Vec<(hecs::Entity, u8)> = Vec::new();
    for &(_, base) in &remaining {
        if let Some(entry) = base_counts.iter_mut().find(|(e, _)| *e == base) {
            entry.1 += 1;
        } else {
            base_counts.push((base, 1));
        }
    }

    let len = snapshots.len();
    let mut to_stack: Vec<(hecs::Entity, hecs::Entity)> = Vec::new();

    for i in 0..len {
        let me = &snapshots[i];

        // Only idle/walking cats with decent energy try to climb
        if !matches!(me.state, BehaviorState::Idle | BehaviorState::Walking) {
            continue;
        }
        if me.personality.energy < 0.4 || me.personality.curiosity < 0.3 {
            continue;
        }
        // Already stacked? Skip.
        if me.is_stacked {
            continue;
        }

        grid.query_neighbors(me.pos, |j_idx| {
            let j = j_idx as usize;
            if j == i || j >= len {
                return;
            }
            let them = &snapshots[j];

            // Base must be stationary
            if !matches!(
                them.state,
                BehaviorState::Idle
                    | BehaviorState::Sleeping
                    | BehaviorState::Grooming
            ) {
                return;
            }

            let dist_sq = (me.pos - them.pos).length_squared();
            if dist_sq > STACK_RADIUS_SQ {
                return;
            }

            // Don't stack on already-stacked cats (no chains, just 1 level)
            if them.is_stacked {
                return;
            }

            // Check stack limit on base
            let count = base_counts
                .iter()
                .find(|(e, _)| *e == them.entity)
                .map(|(_, c)| *c)
                .unwrap_or(0);
            if count >= MAX_ON_ONE_BASE {
                return;
            }

            if rng.f32() < STACK_CHANCE * me.personality.energy {
                to_stack.push((me.entity, them.entity));
            }
        });
    }

    // Apply new stacking
    for (climber, base) in to_stack {
        // Verify not already stacked (may have been claimed this frame)
        if world.get::<&Stacked>(climber).is_ok() {
            continue;
        }
        let _ = world.insert_one(climber, Stacked { base });
        if let Ok(mut state) = world.get::<&mut CatState>(climber) {
            state.state = BehaviorState::Idle;
            state.timer = 5.0 + rng.f32() * 10.0;
        }
        if let Ok(mut vel) = world.get::<&mut Velocity>(climber) {
            vel.0 = Vec2::ZERO;
        }

        // Update base_counts so we don't over-stack in this frame
        if let Some(entry) = base_counts.iter_mut().find(|(e, _)| *e == base) {
            entry.1 += 1;
        } else {
            base_counts.push((base, 1));
        }
    }
}
