//! Join original Native execution to this service's archive predecessor owners.
//!
//! The synchronous result still owns State writers. A consuming capture handoff
//! moves the same execution into detached journals, including archive refusal.
//! Complete pre-execution resource admission remains required before this owner
//! can enter the production validation service; this adapter grants neither a
//! validation marker nor publication authority.
//! TODO: admit execution, nested journal payloads and publication overlap from
//! one finite production pool before attaching this owner to BodyStore. The
//! post-execution readiness checks, sticky State storage refusal and journal
//! admission/geometry failures still return a writer-bearing synchronous owner
//! or a recovery-only decomposition error. They cannot enter `CarrierValidator`
//! as a retryable producer error without losing the original execution.

use super::{
    V2ApplyError, V2ApplyService, VerifiedHeightContext,
    validation_custody::{CarrierValidator, RetainedBodyValidationService},
};
use crate::{
    block::{BlockValidationError, valid::NativeCandidatePreparationError},
    query::{
        provider_ingest_finalized::ProviderCandidateCapture,
        reputation_finalized::ReputationCandidateCapture,
    },
    state::{
        CarrierArchivePreparationError, CarrierJournalInputs, CarrierJournalPreparationError,
        CarrierJournalShellReservation, MergeLedgerCommitError, NativeExecutionResourceAdmission,
        PreparedCarrier, PreparedNativeLaneBatchSourceV1, RetainedCarrier,
    },
    sumeragi::v2_body_store::{
        BodyValidationBusy, LocalValidationRefusal, V2BodyStore, V2BodyStoreError,
    },
};
use iroha_crypto::Hash;
use iroha_data_model::block::{SignedBlock, consensus_v2 as wire};
use iroha_primitives::time::TimeSource;
use mv::allocation::{AllocationBudget, AllocationRefusal};
use std::task::Waker;

/// One planned Native slot: pending, occupied by the original, or consumed.
enum NativeValidatorSource<Admission, BindingAdmission> {
    Pending,
    Available(RetainedCarrier<Admission, BindingAdmission>),
    Consumed,
}

/// A planned descriptor slot cannot accept a foreign or second execution.
#[derive(Debug, thiserror::Error)]
pub(crate) enum NativeCarrierInstallError {
    /// The original differs from the context and proposal bound before execution.
    #[error("original Native carrier differs from its pre-execution descriptor identity")]
    Identity,
    /// One original has already occupied this descriptor, including after consumption.
    #[error("Native validation descriptor has already consumed its original carrier")]
    AlreadyInstalled,
}

/// Admit one already-detached Native execution to BodyStore's retained marker
/// path. Descriptor storage can be funded before execution while this validator
/// is pending; only the matching original may fill it once.
/// TODO: connect the complete bounded execution and publication policy.
pub(crate) struct ReadyNativeCarrierValidator<Admission, BindingAdmission> {
    expected_context_id: wire::HeightContextId,
    expected_proposal_wire_hash: Hash,
    source: NativeValidatorSource<Admission, BindingAdmission>,
    wake: Waker,
}

impl<Admission, BindingAdmission> ReadyNativeCarrierValidator<Admission, BindingAdmission> {
    /// Bind one descriptor to the exact resultless proposal before execution.
    pub(crate) fn new_pending(
        context: &wire::HeightContext,
        body: &SignedBlock,
        wake: Waker,
    ) -> Result<Self, NativeCarrierInstallError> {
        if !body.is_resultless_proposal() {
            return Err(NativeCarrierInstallError::Identity);
        }
        let expected_proposal_wire_hash = body
            .canonical_proposal_wire_hash()
            .map_err(|_| NativeCarrierInstallError::Identity)?;
        Ok(Self {
            expected_context_id: context.id(),
            expected_proposal_wire_hash,
            source: NativeValidatorSource::Pending,
            wake,
        })
    }

    fn matches_expected(&self, context: &wire::HeightContext, body: &SignedBlock) -> bool {
        body.is_resultless_proposal()
            && context.id() == self.expected_context_id
            && body
                .canonical_proposal_wire_hash()
                .is_ok_and(|hash| hash == self.expected_proposal_wire_hash)
    }

