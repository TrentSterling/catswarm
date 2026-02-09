use crate::ecs::components::{Appearance, CatState, Personality, Position, Stacked, Velocity};
use crate::spatial::{CatSnapshot, SpatialHash};

/// Rebuild the spatial hash grid and snapshot cache from current positions.
pub fn rebuild(
    world: &hecs::World,
    grid: &mut SpatialHash,
    snapshots: &mut Vec<CatSnapshot>,
) {
    grid.clear();
    snapshots.clear();
    for (entity, (pos, vel, cat_state, personality, appearance, stacked)) in
        world.query::<(&Position, &Velocity, &CatState, &Personality, &Appearance, Option<&Stacked>)>().iter()
    {
        let idx = snapshots.len() as u32;
        snapshots.push(CatSnapshot {
            entity,
            pos: pos.0,
            vel: vel.0,
            state: cat_state.state,
            personality: *personality,
            size: appearance.size,
            is_stacked: stacked.is_some(),
        });
        grid.insert(pos.0, idx);
    }
}
