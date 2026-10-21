//! One bounded Native-owned Archive attempt; the scheduler owns admission atomically.

use super::super::scheduling::{ArchiveAdmissionErrorV1, ArchiveTargetV1};
use super::*;

/// Expected local maintenance wait, never a financial completion or a durable permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredArchiveWaitV1 {
    /// Recover source-selected Pending before considering idle work.
    SelectedPending,
    /// The current Archive must receive its covering fold first.
    ArchiveFold,
    /// Ordinary current fold backlog must drain before a fresh Archive.
    FoldBacklog,
    /// A requested/open exchange owns priority.
    ExchangeOpen,
    /// Charging, count and age currently admit no Archive.
    NotDue,
    /// Another exact maintenance/priority owner must finish.
    Busy,
}

/// Result of one bounded drain call. Only `Completion::Complete` is completion authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeferredArchiveDrainV1 {
    /// No verified first anchor remains queued.
    NoWork,
    /// A fresh check refused dispatch without monetary mutation.
    Blocked(DeferredArchiveWaitV1),
    /// An already mapped exact result was observed; only Complete removes its queue entry.
    Reconciled(Completion),
    /// One admitted exact Archive was attempted through the ordinary Native executor.
    Executed(Completion),
}

fn blocked(error: ArchiveAdmissionErrorV1) -> Result<DeferredArchiveDrainV1, Error> {
    let wait = match error {
        ArchiveAdmissionErrorV1::Busy => DeferredArchiveWaitV1::Busy,
        ArchiveAdmissionErrorV1::ExchangeOpen => DeferredArchiveWaitV1::ExchangeOpen,
        ArchiveAdmissionErrorV1::FoldBacklog => DeferredArchiveWaitV1::FoldBacklog,
        ArchiveAdmissionErrorV1::NotDue => DeferredArchiveWaitV1::NotDue,
        other => return Err(other.into()),
    };
    Ok(DeferredArchiveDrainV1::Blocked(wait))
}

impl<C: Custody, A: ArchiveStore, N: NativeProofs> Coordinator<C, A, N> {
    /// Durably retain these exact bytes before verifying delivery for the exact local Send.
    /// No Archive or Payment priority is acquired. Pending requires selected recovery first.
    /// # Errors
    /// Custody/publication and verifier failures preserve selected intake for bounded retry;
    /// only a completed receipt rejection retires that intake from verification discovery.
    pub fn observe_credited_for_send(
        &mut self,
        send: &[u8; 32],
        original: &[u8],
    ) -> Result<CreditProjectionV1, Error> {
        let receipt = self.retain_deferred_credited(send, original)?;
        self.verify_deferred_credited(&receipt)
    }

    /// Retry at most the oldest source-selected unverified original after reopening.
    /// # Errors
    /// Same custody and receipt-specific errors as [`Self::verify_deferred_credited`].
    pub fn verify_next_deferred_credited(&mut self) -> Result<Option<CreditProjectionV1>, Error> {
        let Some(receipt) = self.next_deferred_credited()? else {
            return Ok(None);
        };
        self.verify_deferred_credited(&receipt).map(Some)
    }

    /// Reconcile at most the oldest queued exact Archive; no fresh work or proof priority.
    /// # Errors
    /// Selected Pending, source loss or unavailable custody never mean an empty queue.
    pub fn reconcile_next_deferred_archive(&mut self) -> Result<Option<RequestStatusV1>, Error> {
        self.reconcile_archive_admission()?;
        let Some(entry) = self.deferred_archive_summary()?.oldest else {
            return Ok(None);
        };
        self.reconcile_deferred_archive(&entry).map(Some)
    }
}

impl<C: Custody, A: ArchiveStore, N: NativePreparation> Coordinator<C, A, N> {
    /// Recheck source and shared admission, then attempt at most one immutable queued Archive.
    /// Acquire `turn` outside the wallet lock after an advisory maintenance/queue observation.
    /// This call consumes and drops the turn before returning, allowing the required fold to run.
    /// The turn itself grants no permission: Native selects the exact target and the scheduler
    /// atomically checks exchanges, lifecycle, charging/count/age and empty fold backlog.
    /// # Errors
    /// Custody, proof, foreign/closed owner or inconsistent exact attempt. Uncertain execution
    /// retains the scheduler obligation until authoritative exact reconciliation succeeds.
    pub fn drain_deferred_archive(
        &mut self,
        turn: ArchiveTurnV1,
    ) -> Result<DeferredArchiveDrainV1, Error> {
        self.drain_deferred_archive_observing(turn, clock::observe)
    }

