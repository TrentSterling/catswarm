use glam::Vec2;

use crate::ecs::components::{BehaviorState, CatState, Personality, Position, Velocity};

/// Distance within which cats notice the mouse.
const MOUSE_NOTICE_RADIUS: f32 = 200.0;
/// Speed at which cats chase the mouse.
const CHASE_SPEED: f32 = 100.0;
/// Chance per tick that a curious cat starts chasing (if in idle-ish state).
const CHASE_CHANCE_PER_TICK: f32 = 0.02;

/// Moses Effect: radius of cursor repulsion when cursor moves fast.
const MOSES_RADIUS: f32 = 400.0;
/// Cursor speed threshold (px/s) to trigger Moses Effect.
const MOSES_SPEED_THRESHOLD: f32 = 100.0;
/// Moses repulsion strength multiplier.
const MOSES_STRENGTH: f32 = 3.5;
/// Max Moses repulsion velocity applied per tick.
const MOSES_MAX_PUSH: f32 = 300.0;

/// Flee speed range for skittish cats.
const FLEE_SPEED_MIN: f32 = 80.0;
const FLEE_SPEED_MAX: f32 = 140.0;
/// Flee trigger radius.
const FLEE_RADIUS: f32 = 200.0;

/// Cursor speed threshold below which it's considered "still".
const CURSOR_STILL_THRESHOLD: f32 = 5.0;
/// Seconds cursor must be still before brave cats creep toward it.
const CURSOR_STILL_CREEP_TIME: f32 = 5.0;
/// Cautious cat: flee when cursor speed exceeds this.
const CAUTIOUS_FLEE_SPEED: f32 = 100.0;
/// Cautious cat: creep speed toward still cursor.
const CREEP_SPEED: f32 = 25.0;

/// Persistent state for cursor tracking between frames.
pub struct CursorState {
    pub prev_pos: Vec2,
    pub speed: f32,
    pub still_timer: f32,
}

impl CursorState {
    pub fn new() -> Self {
        Self {
            prev_pos: Vec2::ZERO,
            speed: 0.0,
            still_timer: 0.0,
        }
    }

    /// Update cursor speed and still timer. Call once per tick.
    pub fn update(&mut self, mouse_x: f32, mouse_y: f32, dt: f32) {
        let current = Vec2::new(mouse_x, mouse_y);
        let delta = current - self.prev_pos;
        self.speed = delta.length() / dt.max(0.001);
        self.prev_pos = current;

        if self.speed < CURSOR_STILL_THRESHOLD {
            self.still_timer += dt;
        } else {
            self.still_timer = 0.0;
        }
    }
}

