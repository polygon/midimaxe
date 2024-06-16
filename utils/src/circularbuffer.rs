use std::collections::VecDeque;

pub struct CircularBuffer<T> {
    buf: VecDeque<T>,
    capacity: usize,
}

impl<T> CircularBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        CircularBuffer {
            buf: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn add(&mut self, val: T) {
        self.buf.push_back(val);
        if self.buf.len() > self.capacity {
            self.buf.pop_front();
        }
    }

    pub fn get_buf(&self) -> &VecDeque<T> {
        &self.buf
    }

    pub fn is_full(&self) -> bool {
        self.buf.len() == self.capacity
    }

    pub fn clear(&mut self) {
        self.buf.clear();
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn resize(&mut self, capacity: usize) {
        self.capacity = capacity;
        while self.capacity < self.buf.len() {
            self.buf.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cb() {
        let mut cb = CircularBuffer::<i32>::new(3);
        assert!(!cb.is_full());
        cb.add(5);
        cb.add(7);
        cb.add(9);
        assert!(cb.is_full());
        assert!(cb.get_buf().iter().eq([5, 7, 9].iter()));
        cb.add(11);
        assert!(cb.get_buf().iter().eq([7, 9, 11].iter()));
    }

    #[test]
    fn test_resize() {
        let mut cb = CircularBuffer::<i32>::new(3);
        cb.add(1);
        cb.add(2);
        cb.add(3);
        cb.add(4);
        cb.add(5);
        assert!(cb.get_buf().iter().eq([3, 4, 5].iter()));
        cb.resize(4);
        cb.add(6);
        assert!(cb.get_buf().iter().eq([3, 4, 5, 6].iter()));
        cb.resize(2);
        assert!(cb.get_buf().iter().eq([5, 6].iter()));
    }
}
