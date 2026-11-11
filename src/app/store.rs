//! The app's side of the storage thread. Writes are queued the moment they
//! happen, so they land in order and never block the UI. A write that fails
//! sends its message to a feed the app subscribes to ([`Store::failures`]),
//! so handlers stay as they are and their tasks are only their own.

use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, PoisonError};

use iced::Subscription;
use iced::futures::stream;
use rustman_core::store::{Change, StoreError, WorkspaceStore};
use rustman_store::{SqliteStore, StoreThread};
use tokio::sync::mpsc;

use crate::message::{AppMsg, Message};

pub(crate) struct Store {
    thread: StoreThread,
    /// `false` when the data is read-only (another window holds it, or a
    /// newer build wrote it): writes are dropped, and the status line says
    /// why.
    writable: bool,
    failed: mpsc::UnboundedSender<Message>,
    feed: Feed,
}

/// The receiving end of the failure feed, taken once by its subscription.
#[derive(Clone)]
struct Feed(Arc<Mutex<Option<mpsc::UnboundedReceiver<Message>>>>);

impl Hash for Feed {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl Store {
    pub(crate) fn spawn(store: SqliteStore, writable: bool) -> std::io::Result<Self> {
        let (failed, feed) = mpsc::unbounded_channel();
        Ok(Self {
            thread: StoreThread::spawn(store)?,
            writable,
            failed,
            feed: Feed(Arc::new(Mutex::new(Some(feed)))),
        })
    }

    pub(crate) fn writable(&self) -> bool {
        self.writable
    }

    /// Queues `changes` as one transaction.
    pub(crate) fn write(&self, changes: Vec<Change>) {
        self.write_with(move |store| store.apply(&changes), |error| {
            Message::App(AppMsg::StoreFailed(error))
        });
    }

    /// Queues `job`, a write whose failure becomes `on_failure`'s message.
    pub(crate) fn write_with(
        &self,
        job: impl FnOnce(&mut SqliteStore) -> Result<(), StoreError> + Send + 'static,
        on_failure: impl FnOnce(String) -> Message + Send + 'static,
    ) {
        if !self.writable {
            return;
        }
        let failed = self.failed.clone();
        // Queued as it is called; nothing has to wait for it.
        drop(self.thread.run(move |store| {
            if let Err(e) = job(store) {
                // The feed is gone only when the app is.
                let _ = failed.send(on_failure(e.to_string()));
            }
            Ok(())
        }));
    }

    /// Runs `job`, which only reads, after every write queued before it.
    pub(crate) fn read<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut SqliteStore) -> Result<T, StoreError> + Send + 'static,
    ) -> impl Future<Output = Result<T, String>> + Send + 'static {
        let read = self.thread.run(job);
        async move { read.await.map_err(|e| e.to_string()) }
    }

    /// Waits until every write queued so far has landed.
    pub(crate) fn flush(&self) -> Result<(), String> {
        self.thread.run_blocking(|_| Ok(())).map_err(|e| e.to_string())
    }

    /// The messages of writes that failed, as they fail.
    pub(crate) fn failures(&self) -> Subscription<Message> {
        Subscription::run_with(self.feed.clone(), |feed| {
            let feed = feed.0.lock().unwrap_or_else(PoisonError::into_inner).take();
            stream::unfold(feed, |feed| async move {
                let mut feed = feed?;
                let message = feed.recv().await?;
                Some((message, Some(feed)))
            })
        })
    }

    /// The failures of every write queued so far, for tests, which run no
    /// subscriptions.
    #[cfg(test)]
    pub(crate) fn failed_so_far(&self) -> Vec<Message> {
        self.flush().expect("queued writes land");
        let mut feed = self.feed.0.lock().unwrap_or_else(PoisonError::into_inner);
        let mut failed = Vec::new();
        while let Some(message) = feed.as_mut().and_then(|feed| feed.try_recv().ok()) {
            failed.push(message);
        }
        failed
    }

    /// Runs `job` on the store at once, waiting for it: for tests that set
    /// up or inspect the database.
    #[cfg(test)]
    pub(crate) fn now<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut SqliteStore) -> Result<T, StoreError> + Send + 'static,
    ) -> T {
        self.thread.run_blocking(job).expect("the store job runs")
    }
}

impl Drop for Store {
    /// What was queued lands before the store goes, so a state dropped to
    /// restart the app (or by a test) never races its own last writes.
    fn drop(&mut self) {
        if let Err(e) = self.flush() {
            eprintln!("rustman: queued writes were lost: {e}");
        }
    }
}
