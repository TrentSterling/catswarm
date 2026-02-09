use glam::Vec2;

/// A yarn ball toy that cats can chase.
pub struct YarnBall {
    pub pos: Vec2,
    pub vel: Vec2,
    pub active: bool,
}

/// Friction applied to yarn ball each frame.
const YARN_FRICTION: f32 = 0.97;
/// Minimum speed before yarn ball stops.
const YARN_MIN_SPEED: f32 = 2.0;
/// Bounce elasticity.
const YARN_BOUNCE: f32 = 0.6;

impl YarnBall {
    pub fn new() -> Self {
        Self {
            pos: Vec2::ZERO,
            vel: Vec2::ZERO,
            active: false,
        }
    }

    /// Spawn a yarn ball at the given position.
    pub fn spawn(&mut self, pos: Vec2) {
        self.pos = pos;
        self.vel = Vec2::ZERO;
        self.active = true;
    }

    /// Throw the yarn ball with a velocity.
    pub fn throw(&mut self, vel: Vec2) {
        self.vel = vel;
    }

    /// Apply an impulse (from a cat batting at it).
    pub fn bat(&mut self, impulse: Vec2) {
        self.vel += impulse;
    }

    /// Despawn the yarn ball.
    pub fn despawn(&mut self) {
        self.active = false;
    }

    /// Update physics: friction, bounce off screen edges.
    pub fn update(&mut self, dt: f32, screen_w: f32, screen_h: f32) {
        if !self.active {
            return;
        }

        self.pos += self.vel * dt;
        self.vel *= YARN_FRICTION;

        // Bounce off walls
        let margin = 10.0;
        if self.pos.x < margin {
            self.pos.x = margin;
            self.vel.x = self.vel.x.abs() * YARN_BOUNCE;
        }
        if self.pos.x > screen_w - margin {
            self.pos.x = screen_w - margin;
            self.vel.x = -self.vel.x.abs() * YARN_BOUNCE;
        }
        if self.pos.y < margin {
            self.pos.y = margin;
            self.vel.y = self.vel.y.abs() * YARN_BOUNCE;
        }
        if self.pos.y > screen_h - margin {
            self.pos.y = screen_h - margin;
            self.vel.y = -self.vel.y.abs() * YARN_BOUNCE;
        }

        // Stop if very slow
        if self.vel.length_squared() < YARN_MIN_SPEED * YARN_MIN_SPEED {
            self.vel = Vec2::ZERO;
        }
    }
}
