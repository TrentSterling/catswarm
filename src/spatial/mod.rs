use glam::Vec2;

use crate::ecs::components::{BehaviorState, Personality};

/// Snapshot of a cat's state for interaction queries.
/// Stored alongside the spatial hash to avoid ECS lookups in hot path.
pub struct CatSnapshot {
    pub entity: hecs::Entity,
    pub pos: Vec2,
    pub state: BehaviorState,
    pub personality: Personality,
}

/// Spatial hash grid for O(1) neighbor queries.
///
/// Cell size should be ~2x the largest interaction radius.
/// Uses multiplicative hash for even distribution.
pub struct SpatialHash {
    cell_size: f32,
    inv_cell_size: f32,
    table_size: usize,
    /// Each bucket holds entity indices. Pre-allocated, cleared each frame.
    buckets: Vec<Vec<u32>>,
}

impl SpatialHash {
    pub fn new(cell_size: f32, table_size: usize) -> Self {
        let mut buckets = Vec::with_capacity(table_size);
        for _ in 0..table_size {
            // Pre-allocate each bucket to avoid allocs during rebuild.
            buckets.push(Vec::with_capacity(8));
        }
        Self {
            cell_size,
            inv_cell_size: 1.0 / cell_size,
            table_size,
            buckets,
        }
    }

    /// Clear all buckets. Call at start of each rebuild.
    pub fn clear(&mut self) {
        for bucket in &mut self.buckets {
            bucket.clear(); // Keeps allocation.
        }
    }

    /// Insert an entity at the given position.
    pub fn insert(&mut self, pos: Vec2, entity_index: u32) {
        let hash = self.hash(pos);
        self.buckets[hash].push(entity_index);
    }

    /// Query all entities in the same cell and 8 surrounding cells.
    /// Calls `callback` for each entity index found.
    pub fn query_neighbors(&self, pos: Vec2, mut callback: impl FnMut(u32)) {
        let (cx, cy) = self.cell_coords(pos);
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let hash = self.hash_cell(cx.wrapping_add(dx), cy.wrapping_add(dy));
                for &entity_index in &self.buckets[hash] {
                    callback(entity_index);
                }
            }
        }
    }

    fn cell_coords(&self, pos: Vec2) -> (i32, i32) {
        let cx = (pos.x * self.inv_cell_size).floor() as i32;
        let cy = (pos.y * self.inv_cell_size).floor() as i32;
        (cx, cy)
    }

    fn hash(&self, pos: Vec2) -> usize {
        let (cx, cy) = self.cell_coords(pos);
        self.hash_cell(cx, cy)
    }

    fn hash_cell(&self, cx: i32, cy: i32) -> usize {
        // Multiplicative spatial hash — good distribution for grid data.
        let h = (cx as u32).wrapping_mul(73856093) ^ (cy as u32).wrapping_mul(19349663);
        (h as usize) % self.table_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_query() {
        let mut grid = SpatialHash::new(64.0, 256);
        grid.insert(Vec2::new(100.0, 100.0), 0);
        grid.insert(Vec2::new(110.0, 105.0), 1);
        grid.insert(Vec2::new(900.0, 900.0), 2);

        let mut found = Vec::new();
        grid.query_neighbors(Vec2::new(105.0, 102.0), |idx| found.push(idx));

        assert!(found.contains(&0));
        assert!(found.contains(&1));
    }

    #[test]
    fn clear_and_reuse() {
        let mut grid = SpatialHash::new(64.0, 256);
        grid.insert(Vec2::new(50.0, 50.0), 42);
        grid.clear();

        let mut found = Vec::new();
        grid.query_neighbors(Vec2::new(50.0, 50.0), |idx| found.push(idx));
        assert!(found.is_empty());
    }
}
