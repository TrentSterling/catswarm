use glam::Vec2;

use crate::click::ClickState;
use crate::ecs::components::{BehaviorState, CatState, Personality, Position, Velocity};
use crate::ecs::systems::behavior;
use crate::toy::YarnBall;

/// Startle radius: cats within this of a click get startled.
const STARTLE_RADIUS: f32 = 100.0;
/// Flee impulse radius on click.
const CLICK_FLEE_RADIUS: f32 = 200.0;
/// Flee impulse strength.
const CLICK_FLEE_STRENGTH: f32 = 80.0;

/// Treat attraction radius.
const TREAT_ATTRACT_RADIUS: f32 = 300.0;
/// Treat approach speed (base, scaled by curiosity).
const TREAT_APPROACH_SPEED: f32 = 60.0;

/// Laser pointer chase speed multiplier.
const LASER_CHASE_SPEED: f32 = 200.0;
/// Laser jitter amplitude.
const LASER_JITTER: f32 = 40.0;

/// Yarn ball attraction radius.
const YARN_ATTRACT_RADIUS: f32 = 250.0;
/// Yarn ball chase speed.
const YARN_CHASE_SPEED: f32 = 100.0;
/// Yarn ball bat impulse when a cat reaches it.
const YARN_BAT_IMPULSE: f32 = 80.0;

