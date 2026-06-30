// SyncQueue — condvar-style wait queue with a pending-signal counter so
// that `signal()` calls before any waiter parks are not lost.
//
// `park_on` always re-evaluates the predicate after waking up so spurious
// wake-ups and broadcasts that do not actually satisfy the condition
// surface as `false`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::collections::VecDeque;
use std::thread;
use std::time::Duration;

pub struct RegEp {
    pub task_id: usize,
    pub epfd: usize,
    pub fd: usize,
}

pub struct SyncQueue {
    pub(crate) q: Mutex<VecDeque<thread::Thread>>,
    eq: Mutex<VecDeque<RegEp>>,
    pending: AtomicUsize,
}

impl SyncQueue {
    pub fn new() -> Self {
        Self {
            q: Mutex::new(VecDeque::new()),
            eq: Mutex::new(VecDeque::new()),
            pending: AtomicUsize::new(0),
        }
    }

    pub fn park_on<T>(&self, g: &Mutex<T>, pred: impl Fn(&T) -> bool) -> bool {
        let d = g.lock().unwrap();
        let satisfied = pred(&d);
        drop(d);
        if satisfied { return true; }

        let th = thread::current();
        let mut wq = self.q.lock().unwrap();
        loop {
            let p = self.pending.load(Ordering::Acquire);
            if p == 0 { break; }
            if self.pending
                .compare_exchange(p, p - 1, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                drop(wq);
                return true;
            }
        }
        wq.push_back(th);
        drop(wq);

        thread::park();

        let mut wq = self.q.lock().unwrap();
        let me = thread::current().id();
        if let Some(pos) = wq.iter().position(|t| t.id() == me) {
            wq.remove(pos);
        }
        drop(wq);

        let d = g.lock().unwrap();
        pred(&d)
    }

    pub fn signal(&self) {
        let mut q = self.q.lock().unwrap();
        if let Some(t) = q.pop_front() {
            drop(q);
            t.unpark();
        } else {
            drop(q);
            self.pending.fetch_add(1, Ordering::Acquire);
        }
    }

    pub fn broadcast(&self) {
        let mut q = self.q.lock().unwrap();
        let batch: Vec<thread::Thread> = q.drain(..).collect();
        drop(q);
        for t in batch { t.unpark(); }
    }

    pub fn signal_n(&self, n: usize) -> usize {
        let mut q = self.q.lock().unwrap();
        let avail = q.len();
        let to_wake = if n < avail { n } else { avail };
        let mut woken = 0;
        for _ in 0..to_wake {
            match q.pop_front() {
                Some(t) => { t.unpark(); woken += 1; }
                None => break,
            }
        }
        woken
    }

    pub fn pending(&self) -> usize { let q = self.q.lock().unwrap(); q.len() }

    pub fn wait_ev<T>(&self, g: &Mutex<T>, mut cond: impl FnMut(&T) -> Option<bool>) -> bool {
        loop {
            {
                let d = g.lock().unwrap();
                if let Some(r) = cond(&d) { return r; }
            }
            {
                let mut q = self.q.lock().unwrap();
                q.push_back(thread::current());
            }
            thread::park();
        }
    }

    pub fn wait_events<T>(
        queues: &[&SyncQueue],
        g: &Mutex<T>,
        mut cond: impl FnMut(&T) -> Option<bool>,
    ) -> bool {
        loop {
            {
                let d = g.lock().unwrap();
                if let Some(r) = cond(&d) { return r; }
            }
            for wq in queues {
                let mut q = wq.q.lock().unwrap();
                q.push_back(thread::current());
            }
            thread::park();
        }
    }

    pub fn wait_guard<T>(&self, g: &Mutex<T>) {
        {
            let mut q = self.q.lock().unwrap();
            q.push_back(thread::current());
        }
        drop(g.lock().unwrap());
        thread::park();
    }

    pub fn wait_timeout<T>(&self, g: &Mutex<T>, timeout: Duration) -> bool {
        {
            let mut q = self.q.lock().unwrap();
            q.push_back(thread::current());
        }
        drop(g.lock().unwrap());
        thread::park_timeout(timeout);
        true
    }

    pub fn reg_epoll(&self, task_id: usize, epfd: usize, fd: usize) {
        self.eq.lock().unwrap().push_back(RegEp { task_id, epfd, fd });
    }

    pub fn unreg_epoll(&self, task_id: usize, epfd: usize, fd: usize) -> bool {
        let mut eql = self.eq.lock().unwrap();
        for i in 0..eql.len() {
            if eql[i].task_id == task_id && eql[i].epfd == epfd && eql[i].fd == fd {
                eql.remove(i);
                return true;
            }
        }
        false
    }
}
