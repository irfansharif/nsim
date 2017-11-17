pub struct CircularBuffer<T: Clone> {
    vec: Vec<Option<T>>,
    idx: usize,
}

impl<T: Clone> CircularBuffer<T> {
    pub fn new(size: usize) -> CircularBuffer<T> {
        assert!(size > 0);

        CircularBuffer {
            vec: vec![None; size],
            idx: 0,
        }
    }

    pub fn read(&self) -> Option<T> {
        self.vec[self.idx].clone()
    }

    pub fn write(&mut self, t: T) {
        self.vec[self.idx] = Some(t)
    }

    pub fn advance(&mut self) {
        self.idx = (self.idx + 1) % self.vec.len();
    }
}

#[test]
fn test_cbuffer() {
    let mut c: CircularBuffer<u32> = CircularBuffer::new(2);

    assert_eq!(c.read(), None);

    c.write(0);
    assert_eq!(c.read(), Some(0));

    c.advance();
    assert_eq!(c.read(), None);

    c.write(1);
    assert_eq!(c.read(), Some(1));

    c.advance();
    assert_eq!(c.read(), Some(0));
}
