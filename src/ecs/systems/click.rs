use glam::Vec2;

use crate::click::ClickState;
use crate::ecs::components::{BehaviorState, CatState, Personality, Position, Velocity};
use crate::ecs::systems::behavior;
use crate::toy::{Boxes, YarnBalls};

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
const YARN_BAT_IMPULSE: f32 = 250.0;

/// Box attraction radius.
const BOX_ATTRACT_RADIUS: f32 = 200.0;
/// Box approach speed (curiosity-scaled).
const BOX_APPROACH_SPEED: f32 = 40.0;

/// Process click interactions: startle, treats, laser pointer, yarn balls, boxes.
pub fn update(
    world: &mut hecs::World,
    click: &ClickState,
    mouse_pos: Vec2,
    rng: &mut fastrand::Rng,
    yarn_balls: &mut YarnBalls,
    boxes: &mut Boxes,
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

    // --- Treats: attract nearby idle/walking cats ---
    if !click.treats.is_empty() {
        for (_, (pos, vel, state, personality)) in world
            .query_mut::<(&Position, &mut Velocity, &mut CatState, &Personality)>()
        {
            if !matches!(
                state.state,
                BehaviorState::Idle | BehaviorState::Walking
            ) {
                continue;
            }

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
                    state.timer = 0.5;
                }
            }
        }
    }

    // --- Double click laser pointer: frenzied chasing ---
    if click.laser_active {
        for (_, (pos, vel, state, personality)) in world
            .query_mut::<(&Position, &mut Velocity, &mut CatState, &Personality)>()
        {
            if personality.curiosity < 0.4 {
                continue;
            }
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
                state.timer = 0.5;
            }
        }
    }

    // --- Right click: always spawn a new yarn ball ---
    if click.right_clicked {
        yarn_balls.spawn(mouse_pos);
    }

    // --- Yarn balls: attract nearby cats, let them bat ---
    if yarn_balls.any_active() {
        let attract_sq = YARN_ATTRACT_RADIUS * YARN_ATTRACT_RADIUS;
        // Collect bat impulses per ball
        let mut bat_impulses: Vec<(usize, Vec2)> = Vec::new();

        for (_, (pos, vel, state, personality)) in world
            .query_mut::<(&Position, &mut Velocity, &mut CatState, &Personality)>()
        {
            if !matches!(
                state.state,
                BehaviorState::Idle | BehaviorState::Walking | BehaviorState::Running
            ) {
                continue;
            }
            if personality.energy < 0.3 {
                continue;
            }

            // Find nearest yarn ball
            let mut best_idx: Option<usize> = None;
            let mut best_dist_sq = attract_sq;
            for (i, ball) in yarn_balls.balls.iter().enumerate() {
                let dist_sq = (ball.pos - pos.0).length_squared();
                if dist_sq < best_dist_sq {
                    best_dist_sq = dist_sq;
                    best_idx = Some(i);
                }
            }

            if let Some(idx) = best_idx {
                let yarn_pos = yarn_balls.balls[idx].pos;
                let to_yarn = yarn_pos - pos.0;
                let dist = best_dist_sq.sqrt();

                if dist < 20.0 {
                    // Bat the ball
                    let bat_dir = Vec2::new(rng.f32() - 0.5, rng.f32() - 0.5).normalize_or_zero();
                    bat_impulses.push((idx, bat_dir * YARN_BAT_IMPULSE * (0.5 + personality.energy * 0.5)));
                    // Cat runs away after batting
                    let away = -bat_dir;
                    vel.0 = away * 120.0 * personality.energy;
                    state.state = BehaviorState::Running;
                    state.timer = 0.8 + rng.f32() * 0.7;
                } else {
                    // Chase toward nearest yarn ball
                    let dir = to_yarn / dist;
                    let speed = YARN_CHASE_SPEED * (0.5 + personality.energy * 0.5);
                    vel.0 = dir * speed;
                    state.state = BehaviorState::Running;
                    state.timer = 0.5;
                }
            }
        }

        // Apply bat impulses
        for (idx, impulse) in bat_impulses {
            yarn_balls.bat(idx, impulse);
        }
    }

    // --- Cardboard boxes: attract idle/walking cats, they sit inside ---
    if !boxes.boxes.is_empty() {
        let attract_sq = BOX_ATTRACT_RADIUS * BOX_ATTRACT_RADIUS;

        for (_, (pos, vel, state, personality)) in world
            .query_mut::<(&Position, &mut Velocity, &mut CatState, &Personality)>()
        {
            if !matches!(
                state.state,
                BehaviorState::Idle | BehaviorState::Walking
            ) {
                continue;
            }
            if personality.curiosity < 0.3 {
                continue;
            }

            // Find nearest box with room
            let mut best_idx: Option<usize> = None;
            let mut best_dist_sq = attract_sq;
            for (i, cbox) in boxes.boxes.iter().enumerate() {
                if cbox.occupants >= 2 {
                    continue;
                }
                let dist_sq = (cbox.pos - pos.0).length_squared();
                if dist_sq < best_dist_sq {
                    best_dist_sq = dist_sq;
                    best_idx = Some(i);
                }
            }

            if let Some(idx) = best_idx {
                let box_pos = boxes.boxes[idx].pos;
                let to_box = box_pos - pos.0;
                let dist = best_dist_sq.sqrt();

                if dist < 25.0 {
                    // Cat sits in the box (becomes idle/grooming at box position)
                    vel.0 = Vec2::ZERO;
                    state.state = BehaviorState::Idle;
                    state.timer = 5.0 + rng.f32() * 10.0; // sit for a while
                    boxes.boxes[idx].occupants = (boxes.boxes[idx].occupants + 1).min(2);
                } else {
                    // Walk toward box
                    let dir = to_box / dist;
                    let speed = BOX_APPROACH_SPEED * (0.5 + personality.curiosity * 0.5);
                    vel.0 = dir * speed;
                    state.state = BehaviorState::Walking;
                    state.timer = 0.5;
                }
            }
        }
    }
}
