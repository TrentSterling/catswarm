use glam::Vec2;

/// A treat placed by right-clicking — attracts nearby cats.
#[derive(Debug, Clone, Copy)]
pub struct Treat {
    pub pos: Vec2,
    pub timer: f32,
}

/// Max active treats on screen at once.
const MAX_TREATS: usize = 10;
/// Treat lifetime in seconds.
const TREAT_LIFETIME: f32 = 10.0;
/// Double-click window in seconds.
const DOUBLE_CLICK_WINDOW: f64 = 0.3;
/// Laser pointer duration in seconds.
const LASER_DURATION: f32 = 5.0;

/// Tracks mouse button state and click-derived actions.
pub struct ClickState {
    left_was_down: bool,
    right_was_down: bool,
    middle_was_down: bool,
    last_left_click_time: f64,
    pub treats: Vec<Treat>,
    pub laser_active: bool,
    pub laser_timer: f32,
    /// Elapsed time since app start (accumulated).
    elapsed: f64,
    /// Set for one frame when left click detected.
    pub left_clicked: bool,
    /// Set for one frame when right click detected.
    pub right_clicked: bool,
    /// Set for one frame when double click detected.
    pub double_clicked: bool,
    /// Set for one frame when middle click detected (yarn ball spawn/throw).
    pub middle_clicked: bool,
}

impl ClickState {
    pub fn new() -> Self {
        Self {
            left_was_down: false,
            right_was_down: false,
            middle_was_down: false,
            last_left_click_time: -1.0,
            treats: Vec::with_capacity(MAX_TREATS),
            laser_active: false,
            laser_timer: 0.0,
            elapsed: 0.0,
            left_clicked: false,
            right_clicked: false,
            double_clicked: false,
            middle_clicked: false,
        }
    }

    /// Update click state from raw button polling. Call once per frame.
    pub fn update(
        &mut self,
        left_down: bool,
        right_down: bool,
        middle_down: bool,
        mouse_pos: Vec2,
        dt: f32,
    ) {
        self.elapsed += dt as f64;
        self.left_clicked = false;
        self.right_clicked = false;
        self.double_clicked = false;
        self.middle_clicked = false;

        // Edge-detect left click (press, not hold)
        if left_down && !self.left_was_down {
            // Check for double click
            if self.elapsed - self.last_left_click_time < DOUBLE_CLICK_WINDOW {
                self.double_clicked = true;
            } else {
                self.left_clicked = true;
            }
            self.last_left_click_time = self.elapsed;
        }
        self.left_was_down = left_down;

        // Edge-detect right click
        if right_down && !self.right_was_down {
            self.right_clicked = true;
            // Spawn a treat
            if self.treats.len() < MAX_TREATS {
                self.treats.push(Treat {
                    pos: mouse_pos,
                    timer: TREAT_LIFETIME,
                });
            }
        }
        self.right_was_down = right_down;

        // Edge-detect middle click (yarn ball)
        if middle_down && !self.middle_was_down {
            self.middle_clicked = true;
        }
        self.middle_was_down = middle_down;

        // Update treat timers
        for treat in &mut self.treats {
            treat.timer -= dt;
        }
        self.treats.retain(|t| t.timer > 0.0);

        // Update laser timer
        if self.double_clicked {
            self.laser_active = true;
            self.laser_timer = LASER_DURATION;
        }
        if self.laser_active {
            self.laser_timer -= dt;
            if self.laser_timer <= 0.0 {
                self.laser_active = false;
            }
        }
    }
}
