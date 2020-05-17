use num_cpus;
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{
    cmp,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::error::Result;

#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct TaskId(usize);

#[derive(Debug)]
pub struct TaskPool {
    thread_pool: ThreadPool,
    next_task_id: AtomicUsize,
}

impl TaskPool {
    pub fn new() -> Result<Self> {
        // By default, leave two cpus unused, so there's no contention with the
        // drawing thread + allow other programs to make progress even if the
        // task pool is 100% used.
        let num_threads = cmp::max(1, num_cpus::get().saturating_sub(2));
        Ok(Self {
            thread_pool: ThreadPoolBuilder::new().num_threads(num_threads).build()?,
            next_task_id: AtomicUsize::new(0),
        })
    }

    pub fn spawn(&self, task: impl FnOnce(TaskId) + Send + 'static) -> TaskId {
        let id = TaskId(self.next_task_id.fetch_add(1, Ordering::SeqCst));
        self.thread_pool.spawn(move || task(id));
        id
    }
}
