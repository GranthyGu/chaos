// Concurrent filesystem audit log.
//
// Records `(when, who, what)` for every syscall that touches a file
// descriptor.  Storage scheme A: a single `Mutex<VecDeque<AuditRecord>>`
// bounded by `capacity`, protected by an `AtomicBool` global enable
// switch, with an atomically increasing sequence number so events keep a
// strict global order even when two syscalls hit the same `CLK` tick.
//
// Concurrency model:
//   * writers  (`push`)    hold the mutex only long enough to push_back
//                          + evict the head if capacity is exceeded
//   * consumer (`drain`)   swaps the deque out under the lock, returns
//                          the collected Vec — bounded time regardless
//                          of buffer size
//
// Fast path for callers when audit is disabled: `begin()` returns None
// and every hook is a single relaxed atomic load — zero allocation, no
// lock, no work.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::timer::CLK;

/// Which filesystem-touching syscall produced the record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditOp {
    Read,
    Write,
    Open,
    Close,
    Stat,
    Fstat,
    Mmap,
    Munmap,
    Pipe,
    Dup,
    Dup2,
    Fcntl,
}

/// Outcome the syscall returned.  `Failure` carries the errno string so
/// consumers can filter by failure type without decoding numbers.
#[derive(Clone, Copy, Debug)]
pub enum AuditResult {
    Success,
    Failure(&'static str),
}

impl AuditResult {
    pub fn is_success(&self) -> bool { matches!(self, AuditResult::Success) }
    pub fn errno(&self) -> Option<&'static str> {
        match self { AuditResult::Failure(e) => Some(*e), _ => None }
    }
}

/// One committed audit entry.
#[derive(Clone, Debug)]
pub struct AuditRecord {
    /// Strict global order.  Increases monotonically across every
    /// `push` in the whole process — two events that happened at the
    /// same `CLK` tick still get distinct `seq` values.
    pub seq: usize,
    /// `CLK` tick at the moment `begin()` was called.
    pub timestamp: usize,
    /// Task id captured at entry.  0 means "kernel had no current
    /// task" — used by tests to distinguish syscalls issued while
    /// `proc_init` has not run yet.
    pub pid: usize,
    /// Operation that produced this record.
    pub op: AuditOp,
    /// The fd argument if the syscall took one.
    pub fd: Option<usize>,
    /// Human-readable path or descriptor of the target when a syscall
    /// takes one.  chaos does not read user memory, so open() stores
    /// `"<user-addr:0x…>"` rather than the real string.
    pub path: Option<String>,
    /// Outcome captured by `end()`.
    pub result: AuditResult,
    /// Byte count for read/write, size argument for mmap, ignored for
    /// operations that do not have one.
    pub bytes: Option<usize>,
}

/// Intermediate value threaded from `begin()` through the syscall body
/// to `end()`.  Owning it forces every code path to close the audit
/// entry — a `let _ = ...` on the return of `begin()` would leak a
/// half-recorded event, so this type is intentionally not `Copy`.
pub struct AuditDraft {
    seq: usize,
    timestamp: usize,
    pid: usize,
    op: AuditOp,
    fd: Option<usize>,
    path: Option<String>,
}

/// Global audit log.
///
/// * `records`  — bounded ring buffer.  Push evicts the oldest entry
///                when the buffer is full so recent events are always
///                preserved.
/// * `enabled`  — master switch.  Off by default: the standard test
///                suite runs with audit dormant, individual tests turn
///                it on explicitly.
/// * `capacity` — maximum records held in memory at once.
/// * `dropped`  — count of records evicted because the buffer was
///                full.  Non-zero means the test overran the log.
/// * `seq_next` — the next `AuditRecord::seq` value.  Atomic so
///                callers do not need the mutex just to reserve one.
pub struct AuditLog {
    records: Mutex<VecDeque<AuditRecord>>,
    enabled: AtomicBool,
    capacity: usize,
    dropped: AtomicUsize,
    seq_next: AtomicUsize,
}

