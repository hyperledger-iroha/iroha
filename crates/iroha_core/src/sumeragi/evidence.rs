//! Native signed evidence admission and bounded local observation custody.
//!
//! The canonical frame carries signed artifacts only. Every admitting execution obtains
//! authority and exact original signer attribution from its immutable native execution tip.
//! Local observations never enter World until a canonical block admits the same proof.

use crate::state::{
    EvidencePreparationError, NativeExecutionTip, State, StateReadOnly, StateView, WorldReadOnly,
};
use iroha_allocation::{AllocationBudget, ChargedBuffer};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus::{
            Evidence, EvidenceAttribution, EvidenceOffender, EvidencePenaltyStatus, EvidenceRecord,
        },
    },
    consensus::NposPenaltyAction,
};
use iroha_sumeragi::message::Evidence as NativeEvidence;
use mv::storage::StorageReadOnly;

/// Maximum number of original reports admitted by one carrier.
pub(crate) const MAX_EVIDENCE_ADMISSIONS_PER_BLOCK: usize = 8;
/// Aggregate canonical proof bytes admitted by one carrier.
pub(crate) const MAX_EVIDENCE_ADMISSION_BYTES: usize =
    iroha_sumeragi::message::MAX_EVIDENCE_FRAME_BYTES;
/// Four maximum global rosters of retained forensic records.
pub(crate) const MAX_COMMITTED_EVIDENCE_RECORDS: usize = 4 * 31;
/// Aggregate canonical proof bytes retained in World.
pub(crate) const MAX_COMMITTED_EVIDENCE_BYTES: usize = 4 * MAX_EVIDENCE_ADMISSION_BYTES;
/// Aggregate canonical proof bytes retained only in this process.
const MAX_LOCAL_EVIDENCE_BYTES: usize = 2 * MAX_EVIDENCE_ADMISSION_BYTES;
const KEY_DOMAIN: &[u8] = b"iroha:native-consensus-evidence:v1\0";

