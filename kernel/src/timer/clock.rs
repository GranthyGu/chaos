// Global wall/total tick counters and convenience accessors.

use std::sync::atomic::{AtomicUsize, Ordering};
use crate::config::USEC_TICK;

pub static CLK: AtomicUsize = AtomicUsize::new(0);
pub static CLK_ALL: AtomicUsize = AtomicUsize::new(0);

pub fn wclk() -> usize { CLK.load(Ordering::Relaxed) }
pub fn cclk() -> usize { CLK_ALL.load(Ordering::Relaxed) }

pub fn dtk(cpu_id: usize) {
    if cpu_id == 0 { CLK.fetch_add(1, Ordering::Relaxed); }
    CLK_ALL.fetch_add(1, Ordering::Relaxed);
}

pub fn up_ms() -> usize { wclk() * USEC_TICK / 1000 }
pub fn tmr(cpu_id: usize) { dtk(cpu_id); }
pub fn ser(c: u8) -> u8 { if c == b'\r' { b'\n' } else { c } }
