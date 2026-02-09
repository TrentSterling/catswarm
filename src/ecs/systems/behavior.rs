use glam::Vec2;

use crate::ecs::components::{BehaviorState, CatState, Personality, Position, Velocity};

/// Max walk speed in pixels/second.
const WALK_SPEED: f32 = 40.0;
/// Max run speed in pixels/second.
const RUN_SPEED: f32 = 120.0;

/// Update cat behavior state machines — handle transitions, timers.
pub fn update(world: &mut hecs::World, dt: f32, rng: &mut fastrand::Rng) {
    for (_, (state, personality, vel, pos)) in world
        .query_mut::<(&mut CatState, &Personality, &mut Velocity, &Position)>()
    {
        state.timer -= dt;

        if state.timer <= 0.0 {
            // Transition to a new state
            transition(state, personality, vel, pos, rng);
        }

        // Per-state tick behavior
        match state.state {
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
            BehaviorState::ChasingMouse => {
                // Handled by mouse system
            }
            BehaviorState::ChasingCat | BehaviorState::Playing => {
                // Handled by interaction system
            }
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
