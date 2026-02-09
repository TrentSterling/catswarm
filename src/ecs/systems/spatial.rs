use crate::ecs::components::{Appearance, CatState, Personality, Position, Velocity};
use crate::spatial::{CatSnapshot, SpatialHash};

/// Rebuild the spatial hash grid and snapshot cache from current positions.
pub fn rebuild(
    world: &hecs::World,
    grid: &mut SpatialHash,
    snapshots: &mut Vec<CatSnapshot>,
) {
    grid.clear();
    snapshots.clear();
    for (entity, (pos, vel, cat_state, personality, appearance)) in
        world.query::<(&Position, &Velocity, &CatState, &Personality, &Appearance)>().iter()
    {
        let idx = snapshots.len() as u32;
        snapshots.push(CatSnapshot {
            entity,
            pos: pos.0,
            vel: vel.0,
            state: cat_state.state,
            personality: *personality,
            size: appearance.size,
        });
        grid.insert(pos.0, idx);
    }
}
