//! Async DA spool batching for Torii ingest persistence.

use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use iroha_logger::warn;
use tokio::sync::{mpsc, oneshot};

use super::ReceiptInsertOutcome;
use crate::routing::MaybeTelemetry;

const OUTCOME_OK: &str = "ok";
const OUTCOME_PARTIAL_ERROR: &str = "partial_error";
const OUTCOME_ERROR: &str = "error";
const KIND_WORKER: &str = "worker";

/// Result payload emitted by a DA spool action.
pub(crate) enum DaSpoolActionOutput {
    /// The action has no handler-visible output.
    None,
    /// The action appended to the durable receipt log.
    ReceiptOutcome(ReceiptInsertOutcome),
}

/// One synchronous persistence action executed by the DA spool worker.
pub(crate) struct DaSpoolAction {
    kind: &'static str,
    run: Box<dyn FnOnce() -> Result<DaSpoolActionOutput, String> + Send + 'static>,
}

impl DaSpoolAction {
    /// Build a spool action with an operator-facing artifact kind label.
    pub(crate) fn new<F>(kind: &'static str, run: F) -> Self
    where
        F: FnOnce() -> Result<DaSpoolActionOutput, String> + Send + 'static,
    {
        Self {
            kind,
            run: Box::new(run),
        }
    }

    fn execute(self) -> DaSpoolActionReport {
        let kind = self.kind;
        let run = self.run;
        match catch_unwind(AssertUnwindSafe(move || (run)())) {
            Ok(Ok(output)) => DaSpoolActionReport {
                kind,
                outcome: DaSpoolActionOutcome::Ok,
                error: None,
                output: Some(output),
            },
            Ok(Err(error)) => DaSpoolActionReport {
                kind,
                outcome: DaSpoolActionOutcome::Error,
                error: Some(error),
                output: None,
            },
            Err(payload) => DaSpoolActionReport {
                kind,
                outcome: DaSpoolActionOutcome::Error,
                error: Some(format!(
                    "DA spool action `{kind}` panicked: {}",
                    panic_payload_message(payload.as_ref())
                )),
                output: None,
            },
        }
    }
}

fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

/// A handler-visible report for one spool action.
pub(crate) struct DaSpoolActionReport {
    kind: &'static str,
    outcome: DaSpoolActionOutcome,
    error: Option<String>,
    output: Option<DaSpoolActionOutput>,
}

impl DaSpoolActionReport {
    /// Artifact kind label for logs and metrics.
    pub(crate) fn kind(&self) -> &'static str {
        self.kind
    }

    /// Outcome label for logs and metrics.
    pub(crate) fn outcome_label(&self) -> &'static str {
        self.outcome.label()
    }

    /// Error text when the action failed.
    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Handler-visible action output.
    pub(crate) fn output(&self) -> Option<&DaSpoolActionOutput> {
        self.output.as_ref()
    }
}

#[derive(Clone, Copy)]
enum DaSpoolActionOutcome {
    Ok,
    Error,
}

impl DaSpoolActionOutcome {
    const fn label(self) -> &'static str {
        match self {
            Self::Ok => OUTCOME_OK,
            Self::Error => OUTCOME_ERROR,
        }
    }
}

/// A batch of DA persistence actions that should complete before the ingest response is returned.
#[derive(Default)]
pub(crate) struct DaSpoolBatch {
    actions: Vec<DaSpoolAction>,
}

impl DaSpoolBatch {
    /// Create an empty batch.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Append one persistence action.
    pub(crate) fn push(&mut self, action: DaSpoolAction) {
        self.actions.push(action);
    }

    /// Whether the batch has no actions.
    pub(crate) fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Execute the batch synchronously on the current thread.
    pub(crate) fn execute_sync(self) -> DaSpoolBatchReport {
        let started_at = Instant::now();
        let action_reports = self
            .actions
            .into_iter()
            .map(DaSpoolAction::execute)
            .collect();
        DaSpoolBatchReport {
            action_reports,
            write_duration: started_at.elapsed(),
        }
    }
}

/// Handler-visible result of a DA spool batch.
pub(crate) struct DaSpoolBatchReport {
    action_reports: Vec<DaSpoolActionReport>,
    write_duration: Duration,
}

impl DaSpoolBatchReport {
    fn worker_error(error: String) -> Self {
        Self {
            action_reports: vec![DaSpoolActionReport {
                kind: KIND_WORKER,
                outcome: DaSpoolActionOutcome::Error,
                error: Some(error),
                output: None,
            }],
            write_duration: Duration::ZERO,
        }
    }

    /// Action reports in execution order.
    pub(crate) fn actions(&self) -> &[DaSpoolActionReport] {
        &self.action_reports
    }

