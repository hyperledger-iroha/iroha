//! QC aggregate-signature verification workers.

use std::sync::mpsc;

use iroha_logger::prelude::*;

use super::*;

/// QC aggregate verification request payload.
#[derive(Debug)]
pub(super) struct QcVerifyWork {
    pub(super) id: u64,
    pub(super) key: QcVerifyKey,
    pub(super) inputs: QcAggregateInputs,
}

/// QC aggregate verification result payload.
#[derive(Debug)]
pub(super) struct QcVerifyResult {
    pub(super) id: u64,
    pub(super) key: QcVerifyKey,
    pub(super) aggregate_ok: bool,
}

/// Spawn handle for QC aggregate verification workers.
#[derive(Debug)]
pub(super) struct QcVerifyWorkerHandle {
    pub(super) work_txs: Vec<mpsc::SyncSender<QcVerifyWork>>,
    pub(super) result_rx: mpsc::Receiver<QcVerifyResult>,
    pub(super) join_handles: Vec<std::thread::JoinHandle<()>>,
}

/// Spawn QC aggregate verification workers.
pub(super) fn spawn_qc_verify_workers(
    wake_tx: Option<mpsc::SyncSender<()>>,
    worker_threads: usize,
    work_queue_cap: usize,
    result_queue_cap: usize,
) -> QcVerifyWorkerHandle {
    let threads = worker_threads.max(1);
    let work_queue_cap = work_queue_cap.max(1);
    let result_queue_cap = result_queue_cap.max(1);
    let (result_tx, result_rx) = mpsc::sync_channel::<QcVerifyResult>(result_queue_cap);
    let mut work_txs = Vec::with_capacity(threads);
    let mut join_handles = Vec::with_capacity(threads);
    for idx in 0..threads {
        let (work_tx, work_rx) = mpsc::sync_channel::<QcVerifyWork>(work_queue_cap);
        work_txs.push(work_tx);
        let result_tx = result_tx.clone();
        let wake_tx = wake_tx.clone();
        let name = format!("sumeragi-qc-verify-{idx}");
        let join_handle = std::thread::Builder::new()
            .name(name)
            .spawn(move || {
                while let Ok(work) = work_rx.recv() {
                    let QcVerifyWork { id, key, inputs } = work;
                    let aggregate_ok = inputs.verify();
                    if result_tx
                        .send(QcVerifyResult {
                            id,
                            key,
                            aggregate_ok,
                        })
                        .is_err()
                    {
                        break;
                    }
                    if let Some(wake) = wake_tx.as_ref() {
                        let _ = wake.try_send(());
                    }
                }
            })
            .expect("failed to spawn sumeragi QC verify worker thread");
        join_handles.push(join_handle);
    }

    QcVerifyWorkerHandle {
        work_txs,
        result_rx,
        join_handles,
    }
}

impl Actor {
    pub(in crate::sumeragi) fn poll_qc_verify_results(&mut self) -> bool {
        let Some(result_rx) = self.subsystems.qc_verify.result_rx.take() else {
            return false;
        };
        let mut progress = false;
        let mut keep_rx = true;
        loop {
            match result_rx.try_recv() {
                Ok(result) => {
                    let QcVerifyResult {
                        id,
                        key,
                        aggregate_ok,
                    } = result;
                    let Some(inflight) = self.subsystems.qc_verify.inflight.remove(&key) else {
                        warn!(?key, "QC verify result received without inflight entry");
                        continue;
                    };
                    if inflight.id != id {
                        warn!(
                            ?key,
                            inflight_id = inflight.id,
                            result_id = id,
                            "QC verify result id mismatch; ignoring"
                        );
                        continue;
                    }
                    if let Err(err) = self.handle_qc_with_aggregate(inflight.qc, Some(aggregate_ok))
                    {
                        warn!(
                            ?err,
                            ?key,
                            "failed to apply QC after aggregate verification"
                        );
                    }
                    progress = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    warn!("QC verify result channel closed; falling back to inline verification");
                    keep_rx = false;
                    break;
                }
            }
        }
        if keep_rx {
            self.subsystems.qc_verify.result_rx = Some(result_rx);
        } else {
            self.subsystems.qc_verify.work_txs.clear();
            self.subsystems.qc_verify.inflight.clear();
        }
        progress
    }
}
