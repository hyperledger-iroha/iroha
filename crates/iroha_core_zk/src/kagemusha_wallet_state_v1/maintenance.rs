//! Source-selected recovery and Archive fold observations without proof priority.

use super::scheduling::{ArchiveCoverageV1, ArchiveHeadV1, ArchiveResolutionV1, ArchiveTargetV1};
use super::*;

/// Current maintenance work authenticated by Native custody and the selected snapshot.
///
/// This observation is not an exchange capability or permission to dispatch an operation.
/// It is never accepted back as authority; admission must re-read current source state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaintenanceViewV1 {
    /// Authenticated Enrollment with no selected head or archive checkpoint yet.
    /// Maintenance parks; this is never permission for an ordinary fresh mutation.
    BeforeBootstrap,
    /// An operation is source-selected Pending. Recover it without idle admission gates.
    /// The view has not resumed the operation or read its frozen recovery inputs.
    SelectedPending,
    /// The current released head is ArchiveSent and its covering fold is not yet selected.
    /// The exact current head remains discoverable even after its queue entry is removed.
    AwaitingArchiveFold(Snapshot),
    /// A released head without an outstanding current Archive fold obligation.
    /// Its ordinary fold backlog may be nonzero; this variant does not grant drain readiness.
    Released(Snapshot),
}

impl<C: Custody, A: ArchiveStore, N: NativeProofs> Coordinator<C, A, N> {
    pub(super) fn selected_archive_head(
        &mut self,
    ) -> Result<(ArchiveHeadV1, Snapshot, KagemushaWalletOperationKindV1), Error> {
        let (snapshot, kind, capsule) = self.snapshot_selected()?;
        Ok((
            ArchiveHeadV1 {
                sequence: snapshot.sequence,
                head: snapshot.head,
                capsule,
            },
            snapshot,
            kind,
        ))
    }

    // A definitive exact lookup and current selected source must agree. Neither an empty
    // queue nor a generic resume result resolves an interrupted Archive reservation.
    pub(super) fn archive_resolution(
        &mut self,
        target: ArchiveTargetV1,
    ) -> Result<ArchiveResolutionV1, Error> {
        let (outcome, mapped) = self.archive_target_status(&target.request_id)?;
        match outcome {
            RequestStatusV1::Preparing => {
                let (head, _, _) = self.selected_archive_head()?;
                if head != target.source {
                    return Err(ArchiveAdmissionErrorV1::InvalidResolution.into());
                }
                Ok(ArchiveResolutionV1::Unselected { source: head })
            }
            RequestStatusV1::Outcome(Completion::Pending) => {
                let status = self.status()?;
                let SlotStatus::Pending(marker) = &status else {
                    return Err(ArchiveAdmissionErrorV1::InvalidResolution.into());
                };
                let (sequence, operation, capsule) = marker.head().ok_or(Error::NoHead)?;
                if mapped != Some(capsule) {
                    return Err(ArchiveAdmissionErrorV1::InvalidResolution.into());
                }
                let frozen = self.frozen(capsule)?;
                let body = frozen.capsule.body();
                if body.kind != KagemushaWalletOperationKindV1::ArchiveSent
                    || target.source.sequence.checked_add(1) != Some(sequence)
                    || body.statement.sequence != sequence
                    || body.operation_id != operation
                    || body.statement.predecessor.value != target.source.head
                    || body.predecessor_capsule_digest != target.source.capsule
                    || self.status()? != status
                {
                    return Err(ArchiveAdmissionErrorV1::InvalidResolution.into());
                }
                Ok(ArchiveResolutionV1::SelectedPending { capsule })
            }
            RequestStatusV1::Outcome(Completion::Complete(_)) => {
                let (head, snapshot, kind) = self.selected_archive_head()?;
                if mapped != Some(head.capsule)
                    || kind != KagemushaWalletOperationKindV1::ArchiveSent
                {
                    return Err(ArchiveAdmissionErrorV1::InvalidResolution.into());
                }
                let frozen = self.frozen(head.capsule)?;
                if frozen.capsule.body().statement.predecessor.value != target.source.head
                    || frozen.capsule.body().predecessor_capsule_digest != target.source.capsule
                {
                    return Err(ArchiveAdmissionErrorV1::InvalidResolution.into());
                }
                // Re-read the fenced head after the exact predecessor read. Metadata-only
                // publication is permitted, but another head cannot release this attempt.
                if self.selected_archive_head()?.0 != head {
                    return Err(ArchiveAdmissionErrorV1::InvalidResolution.into());
                }
                Ok(ArchiveResolutionV1::SelectedReleased {
                    head,
                    coverage: if snapshot.fold_backlog == 0 {
                        ArchiveCoverageV1::Covered
                    } else {
                        ArchiveCoverageV1::Unfolded
                    },
                })
            }
            _ => Err(Error::WitnessLost(
                "Archive attempt outcome is not retained",
            )),
        }
    }