/// An invalid proof is distinct from local original-history or preparation refusal.
#[derive(Debug, thiserror::Error)]
pub(crate) enum EvidenceAdmissionError {
    /// Deterministically invalid signed input, replay or bounded canonical state.
    #[error("native evidence: {0}")]
    Invalid(String),
    /// A source was missing, corrupt or locally refused before authenticated observation.
    #[error("native evidence source: {0}")]
    History(iroha_data_model::query::error::QueryExecutionFail),
    /// Original process capacity refused preparation.
    #[error(transparent)]
    Preparation(#[from] EvidencePreparationError),
}
impl From<super::evidence_history::NativeEvidenceError> for EvidenceAdmissionError {
    fn from(error: super::evidence_history::NativeEvidenceError) -> Self {
        use super::evidence_history::NativeEvidenceError;
        match error {
            NativeEvidenceError::History(error) => Self::History(error),
            other => Self::Invalid(other.to_string()),
        }
    }
}
fn invalid(reason: &str) -> EvidenceAdmissionError {
    EvidenceAdmissionError::Invalid(reason.into())
}

/// Exact canonical proof-frame length, shared by block and retained-state bounds.
pub(crate) fn evidence_encoded_len(evidence: &Evidence) -> usize {
    evidence.native_frame().len()
}
/// Overflow-safe aggregate evidence byte accounting.
pub(crate) fn checked_evidence_byte_sum(
    initial: usize,
    lengths: impl IntoIterator<Item = usize>,
    limit: usize,
) -> Option<usize> {
    if initial > limit {
        return None;
    }
    lengths.into_iter().try_fold(initial, |total, length| {
        total.checked_add(length).filter(|next| *next <= limit)
    })
}
/// Domain-separated key of the sole canonical native proof. Pair ordering belongs to its codec.
pub(crate) fn evidence_key(evidence: &Evidence) -> Hash {
    Hash::new_from_writer(|writer| {
        writer.write_all(KEY_DOMAIN)?;
        writer.write_all(evidence.native_frame())
    })
    .expect("incremental evidence hashing is infallible")
}

fn attribution_of(
    verified: &super::evidence_history::VerifiedNativeEvidence,
) -> EvidenceAttribution {
    EvidenceAttribution {
        instance: verified.instance().0,
        height: verified.height(),
        epoch: verified.epoch().epoch,
        context_id: verified.epoch().context.0,
        authority_generation: verified.authority_generation().0,
        offenders: verified
            .offenders()
            .iter()
            .map(|(signer, peer_id)| EvidenceOffender {
                signer: *signer,
                peer_id: peer_id.clone(),
            })
            .collect(),
        safety_violation: verified.safety_violation(),
    }
}

fn horizon(world: &(impl WorldReadOnly + ?Sized)) -> Option<u64> {
    world
        .sumeragi_npos_parameters()
        .map(|parameters| parameters.evidence_horizon_blocks())
        .filter(|value| *value > 0)
}
/// Terminal records remain replay fences until their signed offence horizon expires.
pub(crate) fn committed_evidence_record_is_prunable(
    world: &(impl WorldReadOnly + ?Sized),
    record: &EvidenceRecord,
    height: u64,
) -> bool {
    record.penalty_status.is_terminal()
        && horizon(world)
            .is_some_and(|horizon| height.saturating_sub(record.attribution.height) > horizon)
}
/// Borrowed count and byte-capacity observation, without copying nested proofs.
pub(crate) struct CommittedEvidenceCapacity {
    pub(crate) record_capacity_exceeded: bool,
    pub(crate) byte_capacity_exceeded: bool,
}
/// Validate retained-table capacity before allocating a penalty plan.
pub(crate) fn committed_evidence_capacity(
    world: &(impl WorldReadOnly + ?Sized),
) -> CommittedEvidenceCapacity {
    let mut count = 0usize;
    let mut bytes = Some(0);
    for (_, record) in world.consensus_evidence().iter() {
        count = count.saturating_add(1);
        if evidence_encoded_len(&record.evidence) > MAX_EVIDENCE_ADMISSION_BYTES {
            bytes = None;
            break;
        }
        bytes = bytes.and_then(|total| {
            checked_evidence_byte_sum(
                total,
                [evidence_encoded_len(&record.evidence)],
                MAX_COMMITTED_EVIDENCE_BYTES,
            )
        });
        if count > MAX_COMMITTED_EVIDENCE_RECORDS || bytes.is_none() {
            break;
        }
    }
    CommittedEvidenceCapacity {
        record_capacity_exceeded: count > MAX_COMMITTED_EVIDENCE_RECORDS,
        byte_capacity_exceeded: bytes.is_none(),
    }
}

/// Exact parent-derived terminal prune plan retains its original backing through application.
pub(crate) fn committed_evidence_prune_keys_from_state(
    state: &State,
    height: u64,
) -> Result<ChargedBuffer<Hash>, EvidencePreparationError> {
    let mut keys = ChargedBuffer::new(
        MAX_COMMITTED_EVIDENCE_RECORDS,
        state.evidence_preparation_budget(),
    )?;
    let view = state.view();
    for (key, record) in view.world().consensus_evidence().iter() {
        if committed_evidence_record_is_prunable(view.world(), record, height) {
            keys.try_push(*key)
                .map_err(|_| EvidencePreparationError::Invariant)?;
        }
    }
    keys.as_mut_slice().sort_unstable();
    Ok(keys)
}

fn shares_offender(left: &EvidenceAttribution, right: &EvidenceAttribution) -> bool {
    left.instance == right.instance
        && left.epoch == right.epoch
        && left.context_id == right.context_id
        && left.authority_generation == right.authority_generation
        && left.offenders.iter().any(|left| {
            right
                .offenders
                .iter()
                .any(|right| left.peer_id == right.peer_id)
        })
}

/// Original independently verified attribution for exactly one proposed evidence frame.
/// Private fields prevent raw proof or snapshot decoding from manufacturing admission.
pub(crate) struct AdmittedEvidence {
    key: Hash,
    attribution: EvidenceAttribution,
}
impl AdmittedEvidence {
    pub(crate) fn key(&self) -> Hash {
        self.key
    }
    pub(crate) fn attribution(&self) -> &EvidenceAttribution {
        &self.attribution
    }
}

/// Authenticate every incoming proof against the same immutable parent before any State writer.
/// Ordering, aggregate limits, prior-height/horizon, repeated proofs and signer replay are exact.
pub(crate) fn validate_admissions(
    view: &StateView<'_>,
    carrier_height: u64,
    admissions: &[Evidence],
) -> Result<Vec<AdmittedEvidence>, EvidenceAdmissionError> {
    if admissions.len() > MAX_EVIDENCE_ADMISSIONS_PER_BLOCK {
        return Err(invalid("too many evidence admissions"));
    }
    let incoming = checked_evidence_byte_sum(
        0,
        admissions.iter().map(evidence_encoded_len),
        MAX_EVIDENCE_ADMISSION_BYTES,
    )
    .ok_or_else(|| invalid("evidence admission bytes exceed the block limit"))?;
    let capacity = committed_evidence_capacity(view.world());
    if capacity.record_capacity_exceeded || capacity.byte_capacity_exceeded {
        return Err(invalid("retained evidence exceeds its canonical capacity"));
    }
    let mut retained_count = 0usize;
    let mut retained_bytes = 0usize;
    for (_, record) in view.world().consensus_evidence().iter() {
        if !committed_evidence_record_is_prunable(view.world(), record, carrier_height) {
            retained_count += 1;
            retained_bytes += evidence_encoded_len(&record.evidence);
        }
    }
    if retained_count + admissions.len() > MAX_COMMITTED_EVIDENCE_RECORDS
        || checked_evidence_byte_sum(retained_bytes, [incoming], MAX_COMMITTED_EVIDENCE_BYTES)
            .is_none()
    {
        return Err(invalid("retained evidence has no reclaimable capacity"));
    }
    if admissions.is_empty() {
        return Ok(Vec::new());
    }
    let horizon = horizon(view.world())
        .ok_or_else(|| invalid("evidence requires a signed positive NPoS horizon"))?;
    let mut admitted: Vec<AdmittedEvidence> = Vec::with_capacity(admissions.len());
    let mut previous = None;
    for evidence in admissions {
        let native = evidence
            .decode_native()
            .map_err(|error| EvidenceAdmissionError::Invalid(error.to_string()))?;
        let (_, subject_height, _) = super::evidence_history::subject(&native);
        if subject_height >= carrier_height || carrier_height - subject_height > horizon {
            return Err(invalid(
                "offence is not prior to its carrier within the signed horizon",
            ));
        }
        let key = evidence_key(evidence);
        if previous.is_some_and(|previous| previous >= key) {
            return Err(invalid("evidence keys must be strictly increasing"));
        }
        if view.world().consensus_evidence().get(&key).is_some() {
            return Err(invalid("evidence is already committed"));
        }
        // TODO(S8 release blocker): retain the complete history/decoded-proof working graph
        // in the original preparation pool. This functional admission is not resource qualified.
        let verified = super::evidence_history::verify_from_state(view, &native, |_, _| Ok(()))?;
        let attribution = attribution_of(&verified);
        if view.world().consensus_evidence().iter().any(|(_, record)| {
            !committed_evidence_record_is_prunable(view.world(), record, carrier_height)
                && shares_offender(&record.attribution, &attribution)
        }) || admitted
            .iter()
            .any(|record| shares_offender(&record.attribution, &attribution))
        {
            return Err(invalid(
                "a retained report already accounts for an original signer in this epoch",
            ));
        }
        admitted.push(AdmittedEvidence { key, attribution });
        previous = Some(key);
    }
    Ok(admitted)
}

/// Same-block admission can never authorize mandatory slashing.
pub(crate) fn validate_admission_penalty_separation(
    admissions: &[AdmittedEvidence],
    actions: &[NposPenaltyAction],
) -> Result<(), EvidenceAdmissionError> {
    if actions.iter().any(|action| {
        let key = match action {
            NposPenaltyAction::ConsensusSlash(action) => action.evidence_key,
            NposPenaltyAction::MarkConsensusEvidenceApplied(action) => action.evidence_key,
        };
        admissions.iter().any(|admission| admission.key == key)
    }) {
        return Err(invalid("same-block evidence cannot authorize a penalty"));
    }
    Ok(())
}

/// Reauthenticate restored records against actual native execution, including their attribution.
/// A snapshot's key, claimed signer list or decoded context is never authority.
pub(crate) fn validate_persisted_records(
    view: &StateView<'_>,
) -> Result<(), EvidenceAdmissionError> {
    let capacity = committed_evidence_capacity(view.world());
    if capacity.record_capacity_exceeded || capacity.byte_capacity_exceeded {
        return Err(invalid("restored evidence exceeds canonical capacity"));
    }
    if view.world().consensus_evidence().iter().next().is_none() {
        return Ok(());
    }
    let parameters = view
        .world()
        .sumeragi_npos_parameters()
        .ok_or_else(|| invalid("restored evidence requires signed NPoS parameters"))?;
    let committed_height =
        u64::try_from(view.height()).map_err(|_| invalid("restored height overflows"))?;
    for (key, record) in view.world().consensus_evidence().iter() {
        if *key != evidence_key(&record.evidence) {
            return Err(invalid("restored proof key differs"));
        }
        let native = record
            .evidence
            .decode_native()
            .map_err(|error| EvidenceAdmissionError::Invalid(error.to_string()))?;
        let verified = super::evidence_history::verify_from_state(view, &native, |_, _| Ok(()))?;
        if attribution_of(&verified) != record.attribution {
            return Err(invalid(
                "restored attribution differs from original native history",
            ));
        }
        if record.attribution.height >= record.recorded_at_height
            || record.recorded_at_height > committed_height
            || record.recorded_at_height - record.attribution.height
                > parameters.evidence_horizon_blocks()
        {
            return Err(invalid(
                "restored evidence was not admitted within its original horizon",
            ));
        }
        let carrier_height = usize::try_from(record.recorded_at_height)
            .ok()
            .and_then(std::num::NonZeroUsize::new)
            .ok_or_else(|| invalid("restored carrier height overflows"))?;
        let carrier = view
            .canonical_history()
            .executed_receipt(carrier_height, |_, _| Ok(()))
            .map_err(EvidenceAdmissionError::History)?;
        let header = carrier.block().header();
        if header.view_change_index() != record.recorded_at_view
            || header.creation_time_ms != record.recorded_at_ms
            || !carrier
                .block()
                .npos_consensus_effects()
                .is_some_and(|effects| {
                    effects
                        .evidence_admissions
                        .iter()
                        .any(|proof| proof == &record.evidence)
                })
        {
            return Err(invalid(
                "restored evidence was not included by its original recorded carrier",
            ));
        }
        let due = record
            .recorded_at_height
            .checked_add(parameters.slashing_delay_blocks())
            .ok_or_else(|| invalid("restored penalty height overflows"))?;
        let lifecycle_valid = match record.penalty_status {
            EvidencePenaltyStatus::Pending => committed_height < due,
            EvidencePenaltyStatus::Applied { height } => {
                if height != due || height > committed_height {
                    false
                } else {
                    let position = usize::try_from(height)
                        .ok()
                        .and_then(std::num::NonZeroUsize::new)
                        .ok_or_else(|| invalid("restored penalty carrier height overflows"))?;
                    let carrier = view
                        .canonical_history()
                        .executed_receipt(position, |_, _| Ok(()))
                        .map_err(EvidenceAdmissionError::History)?;
                    carrier.block().npos_consensus_effects().is_some_and(|effects|
                        effects.penalty_actions.iter().any(|action| matches!(action,
                            NposPenaltyAction::MarkConsensusEvidenceApplied(mark) if mark.evidence_key == *key && mark.height == height)))
                }
            }
            EvidencePenaltyStatus::Cancelled { .. } => {
                // TODO(S7/S8 release blocker): authenticate cancellation through original
                // execution replay. A snapshot's plausible cancellation height is not authority.
                return Err(invalid(
                    "cancelled evidence restore requires original execution replay",
                ));
            }
        };
        if !lifecycle_valid {
            return Err(invalid("restored penalty lifecycle is impossible"));
        }
        for (other_key, other) in view.world().consensus_evidence().iter() {
            if other_key < key && shares_offender(&record.attribution, &other.attribution) {
                return Err(invalid(
                    "restored evidence duplicates original signer accountability",
                ));
            }
        }
    }
    Ok(())
}

/// One original charged canonical observation, without decoded signer authority.
struct LocalEvidence {
    key: Hash,
    subject_height: u64,
    frame: ChargedBuffer<u8>,
}
/// Finite flat local pool. Both descriptor backing and canonical frames retain original charges.
#[derive(Default)]
pub(crate) struct NativeEvidencePool {
    entries: Option<ChargedBuffer<LocalEvidence>>,
    bytes: usize,
}
impl NativeEvidencePool {
    fn retain(
        &mut self,
        evidence: &Evidence,
        subject_height: u64,
        budget: &AllocationBudget,
    ) -> Result<bool, EvidenceAdmissionError> {
        let key = evidence_key(evidence);
        if self
            .entries
            .as_ref()
            .is_some_and(|entries| entries.as_slice().iter().any(|entry| entry.key == key))
        {
            return Ok(false);
        }
        let Some(bytes) = checked_evidence_byte_sum(
            self.bytes,
            [evidence_encoded_len(evidence)],
            MAX_LOCAL_EVIDENCE_BYTES,
        ) else {
            return Ok(false);
        };
        if self
            .entries
            .as_ref()
            .is_some_and(|entries| entries.as_slice().len() == MAX_COMMITTED_EVIDENCE_RECORDS)
        {
            return Ok(false);
        }
        if self.entries.is_none() {
            self.entries = Some(
                ChargedBuffer::new(MAX_COMMITTED_EVIDENCE_RECORDS, budget)
                    .map_err(EvidencePreparationError::from)?,
            );
        }
        let mut frame = ChargedBuffer::new(evidence_encoded_len(evidence), budget)
            .map_err(EvidencePreparationError::from)?;
        frame
            .append(evidence.native_frame())
            .map_err(|_| EvidencePreparationError::Invariant)?;
        self.entries
            .as_mut()
            .expect("original descriptor backing exists")
            .try_push(LocalEvidence {
                key,
                subject_height,
                frame,
            })
            .map_err(|_| EvidencePreparationError::Invariant)?;
        self.entries
            .as_mut()
            .unwrap()
            .as_mut_slice()
            .sort_unstable_by_key(|entry| entry.key);
        self.bytes = bytes;
        Ok(true)
    }
    fn prune(&mut self, committed: &impl WorldReadOnly, height: u64) {
        let Some(entries) = &mut self.entries else {
            return;
        };
        let horizon = horizon(committed);
        let mut index = 0;
        while index < entries.as_slice().len() {
            let entry = &entries.as_slice()[index];
            if committed.consensus_evidence().get(&entry.key).is_some()
                || horizon
                    .is_some_and(|horizon| height.saturating_sub(entry.subject_height) > horizon)
            {
                let last = entries.as_slice().len() - 1;
                entries.as_mut_slice().swap(index, last);
                let removed = entries.pop().expect("the selected entry remains owned");
                self.bytes -= removed.frame.as_slice().len();
                drop(removed);
            } else {
                index += 1;
            }
        }
        entries
            .as_mut_slice()
            .sort_unstable_by_key(|entry| entry.key);
    }
}

/// Independently validate a reducer observation and retain only its bounded canonical bytes.
/// Finalized World and stake remain unchanged. The actual candidate reauthenticates the proof.
pub(crate) fn observe(
    state: &State,
    native: &NativeEvidence,
) -> Result<bool, EvidenceAdmissionError> {
    let evidence = Evidence::from_native(native)
        .map_err(|error| EvidenceAdmissionError::Invalid(error.to_string()))?;
    let generation = state.state_view_generation();
    let view = state.view();
    let Some(horizon) = horizon(view.world()) else {
        return Ok(false);
    };
    let height =
        u64::try_from(view.height()).map_err(|_| invalid("local history height overflows"))?;
    let (_, subject_height, _) = super::evidence_history::subject(native);
    if height.saturating_sub(subject_height) > horizon {
        return Ok(false);
    }
    super::evidence_history::verify_from_state(&view, native, |_, _| Ok(()))?;
    if generation != state.state_view_generation() || generation % 2 != 0 {
        return Ok(false);
    }
    let mut pending = state.native_pending_evidence.lock();
    pending.prune(view.world(), height);
    pending.retain(
        &evidence,
        subject_height,
        state.evidence_preparation_budget(),
    )
}

/// Proposer-only bounded selection. Follower verification never consults this private pool.
pub(crate) fn pending_evidence_admissions_from_view(
    state: &State,
    height: u64,
    view: &StateView<'_>,
) -> Vec<Evidence> {
    let mut pending = state.native_pending_evidence.lock();
    pending.prune(view.world(), height);
    let mut selected = Vec::new();
    if let Some(entries) = &pending.entries {
        for entry in entries.as_slice() {
            if selected.len() == MAX_EVIDENCE_ADMISSIONS_PER_BLOCK {
                break;
            }
            if entry.subject_height >= height {
                continue;
            }
            selected.push(Evidence {
                native: entry.frame.as_slice().to_vec(),
            });
            if validate_admissions(view, height, &selected).is_err() {
                selected.pop();
            }
        }
    }
    selected
}

/// Finality controls independently prepared from one pristine State generation.
/// This token has no decoder or public constructor; application consumes it once.
pub(crate) struct PreparedStakingEffects<'state> {
    state: &'state State,
    generation: u64,
    header: BlockHeader,
    tip: Option<NativeExecutionTip>,
    effects_hash: Option<HashOf<iroha_data_model::consensus::NposConsensusEffects>>,
    admissions: Vec<AdmittedEvidence>,
    prune: ChargedBuffer<Hash>,
    stake_index: Option<crate::smartcontracts::isi::staking::PublicLaneStakeIndex>,
}
impl PreparedStakingEffects<'_> {
    /// Rejoin the exact State, header and original publication before any execution effect.
    pub(crate) fn apply(
        self,
        state: &mut crate::state::StateBlock<'_>,
        block: &iroha_data_model::block::SignedBlock,
        original: &State,
        generation: u64,
    ) -> eyre::Result<()> {
        if !std::ptr::eq(self.state, original)
            || self.generation != generation
            || original.state_view_generation() != generation
            || generation % 2 != 0
            || self.header != block.header()
            || self.tip != state.native_execution_tip()
            || self.effects_hash != block.npos_consensus_effects().map(HashOf::new)
        {
            return Err(eyre::eyre!(
                "staking effects lost their original pristine native source"
            ));
        }
        if let Some(effects) = block.npos_consensus_effects() {
            state.apply_pristine_npos_consensus_effects(
                effects,
                self.stake_index.as_ref(),
                self.prune.as_slice(),
                &self.admissions,
                block.header().height().get(),
                block.header().view_change_index(),
                block.header().creation_time_ms,
            )?;
        } else if !self.admissions.is_empty() || !self.prune.as_slice().is_empty() {
            return Err(eyre::eyre!(
                "absent staking effects cannot consume prepared records"
            ));
        }
        Ok(())
    }
}

