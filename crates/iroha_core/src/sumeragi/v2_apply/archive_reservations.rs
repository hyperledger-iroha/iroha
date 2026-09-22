//! Original archive predecessor custody acquired before candidate execution.
//!
//! These reservations protect archive predecessors only. They are not aggregate
//! memory admission, validation, finality, or permission to publish State.

use super::{
    BodyValidationBusy, Hash, Kura, SignedBlock, State, V2ApplyError, V2ApplyService, wire,
};
use crate::{
    query::{
        provider_ingest_finalized::{
            ProviderCandidateCapture, ProviderIngestFinalizedArchiveErrorV1 as ProviderError,
            ProviderIngestFinalizedArchiveKeyV1, ProviderIngestFinalizedArchiveV1,
        },
        reputation_finalized::{
            ReputationCandidateCapture, ReputationFinalizedArchive,
            ReputationFinalizedArchiveError as ReputationError, ReputationFinalizedArchiveKeyV1,
        },
    },
    sumeragi::v2_body_store::LocalValidationRefusal,
};
use std::sync::Arc;

/// Move-only ownership of both configured original archive predecessors.
///
/// No physical archive or State guard survives construction. A failed handoff
/// returns this exact owner, retaining both reservations until retry or drop.
pub(crate) struct CandidateArchiveReservations {
    state: Arc<State>,
    kura: Arc<Kura>,
    provider_archive: Option<Arc<ProviderIngestFinalizedArchiveV1>>,
    reputation_archive: Option<Arc<ReputationFinalizedArchive>>,
    context_id: wire::HeightContextId,
    proposal_wire_hash: Hash,
    provider: Option<ProviderCandidateCapture>,
    reputation: Option<ReputationCandidateCapture>,
}

impl std::fmt::Debug for CandidateArchiveReservations {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CandidateArchiveReservations")
            .field("context_id", &self.context_id)
            .field("proposal_wire_hash", &self.proposal_wire_hash)
            .field("provider", &self.provider.is_some())
            .field("reputation", &self.reputation.is_some())
            .finish_non_exhaustive()
    }
}