/// Track global mouse position and update chase/flee targets.
pub fn update_mouse_pos(
    world: &mut hecs::World,
    mouse_x: f32,
    mouse_y: f32,
    cursor: &CursorState,
    rng: &mut fastrand::Rng,
) {
    let mouse = Vec2::new(mouse_x, mouse_y);
    let cursor_speed = cursor.speed;
    let cursor_still = cursor.still_timer >= CURSOR_STILL_CREEP_TIME;
    let moses_active = cursor_speed > MOSES_SPEED_THRESHOLD;

    for (_, (pos, vel, state, personality)) in world
        .query_mut::<(&Position, &mut Velocity, &mut CatState, &Personality)>()
    {
        let to_mouse = mouse - pos.0;
        let dist = to_mouse.length();

        // --- Moses Effect: fast cursor scatters everyone nearby ---
        if moses_active && dist < MOSES_RADIUS && dist > 1.0 {
            let away = -to_mouse / dist;
            let falloff = 1.0 - (dist / MOSES_RADIUS);
            let strength = (cursor_speed / MOSES_SPEED_THRESHOLD) * MOSES_STRENGTH * falloff;
            let push = away * strength.min(MOSES_MAX_PUSH);
            vel.0 += push;

            // Force non-stationary state if pushed hard
            if strength > 50.0
                && matches!(
                    state.state,
                    BehaviorState::Idle | BehaviorState::Sleeping | BehaviorState::Grooming
                )
            {
                state.state = BehaviorState::Running;
                state.timer = 0.3 + rng.f32() * 0.5;
            }
            continue;
        }

        // --- Handle cats already in cursor-related states ---
        if state.state == BehaviorState::ChasingMouse {
            if dist > 10.0 {
                let dir = to_mouse / dist;
                let speed = CHASE_SPEED * (0.7 + personality.curiosity * 0.6);
                vel.0 = dir * speed;
            } else {
                state.state = BehaviorState::Idle;
                state.timer = 0.5 + rng.f32() * 1.0;
            }
            continue;
        }

        if state.state == BehaviorState::FleeingCursor {
            // Keep fleeing away from cursor
            if dist < FLEE_RADIUS * 1.5 && dist > 1.0 {
                let away = -to_mouse / dist;
                let speed = FLEE_SPEED_MIN
                    + (FLEE_SPEED_MAX - FLEE_SPEED_MIN) * personality.skittishness;
                vel.0 = away * speed;
            }
            // Timer handles transition back (in behavior system)
            continue;
        }

        // --- Only idle/walking/grooming cats consider new cursor reactions ---
        if !matches!(
            state.state,
            BehaviorState::Idle | BehaviorState::Walking | BehaviorState::Grooming
        ) {
            continue;
        }

        // --- Personality-driven reactions ---

        // Lazy cats ignore cursor entirely
        if personality.laziness > 0.7 {
            continue;
        }

        // Skittish cats flee when cursor is nearby
        if personality.skittishness > 0.6 && dist < FLEE_RADIUS {
            let flee_chance = 0.05 * personality.skittishness;
            if rng.f32() < flee_chance {
                state.state = BehaviorState::FleeingCursor;
                state.timer = 1.0 + rng.f32() * 1.5;
                if dist > 1.0 {
                    let away = -to_mouse / dist;
                    let speed = FLEE_SPEED_MIN
                        + (FLEE_SPEED_MAX - FLEE_SPEED_MIN) * personality.skittishness;
                    vel.0 = away * speed;
                }
                continue;
            }
        }

        // Curious, non-skittish cats chase the cursor
        if personality.curiosity > 0.5 && personality.skittishness < 0.4 {
            if dist < MOUSE_NOTICE_RADIUS {
                let chance = CHASE_CHANCE_PER_TICK * (0.5 + personality.curiosity);
                if rng.f32() < chance {
                    state.state = BehaviorState::ChasingMouse;
                    state.timer = 2.0 + rng.f32() * 3.0;
                    let dir = to_mouse / dist.max(1.0);
                    let speed = CHASE_SPEED * (0.7 + personality.curiosity * 0.6);
                    vel.0 = dir * speed;
                }
            }
            continue;
        }

        // Everyone else: Cautious behavior
        // Flee when cursor is fast and close
        if cursor_speed > CAUTIOUS_FLEE_SPEED && dist < FLEE_RADIUS * 0.8 {
            let flee_chance = 0.03;
            if rng.f32() < flee_chance {
                state.state = BehaviorState::FleeingCursor;
                state.timer = 0.5 + rng.f32() * 1.0;
                if dist > 1.0 {
                    let away = -to_mouse / dist;
                    vel.0 = away * FLEE_SPEED_MIN;
                }
                continue;
            }
        }

        // Creep toward still cursor (brave cats only)
        if cursor_still && personality.curiosity > 0.4 && dist < MOUSE_NOTICE_RADIUS * 1.5 {
            let creep_chance = 0.005 * personality.curiosity;
            if rng.f32() < creep_chance {
                state.state = BehaviorState::ChasingMouse;
                state.timer = 3.0 + rng.f32() * 3.0;
                if dist > 1.0 {
                    let dir = to_mouse / dist;
                    vel.0 = dir * CREEP_SPEED;
                }
            }
        }
    }
}
