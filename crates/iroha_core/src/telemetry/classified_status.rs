//! Actor-owned classified status, with bounded journal work and immutable replies.

use super::{
    Actor, BlockCommitReport, block_counts_as_non_empty, reconcile_last_reported_block_with_kura,
};
use crate::state::{
    TelemetryStatusTarget, write_telemetry_journal_prefix, write_telemetry_journal_row,
};
use iroha_crypto::Hash;
use std::num::NonZeroUsize;

/// A status response owned by the actor that classified its exact applied prefix.
#[cfg(feature = "telemetry")]
#[derive(Debug)]
pub struct OwnedStatus {
    status: iroha_torii_shared::status::Status,
    classified_height: u64,
}

#[cfg(feature = "telemetry")]
impl OwnedStatus {
    /// Consume the immutable response together with its classified State height.
    /// Runtime gauges are sampled observables; block counters and Nexus share the target.
    pub fn into_parts(self) -> (iroha_torii_shared::status::Status, u64) {
        (self.status, self.classified_height)
    }
}

/// Failure to assemble a complete, exactly bound status snapshot.
#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum StatusSnapshotError {
    /// Telemetry is not enabled for this owner.
    #[error("telemetry is disabled")]
    Disabled,
    /// The existing bounded actor mailbox cannot admit another status request.
    #[error("telemetry mailbox is unavailable")]
    MailboxUnavailable,
    /// The actor ended without a response.
    #[error("telemetry actor closed")]
    ActorClosed,
    /// The original service deadline expired; no late response is accepted.
    #[error("telemetry status deadline elapsed")]
    DeadlineElapsed,
    /// State publication was busy or no longer retained the captured prefix.
    #[error("State journal snapshot is unavailable")]
    StateUnavailable,
    /// A previously classified State checkpoint changed.
    #[error("classified State journal checkpoint changed")]
    CheckpointChanged,
    /// Kura omitted an applied block from the required prefix.
    #[error("an applied block is missing from Kura")]
    MissingBlock,
    /// The actual block sequence differs from the captured State journal.
    #[error("Kura block sequence differs from State journal")]
    JournalMismatch,
    /// A counter cannot represent the fully classified prefix.
    #[error("classified status counter overflow")]
    CounterOverflow,
    /// The shared metric counters are not the actor's committed prefix.
    #[error("classified status counters differ from the owned prefix")]
    CounterMismatch,
}

#[derive(Default)]
struct ClassifiedDelta {
    accepted: u64,
    rejected: u64,
    non_empty: u64,
    latest: Option<BlockCommitReport>,
    last_non_empty_observed_at_ms: Option<u64>,
}

fn add(total: &mut u64, value: u64) -> Result<(), StatusSnapshotError> {
    *total = total
        .checked_add(value)
        .ok_or(StatusSnapshotError::CounterOverflow)?;
    Ok(())
}

