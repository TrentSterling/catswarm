use glam::Vec2;

use crate::ecs::components::{
    BehaviorState, CatState, GiftCarrier, InteractionTarget, Position, SleepingPile, Velocity,
    Personality,
};
use crate::spatial::{CatSnapshot, SpatialHash};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Cats closer than this push each other apart.
const SEPARATION_RADIUS: f32 = 30.0;
const SEPARATION_RADIUS_SQ: f32 = SEPARATION_RADIUS * SEPARATION_RADIUS;
/// Strength of the repulsion force.
const SEPARATION_STRENGTH: f32 = 50.0;
/// Max separation velocity contribution per tick.
const MAX_SEPARATION: f32 = 30.0;

/// Flocking radius — cats within this steer together (cohesion + alignment).
const FLOCK_RADIUS: f32 = 120.0;
const FLOCK_RADIUS_SQ: f32 = FLOCK_RADIUS * FLOCK_RADIUS;
/// Cohesion strength — pull toward local center of mass.
const COHESION_STRENGTH: f32 = 8.0;
/// Max cohesion velocity contribution per tick.
const MAX_COHESION: f32 = 15.0;
/// Alignment strength — match neighbors' average heading.
const ALIGNMENT_STRENGTH: f32 = 5.0;
/// Max alignment velocity contribution per tick.
const MAX_ALIGNMENT: f32 = 12.0;

/// Social interactions happen within this range.
const INTERACTION_RADIUS: f32 = 60.0;
const INTERACTION_RADIUS_SQ: f32 = INTERACTION_RADIUS * INTERACTION_RADIUS;

/// Per-tick probability two idle cats start playing.
const PLAY_CHANCE: f32 = 0.008;
/// Per-tick probability one cat chases another.
const CHASE_CHANCE: f32 = 0.005;
/// Per-tick probability a cat pounces on another.
const POUNCE_CHANCE: f32 = 0.002;
/// Pounce leap speed.
const POUNCE_LEAP_SPEED: f32 = 350.0;
/// Per-tick probability an idle cat joins a sleeping neighbor.
const NAP_CLUSTER_CHANCE: f32 = 0.015;

/// Zoomie contagion: chance a nearby idle/walking cat catches zoomies.
const ZOOMIE_CONTAGION_CHANCE: f32 = 0.05;
/// Contagious yawn: chance a nearby idle/grooming cat yawns.
const YAWN_CONTAGION_CHANCE: f32 = 0.30;
/// Sleeping cat chance per tick to start yawning (seed for cascade).
const YAWN_SEED_CHANCE: f32 = 0.001;
/// Contagion interaction range.
const CONTAGION_RADIUS: f32 = 80.0;
const CONTAGION_RADIUS_SQ: f32 = CONTAGION_RADIUS * CONTAGION_RADIUS;
/// Parade detection: min cats walking in similar direction within range.
const PARADE_MIN_CATS: usize = 3;
/// Parade detection range.
const PARADE_RADIUS: f32 = 100.0;
const PARADE_RADIUS_SQ: f32 = PARADE_RADIUS * PARADE_RADIUS;
/// Parade follow distance behind leader.
const PARADE_FOLLOW_DIST: f32 = 40.0;
/// Parade follow speed.
const PARADE_SPEED: f32 = 45.0;

/// Sleeping pile detection radius.
const PILE_RADIUS: f32 = 40.0;
const PILE_RADIUS_SQ: f32 = PILE_RADIUS * PILE_RADIUS;
/// Min sleeping neighbors for pile membership (3+ cats total).
const PILE_MIN_NEIGHBORS: u32 = 2;
/// Wake cascade radius (wider than pile itself).
const WAKE_CASCADE_RADIUS_SQ: f32 = 80.0 * 80.0;

/// Velocity when chasing another cat.
const CAT_CHASE_SPEED: f32 = 90.0;
/// Velocity when fleeing.
const FLEE_SPEED: f32 = 110.0;
/// Velocity when playing (hovering near partner).
const PLAY_SPEED: f32 = 45.0;

/// ChasingCat gives up if target is further than this.
const CHASE_GIVE_UP_DIST: f32 = 250.0;
const CHASE_GIVE_UP_DIST_SQ: f32 = CHASE_GIVE_UP_DIST * CHASE_GIVE_UP_DIST;
/// Playing gives up if partner is further than this.
const PLAY_GIVE_UP_DIST: f32 = 100.0;
const PLAY_GIVE_UP_DIST_SQ: f32 = PLAY_GIVE_UP_DIST * PLAY_GIVE_UP_DIST;

// ---------------------------------------------------------------------------
// Command types
// ---------------------------------------------------------------------------

