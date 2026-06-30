// Per-task kernel stack. Heap-allocated; freed on Drop.

use crate::config::KSTK_SZ;

pub struct KStk(usize);

impl KStk {
    pub fn new() -> Self {
        let v = vec![0u8; KSTK_SZ].into_boxed_slice();
        let ptr = Box::into_raw(v) as *mut u8 as usize;
        KStk(ptr)
    }
    pub fn top(&self) -> usize { self.0 + KSTK_SZ }
}

impl Drop for KStk {
    fn drop(&mut self) {
        unsafe {
            let _ = Box::from_raw(std::slice::from_raw_parts_mut(self.0 as *mut u8, KSTK_SZ));
        }
    }
}
