use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::timer::CLK;

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

#[derive(Clone, Debug)]
pub struct AuditRecord {
    pub seq: usize,
    pub timestamp: usize,
    pub pid: usize,
    pub op: AuditOp,
    pub fd: Option<usize>,
    pub path: Option<String>,
    pub result: AuditResult,
    pub bytes: Option<usize>,
}

pub struct AuditDraft {
    seq: usize,
    timestamp: usize,
    pid: usize,
    op: AuditOp,
    fd: Option<usize>,
    path: Option<String>,
}

pub struct AuditLog {
    records: Mutex<VecDeque<AuditRecord>>,
    enabled: AtomicBool,
    capacity: usize,
    dropped: AtomicUsize,
    seq_next: AtomicUsize,
}

impl AuditLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            records: Mutex::new(VecDeque::with_capacity(capacity.min(1024))),
            enabled: AtomicBool::new(false),
            capacity,
            dropped: AtomicUsize::new(0),
            seq_next: AtomicUsize::new(0),
        }
    }

    pub fn enable(&self) { self.enabled.store(true, Ordering::Release); }
    pub fn disable(&self) { self.enabled.store(false, Ordering::Release); }
    pub fn is_enabled(&self) -> bool { self.enabled.load(Ordering::Relaxed) }

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
            q.pop_front();
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
        q.push_back(record);
    }

    pub fn count(&self) -> usize { self.records.lock().unwrap().len() }
    pub fn dropped(&self) -> usize { self.dropped.load(Ordering::Relaxed) }

    pub fn drain(&self) -> Vec<AuditRecord> {
        let mut q = self.records.lock().unwrap();
        let taken: VecDeque<AuditRecord> =
            std::mem::replace(&mut *q, VecDeque::with_capacity(self.capacity.min(1024)));
        taken.into_iter().collect()
    }

    pub fn snapshot(&self) -> Vec<AuditRecord> {
        self.records.lock().unwrap().iter().cloned().collect()
    }

    pub fn count_by_op(&self, op: AuditOp) -> usize {
        self.records.lock().unwrap().iter().filter(|r| r.op == op).count()
    }

    pub fn find_by_pid(&self, pid: usize) -> Vec<AuditRecord> {
        self.records
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.pid == pid)
            .cloned()
            .collect()
    }

    pub fn reset(&self) {
        self.records.lock().unwrap().clear();
        self.dropped.store(0, Ordering::Relaxed);
        self.seq_next.store(0, Ordering::Relaxed);
    }
}