impl AuditLog {
    /// Build a fresh log with `capacity` slots.  A capacity of 0 means
    /// no records will be kept even when the switch is on — useful for
    /// benchmarking the cost of the `begin`/`end` overhead alone.
    pub fn new(capacity: usize) -> Self {
        Self {
            records: Mutex::new(VecDeque::with_capacity(capacity.min(1024))),
            enabled: AtomicBool::new(false),
            capacity,
            dropped: AtomicUsize::new(0),
            seq_next: AtomicUsize::new(0),
        }
    }

    /// Turn recording on.
    pub fn enable(&self) { self.enabled.store(true, Ordering::Release); }

    /// Turn recording off.  Pending draft entries created before the
    /// switch flipped will still commit if `end()` is called — this is
    /// deliberate so half-audited paths do not silently disappear.
    pub fn disable(&self) { self.enabled.store(false, Ordering::Release); }

    /// Current state of the switch (cheap `Relaxed` load, safe to spam
    /// from hot paths).
    pub fn is_enabled(&self) -> bool { self.enabled.load(Ordering::Relaxed) }

    /// Start an audit entry.  Returns `None` when audit is off so the
    /// caller can early-return without any allocation.  When on,
    /// captures the timestamp, pid and op-related metadata.
    pub fn begin(
        &self,
        op: AuditOp,
        pid: usize,
        fd: Option<usize>,
        path: Option<String>,
    ) -> Option<AuditDraft> {
        if !self.is_enabled() { return None; }
        Some(AuditDraft {
            seq: self.seq_next.fetch_add(1, Ordering::Relaxed),
            timestamp: CLK.load(Ordering::Relaxed),
            pid,
            op,
            fd,
            path,
        })
    }

    /// Commit an entry begun by `begin()`.  Silently no-ops when the
    /// draft is `None` (audit was off at begin time).
    pub fn end(&self, draft: Option<AuditDraft>, result: AuditResult, bytes: Option<usize>) {
        let draft = match draft {
            Some(d) => d,
            None => return,
        };
        let record = AuditRecord {
            seq: draft.seq,
            timestamp: draft.timestamp,
            pid: draft.pid,
            op: draft.op,
            fd: draft.fd,
            path: draft.path,
            result,
            bytes,
        };
        let mut q = self.records.lock().unwrap();
        if q.len() >= self.capacity {
            q.pop_front();                                                 // evict oldest
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
        q.push_back(record);
    }

    /// Number of records currently held (does not include ones already
    /// dropped due to capacity overflow).
    pub fn count(&self) -> usize { self.records.lock().unwrap().len() }

    /// Number of records evicted because the buffer was full.  Tests
    /// assert this is 0 for capacity-respecting workloads.
    pub fn dropped(&self) -> usize { self.dropped.load(Ordering::Relaxed) }

    /// Snapshot the log — the internal deque is swapped for a fresh
    /// empty one under the lock, then returned as a `Vec`.  O(1) under
    /// the lock, so concurrent writers are not blocked while the caller
    /// walks the vector.
    pub fn drain(&self) -> Vec<AuditRecord> {
        let mut q = self.records.lock().unwrap();
        let taken: VecDeque<AuditRecord> =
            std::mem::replace(&mut *q, VecDeque::with_capacity(self.capacity.min(1024)));
        taken.into_iter().collect()
    }

    /// Read-only clone of every record currently held.  Slower than
    /// `drain` but leaves the log intact — used by tests that want to
    /// look at the buffer without emptying it.
    pub fn snapshot(&self) -> Vec<AuditRecord> {
        self.records.lock().unwrap().iter().cloned().collect()
    }

    /// Count records matching a specific op.  Locks the mutex just
    /// long enough to iterate.
    pub fn count_by_op(&self, op: AuditOp) -> usize {
        self.records.lock().unwrap().iter().filter(|r| r.op == op).count()
    }

    /// Return every record produced by a given pid.  Clones the
    /// matching records so callers do not need to hold the lock.
    pub fn find_by_pid(&self, pid: usize) -> Vec<AuditRecord> {
        self.records
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.pid == pid)
            .cloned()
            .collect()
    }

    /// Reset every runtime counter back to construction-time state.
    /// The enable switch is left untouched so tests do not accidentally
    /// silence themselves between assertions.
    pub fn reset(&self) {
        self.records.lock().unwrap().clear();
        self.dropped.store(0, Ordering::Relaxed);
        self.seq_next.store(0, Ordering::Relaxed);
    }
}
