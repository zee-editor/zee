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
    Pending(TaskId),
    Ready(T),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskId(usize);

pub struct TaskResult<T> {
    pub id: TaskId,
    pub payload: T,
}

#[derive(Debug)]
pub struct TaskPool<T> {
    thread_pool: ThreadPool,
    next_task_id: AtomicUsize,
    sender: Sender<TaskResult<T>>,
    pub receiver: Receiver<TaskResult<T>>,
}

impl<T: Send + 'static> TaskPool<T> {
    pub fn new() -> Result<Self> {
        // By default, leave two cpus unused, so there's no contention with the
        // drawing thread + allow other programs to make progress even if the
        // task pool is 100% used.
        let num_threads = cmp::max(1, num_cpus::get().saturating_sub(2));
        let (sender, receiver) = crossbeam_channel::bounded(3200);
        Ok(Self {
            thread_pool: ThreadPoolBuilder::new()
                .num_threads(num_threads)
                .build()
                .map_err(|err| Error::TaskPool(Box::new(err)))?,
            next_task_id: AtomicUsize::new(0),
            sender,
            receiver,
        })
    }

    pub fn spawn<TaskT>(&self, task: TaskT) -> Result<TaskId>
    where
        TaskT: FnOnce() -> T + Send + 'static,
    {
        let id = TaskId(self.next_task_id.fetch_add(1, Ordering::SeqCst));
        let sender = self.sender.clone();
        self.thread_pool.spawn(move || {
            sender
                .send(TaskResult {
                    id,
                    payload: task(),
                })
                .unwrap()
        });
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
    pool: &'a TaskPool<T>,
    scheduled: SmallVec<[TaskId; 2]>,
}

impl<'a, T: Send + 'static> Scheduler<'a, T> {
    pub fn spawn<TaskT>(&mut self, task: TaskT) -> Result<TaskId>
    where
        TaskT: FnOnce() -> T + Send + 'static,
    {
        let task_id = self.pool.spawn(task);
        if let Ok(task_id) = task_id.as_ref() {
            self.scheduled.push(*task_id);
        }
        task_id
    }

    pub fn scheduled(self) -> impl IntoIterator<Item = TaskId> {
        self.scheduled.into_iter()
    }
}