    fn batch_outcome_label(&self) -> &'static str {
        if self
            .action_reports
            .iter()
            .any(|report| matches!(report.outcome, DaSpoolActionOutcome::Error))
        {
            OUTCOME_PARTIAL_ERROR
        } else {
            OUTCOME_OK
        }
    }

    fn write_ms(&self) -> f64 {
        self.write_duration.as_secs_f64() * 1_000.0
    }
}

struct DaSpoolJob {
    batch: DaSpoolBatch,
    ack: oneshot::Sender<DaSpoolBatchReport>,
}

/// Bounded async worker that batches DA spool writes onto blocking threads.
pub(crate) struct DaSpooler {
    tx: mpsc::Sender<DaSpoolJob>,
    depth: Arc<AtomicUsize>,
    telemetry: MaybeTelemetry,
}

impl DaSpooler {
    /// Spawn a DA spool worker.
    pub(crate) fn spawn(
        queue_capacity: std::num::NonZeroUsize,
        batch_max: std::num::NonZeroUsize,
        telemetry: MaybeTelemetry,
    ) -> Arc<Self> {
        let (tx, rx) = mpsc::channel(queue_capacity.get());
        let depth = Arc::new(AtomicUsize::new(0));
        tokio::spawn(Self::run(
            rx,
            batch_max.get().max(1),
            Arc::clone(&depth),
            telemetry.clone(),
        ));
        Arc::new(Self {
            tx,
            depth,
            telemetry,
        })
    }

    /// Submit a batch and wait for the worker acknowledgement.
    pub(crate) async fn submit(&self, batch: DaSpoolBatch) -> DaSpoolBatchReport {
        if batch.is_empty() {
            let report = batch.execute_sync();
            Self::record_report(&self.telemetry, &report);
            return report;
        }

        let queued_depth = self.depth.fetch_add(1, Ordering::AcqRel).saturating_add(1);
        self.record_queue_depth(queued_depth);
        let (ack, ack_rx) = oneshot::channel();
        match self.tx.send(DaSpoolJob { batch, ack }).await {
            Ok(()) => match ack_rx.await {
                Ok(report) => report,
                Err(err) => {
                    let report = DaSpoolBatchReport::worker_error(format!(
                        "DA spool worker dropped acknowledgement: {err}"
                    ));
                    Self::record_report(&self.telemetry, &report);
                    report
                }
            },
            Err(err) => {
                let restored_depth = self.depth.fetch_sub(1, Ordering::AcqRel).saturating_sub(1);
                self.record_queue_depth(restored_depth);
                let report = err.0.batch.execute_sync();
                Self::record_report(&self.telemetry, &report);
                report
            }
        }
    }

    async fn run(
        mut rx: mpsc::Receiver<DaSpoolJob>,
        batch_max: usize,
        depth: Arc<AtomicUsize>,
        telemetry: MaybeTelemetry,
    ) {
        while let Some(first) = rx.recv().await {
            let mut jobs = Vec::with_capacity(batch_max);
            jobs.push(first);
            while jobs.len() < batch_max {
                match rx.try_recv() {
                    Ok(job) => jobs.push(job),
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => break,
                }
            }

            let drained = jobs.len();
            let depth_after = depth
                .fetch_sub(drained, Ordering::AcqRel)
                .saturating_sub(drained);
            Self::record_queue_depth_for(&telemetry, depth_after);

            let reports = tokio::task::spawn_blocking(move || {
                jobs.into_iter()
                    .map(|job| (job.ack, job.batch.execute_sync()))
                    .collect::<Vec<_>>()
            })
            .await;

            match reports {
                Ok(reports) => {
                    for (ack, report) in reports {
                        Self::record_report(&telemetry, &report);
                        let _ = ack.send(report);
                    }
                }
                Err(err) => {
                    warn!(?err, "DA spool worker join failed");
                }
            }
        }
        Self::record_queue_depth_for(&telemetry, 0);
    }

    fn record_queue_depth(&self, depth: usize) {
        Self::record_queue_depth_for(&self.telemetry, depth);
    }

    fn record_queue_depth_for(telemetry: &MaybeTelemetry, depth: usize) {
        if !telemetry.is_enabled() {
            return;
        }
        telemetry.with_metrics(|handle| {
            handle.set_torii_da_spool_queue_depth(u64::try_from(depth).unwrap_or(u64::MAX));
        });
    }

    fn record_report(telemetry: &MaybeTelemetry, report: &DaSpoolBatchReport) {
        if !telemetry.is_enabled() {
            return;
        }
        telemetry.with_metrics(|handle| {
            handle.record_torii_da_spool_batch(report.batch_outcome_label(), report.write_ms());
            for action in report.actions() {
                handle.record_torii_da_spool_artifact(action.kind(), action.outcome_label(), 1);
            }
        });
    }
}