    #[cfg(test)]
    pub(in crate::kagemusha_wallet_state_v1) fn drain_deferred_archive_with_test_clock(
        &mut self,
        turn: ArchiveTurnV1,
        observe: impl FnOnce() -> Option<u64>,
    ) -> Result<DeferredArchiveDrainV1, Error> {
        self.drain_deferred_archive_observing(turn, || {
            observe()
                .map(|unix_ms| clock::WallObservation { unix_ms })
                .ok_or(clock::ClockUnavailable::BeforeEpoch)
        })
    }

    // The production caller always uses Native's own clock. This private seam lets tests
    // change the observation during source reads without exporting clock/admission authority.
    fn drain_deferred_archive_observing(
        &mut self,
        turn: ArchiveTurnV1,
        observe: impl FnOnce() -> Result<clock::WallObservation, clock::ClockUnavailable>,
    ) -> Result<DeferredArchiveDrainV1, Error> {
        turn.validate_for(&self.scheduler)?;
        let _turn = turn;
        match self.maintenance_view()? {
            MaintenanceViewV1::BeforeBootstrap => return Ok(DeferredArchiveDrainV1::NoWork),
            MaintenanceViewV1::SelectedPending => {
                return Ok(DeferredArchiveDrainV1::Blocked(
                    DeferredArchiveWaitV1::SelectedPending,
                ));
            }
            MaintenanceViewV1::AwaitingArchiveFold(_) => {
                return Ok(DeferredArchiveDrainV1::Blocked(
                    DeferredArchiveWaitV1::ArchiveFold,
                ));
            }
            MaintenanceViewV1::Released(_) => {}
        }
        let summary = self.deferred_archive_summary()?;
        let Some(entry) = summary.oldest else {
            return Ok(DeferredArchiveDrainV1::NoWork);
        };
        let outcome = self.reconcile_deferred_archive(&entry)?;
        match outcome {
            RequestStatusV1::Outcome(completion) => {
                return Ok(DeferredArchiveDrainV1::Reconciled(completion));
            }
            RequestStatusV1::Preparing => {}
            RequestStatusV1::Unknown => return Err(Error::WitnessLost("queued Archive intent")),
        }
        let intent = self.source_bound_archive_intent(&entry.send_request, &entry.original)?;
        // Keep the immutable age origin private and finish all queue/head reads before
        // observing current wall time. A summary's earlier Due is never permission.
        let fresh = self.selected_deferred_archive()?;
        if fresh.oldest.as_ref() != Some(&entry) {
            return Err(Error::WitnessLost("deferred dispatch selection changed"));
        }
        let (source, snapshot, kind) = self.selected_archive_head()?;
        if kind == KagemushaWalletOperationKindV1::ArchiveSent && snapshot.fold_backlog != 0 {
            return Ok(DeferredArchiveDrainV1::Blocked(
                DeferredArchiveWaitV1::ArchiveFold,
            ));
        }
        let target = ArchiveTargetV1 {
            request_id: intent.request_id(),
            source,
        };
        let age_due = clock::age(fresh.first_unix_ms, observe()) == clock::Age::Due;
        let permit = match self.scheduler.reserve_archive(
            &_turn,
            target,
            fresh.count,
            age_due,
            snapshot.fold_backlog,
        ) {
            Ok(permit) => permit,
            Err(error) => return blocked(error),
        };
        let result = self.execute_archive_intent(intent, &permit);
        let resolution = match self.archive_resolution(target) {
            Ok(resolution) => resolution,
            Err(error) => return Err(result.err().unwrap_or(error)),
        };
        self.scheduler.settle_archive(permit, resolution)?;
        let completion = result?;
        if matches!(&completion, Completion::Complete(_)) {
            self.reconcile_deferred_archive(&entry)?;
        }
        Ok(DeferredArchiveDrainV1::Executed(completion))
    }
}
