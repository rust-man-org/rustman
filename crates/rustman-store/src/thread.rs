use std::io;
use std::sync::mpsc;
use std::thread;

use rustman_core::store::StoreError;
use tokio::sync::oneshot;

use crate::sqlite::SqliteStore;

type Job = Box<dyn FnOnce(&mut SqliteStore) + Send>;

/// A thread that owns a store and runs jobs on it one at a time, in the
/// order they were sent, so callers never wait on SQLite.
///
/// The thread ends when the last handle is dropped. A job that panics ends
/// it early; every job after that fails with an error rather than vanishing.
#[derive(Debug, Clone)]
pub struct StoreThread {
    jobs: mpsc::Sender<Job>,
}

impl StoreThread {
    pub fn spawn(store: SqliteStore) -> io::Result<Self> {
        let (jobs, queue) = mpsc::channel::<Job>();
        thread::Builder::new()
            .name("rustman-store".to_owned())
            .spawn(move || {
                let mut store = store;
                for job in queue {
                    job(&mut store);
                }
            })?;
        Ok(Self { jobs })
    }

    /// Queues `job` at once and resolves with what it returns. Dropping the
    /// future doesn't cancel the job, so a write can be fired and forgotten.
    pub fn run<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut SqliteStore) -> Result<T, StoreError> + Send + 'static,
    ) -> impl Future<Output = Result<T, StoreError>> + Send + 'static {
        let (reply, answer) = oneshot::channel();
        let queued = self
            .jobs
            .send(Box::new(move |store: &mut SqliteStore| {
                // The caller may have stopped waiting; the job ran all the same.
                let _ = reply.send(job(store));
            }))
            .is_ok();
        async move {
            if !queued {
                return Err(stopped());
            }
            answer.await.map_err(|_| stopped())?
        }
    }
}

impl StoreThread {
    /// [`run`](Self::run), waiting on the calling thread: for the last
    /// session save before the process exits, and for tests. Never call it
    /// from a job, which would wait on itself.
    pub fn run_blocking<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut SqliteStore) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, StoreError> {
        let (reply, answer) = mpsc::sync_channel(1);
        self.jobs
            .send(Box::new(move |store: &mut SqliteStore| {
                // The caller may have stopped waiting; the job ran all the same.
                let _ = reply.send(job(store));
            }))
            .map_err(|_| stopped())?;
        answer.recv().map_err(|_| stopped())?
    }
}

fn stopped() -> StoreError {
    StoreError::new("the storage thread has stopped")
}
