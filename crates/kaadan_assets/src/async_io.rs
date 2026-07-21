//! Background loading worker used by [`crate::AssetServer::load_async`].
//!
//! A single dedicated worker thread reads bytes via the (shared, `Send + Sync`)
//! resolver and runs the type-specific loader off the caller's thread. Each job
//! produces a type-erased [`Completed`] closure that, when applied on the main
//! thread inside `poll`, routes the finished asset (or failure) into the
//! correct typed storage. Capturing the typed work in the closure keeps the
//! channel itself fully type-erased.

use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;

use crate::server::AssetServer;

/// A unit of work executed on the worker thread. It is fully self-contained:
/// it already captured the resolver, loader, path and reserved handle, so the
/// worker just calls it and forwards the produced [`Completed`].
pub(crate) type Job = Box<dyn FnOnce() -> Completed + Send>;

/// The result of a job, applied back on the owning thread in `poll`. It carries
/// the typed handle + asset (or failure) inside the closure, so routing into the
/// right `AssetStorage<T>` happens without any dynamic type juggling here.
pub(crate) type Completed = Box<dyn FnOnce(&mut AssetServer) + Send>;

/// A single-thread background loader. Jobs are submitted over `job_tx` and their
/// results come back over `result_rx`, drained by [`AssetServer::poll`].
pub(crate) struct AssetWorker {
    job_tx: Option<Sender<Job>>,
    result_rx: Receiver<Completed>,
    handle: Option<JoinHandle<()>>,
}

impl AssetWorker {
    pub(crate) fn new() -> Self {
        let (job_tx, job_rx) = std::sync::mpsc::channel::<Job>();
        let (result_tx, result_rx) = std::sync::mpsc::channel::<Completed>();

        let handle = std::thread::Builder::new()
            .name("kaadan-asset-loader".into())
            .spawn(move || {
                // Runs until the job sender is dropped (server dropped).
                while let Ok(job) = job_rx.recv() {
                    let completed = job();
                    if result_tx.send(completed).is_err() {
                        // Receiver gone; nothing left to do.
                        break;
                    }
                }
            })
            .expect("failed to spawn asset loader thread");

        Self {
            job_tx: Some(job_tx),
            result_rx,
            handle: Some(handle),
        }
    }

    /// Queue a job for background execution.
    pub(crate) fn submit(&self, job: Job) {
        if let Some(tx) = &self.job_tx {
            // If the worker thread has died there is nothing sensible to do; the
            // asset simply stays Queued. This should not happen in practice.
            let _ = tx.send(job);
        }
    }

    /// Drain all currently-finished jobs (non-blocking).
    pub(crate) fn drain_completed(&self) -> Vec<Completed> {
        let mut out = Vec::new();
        while let Ok(completed) = self.result_rx.try_recv() {
            out.push(completed);
        }
        out
    }
}

impl Drop for AssetWorker {
    fn drop(&mut self) {
        // Dropping the sender lets the worker's `recv` loop terminate.
        self.job_tx = None;
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
