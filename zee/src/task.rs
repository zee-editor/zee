use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{
    num::NonZeroUsize,
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
        let num_threads = std::cmp::max(
            1,
            std::cmp::min(
                std::thread::available_parallelism()
                    .map(NonZeroUsize::get)
                    .unwrap_or(1)
                    .saturating_sub(2),
                MAX_NUMBER_OF_THREADS,
            ),
        );
        log::debug!("Creating a compute task pool with {} threads", num_threads);
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

const MAX_NUMBER_OF_THREADS: usize = 8;
