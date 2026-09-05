//! Exact Sumeragi-v2 equivocation validation and bounded admission.
//!
//! The first release accepts only complete signed v2 artifact pairs bound to
//! an authenticated immutable height context. Retired global-v1 evidence
//! layouts are not decoded or reconstructed. Private observations stay in a
//! bounded process-local cache; only proofs carried by committed blocks enter
//! WSV.
use crate::state::{State, WorldReadOnly};
#[cfg(feature = "bls")]
use iroha_crypto::Algorithm;
use iroha_crypto::{Hash, Signature};
use iroha_data_model::{
    NetworkId,
    block::{
        consensus::{
            Evidence, EvidencePenaltyStatus, EvidenceRecord, Height,
            SumeragiV2EquivocationEvidence, View,
        },
        consensus_v2 as wire_v2,
    },
    consensus::NposPenaltyAction,
    nexus::PublicLaneValidatorRecord,
    prelude::PeerId,
};
use mv::storage::StorageReadOnly;
use std::collections::{BTreeMap, BTreeSet};
/// Maximum exact Sumeragi v2 equivocation proofs admitted by one block.
///
/// The count bound makes proof verification work predictable even when a peer
/// has accumulated a large local evidence backlog.
pub(crate) const MAX_V2_EVIDENCE_ADMISSIONS_PER_BLOCK: usize = 8;
/// Maximum aggregate Norito size of exact Sumeragi v2 equivocation proofs in
/// one block.
pub(crate) const MAX_V2_EVIDENCE_ADMISSION_BYTES: usize = 4 * 1024 * 1024;
/// Maximum aggregate Norito bytes retained for node-local pending evidence.
///
/// The pool also retains at most one proof per offender and frozen roster, but
/// this byte bound prevents large valid contexts from multiplying memory use.
pub(crate) const MAX_V2_LOCAL_EVIDENCE_BYTES: usize = 2 * MAX_V2_EVIDENCE_ADMISSION_BYTES;
/// Hard bound for committed evidence records retained in WSV.
///
/// Four complete validator rosters leave ample audit history while keeping
/// state growth independent of node uptime and hostile gossip volume.
pub(crate) const MAX_V2_COMMITTED_EVIDENCE_RECORDS: usize = 4 * wire_v2::MAX_VALIDATORS_PER_HEIGHT;
/// Maximum aggregate Norito bytes of exact v2 proofs retained in WSV.
///
/// Four maximum-sized admission batches preserve the same first-release
/// retention scale as the four-roster count bound without letting unusually
/// large, valid height contexts multiply state and response memory.
pub(crate) const MAX_V2_COMMITTED_EVIDENCE_BYTES: usize = 4 * MAX_V2_EVIDENCE_ADMISSION_BYTES;
const V2_EVIDENCE_KEY_DOMAIN: &[u8] = b"iroha:sumeragi:v2:evidence:v1";
const V2_EVIDENCE_ROSTER_KEY_DOMAIN: &[u8] = b"iroha:sumeragi:v2:evidence-roster:v1";
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct V2EvidenceOffenderRosterKey {
    offender: PeerId,
    epoch: u64,
    epoch_end_height: Height,
    roster_hash: Hash,
}
/// Process-local proof retained only after complete cryptographic validation.
pub(crate) struct LocalV2EvidenceRecord {
    evidence: SumeragiV2EquivocationEvidence,
    offender_roster: V2EvidenceOffenderRosterKey,
    encoded_len: usize,
}
/// Return the exact bare-Norito byte length charged for one v2 proof.
///
/// This is deliberately the proof payload rather than mutable record metadata,
/// so block admission and committed-state retention use one canonical measure.
pub(crate) fn v2_evidence_encoded_len(evidence: &SumeragiV2EquivocationEvidence) -> usize {
    use norito::codec::Encode as _;

    evidence.encoded_len()
}
/// Add exact evidence lengths without overflow and without crossing `limit`.
pub(crate) fn checked_v2_evidence_byte_sum(
    initial: usize,
    encoded_lengths: impl IntoIterator<Item = usize>,
    limit: usize,
) -> Option<usize> {
    if initial > limit {
        return None;
    }
    encoded_lengths.into_iter().try_fold(initial, |total, len| {
        let next = total.checked_add(len)?;
        (next <= limit).then_some(next)
    })
}
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
/// Derive a deterministic, fixed-width deduplication key for an evidence entry.
#[must_use]
pub fn evidence_key(ev: &Evidence) -> Hash {
    let canonical = canonicalize_evidence(ev);
    evidence_key_inner(&canonical)
}
fn evidence_key_inner(ev: &Evidence) -> Hash {
    use norito::codec::Encode as _;
    let encoded = ev.encode();
    let mut preimage = Vec::with_capacity(V2_EVIDENCE_KEY_DOMAIN.len() + encoded.len());
    preimage.extend_from_slice(V2_EVIDENCE_KEY_DOMAIN);
    preimage.extend_from_slice(&encoded);
    Hash::new(preimage)
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
pub(crate) fn v2_evidence_admission_key(evidence: &SumeragiV2EquivocationEvidence) -> Hash {
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
fn v2_conflict_signer(conflict: &wire_v2::SumeragiV2Equivocation) -> wire_v2::ValidatorIndex {
    match conflict {
        wire_v2::SumeragiV2Equivocation::Proposal { first, .. } => first.proposer,
        wire_v2::SumeragiV2Equivocation::PhaseVote { first, .. } => first.signer,
        wire_v2::SumeragiV2Equivocation::TimeoutVote { first, .. } => first.signer,
    }
}
fn v2_evidence_offender(evidence: &SumeragiV2EquivocationEvidence) -> Option<PeerId> {
    let signer = usize::try_from(v2_conflict_signer(&evidence.conflict)).ok()?;
    evidence
        .context
        .roster
        .get(signer)
        .map(|validator| validator.validator.clone())
}
fn v2_evidence_offender_roster_key(
    evidence: &SumeragiV2EquivocationEvidence,
) -> Option<V2EvidenceOffenderRosterKey> {
    use norito::codec::Encode as _;

    let roster = evidence.context.roster.encode();
    Some(V2EvidenceOffenderRosterKey {
        offender: v2_evidence_offender(evidence)?,
        epoch: evidence.context.epoch,
        epoch_end_height: evidence.context.epoch_end_height,
        roster_hash: Hash::new_from_chunks(&[V2_EVIDENCE_ROSTER_KEY_DOMAIN, &roster]),
    })
}
/// Return whether unresolved evidence belongs to this exact retained validator tenure.
pub(crate) fn has_pending_v2_evidence_for_validator_tenure(
    world: &impl WorldReadOnly,
    validator_record: &PublicLaneValidatorRecord,
) -> bool {
    world
        .consensus_evidence()
        .iter()
        .any(|(_, evidence_record)| {
            matches!(
                evidence_record.penalty_status,
                EvidencePenaltyStatus::Pending
            ) && v2_evidence_offender(&evidence_record.evidence.equivocation).as_ref()
                == Some(&validator_record.peer_id)
                && crate::smartcontracts::isi::staking::validator_tenure_contains_height(
                    validator_record,
                    evidence_record.evidence.equivocation.context.height,
                )
                // Cleanup must fail closed: a malformed retained row cannot prove
                // that unresolved evidence belongs to some other tenure.
                .unwrap_or(true)
        })
}
fn evidence_record_is_terminal(record: &EvidenceRecord) -> bool {
    record.penalty_status.is_terminal()
}
fn evidence_record_is_stale(
    record: &EvidenceRecord,
    current_height: u64,
    horizon: Option<u64>,
) -> bool {
    let subject_height = v2_conflict_round(&record.evidence.equivocation.conflict).height;
    !evidence_within_configured_horizon(current_height, horizon, Some(subject_height))
}
fn configured_v2_evidence_horizon(world: &(impl WorldReadOnly + ?Sized)) -> Option<u64> {
    world
        .sumeragi_npos_parameters()
        .map(|params| params.evidence_horizon_blocks())
}
/// Return whether one committed record is safe to prune in this exact world.
///
/// Consensus derives a canonical prune plan from immutable parent state, then
/// rechecks every target against the post-execution overlay before deletion.
/// Missing or invalid signed NPoS parameters therefore fail closed.
pub(crate) fn v2_committed_evidence_record_is_prunable(
    world: &(impl WorldReadOnly + ?Sized),
    record: &EvidenceRecord,
    current_height: u64,
) -> bool {
    evidence_record_is_terminal(record)
        && configured_v2_evidence_horizon(world).is_some_and(|horizon| {
            horizon > 0 && evidence_record_is_stale(record, current_height, Some(horizon))
        })
}
/// Generation-coherent committed evidence inputs used by proposal and validation.
pub(crate) struct V2CommittedEvidenceSnapshot {
    pub(crate) horizon: Option<u64>,
    pub(crate) records: Vec<(Hash, EvidenceRecord)>,
    pub(crate) record_capacity_exceeded: bool,
    pub(crate) byte_capacity_exceeded: bool,
}
/// Copy the complete bounded evidence table and its governing horizon from one world view.
pub(crate) fn v2_committed_evidence_snapshot(
    world: &(impl WorldReadOnly + ?Sized),
) -> V2CommittedEvidenceSnapshot {
    let mut records = Vec::with_capacity(MAX_V2_COMMITTED_EVIDENCE_RECORDS);
    let mut total_bytes = 0_usize;
    let mut record_capacity_exceeded = false;
    let mut byte_capacity_exceeded = false;
    for (key, record) in world.consensus_evidence().iter() {
        if records.len() == MAX_V2_COMMITTED_EVIDENCE_RECORDS {
            record_capacity_exceeded = true;
            break;
        }
        let encoded_len = v2_evidence_encoded_len(&record.evidence.equivocation);
        if encoded_len > MAX_V2_EVIDENCE_ADMISSION_BYTES {
            byte_capacity_exceeded = true;
            break;
        }
        let Some(next_total) = checked_v2_evidence_byte_sum(
            total_bytes,
            [encoded_len],
            MAX_V2_COMMITTED_EVIDENCE_BYTES,
        ) else {
            byte_capacity_exceeded = true;
            break;
        };
        records.push((*key, record.clone()));
        total_bytes = next_total;
    }
    V2CommittedEvidenceSnapshot {
        horizon: configured_v2_evidence_horizon(world),
        records,
        record_capacity_exceeded,
        byte_capacity_exceeded,
    }
}
fn retained_v2_evidence_bytes(
    records: &[(Hash, EvidenceRecord)],
    pruned: &BTreeSet<Hash>,
) -> Option<usize> {
    checked_v2_evidence_byte_sum(
        0,
        records
            .iter()
            .filter(|(key, _)| !pruned.contains(key))
            .map(|(_, record)| v2_evidence_encoded_len(&record.evidence.equivocation)),
        MAX_V2_COMMITTED_EVIDENCE_BYTES,
    )
}
/// Validate the complete canonical evidence table restored from durable state.
///
/// The table is consensus-owned WSV. Restart must reject missing integrity,
/// non-canonical proofs, invalid signatures, foreign networks, impossible
/// lifecycle heights, or state beyond the fixed first-release count and byte
/// capacities.
pub(crate) fn validate_persisted_v2_evidence_records(
    world: &(impl WorldReadOnly + ?Sized),
    kura: &crate::kura::Kura,
    expected_network_id: &NetworkId,
    committed_height: u64,
) -> Result<(), String> {
    let records = world.consensus_evidence();
    let record_count = records.iter().count();
    if record_count > MAX_V2_COMMITTED_EVIDENCE_RECORDS {
        return Err(format!(
            "committed evidence table exceeds the first-release capacity of {MAX_V2_COMMITTED_EVIDENCE_RECORDS} records"
        ));
    }
    let mut total_bytes = 0_usize;
    for (_, record) in records.iter() {
        let encoded_len = v2_evidence_encoded_len(&record.evidence.equivocation);
        if encoded_len > MAX_V2_EVIDENCE_ADMISSION_BYTES {
            return Err(format!(
                "committed evidence proof exceeds the first-release individual capacity of {MAX_V2_EVIDENCE_ADMISSION_BYTES} bytes"
            ));
        }
        total_bytes = checked_v2_evidence_byte_sum(
            total_bytes,
            [encoded_len],
            MAX_V2_COMMITTED_EVIDENCE_BYTES,
        )
        .ok_or_else(|| {
            format!(
                "committed evidence table exceeds the first-release capacity of {MAX_V2_COMMITTED_EVIDENCE_BYTES} proof bytes"
            )
        })?;
    }
    if record_count == 0 {
        return Ok(());
    }
    let npos_parameters = world.sumeragi_npos_parameters().ok_or_else(|| {
        "committed evidence requires valid signed Sumeragi NPoS parameters".to_owned()
    })?;
    let evidence_horizon = npos_parameters.evidence_horizon_blocks();
    let slashing_delay = npos_parameters.slashing_delay_blocks();
    let mut retained_offender_rosters = BTreeSet::new();
    for (key, record) in records.iter() {
        if &record.evidence != &canonical_v2_evidence(&record.evidence.equivocation) {
            return Err("committed evidence proof is not canonically ordered".to_owned());
        }
        if key != &evidence_key(&record.evidence) {
            return Err("committed evidence key does not match its exact proof".to_owned());
        }
        if &record.evidence.equivocation.context.network_id != expected_network_id {
            return Err("committed evidence belongs to another network".to_owned());
        }
        validate_v2_equivocation(&record.evidence.equivocation)
            .map_err(|error| format!("committed evidence proof is invalid: {error}"))?;
        let subject_height = v2_conflict_round(&record.evidence.equivocation.conflict).height;
        match kura.v2_finality_artifact(subject_height) {
            Ok(Some(artifact))
                if artifact.height_context != record.evidence.equivocation.context =>
            {
                return Err(
                    "committed evidence context disagrees with the retained Kura finality artifact"
                        .to_owned(),
                );
            }
            Ok(_) => {
                // Hash-only snapshot imports authenticate the complete World through
                // the outer checkpoint/CommitQC. Re-anchor whenever a historical
                // finality sidecar is retained, but absence is valid on that corridor.
            }
            Err(error) => {
                return Err(format!(
                    "failed to authenticate committed evidence against Kura finality: {error}"
                ));
            }
        }
        if subject_height >= record.recorded_at_height {
            return Err(
                "committed evidence must describe a height before its admission height".to_owned(),
            );
        }
        if record.recorded_at_height > committed_height {
            return Err("committed evidence admission height is in the future".to_owned());
        }
        if record.recorded_at_height.saturating_sub(subject_height) > evidence_horizon {
            return Err("committed evidence was admitted outside the signed horizon".to_owned());
        }
        let due_height = record
            .recorded_at_height
            .checked_add(slashing_delay)
            .ok_or_else(|| "committed evidence penalty due height overflows".to_owned())?;
        match record.penalty_status {
            EvidencePenaltyStatus::Pending => {
                if committed_height >= due_height {
                    return Err(
                        "committed evidence remains pending at or after its penalty due height"
                            .to_owned(),
                    );
                }
            }
            EvidencePenaltyStatus::Applied { height } => {
                if height != due_height {
                    return Err(
                        "committed evidence applied height differs from its deterministic due height"
                            .to_owned(),
                    );
                }
                if height > committed_height {
                    return Err("committed evidence applied height is in the future".to_owned());
                }
            }
            EvidencePenaltyStatus::Cancelled { height } => {
                if height <= record.recorded_at_height || height >= due_height {
                    return Err(
                        "committed evidence cancellation must follow admission and precede its due height"
                            .to_owned(),
                    );
                }
                if height > committed_height {
                    return Err(
                        "committed evidence cancellation height is in the future".to_owned()
                    );
                }
            }
        }
        let offender_roster = v2_evidence_offender_roster_key(&record.evidence.equivocation)
            .ok_or_else(|| {
                "committed evidence signer is absent from its frozen roster".to_owned()
            })?;
        if !retained_offender_rosters.insert(offender_roster) {
            return Err(
                "committed evidence contains multiple retained proofs for one offender and frozen roster"
                    .to_owned(),
            );
        }
    }
    Ok(())
}
/// Select deterministic committed evidence records to prune before admission.
///
/// Terminal records outside the configured horizon are removed. Capacity
/// pressure never evicts an in-horizon record: terminal keys remain replay
/// fences until their horizon expires, and admission reports table-full
/// backpressure while no stale terminal record can be reclaimed.
pub(crate) fn v2_committed_evidence_prune_keys(
    records: &[(Hash, EvidenceRecord)],
    current_height: u64,
    horizon: Option<u64>,
    _incoming_records: usize,
) -> Vec<Hash> {
    let mut pruned = BTreeSet::new();
    for (key, record) in records {
        if evidence_record_is_terminal(record)
            && evidence_record_is_stale(record, current_height, horizon)
        {
            pruned.insert(*key);
        }
    }
    pruned.into_iter().collect()
}
/// Derive the exact canonical evidence-prune plan from immutable parent state.
///
/// Candidate validation and post-finality application both call this helper
/// before opening their block transaction. A parameter update in the candidate
/// therefore cannot change which parent records are reclaimed after validation.
pub(crate) fn v2_committed_evidence_prune_keys_from_state(
    state: &State,
    current_height: u64,
    incoming_records: usize,
) -> Vec<Hash> {
    let view = state.view();
    let snapshot = v2_committed_evidence_snapshot(view.world());
    if snapshot.record_capacity_exceeded || snapshot.byte_capacity_exceeded {
        return Vec::new();
    }
    v2_committed_evidence_prune_keys(
        &snapshot.records,
        current_height,
        snapshot.horizon,
        incoming_records,
    )
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
) -> Result<Vec<Hash>, EvidenceValidationError> {
    if admissions.len() > MAX_V2_EVIDENCE_ADMISSIONS_PER_BLOCK {
        return Err(EvidenceValidationError::V2AdmissionTooMany);
    }
    let total_bytes = checked_v2_evidence_byte_sum(
        0,
        admissions.iter().map(v2_evidence_encoded_len),
        MAX_V2_EVIDENCE_ADMISSION_BYTES,
    )
    .ok_or(EvidenceValidationError::V2AdmissionTooLarge)?;
    let view = state.view();
    let snapshot = v2_committed_evidence_snapshot(view.world());
    if snapshot.record_capacity_exceeded {
        return Err(EvidenceValidationError::V2AdmissionTableFull);
    }
    if snapshot.byte_capacity_exceeded {
        return Err(EvidenceValidationError::V2AdmissionTableBytesFull);
    }
    let horizon = snapshot.horizon;
    let records = snapshot.records;
    let pruned =
        v2_committed_evidence_prune_keys(&records, block_height, horizon, admissions.len())
            .into_iter()
            .collect::<BTreeSet<_>>();
    let retained_count = records.len().saturating_sub(pruned.len());
    if !admissions.is_empty()
        && retained_count.saturating_add(admissions.len()) > MAX_V2_COMMITTED_EVIDENCE_RECORDS
    {
        return Err(EvidenceValidationError::V2AdmissionTableFull);
    }
    let retained_bytes = retained_v2_evidence_bytes(&records, &pruned)
        .ok_or(EvidenceValidationError::V2AdmissionTableBytesFull)?;
    if checked_v2_evidence_byte_sum(
        retained_bytes,
        [total_bytes],
        MAX_V2_COMMITTED_EVIDENCE_BYTES,
    )
    .is_none()
    {
        return Err(EvidenceValidationError::V2AdmissionTableBytesFull);
    }
    let committed_keys = records.iter().map(|(key, _)| *key).collect::<BTreeSet<_>>();
    let mut retained_offender_rosters = records
        .iter()
        .filter(|(key, _)| !pruned.contains(key))
        .filter_map(|(_, record)| v2_evidence_offender_roster_key(&record.evidence.equivocation))
        .collect::<BTreeSet<_>>();
    let mut keys = Vec::with_capacity(admissions.len());
    let mut previous_key: Option<Hash> = None;
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
        if committed_keys.contains(&key) {
            return Err(EvidenceValidationError::V2AdmissionAlreadyCommitted);
        }
        validate_v2_evidence_context_anchor(state, evidence)?;
        validate_v2_equivocation(evidence)?;
        let offender_roster = v2_evidence_offender_roster_key(evidence)
            .ok_or(EvidenceValidationError::V2ArtifactInvalid)?;
        if !retained_offender_rosters.insert(offender_roster) {
            return Err(EvidenceValidationError::V2AdmissionOffenderRetained);
        }
        previous_key = Some(key);
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
    admission_keys: &[Hash],
    actions: &[NposPenaltyAction],
) -> Result<(), EvidenceValidationError> {
    let conflicts = actions.iter().any(|action| {
        let evidence_key = match action {
            NposPenaltyAction::ConsensusSlash(action) => Some(&action.evidence_key),
            NposPenaltyAction::MarkConsensusEvidenceApplied(action) => Some(&action.evidence_key),
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
    let view = state.view();
    let snapshot = v2_committed_evidence_snapshot(view.world());
    pending_v2_evidence_admissions_from_snapshot(state, proposal_height, &snapshot)
}
/// Select local admissions against the same parent snapshot used for penalties.
pub(crate) fn pending_v2_evidence_admissions_from_snapshot(
    state: &State,
    proposal_height: u64,
    snapshot: &V2CommittedEvidenceSnapshot,
) -> Vec<SumeragiV2EquivocationEvidence> {
    if snapshot.record_capacity_exceeded || snapshot.byte_capacity_exceeded {
        return Vec::new();
    }
    let horizon = snapshot.horizon;
    let records = &snapshot.records;
    let committed_keys = records.iter().map(|(key, _)| *key).collect::<BTreeSet<_>>();
    let stale_terminal_prune_keys =
        v2_committed_evidence_prune_keys(records, proposal_height, horizon, 0);
    let pruned = stale_terminal_prune_keys
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let retained_offender_rosters = records
        .iter()
        .filter(|(key, _)| !pruned.contains(key))
        .filter_map(|(_, record)| v2_evidence_offender_roster_key(&record.evidence.equivocation))
        .collect::<BTreeSet<_>>();
    let retained_count = records
        .len()
        .saturating_sub(stale_terminal_prune_keys.len());
    let available_slots = MAX_V2_COMMITTED_EVIDENCE_RECORDS.saturating_sub(retained_count);
    if available_slots == 0 {
        return Vec::new();
    }
    let Some(retained_bytes) = retained_v2_evidence_bytes(records, &pruned) else {
        return Vec::new();
    };

    let mut pending = state.sumeragi_v2_pending_evidence.lock();
    let mut persisted_contexts = BTreeMap::new();
    pending.retain(|stored_key, record| {
        let evidence = &record.evidence;
        let round = v2_conflict_round(&evidence.conflict);
        if &evidence.context.network_id != state.network_id_ref()
            || round.height >= proposal_height
            || !evidence_within_configured_horizon(proposal_height, horizon, Some(round.height))
            || committed_keys.contains(stored_key)
            || v2_evidence_admission_key(evidence) != *stored_key
            || retained_offender_rosters.contains(&record.offender_roster)
        {
            return false;
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
            .is_some_and(|persisted| v2_evidence_matches_persisted_context(evidence, persisted));
        context_anchored
    });

    let mut selected = Vec::new();
    let mut selected_bytes = 0_usize;
    let mut selected_committed_bytes = retained_bytes;
    for record in pending.values() {
        if selected.len() == MAX_V2_EVIDENCE_ADMISSIONS_PER_BLOCK
            || selected.len() == available_slots
        {
            break;
        }
        let Some(next_bytes) = selected_bytes.checked_add(record.encoded_len) else {
            continue;
        };
        if next_bytes > MAX_V2_EVIDENCE_ADMISSION_BYTES {
            continue;
        }
        let Some(next_committed_bytes) = checked_v2_evidence_byte_sum(
            selected_committed_bytes,
            [record.encoded_len],
            MAX_V2_COMMITTED_EVIDENCE_BYTES,
        ) else {
            continue;
        };
        selected.push(record.evidence.clone());
        selected_bytes = next_bytes;
        selected_committed_bytes = next_committed_bytes;
    }
    selected
}
/// Validate and retain exact Sumeragi v2 equivocation artifacts for admission.
///
/// The caller supplies the immutable context and PoPs recovered from the
/// trusted context store. The proof enters a bounded node-local cache only
/// after full structural, roster, PoP, and signature validation. Private
/// observation timing never mutates WSV; a later block admission does that.
pub(crate) fn retain_sumeragi_v2_equivocation(
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
    if v2_evidence_encoded_len(&payload) > MAX_V2_EVIDENCE_ADMISSION_BYTES {
        return Err(EvidenceValidationError::V2AdmissionTooLarge);
    }
    validate_v2_equivocation(&payload)?;
    if &payload.context.network_id != state.network_id_ref() {
        return Err(EvidenceValidationError::V2AdmissionWrongNetwork);
    }
    let canonical = canonicalize_v2_equivocation_evidence(&payload);
    Ok(retain_validated_local_evidence(state, canonical))
}
fn retain_validated_local_evidence(
    state: &State,
    canonical: SumeragiV2EquivocationEvidence,
) -> bool {
    let view = state.view();
    let Ok(current_height) = u64::try_from(view.height()) else {
        return false;
    };
    let snapshot = v2_committed_evidence_snapshot(view.world());
    if snapshot.record_capacity_exceeded || snapshot.byte_capacity_exceeded {
        return false;
    }
    let horizon = snapshot.horizon;
    let encoded_len = v2_evidence_encoded_len(&canonical);
    if encoded_len > MAX_V2_EVIDENCE_ADMISSION_BYTES {
        return false;
    }
    let subject_height = v2_conflict_round(&canonical.conflict).height;
    let Some(next_height) = current_height.checked_add(1) else {
        return false;
    };
    if subject_height > next_height {
        return false;
    }
    let Some(after_subject_height) = subject_height.checked_add(1) else {
        return false;
    };
    let earliest_admission_height = next_height.max(after_subject_height);
    if !evidence_within_configured_horizon(earliest_admission_height, horizon, Some(subject_height))
    {
        return false;
    }
    let key = v2_evidence_admission_key(&canonical);
    let offender_roster = v2_evidence_offender_roster_key(&canonical)
        .expect("validated Sumeragi v2 evidence signer belongs to its frozen roster");
    let pruned =
        v2_committed_evidence_prune_keys(&snapshot.records, earliest_admission_height, horizon, 1)
            .into_iter()
            .collect::<BTreeSet<_>>();
    if snapshot.records.iter().any(|(committed_key, record)| {
        !pruned.contains(committed_key)
            && (committed_key == &key
                || v2_evidence_offender_roster_key(&record.evidence.equivocation).as_ref()
                    == Some(&offender_roster))
    }) {
        return false;
    }

    let mut pending = state.sumeragi_v2_pending_evidence.lock();
    pending.retain(|_, record| {
        let height = v2_conflict_round(&record.evidence.conflict).height;
        let Some(after_height) = height.checked_add(1) else {
            return false;
        };
        let earliest_height = next_height.max(after_height);
        evidence_within_configured_horizon(earliest_height, horizon, Some(height))
    });
    if pending.contains_key(&key)
        || pending
            .values()
            .any(|existing| existing.offender_roster == offender_roster)
    {
        return false;
    }
    let retained_bytes = pending
        .values()
        .try_fold(0_usize, |total, record| {
            total.checked_add(record.encoded_len)
        })
        .unwrap_or(usize::MAX);
    if pending.len() >= MAX_V2_COMMITTED_EVIDENCE_RECORDS
        || retained_bytes
            .checked_add(encoded_len)
            .is_none_or(|bytes| bytes > MAX_V2_LOCAL_EVIDENCE_BYTES)
    {
        return false;
    }
    pending.insert(
        key,
        LocalV2EvidenceRecord {
            evidence: canonical,
            offender_roster,
            encoded_len,
        },
    );
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
    /// A retained committed proof already accounts for this offender in the frozen roster.
    V2AdmissionOffenderRetained,
    /// The bounded committed evidence table has no reclaimable capacity.
    V2AdmissionTableFull,
    /// The bounded committed evidence table has no reclaimable proof-byte capacity.
    V2AdmissionTableBytesFull,
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
            V2AdmissionOffenderRetained => {
                "retained Sumeragi v2 evidence already accounts for this offender and frozen roster"
            }
            V2AdmissionTableFull => {
                "bounded Sumeragi v2 evidence table has no reclaimable capacity"
            }
            V2AdmissionTableBytesFull => {
                "bounded Sumeragi v2 evidence table has no reclaimable proof-byte capacity"
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
        certificate
            .bls_aggregate_signature()
            .map_err(|_| EvidenceValidationError::V2ArtifactInvalid)?,
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
        prelude::{AccountId, ChainId},
    };
    use mv::cell::Cell;
    fn test_network_id(seed: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            seed,
        )))
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
        let mut state = new_test_state_for_v2_fixture_with_world(fixture, world);
        super::super::penalties::configure_penalty_staking_state_for_tests(&mut state);
        install_v2_finality_for_fixture(&state, fixture);
        state
    }
    fn new_test_state_for_v2_fixture_with_world(
        fixture: &V2EvidenceFixture,
        world: World,
    ) -> State {
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        State::new_with_chain_and_network_id_for_testing(
            world,
            kura,
            query,
            ChainId::from("sumeragi-v2-evidence-display-name"),
            fixture.context.network_id,
        )
    }
    fn test_state_for_v2_fixture_with_world(fixture: &V2EvidenceFixture, world: World) -> State {
        let state = new_test_state_for_v2_fixture_with_world(fixture, world);
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
            let network_id = test_network_id(b"sumeragi-v2-evidence-genesis");
            let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
                crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(
                    network_id, 7, &roster,
                );
            let context = wire_v2::HeightContext {
                network_id,
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
                kagemusha_mint_finality_epoch_id,
                kagemusha_mint_finality_epoch_roster,
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
        fn for_epoch(epoch: u64) -> Self {
            let mut fixture = Self::new();
            fixture.context.epoch = epoch;
            fixture
                .context
                .validate()
                .expect("epoch-specific v2 evidence context remains valid");
            fixture
        }
        fn for_height(height: u64) -> Self {
            let mut fixture = Self::new();
            let snapshot_height = height
                .checked_sub(1)
                .filter(|snapshot_height| *snapshot_height > 0)
                .expect("snapshot-backed evidence height must exceed one");
            fixture.context.height = height;
            fixture.context.snapshot_bootstrap = Some(wire_v2::SnapshotBootstrapAnchor {
                snapshot_height,
                snapshot_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"v2 evidence snapshot block",
                )),
                snapshot_block_creation_time_ms: snapshot_height.saturating_mul(1_000),
                snapshot_state_hash: Hash::new(b"v2 evidence snapshot state"),
            });
            fixture
                .context
                .validate()
                .expect("snapshot-backed v2 evidence context remains valid");
            fixture
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
            wire_v2::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
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
        let mut executed_block: iroha_data_model::block::SignedBlock = committed.into();
        executed_block
            .set_transaction_results(Vec::new(), &[], Vec::new())
            .expect("attach deterministic v2 evidence fixture results");
        let block = std::sync::Arc::new(executed_block);
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
        let execution_commitment =
            wire_v2::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
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
        canonical_v2_phase_vote_evidence_for_signer(fixture, 1, first_seed, second_seed)
    }
    fn canonical_v2_phase_vote_evidence_for_signer(
        fixture: &V2EvidenceFixture,
        signer: wire_v2::ValidatorIndex,
        first_seed: u8,
        second_seed: u8,
    ) -> SumeragiV2EquivocationEvidence {
        canonicalize_v2_equivocation_evidence(&fixture.payload(
            wire_v2::SumeragiV2Equivocation::PhaseVote {
                first: fixture.vote(
                    signer,
                    wire_v2::GlobalPhase::Prepare,
                    fixture.subject(first_seed),
                ),
                second: fixture.vote(
                    signer,
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
            finalized_global_beacon_pulse: None,
            v2_evidence_admissions: admissions,
            penalty_actions: Vec::new(),
        };
        let evidence_prune_keys = v2_committed_evidence_prune_keys_from_state(
            state,
            height,
            effects.v2_evidence_admissions.len(),
        );
        let mut transaction = state_block.consensus_effects_transaction();
        super::super::penalties::apply_npos_consensus_effects_to_transaction(
            &mut transaction,
            &effects,
            &evidence_prune_keys,
            None,
            &[],
            height,
            view,
            now_ms,
        )
        .expect("valid exact v2 admission applies");
        transaction.apply_consensus_effects();
        state_block
            .commit_world_overlay_for_testing()
            .expect("test admission block commits");
    }
    fn penalty_header(height: u64) -> BlockHeader {
        BlockHeader::new(
            core::num::NonZeroU64::new(height).expect("non-zero penalty test height"),
            None,
            None,
            None,
            height.saturating_mul(1_000),
            0,
        )
    }
    fn insert_terminal_v2_evidence_for_test(
        state: &State,
        evidence: SumeragiV2EquivocationEvidence,
    ) -> Hash {
        let key = v2_evidence_admission_key(&evidence);
        let mut records = state.world.consensus_evidence.block();
        records.insert(
            key,
            EvidenceRecord {
                evidence: canonical_v2_evidence(&evidence),
                recorded_at_height: 2,
                recorded_at_view: 0,
                recorded_at_ms: 20,
                penalty_status: EvidencePenaltyStatus::Applied { height: 2 },
            },
        );
        records.commit();
        key
    }
    fn add_v2_penalty_validator(state: &State, peer: &PeerId) {
        super::super::penalties::seed_penalty_validator_for_tests(
            state,
            iroha_data_model::nexus::LaneId::SINGLE,
            peer,
            iroha_primitives::numeric::Quantity::from(100_u64),
        );
    }
    #[test]
    fn malformed_validator_tenure_cannot_escape_pending_evidence_lien() {
        let fixture = V2EvidenceFixture::new();
        let mut state = test_state_for_v2_fixture(&fixture);
        super::super::penalties::configure_penalty_staking_state_for_tests(&mut state);
        let offender = fixture.context.roster[1].validator.clone();
        add_v2_penalty_validator(&state, &offender);
        let evidence = canonical_v2_phase_vote_evidence(&fixture, 0x41, 0x42);
        let evidence_key = v2_evidence_admission_key(&evidence);
        let mut evidence_records = state.world.consensus_evidence.block();
        evidence_records.insert(
            evidence_key,
            EvidenceRecord {
                evidence: canonical_v2_evidence(&evidence),
                recorded_at_height: 1,
                recorded_at_view: 0,
                recorded_at_ms: 1,
                penalty_status: EvidencePenaltyStatus::Pending,
            },
        );
        evidence_records.commit();

        let validator = AccountId::new(offender.public_key().clone());
        let validator_key = (iroha_data_model::nexus::LaneId::SINGLE, validator);
        let mut validators = state.world.public_lane_validators.block();
        let mut malformed = validators
            .get(&validator_key)
            .cloned()
            .expect("validator fixture exists");
        malformed.activation_height = 2;
        malformed.deactivation_height = Some(1);
        validators.insert(validator_key.clone(), malformed);
        validators.commit();

        let view = state.view();
        let record = view
            .world()
            .public_lane_validators()
            .get(&validator_key)
            .expect("malformed retained validator row remains visible");
        assert!(
            has_pending_v2_evidence_for_validator_tenure(view.world(), record),
            "malformed tenure metadata must not release unresolved evidence custody"
        );
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
    fn evidence_keys_are_fixed_width_and_pair_order_independent() {
        let fixture = V2EvidenceFixture::new();
        let evidence = canonical_v2_phase_vote_evidence(&fixture, 0x71, 0x72);
        let swapped = SumeragiV2EquivocationEvidence {
            context: evidence.context.clone(),
            proofs_of_possession: evidence.proofs_of_possession.clone(),
            conflict: swap_v2_conflict(&evidence.conflict),
        };
        let key = v2_evidence_admission_key(&evidence);
        assert_eq!(key.as_ref().len(), Hash::LENGTH);
        assert_eq!(key, v2_evidence_admission_key(&swapped));
    }
    #[test]
    fn local_retention_keeps_one_proof_per_offender_and_frozen_roster() {
        let fixture = V2EvidenceFixture::new();
        let state = test_state_for_v2_fixture(&fixture);
        for (index, (first_seed, second_seed)) in
            [(0x73, 0x74), (0x75, 0x76)].into_iter().enumerate()
        {
            let evidence = canonical_v2_phase_vote_evidence(&fixture, first_seed, second_seed);
            let retained = retain_sumeragi_v2_equivocation(
                &state,
                &evidence.context,
                &evidence.proofs_of_possession,
                evidence.conflict,
            )
            .expect("valid exact proof");
            assert_eq!(retained, index == 0);
        }
        assert_eq!(state.sumeragi_v2_pending_evidence.lock().len(), 1);
        assert_eq!(state.world.consensus_evidence.view().iter().count(), 0);
    }
    #[test]
    fn local_retention_rejects_evidence_beyond_the_active_height() {
        let fixture = V2EvidenceFixture::for_height(2);
        let state = new_test_state_for_v2_fixture_with_world(&fixture, World::default());
        let evidence = canonical_v2_phase_vote_evidence(&fixture, 0x75, 0x76);

        assert_eq!(
            retain_sumeragi_v2_equivocation(
                &state,
                &evidence.context,
                &evidence.proofs_of_possession,
                evidence.conflict,
            ),
            Ok(false)
        );
        assert!(state.sumeragi_v2_pending_evidence.lock().is_empty());
    }
    #[test]
    fn stale_terminal_record_does_not_fence_fresh_local_evidence() {
        let old_fixture = V2EvidenceFixture::new();
        let fresh_fixture = V2EvidenceFixture::for_height(3);
        let mut params = Parameters::default();
        params.set_parameter(Parameter::Custom(
            SumeragiNposParameters {
                evidence_horizon_blocks: 2,
                slashing_delay_blocks: 1,
                ..SumeragiNposParameters::default()
            }
            .into_custom_parameter(),
        ));
        let mut world = World::default();
        world.parameters = Cell::new(params);
        let mut state = test_state_for_v2_fixture_with_world(&old_fixture, world);
        for seed in 1_u8..=3 {
            state.push_block_hash_for_testing(HashOf::from_untyped_unchecked(Hash::new([seed])));
        }

        let old = canonical_v2_phase_vote_evidence(&old_fixture, 0x75, 0x76);
        let old_key = v2_evidence_admission_key(&old);
        let fresh = canonical_v2_phase_vote_evidence(&fresh_fixture, 0x77, 0x78);
        assert_eq!(
            v2_evidence_offender_roster_key(&old),
            v2_evidence_offender_roster_key(&fresh),
            "height alone must not change the frozen offender/roster identity"
        );
        let mut records = state.world.consensus_evidence.block();
        records.insert(
            old_key,
            EvidenceRecord {
                evidence: canonical_v2_evidence(&old),
                recorded_at_height: 2,
                recorded_at_view: 0,
                recorded_at_ms: 20,
                penalty_status: EvidencePenaltyStatus::Applied { height: 3 },
            },
        );
        records.commit();

        let snapshot = v2_committed_evidence_snapshot(&state.world.view());
        assert!(
            v2_committed_evidence_prune_keys(&snapshot.records, 3, snapshot.horizon, 1).is_empty(),
            "the terminal replay fence remains live at the committed parent height"
        );
        assert_eq!(
            v2_committed_evidence_prune_keys(&snapshot.records, 4, snapshot.horizon, 1),
            vec![old_key],
            "the terminal replay fence expires at the fresh proof's earliest admission height"
        );
        assert_eq!(
            retain_sumeragi_v2_equivocation(
                &state,
                &fresh.context,
                &fresh.proofs_of_possession,
                fresh.conflict,
            ),
            Ok(true)
        );
        assert_eq!(state.sumeragi_v2_pending_evidence.lock().len(), 1);
    }
    #[test]
    fn stale_pending_records_do_not_consume_local_capacity() {
        let old_fixture = V2EvidenceFixture::new();
        let fresh_fixture = V2EvidenceFixture::for_height(3);
        let mut state = test_state_for_v2_fixture_with_horizon(&old_fixture, 2);
        for seed in 1_u8..=3 {
            state.push_block_hash_for_testing(HashOf::from_untyped_unchecked(Hash::new([seed])));
        }

        let old = canonical_v2_phase_vote_evidence(&old_fixture, 0x79, 0x7A);
        let base_roster = v2_evidence_offender_roster_key(&old)
            .expect("valid fixture signer belongs to its frozen roster");
        let mut pending = state.sumeragi_v2_pending_evidence.lock();
        for index in 0..MAX_V2_COMMITTED_EVIDENCE_RECORDS {
            let discriminator = u64::try_from(index).expect("bounded evidence index fits u64");
            pending.insert(
                Hash::new(discriminator.to_be_bytes()),
                LocalV2EvidenceRecord {
                    evidence: old.clone(),
                    offender_roster: V2EvidenceOffenderRosterKey {
                        roster_hash: Hash::new_from_chunks(&[
                            b"stale pending evidence row",
                            &discriminator.to_be_bytes(),
                        ]),
                        ..base_roster.clone()
                    },
                    encoded_len: 1,
                },
            );
        }
        drop(pending);

        let fresh = canonical_v2_phase_vote_evidence(&fresh_fixture, 0x7B, 0x7C);
        assert_eq!(
            retain_sumeragi_v2_equivocation(
                &state,
                &fresh.context,
                &fresh.proofs_of_possession,
                fresh.conflict,
            ),
            Ok(true),
            "rows stale at the fresh proof's earliest admission height must be reclaimed first"
        );
        assert_eq!(state.sumeragi_v2_pending_evidence.lock().len(), 1);
    }
    #[test]
    fn committed_admission_rejects_second_proof_for_offender_and_frozen_roster() {
        let fixture = V2EvidenceFixture::new();
        let state = test_state_for_v2_fixture(&fixture);
        let first = canonical_v2_phase_vote_evidence(&fixture, 0x77, 0x78);
        let second = canonical_v2_phase_vote_evidence(&fixture, 0x79, 0x7A);
        apply_v2_admissions_for_test(&state, vec![first], 2, 0, 20);
        assert_eq!(
            validate_v2_evidence_admissions(&state, 3, &[second]),
            Err(EvidenceValidationError::V2AdmissionOffenderRetained)
        );
    }

    #[test]
    fn terminal_in_horizon_record_fences_offender_and_frozen_roster() {
        let fixture = V2EvidenceFixture::new();
        let state = test_state_for_v2_fixture(&fixture);
        let first = canonical_v2_phase_vote_evidence(&fixture, 0x7B, 0x7C);
        let first_key = v2_evidence_admission_key(&first);
        apply_v2_admissions_for_test(&state, vec![first], 2, 0, 20);
        let mut records = state.world.consensus_evidence.block();
        let mut terminal = records
            .get(&first_key)
            .cloned()
            .expect("first proof was committed");
        terminal.penalty_status = EvidencePenaltyStatus::Applied { height: 2 };
        records.insert(first_key, terminal);
        records.commit();

        let second = canonical_v2_phase_vote_evidence(&fixture, 0x7D, 0x7E);
        assert_eq!(
            validate_v2_evidence_admissions(&state, 3, &[second]),
            Err(EvidenceValidationError::V2AdmissionOffenderRetained),
            "one Byzantine validator must not fill the bounded table with terminal replay fences"
        );
    }
    #[test]
    fn cancelled_in_horizon_record_fences_offender_and_frozen_roster() {
        let fixture = V2EvidenceFixture::new();
        let state = test_state_for_v2_fixture(&fixture);
        let first = canonical_v2_phase_vote_evidence(&fixture, 0x81, 0x82);
        let first_key = v2_evidence_admission_key(&first);
        apply_v2_admissions_for_test(&state, vec![first], 2, 0, 20);
        let mut records = state.world.consensus_evidence.block();
        let mut cancelled = records
            .get(&first_key)
            .cloned()
            .expect("first proof was committed");
        cancelled.penalty_status = EvidencePenaltyStatus::Cancelled { height: 3 };
        records.insert(first_key, cancelled);
        records.commit();

        let second = canonical_v2_phase_vote_evidence(&fixture, 0x83, 0x84);
        assert_eq!(
            validate_v2_evidence_admissions(&state, 4, &[second]),
            Err(EvidenceValidationError::V2AdmissionOffenderRetained),
            "cancellation must preserve the offender/roster replay and capacity fence"
        );
    }
    #[test]
    fn same_offender_in_distinct_frozen_epoch_is_admissible() {
        let first_fixture = V2EvidenceFixture::for_epoch(7);
        let second_fixture = V2EvidenceFixture::for_epoch(8);
        let first = canonical_v2_phase_vote_evidence(&first_fixture, 0x85, 0x86);
        let second = canonical_v2_phase_vote_evidence(&second_fixture, 0x87, 0x88);
        assert_eq!(
            v2_evidence_offender(&first),
            v2_evidence_offender(&second),
            "the fixtures must retain the same offender"
        );
        assert_ne!(
            v2_evidence_offender_roster_key(&first),
            v2_evidence_offender_roster_key(&second),
            "a finalized epoch is part of the frozen-roster fence identity"
        );

        let state = test_state_for_v2_fixture(&second_fixture);
        insert_terminal_v2_evidence_for_test(&state, first);
        validate_v2_evidence_admissions(&state, 3, &[second])
            .expect("a retained proof from another frozen epoch must not suppress admission");
    }
    #[test]
    fn persisted_evidence_rejects_overdue_pending_status() {
        let fixture = V2EvidenceFixture::new();
        let state = test_state_for_v2_fixture_with_slashing_delay(&fixture, 1);
        let evidence = canonical_v2_phase_vote_evidence(&fixture, 0x89, 0x8A);
        let key = v2_evidence_admission_key(&evidence);
        let mut records = state.world.consensus_evidence.block();
        records.insert(
            key,
            EvidenceRecord {
                evidence: canonical_v2_evidence(&evidence),
                recorded_at_height: 2,
                recorded_at_view: 0,
                recorded_at_ms: 20,
                penalty_status: EvidencePenaltyStatus::Pending,
            },
        );
        records.commit();

        let world = state.world.view();
        validate_persisted_v2_evidence_records(&world, state.kura(), state.network_id_ref(), 2)
            .expect("pending evidence immediately before its due height is canonical");
        let error =
            validate_persisted_v2_evidence_records(&world, state.kura(), state.network_id_ref(), 3)
                .expect_err("pending evidence at its deterministic due height must fail restart");
        assert!(
            error.contains("remains pending at or after its penalty due height"),
            "unexpected persisted evidence validation error: {error}"
        );
    }
    #[test]
    fn persisted_evidence_rejects_duplicate_cancelled_offender_roster_fences() {
        let fixture = V2EvidenceFixture::new();
        let state = test_state_for_v2_fixture_with_slashing_delay(&fixture, 10);
        let first = canonical_v2_phase_vote_evidence(&fixture, 0x8B, 0x8C);
        let second = canonical_v2_phase_vote_evidence(&fixture, 0x8D, 0x8E);
        let mut records = state.world.consensus_evidence.block();
        for evidence in [first, second] {
            records.insert(
                v2_evidence_admission_key(&evidence),
                EvidenceRecord {
                    evidence: canonical_v2_evidence(&evidence),
                    recorded_at_height: 2,
                    recorded_at_view: 0,
                    recorded_at_ms: 20,
                    penalty_status: EvidencePenaltyStatus::Cancelled { height: 3 },
                },
            );
        }
        records.commit();

        let world = state.world.view();
        let error =
            validate_persisted_v2_evidence_records(&world, state.kura(), state.network_id_ref(), 3)
                .expect_err("cancelled records remain duplicate replay and capacity fences");
        assert!(
            error.contains("multiple retained proofs for one offender and frozen roster"),
            "unexpected persisted evidence validation error: {error}"
        );
    }
    #[test]
    fn committed_evidence_byte_sum_is_checked_and_boundary_inclusive() {
        assert_eq!(
            checked_v2_evidence_byte_sum(
                MAX_V2_COMMITTED_EVIDENCE_BYTES - 1,
                [1],
                MAX_V2_COMMITTED_EVIDENCE_BYTES,
            ),
            Some(MAX_V2_COMMITTED_EVIDENCE_BYTES)
        );
        assert_eq!(
            checked_v2_evidence_byte_sum(
                MAX_V2_COMMITTED_EVIDENCE_BYTES - 1,
                [2],
                MAX_V2_COMMITTED_EVIDENCE_BYTES,
            ),
            None
        );
        assert_eq!(
            checked_v2_evidence_byte_sum(usize::MAX, [1], usize::MAX),
            None,
            "machine-word overflow must fail even when the nominal limit is usize::MAX"
        );
    }
    #[test]
    fn committed_evidence_byte_sum_accounts_for_retained_and_incoming_proofs() {
        let retained = MAX_V2_COMMITTED_EVIDENCE_BYTES - MAX_V2_EVIDENCE_ADMISSION_BYTES;
        assert_eq!(
            checked_v2_evidence_byte_sum(
                retained,
                [MAX_V2_EVIDENCE_ADMISSION_BYTES],
                MAX_V2_COMMITTED_EVIDENCE_BYTES,
            ),
            Some(MAX_V2_COMMITTED_EVIDENCE_BYTES)
        );
        assert_eq!(
            checked_v2_evidence_byte_sum(
                retained + 1,
                [MAX_V2_EVIDENCE_ADMISSION_BYTES],
                MAX_V2_COMMITTED_EVIDENCE_BYTES,
            ),
            None
        );
    }
    #[test]
    fn stale_terminal_prune_reclaims_committed_evidence_bytes() {
        let fixture = V2EvidenceFixture::new();
        let evidence =
            canonical_v2_evidence(&canonical_v2_phase_vote_evidence(&fixture, 0x79, 0x7A));
        let proof_bytes = v2_evidence_encoded_len(&evidence.equivocation);
        let stale_terminal_key = Hash::prehashed([0x01; Hash::LENGTH]);
        let stale_pending_key = Hash::prehashed([0x02; Hash::LENGTH]);
        let records = vec![
            (
                stale_terminal_key,
                EvidenceRecord {
                    evidence: evidence.clone(),
                    recorded_at_height: 1,
                    recorded_at_view: 0,
                    recorded_at_ms: 0,
                    penalty_status: EvidencePenaltyStatus::Applied { height: 2 },
                },
            ),
            (
                stale_pending_key,
                EvidenceRecord {
                    evidence,
                    recorded_at_height: 1,
                    recorded_at_view: 0,
                    recorded_at_ms: 0,
                    penalty_status: EvidencePenaltyStatus::Pending,
                },
            ),
        ];
        let pruned = v2_committed_evidence_prune_keys(&records, 3, Some(1), 1)
            .into_iter()
            .collect::<BTreeSet<_>>();

        assert_eq!(pruned, BTreeSet::from([stale_terminal_key]));
        assert_eq!(
            retained_v2_evidence_bytes(&records, &pruned),
            Some(proof_bytes)
        );
        assert_eq!(
            retained_v2_evidence_bytes(&records, &BTreeSet::new()),
            proof_bytes.checked_mul(2)
        );
    }
    #[test]
    fn committed_table_prunes_only_stale_terminal_records() {
        let fixture = V2EvidenceFixture::new();
        let evidence =
            canonical_v2_evidence(&canonical_v2_phase_vote_evidence(&fixture, 0x7B, 0x7C));
        let stale_terminal_key = Hash::prehashed([0x01; Hash::LENGTH]);
        let stale_pending_key = Hash::prehashed([0x02; Hash::LENGTH]);
        let records = vec![
            (
                stale_terminal_key,
                EvidenceRecord {
                    evidence: evidence.clone(),
                    recorded_at_height: 1,
                    recorded_at_view: 0,
                    recorded_at_ms: 0,
                    penalty_status: EvidencePenaltyStatus::Applied { height: 2 },
                },
            ),
            (
                stale_pending_key,
                EvidenceRecord {
                    evidence,
                    recorded_at_height: 1,
                    recorded_at_view: 0,
                    recorded_at_ms: 0,
                    penalty_status: EvidencePenaltyStatus::Pending,
                },
            ),
        ];
        let pruned = v2_committed_evidence_prune_keys(
            &records,
            3,
            Some(1),
            MAX_V2_COMMITTED_EVIDENCE_RECORDS,
        );
        assert_eq!(pruned, vec![stale_terminal_key]);
    }
    #[test]
    fn full_in_horizon_terminal_table_backpressures_without_pruning_replay_fences() {
        let fixture = V2EvidenceFixture::new();
        let state = test_state_for_v2_fixture_with_horizon(&fixture, 100);
        let stored_evidence =
            canonical_v2_evidence(&canonical_v2_phase_vote_evidence(&fixture, 0x73, 0x74));
        let terminal_record = EvidenceRecord {
            evidence: stored_evidence,
            recorded_at_height: 1,
            recorded_at_view: 0,
            recorded_at_ms: 10,
            penalty_status: EvidencePenaltyStatus::Applied { height: 1 },
        };
        let mut records = state.world.consensus_evidence.block();
        for index in 0..MAX_V2_COMMITTED_EVIDENCE_RECORDS {
            let key = Hash::new(index.to_be_bytes());
            records.insert(key, terminal_record.clone());
        }
        records.commit();

        let view = state.view();
        let snapshot = v2_committed_evidence_snapshot(view.world());
        assert_eq!(snapshot.records.len(), MAX_V2_COMMITTED_EVIDENCE_RECORDS);
        assert!(
            v2_committed_evidence_prune_keys(&snapshot.records, 2, snapshot.horizon, 1,).is_empty()
        );

        let pending = canonical_v2_phase_vote_evidence_for_signer(&fixture, 2, 0x75, 0x76);
        assert!(
            retain_sumeragi_v2_equivocation(
                &state,
                &pending.context,
                &pending.proofs_of_possession,
                pending.conflict.clone(),
            )
            .expect("valid in-horizon proof enters the local pending pool")
        );
        assert!(pending_v2_evidence_admissions(&state, 2).is_empty());
        assert_eq!(
            validate_v2_evidence_admissions(&state, 2, &[pending]),
            Err(EvidenceValidationError::V2AdmissionTableFull)
        );
    }
    #[test]
    fn post_execution_horizon_expansion_cannot_prune_a_revived_replay_fence() {
        const BLOCK_HEIGHT: u64 = 3;
        const CANDIDATE_HORIZON: u64 = 100;

        let fixture = V2EvidenceFixture::new();
        let state = test_state_for_v2_fixture_with_horizon(&fixture, 1);
        let evidence = canonical_v2_phase_vote_evidence(&fixture, 0x7D, 0x7E);
        let key = insert_terminal_v2_evidence_for_test(&state, evidence);
        let evidence_prune_keys =
            v2_committed_evidence_prune_keys_from_state(&state, BLOCK_HEIGHT, 0);
        assert_eq!(evidence_prune_keys, vec![key]);

        let header = BlockHeader::new(
            core::num::NonZeroU64::new(BLOCK_HEIGHT).expect("non-zero test height"),
            None,
            None,
            None,
            30,
            0,
        );
        let mut state_block = state.block(header);
        let mut candidate_transaction = state_block.transaction();
        candidate_transaction
            .world
            .parameters
            .get_mut()
            .set_parameter(Parameter::Custom(
                SumeragiNposParameters {
                    evidence_horizon_blocks: CANDIDATE_HORIZON,
                    ..SumeragiNposParameters::default()
                }
                .into_custom_parameter(),
            ));
        candidate_transaction.apply();
        assert_eq!(
            state_block
                .world
                .sumeragi_npos_parameters()
                .map(|params| params.evidence_horizon_blocks()),
            Some(CANDIDATE_HORIZON)
        );

        let effects = iroha_data_model::consensus::NposConsensusEffects::default();
        let validation_error =
            super::super::penalties::validate_npos_consensus_effects_after_execution(
                &mut state_block,
                &effects,
                &evidence_prune_keys,
                None,
                &[],
                BLOCK_HEIGHT,
                0,
                30,
            )
            .expect_err(
                "a post-execution horizon expansion must invalidate the parent prune target",
            );
        assert!(
            validation_error
                .to_string()
                .contains("not stale under the post-execution evidence horizon"),
            "unexpected validation error: {validation_error}"
        );
        assert!(
            state_block.world.consensus_evidence.get(&key).is_some(),
            "post-execution validation must roll its prune simulation back"
        );
        let mut effects_transaction = state_block.consensus_effects_transaction();
        let application_error =
            match super::super::penalties::apply_npos_consensus_effects_to_transaction(
                &mut effects_transaction,
                &effects,
                &evidence_prune_keys,
                None,
                &[],
                BLOCK_HEIGHT,
                0,
                30,
            ) {
                Ok(_) => panic!("commit application must reject the same revived prune target"),
                Err(error) => error,
            };
        assert!(
            application_error
                .to_string()
                .contains("not stale under the post-execution evidence horizon"),
            "unexpected application error: {application_error}"
        );
        assert!(
            effects_transaction
                .world
                .consensus_evidence
                .get(&key)
                .is_some()
        );
    }
    #[test]
    fn immutable_parent_horizon_keeps_terminal_evidence_after_candidate_shrinks_horizon() {
        const BLOCK_HEIGHT: u64 = 3;
        const CANDIDATE_HORIZON: u64 = 1;

        let fixture = V2EvidenceFixture::new();
        let state = test_state_for_v2_fixture_with_horizon(&fixture, 100);
        let evidence = canonical_v2_phase_vote_evidence(&fixture, 0x7F, 0x80);
        let key = insert_terminal_v2_evidence_for_test(&state, evidence);
        let evidence_prune_keys =
            v2_committed_evidence_prune_keys_from_state(&state, BLOCK_HEIGHT, 0);
        assert!(evidence_prune_keys.is_empty());

        let header = BlockHeader::new(
            core::num::NonZeroU64::new(BLOCK_HEIGHT).expect("non-zero test height"),
            None,
            None,
            None,
            30,
            0,
        );
        let mut state_block = state.block(header);
        let mut candidate_transaction = state_block.transaction();
        candidate_transaction
            .world
            .parameters
            .get_mut()
            .set_parameter(Parameter::Custom(
                SumeragiNposParameters {
                    evidence_horizon_blocks: CANDIDATE_HORIZON,
                    ..SumeragiNposParameters::default()
                }
                .into_custom_parameter(),
            ));
        candidate_transaction.apply();
        assert_eq!(
            state_block
                .world
                .sumeragi_npos_parameters()
                .map(|params| params.evidence_horizon_blocks()),
            Some(CANDIDATE_HORIZON)
        );

        let effects = iroha_data_model::consensus::NposConsensusEffects::default();
        super::super::penalties::validate_npos_consensus_effects_after_execution(
            &mut state_block,
            &effects,
            &evidence_prune_keys,
            None,
            &[],
            BLOCK_HEIGHT,
            0,
            30,
        )
        .expect("post-execution validation uses the immutable parent keep plan");
        assert!(
            state_block.world.consensus_evidence.get(&key).is_some(),
            "post-execution validation must leave the retained evidence intact"
        );
        let mut effects_transaction = state_block.consensus_effects_transaction();
        super::super::penalties::apply_npos_consensus_effects_to_transaction(
            &mut effects_transaction,
            &effects,
            &evidence_prune_keys,
            None,
            &[],
            BLOCK_HEIGHT,
            0,
            30,
        )
        .expect("the immutable parent keep plan remains applicable");
        effects_transaction.apply_consensus_effects();
        state_block
            .commit_world_overlay_for_testing()
            .expect("candidate horizon shrink and parent keep commit");
        assert!(state.world.consensus_evidence.view().get(&key).is_some());
    }
    #[test]
    fn sumeragi_v2_equivocation_local_retention_deduplicates_swaps() {
        let fixture = V2EvidenceFixture::new();
        let conflict = wire_v2::SumeragiV2Equivocation::PhaseVote {
            first: fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x81)),
            second: fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x82)),
        };
        let state = test_state_for_v2_fixture(&fixture);
        assert!(
            retain_sumeragi_v2_equivocation(
                &state,
                &fixture.context,
                &fixture.proofs,
                conflict.clone(),
            )
            .expect("retain valid v2 evidence")
        );
        assert!(
            !retain_sumeragi_v2_equivocation(
                &state,
                &fixture.context,
                &fixture.proofs,
                swap_v2_conflict(&conflict),
            )
            .expect("swapped replay is valid")
        );
        // Another service sharing this process observes the same local key.
        assert!(
            !retain_sumeragi_v2_equivocation(&state, &fixture.context, &fixture.proofs, conflict,)
                .expect("exact restart replay is valid")
        );
        assert_eq!(state.world.consensus_evidence.view().iter().count(), 0);
        let pending = state.sumeragi_v2_pending_evidence.lock();
        assert_eq!(pending.len(), 1);
        assert_eq!(
            pending
                .values()
                .next()
                .expect("retained v2 evidence")
                .evidence
                .context,
            fixture.context
        );
    }
    #[test]
    fn sumeragi_v2_equivocation_retention_rejects_invalid_artifacts() {
        let fixture = V2EvidenceFixture::new();
        let mut forged = fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x84));
        forged.signature[0] ^= 0x80;
        let conflict = wire_v2::SumeragiV2Equivocation::PhaseVote {
            first: fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x83)),
            second: forged,
        };
        let state = test_state_for_v2_fixture(&fixture);
        assert_eq!(
            retain_sumeragi_v2_equivocation(&state, &fixture.context, &fixture.proofs, conflict,),
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
        let second = canonical_v2_phase_vote_evidence_for_signer(&fixture, 2, 0x95, 0x96);
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
                penalty_status: EvidencePenaltyStatus::Pending,
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
        let mut proposer = test_state_for_v2_fixture_with_slashing_delay(&fixture, 1);
        let mut follower = test_state_for_v2_fixture_with_slashing_delay(&fixture, 1);
        let offender = fixture.context.roster[1].validator.clone();
        add_v2_penalty_validator(&mut proposer, &offender);
        add_v2_penalty_validator(&mut follower, &offender);
        let conflict = wire_v2::SumeragiV2Equivocation::PhaseVote {
            first: fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x99)),
            second: fixture.vote(1, wire_v2::GlobalPhase::Prepare, fixture.subject(0x9A)),
        };
        retain_sumeragi_v2_equivocation(&proposer, &fixture.context, &fixture.proofs, conflict)
            .expect("local exact proof validates");
        let admissions = pending_v2_evidence_admissions(&proposer, 2);
        assert_eq!(admissions.len(), 1);
        assert!(pending_v2_evidence_admissions(&follower, 2).is_empty());
        validate_v2_evidence_admissions(&follower, 2, &admissions)
            .expect("unaware follower revalidates the attached exact proof");
        let proposer_precommit = super::super::penalties::PenaltyApplier::new(
            &proposer,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(2))
        .expect("derive proposer pre-admission effects");
        let follower_precommit = super::super::penalties::PenaltyApplier::new(
            &follower,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(2))
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
        assert_eq!(proposer_record.recorded_at_height, 2);
        assert_eq!(proposer_record.recorded_at_view, 3);
        assert_eq!(proposer_record.recorded_at_ms, 77);
        let proposer_same_block = super::super::penalties::PenaltyApplier::new(
            &proposer,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(2))
        .expect("derive proposer same-height effects");
        assert!(proposer_same_block.penalty_actions.is_empty());
        let proposer_effects = super::super::penalties::PenaltyApplier::new(
            &proposer,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(3))
        .expect("derive proposer post-admission effects");
        let follower_effects = super::super::penalties::PenaltyApplier::new(
            &follower,
            #[cfg(feature = "telemetry")]
            None,
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(&penalty_header(3))
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
                evidence_key: key,
                signer: 1,
                peer_id: peer.clone(),
                lane_id: iroha_data_model::nexus::LaneId::SINGLE,
                validator: iroha_data_model::account::AccountId::new(peer.public_key().clone()),
                slash_id: key,
                amount: iroha_primitives::numeric::Quantity::from(1_u64),
            },
        );
        assert_eq!(
            validate_v2_admission_penalty_separation(&[key], &[slash]),
            Err(EvidenceValidationError::V2AdmissionSameBlockPenalty)
        );
    }
}
