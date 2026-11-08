//! Native-only suffix preparation and one-use hardware review ownership.
//! Durable records contain scheduling DATA; only a live complete verification owns a tip.
use super::super::suffix_custody::*;
use super::*;
use crate::kagemusha_wallet_suffix_unload_v1::{SuffixTipV1, SuffixVerificationCursorV1};
use iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::*;

mod execute;
mod prepare;
mod source_steps;
mod status;
mod worker;
pub use worker::{ProvedSuffixPreparationV1, SuffixPreparationJobV1};

/// User terms only. Native selects the source, account, witnesses, randomness and chunks.
#[derive(Debug, Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::SuffixUnloadRequestV1")]
#[repr(align(16))]
pub struct SuffixUnloadRequestV1 {
    /// Stable caller request identity, reused for every exact retry.
    pub request_id: [u8; 32],
    /// Positive amount irreversibly removed from the offline wallet.
    pub amount: u128,
    /// Optional exact signed quote and authenticating certificates.
    pub charge: Option<ChargeOriginalsV1>,
    /// Canonical beneficiary account original matching the optional quote.
    pub beneficiary: Option<Vec<u8>>,
}
impl SuffixUnloadRequestV1 {
    fn validate(&self) -> Result<(), Error> {
        if self.request_id == [0; 32] || self.amount == 0 {
            return Err(Error::Invalid("suffix request terms"));
        }
        match (&self.charge, &self.beneficiary) {
            (None, None) => {}
            (Some(charge), Some(beneficiary))
                if !charge.quote.is_empty()
                    && charge.quote.len() <= KAGEMUSHA_WALLET_CHARGE_QUOTE_MAX_BYTES_V1
                    && !charge.certificates.is_empty()
                    && charge.certificates.len() <= KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
                    && !beneficiary.is_empty()
                    && beneficiary.len() <= 4096 => {}
            _ => return Err(Error::Invalid("suffix request charge originals")),
        }
        Ok(())
    }
}
/// Scheduling reason only; never an invalid-proof or unavailable-storage classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum SuffixNotApplicableV1 {
    /// No completed local lineage anchor exists.
    NoAnchor = 1,
    /// The suffix contains an operation outside Receive and prior Unload.
    UnsupportedStep = 2,
    /// The current head already has its completed lineage proof.
    NoSuffix = 3,
}

/// A bounded preparation turn. Ready is an actual Native capability, not its projection.
pub enum SuffixUnloadDecisionV1 {
    /// Named resume authenticated that this exact request is absent from the selected index.
    /// This scheduling DATA is never returned by fresh preparation or reviewed execution.
    NotStarted(SuffixUnloadStatusV1),
    /// Explicit source-selection reason; no proof failure is mapped here.
    NotApplicable(SuffixNotApplicableV1),
    /// One bounded verification turn completed; counters are display data.
    Verifying(SuffixUnloadProgressV1),
    /// Genuine Native capability ready for one hardware approval.
    Ready(ReviewedSuffixUnloadV1),
    /// Exact hardware completion was verified and retained; ledger payout is separate.
    Released(SuffixUnloadReleasedV1),
    /// Selected or uncertain Advance requires exact recovery.
    Pending,
    /// No capability issued because a local maintenance prerequisite is busy.
    Waiting(ReviewMaintenanceWaitV1),
}
/// Outcome after consuming the one-use hardware review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuffixUnloadExecutionV1 {
    /// Exact hardware completion was verified and retained; ledger payout is separate.
    Released(SuffixUnloadReleasedV1),
    /// Selected or uncertain Advance requires exact recovery.
    Pending,
}

