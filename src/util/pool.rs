/// Simple object pool for reusable allocations.
pub struct Pool<T> {
    items: Vec<Option<T>>,
    free: Vec<usize>,
}

impl<T> Pool<T> {
    pub fn with_capacity(cap: usize) -> Self {
        let mut items = Vec::with_capacity(cap);
        let mut free = Vec::with_capacity(cap);
        for i in (0..cap).rev() {
            items.push(None);
            free.push(i);
        }
        Self { items, free }
    }

    pub fn alloc(&mut self, item: T) -> Option<usize> {
        let idx = self.free.pop()?;
        self.items[idx] = Some(item);
        Some(idx)
    }

    pub fn free(&mut self, idx: usize) {
        self.items[idx] = None;
        self.free.push(idx);
    }

    pub fn get(&self, idx: usize) -> Option<&T> {
        self.items.get(idx)?.as_ref()
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut T> {
        self.items.get_mut(idx)?.as_mut()
    }
}
