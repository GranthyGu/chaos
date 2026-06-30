// Synchronization primitives.
//
//   * kernlock   - KernLock (BKL) + global GKL instance, recursive by TID
//   * spin       - Spin (one-bit spinlock for per-data-structure locks)
//   * flgguard   - FlgGuard (RAII nest-depth tracker)
//   * event      - EvFlag bitmask constants + EvBus pub-sub + wait_ev poll
//   * syncqueue  - SyncQueue with pending-signal counter, RegEp epoll entry
//   * sema       - Sema counting semaphore + SemaGuard RAII release
//   * futex      - FutexBucket + FutexTable for userspace wait/wake

pub mod kernlock;
pub mod spin;
pub mod flgguard;
pub mod event;
pub mod syncqueue;
pub mod sema;
pub mod futex;

pub use kernlock::*;
pub use spin::*;
pub use flgguard::*;
pub use event::*;
pub use syncqueue::*;
pub use sema::*;
pub use futex::*;
