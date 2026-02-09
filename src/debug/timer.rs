use instant::Instant;

/// Which phase of the simulation tick is being timed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SystemPhase {
    Mouse = 0,
    Behavior = 1,
    Movement = 2,
    SpatialRebuild = 3,
    Interaction = 4,
    BuildInstances = 5,
    GpuUpload = 6,
    RenderSubmit = 7,
}

impl SystemPhase {
    pub const ALL: [SystemPhase; 8] = [
        Self::Mouse,
        Self::Behavior,
        Self::Movement,
        Self::SpatialRebuild,
        Self::Interaction,
        Self::BuildInstances,
        Self::GpuUpload,
        Self::RenderSubmit,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Mouse => "Mouse",
            Self::Behavior => "Behavior",
            Self::Movement => "Movement",
            Self::SpatialRebuild => "Spatial",
            Self::Interaction => "Interaction",
            Self::BuildInstances => "Build Inst.",
            Self::GpuUpload => "GPU Upload",
            Self::RenderSubmit => "Render",
        }
    }
}

/// Per-system timing with exponential moving average smoothing.
pub struct SystemTimers {
    /// EMA-smoothed duration in microseconds per phase.
    pub durations_us: [f64; 8],
    /// Timestamp when `begin()` was called.
    start: Instant,
}

const EMA_ALPHA: f64 = 0.1;

impl SystemTimers {
    pub fn new() -> Self {
        Self {
            durations_us: [0.0; 8],
            start: Instant::now(),
        }
    }

    /// Call before a system runs.
    pub fn begin(&mut self) {
        self.start = Instant::now();
    }

    /// Call after a system finishes. Records elapsed time for `phase`.
    pub fn end(&mut self, phase: SystemPhase) {
        let elapsed_us = self.start.elapsed().as_secs_f64() * 1_000_000.0;
        let idx = phase as usize;
        self.durations_us[idx] =
            self.durations_us[idx] * (1.0 - EMA_ALPHA) + elapsed_us * EMA_ALPHA;
    }

    /// Sum of all phase durations (microseconds).
    pub fn total_us(&self) -> f64 {
        self.durations_us.iter().sum()
    }
}