enum InteractionCmd {
    StartPlay {
        entity_a: hecs::Entity,
        entity_b: hecs::Entity,
    },
    StartChase {
        chaser: hecs::Entity,
        target: hecs::Entity,
    },
    Flee {
        entity: hecs::Entity,
        away_from: Vec2,
    },
    JoinNap {
        entity: hecs::Entity,
    },
    CatchZoomies {
        entity: hecs::Entity,
    },
    ContagiousYawn {
        entity: hecs::Entity,
    },
    SeedYawn {
        entity: hecs::Entity,
    },
    StartPounce {
        pouncer: hecs::Entity,
        target: hecs::Entity,
    },
}

struct ActiveInteraction {
    entity: hecs::Entity,
    state: BehaviorState,
    pos: Vec2,
    target: hecs::Entity,
    target_pos: Option<Vec2>,
}

// ---------------------------------------------------------------------------
// Buffers — pre-allocated, reused each tick
// ---------------------------------------------------------------------------

pub struct InteractionBuffers {
    commands: Vec<InteractionCmd>,
    separation: Vec<Vec2>,
    cohesion_sum: Vec<Vec2>,
    cohesion_count: Vec<u32>,
    alignment_sum: Vec<Vec2>,
    alignment_count: Vec<u32>,
    parade_dir_sum: Vec<Vec2>,
    parade_count: Vec<u32>,
    parade_follow_pos: Vec<Vec2>,
    parade_follow_dist_sq: Vec<f32>,
    sleeping_neighbor_count: Vec<u32>,
    active: Vec<ActiveInteraction>,
}

