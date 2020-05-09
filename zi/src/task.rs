use crossbeam_channel::{self, Receiver, Sender};
use num_cpus;
use rayon::{ThreadPool, ThreadPoolBuilder};
use smallvec::SmallVec;
use std::{
    cmp,
    marker::PhantomData,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{
    component::template::DynamicMessage,
    error::{Error, Result},
};

#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct TaskId(usize);

#[derive(Debug)]
pub struct TaskPool {
    thread_pool: ThreadPool,
    next_task_id: AtomicUsize,
    sender: Sender<FinishedTask<DynamicMessage>>,
    pub(crate) receiver: Receiver<FinishedTask<DynamicMessage>>,
}

impl TaskPool {
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

    pub fn spawn<TaskFnT, PayloadT>(&self, task: TaskFnT) -> Result<TaskId>
    where
        TaskFnT: FnOnce(TaskId) -> PayloadT + Send + 'static,
        PayloadT: Send + 'static,
    {
        let id = TaskId(self.next_task_id.fetch_add(1, Ordering::SeqCst));
        let sender = self.sender.clone();
        self.thread_pool.spawn(move || {
            sender
                .send(FinishedTask {
                    id,
                    payload: DynamicMessage(Box::new(task(id))),
                })
                .unwrap()
        });
        Ok(id)
    }

    pub fn scheduler<PayloadT>(&self) -> Scheduler<PayloadT> {
        Scheduler {
            pool: self,
            scheduled: SmallVec::new(),
            _payload: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct Scheduler<'a, PayloadT> {
    pool: &'a TaskPool,
    scheduled: SmallVec<[TaskId; 2]>,
    _payload: PhantomData<PayloadT>,
}

impl<'a, PayloadT: Send + 'static> Scheduler<'a, PayloadT> {
    pub fn spawn<TaskFn>(&mut self, task_fn: TaskFn) -> Result<TaskId>
    where
        TaskFn: FnOnce(TaskId) -> PayloadT + Send + 'static,
    {
        let task_id = self.pool.spawn(task_fn);
        if let Ok(task_id) = task_id.as_ref() {
            self.scheduled.push(*task_id);
        }
        task_id
    }

    pub fn into_scheduled(self) -> SmallVec<[TaskId; 2]> {
        self.scheduled
    }
}

pub struct FinishedTask<PayloadT> {
    pub id: TaskId,
    pub payload: PayloadT,
}
