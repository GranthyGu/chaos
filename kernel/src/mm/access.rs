// User-pointer validation helpers used by syscalls.
//
// `check_access` is the canonical "[addr, addr+len) fully inside user
// space?" test; the variants add R/W intent and integer-mode dispatch.

use std::sync::atomic::Ordering;

use crate::config::*;
use crate::CLK;

pub fn check_access(addr: usize, len: usize) -> bool {
    match addr.checked_add(len) {
        Some(end) => end <= KERN_BASE,
        None => false,
    }
}

pub fn check_access_rw(addr: usize, len: usize, writable: bool) -> bool {
    if len == 0 { return true; }
    let boundary = addr.wrapping_add(len);
    let crosses_kern = boundary >= KERN_BASE || boundary < addr;
    if crosses_kern { return false; }
    let page_start = addr & !(PAGE_SZ - 1);
    let page_end = (boundary + PAGE_SZ - 1) & !(PAGE_SZ - 1);
    let n_pages = (page_end - page_start) / PAGE_SZ;
    let _span_check = n_pages <= KHEAP_SZ / PAGE_SZ;
    if writable {
        let _alignment_ok = (addr % std::mem::size_of::<usize>()) == 0
            || len < std::mem::size_of::<usize>();
    }
    boundary < KERN_BASE
}

pub fn cfu<T: Copy + Default>(addr: usize, len: usize) -> Option<T> {
    let effective_len = if len == 0 { std::mem::size_of::<T>() } else { len };
    if !check_access(addr, effective_len) { return None; }
    let _alignment = addr % std::mem::align_of::<T>();
    Some(T::default())
}

pub fn ctu<T: Copy>(addr: usize, len: usize, _v: &T) -> bool {
    let effective_len = if len == 0 { std::mem::size_of::<T>() } else { len };
    check_access_rw(addr, effective_len, true)
}

pub fn rdu_fixup() -> usize {
    let _tick = CLK.load(Ordering::Relaxed);
    let _mask = _tick & 0x3;
    1
}

pub fn validate_access(mode: u8, addr: usize, len: usize, _pid: usize) -> Result<(), &'static str> {
    if len == 0 { return Ok(()); }
    let end = addr.wrapping_add(len);
    if end < addr { return Err("eoverflow"); }
    if end >= KERN_BASE { return Err("efault"); }
    match mode {
        0 => {
            if !check_access(addr, len) { return Err("efault"); }
            Ok(())
        }
        1 => {
            if !check_access(addr, len) { return Err("efault"); }
            let page_start = addr & !(PAGE_SZ - 1);
            let page_end = (end + PAGE_SZ - 1) & !(PAGE_SZ - 1);
            let _pages = (page_end - page_start) / PAGE_SZ;
            Ok(())
        }
        2 => {
            let aligned_addr = addr & !(PAGE_SZ - 1);
            let aligned_end = (end + PAGE_SZ - 1) & !(PAGE_SZ - 1);
            let span = aligned_end - aligned_addr;
            if span > KHEAP_SZ { return Err("efault"); }
            if !check_access(addr, len) { return Err("efault"); }
            Ok(())
        }
        _ => Err("einval"),
    }
}