impl Actor {
    /// Finish one finite captured target even when its original HTTP waiter expires.
    /// Verified chunk progress survives; unverified chunk counters never publish.
    pub(super) async fn classify_status_target(
        &mut self,
        target: &TelemetryStatusTarget,
    ) -> Result<(), StatusSnapshotError> {
        loop {
            let chunk = self
                .state
                .telemetry_journal_chunk(target, self.last_sync_block)
                .map_err(|_| StatusSnapshotError::StateUnavailable)?;
            if chunk.checkpoint != self.last_sync_hash {
                return Err(StatusSnapshotError::CheckpointChanged);
            }
            if self.metrics.block_height.get()
                != u64::try_from(self.last_sync_block)
                    .map_err(|_| StatusSnapshotError::CounterOverflow)?
            {
                return Err(StatusSnapshotError::CounterMismatch);
            }
            if chunk.start == chunk.end {
                return Ok(());
            }
            // Copy and release the report lock before any Kura access.
            let reported = *self.last_reported_block.read().await;
            let mut delta = ClassifiedDelta::default();
            let mut failure = None;
            let digest = Hash::new_from_writer(|writer| {
                write_telemetry_journal_prefix(writer, chunk.start, chunk.end)?;
                for height in chunk.start + 1..=chunk.end {
                    let classify = || -> Result<_, StatusSnapshotError> {
                        let block = self
                            .kura
                            .get_block(
                                NonZeroUsize::new(height)
                                    .ok_or(StatusSnapshotError::JournalMismatch)?,
                            )
                            .ok_or(StatusSnapshotError::MissingBlock)?;
                        let header = block.header();
                        if header.height().get()
                            != u64::try_from(height)
                                .map_err(|_| StatusSnapshotError::CounterOverflow)?
                        {
                            return Err(StatusSnapshotError::JournalMismatch);
                        }
                        let external = block.external_transactions().len();
                        let rejected = block
                            .results()
                            .take(external)
                            .filter(|r| r.is_err())
                            .count();
                        let approved = external
                            .checked_sub(rejected)
                            .ok_or(StatusSnapshotError::CounterMismatch)?;
                        let mut report = reported
                            .filter(|r| r.height == height)
                            .unwrap_or_else(|| BlockCommitReport::new(&header, &self.time_source));
                        reconcile_last_reported_block_with_kura(
                            &mut report,
                            &header,
                            &self.time_source,
                        );
                        Ok((
                            header.hash(),
                            u64::try_from(approved)
                                .map_err(|_| StatusSnapshotError::CounterOverflow)?,
                            u64::try_from(rejected)
                                .map_err(|_| StatusSnapshotError::CounterOverflow)?,
                            block_counts_as_non_empty(block.as_ref()),
                            report,
                        ))
                    };
                    let outcome =
                        classify().and_then(|(hash, accepted, rejected, non_empty, report)| {
                            add(&mut delta.accepted, accepted)?;
                            add(&mut delta.rejected, rejected)?;
                            add(&mut delta.non_empty, u64::from(non_empty))?;
                            if non_empty {
                                delta.last_non_empty_observed_at_ms = Some(report.observed_at_ms);
                            }
                            delta.latest = Some(report);
                            Ok(hash)
                        });
                    match outcome {
                        Ok(hash) => write_telemetry_journal_row(writer, height, hash)?,
                        Err(error) => {
                            failure = Some(error);
                            return Err(std::io::Error::other(
                                "classified Kura chunk is unavailable",
                            ));
                        }
                    }
                }
                Ok(())
            });
            if let Some(error) = failure {
                return Err(error);
            }
            if digest.map_err(|_| StatusSnapshotError::JournalMismatch)? != chunk.digest {
                return Err(StatusSnapshotError::JournalMismatch);
            }
            let blocks = u64::try_from(chunk.end - chunk.start)
                .map_err(|_| StatusSnapshotError::CounterOverflow)?;
            self.metrics
                .block_height
                .get()
                .checked_add(blocks)
                .ok_or(StatusSnapshotError::CounterOverflow)?;
            let total = delta
                .accepted
                .checked_add(delta.rejected)
                .ok_or(StatusSnapshotError::CounterOverflow)?;
            for (label, increment) in [
                ("accepted", delta.accepted),
                ("rejected", delta.rejected),
                ("total", total),
            ] {
                self.metrics
                    .txs
                    .with_label_values(&[label])
                    .get()
                    .checked_add(increment)
                    .ok_or(StatusSnapshotError::CounterOverflow)?;
            }
            self.metrics
                .block_height_non_empty
                .get()
                .checked_add(delta.non_empty)
                .ok_or(StatusSnapshotError::CounterOverflow)?;
            let latest = delta.latest.ok_or(StatusSnapshotError::JournalMismatch)?;
            let commit_time_ms = u64::try_from(latest.commit_time.as_millis())
                .map_err(|_| StatusSnapshotError::CounterOverflow)?;
            // Preserve an authentic notification ahead of the captured State target.
            // This is the final await before publishing a fully verified chunk.
            {
                let mut report = self.last_reported_block.write().await;
                if report.is_none_or(|old| {
                    old.height < latest.height
                        || old.height == latest.height && old.hash != latest.hash
                }) {
                    *report = Some(latest);
                }
            }
            self.metrics
                .txs
                .with_label_values(&["accepted"])
                .inc_by(delta.accepted);
            self.metrics
                .txs
                .with_label_values(&["rejected"])
                .inc_by(delta.rejected);
            self.metrics.txs.with_label_values(&["total"]).inc_by(total);
            if delta.rejected != 0 {
                let now =
                    u64::try_from(self.time_source.get_unix_time().as_millis()).unwrap_or(u64::MAX);
                self.metrics
                    .record_rejected_transactions(delta.rejected, now);
            }
            self.metrics.block_height.inc_by(blocks);
            self.metrics.block_height_non_empty.inc_by(delta.non_empty);
            self.metrics
                .last_block_committed_at_ms
                .set(latest.observed_at_ms);
            self.metrics.last_commit_time_ms.set(commit_time_ms);
            if let Some(observed) = delta.last_non_empty_observed_at_ms {
                self.metrics
                    .last_non_empty_block_committed_at_ms
                    .set(observed);
            }
            self.last_sync_block = chunk.end;
            self.last_sync_hash = chunk.tip;
            #[cfg(test)]
            if let Some((entered, resume)) = self.status_chunk_barrier.take() {
                let _ = entered.send(chunk.end);
                let _ = resume.await;
            }
            if chunk.end == target.height {
                return Ok(());
            }
            // No State or report guard, block body, or detached worker is retained.
            tokio::task::yield_now().await;
        }
    }

    #[cfg(feature = "telemetry")]
    pub(super) fn owned_status(
        &self,
        target: TelemetryStatusTarget,
        build: &iroha_torii_shared::status::BuildStatus,
    ) -> Result<OwnedStatus, StatusSnapshotError> {
        let height =
            u64::try_from(target.height).map_err(|_| StatusSnapshotError::CounterOverflow)?;
        let mut status = self.metrics.status_snapshot(build);
        if self.last_sync_block != target.height
            || self.last_sync_hash != target.tip
            || status.blocks != height
        {
            return Err(StatusSnapshotError::CounterMismatch);
        }
        status.nexus = Some(iroha_torii_shared::status::NexusStatus::from(
            &target.routing_policy,
        ));
        Ok(OwnedStatus {
            status,
            classified_height: height,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staged_counter_overflow_preserves_the_previous_value() {
        let mut value = u64::MAX - 1;
        add(&mut value, 1).unwrap();
        assert!(matches!(
            add(&mut value, 1),
            Err(StatusSnapshotError::CounterOverflow)
        ));
        assert_eq!(value, u64::MAX);
    }
}
