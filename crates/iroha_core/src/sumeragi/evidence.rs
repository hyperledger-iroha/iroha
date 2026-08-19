//! Exact Sumeragi-v2 equivocation validation and WSV persistence.
//!
//! The first release accepts only complete signed v2 artifact pairs bound to
//! an authenticated immutable height context. Retired global-v1 evidence
//! layouts are not decoded, reconstructed, or persisted.
use super::consensus::Evidence;
use crate::state::{State, WorldReadOnly};
#[cfg(feature = "bls")]
use iroha_crypto::Algorithm;
use iroha_crypto::Signature;
use iroha_data_model::{
    NetworkId,
    block::{
        consensus::{EvidenceRecord, Height, SumeragiV2EquivocationEvidence, View},
        consensus_v2 as wire_v2,
    },
    consensus::NposPenaltyAction,
};
use mv::storage::StorageReadOnly;
use std::{
    collections::BTreeMap,
    convert::TryFrom,
    time::{SystemTime, UNIX_EPOCH},
};
/// Maximum exact Sumeragi v2 equivocation proofs admitted by one block.
///
/// The count bound makes proof verification work predictable even when a peer
/// has accumulated a large local evidence backlog.
pub(crate) const MAX_V2_EVIDENCE_ADMISSIONS_PER_BLOCK: usize = 8;
/// Maximum aggregate Norito size of exact Sumeragi v2 equivocation proofs in
/// one block.
pub(crate) const MAX_V2_EVIDENCE_ADMISSION_BYTES: usize = 4 * 1024 * 1024;
/// Context required to cryptographically validate consensus evidence.
#[derive(Debug, Clone, Copy)]
pub struct EvidenceValidationContext<'a> {
    /// Commit topology used to resolve validator indices for signature checks.
    pub topology: &'a super::network_topology::Topology,
    /// Exact genesis-derived network identity bound into consensus preimages.
    pub network_id: &'a NetworkId,
    /// Exact consensus mode authenticated for this height.
    pub mode: wire_v2::ConsensusMode,
}
/// Derive a deterministic deduplication key for an evidence entry.
#[must_use]
pub fn evidence_key(ev: &Evidence) -> Vec<u8> {
    let canonical = canonicalize_evidence(ev);
    evidence_key_inner(&canonical)
}
fn evidence_key_inner(ev: &Evidence) -> Vec<u8> {
    use norito::codec::Encode as _;

    ev.encode()
}
fn canonicalize_evidence(ev: &Evidence) -> Evidence {
    Evidence {
        equivocation: SumeragiV2EquivocationEvidence {
            context: ev.equivocation.context.clone(),
            proofs_of_possession: ev.equivocation.proofs_of_possession.clone(),
            conflict: canonicalize_v2_conflict(&ev.equivocation.conflict),
        },
    }
}
/// Return an exact v2 conflict with its signed artifacts in canonical wire order.
#[must_use]
pub(crate) fn canonicalize_v2_conflict(
    conflict: &wire_v2::SumeragiV2Equivocation,
) -> wire_v2::SumeragiV2Equivocation {
    use norito::codec::Encode;
    fn ordered<T: Clone + Encode>(first: &T, second: &T) -> (T, T) {
        if first.encode() <= second.encode() {
            (first.clone(), second.clone())
        } else {
            (second.clone(), first.clone())
        }
    }
    match conflict {
        wire_v2::SumeragiV2Equivocation::Proposal { first, second } => {
            let (first, second) = ordered(first, second);
            wire_v2::SumeragiV2Equivocation::Proposal { first, second }
        }
        wire_v2::SumeragiV2Equivocation::PhaseVote { first, second } => {
            let (first, second) = ordered(first, second);
            wire_v2::SumeragiV2Equivocation::PhaseVote { first, second }
        }
        wire_v2::SumeragiV2Equivocation::TimeoutVote { first, second } => {
            let (first, second) = ordered(first, second);
            wire_v2::SumeragiV2Equivocation::TimeoutVote { first, second }
        }
    }
}
/// Return an exact v2 proof with the conflicting pair in canonical order.
#[must_use]
pub(crate) fn canonicalize_v2_equivocation_evidence(
    evidence: &SumeragiV2EquivocationEvidence,
) -> SumeragiV2EquivocationEvidence {
    SumeragiV2EquivocationEvidence {
        context: evidence.context.clone(),
        proofs_of_possession: evidence.proofs_of_possession.clone(),
        conflict: canonicalize_v2_conflict(&evidence.conflict),
    }
}
/// Wrap an exact v2 proof in the durable evidence representation.
#[must_use]
pub(crate) fn canonical_v2_evidence(evidence: &SumeragiV2EquivocationEvidence) -> Evidence {
    Evidence {
        equivocation: canonicalize_v2_equivocation_evidence(evidence),
    }
}
/// Return the canonical WSV key for an exact v2 equivocation proof.
#[must_use]
pub(crate) fn v2_evidence_admission_key(evidence: &SumeragiV2EquivocationEvidence) -> Vec<u8> {
    evidence_key_inner(&canonical_v2_evidence(evidence))
}
fn validate_v2_evidence_context_anchor(
    state: &State,
    evidence: &SumeragiV2EquivocationEvidence,
) -> Result<(), EvidenceValidationError> {
    let persisted = state
        .sumeragi_v2_height_context(evidence.context.height)
        .map_err(|_| EvidenceValidationError::V2AdmissionContextUnavailable)?
        .ok_or(EvidenceValidationError::V2AdmissionContextUnavailable)?;
    if persisted != evidence.context {
        return Err(EvidenceValidationError::V2AdmissionContextMismatch);
    }
    Ok(())
}
fn v2_evidence_matches_persisted_context(
    evidence: &SumeragiV2EquivocationEvidence,
    persisted: &wire_v2::HeightContext,
) -> bool {
    persisted == &evidence.context
}
/// Validate the exact v2 evidence admitted by a candidate block.
///
/// Validation is self-contained: a follower does not need to have observed or
/// persisted either conflicting artifact before receiving the candidate. The
/// embedded context is anchored to immutable committed context history.
///
/// # Errors
///
/// Returns an error for an oversized or non-canonical batch, unavailable or
/// mismatched context provenance, stale/future/replayed evidence, or any
/// structural or cryptographic proof failure.
pub(crate) fn validate_v2_evidence_admissions(
    state: &State,
    block_height: u64,
    admissions: &[SumeragiV2EquivocationEvidence],
) -> Result<Vec<Vec<u8>>, EvidenceValidationError> {
    use norito::codec::Encode as _;
    if admissions.len() > MAX_V2_EVIDENCE_ADMISSIONS_PER_BLOCK {
        return Err(EvidenceValidationError::V2AdmissionTooMany);
    }
    let total_bytes = admissions.iter().try_fold(0_usize, |total, evidence| {
        total.checked_add(evidence.encode().len())
    });
    if total_bytes.is_none_or(|size| size > MAX_V2_EVIDENCE_ADMISSION_BYTES) {
        return Err(EvidenceValidationError::V2AdmissionTooLarge);
    }
    let horizon = {
        let world = state.world_view();
        world
            .sumeragi_npos_parameters()
            .map(|params| params.evidence_horizon_blocks())
    };
    let records = state.world.consensus_evidence.view();
    let mut keys = Vec::with_capacity(admissions.len());
    let mut previous_key: Option<Vec<u8>> = None;
    for evidence in admissions {
        if &evidence.context.network_id != state.network_id_ref() {
            return Err(EvidenceValidationError::V2AdmissionWrongNetwork);
        }
        let round = v2_conflict_round(&evidence.conflict);
        if round.height >= block_height {
            return Err(EvidenceValidationError::V2AdmissionNotPrior);
        }
        if !evidence_within_configured_horizon(block_height, horizon, Some(round.height)) {
            return Err(EvidenceValidationError::V2AdmissionStale);
        }
        let canonical = canonicalize_v2_equivocation_evidence(evidence);
        if canonical != *evidence {
            return Err(EvidenceValidationError::V2AdmissionNonCanonical);
        }
        let key = v2_evidence_admission_key(evidence);
        if previous_key
            .as_ref()
            .is_some_and(|previous| previous >= &key)
        {
            return Err(EvidenceValidationError::V2AdmissionOrder);
        }
        if records
            .get(&key)
            .is_some_and(|record| record.consensus_admitted_at_height.is_some())
        {
            return Err(EvidenceValidationError::V2AdmissionAlreadyCommitted);
        }
        validate_v2_evidence_context_anchor(state, evidence)?;
        validate_v2_equivocation(evidence)?;
        previous_key = Some(key.clone());
        keys.push(key);
    }
    Ok(keys)
}
/// Reject penalty actions which consume evidence admitted by the same block.
///
/// Penalties must be derived exclusively from prior committed state so every
/// validator computes the same attachment despite asymmetric artifact gossip.
///
/// # Errors
///
/// Returns an error when a slash or evidence-applied marker references one of
/// the keys admitted by the same block.
pub(crate) fn validate_v2_admission_penalty_separation(
    admission_keys: &[Vec<u8>],
    actions: &[NposPenaltyAction],
) -> Result<(), EvidenceValidationError> {
    let conflicts = actions.iter().any(|action| {
        let evidence_key = match action {
            NposPenaltyAction::ConsensusSlash(action) => Some(&action.evidence_key),
            NposPenaltyAction::MarkConsensusEvidenceApplied(action) => Some(&action.evidence_key),
            NposPenaltyAction::VrfJail(_) | NposPenaltyAction::MarkVrfPenaltiesApplied(_) => None,
        };
        evidence_key.is_some_and(|key| admission_keys.binary_search(key).is_ok())
    });
    if conflicts {
        Err(EvidenceValidationError::V2AdmissionSameBlockPenalty)
    } else {
        Ok(())
    }
}
/// Select a canonical, bounded batch of locally pending exact v2 proofs for a
/// proposer to attach to its next candidate.
#[must_use]
pub(crate) fn pending_v2_evidence_admissions(
    state: &State,
    proposal_height: u64,
) -> Vec<SumeragiV2EquivocationEvidence> {
    use norito::codec::Encode as _;
    let horizon = {
        let world = state.world_view();
        world
            .sumeragi_npos_parameters()
            .map(|params| params.evidence_horizon_blocks())
    };
    let records = state.world.consensus_evidence.view();
    let mut persisted_contexts = BTreeMap::new();
    let mut selected = Vec::new();
    let mut selected_bytes = 0_usize;
    let mut previous_key: Option<Vec<u8>> = None;
    for (stored_key, record) in records.iter() {
        if selected.len() == MAX_V2_EVIDENCE_ADMISSIONS_PER_BLOCK {
            break;
        }
        if record.consensus_admitted_at_height.is_some()
            || record.penalty_applied
            || record.penalty_cancelled
        {
            continue;
        }
        let evidence = canonicalize_v2_equivocation_evidence(&record.evidence.equivocation);
        let round = v2_conflict_round(&evidence.conflict);
        if &evidence.context.network_id != state.network_id_ref()
            || round.height >= proposal_height
            || !evidence_within_configured_horizon(proposal_height, horizon, Some(round.height))
        {
            continue;
        }
        let context_anchored = persisted_contexts
            .entry(round.height)
            .or_insert_with(|| {
                state
                    .sumeragi_v2_height_context(round.height)
                    .ok()
                    .flatten()
            })
            .as_ref()
            .is_some_and(|persisted| v2_evidence_matches_persisted_context(&evidence, persisted));
        if !context_anchored || validate_v2_equivocation(&evidence).is_err() {
            continue;
        }
        let key = v2_evidence_admission_key(&evidence);
        if &key != stored_key
            || previous_key
                .as_ref()
                .is_some_and(|previous| previous >= &key)
        {
            continue;
        }
        let encoded_len = evidence.encode().len();
        let Some(next_bytes) = selected_bytes.checked_add(encoded_len) else {
            continue;
        };
        if next_bytes > MAX_V2_EVIDENCE_ADMISSION_BYTES {
            continue;
        }
        selected.push(evidence);
        selected_bytes = next_bytes;
        previous_key = Some(key);
    }
    drop(records);
    selected
}
/// Validate and durably persist exact Sumeragi v2 equivocation artifacts.
///
/// The caller supplies the immutable context and PoPs recovered from the
/// trusted context store. They are copied into the record only after the full
/// pair passes structural, roster, PoP, and individual-signature validation.
/// A canonical WSV key makes exact replay and swapped-pair replay idempotent.
pub(crate) fn persist_sumeragi_v2_equivocation(
    state: &State,
    context: &wire_v2::HeightContext,
    proofs_of_possession: &[Vec<u8>],
    conflict: wire_v2::SumeragiV2Equivocation,
) -> Result<bool, EvidenceValidationError> {
    let payload = SumeragiV2EquivocationEvidence {
        context: context.clone(),
        proofs_of_possession: proofs_of_possession.to_vec(),
        conflict,
    };
    validate_v2_equivocation(&payload)?;
    let canonical = canonicalize_evidence(&Evidence {
        equivocation: payload,
    });
    Ok(persist_validated_record(state, canonical))
}
fn persist_validated_record(state: &State, canonical: Evidence) -> bool {
    let fallback_height = u64::try_from(state.committed_height()).unwrap_or(0);
    let horizon = {
        let world = state.world_view();
        world
            .sumeragi_npos_parameters()
            .map(|params| params.evidence_horizon_blocks())
    };
    let key = evidence_key_inner(&canonical);
    let view = state.world.consensus_evidence.view();
    if view.get(&key).is_some() {
        return false;
    }
    drop(view);
    let (subject_height, subject_view) = evidence_subject_height_view(&canonical);
    if !evidence_within_configured_horizon(fallback_height, horizon, subject_height) {
        return false;
    }
    let recorded_at_height = subject_height.unwrap_or(fallback_height);
    let recorded_at_view = subject_view.unwrap_or_default();
    let recorded_at_ms = current_unix_ms();
    let record = EvidenceRecord {
        evidence: canonical,
        recorded_at_height,
        recorded_at_view,
        recorded_at_ms,
        penalty_applied: false,
        penalty_cancelled: false,
        penalty_cancelled_at_height: None,
        penalty_applied_at_height: None,
        consensus_admitted_at_height: None,
    };
    let mut block = state.world.consensus_evidence.block();
    block.insert(key, record);
    block.commit();
    true
}
/// Extract the height/view referenced by consensus evidence, when present.
pub fn evidence_subject_height_view(evidence: &Evidence) -> (Option<Height>, Option<View>) {
    let round = v2_conflict_round(&evidence.equivocation.conflict);
    (Some(round.height), Some(round.view))
}
fn v2_conflict_round(conflict: &wire_v2::SumeragiV2Equivocation) -> wire_v2::ConsensusRound {
    match conflict {
        wire_v2::SumeragiV2Equivocation::Proposal { first, .. } => first.round,
        wire_v2::SumeragiV2Equivocation::PhaseVote { first, .. } => first.round,
        wire_v2::SumeragiV2Equivocation::TimeoutVote { first, .. } => first.round,
    }
}
fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(0)
}
fn evidence_within_configured_horizon(
    current_height: u64,
    horizon: Option<u64>,
    subject_height: Option<u64>,
) -> bool {
    let Some(horizon) = horizon else { return true };
    if horizon == 0 {
        return true;
    }
    let reference = subject_height.unwrap_or(current_height);
    let lower_bound = current_height.saturating_sub(horizon);
    reference >= lower_bound
}
/// Errors returned by [`validate_evidence`] when the supplied [`Evidence`] fails basic
/// structural consistency checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceValidationError {
    /// Embedded Sumeragi v2 height context is structurally invalid.
    V2ContextInvalid,
    /// Sumeragi v2 evidence PoPs are not aligned with the frozen roster.
    V2ProofCountMismatch,
    /// A Sumeragi v2 voting key is not BLS-normal.
    V2NonBlsValidator,
    /// A Sumeragi v2 roster proof of possession is invalid.
    V2ProofOfPossessionInvalid,
    /// One Sumeragi v2 artifact is invalid under the embedded context.
    V2ArtifactInvalid,
    /// Sumeragi v2 artifacts target different rounds or phases.
    V2RoundMismatch,
    /// Sumeragi v2 artifacts name different signers.
    V2SignerMismatch,
    /// The two Sumeragi v2 artifacts make the same signed statement.
    V2ArtifactsDoNotConflict,
    /// A Sumeragi v2 individual or aggregate signature is invalid.
    V2SignatureInvalid,
    /// This build cannot verify mandatory Sumeragi v2 BLS material.
    V2CryptographyUnavailable,
    /// A candidate carries more exact v2 proofs than one block may admit.
    V2AdmissionTooMany,
    /// A candidate's exact v2 proof batch exceeds the aggregate byte bound.
    V2AdmissionTooLarge,
    /// A candidate's exact v2 proof is bound to another genesis-derived network.
    V2AdmissionWrongNetwork,
    /// The deterministic v2 context history needed to anchor a proof is absent.
    V2AdmissionContextUnavailable,
    /// A proof's embedded context differs from committed v2 context history.
    V2AdmissionContextMismatch,
    /// A candidate tries to admit evidence from its own or a future height.
    V2AdmissionNotPrior,
    /// A candidate tries to admit evidence outside the configured horizon.
    V2AdmissionStale,
    /// A candidate's exact v2 proof does not use canonical pair ordering.
    V2AdmissionNonCanonical,
    /// Exact v2 admissions are duplicated or not in increasing key order.
    V2AdmissionOrder,
    /// Exact v2 evidence was already admitted by a prior committed block.
    V2AdmissionAlreadyCommitted,
    /// A candidate tries to admit and penalize exact v2 evidence atomically.
    V2AdmissionSameBlockPenalty,
}
impl std::fmt::Display for EvidenceValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use EvidenceValidationError::*;
        let msg = match self {
            V2ContextInvalid => "Sumeragi v2 evidence height context is invalid",
            V2ProofCountMismatch => {
                "Sumeragi v2 evidence PoP count does not match the frozen roster"
            }
            V2NonBlsValidator => "Sumeragi v2 evidence roster contains a non-BLS validator",
            V2ProofOfPossessionInvalid => {
                "Sumeragi v2 evidence contains an invalid proof of possession"
            }
            V2ArtifactInvalid => "Sumeragi v2 evidence contains an invalid artifact",
            V2RoundMismatch => "Sumeragi v2 evidence artifacts target different slots",
            V2SignerMismatch => "Sumeragi v2 evidence artifacts have different signers",
            V2ArtifactsDoNotConflict => {
                "Sumeragi v2 evidence artifacts do not make conflicting statements"
            }
            V2SignatureInvalid => "Sumeragi v2 evidence signature verification failed",
            V2CryptographyUnavailable => {
                "Sumeragi v2 evidence requires unavailable BLS verification support"
            }
            V2AdmissionTooMany => "too many Sumeragi v2 evidence admissions in one block",
            V2AdmissionTooLarge => "Sumeragi v2 evidence admission batch exceeds byte limit",
            V2AdmissionWrongNetwork => "Sumeragi v2 evidence admission belongs to another network",
            V2AdmissionContextUnavailable => {
                "committed Sumeragi v2 context history is unavailable for evidence admission"
            }
            V2AdmissionContextMismatch => {
                "Sumeragi v2 evidence context differs from committed context history"
            }
            V2AdmissionNotPrior => {
                "Sumeragi v2 evidence admission must precede the admitting block"
            }
            V2AdmissionStale => "Sumeragi v2 evidence admission is outside the evidence horizon",
            V2AdmissionNonCanonical => {
                "Sumeragi v2 evidence admission has non-canonical artifact order"
            }
            V2AdmissionOrder => {
                "Sumeragi v2 evidence admissions are duplicated or not canonically ordered"
            }
            V2AdmissionAlreadyCommitted => {
                "Sumeragi v2 evidence was already admitted by a committed block"
            }
            V2AdmissionSameBlockPenalty => {
                "Sumeragi v2 evidence cannot be admitted and penalized in the same block"
            }
        };
        write!(f, "{msg}")
    }
}
impl std::error::Error for EvidenceValidationError {}
/// Validate exact Sumeragi-v2 equivocation evidence against trusted chain identity.
///
/// Retired global-v1 double-vote, invalid-certificate, invalid-proposal, and
/// censorship layouts fail closed. The first release accepts only exact signed
/// v2 artifact pairs bound to one immutable height context.
///
/// # Errors
///
/// Returns [`EvidenceValidationError`] when the provided evidence violates one of the
/// trusted network/roster context is absent or any exact v2 artifact is invalid.
pub fn validate_evidence(
    evidence: &Evidence,
    context: &EvidenceValidationContext<'_>,
) -> Result<(), EvidenceValidationError> {
    let evidence = &evidence.equivocation;
    if &evidence.context.network_id != context.network_id {
        return Err(EvidenceValidationError::V2ContextInvalid);
    }
    if evidence.context.mode != context.mode {
        return Err(EvidenceValidationError::V2ContextInvalid);
    }
    let trusted_roster = context.topology.as_ref();
    if trusted_roster.len() != evidence.context.roster.len()
        || trusted_roster
            .iter()
            .zip(&evidence.context.roster)
            .any(|(trusted, embedded)| trusted != &embedded.validator)
    {
        return Err(EvidenceValidationError::V2ContextInvalid);
    }
    validate_v2_equivocation(evidence)
}
pub(crate) fn validate_v2_equivocation(
    evidence: &SumeragiV2EquivocationEvidence,
) -> Result<(), EvidenceValidationError> {
    let context = &evidence.context;
    context
        .validate()
        .map_err(|_| EvidenceValidationError::V2ContextInvalid)?;
    validate_v2_roster_proofs(context, &evidence.proofs_of_possession)?;
    match &evidence.conflict {
        wire_v2::SumeragiV2Equivocation::Proposal { first, second } => {
            first
                .validate(context)
                .and_then(|()| second.validate(context))
                .map_err(|_| EvidenceValidationError::V2ArtifactInvalid)?;
            if first.round != second.round {
                return Err(EvidenceValidationError::V2RoundMismatch);
            }
            if first.proposer != second.proposer {
                return Err(EvidenceValidationError::V2SignerMismatch);
            }
            if first.subject == second.subject && first.manifest == second.manifest {
                return Err(EvidenceValidationError::V2ArtifactsDoNotConflict);
            }
            verify_v2_proposal_justification(
                context,
                &evidence.proofs_of_possession,
                &first.justification,
            )?;
            verify_v2_proposal_justification(
                context,
                &evidence.proofs_of_possession,
                &second.justification,
            )?;
            verify_v2_individual_signature(
                context,
                first.proposer,
                &first.signature,
                &first.signature_preimage(),
            )?;
            verify_v2_individual_signature(
                context,
                second.proposer,
                &second.signature,
                &second.signature_preimage(),
            )
        }
        wire_v2::SumeragiV2Equivocation::PhaseVote { first, second } => {
            first
                .validate(context)
                .and_then(|()| second.validate(context))
                .map_err(|_| EvidenceValidationError::V2ArtifactInvalid)?;
            if first.round != second.round || first.phase != second.phase {
                return Err(EvidenceValidationError::V2RoundMismatch);
            }
            if first.signer != second.signer {
                return Err(EvidenceValidationError::V2SignerMismatch);
            }
            if first.proposal_round == second.proposal_round
                && first.subject == second.subject
                && first.execution_commitment == second.execution_commitment
            {
                return Err(EvidenceValidationError::V2ArtifactsDoNotConflict);
            }
            verify_v2_individual_signature(
                context,
                first.signer,
                &first.signature,
                &first.signature_preimage(),
            )?;
            verify_v2_individual_signature(
                context,
                second.signer,
                &second.signature,
                &second.signature_preimage(),
            )
        }
        wire_v2::SumeragiV2Equivocation::TimeoutVote { first, second } => {
            first
                .validate(context)
                .and_then(|()| second.validate(context))
                .map_err(|_| EvidenceValidationError::V2ArtifactInvalid)?;
            if first.round != second.round {
                return Err(EvidenceValidationError::V2RoundMismatch);
            }
            if first.signer != second.signer {
                return Err(EvidenceValidationError::V2SignerMismatch);
            }
            if first
                .highest_prepare_qc
                .as_ref()
                .map(wire_v2::QuorumCertificate::as_ref)
                == second
                    .highest_prepare_qc
                    .as_ref()
                    .map(wire_v2::QuorumCertificate::as_ref)
            {
                return Err(EvidenceValidationError::V2ArtifactsDoNotConflict);
            }
            if let Some(certificate) = &first.highest_prepare_qc {
                verify_v2_quorum_certificate(context, &evidence.proofs_of_possession, certificate)?;
            }
            if let Some(certificate) = &second.highest_prepare_qc {
                verify_v2_quorum_certificate(context, &evidence.proofs_of_possession, certificate)?;
            }
            verify_v2_individual_signature(
                context,
                first.signer,
                &first.signature,
                &first.signature_preimage(),
            )?;
            verify_v2_individual_signature(
                context,
                second.signer,
                &second.signature,
                &second.signature_preimage(),
            )
        }
    }
}
fn validate_v2_roster_proofs(
    context: &wire_v2::HeightContext,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), EvidenceValidationError> {
    if proofs_of_possession.len() != context.roster.len() {
        return Err(EvidenceValidationError::V2ProofCountMismatch);
    }
    #[cfg(not(feature = "bls"))]
    {
        let _ = (context, proofs_of_possession);
        Err(EvidenceValidationError::V2CryptographyUnavailable)
    }
    #[cfg(feature = "bls")]
    {
        for (entry, proof) in context.roster.iter().zip(proofs_of_possession) {
            if entry.validator.public_key().try_algorithm() != Ok(Algorithm::BlsNormal) {
                return Err(EvidenceValidationError::V2NonBlsValidator);
            }
            iroha_crypto::bls_normal_pop_verify(entry.validator.public_key(), proof)
                .map_err(|_| EvidenceValidationError::V2ProofOfPossessionInvalid)?;
        }
        Ok(())
    }
}
fn verify_v2_individual_signature(
    context: &wire_v2::HeightContext,
    signer: wire_v2::ValidatorIndex,
    signature: &[u8],
    preimage: &[u8],
) -> Result<(), EvidenceValidationError> {
    let index = usize::try_from(signer)
        .ok()
        .filter(|index| *index < context.roster.len())
        .ok_or(EvidenceValidationError::V2SignerMismatch)?;
    let signature = Signature::try_from_bytes(signature)
        .map_err(|_| EvidenceValidationError::V2SignatureInvalid)?;
    signature
        .verify(context.roster[index].validator.public_key(), preimage)
        .map_err(|_| EvidenceValidationError::V2SignatureInvalid)
}
fn verify_v2_proposal_justification(
    context: &wire_v2::HeightContext,
    proofs_of_possession: &[Vec<u8>],
    justification: &wire_v2::ProposalJustification,
) -> Result<(), EvidenceValidationError> {
    match justification {
        wire_v2::ProposalJustification::ParentCommit(_) => {
            // The current context already binds the semantic parent decision.
            // Its aggregate was verified against the parent context before
            // this immutable context record was persisted.
            Ok(())
        }
        wire_v2::ProposalJustification::Timeout(timeout) => {
            verify_v2_timeout_certificate(
                context,
                proofs_of_possession,
                &timeout.timeout_certificate,
            )?;
            if let Some(certificate) = &timeout.highest_prepare_qc {
                verify_v2_quorum_certificate(context, proofs_of_possession, certificate)?;
            }
            Ok(())
        }
    }
}
fn verify_v2_quorum_certificate(
    context: &wire_v2::HeightContext,
    proofs_of_possession: &[Vec<u8>],
    certificate: &wire_v2::QuorumCertificate,
) -> Result<(), EvidenceValidationError> {
    certificate
        .validate(context)
        .map_err(|_| EvidenceValidationError::V2ArtifactInvalid)?;
    let signer = certificate
        .signers
        .first()
        .copied()
        .ok_or(EvidenceValidationError::V2ArtifactInvalid)?;
    let preimage = certificate
        .signer_preimage(context, signer)
        .map_err(|_| EvidenceValidationError::V2ArtifactInvalid)?;
    verify_v2_aggregate_signature(
        context,
        proofs_of_possession,
        &certificate.signers,
        &certificate.aggregate_signature,
        &preimage,
    )
}
fn verify_v2_timeout_certificate(
    context: &wire_v2::HeightContext,
    proofs_of_possession: &[Vec<u8>],
    certificate: &wire_v2::TimeoutCertificate,
) -> Result<(), EvidenceValidationError> {
    certificate
        .validate(context)
        .map_err(|_| EvidenceValidationError::V2ArtifactInvalid)?;
    for group in &certificate.groups {
        if let Some(highest) = &group.highest_prepare_qc {
            verify_v2_quorum_certificate(context, proofs_of_possession, highest)?;
        }
        let signer = group
            .signers
            .first()
            .copied()
            .ok_or(EvidenceValidationError::V2ArtifactInvalid)?;
        let preimage = wire_v2::TimeoutVote {
            round: certificate.round,
            highest_prepare_qc: group.highest_prepare_qc.clone(),
            signer,
            signature: Vec::new(),
        }
        .signature_preimage();
        verify_v2_aggregate_signature(
            context,
            proofs_of_possession,
            &group.signers,
            &group.aggregate_signature,
            &preimage,
        )?;
    }
    Ok(())
}
fn verify_v2_aggregate_signature(
    context: &wire_v2::HeightContext,
    proofs_of_possession: &[Vec<u8>],
    signers: &[wire_v2::ValidatorIndex],
    aggregate_signature: &[u8],
    preimage: &[u8],
) -> Result<(), EvidenceValidationError> {
    let mut public_keys = Vec::with_capacity(signers.len());
    let mut proofs = Vec::with_capacity(signers.len());
    for signer in signers {
        let index = usize::try_from(*signer)
            .ok()
            .filter(|index| *index < context.roster.len() && *index < proofs_of_possession.len())
            .ok_or(EvidenceValidationError::V2SignerMismatch)?;
        public_keys.push(context.roster[index].validator.public_key());
        proofs.push(proofs_of_possession[index].as_slice());
    }
    #[cfg(feature = "bls")]
    {
        iroha_crypto::bls_normal_verify_preaggregated_same_message(
            preimage,
            aggregate_signature,
            &public_keys,
            &proofs,
        )
        .map_err(|_| EvidenceValidationError::V2SignatureInvalid)
    }
    #[cfg(not(feature = "bls"))]
    {
        let _ = (public_keys, proofs, aggregate_signature, preimage);
        Err(EvidenceValidationError::V2CryptographyUnavailable)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{State, World};
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{
        NetworkId,
        block::BlockHeader,
        parameter::{Parameter, Parameters, system::SumeragiNposParameters},
        peer::PeerId,
        prelude::ChainId,
    };
    use mv::cell::Cell;
    fn test_network_id(seed: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            seed,
        )))
    }
    fn test_state() -> State {
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        State::new_for_testing(World::default(), kura, query)
    }
    fn test_state_for_network(network_id: NetworkId) -> State {
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        State::new_with_chain_and_network_id_for_testing(
            World::default(),
            kura,
            query,
            ChainId::from("evidence-display-name"),
            network_id,
        )
    }
    fn test_state_for_v2_fixture(fixture: &V2EvidenceFixture) -> State {
        test_state_for_v2_fixture_with_world(fixture, World::default())
    }
    fn test_state_for_v2_fixture_with_horizon(fixture: &V2EvidenceFixture, horizon: u64) -> State {
        let mut params = Parameters::default();
        let npos = SumeragiNposParameters {
            evidence_horizon_blocks: horizon,
            ..SumeragiNposParameters::default()
        };
        params.set_parameter(Parameter::Custom(npos.into_custom_parameter()));
        let mut world = World::default();
        world.parameters = Cell::new(params);
        test_state_for_v2_fixture_with_world(fixture, world)
    }
    fn test_state_for_v2_fixture_with_slashing_delay(
        fixture: &V2EvidenceFixture,
        slashing_delay_blocks: u64,
    ) -> State {
        let mut params = Parameters::default();
        let npos = SumeragiNposParameters {
            slashing_delay_blocks,
            ..SumeragiNposParameters::default()
        };
        params.set_parameter(Parameter::Custom(npos.into_custom_parameter()));
        let mut world = World::default();
        world.parameters = Cell::new(params);
        test_state_for_v2_fixture_with_world(fixture, world)
    }
    fn test_state_for_v2_fixture_with_world(fixture: &V2EvidenceFixture, world: World) -> State {
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain_and_network_id_for_testing(
            world,
            kura,
            query,
            ChainId::from("sumeragi-v2-evidence-display-name"),
            fixture.context.network_id,
        );
        install_v2_finality_for_fixture(&state, fixture);
        state
    }
    struct V2EvidenceFixture {
        context: wire_v2::HeightContext,
        keys: Vec<KeyPair>,
        proofs: Vec<Vec<u8>>,
    }
    impl V2EvidenceFixture {
        fn new() -> Self {
            let mut keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic Sumeragi v2 evidence key")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let roster = keys
                .iter()
                .map(|key| wire_v2::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                })
                .collect::<Vec<_>>();
            let context = wire_v2::HeightContext {
                network_id: test_network_id(b"sumeragi-v2-evidence-genesis"),
                protocol_version: wire_v2::PROTOCOL_VERSION,
                height: 1,
                epoch: 7,
                epoch_end_height: u64::MAX,
                next_epoch_snapshot: None,
                snapshot_bootstrap: None,
                mode: wire_v2::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                quorum: wire_v2::DualQuorum::from_roster(&roster).expect("equal-vote quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"v2-evidence-context"),
                execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
                da_layout: wire_v2::DataAvailabilityLayout {
                    encoding: wire_v2::PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 32,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 1024,
                    max_chunk_count: 64,
                },
                leader_seed: [0x51; 32],
            };
            context.validate().expect("valid v2 evidence context");
            let proofs = keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("v2 evidence PoP")
                })
                .collect();
            Self {
                context,
                keys,
                proofs,
            }
        }
        fn round(&self, view: u64) -> wire_v2::ConsensusRound {
            wire_v2::ConsensusRound {
                context_id: self.context.id(),
                height: self.context.height,
                view,
            }
        }
        fn subject(&self, seed: u8) -> wire_v2::BlockSubject {
            wire_v2::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32])),
                payload_hash: Hash::prehashed([seed.wrapping_add(1); 32]),
            }
        }
        fn sign(&self, signer: wire_v2::ValidatorIndex, preimage: &[u8]) -> Vec<u8> {
            Signature::try_new(
                self.keys[usize::try_from(signer).expect("signer index")].private_key(),
                preimage,
            )
            .expect("v2 evidence signature")
            .payload()
            .to_vec()
        }
        fn execution_commitment(&self) -> wire_v2::ExecutionCommitment {
            wire_v2::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"v2 evidence parent state"),
                Hash::new(b"v2 evidence post state"),
                Hash::new(b"v2 evidence ordinary writes"),
                1,
                Hash::new(b"v2 evidence executed block wire"),
            )
        }
        fn vote(
            &self,
            signer: wire_v2::ValidatorIndex,
            phase: wire_v2::GlobalPhase,
            subject: wire_v2::BlockSubject,
        ) -> wire_v2::Vote {
            let mut vote = wire_v2::Vote {
                round: self.round(0),
                proposal_round: self.round(0),
                phase,
                subject,
                execution_commitment: self.execution_commitment(),
                signer,
                signature: Vec::new(),
            };
            vote.signature = self.sign(signer, &vote.signature_preimage());
            vote
        }
        fn prepare_qc(&self, subject: wire_v2::BlockSubject) -> wire_v2::QuorumCertificate {
            let signers = vec![0, 1, 2];
            let unsigned = wire_v2::Vote {
                round: self.round(0),
                proposal_round: self.round(0),
                phase: wire_v2::GlobalPhase::Prepare,
                subject,
                execution_commitment: self.execution_commitment(),
                signer: 0,
                signature: Vec::new(),
            };
            let shares = signers
                .iter()
                .map(|signer| self.sign(*signer, &unsigned.signature_preimage()))
                .collect::<Vec<_>>();
            let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
            wire_v2::QuorumCertificate {
                round: unsigned.round,
                proposal_round: unsigned.proposal_round,
                phase: unsigned.phase,
                subject,
                execution_commitment: unsigned.execution_commitment,
                signers,
                aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                    .expect("aggregate v2 evidence QC"),
            }
        }
        fn proposal(&self, subject: wire_v2::BlockSubject) -> wire_v2::Proposal {
            let round = self.round(0);
            let proposer = self.context.leader(0);
            let body = [subject.payload_hash.as_ref()[0]];
            let chunks = wire_v2::encode_payload_chunks(self.context.da_layout, &body)
                .expect("encode complete v2 evidence fixture chunks");
            // Evidence cases supply the exact subject under test, so derive
            // against that subject after constructing canonical RS16 chunks.
            let manifest = wire_v2::PayloadManifest::derive(
                &self.context,
                round,
                subject,
                u64::try_from(body.len()).expect("small evidence fixture body length fits u64"),
                &chunks,
            )
            .expect("v2 evidence manifest");
            let mut proposal = wire_v2::Proposal {
                round,
                proposer,
                subject,
                manifest,
                justification: wire_v2::ProposalJustification::ParentCommit(
                    wire_v2::ParentCommitJustification { certificate: None },
                ),
                signature: Vec::new(),
            };
            proposal.signature = self.sign(proposer, &proposal.signature_preimage());
            proposal
        }
        fn timeout_vote(
            &self,
            signer: wire_v2::ValidatorIndex,
            highest_prepare_qc: Option<wire_v2::QuorumCertificate>,
        ) -> wire_v2::TimeoutVote {
            let mut vote = wire_v2::TimeoutVote {
                round: self.round(0),
                highest_prepare_qc,
                signer,
                signature: Vec::new(),
            };
            vote.signature = self.sign(signer, &vote.signature_preimage());
            vote
        }
        fn payload(
            &self,
            conflict: wire_v2::SumeragiV2Equivocation,
        ) -> SumeragiV2EquivocationEvidence {
            SumeragiV2EquivocationEvidence {
                context: self.context.clone(),
                proofs_of_possession: self.proofs.clone(),
                conflict,
            }
        }
    }
    fn install_v2_finality_for_fixture(state: &State, fixture: &V2EvidenceFixture) {
        let committed = crate::block::ValidBlock::new_dummy_and_modify_header(
            fixture.keys[0].private_key(),
            |header| {
                header.set_height(core::num::NonZeroU64::new(1).expect("non-zero height"));
                header.set_prev_block_hash(None);
                header.merkle_root = None;
            },
        )
        .commit_unchecked()
        .unpack(|_| {});
        let block: std::sync::Arc<iroha_data_model::block::SignedBlock> =
            std::sync::Arc::new(committed.into());
        state
            .kura()
            .store_block(std::sync::Arc::clone(&block))
            .expect("store canonical v2 evidence fixture block");
        let subject = wire_v2::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: block
                .canonical_proposal_wire_hash()
                .expect("canonical proposal wire"),
        };
        let execution_commitment = wire_v2::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"v2 evidence finality parent state"),
            Hash::new(b"v2 evidence finality post state"),
            Hash::new(b"v2 evidence finality ordinary writes"),
            u64::try_from(block.encode_wire().expect("v2 evidence block wire").len())
                .expect("v2 evidence block wire length fits u64"),
            block
                .executed_block_wire_hash()
                .expect("canonical executed block wire"),
        );
        let round = wire_v2::ConsensusRound {
            context_id: fixture.context.id(),
            height: fixture.context.height,
            view: block.header().view_change_index(),
        };
        let mut certificate = wire_v2::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire_v2::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x5A; 48],
        };
        let preimage = certificate
            .signer_preimage(&fixture.context, 0)
            .expect("valid finality fixture signer");
        let shares = fixture.keys[..3]
            .iter()
            .map(|key| {
                Signature::try_new(key.private_key(), &preimage)
                    .expect("sign finality fixture vote")
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        certificate.aggregate_signature =
            iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate finality fixture CommitQC");
        let _ = state
            .kura()
            .store_v2_finality_artifact(&wire_v2::finality::V2FinalityArtifact::new(
                fixture.context.clone(),
                subject,
                certificate,
                fixture.proofs.clone(),
            ))
            .expect("persist canonical v2 evidence fixture finality");
    }
    fn swap_v2_conflict(
        conflict: &wire_v2::SumeragiV2Equivocation,
    ) -> wire_v2::SumeragiV2Equivocation {
        match conflict {
            wire_v2::SumeragiV2Equivocation::Proposal { first, second } => {
                wire_v2::SumeragiV2Equivocation::Proposal {
                    first: second.clone(),
                    second: first.clone(),
                }
            }
            wire_v2::SumeragiV2Equivocation::PhaseVote { first, second } => {
                wire_v2::SumeragiV2Equivocation::PhaseVote {
                    first: second.clone(),
                    second: first.clone(),
                }
            }
            wire_v2::SumeragiV2Equivocation::TimeoutVote { first, second } => {
                wire_v2::SumeragiV2Equivocation::TimeoutVote {
                    first: second.clone(),
                    second: first.clone(),
                }
            }
        }
    }
    fn canonical_v2_phase_vote_evidence(
        fixture: &V2EvidenceFixture,
        first_seed: u8,
        second_seed: u8,
    ) -> SumeragiV2EquivocationEvidence {
        canonicalize_v2_equivocation_evidence(&fixture.payload(
            wire_v2::SumeragiV2Equivocation::PhaseVote {
                first: fixture.vote(
                    1,
                    wire_v2::GlobalPhase::Prepare,
                    fixture.subject(first_seed),
                ),
                second: fixture.vote(
                    1,
                    wire_v2::GlobalPhase::Prepare,
                    fixture.subject(second_seed),
                ),
            },
        ))
    }
    fn apply_v2_admissions_for_test(
        state: &State,
        admissions: Vec<SumeragiV2EquivocationEvidence>,
        height: u64,
        view: u64,
        now_ms: u64,
    ) {
        let header = BlockHeader::new(
            core::num::NonZeroU64::new(height).expect("non-zero test height"),
            None,
            None,
            None,
            now_ms,
            view,
        );
        let mut state_block = state.block(header);
        let effects = iroha_data_model::consensus::NposConsensusEffects {
            vrf_epoch_seals: Vec::new(),
            v2_evidence_admissions: admissions,
            penalty_actions: Vec::new(),
        };
        let mut transaction = state_block.transaction();
        super::super::penalties::apply_npos_consensus_effects_to_transaction(
            &mut transaction,
            &effects,
            height,
            view,
            now_ms,
            #[cfg(feature = "telemetry")]
            None,
        )
        .expect("valid exact v2 admission applies");
        transaction.apply();
        state_block.commit().expect("test admission block commits");
    }
    fn add_v2_penalty_validator(state: &State, peer: &PeerId) {
        let validator = iroha_data_model::account::AccountId::new(peer.public_key().clone());
        let record = iroha_data_model::nexus::PublicLaneValidatorRecord {
            lane_id: iroha_data_model::nexus::LaneId::SINGLE,
            validator: validator.clone(),
            peer_id: peer.clone(),
            stake_account: validator.clone(),
            total_stake: iroha_primitives::numeric::Quantity::from(100_u64),
            self_stake: iroha_primitives::numeric::Quantity::from(100_u64),
            metadata: iroha_data_model::metadata::Metadata::default(),
            status: iroha_data_model::nexus::PublicLaneValidatorStatus::Active,
            activation_epoch: None,
            activation_height: None,
            last_reward_epoch: None,
        };
        let mut validators = state.world.public_lane_validators.block();
        validators.insert((iroha_data_model::nexus::LaneId::SINGLE, validator), record);
        validators.commit();
    }
    #[test]
    fn sumeragi_v2_equivocation_validates_exact_proposal_vote_and_timeout_pairs() {
        let fixture = V2EvidenceFixture::new();
        let subject_a = fixture.subject(0x61);
        let subject_b = fixture.subject(0x62);
        let proposal = fixture.payload(wire_v2::SumeragiV2Equivocation::Proposal {
            first: fixture.proposal(subject_a),
            second: fixture.proposal(subject_b),
        });
        validate_v2_equivocation(&proposal).expect("valid double proposal");
        let phase_vote = fixture.payload(wire_v2::SumeragiV2Equivocation::PhaseVote {
            first: fixture.vote(1, wire_v2::GlobalPhase::Prepare, subject_a),
            second: fixture.vote(1, wire_v2::GlobalPhase::Prepare, subject_b),
        });
        validate_v2_equivocation(&phase_vote).expect("valid double phase vote");
        let timeout = fixture.payload(wire_v2::SumeragiV2Equivocation::TimeoutVote {
            first: fixture.timeout_vote(2, None),
            second: fixture.timeout_vote(2, Some(fixture.prepare_qc(subject_a))),
        });
        validate_v2_equivocation(&timeout).expect("valid double timeout vote");
    }
    #[test]
    fn sumeragi_v2_equivocation_authenticates_vote_origin_and_execution() {
        let fixture = V2EvidenceFixture::new();
        let subject = fixture.subject(0x6a);
        let signer = 1;
        let mut first = fixture.vote(signer, wire_v2::GlobalPhase::Commit, subject);
        first.round = fixture.round(2);
        first.proposal_round = first.round;
        first.signature = fixture.sign(signer, &first.signature_preimage());
        let mut different_origin = first.clone();
        different_origin.proposal_round = fixture.round(1);
        different_origin.signature = fixture.sign(signer, &different_origin.signature_preimage());
        let origin_conflict = fixture.payload(wire_v2::SumeragiV2Equivocation::PhaseVote {
            first: first.clone(),
            second: different_origin,
        });
        assert_eq!(
            validate_v2_equivocation(&origin_conflict),
            Err(EvidenceValidationError::V2ArtifactInvalid),
            "a vote whose proposal origin differs from its certified round is not canonical evidence"
        );
        let mut different_execution = first.clone();
        different_execution.execution_commitment.post_state_root =
            Hash::new(b"different v2 evidence post state");
        different_execution.signature =
            fixture.sign(signer, &different_execution.signature_preimage());
        let execution_conflict = fixture.payload(wire_v2::SumeragiV2Equivocation::PhaseVote {
            first,
            second: different_execution,
        });
        validate_v2_equivocation(&execution_conflict)
            .expect("different authenticated execution commitments conflict");
    }
    #[test]
    fn sumeragi_v2_equivocation_generic_ingress_anchors_network_and_roster() {
        let fixture = V2EvidenceFixture::new();
        let peers = fixture
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let topology = super::super::network_topology::Topology::new(peers.clone());
        let evidence = Evidence {
            equivocation: fixture.payload(wire_v2::SumeragiV2Equivocation::PhaseVote {
                first: fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x68)),
                second: fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x69)),
            }),
        };
        let context = EvidenceValidationContext {
            topology: &topology,
            network_id: &fixture.context.network_id,
            mode: wire_v2::ConsensusMode::Permissioned,
        };
        validate_evidence(&evidence, &context).expect("trusted v2 evidence ingress");
        let shortened_topology =
            super::super::network_topology::Topology::new(peers.into_iter().skip(1));
        let untrusted_roster = EvidenceValidationContext {
            topology: &shortened_topology,
            ..context
        };
        assert_eq!(
            validate_evidence(&evidence, &untrusted_roster),
            Err(EvidenceValidationError::V2ContextInvalid)
        );
        let foreign_network = test_network_id(b"foreign-v2-evidence-genesis");
        let wrong_network = EvidenceValidationContext {
            network_id: &foreign_network,
            ..context
        };
        assert_eq!(
            validate_evidence(&evidence, &wrong_network),
            Err(EvidenceValidationError::V2ContextInvalid)
        );
    }
    #[test]
    fn sumeragi_v2_equivocation_rejects_forgery_wrong_slot_and_duplicates() {
        let fixture = V2EvidenceFixture::new();
        let subject_a = fixture.subject(0x71);
        let subject_b = fixture.subject(0x72);
        let first = fixture.vote(1, wire_v2::GlobalPhase::Commit, subject_a);
        let second = fixture.vote(1, wire_v2::GlobalPhase::Commit, subject_b);
        let duplicate = fixture.payload(wire_v2::SumeragiV2Equivocation::PhaseVote {
            first: first.clone(),
            second: first.clone(),
        });
        assert_eq!(
            validate_v2_equivocation(&duplicate),
            Err(EvidenceValidationError::V2ArtifactsDoNotConflict)
        );
        let mut forged_pop = fixture.payload(wire_v2::SumeragiV2Equivocation::PhaseVote {
            first: first.clone(),
            second: second.clone(),
        });
        forged_pop.proofs_of_possession[1][0] ^= 0x80;
        assert_eq!(
            validate_v2_equivocation(&forged_pop),
            Err(EvidenceValidationError::V2ProofOfPossessionInvalid)
        );
        let mut missing_pop = fixture.payload(wire_v2::SumeragiV2Equivocation::PhaseVote {
            first: first.clone(),
            second: second.clone(),
        });
        missing_pop.proofs_of_possession.pop();
        assert_eq!(
            validate_v2_equivocation(&missing_pop),
            Err(EvidenceValidationError::V2ProofCountMismatch)
        );
        let mut forged_qc = fixture.prepare_qc(subject_a);
        forged_qc.aggregate_signature[0] ^= 0x80;
        let forged_qc = fixture.payload(wire_v2::SumeragiV2Equivocation::TimeoutVote {
            first: fixture.timeout_vote(1, None),
            second: fixture.timeout_vote(1, Some(forged_qc)),
        });
        assert_eq!(
            validate_v2_equivocation(&forged_qc),
            Err(EvidenceValidationError::V2SignatureInvalid)
        );
        let mut forged_signature = second.clone();
        forged_signature.signature[0] ^= 0x80;
        let forged_signature = fixture.payload(wire_v2::SumeragiV2Equivocation::PhaseVote {
            first: first.clone(),
            second: forged_signature,
        });
        assert_eq!(
            validate_v2_equivocation(&forged_signature),
            Err(EvidenceValidationError::V2SignatureInvalid)
        );
        let mut forged_signer = second.clone();
        forged_signer.signer = 2;
        let forged_signer = fixture.payload(wire_v2::SumeragiV2Equivocation::PhaseVote {
            first: first.clone(),
            second: forged_signer,
        });
        assert_eq!(
            validate_v2_equivocation(&forged_signer),
            Err(EvidenceValidationError::V2SignerMismatch)
        );
        let mut wrong_round = second.clone();
        wrong_round.round.view = 1;
        let wrong_round = fixture.payload(wire_v2::SumeragiV2Equivocation::PhaseVote {
            first: first.clone(),
            second: wrong_round,
        });
        assert!(matches!(
            validate_v2_equivocation(&wrong_round),
            Err(EvidenceValidationError::V2ArtifactInvalid
                | EvidenceValidationError::V2RoundMismatch)
        ));
        let mut wrong_context = second;
        wrong_context.round.context_id =
            wire_v2::HeightContextId(HashOf::from_untyped_unchecked(Hash::prehashed([0xFF; 32])));
        let wrong_context = fixture.payload(wire_v2::SumeragiV2Equivocation::PhaseVote {
            first,
            second: wrong_context,
        });
        assert_eq!(
            validate_v2_equivocation(&wrong_context),
            Err(EvidenceValidationError::V2ArtifactInvalid)
        );
    }
    include!("evidence/missing_signer_pop_test.rs");
    #[test]
    fn sumeragi_v2_equivocation_persistence_deduplicates_swaps_and_restart_replay() {
        let fixture = V2EvidenceFixture::new();
        let conflict = wire_v2::SumeragiV2Equivocation::PhaseVote {
            first: fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x81)),
            second: fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x82)),
        };
        let state = test_state();
        assert!(
            persist_sumeragi_v2_equivocation(
                &state,
                &fixture.context,
                &fixture.proofs,
                conflict.clone(),
            )
            .expect("persist valid v2 evidence")
        );
        assert!(
            !persist_sumeragi_v2_equivocation(
                &state,
                &fixture.context,
                &fixture.proofs,
                swap_v2_conflict(&conflict),
            )
            .expect("swapped replay is valid")
        );
        // A fresh in-memory evidence service after restart still observes the
        // canonical WSV key written by the previous service incarnation.
        assert!(
            !persist_sumeragi_v2_equivocation(&state, &fixture.context, &fixture.proofs, conflict,)
                .expect("exact restart replay is valid")
        );
        let records = state.world.consensus_evidence.view();
        assert_eq!(records.iter().count(), 1);
        let (_, record) = records.iter().next().expect("stored v2 evidence");
        assert_eq!(record.evidence.equivocation.context, fixture.context);
    }
    #[test]
    fn sumeragi_v2_equivocation_persistence_rejects_invalid_artifacts() {
        let fixture = V2EvidenceFixture::new();
        let mut forged = fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x84));
        forged.signature[0] ^= 0x80;
        let conflict = wire_v2::SumeragiV2Equivocation::PhaseVote {
            first: fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x83)),
            second: forged,
        };
        let state = test_state();
        assert_eq!(
            persist_sumeragi_v2_equivocation(&state, &fixture.context, &fixture.proofs, conflict,),
            Err(EvidenceValidationError::V2SignatureInvalid)
        );
        assert_eq!(state.world.consensus_evidence.view().iter().count(), 0);
    }
    #[test]
    fn v2_admission_validates_without_follower_local_observation() {
        let fixture = V2EvidenceFixture::new();
        let evidence = canonical_v2_phase_vote_evidence(&fixture, 0x91, 0x92);
        let follower = test_state_for_v2_fixture(&fixture);
        let keys = validate_v2_evidence_admissions(&follower, 2, &[evidence.clone()])
            .expect("self-contained exact proof must validate on an unaware follower");
        assert_eq!(keys, vec![v2_evidence_admission_key(&evidence)]);
        assert_eq!(follower.world.consensus_evidence.view().iter().count(), 0);
    }
    #[test]
    fn v2_admission_rejects_context_store_only_recovery_record() {
        let fixture = V2EvidenceFixture::new();
        let evidence = canonical_v2_phase_vote_evidence(&fixture, 0x91, 0x92);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let verified = super::super::v2::VerifiedHeightContext::genesis(
            fixture.context.clone(),
            fixture.proofs.clone(),
        )
        .expect("verified fixture height context");
        let store =
            super::super::v2_context_store::V2ContextStore::open(kura.sumeragi_v2_storage_root())
                .expect("open fixture context store");
        store
            .persist(
                &super::super::v2_context_store::PersistedHeightContext::from_verified(&verified),
            )
            .expect("persist fixture height context");
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain_and_network_id_for_testing(
            World::default(),
            kura,
            query,
            ChainId::from("sumeragi-v2-evidence-display-name"),
            fixture.context.network_id,
        );
        assert_eq!(
            state
                .sumeragi_v2_height_context(fixture.context.height)
                .expect("inspect finality-only historical context"),
            None,
            "a checksummed recovery context is not committed authorization"
        );
        assert_eq!(
            validate_v2_evidence_admissions(&state, 2, &[evidence]),
            Err(EvidenceValidationError::V2AdmissionContextUnavailable)
        );
    }
    #[test]
    fn v2_admission_rejects_noncanonical_duplicate_reordered_and_oversize_batches() {
        let fixture = V2EvidenceFixture::new();
        let state = test_state_for_v2_fixture(&fixture);
        let first = canonical_v2_phase_vote_evidence(&fixture, 0x93, 0x94);
        let second = canonical_v2_phase_vote_evidence(&fixture, 0x95, 0x96);
        let mut noncanonical = first.clone();
        noncanonical.conflict = swap_v2_conflict(&noncanonical.conflict);
        assert_eq!(
            validate_v2_evidence_admissions(&state, 2, &[noncanonical]),
            Err(EvidenceValidationError::V2AdmissionNonCanonical)
        );
        assert_eq!(
            validate_v2_evidence_admissions(&state, 2, &[first.clone(), first.clone()]),
            Err(EvidenceValidationError::V2AdmissionOrder)
        );
        let mut ordered = vec![first.clone(), second];
        ordered.sort_by_key(v2_evidence_admission_key);
        validate_v2_evidence_admissions(&state, 2, &ordered)
            .expect("strictly increasing canonical admission keys");
        ordered.reverse();
        assert_eq!(
            validate_v2_evidence_admissions(&state, 2, &ordered),
            Err(EvidenceValidationError::V2AdmissionOrder)
        );
        let oversized = vec![first; MAX_V2_EVIDENCE_ADMISSIONS_PER_BLOCK + 1];
        assert_eq!(
            validate_v2_evidence_admissions(&state, 2, &oversized),
            Err(EvidenceValidationError::V2AdmissionTooMany)
        );
    }
    #[test]
    fn v2_admission_rejects_forged_foreign_future_stale_and_committed_proofs() {
        let fixture = V2EvidenceFixture::new();
        let state = test_state_for_v2_fixture(&fixture);
        let evidence = canonical_v2_phase_vote_evidence(&fixture, 0x97, 0x98);
        let mut forged = evidence.clone();
        let wire_v2::SumeragiV2Equivocation::PhaseVote { second, .. } = &mut forged.conflict else {
            unreachable!("phase-vote fixture")
        };
        second.signature[0] ^= 0x80;
        forged = canonicalize_v2_equivocation_evidence(&forged);
        assert_eq!(
            validate_v2_evidence_admissions(&state, 2, &[forged]),
            Err(EvidenceValidationError::V2SignatureInvalid)
        );
        let foreign = test_state_for_network(test_network_id(b"foreign-v2-admission-genesis"));
        assert_eq!(
            validate_v2_evidence_admissions(&foreign, 2, &[evidence.clone()]),
            Err(EvidenceValidationError::V2AdmissionWrongNetwork)
        );
        let missing_context = test_state_for_network(fixture.context.network_id);
        assert_eq!(
            validate_v2_evidence_admissions(&missing_context, 2, &[evidence.clone()]),
            Err(EvidenceValidationError::V2AdmissionContextUnavailable)
        );
        let mut mismatched_fixture = V2EvidenceFixture::new();
        mismatched_fixture.context.leader_seed = [0xD7; 32];
        mismatched_fixture
            .context
            .validate()
            .expect("mismatched context remains structurally valid");
        let mismatched = canonical_v2_phase_vote_evidence(&mismatched_fixture, 0x97, 0x98);
        assert_eq!(
            validate_v2_evidence_admissions(&state, 2, &[mismatched]),
            Err(EvidenceValidationError::V2AdmissionContextMismatch)
        );
        assert_eq!(
            validate_v2_evidence_admissions(&state, 1, &[evidence.clone()]),
            Err(EvidenceValidationError::V2AdmissionNotPrior)
        );
        let stale_state = test_state_for_v2_fixture_with_horizon(&fixture, 1);
        assert_eq!(
            validate_v2_evidence_admissions(&stale_state, 4, &[evidence.clone()]),
            Err(EvidenceValidationError::V2AdmissionStale)
        );
        let key = v2_evidence_admission_key(&evidence);
        let mut records = state.world.consensus_evidence.block();
        records.insert(
            key,
            EvidenceRecord {
                evidence: canonical_v2_evidence(&evidence),
                recorded_at_height: 2,
                recorded_at_view: 0,
                recorded_at_ms: 20,
                penalty_applied: false,
                penalty_cancelled: false,
                penalty_cancelled_at_height: None,
                penalty_applied_at_height: None,
                consensus_admitted_at_height: Some(2),
            },
        );
        records.commit();
        assert_eq!(
            validate_v2_evidence_admissions(&state, 3, &[evidence]),
            Err(EvidenceValidationError::V2AdmissionAlreadyCommitted)
        );
    }
    #[test]
    fn asymmetric_v2_observation_converges_after_committed_admission() {
        let fixture = V2EvidenceFixture::new();
        let proposer = test_state_for_v2_fixture_with_slashing_delay(&fixture, 1);
        let follower = test_state_for_v2_fixture_with_slashing_delay(&fixture, 1);
        let offender = fixture.context.roster[1].validator.clone();
        add_v2_penalty_validator(&proposer, &offender);
        add_v2_penalty_validator(&follower, &offender);
        let conflict = wire_v2::SumeragiV2Equivocation::PhaseVote {
            first: fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x99)),
            second: fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x9A)),
        };
        persist_sumeragi_v2_equivocation(&proposer, &fixture.context, &fixture.proofs, conflict)
            .expect("local exact proof validates");
        let admissions = pending_v2_evidence_admissions(&proposer, 2);
        assert_eq!(admissions.len(), 1);
        assert!(pending_v2_evidence_admissions(&follower, 2).is_empty());
        validate_v2_evidence_admissions(&follower, 2, &admissions)
            .expect("unaware follower revalidates the attached exact proof");
        let proposer_precommit = super::super::penalties::PenaltyApplier::from_committed_state(
            &proposer,
            iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(2, Vec::new())
        .expect("derive proposer pre-admission effects");
        let follower_precommit = super::super::penalties::PenaltyApplier::from_committed_state(
            &follower,
            iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(2, Vec::new())
        .expect("derive follower pre-admission effects");
        assert_eq!(proposer_precommit.v2_evidence_admissions, admissions);
        assert!(follower_precommit.v2_evidence_admissions.is_empty());
        assert_eq!(
            proposer_precommit.penalty_actions,
            follower_precommit.penalty_actions
        );
        assert!(proposer_precommit.penalty_actions.is_empty());
        apply_v2_admissions_for_test(&proposer, admissions.clone(), 2, 3, 77);
        apply_v2_admissions_for_test(&follower, admissions.clone(), 2, 3, 77);
        let key = v2_evidence_admission_key(&admissions[0]);
        let proposer_record = proposer
            .world
            .consensus_evidence
            .view()
            .get(&key)
            .cloned()
            .expect("proposer committed evidence record");
        let follower_record = follower
            .world
            .consensus_evidence
            .view()
            .get(&key)
            .cloned()
            .expect("follower committed evidence record");
        assert_eq!(proposer_record, follower_record);
        assert_eq!(proposer_record.consensus_admitted_at_height, Some(2));
        assert_eq!(proposer_record.recorded_at_height, 2);
        assert_eq!(proposer_record.recorded_at_view, 3);
        assert_eq!(proposer_record.recorded_at_ms, 77);
        let proposer_same_block = super::super::penalties::PenaltyApplier::from_committed_state(
            &proposer,
            iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(2, Vec::new())
        .expect("derive proposer same-height effects");
        assert!(proposer_same_block.penalty_actions.is_empty());
        let proposer_effects = super::super::penalties::PenaltyApplier::from_committed_state(
            &proposer,
            iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(3, Vec::new())
        .expect("derive proposer post-admission effects");
        let follower_effects = super::super::penalties::PenaltyApplier::from_committed_state(
            &follower,
            iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(3, Vec::new())
        .expect("derive follower post-admission effects");
        assert_eq!(proposer_effects, follower_effects);
        assert!(
            proposer_effects
                .penalty_actions
                .iter()
                .any(|action| matches!(action, NposPenaltyAction::ConsensusSlash(_)))
        );
    }
    #[test]
    fn v2_admission_rejects_same_block_consensus_slash() {
        let fixture = V2EvidenceFixture::new();
        let evidence = canonical_v2_phase_vote_evidence(&fixture, 0x9B, 0x9C);
        let key = v2_evidence_admission_key(&evidence);
        let peer = fixture.context.roster[1].validator.clone();
        let slash = NposPenaltyAction::ConsensusSlash(
            iroha_data_model::consensus::NposConsensusSlashAction {
                evidence_key: key.clone(),
                signer: 1,
                peer_id: peer.clone(),
                lane_id: iroha_data_model::nexus::LaneId::SINGLE,
                validator: iroha_data_model::account::AccountId::new(peer.public_key().clone()),
                slash_id: Hash::new(key.clone()),
                amount: iroha_primitives::numeric::Quantity::from(1_u64),
            },
        );
        assert_eq!(
            validate_v2_admission_penalty_separation(&[key], &[slash]),
            Err(EvidenceValidationError::V2AdmissionSameBlockPenalty)
        );
    }
}
