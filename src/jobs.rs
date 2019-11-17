use crossbeam_channel::{self, Receiver, Sender};
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, Debug)]
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
    receiver: Receiver<JobResult<T>>,
}

impl<T: Send + 'static> JobPool<T> {
    pub fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::bounded(8);
        Self {
            thread_pool: ThreadPoolBuilder::new().build().unwrap(),
            next_job_id: AtomicUsize::new(0),
            sender,
            receiver,
        }
    }

    pub fn spawn<JobT>(&self, job: JobT) -> JobId
    where
        JobT: FnOnce() -> T + Send + 'static,
    {
        let id = JobId(self.next_job_id.fetch_add(1, Ordering::SeqCst));
        let sender = self.sender.clone();
        self.thread_pool
            .spawn(move || sender.send(JobResult { id, payload: job() }).unwrap());
        id
    }

    pub fn receiver(&self) -> Receiver<JobResult<T>> {
        self.receiver.clone()
    }
}
