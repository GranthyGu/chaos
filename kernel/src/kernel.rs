#![allow(unused, dead_code, non_upper_case_globals, non_camel_case_types, unused_assignments, unused_mut)]

// Root module for the Chaos teaching kernel.
//
// `kernel.rs` is the symlink target seen by `chaos-tests/src/lib.rs`, so
// public items declared (or re-exported) here become the public crate API
// that the test binaries consume via `use chaos_tests::*;`.
//
// All real code lives in submodules; this file only declares them and
// `pub use`s their contents at the crate root.

pub mod config;
pub mod util;
pub mod sync;
pub mod timer;
pub mod trap;
pub mod mm;
pub mod fs;
pub mod net;
pub mod ipc;
pub mod task;
pub mod sched;
pub mod proc;
pub mod syscall;
pub mod kernel_core;

pub use config::*;
pub use util::*;
pub use sync::*;
pub use timer::*;
pub use trap::*;
pub use mm::*;
pub use fs::*;
pub use net::*;
pub use ipc::*;
pub use task::*;
pub use sched::*;
pub use proc::*;
pub use syscall::*;
pub use kernel_core::*;
