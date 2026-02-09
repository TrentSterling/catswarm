use crate::ecs::components::Personality;

/// Get the weight multiplier for transitioning to a given behavior.
pub fn transition_weight(personality: &Personality, _target_state: u8) -> f32 {
    // TODO: Milestone 5
    let _ = personality;
    1.0
}