/// Process click interactions: startle, treats, laser pointer, yarn ball.
pub fn update(
    world: &mut hecs::World,
    click: &ClickState,
    mouse_pos: Vec2,
    rng: &mut fastrand::Rng,
    yarn: &mut YarnBall,
) {
    // --- Left click: startle nearest cat + flee impulse ---
    if click.left_clicked {
        // Find nearest cat within startle radius
        let mut nearest_entity = None;
        let mut nearest_dist_sq = STARTLE_RADIUS * STARTLE_RADIUS;

        for (entity, pos) in world.query::<&Position>().iter() {
            let dist_sq = (pos.0 - mouse_pos).length_squared();
            if dist_sq < nearest_dist_sq {
                nearest_dist_sq = dist_sq;
                nearest_entity = Some(entity);
            }
        }

        // Startle the nearest cat
        if let Some(entity) = nearest_entity {
            if let Ok((mut state, mut vel)) =
                world.query_one_mut::<(&mut CatState, &mut Velocity)>(entity)
            {
                behavior::trigger_startle(&mut state, &mut vel, rng);
            }
        }

        // Flee impulse for all cats in wider radius
        let flee_radius_sq = CLICK_FLEE_RADIUS * CLICK_FLEE_RADIUS;
        for (_, (pos, vel, state)) in
            world.query_mut::<(&Position, &mut Velocity, &mut CatState)>()
        {
            let delta = pos.0 - mouse_pos;
            let dist_sq = delta.length_squared();
            if dist_sq < flee_radius_sq && dist_sq > 1.0 {
                let dist = dist_sq.sqrt();
                let away = delta / dist;
                let falloff = 1.0 - (dist / CLICK_FLEE_RADIUS);
                vel.0 += away * CLICK_FLEE_STRENGTH * falloff;

                // Wake up sleeping/idle cats
                if matches!(
                    state.state,
                    BehaviorState::Sleeping | BehaviorState::Idle | BehaviorState::Grooming
                ) {
                    state.state = BehaviorState::Running;
                    state.timer = 0.3 + rng.f32() * 0.5;
                }
            }
        }
    }

    // --- Right click treats: attract nearby idle/walking cats ---
    if !click.treats.is_empty() {
        for (_, (pos, vel, state, personality)) in world
            .query_mut::<(&Position, &mut Velocity, &mut CatState, &Personality)>()
        {
            // Only attract idle/walking cats
            if !matches!(
                state.state,
                BehaviorState::Idle | BehaviorState::Walking
            ) {
                continue;
            }

            // Find nearest treat
            let mut best_treat: Option<Vec2> = None;
            let mut best_dist_sq = TREAT_ATTRACT_RADIUS * TREAT_ATTRACT_RADIUS;
            for treat in &click.treats {
                let dist_sq = (treat.pos - pos.0).length_squared();
                if dist_sq < best_dist_sq {
                    best_dist_sq = dist_sq;
                    best_treat = Some(treat.pos);
                }
            }

            if let Some(treat_pos) = best_treat {
                let to_treat = treat_pos - pos.0;
                let dist = to_treat.length();
                if dist > 5.0 {
                    let dir = to_treat / dist;
                    let speed = TREAT_APPROACH_SPEED * (0.5 + personality.curiosity * 1.0);
                    vel.0 = dir * speed;
                    state.state = BehaviorState::Walking;
                    state.timer = 0.5; // refresh timer so it keeps approaching
                }
            }
        }
    }

    // --- Double click laser pointer: frenzied chasing ---
    if click.laser_active {
        for (_, (pos, vel, state, personality)) in world
            .query_mut::<(&Position, &mut Velocity, &mut CatState, &Personality)>()
        {
            // Only high-curiosity cats react to laser
            if personality.curiosity < 0.4 {
                continue;
            }

            // Only cats in interruptible states
            if !matches!(
                state.state,
                BehaviorState::Idle
                    | BehaviorState::Walking
                    | BehaviorState::Running
                    | BehaviorState::ChasingMouse
            ) {
                continue;
            }

            let to_laser = mouse_pos - pos.0;
            let dist = to_laser.length();
            if dist > 10.0 && dist < 500.0 {
                let dir = to_laser / dist;
                let jitter = Vec2::new(
                    (rng.f32() - 0.5) * LASER_JITTER,
                    (rng.f32() - 0.5) * LASER_JITTER,
                );
                let speed = LASER_CHASE_SPEED * (0.8 + personality.curiosity * 0.4);
                vel.0 = dir * speed + jitter;
                state.state = BehaviorState::ChasingMouse;
                state.timer = 0.5; // refresh frequently for jittery tracking
            }
        }
    }

    // --- Middle click: spawn/throw yarn ball ---
    if click.middle_clicked {
        if yarn.active {
            // Throw it from current position toward mouse
            let dir = (mouse_pos - yarn.pos).normalize_or_zero();
            yarn.throw(dir * 300.0);
        } else {
            yarn.spawn(mouse_pos);
        }
    }

    // --- Yarn ball: attract nearby cats, let them bat it ---
    if yarn.active {
        let yarn_pos = yarn.pos;
        let attract_sq = YARN_ATTRACT_RADIUS * YARN_ATTRACT_RADIUS;
        let mut bat_impulse = Vec2::ZERO;

        for (_, (pos, vel, state, personality)) in world
            .query_mut::<(&Position, &mut Velocity, &mut CatState, &Personality)>()
        {
            // Only idle/walking/running cats with some energy chase the ball
            if !matches!(
                state.state,
                BehaviorState::Idle | BehaviorState::Walking | BehaviorState::Running
            ) {
                continue;
            }
            if personality.energy < 0.3 {
                continue;
            }

            let to_yarn = yarn_pos - pos.0;
            let dist_sq = to_yarn.length_squared();
            if dist_sq > attract_sq || dist_sq < 1.0 {
                continue;
            }

            let dist = dist_sq.sqrt();

            // Cat is close enough to bat the ball
            if dist < 20.0 {
                let bat_dir = Vec2::new(rng.f32() - 0.5, rng.f32() - 0.5).normalize_or_zero();
                bat_impulse += bat_dir * YARN_BAT_IMPULSE * personality.energy;
                // Cat gets excited
                state.state = BehaviorState::Running;
                state.timer = 0.3 + rng.f32() * 0.5;
            } else {
                // Chase toward yarn ball
                let dir = to_yarn / dist;
                let speed = YARN_CHASE_SPEED * (0.5 + personality.energy * 0.5);
                vel.0 = dir * speed;
                state.state = BehaviorState::Running;
                state.timer = 0.5;
            }
        }

        // Apply accumulated bat impulses to the yarn ball
        if bat_impulse.length_squared() > 1.0 {
            yarn.bat(bat_impulse);
        }
    }
}
