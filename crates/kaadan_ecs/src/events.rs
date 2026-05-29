//! Minimal double-buffered event queue.
//!
//! Store an `Events<T>` as a resource, `send` into it from any system, and
//! `iter` it from readers. Call [`Events::update`] once per frame (typically in
//! [`Stage::First`](crate::Stage::First)) so events stay readable for the frame
//! they were sent plus the next one, then are dropped.

/// A double-buffered queue of events of type `T`.
pub struct Events<T> {
    buffers: [Vec<T>; 2],
    write: usize,
}

impl<T> Default for Events<T> {
    fn default() -> Self {
        Self {
            buffers: [Vec::new(), Vec::new()],
            write: 0,
        }
    }
}

impl<T> Events<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue an event.
    pub fn send(&mut self, event: T) {
        self.buffers[self.write].push(event);
    }

    /// Advance the buffers: the oldest buffer is cleared and becomes the new
    /// write target. Call once per frame.
    pub fn update(&mut self) {
        self.write = 1 - self.write;
        self.buffers[self.write].clear();
    }

    /// Iterate all currently-readable events (this frame's and last frame's),
    /// oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let read = 1 - self.write;
        self.buffers[read]
            .iter()
            .chain(self.buffers[self.write].iter())
    }

    pub fn is_empty(&self) -> bool {
        self.buffers[0].is_empty() && self.buffers[1].is_empty()
    }

    pub fn len(&self) -> usize {
        self.buffers[0].len() + self.buffers[1].len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_and_read() {
        let mut events = Events::<u32>::new();
        events.send(1);
        events.send(2);
        let read: Vec<u32> = events.iter().copied().collect();
        assert_eq!(read, vec![1, 2]);
    }

    #[test]
    fn events_expire_after_two_updates() {
        let mut events = Events::<u32>::new();
        events.send(42);
        events.update(); // sent events still readable (one frame old)
        assert_eq!(events.len(), 1);
        events.update(); // now dropped
        assert!(events.is_empty());
    }
}