    /// Attach only the exact detached execution; return it unchanged on refusal.
    /// The slot remains consumed after BodyStore takes that original for a marker.
    fn install_original(
        &mut self,
        context: &wire::HeightContext,
        body: &SignedBlock,
        original: RetainedCarrier<Admission, BindingAdmission>,
    ) -> Result<
        (),
        (
            RetainedCarrier<Admission, BindingAdmission>,
            NativeCarrierInstallError,
        ),
    > {
        if !matches!(&self.source, NativeValidatorSource::Pending) {
            return Err((original, NativeCarrierInstallError::AlreadyInstalled));
        }
        if !self.matches_expected(context, body)
            || !original.matches_validation_candidate(context, body)
        {
            return Err((original, NativeCarrierInstallError::Identity));
        }
        self.source = NativeValidatorSource::Available(original);
        Ok(())
    }

    /// Exercise an already-detached original without a BodyStore descriptor.
    #[cfg(test)]
    pub(crate) fn new_with_original_for_test(
        context: &wire::HeightContext,
        body: &SignedBlock,
        original: RetainedCarrier<Admission, BindingAdmission>,
        wake: Waker,
    ) -> Self {
        let mut validator = Self::new_pending(context, body, wake)
            .expect("test original must bind a resultless proposal");
        if let Err((_, error)) = validator.install_original(context, body, original) {
            panic!("test original must match its planned descriptor: {error}");
        }
        validator
    }
}

impl<Admission: Send + 'static, BindingAdmission: Send + 'static>
    RetainedBodyValidationService<ReadyNativeCarrierValidator<Admission, BindingAdmission>>
{
    /// Fill the original descriptor reservation after source execution detaches.
    /// Descriptor allocation was completed before execution; a mismatch returns
    /// the exact original without granting BodyStore a marker.
    pub(crate) fn install_native_original(
        &mut self,
        context: &wire::HeightContext,
        body: &SignedBlock,
        original: RetainedCarrier<Admission, BindingAdmission>,
    ) -> Result<
        (),
        (
            RetainedCarrier<Admission, BindingAdmission>,
            NativeCarrierInstallError,
        ),
    > {
        self.with_validator_mut(|validator| validator.install_original(context, body, original))
    }
}

/// Source structure, fixed journal shells and BodyStore descriptors admitted together.
/// Execution scratch, nested payloads, archive work and publication overlap
/// still require concrete admission before this can enter the live validator.
/// TODO: attach the exact State/block/IVM demand and charge to the same owner
/// before using this reservation to authorize production execution.
#[must_use = "carry all original reservations through Native execution"]
pub(crate) struct NativePreExecutionRetainedSlots<
    Admission: Send + 'static,
    BindingAdmission: Send + 'static,
> {
    source_admission: NativeExecutionResourceAdmission,
    journal_shells: CarrierJournalShellReservation<Admission>,
    validation_service:
        RetainedBodyValidationService<ReadyNativeCarrierValidator<Admission, BindingAdmission>>,
}

