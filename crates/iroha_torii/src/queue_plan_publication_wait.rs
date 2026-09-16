//! Retain exact admission work through a State publication overlap without renewing its deadline.
use std::time::{Duration, Instant};

use iroha_core::state::{
    MergeLedgerCommitError, PendingQueuePlanAdmissionPersistenceOutcome,
    QueuePlanAdmissionPersistenceScope, State,
};

/// Original ingress deadline, bounded by both its wire clock and local elapsed time.
pub(super) struct PersistenceDeadline {
    started: Instant,
    deadline_unix_ms: u64,
    #[cfg(test)]
    pub(super) observed_wait_height: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl PersistenceDeadline {
    pub(super) fn new(started: Instant, deadline_unix_ms: u64) -> Self {
        Self {
            started,
            deadline_unix_ms,
            #[cfg(test)]
            observed_wait_height: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    pub(super) fn remaining(&self) -> Result<Duration, &'static str> {
        let absolute = super::validate_torii_proxy_deadline(self.deadline_unix_ms)?;
        let local = super::TORII_PROXY_EXECUTION_BUDGET
            .checked_sub(self.started.elapsed())
            .filter(|remaining| !remaining.is_zero())
            .ok_or("original Torii proxy execution budget expired during State publication")?;
        Ok(absolute.min(local))
    }

    /// The caller's W owner stays on the same future stack through every physical attempt.
    pub(super) async fn persist(
        &self,
        state: &State,
        complete_input: &[u8],
    ) -> Result<PendingQueuePlanAdmissionPersistenceOutcome, String> {
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| "QueuePlan persistence requires an active Tokio runtime".to_owned())?;
        if runtime.runtime_flavor() != tokio::runtime::RuntimeFlavor::MultiThread {
            return Err("QueuePlan persistence requires a multi-thread Tokio runtime".to_owned());
        }
        loop {
            self.remaining().map_err(str::to_owned)?;
            // This does not spawn a writer: cancellation cannot detach physical
            // persistence from the complete input or its enclosing W reservation.
            let outcome = tokio::task::block_in_place(|| {
                state.persist_classified_queue_plan_admission(
                    complete_input,
                    QueuePlanAdmissionPersistenceScope::Admission,
                )
            });
            let error = match outcome {
                Ok(outcome) => return Ok(outcome),
                Err(error) => error,
            };
            let Some(required_height) = publication_overlap_height(&error) else {
                return Err(error.to_string());
            };
            let remaining = self.remaining().map_err(str::to_owned)?;
            #[cfg(test)]
            self.observed_wait_height
                .store(required_height, std::sync::atomic::Ordering::Release);
            tokio::time::timeout(remaining, state.wait_for_committed_height(required_height))
                .await
                .map_err(|_| {
                    "original Torii proxy deadline expired awaiting State publication".to_owned()
                })?;
            // A wakeup grants no persistence authority. Re-enter the exact State
            // classification and Kura height fence with the same original bytes.
        }
    }
}

fn publication_overlap_height(error: &MergeLedgerCommitError) -> Option<u64> {
    match error {
        MergeLedgerCommitError::Persistence(
            iroha_core::kura::Error::QueuePlanAdmissionDurableHeightMismatch {
                expected_durable_height,
                actual_durable_height,
            },
        ) if expected_durable_height.checked_add(1) == Some(*actual_durable_height) => {
            Some(*actual_durable_height)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
