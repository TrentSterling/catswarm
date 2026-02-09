use glam::Vec2;

/// Current world position in screen pixels.
#[derive(Debug, Clone, Copy)]
pub struct Position(pub Vec2);

/// Previous tick's position — used for render interpolation.
#[derive(Debug, Clone, Copy)]
pub struct PrevPosition(pub Vec2);

/// Velocity in pixels/second.
#[derive(Debug, Clone, Copy)]
pub struct Velocity(pub Vec2);

/// Current behavior state.
#[derive(Debug, Clone, Copy)]
pub struct CatState {
    pub state: BehaviorState,
    /// Time remaining in current state (seconds).
    pub timer: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BehaviorState {
    Idle,
    Walking,
    Running,
    Sleeping,
    Grooming,
    ChasingMouse,
    FleeingCursor,
    ChasingCat,
    Playing,
    Zoomies,
    Startled,
    Yawning,
    Parading,
}

/// Cat name for tooltips.
#[derive(Debug, Clone)]
pub struct CatName(pub String);

/// Personality traits — each in [0.0, 1.0].
#[derive(Debug, Clone, Copy)]
pub struct Personality {
    pub laziness: f32,
    pub energy: f32,
    pub curiosity: f32,
    pub skittishness: f32,
}

/// Visual appearance — packed for cache efficiency.
#[derive(Debug, Clone, Copy)]
pub struct Appearance {
    /// RGBA packed as u32.
    pub color: u32,
    /// Pattern variant index.
    pub pattern: u8,
    /// Size multiplier (1.0 = normal).
    pub size: f32,
}

/// Cached spatial hash cell index for fast neighbor lookups.
#[derive(Debug, Clone, Copy)]
pub struct SpatialCell(pub u32);

/// Marks a cat that is interacting with another cat (ChasingCat or Playing).
#[derive(Debug, Clone, Copy)]
pub struct InteractionTarget(pub hecs::Entity);

/// Marks a cat as part of a sleeping pile (3+ nearby sleeping cats).
/// Includes a phase offset for breathing animation variety.
#[derive(Debug, Clone, Copy)]
pub struct SleepingPile {
    pub breathing_offset: f32,
}

/// Spawn drop-in animation. Cat falls from top of screen, somersaults, lands on feet.
#[derive(Debug, Clone, Copy)]
pub struct SpawnAnimation {
    /// Where the cat should land.
    pub target_y: f32,
    /// Progress 0.0 (top of screen) to 1.0 (landed).
    pub progress: f32,
    /// Animation speed (randomized so cats don't all land at the same time).
    pub speed: f32,
    /// Number of somersault rotations (0 = no flip, 1-3 = tumble).
    pub flips: u8,
}
