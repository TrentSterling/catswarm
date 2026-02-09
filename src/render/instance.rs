use bytemuck::{Pod, Zeroable};
use glam::Vec2;

use crate::ecs::components::{Appearance, BehaviorState, CatState, Position, PrevPosition};

/// Per-instance data uploaded to GPU each frame.
/// Stride = 24 bytes, GPU-aligned.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CatInstance {
    /// Screen position (x, y).
    pub position: [f32; 2],
    /// Scale multiplier.
    pub size: f32,
    /// RGBA color packed as u32.
    pub color: u32,
    /// Animation frame index (0=sitting, 1=walking, 2=sleeping).
    pub frame: u32,
    /// Padding for 24-byte alignment.
    pub _pad: u32,
}

impl CatInstance {
    /// Build a CatInstance from ECS components, interpolating position.
    pub fn from_components(
        pos: &Position,
        prev_pos: &PrevPosition,
        appearance: &Appearance,
        cat_state: &CatState,
        alpha: f32,
    ) -> Self {
        // Lerp between previous and current position for smooth rendering
        let interp = Vec2::lerp(prev_pos.0, pos.0, alpha);

        // Map behavior state to shader frame index
        let frame = match cat_state.state {
            BehaviorState::Sleeping => 2,
            BehaviorState::Walking
            | BehaviorState::Running
            | BehaviorState::ChasingMouse
            | BehaviorState::FleeingCursor
            | BehaviorState::ChasingCat => 1,
            _ => 0, // Idle, Grooming, Playing
        };

        Self {
            position: interp.into(),
            size: appearance.size,
            color: appearance.color,
            frame,
            _pad: 0,
        }
    }
}