    pub(super) fn reconcile_archive_admission(&mut self) -> Result<(), Error> {
        if let Some(observation) = self.scheduler.reconciling_archive() {
            let resolution = self.archive_resolution(observation.target())?;
            self.scheduler.reconcile_archive(&observation, resolution)?;
        }
        Ok(())
    }

    pub(super) fn require_fresh_work(&mut self) -> Result<(), Error> {
        match self.maintenance_view()? {
            MaintenanceViewV1::BeforeBootstrap => Err(Error::NoHead),
            MaintenanceViewV1::SelectedPending => Err(Error::Pending),
            MaintenanceViewV1::AwaitingArchiveFold(_) => Err(Error::FoldRequired),
            MaintenanceViewV1::Released(_) => {
                self.scheduler.require_archive_clear().map_err(Into::into)
            }
        }
    }

    /// Observe authentic pre-Bootstrap Enrollment, selected recovery or an Archive fold.
    ///
    /// No payment priority is acquired, no operation is resumed and no queue entry is
    /// removed. Empty Enrollment parks maintenance without releasing fresh work. Pending
    /// wins before snapshot/proof reads. Released observations use the same
    /// current-head, manifest, original, fold and final source fences as [`Self::snapshot`].
    /// Fresh mutation entry points enforce this observation before Payment priority. A caller
    /// waiting for it must release the wallet and requery after any scheduler wake hint.
    ///
    /// # Errors
    /// Unavailable or inconsistent custody, missing originals, invalid
    /// fold proof or a source change is an error, never a released/ready observation.
    pub fn maintenance_view(&mut self) -> Result<MaintenanceViewV1, Error> {
        let status = self.status()?;
        if matches!(status, SlotStatus::Pending(_)) {
            return Ok(MaintenanceViewV1::SelectedPending);
        }
        if let SlotStatus::Enrollment(marker) = &status {
            // The manifest reader accepts no persisted unindexed manifest. An absent
            // checkpoint constructs the canonical empty manifest; do not turn missing
            // selected bytes, a later head, or another provider status into this case.
            let (digest, manifest) = self.sync_manifest()?;
            if marker.archive_checkpoint() != [0; 32]
                || digest != [0; 32]
                || manifest.indexed.is_some()
                || self.custody.archive_checkpoint()?.is_some()
                || self.status()? != status
            {
                return Err(Error::WitnessLost("maintenance Enrollment source"));
            }
            self.scheduler.require_archive_clear()?;
            return Ok(MaintenanceViewV1::BeforeBootstrap);
        }
        self.reconcile_archive_admission()?;
        let (snapshot, kind, _) = self.snapshot_selected()?;
        if kind == KagemushaWalletOperationKindV1::ArchiveSent && snapshot.fold_backlog != 0 {
            Ok(MaintenanceViewV1::AwaitingArchiveFold(snapshot))
        } else {
            Ok(MaintenanceViewV1::Released(snapshot))
        }
    }
}