/// Check candidate evidence and exact deterministic parent penalties before writers are acquired.
pub(crate) fn prepare<'state>(
    state: &'state State,
    block: &iroha_data_model::block::SignedBlock,
    generation: u64,
) -> Result<PreparedStakingEffects<'state>, crate::block::BlockValidationError> {
    use crate::block::BlockValidationError;
    let header = block.header();
    let view = state.view();
    let no_effects = iroha_data_model::consensus::NposConsensusEffects::default();
    let effects = block.npos_consensus_effects().unwrap_or(&no_effects);
    let admissions =
        validate_admissions(&view, header.height().get(), &effects.evidence_admissions)
            .map_err(classify)?;
    validate_admission_penalty_separation(&admissions, &effects.penalty_actions)
        .map_err(classify)?;
    let npos = view.world().sumeragi_npos_parameters().is_some();
    let tip = view.native_execution_tip();
    drop(view);
    let (actions, index) = if npos {
        let (actions, index) = super::penalties::PenaltyApplier::new(state, None)
            .derive_npos_penalty_actions(&header)
            .map_err(|error| {
                BlockValidationError::from_npos_application_error(
                    error,
                    "native penalty preparation",
                )
            })?;
        let needs_index = actions
            .iter()
            .any(|action| matches!(action, NposPenaltyAction::ConsensusSlash(_)));
        (actions, needs_index.then_some(index))
    } else {
        (Vec::new(), None)
    };
    if actions != effects.penalty_actions {
        return Err(BlockValidationError::NposEffectsInvalid(
            "penalties differ from exact committed parent derivation".into(),
        ));
    }
    let prune = if block.npos_consensus_effects().is_some() {
        committed_evidence_prune_keys_from_state(state, header.height().get())
            .map_err(BlockValidationError::EvidencePreparation)?
    } else {
        ChargedBuffer::new(0, state.evidence_preparation_budget())
            .map_err(EvidencePreparationError::from)
            .map_err(BlockValidationError::EvidencePreparation)?
    };
    if state.state_view_generation() != generation || generation % 2 != 0 {
        return Err(BlockValidationError::LocalStorageRecoveryRequired {
            reason: "native evidence parent changed during pristine preparation".into(),
        });
    }
    Ok(PreparedStakingEffects {
        state,
        generation,
        header,
        tip,
        effects_hash: block.npos_consensus_effects().map(HashOf::new),
        admissions,
        prune,
        stake_index: index,
    })
}
fn classify(error: EvidenceAdmissionError) -> crate::block::BlockValidationError {
    use crate::block::BlockValidationError;
    match error {
        EvidenceAdmissionError::Preparation(error) => {
            BlockValidationError::EvidencePreparation(error)
        }
        EvidenceAdmissionError::History(error) => {
            BlockValidationError::LocalStorageRecoveryRequired {
                reason: error.to_string(),
            }
        }
        EvidenceAdmissionError::Invalid(error) => BlockValidationError::NposEffectsInvalid(error),
    }
}

#[cfg(test)]
mod tests;
