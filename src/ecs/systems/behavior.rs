use glam::Vec2;

use crate::ecs::components::{BehaviorState, CatState, Personality, Position, Velocity};

/// Max walk speed in pixels/second.
const WALK_SPEED: f32 = 40.0;
/// Max run speed in pixels/second.
const RUN_SPEED: f32 = 120.0;
/// Zoomies speed — fast and frantic.
const ZOOMIES_SPEED: f32 = 300.0;
/// Chance per tick that an idle/walking cat gets zoomies (before energy mult).
const ZOOMIES_CHANCE: f32 = 0.003;
/// Startled upward velocity spike.
const STARTLE_JUMP_VY: f32 = -200.0;
/// Startled horizontal scatter speed.
const STARTLE_SCATTER: f32 = 100.0;

/// Update cat behavior state machines — handle transitions, timers.
pub fn update(world: &mut hecs::World, dt: f32, rng: &mut fastrand::Rng) {
    for (_, (state, personality, vel, pos)) in world
        .query_mut::<(&mut CatState, &Personality, &mut Velocity, &Position)>()
    {
        state.timer -= dt;

        // Per-state tick behavior
        match state.state {
            BehaviorState::Zoomies => {
                // Random direction change every 0.3-0.5s (tracked via sub-timer hack:
                // we check if timer crosses a 0.4s boundary)
                let prev = state.timer + dt;
                let interval = 0.4;
                if (prev / interval).floor() != (state.timer / interval).floor() {
                    let angle = rng.f32() * std::f32::consts::TAU;
                    let speed = ZOOMIES_SPEED * (0.8 + rng.f32() * 0.4);
                    vel.0 = Vec2::new(angle.cos(), angle.sin()) * speed;
                }
            }
            BehaviorState::Startled => {
                // Brief upward jump + gravity pull back (0.3s duration)
                // Simulate gravity: pull velocity Y toward 0 rapidly
                vel.0.y += 600.0 * dt; // gravity pulls back down
            }
            BehaviorState::Yawning => {
                // Stationary — velocity decays via friction
            }
            BehaviorState::Walking => {
                // Velocity was set on transition, friction handles slowdown
            }
            BehaviorState::Running => {
                // Same — velocity set on transition
            }
            BehaviorState::Idle
            | BehaviorState::Sleeping
            | BehaviorState::Grooming => {
                // Stationary states — velocity decays via friction in movement system
            }
            BehaviorState::ChasingMouse | BehaviorState::FleeingCursor => {
                // Handled by mouse system
            }
            BehaviorState::ChasingCat | BehaviorState::Playing => {
                // Handled by interaction system
            }
            BehaviorState::Parading => {
                // Handled by interaction system (velocity maintained there)
            }
        }

        if state.timer <= 0.0 {
            // Special transitions from certain states
            match state.state {
                BehaviorState::Startled => {
                    // After startled, transition to Running (flee direction)
                    state.state = BehaviorState::Running;
                    state.timer = 0.5 + rng.f32() * 1.0;
                    // Keep current velocity direction but normalize to run speed
                    let dir = vel.0.normalize_or_zero();
                    vel.0 = dir * RUN_SPEED;
                    continue;
                }
                BehaviorState::Yawning => {
                    // After yawning, fall asleep
                    state.state = BehaviorState::Sleeping;
                    state.timer = 3.0 + rng.f32() * 5.0;
                    vel.0 = Vec2::ZERO;
                    continue;
                }
                BehaviorState::Zoomies => {
                    // After zoomies, rest (idle)
                    state.state = BehaviorState::Idle;
                    state.timer = 1.0 + rng.f32() * 2.0;
                    continue;
                }
                BehaviorState::Parading => {
                    // Parade ended, go back to walking
                    state.state = BehaviorState::Walking;
                    state.timer = 2.0 + rng.f32() * 3.0;
                    continue;
                }
                _ => {}
            }

            // Normal transition to a new state
            transition(state, personality, vel, pos, rng);
        }
    }
}

/// Pick a new random state and configure velocity/timer.
fn transition(
    state: &mut CatState,
    personality: &Personality,
    vel: &mut Velocity,
    _pos: &Position,
    rng: &mut fastrand::Rng,
) {
    // Check for zoomies first (rare, energy-weighted)
    let zoomies_chance = ZOOMIES_CHANCE * personality.energy;
    if rng.f32() < zoomies_chance {
        state.state = BehaviorState::Zoomies;
        state.timer = 1.0 + rng.f32() * 1.0; // 1-2s
        let angle = rng.f32() * std::f32::consts::TAU;
        vel.0 = Vec2::new(angle.cos(), angle.sin()) * ZOOMIES_SPEED;
        return;
    }

    // Weighted random state selection based on personality
    let roll = rng.f32();

    // Lazier cats idle/sleep more, energetic cats walk/run more
    let idle_weight = 0.25 + personality.laziness * 0.2;
    let sleep_weight = 0.15 + personality.laziness * 0.15;
    let groom_weight = 0.1;
    let walk_weight = 0.25 + personality.energy * 0.15;
    let run_weight = 0.1 + personality.energy * 0.1;

    let total = idle_weight + sleep_weight + groom_weight + walk_weight + run_weight;
    let r = roll * total;

    let mut acc = 0.0;

    acc += idle_weight;
    if r < acc {
        state.state = BehaviorState::Idle;
        state.timer = 1.0 + rng.f32() * 3.0;
        return;
    }

    acc += sleep_weight;
    if r < acc {
        state.state = BehaviorState::Sleeping;
        state.timer = 3.0 + rng.f32() * 5.0;
        return;
    }

    acc += groom_weight;
    if r < acc {
        state.state = BehaviorState::Grooming;
        state.timer = 1.5 + rng.f32() * 2.0;
        return;
    }

    acc += walk_weight;
    if r < acc {
        state.state = BehaviorState::Walking;
        state.timer = 2.0 + rng.f32() * 4.0;
        // Pick a random walk direction
        let angle = rng.f32() * std::f32::consts::TAU;
        let speed = WALK_SPEED * (0.5 + personality.energy * 0.5);
        vel.0 = Vec2::new(angle.cos(), angle.sin()) * speed;
        return;
    }

    // Run
    state.state = BehaviorState::Running;
    state.timer = 0.8 + rng.f32() * 1.5;
    let angle = rng.f32() * std::f32::consts::TAU;
    let speed = RUN_SPEED * (0.5 + personality.energy * 0.5);
    vel.0 = Vec2::new(angle.cos(), angle.sin()) * speed;
}

/// Trigger a startle on a specific entity (called externally by interaction/click systems).
pub fn trigger_startle(
    state: &mut CatState,
    vel: &mut Velocity,
    rng: &mut fastrand::Rng,
) {
    state.state = BehaviorState::Startled;
    state.timer = 0.3;
    // Upward spike + random horizontal scatter
    vel.0.y += STARTLE_JUMP_VY;
    vel.0.x += (rng.f32() - 0.5) * STARTLE_SCATTER * 2.0;
}