impl V2ApplyService {
    /// Acquire configured archive predecessors before any candidate State execution.
    ///
    /// The second refusal drops the first original reservation. Busy observations
    /// retain the actual blocking owner and this service's Queue wake destination;
    /// corrupt or unavailable local archives require recovery, never body rejection.
    pub(crate) fn try_reserve_candidate_archives(
        &self,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<CandidateArchiveReservations, V2ApplyError> {
        self.validate_archive_candidate_binding(context, body)?;
        let context_id = context.id();
        // Bind the exact canonical proposal, including signatures. Header identity
        // alone would accept a different wire body. This temporary identity encoding
        // does not assert that aggregate memory has been admitted.
        let proposal_wire_hash = archive_proposal_wire_hash(body)?;
        let provider = self
            .provider_ingest_finalized_archive
            .as_ref()
            .map(|archive| {
                let key = ProviderIngestFinalizedArchiveKeyV1::try_new(
                    context.network_id,
                    context.height,
                    *body.hash().as_ref(),
                    body.header().creation_time_ms,
                )
                .map_err(|error| self.provider_archive_refusal(error))?;
                archive
                    .try_reserve_candidate(key, &self.kura)
                    .map_err(|error| self.provider_archive_refusal(error))
            })
            .transpose()?;
        let reputation = self
            .reputation_finalized_archive
            .as_ref()
            .map(|archive| {
                let key = ReputationFinalizedArchiveKeyV1::try_new(
                    context.network_id,
                    context.height,
                    *body.hash().as_ref(),
                )
                .map_err(|error| self.reputation_archive_refusal(error))?;
                archive
                    .try_reserve_candidate(key, body.header().creation_time_ms, &self.kura)
                    .map_err(|error| self.reputation_archive_refusal(error))
            })
            .transpose()?;
        Ok(CandidateArchiveReservations {
            state: Arc::clone(&self.state),
            kura: Arc::clone(&self.kura),
            provider_archive: self.provider_ingest_finalized_archive.clone(),
            reputation_archive: self.reputation_finalized_archive.clone(),
            context_id,
            proposal_wire_hash,
            provider,
            reputation,
        })
    }

    fn validate_archive_candidate_binding(
        &self,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<(), V2ApplyError> {
        if !self.state.matches_kura_instance(&self.kura) {
            return Err(LocalValidationRefusal::RecoveryRequired(
                "candidate archive reservation requires the original State/Kura pair".to_owned(),
            )
            .into());
        }
        if !body.is_resultless_proposal() {
            return Err(V2ApplyError::ResultBearingProposal);
        }
        let parent = context
            .parent_commit_qc
            .as_ref()
            .map(|certificate| certificate.subject.block_hash)
            .or_else(|| {
                context
                    .snapshot_bootstrap
                    .map(|anchor| anchor.snapshot_block_hash)
            });
        if context.network_id != self.network_id
            || body.header().height().get() != context.height
            || body.header().prev_block_hash() != parent
        {
            return Err(V2ApplyError::TaskMismatch);
        }
        Ok(())
    }

    fn archive_busy(
        &self,
        resource: &'static str,
        wait: concread::release::ReleaseWait,
    ) -> V2ApplyError {
        LocalValidationRefusal::PhysicalBusy(BodyValidationBusy::new(
            resource,
            wait,
            self.queue.sumeragi_waker(),
        ))
        .into()
    }

    fn provider_archive_refusal(&self, error: ProviderError) -> V2ApplyError {
        match error {
            ProviderError::IndexBusy { wait } => self.archive_busy("provider_archive_index", wait),
            ProviderError::CaptureReserved { wait } => {
                self.archive_busy("provider_archive_capture", wait.release_wait().clone())
            }
            error => LocalValidationRefusal::RecoveryRequired(format!(
                "candidate provider archive reservation requires recovery: {error}"
            ))
            .into(),
        }
    }

    fn reputation_archive_refusal(&self, error: ReputationError) -> V2ApplyError {
        match error {
            ReputationError::IndexBusy { wait } => {
                self.archive_busy("reputation_archive_index", wait)
            }
            ReputationError::CaptureReserved { wait } => {
                self.archive_busy("reputation_archive_capture", wait.release_wait().clone())
            }
            error => LocalValidationRefusal::RecoveryRequired(format!(
                "candidate reputation archive reservation requires recovery: {error}"
            ))
            .into(),
        }
    }
}

impl CandidateArchiveReservations {
    /// Transfer the exact captures into the original candidate's preparation.
    ///
    /// A different State, Kura, archive configuration, context, or wire body cannot
    /// consume these owners. The caller must also hold its full resource admission
    /// before executing, then pass these captures to `PreparedCarrier::prepare_journals`.
    #[allow(clippy::type_complexity)]
    pub(crate) fn into_captures(
        self,
        service: &V2ApplyService,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<
        (
            Option<ProviderCandidateCapture>,
            Option<ReputationCandidateCapture>,
        ),
        (Self, V2ApplyError),
    > {
        if !Arc::ptr_eq(&self.state, &service.state)
            || !Arc::ptr_eq(&self.kura, &service.kura)
            || !same_archive(
                &self.provider_archive,
                &service.provider_ingest_finalized_archive,
            )
            || !same_archive(
                &self.reputation_archive,
                &service.reputation_finalized_archive,
            )
            || self.context_id != context.id()
        {
            return Err((self, V2ApplyError::TaskMismatch));
        }
        if let Err(error) = service.validate_archive_candidate_binding(context, body) {
            return Err((self, error));
        }
        match archive_proposal_wire_hash(body) {
            Ok(hash) if hash == self.proposal_wire_hash => Ok((self.provider, self.reputation)),
            Ok(_) => Err((self, V2ApplyError::TaskMismatch)),
            Err(error) => Err((self, error)),
        }
    }
}

fn same_archive<T>(original: &Option<Arc<T>>, actual: &Option<Arc<T>>) -> bool {
    match (original, actual) {
        (Some(original), Some(actual)) => Arc::ptr_eq(original, actual),
        (None, None) => true,
        _ => false,
    }
}

fn archive_proposal_wire_hash(body: &SignedBlock) -> Result<Hash, V2ApplyError> {
    body.encode_wire()
        .map(|wire| Hash::new(&wire))
        .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))
}
