pub struct CircularBuffer<T: Clone> {
    vec: Vec<T>,
    idx: usize,
}

impl<T: Clone> CircularBuffer<T> {
    pub fn new(size: usize, default: T) -> CircularBuffer<T> {
        assert!(size > 0);

        CircularBuffer {
            vec: vec![default; size],
            idx: 0,
        }
    }

    pub fn read(&self) -> T {
        self.vec[self.idx].clone()
    }

    pub fn write(&mut self, t: T) {
        self.vec[self.idx] = t
    }

    pub fn tick(&mut self) {
        self.idx = (self.idx + 1) % self.vec.len();
    }
}

mod tests {
    #[test]
    fn test_cbuffer() {
        let mut c: super::CircularBuffer<u32> = super::CircularBuffer::new(2, 0);

        assert_eq!(c.read(), 0);

        c.write(1);
        assert_eq!(c.read(), 1);

        c.tick();
        assert_eq!(c.read(), 0);

        c.write(1);
        assert_eq!(c.read(), 1);

        c.tick();
        assert_eq!(c.read(), 1);
    }
}