impl InteractionBuffers {
    pub fn new(capacity: usize) -> Self {
        Self {
            commands: Vec::with_capacity(64),
            separation: vec![Vec2::ZERO; capacity],
            cohesion_sum: vec![Vec2::ZERO; capacity],
            cohesion_count: vec![0; capacity],
            alignment_sum: vec![Vec2::ZERO; capacity],
            alignment_count: vec![0; capacity],
            parade_dir_sum: vec![Vec2::ZERO; capacity],
            parade_count: vec![0; capacity],
            parade_follow_pos: vec![Vec2::ZERO; capacity],
            parade_follow_dist_sq: vec![f32::MAX; capacity],
            sleeping_neighbor_count: vec![0; capacity],
            active: Vec::with_capacity(64),
        }
    }
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Gift spawn chance per tick (for idle curious cats).
const GIFT_SPAWN_CHANCE: f32 = 0.0003;
/// Gift carry speed.
const GIFT_CARRY_SPEED: f32 = 55.0;
/// Gift drop distance to cursor.
const GIFT_DROP_DIST: f32 = 60.0;

pub fn update(
    world: &mut hecs::World,
    snapshots: &[CatSnapshot],
    grid: &SpatialHash,
    bufs: &mut InteractionBuffers,
    rng: &mut fastrand::Rng,
    dt: f32,
    mouse_pos: Vec2,
) {
    // Phase A: Steer cats already in ChasingCat/Playing states
    steer_active(world, bufs, rng);

    // Phase B: Pure-data read pass — separation + new interaction decisions
    phase_read(snapshots, grid, bufs, rng, dt);

    // Phase C: Apply results to the ECS world
    phase_write(world, bufs, snapshots, rng);

    // Phase D: Sleeping pile management
    phase_sleeping_piles(world, snapshots, bufs, rng);

    // Phase E: Gift giving — cats carry gifts to cursor
    phase_gifts(world, rng, dt, mouse_pos);
}

fn phase_gifts(
    world: &mut hecs::World,
    rng: &mut fastrand::Rng,
    dt: f32,
    mouse_pos: Vec2,
) {
    // Steer existing gift carriers toward cursor
    let mut delivered: Vec<(hecs::Entity, Vec2)> = Vec::new();
    let mut expired: Vec<hecs::Entity> = Vec::new();

    for (entity, (pos, vel, gift)) in
        world.query_mut::<(&Position, &mut Velocity, &mut GiftCarrier)>()
    {
        gift.timer -= dt;
        if gift.timer <= 0.0 {
            expired.push(entity);
            continue;
        }

        let to_cursor = mouse_pos - pos.0;
        let dist = to_cursor.length();

        if dist < GIFT_DROP_DIST {
            // Close enough — drop the gift!
            delivered.push((entity, pos.0));
        } else {
            // Walk toward cursor
            let dir = to_cursor / dist;
            vel.0 = dir * GIFT_CARRY_SPEED;
        }
    }

    // Remove gift component from delivered/expired cats
    for (entity, _pos) in &delivered {
        let _ = world.remove_one::<GiftCarrier>(*entity);
        if let Ok(mut state) = world.get::<&mut CatState>(*entity) {
            state.state = BehaviorState::Idle;
            state.timer = 1.0 + rng.f32() * 2.0;
        }
        if let Ok(mut vel) = world.get::<&mut Velocity>(*entity) {
            vel.0 = Vec2::ZERO;
        }
    }
    for entity in expired {
        let _ = world.remove_one::<GiftCarrier>(entity);
    }

    // Spawn new gift carriers (rare, from idle curious cats)
    // Only if there aren't too many already
    let carrier_count = world.query::<&GiftCarrier>().iter().count();
    if carrier_count >= 3 {
        return;
    }

    let mut new_carrier: Option<hecs::Entity> = None;
    for (entity, (state, personality, _gift)) in
        world.query::<(&CatState, &Personality, Option<&GiftCarrier>)>().iter()
    {
        if _gift.is_some() {
            continue;
        }
        if !matches!(state.state, BehaviorState::Idle | BehaviorState::Walking) {
            continue;
        }
        if personality.curiosity < 0.6 {
            continue;
        }
        if rng.f32() < GIFT_SPAWN_CHANCE {
            new_carrier = Some(entity);
            break;
        }
    }

    if let Some(entity) = new_carrier {
        let _ = world.insert_one(entity, GiftCarrier { timer: 15.0 });
        if let Ok(mut state) = world.get::<&mut CatState>(entity) {
            state.state = BehaviorState::Walking;
            state.timer = 15.0;
        }
    }
}

// ---------------------------------------------------------------------------
// Phase A: Steer already-interacting cats
// ---------------------------------------------------------------------------

fn steer_active(
    world: &mut hecs::World,
    bufs: &mut InteractionBuffers,
    rng: &mut fastrand::Rng,
) {
    bufs.active.clear();

    // Pass 1 (read): Collect currently interacting cats
    for (entity, (pos, cat_state, target)) in
        world.query::<(&Position, &CatState, &InteractionTarget)>().iter()
    {
        bufs.active.push(ActiveInteraction {
            entity,
            state: cat_state.state,
            pos: pos.0,
            target: target.0,
            target_pos: None,
        });
    }

    // Resolve target positions
    for ai in bufs.active.iter_mut() {
        if let Ok(target_pos) = world.get::<&Position>(ai.target) {
            ai.target_pos = Some(target_pos.0);
        }
    }

    // Pass 2 (write): Update velocities or clean up
    for ai in bufs.active.iter() {
        // Pouncing: still crouching, keep waiting for timer to expire
        if ai.state == BehaviorState::Pouncing {
            continue;
        }

        // If the cat's behavior already changed (timer expired in behavior system),
        // just remove the InteractionTarget component.
        if !matches!(ai.state, BehaviorState::ChasingCat | BehaviorState::Playing) {
            // Was this cat pouncing? (behavior.rs set it to Idle on timer expiry)
            // If it has an InteractionTarget and just left Pouncing, do the leap.
            if let Some(target_pos) = ai.target_pos {
                let to_target = target_pos - ai.pos;
                let dist = to_target.length();
                if dist > 1.0 && dist < 200.0 {
                    let dir = to_target / dist;
                    if let Ok(mut vel) = world.get::<&mut Velocity>(ai.entity) {
                        vel.0 = dir * POUNCE_LEAP_SPEED;
                    }
                    if let Ok(mut state) = world.get::<&mut CatState>(ai.entity) {
                        state.state = BehaviorState::Running;
                        state.timer = 0.3;
                    }
                    // Startle the target
                    if let Ok((mut target_state, mut target_vel)) =
                        world.query_one_mut::<(&mut CatState, &mut Velocity)>(ai.target)
                    {
                        crate::ecs::systems::behavior::trigger_startle(&mut target_state, &mut target_vel, rng);
                    }
                }
            }
            let _ = world.remove_one::<InteractionTarget>(ai.entity);
            continue;
        }

        let target_pos = match ai.target_pos {
            Some(p) => p,
            None => {
                // Target entity gone — go idle
                give_up(world, ai.entity, rng);
                continue;
            }
        };

        let to_target = target_pos - ai.pos;
        let dist_sq = to_target.length_squared();

        match ai.state {
            BehaviorState::ChasingCat => {
                if dist_sq > CHASE_GIVE_UP_DIST_SQ {
                    give_up(world, ai.entity, rng);
                } else if dist_sq > 1.0 {
                    let dir = to_target / dist_sq.sqrt();
                    if let Ok(mut vel) = world.get::<&mut Velocity>(ai.entity) {
                        vel.0 = dir * CAT_CHASE_SPEED;
                    }
                }
            }
            BehaviorState::Playing => {
                if dist_sq > PLAY_GIVE_UP_DIST_SQ {
                    give_up(world, ai.entity, rng);
                } else {
                    // Jitter around partner
                    let jitter = Vec2::new(
                        rng.f32() * 2.0 - 1.0,
                        rng.f32() * 2.0 - 1.0,
                    );
                    let dir = if dist_sq > 1.0 {
                        to_target / dist_sq.sqrt()
                    } else {
                        jitter.normalize_or_zero()
                    };
                    if let Ok(mut vel) = world.get::<&mut Velocity>(ai.entity) {
                        vel.0 = (dir + jitter * 0.5) * PLAY_SPEED;
                    }
                }
            }
            _ => {}
        }
    }
}

fn give_up(world: &mut hecs::World, entity: hecs::Entity, rng: &mut fastrand::Rng) {
    let _ = world.remove_one::<InteractionTarget>(entity);
    if let Ok(mut state) = world.get::<&mut CatState>(entity) {
        state.state = BehaviorState::Idle;
        state.timer = 0.5 + rng.f32() * 1.5;
    }
    if let Ok(mut vel) = world.get::<&mut Velocity>(entity) {
        vel.0 = Vec2::ZERO;
    }
}

// ---------------------------------------------------------------------------
// Phase B: Read-only pass over snapshots (no world borrow)
// ---------------------------------------------------------------------------

fn phase_read(
    snapshots: &[CatSnapshot],
    grid: &SpatialHash,
    bufs: &mut InteractionBuffers,
    rng: &mut fastrand::Rng,
    _dt: f32,
) {
    bufs.commands.clear();

    // Ensure buffers are big enough and zeroed
    let len = snapshots.len();
    bufs.separation.resize(len, Vec2::ZERO);
    bufs.cohesion_sum.resize(len, Vec2::ZERO);
    bufs.cohesion_count.resize(len, 0);
    bufs.alignment_sum.resize(len, Vec2::ZERO);
    bufs.alignment_count.resize(len, 0);
    bufs.parade_dir_sum.resize(len, Vec2::ZERO);
    bufs.parade_count.resize(len, 0);
    bufs.parade_follow_pos.resize(len, Vec2::ZERO);
    bufs.parade_follow_dist_sq.resize(len, f32::MAX);
    bufs.sleeping_neighbor_count.resize(len, 0);
    for i in 0..len {
        bufs.separation[i] = Vec2::ZERO;
        bufs.cohesion_sum[i] = Vec2::ZERO;
        bufs.cohesion_count[i] = 0;
        bufs.alignment_sum[i] = Vec2::ZERO;
        bufs.alignment_count[i] = 0;
        bufs.parade_dir_sum[i] = Vec2::ZERO;
        bufs.parade_count[i] = 0;
        bufs.parade_follow_pos[i] = Vec2::ZERO;
        bufs.parade_follow_dist_sq[i] = f32::MAX;
        bufs.sleeping_neighbor_count[i] = 0;
    }

    let count = snapshots.len();
    for my_idx in 0..count {
        let me = &snapshots[my_idx];

        // Skip cats already in interaction states
        let my_interactable = matches!(
            me.state,
            BehaviorState::Idle
                | BehaviorState::Walking
                | BehaviorState::Grooming
                | BehaviorState::Sleeping
        );

        grid.query_neighbors(me.pos, |neighbor_idx| {
            let ni = neighbor_idx as usize;
            if ni == my_idx || ni >= count {
                return;
            }
            let them = &snapshots[ni];
            let delta = me.pos - them.pos;
            let dist_sq = delta.length_squared();

            // --- Separation (always applies, close range) ---
            // Bigger cats push harder (1.0 at size=1.0, 1.3 at size=1.4)
            if dist_sq < SEPARATION_RADIUS_SQ && dist_sq > 0.001 {
                let dist = dist_sq.sqrt();
                let overlap = SEPARATION_RADIUS - dist;
                let size_factor = 0.5 + them.size * 0.5;
                let push = delta / dist * overlap * SEPARATION_STRENGTH * size_factor;
                bufs.separation[my_idx] += push;
            }

            // --- Flocking: cohesion + alignment (wider range, mobile cats only) ---
            if dist_sq < FLOCK_RADIUS_SQ {
                let both_mobile = matches!(
                    me.state,
                    BehaviorState::Idle | BehaviorState::Walking | BehaviorState::Running
                ) && matches!(
                    them.state,
                    BehaviorState::Idle | BehaviorState::Walking | BehaviorState::Running
                );
                if both_mobile {
                    // Cohesion: accumulate neighbor positions
                    bufs.cohesion_sum[my_idx] += them.pos;
                    bufs.cohesion_count[my_idx] += 1;

                    // Alignment: accumulate neighbor velocities
                    bufs.alignment_sum[my_idx] += them.vel;
                    bufs.alignment_count[my_idx] += 1;
                }
            }

            // --- Parade detection (walking/running cats with aligned velocity) ---
            if dist_sq < PARADE_RADIUS_SQ
                && matches!(me.state, BehaviorState::Walking | BehaviorState::Running | BehaviorState::Parading)
                && matches!(them.state, BehaviorState::Walking | BehaviorState::Running | BehaviorState::Parading)
                && me.vel.length_squared() > 1.0
                && them.vel.length_squared() > 1.0
            {
                let my_dir = me.vel.normalize();
                let dot = my_dir.dot(them.vel.normalize());
                if dot > 0.7 {
                    bufs.parade_dir_sum[my_idx] += them.vel.normalize();
                    bufs.parade_count[my_idx] += 1;

                    // Follow-the-leader: track nearest aligned cat ahead of me
                    let rel_pos = them.pos - me.pos;
                    let ahead = rel_pos.dot(my_dir);
                    if ahead > 5.0 && dist_sq < bufs.parade_follow_dist_sq[my_idx] {
                        bufs.parade_follow_dist_sq[my_idx] = dist_sq;
                        bufs.parade_follow_pos[my_idx] = them.pos;
                    }
                }
            }

            // --- Sleeping pile detection ---
            if dist_sq < PILE_RADIUS_SQ
                && me.state == BehaviorState::Sleeping
                && them.state == BehaviorState::Sleeping
            {
                bufs.sleeping_neighbor_count[my_idx] += 1;
            }

            // --- Social interactions (only process each pair once) ---
            if my_idx >= ni {
                return; // deduplicate
            }
            if dist_sq > INTERACTION_RADIUS_SQ {
                return;
            }
            if !my_interactable {
                return;
            }

            let their_interactable = matches!(
                them.state,
                BehaviorState::Idle
                    | BehaviorState::Walking
                    | BehaviorState::Grooming
                    | BehaviorState::Sleeping
            );

            // Play: both idle/walking
            if matches!(me.state, BehaviorState::Idle | BehaviorState::Walking)
                && matches!(them.state, BehaviorState::Idle | BehaviorState::Walking)
            {
                let chance = PLAY_CHANCE
                    * (1.0 - me.personality.skittishness)
                    * (1.0 - them.personality.skittishness);
                if rng.f32() < chance {
                    bufs.commands.push(InteractionCmd::StartPlay {
                        entity_a: me.entity,
                        entity_b: them.entity,
                    });
                    return;
                }
            }

            // Chase: cat A idle/walking, cat B walking/running
            if matches!(me.state, BehaviorState::Idle | BehaviorState::Walking)
                && matches!(them.state, BehaviorState::Walking | BehaviorState::Running)
            {
                let chance = CHASE_CHANCE * me.personality.curiosity * me.personality.energy;
                if rng.f32() < chance {
                    bufs.commands.push(InteractionCmd::StartChase {
                        chaser: me.entity,
                        target: them.entity,
                    });
                    // If target is skittish, they flee
                    if them.personality.skittishness > 0.5 && rng.f32() < them.personality.skittishness {
                        bufs.commands.push(InteractionCmd::Flee {
                            entity: them.entity,
                            away_from: me.pos,
                        });
                    }
                    return;
                }
            }

            // Symmetric chase: cat B chases cat A
            if matches!(them.state, BehaviorState::Idle | BehaviorState::Walking)
                && matches!(me.state, BehaviorState::Walking | BehaviorState::Running)
                && their_interactable
            {
                let chance = CHASE_CHANCE * them.personality.curiosity * them.personality.energy;
                if rng.f32() < chance {
                    bufs.commands.push(InteractionCmd::StartChase {
                        chaser: them.entity,
                        target: me.entity,
                    });
                    if me.personality.skittishness > 0.5 && rng.f32() < me.personality.skittishness {
                        bufs.commands.push(InteractionCmd::Flee {
                            entity: me.entity,
                            away_from: them.pos,
                        });
                    }
                    return;
                }
            }

            // Pounce: energetic cat pounces on idle/grooming cat
            if matches!(me.state, BehaviorState::Idle | BehaviorState::Walking)
                && matches!(them.state, BehaviorState::Idle | BehaviorState::Walking | BehaviorState::Grooming)
                && me.personality.energy > 0.5
                && me.personality.curiosity > 0.3
            {
                let chance = POUNCE_CHANCE * me.personality.energy;
                if rng.f32() < chance {
                    bufs.commands.push(InteractionCmd::StartPounce {
                        pouncer: me.entity,
                        target: them.entity,
                    });
                    return;
                }
            }

            // Nap cluster: one sleeping, other idle/grooming
            if me.state == BehaviorState::Sleeping
                && matches!(them.state, BehaviorState::Idle | BehaviorState::Grooming)
            {
                let chance = NAP_CLUSTER_CHANCE * them.personality.laziness;
                if rng.f32() < chance {
                    bufs.commands.push(InteractionCmd::JoinNap {
                        entity: them.entity,
                    });
                    return;
                }
            }
            if them.state == BehaviorState::Sleeping
                && matches!(me.state, BehaviorState::Idle | BehaviorState::Grooming)
            {
                let chance = NAP_CLUSTER_CHANCE * me.personality.laziness;
                if rng.f32() < chance {
                    bufs.commands.push(InteractionCmd::JoinNap {
                        entity: me.entity,
                    });
                    return;
                }
            }

            // --- Contagion interactions (use wider radius) ---
            if dist_sq > CONTAGION_RADIUS_SQ {
                return;
            }

            // Zoomie contagion: zooming cat near idle/walking cat
            if me.state == BehaviorState::Zoomies
                && matches!(them.state, BehaviorState::Idle | BehaviorState::Walking)
            {
                if rng.f32() < ZOOMIE_CONTAGION_CHANCE {
                    bufs.commands.push(InteractionCmd::CatchZoomies {
                        entity: them.entity,
                    });
                }
            }
            if them.state == BehaviorState::Zoomies
                && matches!(me.state, BehaviorState::Idle | BehaviorState::Walking)
            {
                if rng.f32() < ZOOMIE_CONTAGION_CHANCE {
                    bufs.commands.push(InteractionCmd::CatchZoomies {
                        entity: me.entity,
                    });
                }
            }

            // Contagious yawn: yawning cat near idle/grooming cat
            if me.state == BehaviorState::Yawning
                && matches!(them.state, BehaviorState::Idle | BehaviorState::Grooming)
            {
                if rng.f32() < YAWN_CONTAGION_CHANCE {
                    bufs.commands.push(InteractionCmd::ContagiousYawn {
                        entity: them.entity,
                    });
                }
            }
            if them.state == BehaviorState::Yawning
                && matches!(me.state, BehaviorState::Idle | BehaviorState::Grooming)
            {
                if rng.f32() < YAWN_CONTAGION_CHANCE {
                    bufs.commands.push(InteractionCmd::ContagiousYawn {
                        entity: me.entity,
                    });
                }
            }
        });

        // Seed yawns: sleeping cats occasionally start yawning
        if me.state == BehaviorState::Sleeping && rng.f32() < YAWN_SEED_CHANCE {
            bufs.commands.push(InteractionCmd::SeedYawn {
                entity: me.entity,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Phase C: Apply commands and separation to the ECS world
// ---------------------------------------------------------------------------

fn phase_write(
    world: &mut hecs::World,
    bufs: &mut InteractionBuffers,
    snapshots: &[CatSnapshot],
    rng: &mut fastrand::Rng,
) {
    // Apply separation + cohesion + alignment velocities
    for (idx, snap) in snapshots.iter().enumerate() {
        let mut total_force = Vec2::ZERO;

        // Separation
        let sep = bufs.separation[idx];
        if sep.length_squared() > 0.01 {
            total_force += clamp_length(sep, MAX_SEPARATION);
        }

        // Cohesion: steer toward center of nearby cats
        if bufs.cohesion_count[idx] > 0 {
            let center = bufs.cohesion_sum[idx] / bufs.cohesion_count[idx] as f32;
            let to_center = center - snap.pos;
            if to_center.length_squared() > 1.0 {
                let cohesion = to_center.normalize() * COHESION_STRENGTH;
                total_force += clamp_length(cohesion, MAX_COHESION);
            }
        }

        // Alignment: steer toward average heading of neighbors
        if bufs.alignment_count[idx] > 0 {
            let avg_vel = bufs.alignment_sum[idx] / bufs.alignment_count[idx] as f32;
            if avg_vel.length_squared() > 1.0 {
                let alignment = (avg_vel.normalize() * ALIGNMENT_STRENGTH) - snap.vel.normalize_or_zero() * ALIGNMENT_STRENGTH;
                total_force += clamp_length(alignment, MAX_ALIGNMENT);
            }
        }

        if total_force.length_squared() < 0.01 {
            continue;
        }
        if let Ok(mut vel) = world.get::<&mut Velocity>(snap.entity) {
            vel.0 += total_force;
        }
    }

    // Apply parade behavior: follow-the-leader with spacing
    for (idx, snap) in snapshots.iter().enumerate() {
        if bufs.parade_count[idx] < (PARADE_MIN_CATS - 1) as u32 {
            continue;
        }
        let avg_dir = bufs.parade_dir_sum[idx] / bufs.parade_count[idx] as f32;
        if avg_dir.length_squared() < 0.01 {
            continue;
        }
        let parade_dir = avg_dir.normalize();

        if bufs.parade_follow_dist_sq[idx] < f32::MAX {
            // Follower: steer toward a point PARADE_FOLLOW_DIST behind the leader
            let leader_pos = bufs.parade_follow_pos[idx];
            let target = leader_pos - parade_dir * PARADE_FOLLOW_DIST;
            let to_target = target - snap.pos;
            let follow_vel = if to_target.length_squared() > 1.0 {
                to_target.normalize() * PARADE_SPEED
            } else {
                parade_dir * PARADE_SPEED
            };
            if let Ok(mut vel) = world.get::<&mut Velocity>(snap.entity) {
                vel.0 = vel.0 * 0.5 + follow_vel * 0.5;
            }
        } else {
            // Leader (no one ahead): just align to parade direction
            let parade_vel = parade_dir * PARADE_SPEED;
            if let Ok(mut vel) = world.get::<&mut Velocity>(snap.entity) {
                vel.0 = vel.0 * 0.7 + parade_vel * 0.3;
            }
        }

        if let Ok(mut state) = world.get::<&mut CatState>(snap.entity) {
            if matches!(
                state.state,
                BehaviorState::Walking | BehaviorState::Running | BehaviorState::Parading
            ) {
                state.state = BehaviorState::Parading;
                state.timer = 0.5; // refresh each tick while aligned
            }
        }
    }

    // Apply interaction commands
    for cmd in bufs.commands.drain(..) {
        match cmd {
            InteractionCmd::StartPlay { entity_a, entity_b } => {
                if !can_start_interaction(world, entity_a)
                    || !can_start_interaction(world, entity_b)
                {
                    continue;
                }
                let timer = 2.0 + rng.f32() * 3.0;
                set_interaction_state(world, entity_a, BehaviorState::Playing, timer, entity_b);
                set_interaction_state(world, entity_b, BehaviorState::Playing, timer, entity_a);
            }
            InteractionCmd::StartChase { chaser, target } => {
                if !can_start_interaction(world, chaser) {
                    continue;
                }
                let timer = 2.0 + rng.f32() * 4.0;
                set_interaction_state(world, chaser, BehaviorState::ChasingCat, timer, target);
                // Steer toward target immediately
                if let (Ok(chaser_pos), Ok(target_pos)) = (
                    world.get::<&Position>(chaser),
                    world.get::<&Position>(target),
                ) {
                    let dir = (target_pos.0 - chaser_pos.0).normalize_or_zero();
                    drop(chaser_pos);
                    drop(target_pos);
                    if let Ok(mut vel) = world.get::<&mut Velocity>(chaser) {
                        vel.0 = dir * CAT_CHASE_SPEED;
                    }
                }
            }
            InteractionCmd::Flee { entity, away_from } => {
                if !can_start_interaction(world, entity) {
                    continue;
                }
                if let Ok(pos) = world.get::<&Position>(entity) {
                    let dir = (pos.0 - away_from).normalize_or_zero();
                    drop(pos);
                    if let Ok(mut vel) = world.get::<&mut Velocity>(entity) {
                        vel.0 = dir * FLEE_SPEED;
                    }
                }
                if let Ok(mut state) = world.get::<&mut CatState>(entity) {
                    state.state = BehaviorState::Running;
                    state.timer = 1.0 + rng.f32() * 1.5;
                }
            }
            InteractionCmd::JoinNap { entity } => {
                if !can_start_interaction(world, entity) {
                    continue;
                }
                if let Ok(mut state) = world.get::<&mut CatState>(entity) {
                    state.state = BehaviorState::Sleeping;
                    state.timer = 3.0 + rng.f32() * 5.0;
                }
                if let Ok(mut vel) = world.get::<&mut Velocity>(entity) {
                    vel.0 = Vec2::ZERO;
                }
            }
            InteractionCmd::CatchZoomies { entity } => {
                if !can_start_interaction(world, entity) {
                    continue;
                }
                if let Ok(mut state) = world.get::<&mut CatState>(entity) {
                    state.state = BehaviorState::Zoomies;
                    state.timer = 1.0 + rng.f32() * 1.0;
                }
                if let Ok(mut vel) = world.get::<&mut Velocity>(entity) {
                    let angle = rng.f32() * std::f32::consts::TAU;
                    vel.0 = Vec2::new(angle.cos(), angle.sin()) * 300.0;
                }
            }
            InteractionCmd::ContagiousYawn { entity } => {
                if !can_start_interaction(world, entity) {
                    continue;
                }
                if let Ok(mut state) = world.get::<&mut CatState>(entity) {
                    state.state = BehaviorState::Yawning;
                    state.timer = 1.0;
                }
                if let Ok(mut vel) = world.get::<&mut Velocity>(entity) {
                    vel.0 = Vec2::ZERO;
                }
            }
            InteractionCmd::StartPounce { pouncer, target } => {
                if !can_start_interaction(world, pouncer) {
                    continue;
                }
                // Wind-up: pouncer crouches for 0.4s (Pouncing state)
                if let Ok(mut state) = world.get::<&mut CatState>(pouncer) {
                    state.state = BehaviorState::Pouncing;
                    state.timer = 0.4;
                }
                if let Ok(mut vel) = world.get::<&mut Velocity>(pouncer) {
                    vel.0 = Vec2::ZERO; // crouch still
                }
                // Store target for the leap (uses InteractionTarget)
                let _ = world.insert_one(pouncer, InteractionTarget(target));
            }
            InteractionCmd::SeedYawn { entity } => {
                // Sleeping cat starts yawning (seed for cascade)
                if let Ok(state) = world.get::<&CatState>(entity) {
                    if state.state != BehaviorState::Sleeping {
                        continue;
                    }
                }
                if let Ok(mut state) = world.get::<&mut CatState>(entity) {
                    state.state = BehaviorState::Yawning;
                    state.timer = 1.0;
                }
            }
        }
    }
}

/// Check if a cat is in a state that allows starting a new interaction.
fn can_start_interaction(world: &hecs::World, entity: hecs::Entity) -> bool {
    if let Ok(state) = world.get::<&CatState>(entity) {
        matches!(
            state.state,
            BehaviorState::Idle
                | BehaviorState::Walking
                | BehaviorState::Grooming
                | BehaviorState::Sleeping
        )
    } else {
        false
    }
}

fn set_interaction_state(
    world: &mut hecs::World,
    entity: hecs::Entity,
    behavior: BehaviorState,
    timer: f32,
    target: hecs::Entity,
) {
    if let Ok(mut state) = world.get::<&mut CatState>(entity) {
        state.state = behavior;
        state.timer = timer;
    }
    let _ = world.insert_one(entity, InteractionTarget(target));
}

fn clamp_length(v: Vec2, max_len: f32) -> Vec2 {
    let len_sq = v.length_squared();
    if len_sq > max_len * max_len {
        v / len_sq.sqrt() * max_len
    } else {
        v
    }
}

// ---------------------------------------------------------------------------
// Phase D: Sleeping pile detection and wake cascade
// ---------------------------------------------------------------------------

fn phase_sleeping_piles(
    world: &mut hecs::World,
    snapshots: &[CatSnapshot],
    bufs: &InteractionBuffers,
    rng: &mut fastrand::Rng,
) {
    // Step 1: Wake cascade — find pile members that are no longer sleeping
    // (woken by behavior transitions, clicks, mouse interactions, etc.)
    let mut woken_positions: Vec<Vec2> = Vec::new();
    for (_, (pos, state, _pile)) in
        world.query::<(&Position, &CatState, &SleepingPile)>().iter()
    {
        if state.state != BehaviorState::Sleeping {
            woken_positions.push(pos.0);
        }
    }

    // Find sleeping cats near woken pile members and wake them
    if !woken_positions.is_empty() {
        let mut to_wake: Vec<hecs::Entity> = Vec::new();
        for (entity, (pos, state)) in world.query::<(&Position, &CatState)>().iter() {
            if state.state != BehaviorState::Sleeping {
                continue;
            }
            for woken_pos in &woken_positions {
                let dist_sq = (pos.0 - *woken_pos).length_squared();
                if dist_sq < WAKE_CASCADE_RADIUS_SQ {
                    to_wake.push(entity);
                    break;
                }
            }
        }

        for entity in to_wake {
            if let Ok((state, vel)) =
                world.query_one_mut::<(&mut CatState, &mut Velocity)>(entity)
            {
                if state.state == BehaviorState::Sleeping {
                    // Gentle startle — woken from pile
                    state.state = BehaviorState::Startled;
                    state.timer = 0.3;
                    vel.0.y -= 150.0;
                    vel.0.x += (rng.f32() - 0.5) * 80.0;
                }
            }
        }
    }

    // Step 2: Update pile membership from sleeping_neighbor_count
    for (idx, snap) in snapshots.iter().enumerate() {
        if idx >= bufs.sleeping_neighbor_count.len() {
            break;
        }
        if snap.state == BehaviorState::Sleeping
            && bufs.sleeping_neighbor_count[idx] >= PILE_MIN_NEIGHBORS
        {
            // Part of a pile — add component if missing
            if world.get::<&SleepingPile>(snap.entity).is_err() {
                let _ = world.insert_one(
                    snap.entity,
                    SleepingPile {
                        breathing_offset: rng.f32() * std::f32::consts::TAU,
                    },
                );
            }
        } else {
            // Not in a pile — remove component if present
            let _ = world.remove_one::<SleepingPile>(snap.entity);
        }
    }
}