/// Exact pre-execution fixed-slot refusal; neither case executes a candidate.
#[derive(Debug, thiserror::Error)]
pub(crate) enum NativeRetainedSlotsError {
    /// An invalid planned body cannot reserve marker authority.
    #[error(transparent)]
    Identity(#[from] NativeCarrierInstallError),
    /// Original source structure does not fit the finite pool.
    #[error("Native source structure admission: {0}")]
    Source(AllocationRefusal),
    /// Fixed original journal shells do not fit the finite pool.
    #[error("Native journal shell admission: {0}")]
    JournalShells(#[from] AllocationRefusal),
    /// The same pool cannot also hold the exact BodyStore descriptor vectors.
    #[error("Native retained descriptor admission: {0}")]
    Descriptors(#[from] V2BodyStoreError),
}

impl<Admission: Send + 'static, BindingAdmission: Send + 'static>
    NativePreExecutionRetainedSlots<Admission, BindingAdmission>
{
    /// Reserve the fixed overlap from one original finite pool before execution.
    /// A descriptor refusal drops and refunds the earlier shell reservation.
    pub(crate) fn try_reserve(
        store: &V2BodyStore,
        budget: &AllocationBudget,
        context: &wire::HeightContext,
        body: &SignedBlock,
        wake: Waker,
    ) -> Result<Self, NativeRetainedSlotsError> {
        if !store.matches_context(context) {
            return Err(NativeCarrierInstallError::Identity.into());
        }
        let validator = ReadyNativeCarrierValidator::new_pending(context, body, wake)?;
        let group_count = crate::block::native_lane_batch_for_execution(body)
            .map_err(|_| NativeCarrierInstallError::Identity)?
            .groups
            .len();
        let source_admission =
            NativeExecutionResourceAdmission::try_reserve_source(budget, group_count)
                .map_err(NativeRetainedSlotsError::Source)?;
        let journal_shells = PreparedCarrier::reserve_journal_shells(budget)?;
        let validation_service = store.retained_validation_service(validator, budget)?;
        Ok(Self {
            source_admission,
            journal_shells,
            validation_service,
        })
    }

    /// Transfer both original reservations to execution and retained capture.
    pub(crate) fn into_parts(
        self,
    ) -> (
        NativeExecutionResourceAdmission,
        CarrierJournalShellReservation<Admission>,
        RetainedBodyValidationService<ReadyNativeCarrierValidator<Admission, BindingAdmission>>,
    ) {
        (
            self.source_admission,
            self.journal_shells,
            self.validation_service,
        )
    }
}

impl<Admission: Send + 'static, BindingAdmission: Send + 'static> CarrierValidator
    for ReadyNativeCarrierValidator<Admission, BindingAdmission>
{
    type Owner = RetainedCarrier<Admission, BindingAdmission>;
    type Error = LocalValidationRefusal;

    fn prepare(
        &mut self,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<Self::Owner, Self::Error> {
        if !self.matches_expected(context, body) {
            return Err(LocalValidationRefusal::RecoveryRequired(
                "Native validation descriptor differs from the authenticated body".into(),
            ));
        }
        let NativeValidatorSource::Available(original) = &self.source else {
            return Err(LocalValidationRefusal::RecoveryRequired(
                "no separately admitted original Native execution for this candidate".into(),
            ));
        };
        if !original.matches_validation_candidate(context, body) {
            return Err(LocalValidationRefusal::RecoveryRequired(
                "original Native execution differs from the authenticated body".into(),
            ));
        }
        let NativeValidatorSource::Available(original) =
            std::mem::replace(&mut self.source, NativeValidatorSource::Consumed)
        else {
            unreachable!("the matching original was checked above")
        };
        Ok(original)
    }

    fn resume(
        &mut self,
        owner: Self::Owner,
    ) -> Result<Self::Owner, (Self::Owner, LocalValidationRefusal)> {
        owner.resume_capture().map_err(|(owner, error)| {
            let dependency = match &error {
                CarrierArchivePreparationError::Provider(error) => match error.as_ref() {
                    crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1::IndexBusy { wait } => {
                        Some(("provider archive index", wait.clone()))
                    }
                    crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1::CaptureReserved { wait } => {
                        Some(("provider archive capture", wait.release_wait().clone()))
                    }
                    _ => None,
                },
                CarrierArchivePreparationError::Reputation(error) => match error.as_ref() {
                    crate::query::reputation_finalized::ReputationFinalizedArchiveError::IndexBusy { wait } => {
                        Some(("reputation archive index", wait.clone()))
                    }
                    crate::query::reputation_finalized::ReputationFinalizedArchiveError::CaptureReserved { wait } => {
                        Some(("reputation archive capture", wait.release_wait().clone()))
                    }
                    _ => None,
                },
            };
            let refusal = match dependency {
                Some((resource, wait)) => LocalValidationRefusal::PhysicalBusy(
                    BodyValidationBusy::new(resource, wait, self.wake.clone()),
                ),
                None => LocalValidationRefusal::RecoveryRequired(error.to_string()),
            };
            (owner, refusal)
        })
    }
}

/// One executed candidate and the archive predecessors reserved before execution.
/// Field order retires all State writers before either logical archive owner.
#[must_use = "detach the original journals synchronously or abandon the complete candidate"]
pub(crate) struct PreparedNativeServiceCandidate<'state, Admission> {
    carrier: PreparedCarrier<'state>,
    provider: Option<ProviderCandidateCapture>,
    reputation: Option<ReputationCandidateCapture>,
    journal_shells: CarrierJournalShellReservation<Admission>,
    service: &'state V2ApplyService,
}

/// Synchronous capture refusal. Recoverable variants retain their original
/// candidate; an invariant failure in journal decomposition is recovery-only
/// and must never be converted to a deterministic body rejection or retry.
/// A caller must resolve or abandon a writer-bearing candidate before a wait.
pub(crate) enum NativeCandidateCaptureRefusal<'state, Admission, E> {
    /// Post-execution readiness still owns every State writer and archive.
    #[expect(
        dead_code,
        reason = "TODO: live Native capture caller must consume this owner"
    )]
    Readiness(NativeCandidateReadinessRefusal<'state, Admission>),
    /// Journal capture's typed refusal, including any recoverable original owner.
    Journals(CarrierJournalPreparationError<'state, Admission, E>),
}

/// Typed post-execution local refusal with the exact State writer, archives,
/// source and pre-execution shell charge still joined in one synchronous owner.
/// It cannot wait across threads or authorize a BodyStore rejection marker.
#[must_use = "retry or abandon the original writer-bearing Native candidate synchronously"]
pub(crate) struct NativeCandidateReadinessRefusal<'state, Admission> {
    /// The sole executed candidate, including its original source and capacity.
    pub(crate) candidate: PreparedNativeServiceCandidate<'state, Admission>,
    /// The exact Queue or Kura readiness diagnosis from that candidate.
    pub(crate) error: V2ApplyError,
}

impl<'state, Admission> NativeCandidateReadinessRefusal<'state, Admission> {
    /// Recheck only a physical dependency with the same candidate and archives.
    /// A fixed local-capacity or semantic refusal returns this owner for explicit
    /// fail-stop handling instead of spinning an uncaused retry. A repeated
    /// physical refusal returns the entire owner, without executing again.
    pub(crate) fn retry(self) -> Result<NativeCandidateParts<'state, Admission>, Self> {
        if !matches!(
            &self.error,
            V2ApplyError::LocalValidation(
                LocalValidationRefusal::PhysicalBusy(_)
                    | LocalValidationRefusal::QueueRelease { .. }
            )
        ) {
            return Err(self);
        }
        self.candidate.try_into_parts()
    }
}