/// Exact independently verified review. No Clone, public constructor or serialization.
/// It contains the real settled Native tip; display DATA can never create this authority.
pub struct ReviewedSuffixUnloadV1 {
    request: SuffixUnloadRequestV1,
    projection: SuffixUnloadReviewProjectionV1,
    tip: SuffixTipV1,
    frozen: SuffixFrozenTransitionV1,
    selection: [u8; 32],
    source_checkpoint: [u8; 32],
    source_selected_generation: u128,
}
impl ReviewedSuffixUnloadV1 {
    #[must_use]
    /// Read display data without transferring or reproducing this capability.
    pub fn projection(&self) -> &SuffixUnloadReviewProjectionV1 {
        &self.projection
    }
}
const _: () = {
    const fn require_send<T: Send>() {}
    require_send::<ReviewedSuffixUnloadV1>();
};

/// Only pre-Advance maintenance may return the same review unchanged.
#[derive(thiserror::Error)]
#[error("{error}")]
pub struct ReviewedSuffixFailureV1 {
    #[source]
    error: Error,
    retained: Option<ReviewedSuffixUnloadV1>,
    wait: Option<ReviewMaintenanceWaitV1>,
}
impl std::fmt::Debug for ReviewedSuffixFailureV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReviewedSuffixFailureV1")
            .field("error", &self.error)
            .field("retained", &self.retained.is_some())
            .field("wait", &self.wait)
            .finish()
    }
}
impl From<Error> for ReviewedSuffixFailureV1 {
    fn from(error: Error) -> Self {
        Self {
            error,
            retained: None,
            wait: None,
        }
    }
}
impl ReviewedSuffixFailureV1 {
    #[must_use]
    /// The sole reason for retaining the exact unconsumed review, if any.
    pub const fn maintenance_wait(&self) -> Option<ReviewMaintenanceWaitV1> {
        self.wait
    }
    #[must_use]
    /// Consume the failure and recover the original review only for a pre-admission wait.
    pub fn into_parts(self) -> (Error, Option<ReviewedSuffixUnloadV1>) {
        (self.error, self.retained)
    }
}

/// Nonserialized correlation for one source-selected bounded proof turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WorkBinding {
    request: [u8; 32],
    source_capsule: [u8; 32],
    source_sequence: u128,
    source_selected_generation: u128,
    anchor_sequence: u128,
    turn: u64,
}

/// The immutable inputs to final Unload derivation. Every value is selected by Native
/// under the source lock and rechecked at publication; none is a foreign trust verdict.
struct FinishInputs {
    request: SuffixUnloadRequestV1,
    credential: KagemushaWalletCredentialV1,
    certificates: KagemushaWalletCertificateSetV1,
    account: iroha_data_model::account::AccountId,
    account_original: Vec<u8>,
    anchor_digest: [u8; 32],
    chunk_digests: Vec<[u8; 32]>,
    stream_bytes: u128,
    source_capsule: [u8; 32],
    source_operation: [u8; 32],
    pins: IndexRoot,
    recovery: KagemushaWalletIndexedInsertV1,
    nonce: [u8; 32],
    existing: Option<SuffixFrozenTransitionV1>,
    successor_custody: preparation_custody::SourceCustodyV1,
}

/// Scheduler ownership and live cursor are transient. Restart re-enters full anchor/step
/// verification from selected exact originals; persisted progress is never a proof cache.
pub(super) struct LiveSuffixPass {
    scheduler: Scheduler,
    binding: WorkBinding,
    cursor: Option<SuffixVerificationCursorV1>,
    tip: Option<SuffixTipV1>,
    inflight: Option<Speculation<()>>,
}

impl Drop for LiveSuffixPass {
    fn drop(&mut self) {
        if let Some(inflight) = &self.inflight {
            // Superseding the source wakes an unadmitted worker as well as cancelling
            // cryptographic work; it never waits for unrelated payment ownership.
            self.scheduler.cancel_suffix_speculation(&inflight.job());
        }
    }
}

