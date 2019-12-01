use crossbeam_channel::{self, Receiver, Sender};
use num_cpus;
use rayon::{ThreadPool, ThreadPoolBuilder};
use smallvec::SmallVec;
use std::{
    cmp,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::error::{Error, Result};

#[derive(Debug)]
pub enum Poll<T> {
    Pending(JobId),
    Ready(T),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JobId(usize);

pub struct JobResult<T> {
    pub id: JobId,
    pub payload: T,
}

#[derive(Debug)]
pub struct JobPool<T> {
    thread_pool: ThreadPool,
    next_job_id: AtomicUsize,
    sender: Sender<JobResult<T>>,
    pub receiver: Receiver<JobResult<T>>,
}

impl<T: Send + 'static> JobPool<T> {
    pub fn new() -> Result<Self> {
        // By default, leave two cpus unused, so there's no contention with the
        // drawing thread + allow other programs to make progress even if the
        // job pool is 100% used.
        let num_threads = cmp::max(1, num_cpus::get().saturating_sub(2));
        let (sender, receiver) = crossbeam_channel::bounded(3200);
        Ok(Self {
            thread_pool: ThreadPoolBuilder::new()
                .num_threads(num_threads)
                .build()
                .map_err(|err| Error::TaskPool(Box::new(err)))?,
            next_job_id: AtomicUsize::new(0),
            sender,
            receiver,
        })
    }

    pub fn spawn<JobT>(&self, job: JobT) -> Result<JobId>
    where
        JobT: FnOnce() -> T + Send + 'static,
    {
        let id = JobId(self.next_job_id.fetch_add(1, Ordering::SeqCst));
        let sender = self.sender.clone();
        self.thread_pool
            .spawn(move || sender.send(JobResult { id, payload: job() }).unwrap());
        Ok(id)
    }

    pub fn scheduler(&self) -> Scheduler<T> {
        Scheduler {
            pool: self,
            scheduled: SmallVec::new(),
        }
    }
}

#[derive(Debug)]
pub struct Scheduler<'a, T> {
    pool: &'a JobPool<T>,
    scheduled: SmallVec<[JobId; 2]>,
}

impl<'a, T: Send + 'static> Scheduler<'a, T> {
    pub fn spawn<JobT>(&mut self, job: JobT) -> Result<JobId>
    where
        JobT: FnOnce() -> T + Send + 'static,
    {
        let job_id = self.pool.spawn(job);
        if let Ok(job_id) = job_id.as_ref() {
            self.scheduled.push(*job_id);
        }
        job_id
    }

    pub fn scheduled(self) -> impl IntoIterator<Item = JobId> {
        self.scheduled.into_iter()
    }
}
