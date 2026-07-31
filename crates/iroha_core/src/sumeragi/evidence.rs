//! Minimal evidence validation helpers plus WSV persistence wiring.
//! This module provides helpers to construct evidence for double-votes,
//! basic commit-certificate shape checks, an in-memory deduplication store for the Sumeragi
//! actor, exact Sumeragi v2 equivocation-pair verification, and routines that
//! persist new evidence records into the world state.

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryFrom,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(test)]
use iroha_crypto::HashOf;
use iroha_crypto::{Algorithm, Signature};
#[cfg(test)]
use iroha_data_model::block::BlockHeader;
use iroha_data_model::{
    block::{
        consensus::{EvidenceRecord, Height, SumeragiV2EquivocationEvidence, View},
        consensus_v2 as wire_v2,
    },
    consensus::NposPenaltyAction,
    peer::PeerId,
    prelude::ChainId,
};
use mv::storage::StorageReadOnly;

use super::consensus::{
    Evidence, EvidenceKind, EvidencePayload, NPOS_TAG, PERMISSIONED_TAG, Phase, Vote, vote_preimage,
};
use crate::state::{State, WorldReadOnly};

/// Minimum expected length for BLS signatures attached to consensus votes.
///
/// Consensus validators authenticate votes with BLS-normal signatures (96 bytes). Treating this
/// as a lower bound catches truncated payloads emitted by buggy or malicious peers.
const MIN_BLS_SIGNATURE_LEN: usize = 96;

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
    /// Chain identifier bound into consensus preimages.
    pub chain_id: &'a ChainId,
    /// Consensus mode tag (permissioned or `NPoS`) for preimage separation.
    pub mode_tag: &'a str,
    /// Optional PRF seed for `NPoS` topology rotation.
    pub prf_seed: Option<[u8; 32]>,
}

