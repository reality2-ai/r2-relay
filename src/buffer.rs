use std::collections::VecDeque;

/// A buffered frame with its receive timestamp.
pub struct BufferedFrame {
    pub timestamp: u64,
    pub data: Vec<u8>,
}

/// Bounded ring buffer of recent R2-WIRE frames for catchup.
///
/// Per R2-TRANSPORT-RELAY §4: volatile, default 1000 frames per trust group.
pub struct RingBuffer {
    frames: VecDeque<BufferedFrame>,
    capacity: usize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        RingBuffer {
            frames: VecDeque::with_capacity(capacity.min(1024)),
            capacity,
        }
    }

    /// Push a frame into the buffer, evicting the oldest if full.
    pub fn push(&mut self, data: Vec<u8>, timestamp: u64) {
        if self.frames.len() >= self.capacity {
            self.frames.pop_front();
        }
        self.frames.push_back(BufferedFrame { timestamp, data });
    }

    /// Iterate frames newer than the given timestamp.
    pub fn since(&self, timestamp: u64) -> impl Iterator<Item = &BufferedFrame> {
        self.frames.iter().filter(move |f| f.timestamp > timestamp)
    }

    /// Timestamp of the oldest buffered frame, or 0 if empty.
    pub fn oldest_timestamp(&self) -> u64 {
        self.frames.front().map(|f| f.timestamp).unwrap_or(0)
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_evict() {
        let mut buf = RingBuffer::new(3);
        buf.push(vec![1], 100);
        buf.push(vec![2], 200);
        buf.push(vec![3], 300);
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.oldest_timestamp(), 100);

        buf.push(vec![4], 400);
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.oldest_timestamp(), 200);
    }

    #[test]
    fn since_filter() {
        let mut buf = RingBuffer::new(10);
        buf.push(vec![1], 100);
        buf.push(vec![2], 200);
        buf.push(vec![3], 300);

        let frames: Vec<_> = buf.since(150).collect();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].data, vec![2]);
        assert_eq!(frames[1].data, vec![3]);
    }
}