/// Cancellation handle for exactly one bounded suffix proof job. It contains no monetary
/// authority and can only stop ephemeral verification; selected Advance recovery persists.
#[derive(Clone)]
pub struct SuffixPreparationCancellationV1 {
    scheduler: Scheduler,
    job: SpeculationJob<()>,
}
impl SuffixPreparationCancellationV1 {
    /// Stop this job at the next native cryptographic checkpoint.
    pub fn cancel(&self) {
        self.scheduler.cancel_suffix_speculation(&self.job);
    }
    /// Check whether this job's exact exchange or owner has cancelled it.
    /// # Errors
    /// Returns `Cancelled` after cancellation; no invalid-proof verdict is inferred.
    pub fn check(&self) -> Result<(), Error> {
        self.job.token().check()
    }
}

/// Ends an unadmitted job when its bridge call disappears before verification starts.
/// This is separate from the proof owner so moving a job phase keeps ordinary Rust moves.
struct SuffixJobLifetime(SpeculationJob<()>);
impl Drop for SuffixJobLifetime {
    fn drop(&mut self) {
        self.0.abandon_waiting();
    }
}

#[cfg(test)]
mod scheduling_tests {
    use super::*;

    #[test]
    fn abandoned_unadmitted_suffix_job_does_not_leave_request_busy() {
        let owner = Speculation::<()>::new();
        let job = owner.job();
        assert!(!owner.finished());
        drop(SuffixJobLifetime(job.clone()));
        assert!(owner.finished());
        assert!(Scheduler::new().speculate(&job).is_none());
        assert!(owner.take().is_none());
    }

    #[test]
    fn exact_suffix_cancel_stops_its_worker_without_cancelling_another_job() {
        let scheduler = Scheduler::new();
        let owner = Speculation::<()>::new();
        let job = owner.job();
        let cancellation = SuffixPreparationCancellationV1 {
            scheduler: scheduler.clone(),
            job: job.clone(),
        };
        let run = scheduler.speculate(&job).expect("suffix worker admitted");
        let other = Speculation::<()>::new();
        let other_cancel = SuffixPreparationCancellationV1 {
            scheduler: scheduler.clone(),
            job: other.job(),
        };
        cancellation.cancel();
        assert!(matches!(cancellation.check(), Err(Error::Cancelled)));
        assert!(matches!(run.token().check(), Err(Error::Cancelled)));
        assert!(other_cancel.check().is_ok());
        drop(run);
        assert!(owner.finished());
        assert!(!other.finished());
    }

    #[test]
    fn cancelled_suffix_waiter_exits_without_waiting_for_an_unrelated_payment() {
        let scheduler = Scheduler::new();
        let payment = scheduler.payment();
        let owner = Speculation::<()>::new();
        let job = owner.job();
        let cancellation = SuffixPreparationCancellationV1 {
            scheduler: scheduler.clone(),
            job: job.clone(),
        };
        let (started, admitted) = std::sync::mpsc::channel();
        let (finished, stopped) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            started.send(()).unwrap();
            let refused = scheduler.speculate(&job).is_none();
            finished.send(refused).unwrap();
        });
        admitted.recv().unwrap();
        cancellation.cancel();
        // The payment remains held: only exact cancellation can release the waiter.
        let result = stopped.recv_timeout(std::time::Duration::from_secs(2));
        drop(payment);
        worker.join().unwrap();
        assert_eq!(result, Ok(true));
    }

    #[test]
    fn dropped_worker_run_can_restart_but_finished_result_is_taken_once() {
        let scheduler = Scheduler::new();
        let failed = Speculation::<()>::new();
        let failed_job = failed.job();
        let run = scheduler.speculate(&failed_job).expect("admitted worker");
        assert!(!failed.finished());
        drop(run);
        assert!(failed.finished());
        assert!(failed.take().is_none());
        let retried = Speculation::<()>::new();
        let retried_job = retried.job();
        let run = scheduler
            .speculate(&retried_job)
            .expect("next bounded retry");
        run.finish(());
        drop(SuffixJobLifetime(retried_job));
        assert!(retried.finished());
        assert_eq!(retried.take(), Some(()));
    }
}
