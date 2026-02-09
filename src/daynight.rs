/// Day/night cycle based on system clock.
/// Provides ambient color tint and behavior energy modifier.

/// Time-of-day state computed from the system clock.
#[derive(Debug, Clone, Copy)]
pub struct DayNightState {
    /// Current hour (0.0-24.0, with fractional minutes).
    pub hour: f32,
    /// Color tint multiplier (r, g, b) applied to cat instances.
    pub tint: [f32; 3],
    /// Energy scale modifier (multiplied with mode energy scale).
    pub energy_modifier: f32,
}

impl DayNightState {
    pub fn new() -> Self {
        let mut s = Self {
            hour: 12.0,
            tint: [1.0, 1.0, 1.0],
            energy_modifier: 1.0,
        };
        s.update();
        s
    }

    /// Refresh from system clock. Call once per frame or less.
    pub fn update(&mut self) {
        #[cfg(windows)]
        {
            self.hour = crate::platform::win32::get_local_hour();
        }
        #[cfg(not(windows))]
        {
            // Fallback: assume noon
            self.hour = 12.0;
        }
        self.tint = compute_tint(self.hour);
        self.energy_modifier = compute_energy_modifier(self.hour);
    }
}

/// Smooth hermite interpolation.
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Lerp between two [f32; 3] arrays.
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Compute ambient color tint based on hour of day.
fn compute_tint(hour: f32) -> [f32; 3] {
    const NIGHT: [f32; 3] = [0.65, 0.68, 0.92];
    const DAWN: [f32; 3] = [1.0, 0.88, 0.75];
    const DAY: [f32; 3] = [1.0, 1.0, 1.0];
    const DUSK: [f32; 3] = [1.0, 0.85, 0.72];
    const EVENING: [f32; 3] = [0.78, 0.82, 0.98];

    if hour < 5.0 {
        NIGHT
    } else if hour < 7.0 {
        let t = smoothstep(5.0, 7.0, hour);
        lerp3(NIGHT, DAWN, t)
    } else if hour < 8.5 {
        let t = smoothstep(7.0, 8.5, hour);
        lerp3(DAWN, DAY, t)
    } else if hour < 17.0 {
        DAY
    } else if hour < 19.0 {
        let t = smoothstep(17.0, 19.0, hour);
        lerp3(DAY, DUSK, t)
    } else if hour < 21.0 {
        let t = smoothstep(19.0, 21.0, hour);
        lerp3(DUSK, EVENING, t)
    } else if hour < 23.0 {
        let t = smoothstep(21.0, 23.0, hour);
        lerp3(EVENING, NIGHT, t)
    } else {
        NIGHT
    }
}

/// Compute behavior energy modifier based on hour of day.
fn compute_energy_modifier(hour: f32) -> f32 {
    if hour < 5.0 {
        0.4
    } else if hour < 7.0 {
        let t = smoothstep(5.0, 7.0, hour);
        0.4 + t * 0.4
    } else if hour < 9.0 {
        let t = smoothstep(7.0, 9.0, hour);
        0.8 + t * 0.2
    } else if hour < 17.0 {
        1.0
    } else if hour < 20.0 {
        let t = smoothstep(17.0, 20.0, hour);
        1.0 - t * 0.2
    } else if hour < 23.0 {
        let t = smoothstep(20.0, 23.0, hour);
        0.8 - t * 0.4
    } else {
        0.4
    }
}
