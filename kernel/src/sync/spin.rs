// Lightweight spinlock used by per-data-structure critical sections.

use std::sync::atomic::{AtomicBool, Ordering};

pub struct Spin {
    pub(crate) v: AtomicBool,
}

impl Spin {
    pub const fn new() -> Self { Self { v: AtomicBool::new(false) } }

    pub fn acquire(&self) {
        while self
            .v
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
    }

    pub fn try_acquire(&self) -> bool {
        self.v
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    pub fn release(&self) { self.v.store(false, Ordering::Release); }
    pub fn is_held(&self) -> bool { self.v.load(Ordering::Relaxed) }
}

unsafe impl Send for Spin {}
unsafe impl Sync for Spin {}
