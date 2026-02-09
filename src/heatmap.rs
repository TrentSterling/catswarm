/// Cursor heatmap — tracks where the mouse spends time.
/// Used for visual overlay and movement avoidance.

/// Grid resolution (cells per axis).
const GRID_SIZE: usize = 64;
/// Decay factor per frame (~5s half-life at 60fps: 0.995^300 ≈ 0.22).
const DECAY: f32 = 0.995;
/// Heat accumulation rate per second.
const HEAT_RATE: f32 = 2.0;

pub struct Heatmap {
    /// Heat values in [0, 1], row-major.
    pub cells: Vec<f32>,
    pub grid_size: usize,
    pub cell_w: f32,
    pub cell_h: f32,
    pub enabled: bool,
}

impl Heatmap {
    pub fn new(screen_w: f32, screen_h: f32) -> Self {
        Self {
            cells: vec![0.0; GRID_SIZE * GRID_SIZE],
            grid_size: GRID_SIZE,
            cell_w: screen_w / GRID_SIZE as f32,
            cell_h: screen_h / GRID_SIZE as f32,
            enabled: false,
        }
    }

    /// Update on screen resize.
    pub fn resize(&mut self, screen_w: f32, screen_h: f32) {
        self.cell_w = screen_w / GRID_SIZE as f32;
        self.cell_h = screen_h / GRID_SIZE as f32;
    }

    /// Update heatmap: accumulate at cursor, decay all.
    pub fn update(&mut self, cursor_x: f32, cursor_y: f32, dt: f32) {
        // Decay all cells
        for cell in &mut self.cells {
            *cell *= DECAY;
        }

        // Accumulate heat at cursor position
        let cx = (cursor_x / self.cell_w) as usize;
        let cy = (cursor_y / self.cell_h) as usize;
        if cx < GRID_SIZE && cy < GRID_SIZE {
            let idx = cy * GRID_SIZE + cx;
            self.cells[idx] = (self.cells[idx] + dt * HEAT_RATE).min(1.0);

            // Spread to neighbors (gaussian-ish)
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx >= 0 && nx < GRID_SIZE as i32 && ny >= 0 && ny < GRID_SIZE as i32 {
                        let ni = ny as usize * GRID_SIZE + nx as usize;
                        self.cells[ni] = (self.cells[ni] + dt * HEAT_RATE * 0.3).min(1.0);
                    }
                }
            }
        }
    }

    /// Sample heat at a world position. Returns 0.0-1.0.
    pub fn sample(&self, x: f32, y: f32) -> f32 {
        let cx = (x / self.cell_w) as usize;
        let cy = (y / self.cell_h) as usize;
        if cx < GRID_SIZE && cy < GRID_SIZE {
            self.cells[cy * GRID_SIZE + cx]
        } else {
            0.0
        }
    }

    /// Flatten to R8 texture data for GPU upload.
    pub fn to_texture_data(&self) -> Vec<u8> {
        self.cells
            .iter()
            .map(|&v| (v * 255.0).min(255.0) as u8)
            .collect()
    }
}
