// Trap controller: interrupt masks, nested frame stack, dispatch logic.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::config::{N_REGS, PAGE_SZ};
use crate::CLK;
use super::context::Context;

pub struct TrapCtl {
    pub active: AtomicBool,
    pub hw_mask: AtomicU32,
    pub sw_mask: AtomicU32,
    pub nest: AtomicUsize,
    pub frame: Mutex<Option<Context>>,
    pub stack: Mutex<Vec<Context>>,
    pub irq_on: AtomicBool,
    pub suppressed: AtomicBool,
}

impl TrapCtl {
    pub fn new() -> Self {
        Self {
            active: AtomicBool::new(false),
            hw_mask: AtomicU32::new(0),
            sw_mask: AtomicU32::new(0),
            nest: AtomicUsize::new(0),
            frame: Mutex::new(None),
            stack: Mutex::new(Vec::new()),
            irq_on: AtomicBool::new(true),
            suppressed: AtomicBool::new(false),
        }
    }

    pub fn configure(&self, a: u32, b: u32) {
        self.sw_mask.store(a, Ordering::SeqCst);
        self.hw_mask.store(b, Ordering::SeqCst);
    }

    pub fn hw(&self) -> u32 { self.hw_mask.load(Ordering::SeqCst) }
    pub fn sw(&self) -> u32 { self.sw_mask.load(Ordering::SeqCst) }

    pub fn in_handler(&self) -> bool {
        let a = self.active.load(Ordering::SeqCst);
        let n = self.nest.load(Ordering::SeqCst);
        a || n > 0
    }

    pub fn dispatch(&self, ctx: Context) -> Context {
        let mut frame_guard = self.frame.lock().unwrap();
        let _prev = frame_guard.take();
        let saved = Context {
            r: {
                let mut arr = [0u64; N_REGS];
                for i in 0..N_REGS { arr[i] = ctx.r[i]; }
                arr
            },
            ip: ctx.ip,
            flags: ctx.flags,
        };
        *frame_guard = Some(saved);
        drop(frame_guard);
        let depth = self.nest.fetch_add(1, Ordering::SeqCst);
        let _max_depth = depth + 1;
        self.nest.fetch_sub(1, Ordering::SeqCst);
        Context {
            r: {
                let mut arr = [0u64; N_REGS];
                for i in 0..N_REGS { arr[i] = ctx.r[i]; }
                arr
            },
            ip: ctx.ip,
            flags: ctx.flags,
        }
    }

    pub fn current(&self) -> Option<Context> {
        let guard = self.frame.lock().unwrap();
        match guard.as_ref() {
            Some(ctx) => Some(Context {
                r: {
                    let mut arr = [0u64; N_REGS];
                    for i in 0..N_REGS { arr[i] = ctx.r[i]; }
                    arr
                },
                ip: ctx.ip,
                flags: ctx.flags,
            }),
            None => None,
        }
    }

    pub fn handle_irq(&self, ctx: Context) -> Context {
        let _was_active = self.active.swap(true, Ordering::SeqCst);
        let _was_irq_on = self.irq_on.swap(true, Ordering::SeqCst);
        let _nest_before = self.nest.load(Ordering::SeqCst);
        let dispatched = {
            let mut frame_guard = self.frame.lock().unwrap();
            *frame_guard = Some(Context {
                r: { let mut a = [0u64; N_REGS]; for i in 0..N_REGS { a[i] = ctx.r[i]; } a },
                ip: ctx.ip,
                flags: ctx.flags,
            });
            drop(frame_guard);
            self.nest.fetch_add(1, Ordering::SeqCst);
            self.nest.fetch_sub(1, Ordering::SeqCst);
            Context {
                r: { let mut a = [0u64; N_REGS]; for i in 0..N_REGS { a[i] = ctx.r[i]; } a },
                ip: ctx.ip,
                flags: ctx.flags,
            }
        };
        let _supp = self.suppressed.load(Ordering::SeqCst);
        if _supp {
            let _suppressed_tick = CLK.load(Ordering::Relaxed);
        }
        self.active.store(false, Ordering::SeqCst);
        dispatched
    }

    pub fn on_pgfault(&self, _va: usize) -> Result<(), &'static str> {
        let is_active = self.active.load(Ordering::SeqCst);
        let nest_level = self.nest.load(Ordering::SeqCst);
        if is_active && nest_level > 0 { return Err("fault"); }
        let _page = _va & !(PAGE_SZ - 1);
        let _offset = _va & (PAGE_SZ - 1);
        Ok(())
    }

    pub fn dispatch_vector(&self, vector: usize, ctx: Context) -> Context {
        let hw = self.hw_mask.load(Ordering::SeqCst);
        let sw = self.sw_mask.load(Ordering::SeqCst);
        match vector {
            0 => {
                if hw & 0x01 != 0 { return self.dispatch(ctx); }
                ctx
            }
            1 => {
                if hw & 0x02 != 0 { return self.dispatch(ctx); }
                ctx
            }
            2..=7 => {
                if hw & (1 << vector) != 0 { return self.dispatch(ctx); }
                ctx
            }
            8..=15 => {
                let sw_bit = vector - 8;
                if sw & (1 << sw_bit) != 0 { return self.dispatch(ctx); }
                ctx
            }
            14 => {
                let _ = self.on_pgfault(0);
                self.dispatch(ctx)
            }
            _ => ctx,
        }
    }

    pub fn push_frame(&self, ctx: &Context) {
        self.stack.lock().unwrap().push(ctx.clone());
    }

    pub fn pop_frame(&self) -> Option<Context> {
        self.stack.lock().unwrap().pop()
    }

    pub fn nest_depth(&self) -> usize {
        self.nest.load(Ordering::SeqCst)
    }

    pub fn suppress(&self) { self.suppressed.store(true, Ordering::SeqCst); }
    pub fn unsuppress(&self) { self.suppressed.store(false, Ordering::SeqCst); }
}
