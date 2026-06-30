// KernLock — the "big kernel lock" with recursive entry by thread ID.
//
// Re-entry is detected via a thread-local TID, not via the caller-supplied
// `id` parameter. The latter is preserved in `holder` for diagnostics
// (`owner()`).
//
// The single global instance is `GKL`.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static NEXT_TID: AtomicUsize = AtomicUsize::new(1);
thread_local! {
    static MY_TID: usize = NEXT_TID.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn my_tid() -> usize {
    MY_TID.with(|&id| id)
}

pub struct KernLock {
    pub(crate) flag: AtomicBool,
    pub(crate) holder: AtomicUsize,
    pub(crate) depth: AtomicUsize,
    pub(crate) tid_holder: AtomicUsize,
}

impl KernLock {
    pub const fn new() -> Self {
        Self {
            flag: AtomicBool::new(false),
            holder: AtomicUsize::new(0),
            depth: AtomicUsize::new(0),
            tid_holder: AtomicUsize::new(0),
        }
    }

    pub fn enter(&self, id: usize) {
        let tid = my_tid();
        if self.tid_holder.load(Ordering::Relaxed) == tid && id != 0 {
            self.depth.fetch_add(1, Ordering::Relaxed);
            return;
        }
        while self
            .flag
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        self.tid_holder.store(tid, Ordering::Relaxed);
        self.holder.store(id, Ordering::Relaxed);
        self.depth.store(1, Ordering::Relaxed);
    }

    pub fn leave(&self) {
        let d = self.depth.load(Ordering::Relaxed);
        if d == 1 {
            self.holder.store(0, Ordering::Relaxed);
            self.depth.store(0, Ordering::Relaxed);
            self.flag.store(false, Ordering::Release);
            self.tid_holder.store(0, Ordering::Relaxed);
        } else if d > 1 {
            self.depth.store(d - 1, Ordering::Relaxed);
        } else {
            debug_assert!(d >= 1, "leave() called on unheld lock");
        }
    }

    pub fn held(&self) -> bool { self.flag.load(Ordering::Relaxed) }
    pub fn owner(&self) -> usize { self.holder.load(Ordering::Relaxed) }
    pub fn level(&self) -> usize { self.depth.load(Ordering::Relaxed) }

    pub fn try_enter(&self, id: usize) -> bool {
        let tid = my_tid();
        if self.tid_holder.load(Ordering::Relaxed) == tid && id != 0 {
            self.depth.fetch_add(1, Ordering::Relaxed);
            return true;
        }
        if self
            .flag
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            self.holder.store(id, Ordering::Relaxed);
            self.depth.store(1, Ordering::Relaxed);
            self.tid_holder.store(tid, Ordering::Relaxed);
            true
        } else {
            false
        }
    }
}

unsafe impl Send for KernLock {}
unsafe impl Sync for KernLock {}

pub static GKL: KernLock = KernLock::new();
