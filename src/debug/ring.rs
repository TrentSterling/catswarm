/// Fixed-capacity circular buffer. Pre-allocated, no heap allocs after init.
pub struct RingBuffer<T> {
    buf: Vec<T>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl<T: Copy + Default> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: vec![T::default(); capacity],
            capacity,
            head: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, value: T) {
        self.buf[self.head] = value;
        self.head = (self.head + 1) % self.capacity;
        if self.len < self.capacity {
            self.len += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// Iterate from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let start = if self.len < self.capacity {
            0
        } else {
            self.head
        };
        let cap = self.capacity;
        let len = self.len;
        (0..len).map(move |i| &self.buf[(start + i) % cap])
    }
}
