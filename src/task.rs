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
    components::{buffer::Buffer, prompt::Prompt, Component},
    error::{Error, Result},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskId(usize);

#[derive(Debug)]
pub struct TaskPool {
    thread_pool: ThreadPool,
    next_task_id: AtomicUsize,
    sender: Sender<TaskDone<TaskPayload>>,
    pub receiver: Receiver<TaskDone<TaskPayload>>,
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
        TaskFnT: FnOnce() -> PayloadT + Send + 'static,
        PayloadT: Into<TaskPayload>,
    {
        let id = TaskId(self.next_task_id.fetch_add(1, Ordering::SeqCst));
        let sender = self.sender.clone();
        self.thread_pool.spawn(move || {
            sender
                .send(TaskDone {
                    id,
                    payload: task().into(),
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

impl<'a, PayloadT: Into<TaskPayload> + Send + 'static> Scheduler<'a, PayloadT> {
    pub fn spawn<TaskFn>(&mut self, task_fn: TaskFn) -> Result<TaskId>
    where
        TaskFn: FnOnce() -> PayloadT + Send + 'static,
    {
        let task_id = self.pool.spawn(task_fn);
        if let Ok(task_id) = task_id.as_ref() {
            self.scheduled.push(*task_id);
        }
        task_id
    }

    pub fn scheduled(self) -> impl IntoIterator<Item = TaskId> {
        self.scheduled.into_iter()
    }
}

type BufferTaskPayload = <Buffer as Component>::TaskPayload;
type PromptTaskPayload = <Prompt as Component>::TaskPayload;

pub enum TaskPayload {
    Buffer(BufferTaskPayload),
    Prompt(PromptTaskPayload),
}

impl TaskPayload {
    pub fn unwrap_buffer(self) -> BufferTaskPayload {
        match self {
            TaskPayload::Buffer(task) => task,
            _ => panic!("unwrapping buffer task from different task type"),
        }
    }

    pub fn unwrap_prompt(self) -> PromptTaskPayload {
        match self {
            TaskPayload::Prompt(task) => task,
            _ => panic!("unwrapping prompt task from different task type"),
        }
    }
}

impl From<BufferTaskPayload> for TaskPayload {
    fn from(payload: BufferTaskPayload) -> Self {
        Self::Buffer(payload)
    }
}

impl From<PromptTaskPayload> for TaskPayload {
    fn from(payload: PromptTaskPayload) -> Self {
        Self::Prompt(payload)
    }
}

pub struct TaskDone<PayloadT> {
    pub id: TaskId,
    pub payload: PayloadT,
}

impl TaskDone<TaskPayload> {
    pub fn unwrap_buffer(self) -> TaskDone<BufferTaskPayload> {
        TaskDone {
            id: self.id,
            payload: self.payload.unwrap_buffer(),
        }
    }

    pub fn unwrap_prompt(self) -> TaskDone<PromptTaskPayload> {
        TaskDone {
            id: self.id,
            payload: self.payload.unwrap_prompt(),
        }
    }
}
