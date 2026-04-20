use std::collections::BTreeMap;
use std::time::Duration;

/// Emits (frame_index, t) pairs for a render range.
///
/// `from` and `to` bound the time axis; `fps` determines the emit rate.
/// The emitted count is `round((to - from) * fps)`, starting from frame_index 0.
pub struct FrameScheduler {
    fps: u32,
    from: Duration,
    total_frames: u64,
    next_idx: u64,
}

impl FrameScheduler {
    pub fn new(from: Duration, to: Duration, fps: u32) -> Self {
        debug_assert!(to >= from);
        let span = (to - from).as_secs_f64();
        let total_frames = (span * fps as f64).round() as u64;
        Self { fps, from, total_frames, next_idx: 0 }
    }

    pub fn total_frames(&self) -> u64 { self.total_frames }
}

impl Iterator for FrameScheduler {
    type Item = (u64, Duration);
    fn next(&mut self) -> Option<Self::Item> {
        if self.next_idx >= self.total_frames { return None; }
        let idx = self.next_idx;
        self.next_idx += 1;
        let t = self.from + Duration::from_secs_f64(idx as f64 / self.fps as f64);
        Some((idx, t))
    }
}

/// A small capacity-bounded reorder buffer: `push(idx, bytes)` inserts into
/// a sorted store, and `drain_ready()` yields all contiguous frames starting
/// at `next_expected`. If the buffer grows past `cap`, callers should
/// block before pushing more (synchronization is the caller's problem).
pub struct ReorderBuffer {
    cap: usize,
    next_expected: u64,
    map: BTreeMap<u64, Vec<u8>>,
}

impl ReorderBuffer {
    pub fn new(cap: usize) -> Self {
        Self { cap, next_expected: 0, map: BTreeMap::new() }
    }

    pub fn len(&self) -> usize { self.map.len() }
    pub fn is_full(&self) -> bool { self.map.len() >= self.cap }
    pub fn cap(&self) -> usize { self.cap }
    pub fn next_expected(&self) -> u64 { self.next_expected }

    pub fn push(&mut self, idx: u64, bytes: Vec<u8>) {
        self.map.insert(idx, bytes);
    }

    /// Drain all contiguous frames from `next_expected` forward.
    /// Returns the buffers in order. Advances `next_expected` past them.
    pub fn drain_ready(&mut self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        loop {
            match self.map.remove(&self.next_expected) {
                Some(b) => { out.push(b); self.next_expected += 1; }
                None => break,
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_emits_correct_count() {
        let sch = FrameScheduler::new(Duration::ZERO, Duration::from_secs(2), 30);
        assert_eq!(sch.total_frames(), 60);
        let all: Vec<_> = sch.collect();
        assert_eq!(all.len(), 60);
        assert_eq!(all[0].0, 0);
        assert_eq!(all[0].1, Duration::ZERO);
        assert_eq!(all[59].0, 59);
        // Frame 59 at 30fps = 59/30 = 1.9666...s
        assert!((all[59].1.as_secs_f64() - 59.0 / 30.0).abs() < 1e-9);
    }

    #[test]
    fn scheduler_empty_range() {
        let sch = FrameScheduler::new(Duration::from_secs(5), Duration::from_secs(5), 30);
        assert_eq!(sch.total_frames(), 0);
    }

    #[test]
    fn reorder_drains_contiguous_only() {
        let mut r = ReorderBuffer::new(16);
        r.push(2, vec![2]);
        r.push(0, vec![0]);
        r.push(1, vec![1]);
        r.push(4, vec![4]);
        let flushed = r.drain_ready();
        assert_eq!(flushed, vec![vec![0], vec![1], vec![2]]);
        assert_eq!(r.next_expected(), 3);
        // Frame 3 arrives:
        r.push(3, vec![3]);
        let flushed = r.drain_ready();
        assert_eq!(flushed, vec![vec![3], vec![4]]);
        assert_eq!(r.next_expected(), 5);
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn reorder_is_full_predicate() {
        let mut r = ReorderBuffer::new(2);
        assert!(!r.is_full());
        r.push(0, vec![]);
        assert!(!r.is_full());
        r.push(1, vec![]);
        assert!(r.is_full());
    }
}
