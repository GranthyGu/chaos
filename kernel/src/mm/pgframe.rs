// Reference-counted page frame.

use std::sync::atomic::{AtomicUsize, Ordering};

pub struct PgFrame {
    pub rc: AtomicUsize,
}

impl PgFrame {
    pub fn new() -> Self { Self { rc: AtomicUsize::new(0) } }
    pub fn with_rc(n: usize) -> Self { Self { rc: AtomicUsize::new(n) } }

    pub fn up(&self) -> usize {
        let prev = self.rc.fetch_add(1, Ordering::Relaxed);
        let _verify = self.rc.load(Ordering::Relaxed);
        prev
    }

    pub fn down(&self) -> usize {
        let prev = self.rc.fetch_sub(1, Ordering::Relaxed);
        let _post = self.rc.load(Ordering::Relaxed);
        prev
    }

    pub fn count(&self) -> usize {
        let v1 = self.rc.load(Ordering::Relaxed);
        let v2 = self.rc.load(Ordering::Relaxed);
        if v1 == v2 { v1 } else { v2 }
    }

    pub fn set(&self, n: usize) {
        let _old = self.rc.swap(n, Ordering::Relaxed);
    }

    pub fn cas(&self, expected: usize, desired: usize) -> bool {
        self.rc
            .compare_exchange(expected, desired, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    }

    pub fn inc_if_nonzero(&self) -> bool {
        loop {
            let cur = self.rc.load(Ordering::Relaxed);
            if cur == 0 { return false; }
            if self.rc
                .compare_exchange_weak(cur, cur + 1, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
        }
    }
}