/// Complete original writer-bearing handoff after Queue and Kura readiness.
pub(crate) type NativeCandidateParts<'state, Admission> = (
    PreparedCarrier<'state>,
    Option<ProviderCandidateCapture>,
    Option<ReputationCandidateCapture>,
    CarrierJournalShellReservation<Admission>,
);

impl<'state, Admission> PreparedNativeServiceCandidate<'state, Admission> {
    /// Detach the original Native execution after the caller has reserved its
    /// typed journal shells before execution. The admission callback must fund
    /// all aggregate journal payloads before capture; shell capacity alone is
    /// insufficient. Archive insertion contention returns the already-detached
    /// phase. The retained validator must immediately call `resume_capture` to
    /// surface its typed wait or recovery diagnosis before scheduling another
    /// turn; a permanent original capture refusal stays pinned in that phase.
    pub(crate) fn try_capture_original<BindingAdmission, E>(
        self,
        admit_journals: impl FnOnce(CarrierJournalInputs<'_, 'state>) -> Result<Admission, E>,
    ) -> Result<
        RetainedCarrier<Admission, BindingAdmission>,
        NativeCandidateCaptureRefusal<'state, Admission, E>,
    > {
        let (carrier, provider, reputation, journal_shells) = match self.try_into_parts() {
            Ok(parts) => parts,
            Err(refusal) => {
                return Err(NativeCandidateCaptureRefusal::Readiness(refusal));
            }
        };
        match carrier.prepare_journals(journal_shells, provider, reputation, admit_journals) {
            Ok(journals) => Ok(RetainedCarrier::Validated(journals)),
            Err(CarrierJournalPreparationError::ArchivePreparation { carrier, .. }) => {
                Ok(RetainedCarrier::Capturing(carrier))
            }
            Err(error) => Err(NativeCandidateCaptureRefusal::Journals(error)),
        }
    }

    /// Move the actual execution and its original captures into journal preparation.
    /// The caller must provide the real journal admission policy; this handoff
    /// does not replace it with a scalar receipt or an empty reservation.
    /// A local post-execution refusal returns this same synchronous owner. Its
    /// State writers cannot cross an async wait; production must either retry
    /// synchronously or fail stop until the readiness check can be moved behind
    /// the same detached owner.
    /// TODO: represent Queue/Kura post-execution readiness on the detached
    /// owner before connecting the live retained validator.
    pub(crate) fn try_into_parts(
        self,
    ) -> Result<
        NativeCandidateParts<'state, Admission>,
        NativeCandidateReadinessRefusal<'state, Admission>,
    > {
        self.try_into_parts_with_readiness(|candidate| {
            candidate
                .service
                .try_validate_prospective_autoscale_retirement_queue(
                    candidate.carrier.block(),
                    candidate.carrier.state(),
                )?;
            candidate
                .service
                .kura
                .validate_native_amx_participant_application_evidence_byte_budget(
                    candidate.carrier.native_amx_manifest(),
                    None,
                )
                .map_err(V2ApplyService::classify_native_amx_evidence_byte_budget_error)
        })
    }

    fn try_into_parts_with_readiness(
        self,
        readiness: impl FnOnce(&Self) -> Result<(), V2ApplyError>,
    ) -> Result<
        NativeCandidateParts<'state, Admission>,
        NativeCandidateReadinessRefusal<'state, Admission>,
    > {
        if let Err(error) = readiness(&self) {
            return Err(NativeCandidateReadinessRefusal {
                candidate: self,
                error,
            });
        }
        Ok((
            self.carrier,
            self.provider,
            self.reputation,
            self.journal_shells,
        ))
    }

