use crate::ecs::components::{CatState, Personality, Position, Velocity};
use crate::spatial::{CatSnapshot, SpatialHash};

/// Rebuild the spatial hash grid and snapshot cache from current positions.
pub fn rebuild(
    world: &hecs::World,
    grid: &mut SpatialHash,
    snapshots: &mut Vec<CatSnapshot>,
) {
    grid.clear();
    snapshots.clear();
    for (entity, (pos, vel, cat_state, personality)) in
        world.query::<(&Position, &Velocity, &CatState, &Personality)>().iter()
    {
        let idx = snapshots.len() as u32;
        snapshots.push(CatSnapshot {
            entity,
            pos: pos.0,
            vel: vel.0,
            state: cat_state.state,
            personality: *personality,
        });
        grid.insert(pos.0, idx);
    }
}