/// Reconstruct the legacy signature roster for archival evidence validation.
///
/// Live Sumeragi v2 never calls this path: v2 votes carry the frozen
/// [`iroha_data_model::block::consensus_v2::HeightContext`] identity and are
/// authenticated before entering the reducer. Keeping the rotation beside the
/// archival validator prevents the retired actor from remaining a compiled
/// dependency merely to inspect historical evidence.
fn archival_topology_for_view(
    topology: &super::network_topology::Topology,
    height: u64,
    view: u64,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> super::network_topology::Topology {
    let mut rotated = topology.clone();
    rotated.canonicalize_order();
    match mode_tag {
        PERMISSIONED_TAG => {
            if let Some(seed) = prf_seed {
                rotated.shuffle_prf(seed, height);
            }
            rotated.nth_rotation(view);
        }
        NPOS_TAG => {
            if let Some(seed) = prf_seed {
                let leader = rotated.leader_index_prf(seed, height, view);
                rotated.rotate_preserve_view_to_front(leader);
            }
        }
        _ => {}
    }
    rotated
}

fn archival_vote_signature_check(
    vote: &Vote,
    topology: &super::network_topology::Topology,
    chain_id: &ChainId,
    mode_tag: &str,
) -> Result<(), EvidenceValidationError> {
    let index =
        usize::try_from(vote.signer).map_err(|_| EvidenceValidationError::SignatureInvalid)?;
    let peer = topology
        .as_ref()
        .get(index)
        .ok_or(EvidenceValidationError::SignatureInvalid)?;
    if vote.bls_sig.is_empty() {
        return Err(EvidenceValidationError::SignatureInvalid);
    }
    let signature = match peer.public_key().try_algorithm() {
        Ok(Algorithm::Ed25519) => iroha_crypto::ed25519_parse_signature(&vote.bls_sig),
        Ok(Algorithm::MlDsa) => iroha_crypto::mldsa65_parse_signature(&vote.bls_sig),
        Ok(_) => Signature::try_from_bytes(&vote.bls_sig).map_err(iroha_crypto::Error::from),
        Err(_) => return Err(EvidenceValidationError::SignatureInvalid),
    }
    .map_err(|_| EvidenceValidationError::SignatureInvalid)?;
    signature
        .verify(peer.public_key(), &vote_preimage(chain_id, mode_tag, vote))
        .map_err(|_| EvidenceValidationError::SignatureInvalid)
}

/// Derive a deterministic deduplication key for an evidence entry.
#[must_use]
pub fn evidence_key(ev: &Evidence) -> Vec<u8> {
    let canonical = canonicalize_evidence(ev);
    evidence_key_inner(&canonical)
}

fn evidence_key_inner(ev: &Evidence) -> Vec<u8> {
    use norito::codec::Encode as _;
    let mut key = Vec::new();
    key.push(ev.kind as u8);
    key.extend_from_slice(&ev.encode());
    key
}

fn canonicalize_evidence(ev: &Evidence) -> Evidence {
    match &ev.payload {
        EvidencePayload::DoubleVote { v1, v2 } => {
            let (first, second) = canonical_vote_pair(v1, v2);
            Evidence {
                kind: ev.kind,
                payload: EvidencePayload::DoubleVote {
                    v1: first,
                    v2: second,
                },
            }
        }
        EvidencePayload::Censorship { tx_hash, receipts } => Evidence {
            kind: ev.kind,
            payload: EvidencePayload::Censorship {
                tx_hash: *tx_hash,
                receipts: canonicalize_censorship_receipts(receipts),
            },
        },
        EvidencePayload::SumeragiV2Equivocation(evidence) => Evidence {
            kind: ev.kind,
            payload: EvidencePayload::SumeragiV2Equivocation(SumeragiV2EquivocationEvidence {
                context: evidence.context.clone(),
                proofs_of_possession: evidence.proofs_of_possession.clone(),
                conflict: canonicalize_v2_conflict(&evidence.conflict),
            }),
        },
        _ => ev.clone(),
    }
}

fn canonicalize_v2_conflict(
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
        kind: EvidenceKind::SumeragiV2Equivocation,
        payload: EvidencePayload::SumeragiV2Equivocation(canonicalize_v2_equivocation_evidence(
            evidence,
        )),
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
        if &evidence.context.chain_id != state.chain_id_ref() {
            return Err(EvidenceValidationError::V2AdmissionWrongChain);
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
        let EvidencePayload::SumeragiV2Equivocation(evidence) = &record.evidence.payload else {
            continue;
        };
        let evidence = canonicalize_v2_equivocation_evidence(evidence);
        let round = v2_conflict_round(&evidence.conflict);
        if &evidence.context.chain_id != state.chain_id_ref()
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

fn canonicalize_censorship_receipts(
    receipts: &[iroha_data_model::transaction::TransactionSubmissionReceipt],
) -> Vec<iroha_data_model::transaction::TransactionSubmissionReceipt> {
    use norito::codec::Encode as _;
    let mut keyed: Vec<_> = receipts
        .iter()
        .cloned()
        .map(|receipt| (receipt.encode(), receipt))
        .collect();
    keyed.sort_by(|(left, _), (right, _)| left.cmp(right));
    keyed.into_iter().map(|(_, receipt)| receipt).collect()
}

fn double_vote_kind_for_phases(first: Phase, second: Phase) -> Option<EvidenceKind> {
    match (first, second) {
        (Phase::Prepare, Phase::Prepare) => Some(EvidenceKind::DoublePrepare),
        (Phase::Commit, Phase::Commit | Phase::Prepare) | (Phase::Prepare, Phase::Commit) => {
            // Cross-phase equivocation in the same round is treated as a commit double-vote since
            // the validator advanced while committing to conflicting blocks.
            Some(EvidenceKind::DoubleCommit)
        }
        _ => None,
    }
}

fn canonical_vote_pair(v1: &Vote, v2: &Vote) -> (Vote, Vote) {
    let left = (
        v1.phase as u8,
        v1.block_hash.as_ref(),
        v1.parent_state_root,
        v1.post_state_root,
    );
    let right = (
        v2.phase as u8,
        v2.block_hash.as_ref(),
        v2.parent_state_root,
        v2.post_state_root,
    );
    if left <= right {
        (v1.clone(), v2.clone())
    } else {
        (v2.clone(), v1.clone())
    }
}

/// Check for a double-vote: same validator at the same height/view/epoch on conflicting blocks.
pub fn check_double_vote(v1: &Vote, v2: &Vote) -> Option<Evidence> {
    if v1.height == v2.height
        && v1.view == v2.view
        && v1.epoch == v2.epoch
        && v1.signer == v2.signer
    {
        let conflicts = if v1.block_hash != v2.block_hash {
            true
        } else if v1.phase == Phase::Commit && v2.phase == Phase::Commit {
            v1.parent_state_root != v2.parent_state_root || v1.post_state_root != v2.post_state_root
        } else {
            false
        };
        if conflicts {
            let (first, second) = canonical_vote_pair(v1, v2);
            return double_vote_kind_for_phases(first.phase, second.phase).map(|kind| Evidence {
                kind,
                payload: EvidencePayload::DoubleVote {
                    v1: first,
                    v2: second,
                },
            });
        }
        None
    } else {
        None
    }
}

fn signer_peer_for_vote(
    vote: &Vote,
    context: &EvidenceValidationContext<'_>,
) -> Result<PeerId, EvidenceValidationError> {
    let signature_topology = archival_topology_for_view(
        context.topology,
        vote.height,
        vote.view,
        context.mode_tag,
        context.prf_seed,
    );
    usize::try_from(vote.signer)
        .ok()
        .and_then(|idx| signature_topology.as_ref().get(idx).cloned())
        .ok_or(EvidenceValidationError::SignerMismatch)
}

#[cfg(test)]
fn check_double_vote_with_context(
    v1: &Vote,
    v2: &Vote,
    context: &EvidenceValidationContext<'_>,
) -> Option<Evidence> {
    if v1.height != v2.height || v1.view != v2.view || v1.epoch != v2.epoch {
        return None;
    }
    let peer_a = signer_peer_for_vote(v1, context).ok()?;
    let peer_b = signer_peer_for_vote(v2, context).ok()?;
    if peer_a != peer_b {
        return None;
    }
    let conflicts = if v1.block_hash != v2.block_hash {
        true
    } else if v1.phase == Phase::Commit && v2.phase == Phase::Commit {
        v1.parent_state_root != v2.parent_state_root || v1.post_state_root != v2.post_state_root
    } else {
        false
    };
    if !conflicts {
        return None;
    }
    let (first, second) = canonical_vote_pair(v1, v2);
    double_vote_kind_for_phases(first.phase, second.phase).map(|kind| Evidence {
        kind,
        payload: EvidencePayload::DoubleVote {
            v1: first,
            v2: second,
        },
    })
}

/// Simple in-memory evidence store to deduplicate by a deterministic key.
#[derive(Default)]
#[cfg(test)]
pub struct EvidenceStore {
    // Deterministic key set of evidence entries (for quick membership and count)
    seen: BTreeSet<Vec<u8>>, // keys are hashed payloads
    // Optional payload map for audit/listing
    entries: BTreeMap<Vec<u8>, Evidence>,
}

#[cfg(test)]
impl EvidenceStore {
    fn new() -> Self {
        Self {
            seen: BTreeSet::new(),
            entries: BTreeMap::new(),
        }
    }

    /// Insert evidence if unseen. Returns true if newly inserted.
    fn insert(&mut self, ev: &Evidence, context: &EvidenceValidationContext<'_>) -> bool {
        let canonical = canonicalize_evidence(ev);
        if validate_evidence(&canonical, context).is_err() {
            return false;
        }
        let key = evidence_key_inner(&canonical);
        if self.seen.insert(key.clone()) {
            self.entries.insert(key, canonical);
            true
        } else {
            false
        }
    }
}

/// Persist an [`EvidenceRecord`] into the world state if unseen.
///
/// Returns `true` when the supplied evidence was newly inserted, `false`
/// when an identical entry already exists in storage.
pub fn persist_record(
    state: &State,
    evidence: &Evidence,
    context: &EvidenceValidationContext<'_>,
) -> bool {
    let canonical = canonicalize_evidence(evidence);
    if validate_evidence(&canonical, context).is_err() {
        return false;
    }
    persist_validated_record(state, canonical)
}

/// Validate and durably persist exact Sumeragi v2 equivocation artifacts.
///
/// The caller supplies the immutable context and PoPs recovered from the
/// trusted context store. They are copied into the record only after the full
/// pair passes structural, roster, PoP, and individual-signature validation.
/// A canonical WSV key makes exact replay and swapped-pair replay idempotent.
#[cfg(test)]
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
        kind: EvidenceKind::SumeragiV2Equivocation,
        payload: EvidencePayload::SumeragiV2Equivocation(payload),
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

/// Detect and persist a double-vote in isolated archival-evidence tests.
///
/// Returns `true` when evidence was newly recorded (store + WSV), `false` otherwise.
#[cfg(test)]
pub fn record_double_vote(
    store: &mut EvidenceStore,
    state: &State,
    previous: &Vote,
    current: &Vote,
    context: &EvidenceValidationContext<'_>,
) -> bool {
    let Some(evidence) = check_double_vote_with_context(previous, current, context) else {
        return false;
    };
    if !store.insert(&evidence, context) {
        return false;
    }

    persist_record(state, &evidence, context)
}

/// Extract the height/view referenced by consensus evidence, when present.
pub fn evidence_subject_height_view(evidence: &Evidence) -> (Option<Height>, Option<View>) {
    match &evidence.payload {
        EvidencePayload::DoubleVote { v1, .. } => (Some(v1.height), Some(v1.view)),
        EvidencePayload::InvalidQc { certificate, .. } => {
            (Some(certificate.height), Some(certificate.view))
        }
        EvidencePayload::InvalidProposal { proposal, .. } => {
            (Some(proposal.header.height), Some(proposal.header.view))
        }
        EvidencePayload::Censorship { receipts, .. } => {
            let height = receipts
                .iter()
                .map(|receipt| receipt.payload.submitted_at_height)
                .max();
            (height, None)
        }
        EvidencePayload::SumeragiV2Equivocation(evidence) => {
            let round = v2_conflict_round(&evidence.conflict);
            (Some(round.height), Some(round.view))
        }
    }
}

#[cfg(test)]
fn evidence_block_refs(evidence: &Evidence) -> Vec<(u64, HashOf<BlockHeader>)> {
    let mut refs = Vec::new();
    match &evidence.payload {
        EvidencePayload::DoubleVote { v1, v2 } => {
            refs.push((v1.height, v1.block_hash));
            if v2.block_hash != v1.block_hash {
                refs.push((v2.height, v2.block_hash));
            }
        }
        EvidencePayload::InvalidQc { certificate, .. } => {
            refs.push((certificate.height, certificate.subject_block_hash));
        }
        EvidencePayload::SumeragiV2Equivocation(evidence) => match &evidence.conflict {
            wire_v2::SumeragiV2Equivocation::Proposal { first, second } => {
                refs.push((first.round.height, first.subject.block_hash));
                if first.subject.block_hash != second.subject.block_hash {
                    refs.push((second.round.height, second.subject.block_hash));
                }
            }
            wire_v2::SumeragiV2Equivocation::PhaseVote { first, second } => {
                refs.push((first.round.height, first.subject.block_hash));
                if first.subject.block_hash != second.subject.block_hash {
                    refs.push((second.round.height, second.subject.block_hash));
                }
            }
            wire_v2::SumeragiV2Equivocation::TimeoutVote { .. } => {}
        },
        EvidencePayload::InvalidProposal { .. } | EvidencePayload::Censorship { .. } => {}
    }
    refs
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
    /// Evidence validation was requested under a retired or unknown signature domain.
    UnsupportedModeTag,
    /// Invalid-QC claims have no typed self-verifying proof in protocol v1.
    UnverifiableInvalidQc,
    /// Invalid-proposal claims have no typed self-verifying proof in protocol v1.
    UnverifiableInvalidProposal,
    /// [`EvidenceKind`] does not match the payload variant.
    KindPayloadMismatch,
    /// Double-vote evidence carries votes for mismatched phases.
    PhaseMismatch,
    /// Double-vote evidence carries votes for different block heights.
    HeightMismatch,
    /// Double-vote evidence carries votes from different consensus views.
    ViewMismatch,
    /// Double-vote evidence carries votes for different epochs.
    EpochMismatch,
    /// Double-vote evidence carries votes signed by different validators.
    SignerMismatch,
    /// Double-vote evidence references the same block hash for both votes.
    BlockHashMatch,
    /// Double-vote evidence phase disagrees with its [`EvidenceKind`].
    PhaseKindMismatch,
    /// Evidence references votes that lack the expected BLS signature payload.
    SignatureMissing,
    /// Evidence references votes whose signatures appear truncated or forged.
    SignatureTruncated,
    /// Evidence references votes whose signatures fail cryptographic verification.
    SignatureInvalid,
    /// Censorship evidence carries no receipts.
    ReceiptMissing,
    /// Censorship evidence receipts refer to different transaction hashes.
    ReceiptTxHashMismatch,
    /// Censorship evidence receipts are signed by non-validators.
    ReceiptSignerOutOfTopology,
    /// Censorship evidence receipt signature verification failed.
    ReceiptSignatureInvalid,
    /// Censorship evidence does not meet the f + 1 receipt threshold.
    ReceiptQuorumMissing,
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
    /// A candidate's exact v2 proof is bound to another chain.
    V2AdmissionWrongChain,
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
            UnsupportedModeTag => "evidence uses an unsupported consensus mode tag",
            UnverifiableInvalidQc => "invalid-QC evidence lacks a typed self-verifying proof",
            UnverifiableInvalidProposal => {
                "invalid-proposal evidence lacks a typed self-verifying proof"
            }
            KindPayloadMismatch => "evidence kind does not match payload variant",
            PhaseMismatch => "double-vote evidence phases must match",
            HeightMismatch => "double-vote evidence heights must match",
            ViewMismatch => "double-vote evidence views must match",
            EpochMismatch => "double-vote evidence epochs must match",
            SignerMismatch => "double-vote evidence signers must match",
            BlockHashMatch => "double-vote evidence must reference distinct block hashes",
            PhaseKindMismatch => "double-vote evidence phase disagrees with its kind",
            SignatureMissing => "consensus vote BLS signature payload missing",
            SignatureTruncated => "consensus vote BLS signature payload truncated or forged",
            SignatureInvalid => "consensus vote BLS signature verification failed",
            ReceiptMissing => "censorship evidence must include receipts",
            ReceiptTxHashMismatch => "censorship evidence receipts must match the tx hash",
            ReceiptSignerOutOfTopology => "censorship evidence signer not in topology",
            ReceiptSignatureInvalid => "censorship evidence receipt signature invalid",
            ReceiptQuorumMissing => "censorship evidence below f + 1 receipt threshold",
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
            V2AdmissionWrongChain => "Sumeragi v2 evidence admission belongs to another chain",
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

/// Ensure that [`Evidence`] metadata and attached signatures remain consistent.
///
/// This routine enforces invariants that malicious peers could violate by crafting
/// forged payloads (e.g., mismatching the [`EvidenceKind`] with its payload variant,
/// mixing votes from different heights/views/epochs, or attaching invalid signatures).
/// Downstream consumers expect those invariants to hold when persisting slashing material.
/// Protocol v1 accepts only objectively self-verifying double-vote and censorship
/// claims. Free-form invalid-QC and invalid-proposal assertions fail closed until
/// their wire variants carry typed proofs that independently establish invalidity.
///
/// # Errors
///
/// Returns [`EvidenceValidationError`] when the provided evidence violates one of the
/// invariants (kind/payload mismatch, inconsistent heights/views, or invalid signatures).
pub fn validate_evidence(
    evidence: &Evidence,
    context: &EvidenceValidationContext<'_>,
) -> Result<(), EvidenceValidationError> {
    if !matches!(context.mode_tag, PERMISSIONED_TAG | NPOS_TAG) {
        return Err(EvidenceValidationError::UnsupportedModeTag);
    }
    match (&evidence.kind, &evidence.payload) {
        (
            EvidenceKind::DoublePrepare | EvidenceKind::DoubleCommit,
            EvidencePayload::DoubleVote { v1, v2 },
        ) => validate_double_vote(evidence.kind, v1, v2, context),
        (EvidenceKind::InvalidQc, EvidencePayload::InvalidQc { .. }) => {
            Err(EvidenceValidationError::UnverifiableInvalidQc)
        }
        (EvidenceKind::InvalidProposal, EvidencePayload::InvalidProposal { .. }) => {
            Err(EvidenceValidationError::UnverifiableInvalidProposal)
        }
        (EvidenceKind::Censorship, EvidencePayload::Censorship { tx_hash, receipts }) => {
            validate_censorship(tx_hash, receipts, context)
        }
        (
            EvidenceKind::SumeragiV2Equivocation,
            EvidencePayload::SumeragiV2Equivocation(evidence),
        ) => {
            if &evidence.context.chain_id != context.chain_id {
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
        _ => Err(EvidenceValidationError::KindPayloadMismatch),
    }
}

fn validate_vote_signatures(
    v1: &Vote,
    v2: &Vote,
    context: &EvidenceValidationContext<'_>,
) -> Result<(), EvidenceValidationError> {
    let signature_topology_v1 = archival_topology_for_view(
        context.topology,
        v1.height,
        v1.view,
        context.mode_tag,
        context.prf_seed,
    );
    let signature_topology_v2 = archival_topology_for_view(
        context.topology,
        v2.height,
        v2.view,
        context.mode_tag,
        context.prf_seed,
    );
    archival_vote_signature_check(
        v1,
        &signature_topology_v1,
        context.chain_id,
        context.mode_tag,
    )
    .map_err(|_| EvidenceValidationError::SignatureInvalid)?;
    archival_vote_signature_check(
        v2,
        &signature_topology_v2,
        context.chain_id,
        context.mode_tag,
    )
    .map_err(|_| EvidenceValidationError::SignatureInvalid)?;
    Ok(())
}

fn validate_double_vote(
    kind: EvidenceKind,
    v1: &Vote,
    v2: &Vote,
    context: &EvidenceValidationContext<'_>,
) -> Result<(), EvidenceValidationError> {
    if v1.bls_sig.is_empty() || v2.bls_sig.is_empty() {
        return Err(EvidenceValidationError::SignatureMissing);
    }
    if v1.bls_sig.len() < MIN_BLS_SIGNATURE_LEN || v2.bls_sig.len() < MIN_BLS_SIGNATURE_LEN {
        return Err(EvidenceValidationError::SignatureTruncated);
    }
    let Some(expected_kind) = double_vote_kind_for_phases(v1.phase, v2.phase) else {
        return Err(EvidenceValidationError::PhaseMismatch);
    };
    if v1.height != v2.height {
        return Err(EvidenceValidationError::HeightMismatch);
    }
    if v1.view != v2.view {
        return Err(EvidenceValidationError::ViewMismatch);
    }
    if v1.epoch != v2.epoch {
        return Err(EvidenceValidationError::EpochMismatch);
    }
    if signer_peer_for_vote(v1, context)? != signer_peer_for_vote(v2, context)? {
        return Err(EvidenceValidationError::SignerMismatch);
    }
    let block_hash_conflict = v1.block_hash != v2.block_hash;
    let root_conflict = v1.phase == Phase::Commit
        && v2.phase == Phase::Commit
        && (v1.parent_state_root != v2.parent_state_root
            || v1.post_state_root != v2.post_state_root);
    if !block_hash_conflict && !root_conflict {
        return Err(EvidenceValidationError::BlockHashMatch);
    }

    match (kind, expected_kind) {
        (EvidenceKind::DoublePrepare, EvidenceKind::DoublePrepare)
        | (EvidenceKind::DoubleCommit, EvidenceKind::DoubleCommit) => {
            validate_vote_signatures(v1, v2, context)?;
            Ok(())
        }
        (EvidenceKind::DoublePrepare, EvidenceKind::DoubleCommit)
        | (EvidenceKind::DoubleCommit, EvidenceKind::DoublePrepare) => {
            Err(EvidenceValidationError::PhaseKindMismatch)
        }
        _ => Err(EvidenceValidationError::KindPayloadMismatch),
    }
}

fn validate_censorship(
    tx_hash: &iroha_crypto::HashOf<iroha_data_model::transaction::SignedTransaction>,
    receipts: &[iroha_data_model::transaction::TransactionSubmissionReceipt],
    context: &EvidenceValidationContext<'_>,
) -> Result<(), EvidenceValidationError> {
    if receipts.is_empty() {
        return Err(EvidenceValidationError::ReceiptMissing);
    }
    let required = context.topology.min_votes_for_view_change();
    let mut unique = BTreeSet::new();
    for receipt in receipts {
        if &receipt.payload.tx_hash != tx_hash {
            return Err(EvidenceValidationError::ReceiptTxHashMismatch);
        }
        if context.topology.position(&receipt.payload.signer).is_none() {
            return Err(EvidenceValidationError::ReceiptSignerOutOfTopology);
        }
        receipt
            .verify()
            .map_err(|_| EvidenceValidationError::ReceiptSignatureInvalid)?;
        unique.insert(receipt.payload.signer.clone());
    }
    if unique.len() < required {
        return Err(EvidenceValidationError::ReceiptQuorumMissing);
    }
    Ok(())
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
            .filter(|index| *index < context.roster.len())
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
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{
        block::BlockHeader,
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        parameter::{Parameter, Parameters, system::SumeragiNposParameters},
        peer::PeerId,
        prelude::ChainId,
        transaction::{
            SignedTransaction, TransactionSubmissionReceipt, TransactionSubmissionReceiptPayload,
        },
    };
    use mv::cell::Cell;
    use norito::codec::{Decode, Encode as _};
    use rand::{Rng, SeedableRng, rngs::StdRng, seq::SliceRandom};

    use super::{
        super::consensus::{
            ConsensusBlockHeader, Phase, Proposal, Qc, QcAggregate, QcHeaderRef, Vote,
        },
        *,
    };
    use crate::state::{State, World};

    type EvidenceCase = (EvidenceKind, EvidencePayload, EvidenceValidationError);
    type EvidenceRoundtripCase = (
        &'static str,
        EvidenceValidationError,
        fn(&EvidenceTestContext) -> Evidence,
    );

    fn checked_bls_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("Sumeragi evidence fixture BLS key generation should succeed")
    }

    struct EvidenceTestContext {
        chain_id: ChainId,
        mode_tag: &'static str,
        prf_seed: [u8; 32],
        keypairs: Vec<KeyPair>,
        topology: super::super::network_topology::Topology,
    }

    impl EvidenceTestContext {
        fn new(peer_count: usize) -> Self {
            let keypairs: Vec<_> = (0..peer_count).map(|_| checked_bls_keypair()).collect();
            let peers = keypairs
                .iter()
                .map(|kp| PeerId::new(kp.public_key().clone()));
            let topology = super::super::network_topology::Topology::new(peers);
            Self {
                chain_id: ChainId::from("test"),
                mode_tag: super::super::consensus::PERMISSIONED_TAG,
                prf_seed: [0x11; 32],
                keypairs,
                topology,
            }
        }

        fn validation_context(&self) -> EvidenceValidationContext<'_> {
            EvidenceValidationContext {
                topology: &self.topology,
                chain_id: &self.chain_id,
                mode_tag: self.mode_tag,
                prf_seed: Some(self.prf_seed),
            }
        }

        fn signer_keypair_for_view(&self, signer: u32, height: u64, view: u64) -> &KeyPair {
            let idx = usize::try_from(signer).expect("signer index fits usize");
            let rotated = super::archival_topology_for_view(
                &self.topology,
                height,
                view,
                self.mode_tag,
                Some(self.prf_seed),
            );
            let peer = rotated
                .as_ref()
                .get(idx)
                .expect("signer index must be in range for view-aligned topology");
            self.keypairs
                .iter()
                .find(|kp| kp.public_key() == peer.public_key())
                .expect("signer keypair must exist for view-aligned topology")
        }

        fn signer_index_for_keypair_at_view(
            &self,
            keypair: &KeyPair,
            height: u64,
            view: u64,
        ) -> u32 {
            let rotated = super::archival_topology_for_view(
                &self.topology,
                height,
                view,
                self.mode_tag,
                Some(self.prf_seed),
            );
            let index = rotated
                .as_ref()
                .iter()
                .position(|peer| peer.public_key() == keypair.public_key())
                .expect("keypair must be present in view-aligned topology");
            u32::try_from(index).expect("signer index fits u32")
        }

        fn sign_vote(&self, vote: &mut Vote) {
            let keypair = self.signer_keypair_for_view(vote.signer, vote.height, vote.view);
            let preimage =
                super::super::consensus::vote_preimage(&self.chain_id, self.mode_tag, vote);
            let signature = Signature::try_new(keypair.private_key(), &preimage)
                .expect("test fixture signing should succeed");
            vote.bls_sig = signature.payload().to_vec();
        }
    }

    fn test_context() -> EvidenceTestContext {
        EvidenceTestContext::new(12)
    }

    fn zero_state_root() -> Hash {
        Hash::prehashed([0u8; 32])
    }

    fn sample_validator_set() -> Vec<PeerId> {
        let keypair = KeyPair::try_from_seed(b"evidence-validator".to_vec(), Algorithm::BlsNormal)
            .expect("fixture seed must derive a valid BLS keypair");
        vec![PeerId::new(keypair.public_key().clone())]
    }

    #[test]
    fn sample_validator_set_uses_checked_seed_derivation() {
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::BlsNormal).is_err(),
            "checked BLS seed derivation must reject weak all-zero fixture seeds"
        );
        assert_eq!(sample_validator_set().len(), 1);
    }

    fn test_state() -> State {
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        State::new_for_testing(World::default(), kura, query)
    }

    fn test_state_for_chain(chain_id: ChainId) -> State {
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        State::new_with_chain_for_testing(World::default(), kura, query, chain_id)
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
        State::new_with_chain_for_testing(world, kura, query, fixture.context.chain_id.clone())
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
                chain_id: ChainId::from("sumeragi-v2-evidence-test"),
                protocol_version: wire_v2::PROTOCOL_VERSION,
                height: 1,
                epoch: 7,
                epoch_end_height: u64::MAX,
                next_epoch_snapshot: None,
                snapshot_bootstrap: None,
                mode: wire_v2::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                quorum: wire_v2::DualQuorum::from_roster(&roster).expect("dual quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"v2-evidence-context"),
                execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
                da_layout: wire_v2::DataAvailabilityLayout {
                    encoding: wire_v2::PayloadEncoding::Plain,
                    chunk_size_bytes: 32,
                    data_shards: 0,
                    parity_shards: 0,
                    max_payload_size_bytes: 1024,
                    max_chunk_count: 32,
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
            wire_v2::ExecutionCommitment::without_topups(
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
            let manifest = wire_v2::PayloadManifest::derive(
                &self.context,
                round,
                subject,
                1,
                &[vec![subject.payload_hash.as_ref()[0]]],
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
        let execution_commitment = wire_v2::ExecutionCommitment::without_topups(
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
        let nexus = state.nexus_snapshot();
        let effects = iroha_data_model::consensus::NposConsensusEffects {
            vrf_epoch_seals: Vec::new(),
            v2_evidence_admissions: admissions,
            penalty_actions: Vec::new(),
        };
        let mut transaction = state_block.transaction();
        super::super::penalties::apply_npos_consensus_effects_to_transaction(
            &mut transaction,
            &effects,
            &nexus.dataspace_catalog,
            &nexus.staking,
            height,
            view,
            now_ms,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
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
    fn sumeragi_v2_equivocation_generic_ingress_anchors_chain_and_roster() {
        let fixture = V2EvidenceFixture::new();
        let peers = fixture
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let topology = super::super::network_topology::Topology::new(peers.clone());
        let evidence = Evidence {
            kind: EvidenceKind::SumeragiV2Equivocation,
            payload: EvidencePayload::SumeragiV2Equivocation(fixture.payload(
                wire_v2::SumeragiV2Equivocation::PhaseVote {
                    first: fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x68)),
                    second: fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x69)),
                },
            )),
        };
        let context = EvidenceValidationContext {
            topology: &topology,
            chain_id: &fixture.context.chain_id,
            mode_tag: super::super::consensus::PERMISSIONED_TAG,
            prf_seed: None,
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

        let foreign_chain = ChainId::from("foreign-v2-evidence-chain");
        let wrong_chain = EvidenceValidationContext {
            chain_id: &foreign_chain,
            ..context
        };
        assert_eq!(
            validate_evidence(&evidence, &wrong_chain),
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
        assert_eq!(record.evidence.kind, EvidenceKind::SumeragiV2Equivocation);
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

        let foreign = test_state_for_chain(ChainId::from("foreign-v2-admission-chain"));
        assert_eq!(
            validate_v2_evidence_admissions(&foreign, 2, &[evidence.clone()]),
            Err(EvidenceValidationError::V2AdmissionWrongChain)
        );
        let missing_context = test_state_for_chain(fixture.context.chain_id.clone());
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
        install_v2_finality_for_fixture(&proposer, &fixture);
        install_v2_finality_for_fixture(&follower, &fixture);
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

    fn state_with_horizon(current_height: u64, horizon: u64) -> State {
        let mut params = Parameters::default();
        let npos = SumeragiNposParameters {
            evidence_horizon_blocks: horizon,
            ..SumeragiNposParameters::default()
        };
        params.set_parameter(Parameter::Custom(npos.into_custom_parameter()));
        let mut world = World::default();
        world.parameters = Cell::new(params);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        if current_height > 0 {
            let mut hashes = state.block_hashes.block();
            let len = usize::try_from(current_height)
                .expect("current height must fit into usize for resize");
            let fill_hash =
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xFF; 32]));
            for _ in 0..len {
                hashes.push(fill_hash);
            }
            hashes.commit();
        }
        state
    }

    #[test]
    fn evidence_horizon_formal_gate_configured_matrix() {
        let cases = [
            ("unconfigured horizon", 10, None, Some(0), true),
            ("zero horizon stale", 10, Some(0), Some(0), true),
            ("zero horizon missing subject", 10, Some(0), None, true),
            (
                "missing subject defaults to current",
                10,
                Some(3),
                None,
                true,
            ),
            ("exact lower bound", 10, Some(3), Some(7), true),
            ("below lower bound", 10, Some(3), Some(6), false),
            ("above lower bound", 10, Some(3), Some(8), true),
            ("saturating lower bound", 5, Some(10), Some(0), true),
            ("current zero subject zero", 0, Some(10), Some(0), true),
            ("future subject", 10, Some(3), Some(12), true),
            ("stale when horizon one", 10, Some(1), Some(8), false),
        ];

        for (case, current_height, horizon, subject_height, expected) in cases {
            assert_eq!(
                evidence_within_configured_horizon(current_height, horizon, subject_height),
                expected,
                "{case}"
            );
        }
    }

    fn sample_double_vote_pair(ctx: &EvidenceTestContext) -> (Vote, Vote) {
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x80; 32]));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Prepare,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 11,
            view: 5,
            epoch: 3,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 2,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x81; 32]));
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        (v1, v2)
    }

    #[test]
    fn evidence_rejects_retired_or_unknown_signature_domains() {
        let mut ctx = test_context();
        ctx.mode_tag = "sumeragi-legacy-permissioned";
        let (v1, v2) = sample_double_vote_pair(&ctx);
        let evidence = check_double_vote(&v1, &v2).expect("conflicting signed votes");

        assert_invalid_evidence_rejected(
            &ctx.validation_context(),
            &evidence,
            EvidenceValidationError::UnsupportedModeTag,
        );
    }

    fn sample_tx_hash(tag: u8) -> HashOf<SignedTransaction> {
        HashOf::from_untyped_unchecked(Hash::prehashed([tag; Hash::LENGTH]))
    }

    fn submission_receipt_for(
        ctx: &EvidenceTestContext,
        signer_idx: usize,
        tx_hash: HashOf<SignedTransaction>,
        submitted_at_height: u64,
    ) -> TransactionSubmissionReceipt {
        let keypair = ctx
            .keypairs
            .get(signer_idx)
            .expect("signer index must be in range for test context");
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash,
            entrypoint_hash: HashOf::from_untyped_unchecked(Hash::from(tx_hash)),
            signed_transaction_hash: Some(tx_hash),
            submitted_at_ms: 1,
            submitted_at_height,
            signer: keypair.public_key().clone(),
        };
        TransactionSubmissionReceipt::sign(payload, keypair)
    }

    fn submission_receipt_with_invalid_signature(
        ctx: &EvidenceTestContext,
        signer_idx: usize,
        tx_hash: HashOf<SignedTransaction>,
        submitted_at_height: u64,
    ) -> TransactionSubmissionReceipt {
        let signer_key = ctx
            .keypairs
            .get(signer_idx)
            .expect("signer index must be in range for test context");
        let other_idx = if signer_idx + 1 < ctx.keypairs.len() {
            signer_idx + 1
        } else {
            0
        };
        let signing_key = ctx
            .keypairs
            .get(other_idx)
            .expect("backup signer key exists");
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash,
            entrypoint_hash: HashOf::from_untyped_unchecked(Hash::from(tx_hash)),
            signed_transaction_hash: Some(tx_hash),
            submitted_at_ms: 1,
            submitted_at_height,
            signer: signer_key.public_key().clone(),
        };
        TransactionSubmissionReceipt::sign(payload, signing_key)
    }

    fn sample_invalid_qc_evidence(
        ctx: &EvidenceTestContext,
        tag: u8,
        height: u64,
        view: u64,
    ) -> Evidence {
        let validator_set = ctx.topology.as_ref().to_vec();
        let certificate = Qc {
            phase: Phase::Prepare,
            subject_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [tag; Hash::LENGTH],
            )),
            parent_state_root: zero_state_root(),
            post_state_root: zero_state_root(),
            height,
            view,
            epoch: 2,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: ctx.mode_tag.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: vec![0x01],
                bls_aggregate_signature: vec![tag; 96],
            },
        };
        Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::InvalidQc {
                certificate,
                reason: format!("invalid qc {tag}"),
            },
        }
    }

    fn sample_invalid_proposal_evidence(tag: u8, height: u64, view: u64) -> Evidence {
        let parent_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([tag; Hash::LENGTH]));
        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash,
                tx_root: Hash::prehashed([tag.wrapping_add(1); Hash::LENGTH]),
                state_root: Hash::prehashed([tag.wrapping_add(2); Hash::LENGTH]),
                proposer: 7,
                height,
                view,
                epoch: 3,
                highest_qc: QcHeaderRef {
                    height: height.saturating_sub(1),
                    view: view.saturating_sub(1),
                    epoch: 3,
                    subject_block_hash: parent_hash,
                    phase: Phase::Commit,
                },
            },
            payload_hash: Hash::prehashed([tag.wrapping_add(3); Hash::LENGTH]),
        };
        Evidence {
            kind: EvidenceKind::InvalidProposal,
            payload: EvidencePayload::InvalidProposal {
                proposal,
                reason: format!("invalid proposal {tag}"),
            },
        }
    }

    fn sample_censorship_evidence(
        ctx: &EvidenceTestContext,
        tag: u8,
        submitted_heights: &[u64],
    ) -> Evidence {
        let tx_hash = sample_tx_hash(tag);
        let receipts = submitted_heights
            .iter()
            .enumerate()
            .map(|(idx, height)| submission_receipt_for(ctx, idx, tx_hash, *height))
            .collect();
        Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship { tx_hash, receipts },
        }
    }

    fn double_vote_with(
        ctx: &EvidenceTestContext,
        mutate: impl FnOnce(&mut Vote, &mut Vote),
    ) -> Evidence {
        let (mut v1, mut v2) = sample_double_vote_pair(ctx);
        mutate(&mut v1, &mut v2);
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        }
    }

    fn double_vote_with_unchecked(
        ctx: &EvidenceTestContext,
        mutate: impl FnOnce(&mut Vote, &mut Vote),
    ) -> Evidence {
        let (mut v1, mut v2) = sample_double_vote_pair(ctx);
        mutate(&mut v1, &mut v2);
        Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        }
    }

    fn roundtrip_case_duplicate_signer(ctx: &EvidenceTestContext) -> Evidence {
        double_vote_with(ctx, |_, v2| v2.signer = v2.signer.saturating_add(1))
    }

    fn roundtrip_case_conflicting_height(ctx: &EvidenceTestContext) -> Evidence {
        double_vote_with(ctx, |_, v2| v2.height = v2.height.saturating_add(1))
    }

    fn roundtrip_case_conflicting_view(ctx: &EvidenceTestContext) -> Evidence {
        double_vote_with(ctx, |_, v2| v2.view = v2.view.saturating_add(1))
    }

    fn roundtrip_case_signature_truncated(ctx: &EvidenceTestContext) -> Evidence {
        double_vote_with_unchecked(ctx, |v1, v2| {
            v1.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
            v2.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
        })
    }

    fn roundtrip_case_mixed_manifest_payload(ctx: &EvidenceTestContext) -> Evidence {
        let (v1, v2) = sample_double_vote_pair(ctx);
        Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        }
    }

    #[allow(clippy::too_many_lines)]
    fn mismatched_payload_cases(ctx: &EvidenceTestContext) -> Vec<EvidenceCase> {
        let (v1, v2) = sample_double_vote_pair(ctx);
        let double_vote_payload = EvidencePayload::DoubleVote { v1, v2 };

        let validator_set = ctx.topology.as_ref().to_vec();
        let zero_root = zero_state_root();
        let qc = Qc {
            phase: Phase::Prepare,
            subject_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [0xC0; 32],
            )),
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 21,
            view: 5,
            epoch: 2,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: ctx.mode_tag.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: vec![0x01],
                bls_aggregate_signature: vec![0xC1; 96],
            },
        };
        let invalid_qc_payload = EvidencePayload::InvalidQc {
            certificate: qc,
            reason: "forged QC payload variant".to_owned(),
        };

        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0xC2; 32],
                )),
                tx_root: Hash::prehashed([0xC3; 32]),
                state_root: Hash::prehashed([0xC4; 32]),
                proposer: 6,
                height: 44,
                view: 9,
                epoch: 3,
                highest_qc: QcHeaderRef {
                    height: 43,
                    view: 8,
                    epoch: 3,
                    subject_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                        Hash::prehashed([0xC5; 32]),
                    ),
                    phase: Phase::Commit,
                },
            },
            payload_hash: Hash::prehashed([0xC6; 32]),
        };
        let invalid_proposal_payload = EvidencePayload::InvalidProposal {
            proposal,
            reason: "forged proposal payload variant".to_owned(),
        };

        let censorship_payload = EvidencePayload::Censorship {
            tx_hash: sample_tx_hash(0xCC),
            receipts: Vec::new(),
        };

        let expected = EvidenceValidationError::KindPayloadMismatch;

        vec![
            (
                EvidenceKind::InvalidQc,
                double_vote_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::InvalidProposal,
                double_vote_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::DoublePrepare,
                invalid_qc_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::DoubleCommit,
                invalid_qc_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::DoublePrepare,
                invalid_proposal_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::DoubleCommit,
                invalid_proposal_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::InvalidQc,
                invalid_proposal_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::InvalidProposal,
                invalid_qc_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::Censorship,
                invalid_qc_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::InvalidQc,
                censorship_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::InvalidQc,
                invalid_proposal_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::InvalidProposal,
                invalid_qc_payload.clone(),
                expected,
            ),
        ]
    }

    fn assert_invalid_evidence_rejected(
        context: &EvidenceValidationContext<'_>,
        evidence: &Evidence,
        expected_error: EvidenceValidationError,
    ) {
        let encoded = evidence.encode();
        let mut slice = encoded.as_slice();
        let decoded = Evidence::decode(&mut slice).expect("invalid evidence payload must decode");
        assert_eq!(
            decoded, *evidence,
            "encoded/decoded evidence should roundtrip without mutation"
        );
        let evidence = decoded;
        let key = evidence_key(&evidence);

        assert_eq!(validate_evidence(&evidence, context), Err(expected_error));

        let mut store = EvidenceStore::new();
        assert!(
            !store.insert(&evidence, context),
            "EvidenceStore must reject {expected_error:?}"
        );
        assert!(store.entries.is_empty());

        let state = test_state();
        assert!(
            !persist_record(&state, &evidence, context),
            "persist_record must reject {expected_error:?}"
        );
        let view = state.world.consensus_evidence.view();
        assert_eq!(view.iter().count(), 0);
        assert!(
            view.get(&key).is_none(),
            "rejected evidence must not expose a staking lookup key"
        );
    }

    fn assert_validation_case(
        context: &EvidenceValidationContext<'_>,
        case: &str,
        evidence: Evidence,
        expected: Result<(), EvidenceValidationError>,
    ) {
        assert_eq!(validate_evidence(&evidence, context), expected, "{case}");
    }

    fn rotated_peer_at(ctx: &EvidenceTestContext, height: u64, view: u64, signer: u32) -> PeerId {
        let rotated = super::archival_topology_for_view(
            &ctx.topology,
            height,
            view,
            ctx.mode_tag,
            Some(ctx.prf_seed),
        );
        rotated
            .as_ref()
            .get(usize::try_from(signer).expect("signer index fits usize"))
            .expect("signer index must be present in rotated topology")
            .clone()
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn evidence_canonicalization_formal_gate_key_order_and_payload_matrix() {
        let ctx = test_context();
        let (prepare_left, prepare_right) = sample_double_vote_pair(&ctx);
        let prepare_ordered = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: prepare_left.clone(),
                v2: prepare_right.clone(),
            },
        };
        let prepare_swapped = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: prepare_right,
                v2: prepare_left,
            },
        };
        assert_eq!(
            evidence_key(&prepare_ordered),
            evidence_key(&prepare_swapped)
        );

        let (mut cross_prepare, mut cross_commit) = sample_double_vote_pair(&ctx);
        cross_prepare.phase = Phase::Prepare;
        cross_commit.phase = Phase::Commit;
        ctx.sign_vote(&mut cross_prepare);
        ctx.sign_vote(&mut cross_commit);
        let cross_phase = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote {
                v1: cross_prepare.clone(),
                v2: cross_commit.clone(),
            },
        };
        let cross_phase_swapped = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote {
                v1: cross_commit,
                v2: cross_prepare,
            },
        };
        assert_eq!(
            evidence_key(&cross_phase),
            evidence_key(&cross_phase_swapped)
        );

        let (mut root_left, mut root_right) = sample_double_vote_pair(&ctx);
        root_left.phase = Phase::Commit;
        root_right.phase = Phase::Commit;
        root_right.block_hash = root_left.block_hash;
        root_left.parent_state_root = Hash::prehashed([0x41; Hash::LENGTH]);
        root_left.post_state_root = Hash::prehashed([0x42; Hash::LENGTH]);
        root_right.parent_state_root = Hash::prehashed([0x43; Hash::LENGTH]);
        root_right.post_state_root = Hash::prehashed([0x44; Hash::LENGTH]);
        ctx.sign_vote(&mut root_left);
        ctx.sign_vote(&mut root_right);
        let root_conflict = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote {
                v1: root_left.clone(),
                v2: root_right.clone(),
            },
        };
        let root_conflict_swapped = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote {
                v1: root_right,
                v2: root_left,
            },
        };
        assert_eq!(
            evidence_key(&root_conflict),
            evidence_key(&root_conflict_swapped)
        );

        let censorship = sample_censorship_evidence(&ctx, 0xE0, &[6, 12, 9, 15]);
        let EvidencePayload::Censorship { tx_hash, receipts } = &censorship.payload else {
            panic!("sample_censorship_evidence must produce censorship payload");
        };
        let mut reversed_receipts = receipts.clone();
        reversed_receipts.reverse();
        let censorship_swapped = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash: *tx_hash,
                receipts: reversed_receipts,
            },
        };
        assert_eq!(evidence_key(&censorship), evidence_key(&censorship_swapped));

        let mut duplicate_receipts = receipts.clone();
        duplicate_receipts.push(
            receipts
                .first()
                .expect("censorship sample must contain receipts")
                .clone(),
        );
        let censorship_with_duplicate = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash: *tx_hash,
                receipts: duplicate_receipts,
            },
        };
        assert_ne!(
            evidence_key(&censorship),
            evidence_key(&censorship_with_duplicate),
            "canonicalization must sort receipts without collapsing duplicate payloads"
        );

        let wrong_kind = Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: censorship.payload.clone(),
        };
        assert_ne!(
            evidence_key(&censorship),
            evidence_key(&wrong_kind),
            "deduplication keys must bind the evidence kind"
        );

        let invalid_qc = sample_invalid_qc_evidence(&ctx, 0xB0, 21, 3);
        assert_eq!(canonicalize_evidence(&invalid_qc), invalid_qc);

        let invalid_proposal = sample_invalid_proposal_evidence(0xB1, 22, 4);
        assert_eq!(canonicalize_evidence(&invalid_proposal), invalid_proposal);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn evidence_canonicalization_formal_gate_subject_and_block_refs_matrix() {
        let ctx = test_context();
        let (subject_first, mut subject_second) = sample_double_vote_pair(&ctx);
        subject_second.height = subject_first.height + 8;
        subject_second.view = subject_first.view + 2;
        let subject_probe = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: subject_first.clone(),
                v2: subject_second,
            },
        };
        assert_eq!(
            evidence_subject_height_view(&subject_probe),
            (Some(subject_first.height), Some(subject_first.view)),
            "double-vote subject extraction is anchored to the first canonical vote"
        );

        let invalid_qc = sample_invalid_qc_evidence(&ctx, 0xC0, 31, 7);
        assert_eq!(
            evidence_subject_height_view(&invalid_qc),
            (Some(31), Some(7))
        );

        let invalid_proposal = sample_invalid_proposal_evidence(0xC1, 32, 8);
        assert_eq!(
            evidence_subject_height_view(&invalid_proposal),
            (Some(32), Some(8))
        );

        let empty_censorship = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash: sample_tx_hash(0xC2),
                receipts: Vec::new(),
            },
        };
        assert_eq!(
            evidence_subject_height_view(&empty_censorship),
            (None, None)
        );

        let censorship = sample_censorship_evidence(&ctx, 0xC3, &[9, 13, 11, 10]);
        assert_eq!(evidence_subject_height_view(&censorship), (Some(13), None));

        let (ref_left, ref_right) = sample_double_vote_pair(&ctx);
        let double_refs = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: ref_left.clone(),
                v2: ref_right.clone(),
            },
        };
        assert_eq!(
            evidence_block_refs(&double_refs),
            vec![
                (ref_left.height, ref_left.block_hash),
                (ref_right.height, ref_right.block_hash),
            ]
        );

        let (mut root_left, mut root_right) = sample_double_vote_pair(&ctx);
        root_left.phase = Phase::Commit;
        root_right.phase = Phase::Commit;
        root_right.block_hash = root_left.block_hash;
        root_right.parent_state_root = Hash::prehashed([0x52; Hash::LENGTH]);
        root_right.post_state_root = Hash::prehashed([0x53; Hash::LENGTH]);
        let root_conflict = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote {
                v1: root_left.clone(),
                v2: root_right,
            },
        };
        assert_eq!(
            evidence_block_refs(&root_conflict),
            vec![(root_left.height, root_left.block_hash)],
            "same-hash root conflicts should not duplicate block references"
        );

        let EvidencePayload::InvalidQc { certificate, .. } = &invalid_qc.payload else {
            panic!("sample_invalid_qc_evidence must produce invalid QC payload");
        };
        assert_eq!(
            evidence_block_refs(&invalid_qc),
            vec![(certificate.height, certificate.subject_block_hash)]
        );
        assert!(evidence_block_refs(&invalid_proposal).is_empty());
        assert!(evidence_block_refs(&censorship).is_empty());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn evidence_canonicalization_formal_gate_store_and_persist_metadata() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let (first, second) = sample_double_vote_pair(&ctx);
        let swapped = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: second.clone(),
                v2: first.clone(),
            },
        };
        let canonical = canonicalize_evidence(&swapped);

        let mut store = EvidenceStore::new();
        assert!(store.insert(&swapped, &context));
        assert_eq!(store.entries.len(), 1);
        assert_eq!(
            store
                .entries
                .values()
                .next()
                .expect("store should retain inserted evidence"),
            &canonical
        );
        assert!(
            !store.insert(&canonical, &context),
            "canonical duplicate should not insert"
        );
        assert!(
            !store.insert(&swapped, &context),
            "swapped duplicate should not insert"
        );

        let (mut invalid_left, mut invalid_right) = sample_double_vote_pair(&ctx);
        invalid_right.block_hash = invalid_left.block_hash;
        ctx.sign_vote(&mut invalid_left);
        ctx.sign_vote(&mut invalid_right);
        let invalid = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: invalid_left,
                v2: invalid_right,
            },
        };
        assert!(
            !store.insert(&invalid, &context),
            "invalid evidence must be rejected before deduplication"
        );
        assert_eq!(store.entries.len(), 1);

        let (new_left, mut new_right) = sample_double_vote_pair(&ctx);
        new_right.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x95; Hash::LENGTH]));
        ctx.sign_vote(&mut new_right);
        let new_valid = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: new_left,
                v2: new_right,
            },
        };
        assert!(store.insert(&new_valid, &context));
        assert_eq!(store.entries.len(), 2);

        let state = test_state();
        assert!(persist_record(&state, &swapped, &context));
        assert!(
            !persist_record(&state, &canonical, &context),
            "canonical duplicate should not persist"
        );
        assert!(
            !persist_record(&state, &swapped, &context),
            "swapped duplicate should not persist"
        );
        let view = state.world.consensus_evidence.view();
        let stored: Vec<_> = view.iter().map(|(_, record)| record.clone()).collect();
        assert_eq!(stored.len(), 1);
        let record = &stored[0];
        assert_eq!(record.evidence, canonical);
        assert_eq!(record.recorded_at_height, first.height);
        assert_eq!(record.recorded_at_view, first.view);
        assert!(!record.penalty_applied);
        assert!(!record.penalty_cancelled);
        assert_eq!(record.penalty_applied_at_height, None);
        assert_eq!(record.penalty_cancelled_at_height, None);

        let invalid_state = test_state();
        assert!(!persist_record(&invalid_state, &invalid, &context));
        assert_eq!(
            invalid_state.world.consensus_evidence.view().iter().count(),
            0
        );

        let censorship = sample_censorship_evidence(&ctx, 0xD4, &[8, 13, 11, 10]);
        let canonical_censorship = canonicalize_evidence(&censorship);
        let censorship_state = test_state();
        assert!(persist_record(&censorship_state, &censorship, &context));
        let view = censorship_state.world.consensus_evidence.view();
        let stored: Vec<_> = view.iter().map(|(_, record)| record.clone()).collect();
        assert_eq!(stored.len(), 1);
        let record = &stored[0];
        assert_eq!(record.evidence, canonical_censorship);
        assert_eq!(record.recorded_at_height, 13);
        assert_eq!(record.recorded_at_view, 0);
        assert!(!record.penalty_applied);
        assert!(!record.penalty_cancelled);
        assert_eq!(record.penalty_applied_at_height, None);
        assert_eq!(record.penalty_cancelled_at_height, None);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn evidence_validation_formal_gate_kind_and_double_vote_matrix() {
        let ctx = test_context();
        let context = ctx.validation_context();

        assert_validation_case(
            &context,
            "invalid_qc_without_typed_proof",
            sample_invalid_qc_evidence(&ctx, 0x61, 11, 2),
            Err(EvidenceValidationError::UnverifiableInvalidQc),
        );

        let invalid_qc_payload = sample_invalid_qc_evidence(&ctx, 0x62, 12, 3).payload;
        assert_validation_case(
            &context,
            "kind_mismatch_double_kind_invalid_qc_payload",
            Evidence {
                kind: EvidenceKind::DoublePrepare,
                payload: invalid_qc_payload,
            },
            Err(EvidenceValidationError::KindPayloadMismatch),
        );

        let (double_left, double_right) = sample_double_vote_pair(&ctx);
        assert_validation_case(
            &context,
            "kind_mismatch_invalid_qc_kind_double_payload",
            Evidence {
                kind: EvidenceKind::InvalidQc,
                payload: EvidencePayload::DoubleVote {
                    v1: double_left.clone(),
                    v2: double_right.clone(),
                },
            },
            Err(EvidenceValidationError::KindPayloadMismatch),
        );

        let proposal_payload = sample_invalid_proposal_evidence(0x63, 13, 4).payload;
        assert_validation_case(
            &context,
            "kind_mismatch_censorship_kind_proposal_payload",
            Evidence {
                kind: EvidenceKind::Censorship,
                payload: proposal_payload,
            },
            Err(EvidenceValidationError::KindPayloadMismatch),
        );

        assert_validation_case(
            &context,
            "double_prepare_valid",
            Evidence {
                kind: EvidenceKind::DoublePrepare,
                payload: EvidencePayload::DoubleVote {
                    v1: double_left.clone(),
                    v2: double_right.clone(),
                },
            },
            Ok(()),
        );

        let (mut commit_left, mut commit_right) = sample_double_vote_pair(&ctx);
        commit_left.phase = Phase::Commit;
        commit_right.phase = Phase::Commit;
        ctx.sign_vote(&mut commit_left);
        ctx.sign_vote(&mut commit_right);
        assert_validation_case(
            &context,
            "double_commit_block_valid",
            Evidence {
                kind: EvidenceKind::DoubleCommit,
                payload: EvidencePayload::DoubleVote {
                    v1: commit_left.clone(),
                    v2: commit_right.clone(),
                },
            },
            Ok(()),
        );

        let (mut root_left, mut root_right) = sample_double_vote_pair(&ctx);
        root_left.phase = Phase::Commit;
        root_right.phase = Phase::Commit;
        root_right.block_hash = root_left.block_hash;
        root_right.parent_state_root = Hash::prehashed([0x71; Hash::LENGTH]);
        root_right.post_state_root = Hash::prehashed([0x72; Hash::LENGTH]);
        ctx.sign_vote(&mut root_left);
        ctx.sign_vote(&mut root_right);
        assert_validation_case(
            &context,
            "double_commit_root_valid",
            Evidence {
                kind: EvidenceKind::DoubleCommit,
                payload: EvidencePayload::DoubleVote {
                    v1: root_left.clone(),
                    v2: root_right.clone(),
                },
            },
            Ok(()),
        );

        let (mut cross_prepare, mut cross_commit) = sample_double_vote_pair(&ctx);
        cross_prepare.phase = Phase::Prepare;
        cross_commit.phase = Phase::Commit;
        ctx.sign_vote(&mut cross_prepare);
        ctx.sign_vote(&mut cross_commit);
        assert_validation_case(
            &context,
            "double_cross_phase_valid",
            Evidence {
                kind: EvidenceKind::DoubleCommit,
                payload: EvidencePayload::DoubleVote {
                    v1: cross_prepare,
                    v2: cross_commit,
                },
            },
            Ok(()),
        );

        let mut missing_sig = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: double_left.clone(),
                v2: double_right.clone(),
            },
        };
        if let EvidencePayload::DoubleVote { v1, v2 } = &mut missing_sig.payload {
            v1.bls_sig.clear();
            v2.bls_sig.clear();
        }
        assert_validation_case(
            &context,
            "double_missing_signature",
            missing_sig,
            Err(EvidenceValidationError::SignatureMissing),
        );

        let mut truncated_sig = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: double_left.clone(),
                v2: double_right.clone(),
            },
        };
        if let EvidencePayload::DoubleVote { v1, v2 } = &mut truncated_sig.payload {
            v1.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
            v2.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
        }
        assert_validation_case(
            &context,
            "double_truncated_signature",
            truncated_sig,
            Err(EvidenceValidationError::SignatureTruncated),
        );

        let mut all_zero_sig = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: double_left.clone(),
                v2: double_right.clone(),
            },
        };
        if let EvidencePayload::DoubleVote { v1, v2 } = &mut all_zero_sig.payload {
            v1.bls_sig = vec![0_u8; super::MIN_BLS_SIGNATURE_LEN];
            v2.bls_sig = vec![0_u8; super::MIN_BLS_SIGNATURE_LEN];
        }
        assert_validation_case(
            &context,
            "double_all_zero_signature_material",
            all_zero_sig,
            Err(EvidenceValidationError::SignatureInvalid),
        );

        let mut bad_phase_right = double_right.clone();
        bad_phase_right.phase = Phase::NewView;
        assert_validation_case(
            &context,
            "double_bad_phase_pair",
            Evidence {
                kind: EvidenceKind::DoublePrepare,
                payload: EvidencePayload::DoubleVote {
                    v1: double_left.clone(),
                    v2: bad_phase_right,
                },
            },
            Err(EvidenceValidationError::PhaseMismatch),
        );

        let mut height_mismatch_right = double_right.clone();
        height_mismatch_right.height += 1;
        ctx.sign_vote(&mut height_mismatch_right);
        assert_validation_case(
            &context,
            "double_height_mismatch",
            Evidence {
                kind: EvidenceKind::DoublePrepare,
                payload: EvidencePayload::DoubleVote {
                    v1: double_left.clone(),
                    v2: height_mismatch_right,
                },
            },
            Err(EvidenceValidationError::HeightMismatch),
        );

        let mut view_mismatch_right = double_right.clone();
        view_mismatch_right.view += 1;
        ctx.sign_vote(&mut view_mismatch_right);
        assert_validation_case(
            &context,
            "double_view_mismatch",
            Evidence {
                kind: EvidenceKind::DoublePrepare,
                payload: EvidencePayload::DoubleVote {
                    v1: double_left.clone(),
                    v2: view_mismatch_right,
                },
            },
            Err(EvidenceValidationError::ViewMismatch),
        );

        let mut epoch_mismatch_right = double_right.clone();
        epoch_mismatch_right.epoch += 1;
        ctx.sign_vote(&mut epoch_mismatch_right);
        assert_validation_case(
            &context,
            "double_epoch_mismatch",
            Evidence {
                kind: EvidenceKind::DoublePrepare,
                payload: EvidencePayload::DoubleVote {
                    v1: double_left.clone(),
                    v2: epoch_mismatch_right,
                },
            },
            Err(EvidenceValidationError::EpochMismatch),
        );

        let mut signer_mismatch_right = double_right.clone();
        signer_mismatch_right.signer += 1;
        ctx.sign_vote(&mut signer_mismatch_right);
        assert_validation_case(
            &context,
            "double_signer_mismatch",
            Evidence {
                kind: EvidenceKind::DoublePrepare,
                payload: EvidencePayload::DoubleVote {
                    v1: double_left.clone(),
                    v2: signer_mismatch_right,
                },
            },
            Err(EvidenceValidationError::SignerMismatch),
        );

        let mut same_hash_prepare_right = double_right.clone();
        same_hash_prepare_right.block_hash = double_left.block_hash;
        ctx.sign_vote(&mut same_hash_prepare_right);
        assert_validation_case(
            &context,
            "double_same_hash_prepare",
            Evidence {
                kind: EvidenceKind::DoublePrepare,
                payload: EvidencePayload::DoubleVote {
                    v1: double_left.clone(),
                    v2: same_hash_prepare_right,
                },
            },
            Err(EvidenceValidationError::BlockHashMatch),
        );

        let mut same_roots_commit_right = commit_right.clone();
        same_roots_commit_right.block_hash = commit_left.block_hash;
        same_roots_commit_right.parent_state_root = commit_left.parent_state_root;
        same_roots_commit_right.post_state_root = commit_left.post_state_root;
        ctx.sign_vote(&mut same_roots_commit_right);
        assert_validation_case(
            &context,
            "double_same_hash_commit_same_roots",
            Evidence {
                kind: EvidenceKind::DoubleCommit,
                payload: EvidencePayload::DoubleVote {
                    v1: commit_left.clone(),
                    v2: same_roots_commit_right,
                },
            },
            Err(EvidenceValidationError::BlockHashMatch),
        );

        assert_validation_case(
            &context,
            "double_prepare_kind_for_commit",
            Evidence {
                kind: EvidenceKind::DoublePrepare,
                payload: EvidencePayload::DoubleVote {
                    v1: commit_left.clone(),
                    v2: commit_right.clone(),
                },
            },
            Err(EvidenceValidationError::PhaseKindMismatch),
        );
        assert_validation_case(
            &context,
            "double_commit_kind_for_prepare",
            Evidence {
                kind: EvidenceKind::DoubleCommit,
                payload: EvidencePayload::DoubleVote {
                    v1: double_left.clone(),
                    v2: double_right.clone(),
                },
            },
            Err(EvidenceValidationError::PhaseKindMismatch),
        );

        let mut invalid_signature = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: double_left.clone(),
                v2: double_right.clone(),
            },
        };
        if let EvidencePayload::DoubleVote { v1, v2 } = &mut invalid_signature.payload {
            v1.bls_sig[0] ^= 0x5A;
            v2.bls_sig[0] ^= 0xA5;
        }
        assert_validation_case(
            &context,
            "double_signature_invalid",
            invalid_signature,
            Err(EvidenceValidationError::SignatureInvalid),
        );

        let mut missing_precedence = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: double_left.clone(),
                v2: double_right.clone(),
            },
        };
        if let EvidencePayload::DoubleVote { v1, v2 } = &mut missing_precedence.payload {
            v1.bls_sig.clear();
            v2.height += 1;
        }
        assert_validation_case(
            &context,
            "double_missing_signature_precedes_height",
            missing_precedence,
            Err(EvidenceValidationError::SignatureMissing),
        );

        let mut truncated_precedence = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: double_left.clone(),
                v2: double_right.clone(),
            },
        };
        if let EvidencePayload::DoubleVote { v1, v2 } = &mut truncated_precedence.payload {
            v1.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
            v2.phase = Phase::NewView;
        }
        assert_validation_case(
            &context,
            "double_truncated_signature_precedes_phase",
            truncated_precedence,
            Err(EvidenceValidationError::SignatureTruncated),
        );

        let mut phase_precedence_right = double_right.clone();
        phase_precedence_right.phase = Phase::NewView;
        phase_precedence_right.height += 1;
        assert_validation_case(
            &context,
            "double_phase_precedes_height",
            Evidence {
                kind: EvidenceKind::DoublePrepare,
                payload: EvidencePayload::DoubleVote {
                    v1: double_left.clone(),
                    v2: phase_precedence_right,
                },
            },
            Err(EvidenceValidationError::PhaseMismatch),
        );

        let mut height_precedence_right = double_right.clone();
        height_precedence_right.height += 1;
        height_precedence_right.epoch += 1;
        ctx.sign_vote(&mut height_precedence_right);
        assert_validation_case(
            &context,
            "double_height_precedes_epoch",
            Evidence {
                kind: EvidenceKind::DoublePrepare,
                payload: EvidencePayload::DoubleVote {
                    v1: double_left.clone(),
                    v2: height_precedence_right,
                },
            },
            Err(EvidenceValidationError::HeightMismatch),
        );

        let mut epoch_precedence_right = double_right.clone();
        epoch_precedence_right.epoch += 1;
        epoch_precedence_right.signer += 1;
        ctx.sign_vote(&mut epoch_precedence_right);
        assert_validation_case(
            &context,
            "double_epoch_precedes_signer",
            Evidence {
                kind: EvidenceKind::DoublePrepare,
                payload: EvidencePayload::DoubleVote {
                    v1: double_left.clone(),
                    v2: epoch_precedence_right,
                },
            },
            Err(EvidenceValidationError::EpochMismatch),
        );

        let mut signer_precedence_right = double_right;
        signer_precedence_right.signer += 1;
        signer_precedence_right.block_hash = double_left.block_hash;
        ctx.sign_vote(&mut signer_precedence_right);
        assert_validation_case(
            &context,
            "double_signer_precedes_block",
            Evidence {
                kind: EvidenceKind::DoublePrepare,
                payload: EvidencePayload::DoubleVote {
                    v1: double_left,
                    v2: signer_precedence_right,
                },
            },
            Err(EvidenceValidationError::SignerMismatch),
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn unverifiable_proposal_claims_fail_closed_without_typed_proofs() {
        let ctx = test_context();
        let context = ctx.validation_context();

        assert_validation_case(
            &context,
            "structurally_plausible_proposal_claim",
            sample_invalid_proposal_evidence(0x80, 51, 5),
            Err(EvidenceValidationError::UnverifiableInvalidProposal),
        );

        let mut equal_height = sample_invalid_proposal_evidence(0x81, 51, 5);
        if let EvidencePayload::InvalidProposal { proposal, .. } = &mut equal_height.payload {
            proposal.header.height = proposal.header.highest_qc.height;
        }
        assert_validation_case(
            &context,
            "proposal_equal_height",
            equal_height,
            Err(EvidenceValidationError::UnverifiableInvalidProposal),
        );

        let mut lower_height = sample_invalid_proposal_evidence(0x82, 51, 5);
        if let EvidencePayload::InvalidProposal { proposal, .. } = &mut lower_height.payload {
            proposal.header.height = proposal.header.highest_qc.height.saturating_sub(1);
        }
        assert_validation_case(
            &context,
            "proposal_lower_height",
            lower_height,
            Err(EvidenceValidationError::UnverifiableInvalidProposal),
        );

        let mut parent_mismatch = sample_invalid_proposal_evidence(0x83, 51, 5);
        if let EvidencePayload::InvalidProposal { proposal, .. } = &mut parent_mismatch.payload {
            proposal.header.parent_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
                Hash::prehashed([0x84; Hash::LENGTH]),
            );
        }
        assert_validation_case(
            &context,
            "proposal_parent_mismatch",
            parent_mismatch,
            Err(EvidenceValidationError::UnverifiableInvalidProposal),
        );

        let mut height_parent_precedence = sample_invalid_proposal_evidence(0x85, 51, 5);
        if let EvidencePayload::InvalidProposal { proposal, .. } =
            &mut height_parent_precedence.payload
        {
            proposal.header.height = proposal.header.highest_qc.height;
            proposal.header.parent_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
                Hash::prehashed([0x86; Hash::LENGTH]),
            );
        }
        assert_validation_case(
            &context,
            "proposal_height_parent_precedence",
            height_parent_precedence,
            Err(EvidenceValidationError::UnverifiableInvalidProposal),
        );

        let mut view_reset = sample_invalid_proposal_evidence(0x87, 51, 0);
        if let EvidencePayload::InvalidProposal { proposal, .. } = &mut view_reset.payload {
            proposal.header.view = 0;
            proposal.header.highest_qc.view = 12;
        }
        assert_validation_case(
            &context,
            "proposal_view_reset_is_not_an_invalidity_proof",
            view_reset,
            Err(EvidenceValidationError::UnverifiableInvalidProposal),
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn evidence_validation_formal_gate_censorship_matrix() {
        let ctx = EvidenceTestContext::new(4);
        let context = ctx.validation_context();
        let tx_hash = sample_tx_hash(0x91);
        let required = ctx.topology.min_votes_for_view_change();
        let exact_receipts: Vec<_> = (0..required)
            .map(|idx| submission_receipt_for(&ctx, idx, tx_hash, 10))
            .collect();
        assert_validation_case(
            &context,
            "censorship_valid_exact_quorum",
            Evidence {
                kind: EvidenceKind::Censorship,
                payload: EvidencePayload::Censorship {
                    tx_hash,
                    receipts: exact_receipts.clone(),
                },
            },
            Ok(()),
        );

        let mut extra_duplicate = exact_receipts.clone();
        extra_duplicate.push(
            exact_receipts
                .first()
                .expect("exact quorum sample must have a receipt")
                .clone(),
        );
        assert_validation_case(
            &context,
            "censorship_valid_extra_duplicate",
            Evidence {
                kind: EvidenceKind::Censorship,
                payload: EvidencePayload::Censorship {
                    tx_hash,
                    receipts: extra_duplicate,
                },
            },
            Ok(()),
        );

        assert_validation_case(
            &context,
            "censorship_empty",
            Evidence {
                kind: EvidenceKind::Censorship,
                payload: EvidencePayload::Censorship {
                    tx_hash,
                    receipts: Vec::new(),
                },
            },
            Err(EvidenceValidationError::ReceiptMissing),
        );

        let receipt = submission_receipt_for(&ctx, 0, tx_hash, 10);
        assert_validation_case(
            &context,
            "censorship_tx_mismatch",
            Evidence {
                kind: EvidenceKind::Censorship,
                payload: EvidencePayload::Censorship {
                    tx_hash: sample_tx_hash(0x92),
                    receipts: vec![receipt.clone()],
                },
            },
            Err(EvidenceValidationError::ReceiptTxHashMismatch),
        );

        let outsider = checked_bls_keypair();
        let outsider_payload = TransactionSubmissionReceiptPayload {
            tx_hash,
            entrypoint_hash: HashOf::from_untyped_unchecked(Hash::from(tx_hash)),
            signed_transaction_hash: Some(tx_hash),
            submitted_at_ms: 1,
            submitted_at_height: 10,
            signer: outsider.public_key().clone(),
        };
        assert_validation_case(
            &context,
            "censorship_signer_out",
            Evidence {
                kind: EvidenceKind::Censorship,
                payload: EvidencePayload::Censorship {
                    tx_hash,
                    receipts: vec![TransactionSubmissionReceipt::sign(
                        outsider_payload.clone(),
                        &outsider,
                    )],
                },
            },
            Err(EvidenceValidationError::ReceiptSignerOutOfTopology),
        );

        assert_validation_case(
            &context,
            "censorship_bad_signature",
            Evidence {
                kind: EvidenceKind::Censorship,
                payload: EvidencePayload::Censorship {
                    tx_hash,
                    receipts: vec![submission_receipt_with_invalid_signature(
                        &ctx, 0, tx_hash, 10,
                    )],
                },
            },
            Err(EvidenceValidationError::ReceiptSignatureInvalid),
        );

        assert_validation_case(
            &context,
            "censorship_duplicate_below_quorum",
            Evidence {
                kind: EvidenceKind::Censorship,
                payload: EvidencePayload::Censorship {
                    tx_hash,
                    receipts: vec![receipt.clone(), receipt],
                },
            },
            Err(EvidenceValidationError::ReceiptQuorumMissing),
        );

        let larger_ctx = EvidenceTestContext::new(7);
        let larger_context = larger_ctx.validation_context();
        let larger_tx_hash = sample_tx_hash(0x93);
        let two_unique_receipts = vec![
            submission_receipt_for(&larger_ctx, 0, larger_tx_hash, 10),
            submission_receipt_for(&larger_ctx, 1, larger_tx_hash, 10),
        ];
        assert_validation_case(
            &larger_context,
            "censorship_two_unique_below_quorum",
            Evidence {
                kind: EvidenceKind::Censorship,
                payload: EvidencePayload::Censorship {
                    tx_hash: larger_tx_hash,
                    receipts: two_unique_receipts,
                },
            },
            Err(EvidenceValidationError::ReceiptQuorumMissing),
        );

        assert_validation_case(
            &context,
            "censorship_tx_mismatch_precedes_quorum",
            Evidence {
                kind: EvidenceKind::Censorship,
                payload: EvidencePayload::Censorship {
                    tx_hash: sample_tx_hash(0x94),
                    receipts: vec![submission_receipt_for(&ctx, 0, tx_hash, 10)],
                },
            },
            Err(EvidenceValidationError::ReceiptTxHashMismatch),
        );

        let outsider_bad_sig_receipt =
            TransactionSubmissionReceipt::sign(outsider_payload, &ctx.keypairs[0]);
        assert_validation_case(
            &context,
            "censorship_outsider_precedes_signature",
            Evidence {
                kind: EvidenceKind::Censorship,
                payload: EvidencePayload::Censorship {
                    tx_hash,
                    receipts: vec![outsider_bad_sig_receipt],
                },
            },
            Err(EvidenceValidationError::ReceiptSignerOutOfTopology),
        );

        assert_validation_case(
            &context,
            "censorship_signature_precedes_quorum",
            Evidence {
                kind: EvidenceKind::Censorship,
                payload: EvidencePayload::Censorship {
                    tx_hash,
                    receipts: vec![submission_receipt_with_invalid_signature(
                        &ctx, 0, tx_hash, 10,
                    )],
                },
            },
            Err(EvidenceValidationError::ReceiptSignatureInvalid),
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn double_vote_recording_formal_gate_detection_matrix() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let (prepare_left, prepare_right) = sample_double_vote_pair(&ctx);

        let prepare = check_double_vote(&prepare_left, &prepare_right)
            .expect("bare_prepare_conflict must emit");
        assert_eq!(prepare.kind, EvidenceKind::DoublePrepare);
        let swapped_prepare = check_double_vote(&prepare_right, &prepare_left)
            .expect("bare_swapped_prepare_conflict must emit");
        assert_eq!(prepare.kind, swapped_prepare.kind);
        assert_eq!(evidence_key(&prepare), evidence_key(&swapped_prepare));

        let (mut commit_left, mut commit_right) = sample_double_vote_pair(&ctx);
        commit_left.phase = Phase::Commit;
        commit_right.phase = Phase::Commit;
        ctx.sign_vote(&mut commit_left);
        ctx.sign_vote(&mut commit_right);
        let commit_block = check_double_vote(&commit_left, &commit_right)
            .expect("bare_commit_block_conflict must emit");
        assert_eq!(commit_block.kind, EvidenceKind::DoubleCommit);

        let (mut root_left, mut root_right) = sample_double_vote_pair(&ctx);
        root_left.phase = Phase::Commit;
        root_right.phase = Phase::Commit;
        root_right.block_hash = root_left.block_hash;
        root_right.post_state_root = Hash::prehashed([0xD1; Hash::LENGTH]);
        ctx.sign_vote(&mut root_left);
        ctx.sign_vote(&mut root_right);
        let commit_root = check_double_vote(&root_left, &root_right)
            .expect("bare_commit_root_conflict must emit");
        assert_eq!(commit_root.kind, EvidenceKind::DoubleCommit);

        let (mut cross_prepare, mut cross_commit) = sample_double_vote_pair(&ctx);
        cross_prepare.phase = Phase::Prepare;
        cross_commit.phase = Phase::Commit;
        ctx.sign_vote(&mut cross_prepare);
        ctx.sign_vote(&mut cross_commit);
        let cross_phase = check_double_vote(&cross_prepare, &cross_commit)
            .expect("bare_cross_phase_prepare_commit must emit");
        let cross_phase_swapped = check_double_vote(&cross_commit, &cross_prepare)
            .expect("bare_cross_phase_commit_prepare must emit");
        assert_eq!(cross_phase.kind, EvidenceKind::DoubleCommit);
        assert_eq!(cross_phase_swapped.kind, EvidenceKind::DoubleCommit);
        assert_eq!(
            evidence_key(&cross_phase),
            evidence_key(&cross_phase_swapped)
        );

        let mut same_hash_prepare_right = prepare_right.clone();
        same_hash_prepare_right.block_hash = prepare_left.block_hash;
        ctx.sign_vote(&mut same_hash_prepare_right);
        assert!(
            check_double_vote(&prepare_left, &same_hash_prepare_right).is_none(),
            "bare_same_hash_prepare must not emit"
        );

        let mut same_roots_commit_right = commit_right.clone();
        same_roots_commit_right.block_hash = commit_left.block_hash;
        same_roots_commit_right.parent_state_root = commit_left.parent_state_root;
        same_roots_commit_right.post_state_root = commit_left.post_state_root;
        ctx.sign_vote(&mut same_roots_commit_right);
        assert!(
            check_double_vote(&commit_left, &same_roots_commit_right).is_none(),
            "bare_same_hash_commit_same_roots must not emit"
        );

        let mut height_mismatch = prepare_right.clone();
        height_mismatch.height += 1;
        ctx.sign_vote(&mut height_mismatch);
        assert!(check_double_vote(&prepare_left, &height_mismatch).is_none());

        let mut epoch_mismatch = prepare_right.clone();
        epoch_mismatch.epoch += 1;
        ctx.sign_vote(&mut epoch_mismatch);
        assert!(check_double_vote(&prepare_left, &epoch_mismatch).is_none());

        let mut signer_mismatch = prepare_right.clone();
        signer_mismatch.signer += 1;
        ctx.sign_vote(&mut signer_mismatch);
        assert!(check_double_vote(&prepare_left, &signer_mismatch).is_none());

        let mut bad_phase = prepare_right.clone();
        bad_phase.phase = Phase::NewView;
        assert!(check_double_vote(&prepare_left, &bad_phase).is_none());

        let ctx_same = check_double_vote_with_context(&prepare_left, &prepare_right, &context)
            .expect("ctx_same_peer_same_index must emit");
        assert_eq!(ctx_same.kind, EvidenceKind::DoublePrepare);

        let height = prepare_left.height;
        let base_view = 0;
        let target_signer = 2;
        let target_keypair = ctx.signer_keypair_for_view(target_signer, height, base_view);
        let rotated_view = (1..16)
            .find(|view| {
                ctx.signer_index_for_keypair_at_view(target_keypair, height, *view) != target_signer
            })
            .expect("test topology should rotate signer indices across views");
        let rotated_signer =
            ctx.signer_index_for_keypair_at_view(target_keypair, height, rotated_view);
        let mut rotated_left = prepare_left.clone();
        rotated_left.view = base_view;
        rotated_left.signer = target_signer;
        rotated_left.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xD2; Hash::LENGTH]));
        let mut rotated_right = rotated_left.clone();
        rotated_right.view = rotated_view;
        rotated_right.signer = rotated_signer;
        rotated_right.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xD3; Hash::LENGTH]));
        ctx.sign_vote(&mut rotated_left);
        ctx.sign_vote(&mut rotated_right);
        assert!(
            check_double_vote_with_context(&rotated_left, &rotated_right, &context).is_none(),
            "ctx_same_peer_rotated_index must not treat later-view voting as equivocation"
        );

        let mut cross_view_left = prepare_left.clone();
        cross_view_left.view = 0;
        cross_view_left.signer = 1;
        let cross_view_keypair =
            ctx.signer_keypair_for_view(cross_view_left.signer, height, cross_view_left.view);
        let mut cross_view_right = cross_view_left.clone();
        cross_view_right.view = rotated_view;
        cross_view_right.signer =
            ctx.signer_index_for_keypair_at_view(cross_view_keypair, height, rotated_view);
        cross_view_right.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xD4; Hash::LENGTH]));
        ctx.sign_vote(&mut cross_view_left);
        ctx.sign_vote(&mut cross_view_right);
        assert!(
            check_double_vote_with_context(&cross_view_left, &cross_view_right, &context).is_none(),
            "ctx_cross_view_same_peer must not emit"
        );

        let raw_signer = 0;
        let raw_base_peer = rotated_peer_at(&ctx, height, 0, raw_signer);
        let different_peer_view = (1..16)
            .find(|view| rotated_peer_at(&ctx, height, *view, raw_signer) != raw_base_peer)
            .expect("test topology should rotate raw index to a different peer");
        let mut raw_left = prepare_left.clone();
        raw_left.view = 0;
        raw_left.signer = raw_signer;
        let mut raw_right = raw_left.clone();
        raw_right.view = different_peer_view;
        raw_right.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xD5; Hash::LENGTH]));
        ctx.sign_vote(&mut raw_left);
        ctx.sign_vote(&mut raw_right);
        assert!(
            check_double_vote_with_context(&raw_left, &raw_right, &context).is_none(),
            "ctx_same_raw_different_peer must not emit"
        );

        let mut out_of_range_first = prepare_left.clone();
        out_of_range_first.signer =
            u32::try_from(ctx.topology.as_ref().len() + 1).expect("test topology length fits u32");
        assert!(
            check_double_vote_with_context(&out_of_range_first, &prepare_right, &context).is_none(),
            "ctx_out_of_range_first must not emit"
        );

        let mut out_of_range_second = prepare_right.clone();
        out_of_range_second.signer =
            u32::try_from(ctx.topology.as_ref().len() + 1).expect("test topology length fits u32");
        assert!(
            check_double_vote_with_context(&prepare_left, &out_of_range_second, &context).is_none(),
            "ctx_out_of_range_second must not emit"
        );

        assert!(
            check_double_vote_with_context(&prepare_left, &same_hash_prepare_right, &context)
                .is_none(),
            "ctx_nonconflict must not emit"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn double_vote_recording_formal_gate_record_control_flow_matrix() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let (v1, v2) = sample_double_vote_pair(&ctx);

        let mut store = EvidenceStore::new();
        let state = test_state();
        let mut nonconflict = v1.clone();
        nonconflict.block_hash = v1.block_hash;
        ctx.sign_vote(&mut nonconflict);
        assert!(
            !record_double_vote(&mut store, &state, &v1, &nonconflict, &context),
            "record_no_evidence must return false"
        );
        assert_eq!(store.entries.len(), 0);
        assert_eq!(state.world.consensus_evidence.view().iter().count(), 0);

        let mut store = EvidenceStore::new();
        let state = test_state();
        let expected = check_double_vote(&v1, &v2).expect("valid double vote expected");
        let expected_key = evidence_key(&expected);
        assert!(
            record_double_vote(&mut store, &state, &v1, &v2, &context),
            "record_new_valid must return true"
        );
        assert!(store.seen.contains(&expected_key));
        assert_eq!(store.entries.len(), 1);
        assert_eq!(state.world.consensus_evidence.view().iter().count(), 1);
        assert!(
            !record_double_vote(&mut store, &state, &v1, &v2, &context),
            "record_store_duplicate must return false"
        );
        assert_eq!(store.entries.len(), 1);
        assert_eq!(state.world.consensus_evidence.view().iter().count(), 1);

        let mut validation_store = EvidenceStore::new();
        let validation_state = test_state();
        let mut unsigned_left = v1.clone();
        let mut unsigned_right = v2.clone();
        unsigned_left.bls_sig.clear();
        unsigned_right.bls_sig.clear();
        assert!(
            check_double_vote_with_context(&unsigned_left, &unsigned_right, &context).is_some(),
            "store-validation case must detect before store validation rejects"
        );
        assert!(
            !record_double_vote(
                &mut validation_store,
                &validation_state,
                &unsigned_left,
                &unsigned_right,
                &context
            ),
            "record_store_validation_reject must return false"
        );
        assert_eq!(validation_store.entries.len(), 0);
        assert_eq!(
            validation_state
                .world
                .consensus_evidence
                .view()
                .iter()
                .count(),
            0
        );

        let mut initial_store = EvidenceStore::new();
        let duplicate_state = test_state();
        assert!(record_double_vote(
            &mut initial_store,
            &duplicate_state,
            &v1,
            &v2,
            &context
        ));
        let mut fresh_store = EvidenceStore::new();
        assert!(
            !record_double_vote(&mut fresh_store, &duplicate_state, &v1, &v2, &context),
            "record_persist_duplicate_fresh_store must return false"
        );
        assert!(fresh_store.seen.contains(&expected_key));
        assert_eq!(fresh_store.entries.len(), 1);
        assert_eq!(
            duplicate_state
                .world
                .consensus_evidence
                .view()
                .iter()
                .count(),
            1
        );

        let stale_state = state_with_horizon(50, 3);
        let (mut stale_left, mut stale_right) = sample_double_vote_pair(&ctx);
        stale_left.height = 40;
        stale_right.height = 40;
        stale_left.view = 2;
        stale_right.view = 2;
        ctx.sign_vote(&mut stale_left);
        ctx.sign_vote(&mut stale_right);
        let stale_expected = check_double_vote(&stale_left, &stale_right)
            .expect("stale double vote should still detect before horizon filtering");
        let stale_key = evidence_key(&stale_expected);
        let mut stale_store = EvidenceStore::new();
        assert!(
            !record_double_vote(
                &mut stale_store,
                &stale_state,
                &stale_left,
                &stale_right,
                &context
            ),
            "record_persist_horizon_reject must return false"
        );
        assert!(stale_store.seen.contains(&stale_key));
        assert_eq!(stale_store.entries.len(), 1);
        assert_eq!(
            stale_state.world.consensus_evidence.view().iter().count(),
            0
        );

        let mut swapped_store = EvidenceStore::new();
        let swapped_state = test_state();
        assert!(record_double_vote(
            &mut swapped_store,
            &swapped_state,
            &v1,
            &v2,
            &context
        ));
        assert!(
            !record_double_vote(&mut swapped_store, &swapped_state, &v2, &v1, &context),
            "record_swapped_duplicate must return false"
        );
        assert_eq!(swapped_store.entries.len(), 1);
        assert_eq!(
            swapped_state.world.consensus_evidence.view().iter().count(),
            1
        );

        let (mut cross_prepare, mut cross_commit) = sample_double_vote_pair(&ctx);
        cross_prepare.phase = Phase::Prepare;
        cross_commit.phase = Phase::Commit;
        ctx.sign_vote(&mut cross_prepare);
        ctx.sign_vote(&mut cross_commit);
        let mut cross_store = EvidenceStore::new();
        let cross_state = test_state();
        assert!(record_double_vote(
            &mut cross_store,
            &cross_state,
            &cross_prepare,
            &cross_commit,
            &context
        ));
        let cross_record = cross_state
            .world
            .consensus_evidence
            .view()
            .iter()
            .next()
            .expect("cross-phase evidence should persist")
            .1
            .clone();
        assert_eq!(cross_record.evidence.kind, EvidenceKind::DoubleCommit);
        assert!(
            cross_store
                .seen
                .contains(&evidence_key(&cross_record.evidence))
        );

        let (mut root_left, mut root_right) = sample_double_vote_pair(&ctx);
        root_left.phase = Phase::Commit;
        root_right.phase = Phase::Commit;
        root_right.block_hash = root_left.block_hash;
        root_right.post_state_root = Hash::prehashed([0xE1; Hash::LENGTH]);
        ctx.sign_vote(&mut root_left);
        ctx.sign_vote(&mut root_right);
        let mut root_store = EvidenceStore::new();
        let root_state = test_state();
        assert!(record_double_vote(
            &mut root_store,
            &root_state,
            &root_left,
            &root_right,
            &context
        ));
        let root_record = root_state
            .world
            .consensus_evidence
            .view()
            .iter()
            .next()
            .expect("commit-root evidence should persist")
            .1
            .clone();
        assert_eq!(root_record.evidence.kind, EvidenceKind::DoubleCommit);
        assert!(
            root_store
                .seen
                .contains(&evidence_key(&root_record.evidence))
        );
    }

    #[test]
    fn invalid_qc_claims_fail_closed_without_typed_proofs() {
        let ctx = test_context();
        let context = ctx.validation_context();

        let claim = |tag, height, view, bitmap: Vec<u8>| {
            let mut evidence = sample_invalid_qc_evidence(&ctx, tag, height, view);
            let EvidencePayload::InvalidQc { certificate, .. } = &mut evidence.payload else {
                panic!("sample_invalid_qc_evidence must produce invalid QC payload");
            };
            certificate.aggregate.signers_bitmap = bitmap;
            evidence
        };

        for (_case, evidence) in [
            ("empty_bitmap_nonzero", claim(0xF1, 7, 2, Vec::new())),
            ("zero_sentinel_nonempty", claim(0xF2, 0, 0, vec![0x01])),
            ("both_empty_and_zero", claim(0xF3, 0, 0, Vec::new())),
            (
                "empty_bitmap_height_zero_view_nonzero",
                claim(0xF4, 0, 5, Vec::new()),
            ),
            ("height_zero_alone_nonempty", claim(0xF5, 0, 3, vec![0x01])),
            ("view_zero_alone_nonempty", claim(0xF6, 3, 0, vec![0x01])),
            (
                "structurally_plausible_nonempty_nonzero",
                claim(0xF7, 3, 1, vec![0x01]),
            ),
        ] {
            assert_invalid_evidence_rejected(
                &context,
                &evidence,
                EvidenceValidationError::UnverifiableInvalidQc,
            );
        }
    }

    #[test]
    fn detect_double_prevote() {
        let ctx = test_context();
        let h =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([2; 32]));
        let zero_root = iroha_crypto::Hash::prehashed([0u8; 32]);
        let mut v1 = Vote {
            phase: super::super::consensus::Phase::Prepare,
            block_hash: h,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 5,
            view: 7,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 1,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([3; 32]));
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let ev = check_double_vote(&v1, &v2).expect("should detect double vote");
        assert!(matches!(ev.kind, EvidenceKind::DoublePrepare));
    }

    #[test]
    fn detect_double_precommit() {
        let ctx = test_context();
        let (mut v1, mut v2) = sample_double_vote_pair(&ctx);
        v1.phase = Phase::Commit;
        v2.phase = Phase::Commit;
        let parent_root = iroha_crypto::Hash::prehashed([0xA1; 32]);
        let post_root = iroha_crypto::Hash::prehashed([0xA2; 32]);
        v1.parent_state_root = parent_root;
        v1.post_state_root = post_root;
        v2.parent_state_root = parent_root;
        v2.post_state_root = post_root;
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let ev = check_double_vote(&v1, &v2).expect("should detect double vote");
        assert!(matches!(ev.kind, EvidenceKind::DoubleCommit));
    }

    #[test]
    fn double_vote_detects_commit_root_mismatch() {
        let ctx = test_context();
        let subject = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x22; 32],
        ));
        let parent_root = iroha_crypto::Hash::prehashed([0xA1; 32]);
        let post_root = iroha_crypto::Hash::prehashed([0xA2; 32]);
        let other_post_root = iroha_crypto::Hash::prehashed([0xA3; 32]);
        let mut v1 = Vote {
            phase: Phase::Commit,
            block_hash: subject,
            parent_state_root: parent_root,
            post_state_root: post_root,
            height: 9,
            view: 2,
            epoch: 1,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 1,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.post_state_root = other_post_root;
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let ev = check_double_vote(&v1, &v2).expect("commit root mismatch must yield evidence");
        assert!(matches!(ev.kind, EvidenceKind::DoubleCommit));
    }

    #[test]
    fn double_vote_requires_distinct_block_hashes() {
        let ctx = test_context();
        let h = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x10; 32],
        ));
        let zero_root = iroha_crypto::Hash::prehashed([0u8; 32]);
        let mut v1 = Vote {
            phase: super::super::consensus::Phase::Prepare,
            block_hash: h,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 11,
            view: 3,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 1,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v1);
        let v2 = v1.clone();
        assert!(
            check_double_vote(&v1, &v2).is_none(),
            "identical votes should not yield double-vote evidence"
        );
    }

    #[test]
    fn double_vote_requires_matching_height_view_and_epoch() {
        let ctx = test_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x20; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: super::super::consensus::Phase::Commit,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 5,
            view: 2,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 7,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v1);
        let mut v2 = v1.clone();
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x21; 32]),
        );
        v2.height += 1;
        v2.view += 1;
        ctx.sign_vote(&mut v2);
        assert!(
            check_double_vote(&v1, &v2).is_none(),
            "height mismatch must not produce double-vote evidence"
        );

        v2.height = v1.height;
        v2.view = v1.view.saturating_add(1);
        v2.epoch = v1.epoch;
        ctx.sign_vote(&mut v2);
        assert!(
            check_double_vote(&v1, &v2).is_none(),
            "a legitimate later-view vote must not produce double-vote evidence"
        );

        // Restore view but change epoch to confirm epoch mismatch rejects evidence too.
        v2.height = v1.height;
        v2.view = v1.view;
        v2.epoch = 1;
        ctx.sign_vote(&mut v2);
        assert!(
            check_double_vote(&v1, &v2).is_none(),
            "epoch mismatch must not produce double-vote evidence"
        );
    }

    #[test]
    fn double_vote_requires_same_signer() {
        let ctx = test_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x30; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: super::super::consensus::Phase::Prepare,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 8,
            view: 4,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 3,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v1);
        let mut v2 = v1.clone();
        v2.signer = 4;
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x31; 32]),
        );
        ctx.sign_vote(&mut v2);
        assert!(
            check_double_vote(&v1, &v2).is_none(),
            "votes from different signers must not emit double-vote evidence"
        );
    }

    #[test]
    fn double_vote_detects_cross_phase_conflict() {
        let ctx = test_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x33; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: super::super::consensus::Phase::Prepare,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 12,
            view: 6,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 1,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.phase = super::super::consensus::Phase::Commit;
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x34; 32]),
        );
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);

        let evidence =
            check_double_vote(&v1, &v2).expect("cross-phase conflict must emit evidence");
        assert_eq!(evidence.kind, EvidenceKind::DoubleCommit);
    }

    #[test]
    fn double_vote_phase_must_match_kind() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x40; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Commit,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 6,
            view: 9,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 2,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x41; 32]),
        );
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);

        let forged = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: v1.clone(),
                v2: v2.clone(),
            },
        };
        assert_eq!(
            validate_evidence(&forged, &context),
            Err(EvidenceValidationError::PhaseKindMismatch)
        );

        let valid = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert!(validate_evidence(&valid, &context).is_ok());
    }

    #[test]
    fn validate_double_vote_accepts_cross_phase_conflict() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x43; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Prepare,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 14,
            view: 7,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 2,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.phase = Phase::Commit;
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x44; 32]),
        );
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);

        let ev = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert!(validate_evidence(&ev, &context).is_ok());
    }

    #[test]
    fn legitimate_cross_view_votes_cannot_persist_or_reach_staking_key() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x46; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Commit,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 15,
            view: 2,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 3,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.view = v1.view.saturating_add(1);
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x47; 32]),
        );
        let signer_keypair = ctx.signer_keypair_for_view(v1.signer, v1.height, v1.view);
        v2.signer = ctx.signer_index_for_keypair_at_view(signer_keypair, v2.height, v2.view);
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);

        let ev = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert_invalid_evidence_rejected(&context, &ev, EvidenceValidationError::ViewMismatch);
    }

    #[test]
    fn validate_double_vote_rejects_same_block_hash() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let hash = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x42; 32],
        ));
        let zero_root = zero_state_root();
        let mut vote = Vote {
            phase: Phase::Prepare,
            block_hash: hash,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 9,
            view: 1,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 5,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut vote);
        let forged = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: vote.clone(),
                v2: vote.clone(),
            },
        };
        assert_eq!(
            validate_evidence(&forged, &context),
            Err(EvidenceValidationError::BlockHashMatch)
        );
    }

    #[test]
    fn validate_double_vote_rejects_epoch_mismatch() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x43; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Prepare,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 4,
            view: 3,
            epoch: 1,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 6,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v1);
        let mut v2 = v1.clone();
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x44; 32]),
        );
        v2.epoch = 2;
        ctx.sign_vote(&mut v2);
        let forged = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert_eq!(
            validate_evidence(&forged, &context),
            Err(EvidenceValidationError::EpochMismatch)
        );
    }

    #[test]
    fn validate_double_vote_rejects_signer_mismatch() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x45; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Prepare,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 7,
            view: 5,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 3,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v1);
        let mut v2 = v1.clone();
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x46; 32]),
        );
        v2.signer = 4;
        ctx.sign_vote(&mut v2);
        let forged = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert_eq!(
            validate_evidence(&forged, &context),
            Err(EvidenceValidationError::SignerMismatch)
        );
    }

    #[test]
    fn kind_payload_mismatch_is_rejected() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Prepare,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([0x50; 32]),
            ),
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 10,
            view: 1,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 1,
            bls_sig: Vec::new(),
        };
        let mut v2 = Vote {
            phase: Phase::Prepare,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([0x51; 32]),
            ),
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 10,
            view: 1,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 1,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let ev = Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert_eq!(
            validate_evidence(&ev, &context),
            Err(EvidenceValidationError::KindPayloadMismatch)
        );
    }

    #[test]
    fn store_deduplicates() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let h =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([4; 32]));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: super::super::consensus::Phase::Commit,
            block_hash: h,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 1,
            view: 1,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([5; 32]));
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let ev = check_double_vote(&v1, &v2).unwrap();
        let mut store = EvidenceStore::new();
        assert!(store.insert(&ev, &context));
        assert!(!store.insert(&ev, &context));
        // Listing contains exactly one
        assert_eq!(store.entries.len(), 1);
    }

    #[test]
    fn store_rejects_invalid_evidence() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let h = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x60; 32],
        ));
        let zero_root = zero_state_root();
        let mut v = Vote {
            phase: Phase::Prepare,
            block_hash: h,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 3,
            view: 2,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 5,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v);
        let forged = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: v.clone(),
                v2: v.clone(),
            },
        };
        let mut store = EvidenceStore::new();
        assert!(
            !store.insert(&forged, &context),
            "forged double-vote evidence must be rejected"
        );
        assert!(store.entries.is_empty());
    }

    #[test]
    fn persist_record_rejects_invalid_double_vote() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let state = test_state();
        let h = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x70; 32],
        ));
        let zero_root = zero_state_root();
        let mut v = Vote {
            phase: Phase::Prepare,
            block_hash: h,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 12,
            view: 4,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 6,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v);
        let forged = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: v.clone(),
                v2: v.clone(),
            },
        };
        assert!(
            !persist_record(&state, &forged, &context),
            "persist_record must ignore invalid double-vote evidence"
        );
        let view = state.world.consensus_evidence.view();
        assert_eq!(view.iter().count(), 0);
    }

    #[test]
    fn persist_record_rejects_missing_signature_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let evidence = double_vote_with_unchecked(&ctx, |v1, v2| {
            v1.bls_sig.clear();
            v2.bls_sig.clear();
        });
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::SignatureMissing,
        );
    }

    #[test]
    fn persist_record_rejects_truncated_signature_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let evidence = double_vote_with_unchecked(&ctx, |v1, v2| {
            v1.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
            v2.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
        });
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::SignatureTruncated,
        );
    }

    #[test]
    fn persist_record_rejects_invalid_signature_payload() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut evidence = double_vote_with(&ctx, |_, _| {});
        if let EvidencePayload::DoubleVote { v1, v2 } = &mut evidence.payload {
            if let Some(byte) = v1.bls_sig.first_mut() {
                *byte ^= 0x5A;
            }
            if let Some(byte) = v2.bls_sig.first_mut() {
                *byte ^= 0xA5;
            }
        }
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::SignatureInvalid,
        );
    }

    #[test]
    fn persist_record_rejects_duplicate_signer_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let evidence = double_vote_with(&ctx, |_, v2| {
            v2.signer = v2.signer.saturating_add(1);
        });
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::SignerMismatch,
        );
    }

    #[test]
    fn persist_record_rejects_height_mismatch_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let evidence = double_vote_with(&ctx, |_, v2| {
            v2.height = v2.height.saturating_add(1);
        });
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::HeightMismatch,
        );
    }

    #[test]
    fn persist_record_rejects_cross_view_mutation_before_signer_resolution() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let evidence = double_vote_with(&ctx, |_, v2| {
            v2.view = v2.view.saturating_add(1);
        });
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::ViewMismatch,
        );
    }

    #[test]
    fn persist_record_rejects_epoch_mismatch_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let evidence = double_vote_with(&ctx, |_, v2| {
            v2.epoch = v2.epoch.saturating_add(1);
        });
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::EpochMismatch,
        );
    }

    #[test]
    fn persist_record_rejects_phase_kind_mismatch_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let (v1, v2) = sample_double_vote_pair(&ctx);
        // Keep block hashes distinct but forge the evidence kind to mismatch the vote phase.
        let evidence = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::PhaseKindMismatch,
        );
    }

    #[test]
    fn persist_record_rejects_kind_payload_mismatch_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0x90; 32],
                )),
                tx_root: Hash::prehashed([0x91; 32]),
                state_root: Hash::prehashed([0x92; 32]),
                proposer: 7,
                height: 42,
                view: 3,
                epoch: 2,
                highest_qc: QcHeaderRef {
                    height: 41,
                    view: 2,
                    epoch: 2,
                    subject_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                        Hash::prehashed([0x93; 32]),
                    ),
                    phase: Phase::Commit,
                },
            },
            payload_hash: Hash::prehashed([0x94; 32]),
        };
        let evidence = Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::InvalidProposal {
                proposal,
                reason: "forged payload variant".to_owned(),
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::KindPayloadMismatch,
        );
    }

    #[test]
    fn persist_record_rejects_unverified_proposal_height_claim() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0xA0; 32],
                )),
                tx_root: Hash::prehashed([0xA1; 32]),
                state_root: Hash::prehashed([0xA2; 32]),
                proposer: 3,
                height: 40,
                view: 5,
                epoch: 1,
                highest_qc: QcHeaderRef {
                    height: 40,
                    view: 4,
                    epoch: 1,
                    subject_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                        Hash::prehashed([0xA3; 32]),
                    ),
                    phase: Phase::Commit,
                },
            },
            payload_hash: Hash::prehashed([0xA4; 32]),
        };
        let evidence = Evidence {
            kind: EvidenceKind::InvalidProposal,
            payload: EvidencePayload::InvalidProposal {
                proposal,
                reason: "stale highest_qc height".to_owned(),
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::UnverifiableInvalidProposal,
        );
    }

    #[test]
    fn persist_record_rejects_plausible_proposal_without_invalidity_proof() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let parent = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA5; 32]));
        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: parent,
                tx_root: Hash::prehashed([0xA6; 32]),
                state_root: Hash::prehashed([0xA7; 32]),
                proposer: 4,
                height: 41,
                view: 0,
                epoch: 1,
                highest_qc: QcHeaderRef {
                    height: 40,
                    view: 6,
                    epoch: 1,
                    subject_block_hash: parent,
                    phase: Phase::Commit,
                },
            },
            payload_hash: Hash::prehashed([0xA9; 32]),
        };
        let evidence = Evidence {
            kind: EvidenceKind::InvalidProposal,
            payload: EvidencePayload::InvalidProposal {
                proposal,
                reason: "view reset after height advance".to_owned(),
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::UnverifiableInvalidProposal,
        );
    }

    #[test]
    fn persist_record_rejects_unverified_proposal_parent_claim() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0xAA; 32],
                )),
                tx_root: Hash::prehashed([0xAB; 32]),
                state_root: Hash::prehashed([0xAC; 32]),
                proposer: 5,
                height: 60,
                view: 8,
                epoch: 2,
                highest_qc: QcHeaderRef {
                    height: 59,
                    view: 7,
                    epoch: 2,
                    subject_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                        Hash::prehashed([0xAD; 32]),
                    ),
                    phase: Phase::Commit,
                },
            },
            payload_hash: Hash::prehashed([0xAE; 32]),
        };
        let evidence = Evidence {
            kind: EvidenceKind::InvalidProposal,
            payload: EvidencePayload::InvalidProposal {
                proposal,
                reason: "parent/qc mismatch".to_owned(),
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::UnverifiableInvalidProposal,
        );
    }

    #[test]
    fn censorship_evidence_accepts_quorum_receipts() {
        let ctx = EvidenceTestContext::new(4);
        let context = ctx.validation_context();
        let tx_hash = sample_tx_hash(0xDD);
        let required = ctx.topology.min_votes_for_view_change();
        let receipts: Vec<_> = (0..required)
            .map(|idx| submission_receipt_for(&ctx, idx, tx_hash, 10))
            .collect();
        let evidence = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship { tx_hash, receipts },
        };
        assert!(validate_evidence(&evidence, &context).is_ok());
        let mut store = EvidenceStore::new();
        assert!(store.insert(&evidence, &context));
    }

    #[test]
    fn censorship_evidence_dedups_receipt_order() {
        let ctx = EvidenceTestContext::new(4);
        let context = ctx.validation_context();
        let tx_hash = sample_tx_hash(0xEE);
        let required = ctx.topology.min_votes_for_view_change();
        let receipts: Vec<_> = (0..required)
            .map(|idx| submission_receipt_for(&ctx, idx, tx_hash, 10))
            .collect();
        let mut reversed = receipts.clone();
        reversed.reverse();

        let evidence = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship { tx_hash, receipts },
        };
        let reordered = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: reversed,
            },
        };

        assert_eq!(evidence_key(&evidence), evidence_key(&reordered));

        let mut store = EvidenceStore::new();
        assert!(store.insert(&evidence, &context));
        assert!(
            !store.insert(&reordered, &context),
            "reordered receipts should not create a new evidence entry"
        );
    }

    #[test]
    fn censorship_subject_height_uses_latest_receipt() {
        let ctx = EvidenceTestContext::new(4);
        let tx_hash = sample_tx_hash(0xEF);
        let receipts = vec![
            submission_receipt_for(&ctx, 0, tx_hash, 5),
            submission_receipt_for(&ctx, 1, tx_hash, 12),
            submission_receipt_for(&ctx, 2, tx_hash, 9),
        ];
        let evidence = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship { tx_hash, receipts },
        };

        let (height, view) = evidence_subject_height_view(&evidence);
        assert_eq!(height, Some(12));
        assert_eq!(view, None);
    }

    #[test]
    fn double_vote_evidence_dedups_vote_order() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let (v1, v2) = sample_double_vote_pair(&ctx);
        let evidence = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: v1.clone(),
                v2: v2.clone(),
            },
        };
        let reordered = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1: v2, v2: v1 },
        };

        assert_eq!(evidence_key(&evidence), evidence_key(&reordered));

        let mut store = EvidenceStore::new();
        assert!(store.insert(&evidence, &context));
        assert!(
            !store.insert(&reordered, &context),
            "reordered votes should not create a new evidence entry"
        );
    }

    #[test]
    fn censorship_evidence_rejects_invalid_receipts() {
        let ctx = EvidenceTestContext::new(4);
        let context = ctx.validation_context();
        let tx_hash = sample_tx_hash(0xDE);

        let missing = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: Vec::new(),
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &missing,
            EvidenceValidationError::ReceiptMissing,
        );

        let mismatched = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash: sample_tx_hash(0xDF),
                receipts: vec![submission_receipt_for(&ctx, 0, tx_hash, 10)],
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &mismatched,
            EvidenceValidationError::ReceiptTxHashMismatch,
        );

        let outsider = checked_bls_keypair();
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash,
            entrypoint_hash: HashOf::from_untyped_unchecked(Hash::from(tx_hash)),
            signed_transaction_hash: Some(tx_hash),
            submitted_at_ms: 1,
            submitted_at_height: 3,
            signer: outsider.public_key().clone(),
        };
        let outsider_receipt = TransactionSubmissionReceipt::sign(payload, &outsider);
        let outsider_ev = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: vec![outsider_receipt],
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &outsider_ev,
            EvidenceValidationError::ReceiptSignerOutOfTopology,
        );

        let invalid_sig = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: vec![submission_receipt_with_invalid_signature(
                    &ctx, 0, tx_hash, 3,
                )],
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &invalid_sig,
            EvidenceValidationError::ReceiptSignatureInvalid,
        );

        let below_quorum = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: vec![submission_receipt_for(&ctx, 0, tx_hash, 3)],
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &below_quorum,
            EvidenceValidationError::ReceiptQuorumMissing,
        );
    }

    #[test]
    fn persist_record_rejects_mixed_manifest_payloads() {
        let ctx = test_context();
        let context = ctx.validation_context();
        for (kind, payload, expected) in mismatched_payload_cases(&ctx) {
            let evidence = Evidence { kind, payload };
            assert_invalid_evidence_rejected(&context, &evidence, expected);
        }
    }

    #[test]
    #[allow(clippy::type_complexity)]
    #[allow(clippy::too_many_lines)]
    fn fuzz_invalid_double_vote_mutations_are_rejected() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut rng = StdRng::seed_from_u64(0xF0_0D);
        let cases: &[(fn(&mut Vote, &mut Vote), EvidenceValidationError)] = &[
            (
                |_, v2| v2.signer = v2.signer.saturating_add(1),
                EvidenceValidationError::SignerMismatch,
            ),
            (
                |_, v2| v2.height = v2.height.saturating_add(1),
                EvidenceValidationError::HeightMismatch,
            ),
            (
                |_, v2| v2.view = v2.view.saturating_add(1),
                EvidenceValidationError::ViewMismatch,
            ),
            (
                |v1, v2| {
                    v2.block_hash = v1.block_hash;
                },
                EvidenceValidationError::BlockHashMatch,
            ),
            (
                |v1, v2| {
                    v1.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
                    v2.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
                },
                EvidenceValidationError::SignatureTruncated,
            ),
            (
                |_, v2| v2.epoch = v2.epoch.saturating_add(1),
                EvidenceValidationError::EpochMismatch,
            ),
        ];

        for _ in 0..32 {
            for (mutate, expected) in cases {
                let (mut v1, mut v2) = sample_double_vote_pair(&ctx);
                mutate(&mut v1, &mut v2);
                if !matches!(
                    expected,
                    EvidenceValidationError::SignatureMissing
                        | EvidenceValidationError::SignatureTruncated
                ) {
                    ctx.sign_vote(&mut v1);
                    ctx.sign_vote(&mut v2);
                }
                // jitter signatures to ensure encode/decode paths see varied payloads
                let noise: u8 = rng.random();
                v1.bls_sig.push(noise);
                v2.bls_sig.push(noise ^ 0xFF);

                let evidence = Evidence {
                    kind: EvidenceKind::DoublePrepare,
                    payload: EvidencePayload::DoubleVote { v1, v2 },
                };
                assert_invalid_evidence_rejected(&context, &evidence, *expected);
            }
        }

        // Cover signature missing and phase/kind mismatch explicitly.
        let evidence = double_vote_with_unchecked(&ctx, |v1, v2| {
            v1.bls_sig.clear();
            v2.bls_sig.clear();
        });
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::SignatureMissing,
        );

        let (v1, v2) = sample_double_vote_pair(&ctx);
        let forged = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert_invalid_evidence_rejected(
            &context,
            &forged,
            EvidenceValidationError::PhaseKindMismatch,
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fuzz_evidence_roundtrip_rejects_invalid_cases() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut rng = StdRng::seed_from_u64(0xE1D1_D00D);
        for _ in 0..32 {
            match rng.random::<u8>() % 4 {
                0 => {
                    let (v1, mut v2) = sample_double_vote_pair(&ctx);
                    let bump = u32::from(rng.random::<u8>() % 8 + 1);
                    v2.signer = v2.signer.saturating_add(bump);
                    ctx.sign_vote(&mut v2);
                    let evidence = Evidence {
                        kind: EvidenceKind::DoublePrepare,
                        payload: EvidencePayload::DoubleVote { v1, v2 },
                    };
                    assert_invalid_evidence_rejected(
                        &context,
                        &evidence,
                        EvidenceValidationError::SignerMismatch,
                    );
                }
                1 => {
                    let (v1, mut v2) = sample_double_vote_pair(&ctx);
                    if rng.random::<u8>() & 1 == 0 {
                        let delta = rng.random::<u64>() % 4 + 1;
                        v2.height = v2.height.saturating_add(delta);
                        ctx.sign_vote(&mut v2);
                        let evidence = Evidence {
                            kind: EvidenceKind::DoublePrepare,
                            payload: EvidencePayload::DoubleVote { v1, v2 },
                        };
                        assert_invalid_evidence_rejected(
                            &context,
                            &evidence,
                            EvidenceValidationError::HeightMismatch,
                        );
                    } else {
                        let delta = rng.random::<u64>() % 4 + 1;
                        v2.view = v2.view.saturating_add(delta);
                        ctx.sign_vote(&mut v2);
                        let evidence = Evidence {
                            kind: EvidenceKind::DoublePrepare,
                            payload: EvidencePayload::DoubleVote { v1, v2 },
                        };
                        assert_invalid_evidence_rejected(
                            &context,
                            &evidence,
                            EvidenceValidationError::ViewMismatch,
                        );
                    }
                }
                2 => {
                    let (mut v1, mut v2) = sample_double_vote_pair(&ctx);
                    if rng.random::<u8>() & 1 == 0 {
                        v1.bls_sig.clear();
                        v2.bls_sig.clear();
                        let evidence = Evidence {
                            kind: EvidenceKind::DoublePrepare,
                            payload: EvidencePayload::DoubleVote { v1, v2 },
                        };
                        assert_invalid_evidence_rejected(
                            &context,
                            &evidence,
                            EvidenceValidationError::SignatureMissing,
                        );
                    } else {
                        let bound = u16::try_from(super::MIN_BLS_SIGNATURE_LEN - 1).unwrap();
                        let truncate_to = usize::from(rng.random::<u16>() % bound + 1);
                        v1.bls_sig.truncate(truncate_to);
                        v2.bls_sig.truncate(truncate_to);
                        let evidence = Evidence {
                            kind: EvidenceKind::DoublePrepare,
                            payload: EvidencePayload::DoubleVote { v1, v2 },
                        };
                        assert_invalid_evidence_rejected(
                            &context,
                            &evidence,
                            EvidenceValidationError::SignatureTruncated,
                        );
                    }
                }
                _ => {
                    let horizon = rng.random::<u64>() % 8 + 1;
                    let stale_delta = rng.random::<u64>() % 5 + 1;
                    let current_height = horizon + stale_delta + (rng.random::<u64>() % 16 + 1);
                    let stale_height = current_height - horizon - stale_delta;
                    let mut v1;
                    let mut v2;
                    {
                        let pair = sample_double_vote_pair(&ctx);
                        v1 = pair.0;
                        v2 = pair.1;
                    }
                    v1.height = stale_height;
                    v2.height = stale_height;
                    let view = rng.random::<u64>() % 16 + 1;
                    v1.view = view;
                    v2.view = view;
                    let epoch = rng.random::<u64>() % 5;
                    v1.epoch = epoch;
                    v2.epoch = epoch;
                    ctx.sign_vote(&mut v1);
                    ctx.sign_vote(&mut v2);
                    let evidence = Evidence {
                        kind: EvidenceKind::DoublePrepare,
                        payload: EvidencePayload::DoubleVote {
                            v1: v1.clone(),
                            v2: v2.clone(),
                        },
                    };

                    let bytes = evidence.encode();
                    let mut slice = bytes.as_slice();
                    let decoded =
                        Evidence::decode(&mut slice).expect("decode stale evidence payload");
                    assert_eq!(decoded, evidence);
                    assert!(
                        validate_evidence(&decoded, &context).is_ok(),
                        "stale evidence must pass structural validation"
                    );

                    let state = state_with_horizon(current_height, horizon);
                    assert!(
                        !persist_record(&state, &decoded, &context),
                        "stale evidence (current_height={current_height}, horizon={horizon}, subject_height={stale_height}) must not persist"
                    );
                    let view = state.world.consensus_evidence.view();
                    assert_eq!(view.iter().count(), 0);
                }
            }
        }

        let mut cases = mismatched_payload_cases(&ctx);
        cases.shuffle(&mut rng);
        for (kind, payload, expected) in cases {
            let evidence = Evidence { kind, payload };
            assert_invalid_evidence_rejected(&context, &evidence, expected);
        }
    }

    #[test]
    fn persist_record_inserts_once() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let state = test_state();
        let h =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([6; 32]));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: super::super::consensus::Phase::Prepare,
            block_hash: h,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 3,
            view: 2,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 4,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([7; 32]));
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let evidence = check_double_vote(&v1, &v2).expect("double vote expected");

        assert!(persist_record(&state, &evidence, &context));
        // second insertion should be ignored
        assert!(!persist_record(&state, &evidence, &context));

        let view = state.world.consensus_evidence.view();
        let stored: Vec<_> = view.iter().map(|(_, rec)| rec.clone()).collect();
        assert_eq!(stored.len(), 1);
        let rec = &stored[0];
        assert_eq!(rec.evidence, evidence);
        assert_eq!(rec.recorded_at_height, v1.height);
        assert_eq!(rec.recorded_at_view, v1.view);
    }

    #[test]
    fn persist_record_inserts_once_for_precommit() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let state = test_state();
        let (mut v1, mut v2) = sample_double_vote_pair(&ctx);
        v1.phase = Phase::Commit;
        v2.phase = Phase::Commit;
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let evidence = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };

        assert!(persist_record(&state, &evidence, &context));
        assert!(
            !persist_record(&state, &evidence, &context),
            "duplicate precommit evidence should be ignored"
        );

        let view = state.world.consensus_evidence.view();
        let stored: Vec<_> = view.iter().map(|(_, rec)| rec.clone()).collect();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].evidence.kind, EvidenceKind::DoubleCommit);
    }

    #[test]
    fn record_double_vote_persists_once() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut store = EvidenceStore::new();
        let state = test_state();
        let (v1, v2) = sample_double_vote_pair(&ctx);

        assert!(
            record_double_vote(&mut store, &state, &v1, &v2, &context),
            "first equivocation must be recorded"
        );
        assert!(
            !record_double_vote(&mut store, &state, &v1, &v2, &context),
            "duplicate equivocation should be deduplicated"
        );

        let view = state.world.consensus_evidence.view();
        let stored: Vec<_> = view.iter().collect();
        assert_eq!(stored.len(), 1);
        assert_eq!(store.entries.len(), 1);
    }

    #[test]
    fn record_double_vote_rejects_persisted_duplicates() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let state = test_state();
        let (v1, v2) = sample_double_vote_pair(&ctx);

        let mut store = EvidenceStore::new();
        assert!(
            record_double_vote(&mut store, &state, &v1, &v2, &context),
            "first equivocation must be recorded"
        );
        assert_eq!(
            state.world.consensus_evidence.view().iter().count(),
            1,
            "evidence should be persisted to WSV"
        );

        // Simulate a restart with a fresh in-memory store.
        let mut fresh_store = EvidenceStore::new();
        assert!(
            !record_double_vote(&mut fresh_store, &state, &v1, &v2, &context),
            "persisted evidence must block duplicates even when the in-memory store is empty"
        );
        assert_eq!(
            state.world.consensus_evidence.view().iter().count(),
            1,
            "WSV must not store duplicates"
        );
        assert_eq!(
            fresh_store.entries.len(),
            1,
            "fresh store should still record the duplicate to avoid relogging"
        );
    }

    #[test]
    fn record_double_vote_handles_precommit_equivocation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut store = EvidenceStore::new();
        let state = test_state();
        let (mut v1, mut v2) = sample_double_vote_pair(&ctx);
        v1.phase = Phase::Commit;
        v2.phase = Phase::Commit;
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);

        assert!(
            record_double_vote(&mut store, &state, &v1, &v2, &context),
            "precommit equivocation should be recorded"
        );
        let view = state.world.consensus_evidence.view();
        let (_, record) = view.iter().next().expect("evidence must be stored");
        assert_eq!(record.evidence.kind, EvidenceKind::DoubleCommit);
    }

    #[test]
    fn record_double_vote_dedupes_per_phase() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut store = EvidenceStore::new();
        let state = test_state();

        // Prepare equivocation inserts once.
        let (prevote1, prevote2) = sample_double_vote_pair(&ctx);
        assert!(record_double_vote(
            &mut store, &state, &prevote1, &prevote2, &context
        ));
        assert!(
            !record_double_vote(&mut store, &state, &prevote1, &prevote2, &context),
            "duplicate prevote evidence should be ignored"
        );

        // Commit equivocation inserts independently and dedupes.
        let (mut precommit1, mut precommit2) = sample_double_vote_pair(&ctx);
        precommit1.phase = Phase::Commit;
        precommit2.phase = Phase::Commit;
        ctx.sign_vote(&mut precommit1);
        ctx.sign_vote(&mut precommit2);
        assert!(record_double_vote(
            &mut store,
            &state,
            &precommit1,
            &precommit2,
            &context
        ));
        assert!(
            !record_double_vote(&mut store, &state, &precommit1, &precommit2, &context),
            "duplicate precommit evidence should be ignored"
        );

        let view = state.world.consensus_evidence.view();
        let kinds: Vec<EvidenceKind> = view.iter().map(|(_, rec)| rec.evidence.kind).collect();
        assert_eq!(kinds.len(), 2, "one record per phase expected");
        assert!(
            kinds.contains(&EvidenceKind::DoublePrepare)
                && kinds.contains(&EvidenceKind::DoubleCommit),
            "prevote and precommit evidence should both be persisted"
        );
    }

    #[test]
    fn record_double_vote_detects_cross_phase_conflict() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut store = EvidenceStore::new();
        let state = test_state();
        let (v1, mut v2) = sample_double_vote_pair(&ctx);
        v2.phase = Phase::Commit;
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x90; 32]),
        );
        ctx.sign_vote(&mut v2);

        assert!(
            record_double_vote(&mut store, &state, &v1, &v2, &context),
            "cross-phase conflict must be recorded once"
        );
        assert!(
            !record_double_vote(&mut store, &state, &v1, &v2, &context),
            "duplicate cross-phase conflict should be deduplicated"
        );

        let view = state.world.consensus_evidence.view();
        let (_, record) = view.iter().next().expect("evidence must persist to WSV");
        assert_eq!(record.evidence.kind, EvidenceKind::DoubleCommit);
    }

    #[test]
    fn record_double_vote_dedupes_cross_phase_ordering() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut store = EvidenceStore::new();
        let state = test_state();
        let (mut prevote, mut precommit) = sample_double_vote_pair(&ctx);
        prevote.phase = Phase::Prepare;
        precommit.phase = Phase::Commit;
        precommit.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x91; 32]),
        );
        ctx.sign_vote(&mut prevote);
        ctx.sign_vote(&mut precommit);

        assert!(
            record_double_vote(&mut store, &state, &prevote, &precommit, &context),
            "first cross-phase conflict must be recorded"
        );
        assert!(
            !record_double_vote(&mut store, &state, &precommit, &prevote, &context),
            "reversed cross-phase detection should deduplicate the same conflict"
        );

        let view = state.world.consensus_evidence.view();
        assert_eq!(view.iter().count(), 1, "WSV must persist only one record");
        assert_eq!(store.entries.len(), 1, "in-memory store must dedupe");
    }

    #[test]
    fn persist_record_rejects_stale_evidence_replay() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let state = state_with_horizon(50, 3);
        let view = state.view();
        assert_eq!(
            view.world()
                .sumeragi_npos_parameters()
                .map(|params| params.evidence_horizon_blocks()),
            Some(3)
        );
        let current_height = u64::try_from(view.height()).unwrap_or(0);
        assert_eq!(current_height, 50);
        drop(view);

        let h = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0xB3; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Prepare,
            block_hash: h,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 40,
            view: 2,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 7,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0xB4; 32]),
        );
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let original = check_double_vote(&v1, &v2).expect("double vote expected");
        let encoded = original.encode();
        let mut slice = encoded.as_slice();
        let evidence = Evidence::decode(&mut slice).expect("roundtrip stale evidence");
        assert_eq!(evidence, original);
        assert!(
            !persist_record(&state, &evidence, &context),
            "evidence beyond configured horizon must not persist"
        );
        let view = state.world.consensus_evidence.view();
        assert_eq!(view.iter().count(), 0);
    }

    #[test]
    fn roadmap_invalid_evidence_roundtrip_cases() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let cases: &[EvidenceRoundtripCase] = &[
            (
                "duplicate signer",
                EvidenceValidationError::SignerMismatch,
                roundtrip_case_duplicate_signer,
            ),
            (
                "conflicting height",
                EvidenceValidationError::HeightMismatch,
                roundtrip_case_conflicting_height,
            ),
            (
                "conflicting view",
                EvidenceValidationError::ViewMismatch,
                roundtrip_case_conflicting_view,
            ),
            (
                "forged signature length",
                EvidenceValidationError::SignatureTruncated,
                roundtrip_case_signature_truncated,
            ),
            (
                "mixed manifest payload",
                EvidenceValidationError::KindPayloadMismatch,
                roundtrip_case_mixed_manifest_payload,
            ),
        ];

        for (label, expected, build) in cases {
            let evidence = build(&ctx);
            assert_invalid_evidence_rejected(&context, &evidence, *expected);
            assert!(
                validate_evidence(&evidence, &context).is_err(),
                "{label}: expected structural validation to fail"
            );
        }
    }
}
