use glam::Vec2;

/// A single yarn ball with physics.
#[derive(Debug, Clone, Copy)]
pub struct YarnBall {
    pub pos: Vec2,
    pub vel: Vec2,
    pub lifetime: f32,
}

/// Manages multiple yarn balls.
pub struct YarnBalls {
    pub balls: Vec<YarnBall>,
}

/// Max yarn balls on screen at once.
const MAX_YARN_BALLS: usize = 20;
/// Yarn ball lifetime in seconds (auto-despawn).
const YARN_LIFETIME: f32 = 30.0;
/// Friction applied each frame.
const YARN_FRICTION: f32 = 0.995;
/// Minimum speed before yarn ball stops.
const YARN_MIN_SPEED: f32 = 5.0;
/// Bounce elasticity.
const YARN_BOUNCE: f32 = 0.75;
/// Mouse push radius.
const MOUSE_PUSH_RADIUS: f32 = 120.0;
/// Mouse push strength.
const MOUSE_PUSH_STRENGTH: f32 = 600.0;

impl YarnBalls {
    pub fn new() -> Self {
        Self {
            balls: Vec::with_capacity(MAX_YARN_BALLS),
        }
    }

    /// Spawn a new yarn ball at position.
    pub fn spawn(&mut self, pos: Vec2) {
        // Remove oldest if at capacity
        if self.balls.len() >= MAX_YARN_BALLS {
            self.balls.remove(0);
        }
        self.balls.push(YarnBall {
            pos,
            vel: Vec2::ZERO,
            lifetime: YARN_LIFETIME,
        });
    }

    /// Update all yarn balls: physics, mouse push, bouncing, lifetime.
    pub fn update(&mut self, dt: f32, screen_w: f32, screen_h: f32, mouse_pos: Vec2) {
        for ball in &mut self.balls {
            // Mouse pushes yarn balls
            let to_ball = ball.pos - mouse_pos;
            let dist = to_ball.length();
            if dist < MOUSE_PUSH_RADIUS && dist > 1.0 {
                let push_dir = to_ball / dist;
                let push_strength = (1.0 - dist / MOUSE_PUSH_RADIUS) * MOUSE_PUSH_STRENGTH;
                ball.vel += push_dir * push_strength * dt;
            }

            ball.pos += ball.vel * dt;
            ball.vel *= YARN_FRICTION;
            ball.lifetime -= dt;

            // Bounce off walls
            let margin = 10.0;
            if ball.pos.x < margin {
                ball.pos.x = margin;
                ball.vel.x = ball.vel.x.abs() * YARN_BOUNCE;
            }
            if ball.pos.x > screen_w - margin {
                ball.pos.x = screen_w - margin;
                ball.vel.x = -ball.vel.x.abs() * YARN_BOUNCE;
            }
            if ball.pos.y < margin {
                ball.pos.y = margin;
                ball.vel.y = ball.vel.y.abs() * YARN_BOUNCE;
            }
            if ball.pos.y > screen_h - margin {
                ball.pos.y = screen_h - margin;
                ball.vel.y = -ball.vel.y.abs() * YARN_BOUNCE;
            }

            // Stop if very slow
            if ball.vel.length_squared() < YARN_MIN_SPEED * YARN_MIN_SPEED {
                ball.vel = Vec2::ZERO;
            }
        }

        // Remove expired
        self.balls.retain(|b| b.lifetime > 0.0);
    }

    /// Apply a bat impulse to a specific ball by index.
    pub fn bat(&mut self, index: usize, impulse: Vec2) {
        if let Some(ball) = self.balls.get_mut(index) {
            ball.vel += impulse;
        }
    }

    /// Are there any active yarn balls?
    pub fn any_active(&self) -> bool {
        !self.balls.is_empty()
    }
}
