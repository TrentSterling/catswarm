use glam::Vec2;

use crate::ecs::components::BehaviorState;
use crate::render::instance::CatInstance;

/// Maximum concurrent particles.
const MAX_PARTICLES: usize = 2048;

/// A single emotion particle floating above a cat.
#[derive(Debug, Clone, Copy)]
struct Particle {
    pos: Vec2,
    vel: Vec2,
    lifetime: f32,
    max_lifetime: f32,
    color: u32,
    size: f32,
    /// GPU frame index: 3=circle, 4=heart, 5=star, 6=Z-letter
    frame: u32,
}

/// Particle system that manages emotion particles above cats.
pub struct ParticleSystem {
    particles: Vec<Particle>,
    pub enabled: bool,
}

impl ParticleSystem {
    pub fn new() -> Self {
        Self {
            particles: Vec::with_capacity(MAX_PARTICLES),
            enabled: true,
        }
    }

    /// Spawn particles based on cat behavior states.
    /// `cats` is a slice of (position, behavior_state, size).
    pub fn spawn_from_behaviors(
        &mut self,
        cats: &[(Vec2, BehaviorState, f32)],
        rng: &mut fastrand::Rng,
        dt: f32,
    ) {
        for &(pos, state, cat_size) in cats {
            let spawn_info = match state {
                BehaviorState::Sleeping => {
                    // Zzz particles — low rate, float up slowly
                    if rng.f32() < 0.8 * dt {
                        Some(ParticleSpawn {
                            color: 0x6688CCCC, // soft blue, semi-transparent
                            frame: 6,          // Z shape
                            size: 0.2 + rng.f32() * 0.15,
                            vel: Vec2::new(rng.f32() * 20.0 - 10.0, -30.0 - rng.f32() * 20.0),
                            lifetime: 1.5 + rng.f32() * 1.0,
                        })
                    } else {
                        None
                    }
                }
                BehaviorState::Zoomies => {
                    // Speed sparks — high rate, scatter outward
                    if rng.f32() < 8.0 * dt {
                        let angle = rng.f32() * std::f32::consts::TAU;
                        Some(ParticleSpawn {
                            color: 0xFFAA33DD, // orange-yellow
                            frame: 5,          // star
                            size: 0.12 + rng.f32() * 0.1,
                            vel: Vec2::new(angle.cos() * 80.0, angle.sin() * 80.0),
                            lifetime: 0.3 + rng.f32() * 0.3,
                        })
                    } else {
                        None
                    }
                }
                BehaviorState::Startled => {
                    // Exclamation burst — quick burst outward
                    if rng.f32() < 12.0 * dt {
                        let angle = rng.f32() * std::f32::consts::TAU;
                        Some(ParticleSpawn {
                            color: 0xFFFF44EE, // bright yellow
                            frame: 5,          // star
                            size: 0.15 + rng.f32() * 0.1,
                            vel: Vec2::new(angle.cos() * 120.0, angle.sin() * 120.0 - 40.0),
                            lifetime: 0.2 + rng.f32() * 0.2,
                        })
                    } else {
                        None
                    }
                }
                BehaviorState::ChasingMouse | BehaviorState::ChasingCat => {
                    // Hearts — float upward
                    if rng.f32() < 1.5 * dt {
                        Some(ParticleSpawn {
                            color: 0xFF6699CC, // pink
                            frame: 4,          // heart
                            size: 0.2 + rng.f32() * 0.1,
                            vel: Vec2::new(rng.f32() * 30.0 - 15.0, -40.0 - rng.f32() * 20.0),
                            lifetime: 1.0 + rng.f32() * 0.5,
                        })
                    } else {
                        None
                    }
                }
                BehaviorState::Playing => {
                    // Colorful sparkles
                    if rng.f32() < 3.0 * dt {
                        let colors = [0xFF88FFCC, 0xFFFF88CC, 0x88FFFFcc, 0xFFBB44CC];
                        Some(ParticleSpawn {
                            color: colors[rng.usize(0..colors.len())],
                            frame: 5, // star
                            size: 0.12 + rng.f32() * 0.08,
                            vel: Vec2::new(
                                rng.f32() * 60.0 - 30.0,
                                -50.0 - rng.f32() * 30.0,
                            ),
                            lifetime: 0.5 + rng.f32() * 0.5,
                        })
                    } else {
                        None
                    }
                }
                BehaviorState::Grooming => {
                    // Soft sparkles
                    if rng.f32() < 1.0 * dt {
                        Some(ParticleSpawn {
                            color: 0xAADDFFBB, // light blue
                            frame: 5,          // star
                            size: 0.1 + rng.f32() * 0.08,
                            vel: Vec2::new(rng.f32() * 20.0 - 10.0, -25.0 - rng.f32() * 15.0),
                            lifetime: 0.8 + rng.f32() * 0.4,
                        })
                    } else {
                        None
                    }
                }
                BehaviorState::Yawning => {
                    // Sleepy dots drifting up
                    if rng.f32() < 2.0 * dt {
                        Some(ParticleSpawn {
                            color: 0x8899BBAA, // muted blue
                            frame: 3,          // circle
                            size: 0.08 + rng.f32() * 0.06,
                            vel: Vec2::new(rng.f32() * 15.0 - 7.5, -20.0 - rng.f32() * 10.0),
                            lifetime: 0.6 + rng.f32() * 0.4,
                        })
                    } else {
                        None
                    }
                }
                // No particles for Idle, Walking, Running, FleeingCursor, Parading
                _ => None,
            };

            if let Some(spawn) = spawn_info {
                if self.particles.len() < MAX_PARTICLES {
                    // Spawn above the cat's head
                    let offset = Vec2::new(
                        rng.f32() * 10.0 - 5.0,
                        -cat_size * 30.0 - rng.f32() * 5.0,
                    );
                    self.particles.push(Particle {
                        pos: pos + offset,
                        vel: spawn.vel,
                        lifetime: spawn.lifetime,
                        max_lifetime: spawn.lifetime,
                        color: spawn.color,
                        size: spawn.size,
                        frame: spawn.frame,
                    });
                }
            }
        }
    }

    /// Update all particles: move, age, remove dead.
    pub fn update(&mut self, dt: f32) {
        // Update in-place, swap-remove dead ones
        let mut i = 0;
        while i < self.particles.len() {
            let p = &mut self.particles[i];
            p.pos += p.vel * dt;
            // Gentle upward drift + slow down
            p.vel.y -= 10.0 * dt; // slight lift
            p.vel *= 1.0 - 2.0 * dt; // drag
            p.lifetime -= dt;

            if p.lifetime <= 0.0 {
                self.particles.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Append particle instances to the render buffer.
    pub fn build_instances(&self, buf: &mut Vec<CatInstance>) {
        for p in &self.particles {
            // Fade alpha based on remaining lifetime
            let alpha_frac = (p.lifetime / p.max_lifetime).clamp(0.0, 1.0);
            // Ease out: fade faster near death
            let alpha = alpha_frac * alpha_frac;

            // Modify alpha channel of the packed color
            let base_alpha = (p.color & 0xFF) as f32;
            let new_alpha = (base_alpha * alpha) as u32;
            let color = (p.color & 0xFFFFFF00) | new_alpha;

            buf.push(CatInstance {
                position: p.pos.into(),
                size: p.size,
                color,
                frame: p.frame,
                rotation: 0.0,
            });
        }
    }

    /// Number of active particles.
    pub fn count(&self) -> usize {
        self.particles.len()
    }
}

struct ParticleSpawn {
    color: u32,
    frame: u32,
    size: f32,
    vel: Vec2,
    lifetime: f32,
}
