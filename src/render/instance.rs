use bytemuck::{Pod, Zeroable};
use glam::Vec2;

use crate::ecs::components::{Appearance, BehaviorState, CatState, Position, PrevPosition};

/// Per-instance data uploaded to GPU each frame.
/// Stride = 28 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CatInstance {
    /// Screen position (x, y).
    pub position: [f32; 2],
    /// Scale multiplier (width, height) — supports squash & stretch.
    pub size: [f32; 2],
    /// RGBA color packed as u32.
    pub color: u32,
    /// Animation frame index (0=sitting, 1=walking, 2=sleeping, 3=circle, etc).
    pub frame: u32,
    /// Rotation angle in radians (used for spawn somersault).
    pub rotation: f32,
}

impl CatInstance {
    /// Build a CatInstance from ECS components, interpolating position.
    pub fn from_components(
        pos: &Position,
        prev_pos: &PrevPosition,
        appearance: &Appearance,
        cat_state: &CatState,
        alpha: f32,
        time: f32,
    ) -> Self {
        // Lerp between previous and current position for smooth rendering
        let interp = Vec2::lerp(prev_pos.0, pos.0, alpha);

        // Map behavior state to shader frame index
        let is_moving = matches!(
            cat_state.state,
            BehaviorState::Walking
            | BehaviorState::Running
            | BehaviorState::ChasingMouse
            | BehaviorState::FleeingCursor
            | BehaviorState::ChasingCat
            | BehaviorState::Zoomies
            | BehaviorState::Startled
            | BehaviorState::Parading
        );

        let frame = if cat_state.state == BehaviorState::Sleeping {
            2
        } else if is_moving {
            // Walk cycle: alternate between frame 1 and 7.
            // Use position as phase offset so each cat steps differently.
            // Speed-based: faster cats cycle faster.
            let phase_offset = pos.0.x * 0.013 + pos.0.y * 0.017;
            let cycle_speed = match cat_state.state {
                BehaviorState::Running | BehaviorState::Zoomies => 8.0,
                BehaviorState::Walking | BehaviorState::Parading => 3.5,
                _ => 5.0,
            };
            let phase = (time * cycle_speed + phase_offset).sin();
            if phase > 0.0 { 1 } else { 7 }
        } else {
            0 // Idle, Grooming, Playing, Yawning
        };

        Self {
            position: interp.into(),
            size: [appearance.size, appearance.size],
            color: appearance.color,
            frame,
            rotation: 0.0,
        }
    }
}
