/// Application mode — determines how cats behave relative to user activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Work,
    Play,
    Zen,
    Chaos,
}

impl AppMode {
    pub fn label(self) -> &'static str {
        match self {
            AppMode::Work => "Work",
            AppMode::Play => "Play",
            AppMode::Zen => "Zen",
            AppMode::Chaos => "Chaos",
        }
    }

    pub fn next(self) -> Self {
        match self {
            AppMode::Work => AppMode::Play,
            AppMode::Play => AppMode::Zen,
            AppMode::Zen => AppMode::Chaos,
            AppMode::Chaos => AppMode::Work,
        }
    }
}

const ALL_MODES: [AppMode; 4] = [AppMode::Work, AppMode::Play, AppMode::Zen, AppMode::Chaos];

/// State tracking for mode system and AFK escalation.
pub struct ModeState {
    pub mode: AppMode,
    /// Seconds since last user input (from GetLastInputInfo).
    pub idle_seconds: f64,
    /// Whether to auto-transition to Zen on AFK.
    pub auto_zen: bool,
    /// Edge affinity: 0.0 = uniform distribution, 1.0 = edges only.
    pub edge_affinity: f32,
    /// Multiplier on behavior energy (walk/run weights).
    pub behavior_energy_scale: f32,
    /// Whether mouse chase is enabled.
    pub chase_enabled: bool,
    /// Extra cats spawned from AFK escalation.
    pub bonus_cats_spawned: usize,
    /// Max bonus cats from AFK.
    pub bonus_cats_cap: usize,
    /// Mode before AFK auto-transition (to restore on return).
    prev_mode: Option<AppMode>,
    /// Whether AFK escalation is actively running.
    pub afk_active: bool,
    /// F11 edge detection.
    f11_was_down: bool,
}

impl ModeState {
    pub fn new() -> Self {
        let mut s = Self {
            mode: AppMode::Play,
            idle_seconds: 0.0,
            auto_zen: true,
            edge_affinity: 0.0,
            behavior_energy_scale: 1.0,
            chase_enabled: true,
            bonus_cats_spawned: 0,
            bonus_cats_cap: 1000,
            prev_mode: None,
            afk_active: false,
            f11_was_down: false,
        };
        s.apply_mode_preset();
        s
    }

    /// Apply preset values for the current mode.
    fn apply_mode_preset(&mut self) {
        match self.mode {
            AppMode::Work => {
                self.edge_affinity = 0.7;
                self.behavior_energy_scale = 0.3;
                self.chase_enabled = false;
            }
            AppMode::Play => {
                self.edge_affinity = 0.0;
                self.behavior_energy_scale = 1.0;
                self.chase_enabled = true;
            }
            AppMode::Zen => {
                self.edge_affinity = 0.0;
                self.behavior_energy_scale = 1.5;
                self.chase_enabled = true;
            }
            AppMode::Chaos => {
                self.edge_affinity = 0.0;
                self.behavior_energy_scale = 3.0;
                self.chase_enabled = true;
            }
        }
    }

    /// Cycle to next mode (F11). Returns true if mode changed.
    pub fn poll_f11(&mut self, f11_down: bool) -> bool {
        if f11_down && !self.f11_was_down {
            self.f11_was_down = true;
            self.mode = self.mode.next();
            self.apply_mode_preset();
            // Cancel AFK if manually switching
            self.afk_active = false;
            self.prev_mode = None;
            return true;
        }
        if !f11_down {
            self.f11_was_down = false;
        }
        false
    }

    /// Set mode directly (from debug UI).
    pub fn set_mode(&mut self, mode: AppMode) {
        if self.mode != mode {
            self.mode = mode;
            self.apply_mode_preset();
            self.afk_active = false;
            self.prev_mode = None;
        }
    }

    /// Update AFK escalation logic. Call once per frame.
    /// Returns number of bonus cats to spawn this frame (0 usually).
    pub fn update_afk(&mut self, idle_seconds: f64, dt: f64) -> AtkAction {
        let prev_idle = self.idle_seconds;
        self.idle_seconds = idle_seconds;

        // Detect return from idle (idle dropped significantly)
        if prev_idle > 2.0 && idle_seconds < 1.0 && self.afk_active {
            return self.on_return_from_afk();
        }

        if !self.auto_zen {
            return AtkAction::None;
        }

        // AFK escalation thresholds
        if idle_seconds < 30.0 {
            return AtkAction::None;
        }

        if idle_seconds < 120.0 {
            // 30s-2min: gradually reduce edge affinity (cats drift center)
            let t = ((idle_seconds - 30.0) / 90.0) as f32; // 0..1 over 90s
            self.edge_affinity = self.edge_affinity * (1.0 - t * 0.5);
            return AtkAction::None;
        }

        if idle_seconds < 300.0 {
            // 2-5min: increase energy
            if !self.afk_active {
                self.afk_active = true;
                self.prev_mode = Some(self.mode);
            }
            self.behavior_energy_scale = 1.5;
            self.edge_affinity = 0.0;
            return AtkAction::None;
        }

        // 5min+: full Zen mode, spawn bonus cats
        if !self.afk_active {
            self.afk_active = true;
            self.prev_mode = Some(self.mode);
        }
        self.mode = AppMode::Zen;
        self.apply_mode_preset();

        // Spawn ~50 cats/min = ~0.83/s
        if self.bonus_cats_spawned < self.bonus_cats_cap {
            let spawn_rate = 50.0 / 60.0; // cats per second
            let to_spawn = (spawn_rate * dt) as usize;
            if to_spawn > 0 {
                self.bonus_cats_spawned += to_spawn;
                return AtkAction::SpawnCats(to_spawn);
            }
        }

        AtkAction::None
    }

    fn on_return_from_afk(&mut self) -> AtkAction {
        self.afk_active = false;
        let despawn = self.bonus_cats_spawned;
        self.bonus_cats_spawned = 0;

        // Restore previous mode
        if let Some(prev) = self.prev_mode.take() {
            self.mode = prev;
            self.apply_mode_preset();
        }

        if despawn > 0 {
            AtkAction::ScatterAndDespawn(despawn)
        } else {
            AtkAction::Scatter
        }
    }

    pub fn all_modes() -> &'static [AppMode] {
        &ALL_MODES
    }
}

/// Action to take from AFK update.
pub enum AtkAction {
    None,
    /// Spawn N bonus cats.
    SpawnCats(usize),
    /// User returned: scatter all cats + despawn N bonus cats.
    ScatterAndDespawn(usize),
    /// User returned: scatter all cats (no bonus to despawn).
    Scatter,
}