    /// Inject only the local readiness result while retaining the actual
    /// executed owner, for the exact-owner refusal regression.
    #[cfg(test)]
    pub(crate) fn refuse_readiness_for_test(
        self,
        error: V2ApplyError,
    ) -> NativeCandidateReadinessRefusal<'state, Admission> {
        self.try_into_parts_with_readiness(|_| Err(error))
            .err()
            .expect("injected local readiness refusal")
    }
}

impl V2ApplyService {
    /// Execute authenticated Native sources once with prepaid journal shells,
    /// then reserve both archive predecessors before opening State writers.
    /// A stale source returns no owner and never becomes an invalid-body marker.
    /// The exact State and proposal are checked before any archive reservation;
    /// callers cannot pair a foreign source with an otherwise identical service.
    pub(crate) fn prepare_native_source<'state, Admission>(
        &'state self,
        body: &SignedBlock,
        source: PreparedNativeLaneBatchSourceV1<'state>,
        context: VerifiedHeightContext,
        journal_shells: CarrierJournalShellReservation<Admission>,
    ) -> Result<Option<PreparedNativeServiceCandidate<'state, Admission>>, V2ApplyError> {
        crate::sumeragi::witness::ensure_state_access_without_exec_witness().map_err(|reason| {
            LocalValidationRefusal::RecoveryRequired(format!(
                "Native service execution recorder ownership conflict: {reason}"
            ))
        })?;
        if !body.is_resultless_proposal() {
            return Err(V2ApplyError::ResultBearingProposal);
        }
        let Some((state, original, _)) = source.preparation_input() else {
            return Ok(None);
        };
        if !std::ptr::eq(state, self.state.as_ref()) || original != body {
            return Err(V2ApplyError::TaskMismatch);
        }
        // Match the live candidate validator's immutable payload policy before
        // reserving archives or opening execution writers. The authenticated
        // Native source alone does not authorize a different lane payload plan.
        self.validate_lane_payload_plan(context.context(), body)?;
        let archives = self.try_reserve_candidate_archives(context.context(), body)?;
        let (provider, reputation) = archives
            .into_captures(self, context.context(), body)
            .map_err(|(_, error)| error)?;
        #[cfg(test)]
        self.test_failures
            .candidate_executions
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let carrier = match source.prepare_candidate(
            context,
            &self.genesis_account,
            &TimeSource::new_system(),
            self.block_cadence,
        ) {
            Ok(Some(carrier)) => carrier,
            Ok(None)
            | Err(NativeCandidatePreparationError::Execution(
                MergeLedgerCommitError::ExecutionObservationChanged,
            )) => return Ok(None),
            Err(error) => return Err(self.classify_native_preparation_failure(body, error)),
        };
        Ok(Some(PreparedNativeServiceCandidate {
            carrier,
            provider,
            reputation,
            journal_shells,
            service: self,
        }))
    }

    /// Count actual execution attempts, excluding source and archive refusals.
    #[cfg(test)]
    pub(crate) fn candidate_executions_for_test(&self) -> usize {
        self.test_failures
            .candidate_executions
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Preserve original local dependencies and semantic validation provenance.
    pub(super) fn classify_native_preparation_failure(
        &self,
        body: &SignedBlock,
        error: NativeCandidatePreparationError,
    ) -> V2ApplyError {
        match error {
            NativeCandidatePreparationError::Preflight(error)
            | NativeCandidatePreparationError::Execution(
                MergeLedgerCommitError::NativeControlValidation(error),
            ) => self.classify_validation_failure(None, body, &error),
            NativeCandidatePreparationError::Execution(
                MergeLedgerCommitError::StateStorageAdmission(error),
            ) => self.classify_validation_failure(
                None,
                body,
                &BlockValidationError::StateStorageAdmission(error),
            ),
            NativeCandidatePreparationError::Execution(
                error @ MergeLedgerCommitError::ExecutionBatchFull { .. },
            ) => V2ApplyError::Validation(error.to_string()),
            // Only the global preflight and explicit governed batch limit above
            // establish a semantic body verdict. Remaining source-execution and
            // metadata errors include local State/recorder recovery failures.
            // Diagnostic strings must never authorize a negative marker.
            error => LocalValidationRefusal::RecoveryRequired(error.to_string()).into(),
        }
    }
}
