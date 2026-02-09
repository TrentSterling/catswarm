/// Animation frame data for a cat.
#[derive(Debug, Clone, Copy)]
pub struct AnimationState {
    pub clip: u8,
    pub frame: u8,
    pub elapsed: f32,
    pub speed: f32,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self {
            clip: 0,
            frame: 0,
            elapsed: 0.0,
            speed: 1.0,
        }
    }
}
