//! Data shared between the simulation and the GPU, kept free of wgpu so the
//! simulation builds and tests on any OS.

use bytemuck::{Pod, Zeroable};

/// Texture array layers. 0 kitty, 1 heart, 2 paper, the rest are cat cards.
pub const LAYERS: u32 = 64;
pub const LAYER_KITTY: u32 = 0;
pub const LAYER_HEART: u32 = 1;
pub const LAYER_PAPER: u32 = 2;
pub const FIRST_CARD_LAYER: u32 = 3;

/// One textured quad. Negative `size[0]` mirrors it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Sprite {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub rot: f32,
    pub layer: u32,
    pub tint: [f32; 4],
}
