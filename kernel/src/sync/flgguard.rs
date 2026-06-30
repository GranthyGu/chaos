// FlgGuard — RAII nest-depth tracker.
//
// Each `enter()` increments a global counter; the returned guard's drop
// decrements it. `self.0` records the depth at entry, so the outermost
// guard can identify itself via `is_outermost()`.

use std::sync::atomic::{AtomicUsize, Ordering};

static FLG_DEPTH: AtomicUsize = AtomicUsize::new(0);

pub struct FlgGuard(usize);

impl FlgGuard {
    pub fn enter() -> Self {
        let prev = FLG_DEPTH.fetch_add(1, Ordering::Acquire);
        Self(prev)
    }
    pub fn depth(&self) -> usize { self.0 + 1 }
    pub fn is_outermost(&self) -> bool { self.0 == 0 }
}

impl Drop for FlgGuard {
    fn drop(&mut self) {
        FLG_DEPTH.fetch_sub(1, Ordering::Release);
    }
}
