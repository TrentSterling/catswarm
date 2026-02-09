use glam::Vec2;

use crate::ecs::components::{
    BehaviorState, CatState, InteractionTarget, Position, Velocity,
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
            active: Vec::with_capacity(64),
        }
    }
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

pub fn update(
    world: &mut hecs::World,
    snapshots: &[CatSnapshot],
    grid: &SpatialHash,
    bufs: &mut InteractionBuffers,
    rng: &mut fastrand::Rng,
    dt: f32,
) {
    // Phase A: Steer cats already in ChasingCat/Playing states
    steer_active(world, bufs, rng);

    // Phase B: Pure-data read pass — separation + new interaction decisions
    phase_read(snapshots, grid, bufs, rng, dt);

    // Phase C: Apply results to the ECS world
    phase_write(world, bufs, snapshots, rng);
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
        // If the cat's behavior already changed (timer expired in behavior system),
        // just remove the InteractionTarget component.
        if !matches!(ai.state, BehaviorState::ChasingCat | BehaviorState::Playing) {
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
    for i in 0..len {
        bufs.separation[i] = Vec2::ZERO;
        bufs.cohesion_sum[i] = Vec2::ZERO;
        bufs.cohesion_count[i] = 0;
        bufs.alignment_sum[i] = Vec2::ZERO;
        bufs.alignment_count[i] = 0;
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
            if dist_sq < SEPARATION_RADIUS_SQ && dist_sq > 0.001 {
                let dist = dist_sq.sqrt();
                let overlap = SEPARATION_RADIUS - dist;
                let push = delta / dist * overlap * SEPARATION_STRENGTH;
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
