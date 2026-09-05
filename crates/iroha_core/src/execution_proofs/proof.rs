//! Native RaceV1 proving, cryptographic verification, and ledger-history binding.
//!
//! The verifier reconstructs public controls and fixed columns, but never replays vehicle physics.
//! Transition correctness is checked by the degree-four integer AIR and the existing native
//! Goldilocks/Fp4 DEEP-ALI/FRI proof driver. Payout admission additionally binds authenticated
//! checkpoints and every consensus-selected forced control and DNF event.

use super::{
    integer_air::field,
    race::{initial_race_state_v1, race_result_v1, replay_race_v1},
    race_air::{FIXED_PER_CAR, FIXED_PREFIX, RaceAirV1, car_values},
};
use crate::privacy_engines::{
    aggregate_stark::{AggregateStarkDomainsV1, AggregateStarkParametersV1},
    proof_managed_note_stark::{
        NOTE_COPY_AUX_WIDTH_V1, NOTE_COPY_FIXED_WIDTH_V1, NOTE_COPY_WIDTH_V1, NoteCopyCellPolicyV1,
        NoteCopyChallengesV1, NoteCopyScheduleV1, PROOF_MANAGED_NOTE_STARK_GEOMETRY_DESCRIPTOR_V1,
        ProofManagedNoteStarkAdapterV1, ProofManagedNoteStarkErrorV1,
        ProofManagedNoteStarkProtocolV1, prove_proof_managed_note_stark_v1,
        verify_proof_managed_note_stark_v1,
    },
    transparent_stark::{
        GoldilocksDigest384V1, GoldilocksFieldV1 as F, TransparentStarkDigestContextV1,
        TransparentTranscriptV1, goldilocks_digest384_frame_v1,
    },
};
use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    execution_proofs::*,
    race::{RaceCheckpointV1, RaceForcedBatchV1, RaceParticipantV1, race_message_hash_v1},
};
use norito::codec::{Decode, Encode};
use thiserror::Error;

/// Absolute bounded wire admission ceiling; deployment qualification additionally measures transport.
pub const RACE_MAX_PROOF_BYTES_V1: usize = 32 * 1024 * 1024;
const TRACE_LOG2: u8 = 13;
const TRACE_SIZE: usize = 1 << TRACE_LOG2;
const MAX_AIR_COLUMNS: usize = 12_000;
const RULES:&[u8]=b"iroha-race-rules-v1:ticks=30:max=5400:players=1..8:multiplayer=2..8:laps=3:batch=6:skins=6:grid-progress=-floor(slot/2)*4000:grid-x=even?-1800:1800:grid-speed=0:grid-vx=0:grid-energy=1000:controls=throttle,brake,left,right,drift,boost:boost=bit5&&energy>=25:energy=boost?-25:min(1000,+4):top=boost?3000:2400:accel=brake?-100:throttle?40:-12:speed=clamp(speed+accel,0,top):steer=right-left:vx=clamp(trunc((vx+steer*(drift?28:18))*7/8),-320,320):curve-cell=floor(remEuclid(oldProgress,length)*12/length):force=trunc(curve*speed/120):x=clamp(x+vx+force,-9000,9000):abs(x)>6000=>speed=max(0,speed-90):progress+=speed:contacts=ascending-i-j,abs(dp)<3600&&abs(dx)<1800,push=ceil((1800-abs(dx))/2),low-x-or-low-slot-tie-goes-left,speed=max(0,speed-120):finish=after-contacts,progress>=3*length,clamp-and-freeze:dnf=before-exact-tick,unfinished-only,freeze-and-ghost:finished-ghost:terminal=(tick%6==0&&all-finished-or-dnf)||tick5400:winners=all-earliest-finish:tracks=NeonTokyo/2000000/[0,1,2,1,0,-1,-2,-1,0,2,-2,0];Harbor/2400000/[0,-2,-2,0,1,3,1,0,-1,-3,-1,0];Sakura/1800000/[0,1,1,0,-2,-1,0,2,3,1,-2,0]";
const PROFILE:&[u8]=b"iroha-native-execution-race-v1:wire=RCE1/1:public-replay:trace=8192:integer-air-degree=4:shared-degree=2:proof-max=33554432:base-max=12008:aux=118:state=progress,x,speed,vx,energy,finish-tick,dnf-tick-plus-one:all-car-dynamics:all-ordered-contacts:initial-intermediate-final-boundaries";
const CONTEXT: TransparentStarkDigestContextV1 =
    TransparentStarkDigestContextV1::execution_v1(b"race-v1");
const DOMAINS: AggregateStarkDomainsV1 = AggregateStarkDomainsV1 {
    digest_context: CONTEXT,
    base_leaf: b"execution-race-base-leaf-v1",
    base_node: b"execution-race-base-node-v1",
    aux_leaf: b"execution-race-aux-leaf-v1",
    aux_node: b"execution-race-aux-node-v1",
    composition_leaf: b"execution-race-composition-leaf-v1",
    composition_node: b"execution-race-composition-node-v1",
    fri_leaf: b"execution-race-fri-leaf-v1",
    fri_node: b"execution-race-fri-node-v1",
    layout_label: b"execution-race-layout-v1",
    base_root_label: b"execution-race-base-root-v1",
    aux_root_label: b"execution-race-aux-root-v1",
    composition_root_label: b"execution-race-composition-root-v1",
    fri_root_label: b"execution-race-fri-root-v1",
    fri_beta_label: b"execution-race-fri-beta-v1",
    query_seed: b"execution-race-query-seed-v1",
};

/// Fail-closed release gate. A compiled profile is not funded until cryptographic and resource
/// qualification evidence is reviewed; development prover/verifier APIs remain directly testable.
#[must_use]
pub const fn race_profile_is_qualified_v1() -> bool {
    false
}
/// Reconstruct the sole compiled descriptor; callers cannot supply alternative circuits or bounds.
#[must_use]
pub fn compiled_race_profile_v1() -> ExecutionProofProfileV1 {
    ExecutionProofProfileV1 {
        version: 1,
        profile_id: race_profile_id_v1(),
        rules_hash: race_rules_hash_v1(),
        relation: ExecutionProofRelationV1::RaceV1,
        target_soundness_bits: 128,
        maximum_proof_bytes: RACE_MAX_PROOF_BYTES_V1 as u32,
        qualified: race_profile_is_qualified_v1(),
    }
}
/// Commitment to every immutable gameplay parameter and operation order.
#[must_use]
pub fn race_rules_hash_v1() -> Hash {
    Hash::new(RULES)
}
/// Commitment to native proof geometry and the complete compiled integer relation source.
#[must_use]
pub fn race_profile_id_v1() -> Hash {
    static ID: std::sync::OnceLock<Hash> = std::sync::OnceLock::new();
    *ID.get_or_init(|| {
        Hash::new_from_chunks(&[
            b"iroha:execution:profile:v1\0",
            PROFILE,
            RULES,
            PROOF_MANAGED_NOTE_STARK_GEOMETRY_DESCRIPTOR_V1,
            include_bytes!("integer_air.rs"),
            include_bytes!("race_air.rs"),
            include_bytes!("proof.rs"),
            include_bytes!("../privacy_engines/transparent_stark.rs"),
            include_bytes!("../privacy_engines/aggregate_stark.rs"),
            include_bytes!("../privacy_engines/proof_managed_note_stark.rs"),
        ])
    })
}
/// Exact canonical public replay commitment used by peer and ledger certificates.
#[must_use]
pub fn race_transcript_root_v1(network: &NetworkId, replay: &RaceReplayV1) -> Hash {
    race_message_hash_v1(network, "input-transcript", replay)
}
/// Exact canonical state commitment used by peer and ledger certificates.
#[must_use]
pub fn race_state_root_v1(network: &NetworkId, state: &RaceStateV1) -> Hash {
    race_message_hash_v1(network, "simulation-state", state)
}

/// A proof fails closed at its exact validation layer.
#[derive(Debug, Error)]
pub enum ExecutionProofErrorV1 {
    /// Wrong envelope, resource limit, or canonical payload.
    #[error("invalid or oversized native execution proof envelope")]
    Envelope,
    /// Public claim differs from the exact compiled profile or replay.
    #[error("native race public statement mismatch")]
    Statement,
    /// Transcript or terminal-state structure is invalid.
    #[error("invalid native race replay structure")]
    Replay,
    /// The replay does not extend the exact authenticated consensus history.
    #[error("native race proof does not bind the retained chain history")]
    History,
    /// Native cryptographic proof verification failed.
    #[error("native race STARK failed: {0}")]
    Cryptography(String),
}
impl From<ProofManagedNoteStarkErrorV1> for ExecutionProofErrorV1 {
    fn from(error: ProofManagedNoteStarkErrorV1) -> Self {
        Self::Cryptography(error.to_string())
    }
}

fn validate_payload(
    statement: &RacePublicInputsV1,
    payload: &RaceProofPayloadV1,
) -> Result<(), ExecutionProofErrorV1> {
    let replay = &payload.replay;
    let final_state = &payload.final_state;
    if !(1..=8).contains(&replay.player_count)
        || replay.frames.len() > RACE_MAX_TICKS_V1 as usize
        || replay.frames.iter().enumerate().any(|(tick, frame)| {
            frame.tick != tick as u32
                || frame.controls.len() != usize::from(replay.player_count)
                || frame
                    .controls
                    .iter()
                    .any(|control| control & !RACE_CONTROL_MASK_V1 != 0)
        })
        || replay
            .dnf_events
            .windows(2)
            .any(|events| events[0].tick >= events[1].tick)
        || replay.dnf_events.iter().any(|event| {
            event.tick > replay.frames.len() as u32
                || event.slots.is_empty()
                || event.slots.windows(2).any(|slots| slots[0] >= slots[1])
                || event.slots.iter().any(|slot| *slot >= replay.player_count)
        })
        || final_state.tick % 6 != 0
        || final_state.cars.len() != usize::from(replay.player_count)
        || final_state.track != replay.track
        || final_state.tick != replay.frames.len() as u32
        || (final_state.tick != RACE_MAX_TICKS_V1
            && !final_state
                .cars
                .iter()
                .all(|car| car.finish_tick.is_some() || car.dnf_tick.is_some()))
    {
        return Err(ExecutionProofErrorV1::Replay);
    }
    let mut removed = [false; 8];
    for event in &replay.dnf_events {
        for slot in &event.slots {
            if std::mem::replace(&mut removed[usize::from(*slot)], true) {
                return Err(ExecutionProofErrorV1::Replay);
            }
        }
    }
    if statement.rules_hash != race_rules_hash_v1()
        || statement.track != replay.track
        || statement.transcript_root != race_transcript_root_v1(&statement.network_id, replay)
        || statement.result
            != race_result_v1(final_state).map_err(|_| ExecutionProofErrorV1::Replay)?
    {
        return Err(ExecutionProofErrorV1::Statement);
    }
    if let Some(checkpoint) = &payload.checkpoint_state {
        if checkpoint.tick > final_state.tick
            || checkpoint.track != replay.track
            || checkpoint.cars.len() != final_state.cars.len()
        {
            return Err(ExecutionProofErrorV1::Replay);
        }
        race_result_v1(checkpoint).map_err(|_| ExecutionProofErrorV1::Replay)?;
    }
    Ok(())
}

struct RaceAdapterV1<'a> {
    statement: &'a RacePublicInputsV1,
    payload: &'a RaceProofPayloadV1,
    compiled: RaceAirV1,
}
impl<'a> RaceAdapterV1<'a> {
    fn new(
        statement: &'a RacePublicInputsV1,
        payload: &'a RaceProofPayloadV1,
    ) -> Result<Self, ExecutionProofErrorV1> {
        validate_payload(statement, payload)?;
        let compiled = RaceAirV1::compile(
            &payload.replay,
            &payload.final_state,
            payload.checkpoint_state.as_ref(),
        );
        if compiled.air.width() > MAX_AIR_COLUMNS {
            return Err(ExecutionProofErrorV1::Envelope);
        }
        Ok(Self {
            statement,
            payload,
            compiled,
        })
    }
}
impl ProofManagedNoteStarkAdapterV1 for RaceAdapterV1<'_> {
    type ProfileChallenges = ();
    fn protocol_v1(&self) -> ProofManagedNoteStarkProtocolV1 {
        ProofManagedNoteStarkProtocolV1 {
            parameters: AggregateStarkParametersV1 {
                proof_magic: *b"RCE1",
                proof_version: 1,
                security_lanes: 1,
                query_count: 136,
                blowup_log2: 3,
                terminal_log2: 10,
                terminal_degree_bound: 143,
                composition_degree_chunks: 4,
                minimum_trace_log2: TRACE_LOG2,
                maximum_trace_log2: TRACE_LOG2,
                maximum_trace_groups: 1,
                maximum_segment_instances: 1,
                maximum_base_columns_per_instance: MAX_AIR_COLUMNS + NOTE_COPY_WIDTH_V1,
                maximum_aux_columns_per_instance: NOTE_COPY_AUX_WIDTH_V1,
                maximum_proof_bytes: RACE_MAX_PROOF_BYTES_V1,
            },
            domains: DOMAINS,
            maximum_constraint_degree: 4,
            profile_binding_label: b"execution-race-profile-binding-v1",
            profile_descriptor: PROFILE,
            relation_layout_domain: b"execution-race-relation-layout-v1",
        }
    }
    fn public_input_digest_v1(
        &self,
    ) -> Result<GoldilocksDigest384V1, ProofManagedNoteStarkErrorV1> {
        goldilocks_digest384_frame_v1(
            CONTEXT,
            b"execution-race-public-statement-v1",
            b"statement",
            0,
            0,
            0,
            &[
                race_profile_id_v1().as_ref(),
                &self.statement.encode(),
                &self.payload.replay.encode(),
                &self.payload.final_state.encode(),
                &self.payload.checkpoint_state.encode(),
            ],
        )
        .map_err(|_| ProofManagedNoteStarkErrorV1::InvalidProfile)
    }
    fn trace_log2_v1(&self) -> u8 {
        TRACE_LOG2
    }
    fn base_width_v1(&self) -> usize {
        NOTE_COPY_WIDTH_V1 + self.compiled.air.width()
    }
    fn profile_aux_width_v1(&self) -> usize {
        0
    }
    fn profile_fixed_width_v1(&self) -> usize {
        FIXED_PREFIX + usize::from(self.payload.replay.player_count) * FIXED_PER_CAR
    }
    fn profile_constraint_count_v1(&self) -> usize {
        self.compiled.air.constraint_count()
    }
    fn copy_schedule_v1(&self) -> Result<NoteCopyScheduleV1, ProofManagedNoteStarkErrorV1> {
        Ok(NoteCopyScheduleV1 {
            policies: vec![[NoteCopyCellPolicyV1::Inactive; NOTE_COPY_WIDTH_V1]; TRACE_SIZE],
            sigma: (0..TRACE_SIZE)
                .map(|row| {
                    std::array::from_fn(|column| (row * NOTE_COPY_WIDTH_V1 + column + 1) as u32)
                })
                .collect(),
        })
    }
    fn profile_fixed_columns_v1(&self) -> Result<Vec<Vec<F>>, ProofManagedNoteStarkErrorV1> {
        let mut columns = vec![Vec::with_capacity(TRACE_SIZE); self.profile_fixed_width_v1()];
        for row in 0..TRACE_SIZE {
            for (column, value) in columns.iter_mut().zip(RaceAirV1::fixed_row(
                &self.payload.replay,
                row,
                TRACE_SIZE,
                self.payload
                    .checkpoint_state
                    .as_ref()
                    .map(|state| state.tick),
            )) {
                column.push(field(value));
            }
        }
        Ok(columns)
    }
    fn derive_profile_challenges_v1(
        &self,
        _: &mut TransparentTranscriptV1,
        _: NoteCopyChallengesV1,
    ) -> Result<(), ProofManagedNoteStarkErrorV1> {
        Ok(())
    }
    fn build_profile_aux_columns_v1(
        &self,
        _: &[Vec<F>],
        _: &[Vec<F>],
        _: &[Vec<F>],
        _: NoteCopyChallengesV1,
        _: &(),
    ) -> Result<Vec<Vec<F>>, ProofManagedNoteStarkErrorV1> {
        Ok(vec![])
    }
    fn profile_constraint_residues_v1(
        &self,
        current: &[F],
        next: &[F],
        _: &[F],
        _: &[F],
        fixed: &[F],
        _: NoteCopyChallengesV1,
        _: &(),
    ) -> Result<Vec<F>, ProofManagedNoteStarkErrorV1> {
        Ok(self.compiled.air.residues(
            &current[NOTE_COPY_WIDTH_V1..],
            &next[NOTE_COPY_WIDTH_V1..],
            &fixed[NOTE_COPY_FIXED_WIDTH_V1..],
        ))
    }
}

/// Generate and self-verify a genuine native transparent execution proof using local entropy.
pub fn prove_race_v1(
    request: RaceProverRequestV1,
) -> Result<ExecutionProofEnvelopeV1, ExecutionProofErrorV1> {
    let final_state = replay_race_v1(&request.replay).map_err(|_| ExecutionProofErrorV1::Replay)?;
    let mut payload = RaceProofPayloadV1 {
        replay: request.replay,
        final_state,
        checkpoint_state: request.checkpoint_state,
        stark_bytes: vec![],
    };
    let adapter = RaceAdapterV1::new(&request.statement, &payload)?;
    let grid = initial_race_state_v1(payload.replay.track, payload.replay.player_count)
        .map_err(|_| ExecutionProofErrorV1::Replay)?;
    let mut inputs = grid.cars.iter().flat_map(car_values).collect::<Vec<_>>();
    let mut columns = vec![Vec::with_capacity(TRACE_SIZE); adapter.base_width_v1()];
    for row_index in 0..TRACE_SIZE {
        let fixed = RaceAirV1::fixed_row(
            &payload.replay,
            row_index,
            TRACE_SIZE,
            payload.checkpoint_state.as_ref().map(|state| state.tick),
        );
        let row = adapter.compiled.air.witness(&inputs, &fixed);
        inputs = adapter.compiled.next_inputs(&row);
        for column in &mut columns[..NOTE_COPY_WIDTH_V1] {
            column.push(F::ZERO);
        }
        for (column, value) in columns[NOTE_COPY_WIDTH_V1..].iter_mut().zip(row) {
            column.push(field(value));
        }
    }
    payload.stark_bytes = prove_proof_managed_note_stark_v1(&adapter, &columns)?;
    let envelope = ExecutionProofEnvelopeV1 {
        version: 1,
        profile_id: race_profile_id_v1(),
        statement: request.statement,
        proof_bytes: payload.encode(),
    };
    if envelope.proof_bytes.len() > RACE_MAX_PROOF_BYTES_V1 {
        return Err(ExecutionProofErrorV1::Envelope);
    }
    verify_race_proof_v1(&envelope)?;
    Ok(envelope)
}

fn decode_payload(
    envelope: &ExecutionProofEnvelopeV1,
) -> Result<RaceProofPayloadV1, ExecutionProofErrorV1> {
    if envelope.version != 1
        || envelope.profile_id != race_profile_id_v1()
        || envelope.proof_bytes.is_empty()
        || envelope.proof_bytes.len() > RACE_MAX_PROOF_BYTES_V1
    {
        return Err(ExecutionProofErrorV1::Envelope);
    }
    let mut bytes = envelope.proof_bytes.as_slice();
    let payload =
        RaceProofPayloadV1::decode(&mut bytes).map_err(|_| ExecutionProofErrorV1::Envelope)?;
    if !bytes.is_empty() || payload.encode() != envelope.proof_bytes {
        return Err(ExecutionProofErrorV1::Envelope);
    }
    Ok(payload)
}
/// Verify the complete semantic execution relation, with no replay-only or binding-only mode.
pub fn verify_race_proof_v1(
    envelope: &ExecutionProofEnvelopeV1,
) -> Result<(), ExecutionProofErrorV1> {
    let payload = decode_payload(envelope)?;
    let adapter = RaceAdapterV1::new(&envelope.statement, &payload)?;
    verify_proof_managed_note_stark_v1(&adapter, &payload.stark_bytes)?;
    Ok(())
}

/// Verify semantic execution and exact retained checkpoint, forced inputs, and removals.
pub fn verify_race_proof_for_history_v1(
    envelope: &ExecutionProofEnvelopeV1,
    checkpoint: Option<&RaceCheckpointV1>,
    batches: &[RaceForcedBatchV1],
    participants: &[RaceParticipantV1],
    epoch: u64,
) -> Result<(), ExecutionProofErrorV1> {
    let payload = decode_payload(envelope)?;
    validate_payload(&envelope.statement, &payload)?;
    let statement = &envelope.statement;
    let replay = &payload.replay;
    if participants.len() != usize::from(replay.player_count)
        || statement.dispute_root
            != race_message_hash_v1(
                &statement.network_id,
                "dispute-history",
                &(
                    statement.race_id,
                    epoch,
                    checkpoint.cloned(),
                    batches.to_vec(),
                ),
            )
    {
        return Err(ExecutionProofErrorV1::History);
    }
    let mut expected_dnfs = std::collections::BTreeMap::<u32, Vec<u8>>::new();
    for (slot, participant) in participants.iter().enumerate() {
        if let Some(tick) = participant
            .dnf_at_tick
            .filter(|tick| *tick <= payload.final_state.tick)
        {
            expected_dnfs.entry(tick).or_default().push(slot as u8);
        }
    }
    if replay.dnf_events
        != expected_dnfs
            .into_iter()
            .map(|(tick, slots)| RaceDnfEventV1 { tick, slots })
            .collect::<Vec<_>>()
    {
        return Err(ExecutionProofErrorV1::History);
    }
    let mut tail = 0;
    match (checkpoint, payload.checkpoint_state.as_ref()) {
        (Some(checkpoint), Some(state)) => {
            if checkpoint.race_id != statement.race_id
                || checkpoint.tick > replay.frames.len() as u32
                || state.tick != checkpoint.tick
                || race_state_root_v1(&statement.network_id, state) != checkpoint.state_root
            {
                return Err(ExecutionProofErrorV1::History);
            }
            let prefix = RaceReplayV1 {
                track: replay.track,
                player_count: replay.player_count,
                frames: replay.frames[..checkpoint.tick as usize].to_vec(),
                dnf_events: replay
                    .dnf_events
                    .iter()
                    .filter(|event| event.tick < checkpoint.tick)
                    .cloned()
                    .collect(),
            };
            if race_transcript_root_v1(&statement.network_id, &prefix) != checkpoint.transcript_root
            {
                return Err(ExecutionProofErrorV1::History);
            }
            tail = checkpoint.tick;
        }
        (None, None) => {}
        _ => return Err(ExecutionProofErrorV1::History),
    }
    for batch in batches {
        if batch.start_tick < tail {
            continue;
        }
        if batch.start_tick != tail
            || batch.controls.len() != participants.len()
            || batch.controls.iter().any(|controls| controls.len() != 6)
            || batch.epoch > epoch
        {
            return Err(ExecutionProofErrorV1::History);
        }
        for offset in 0..6 {
            let tick = batch.start_tick + offset;
            if let Some(frame) = replay.frames.get(tick as usize) {
                if frame.controls
                    != batch
                        .controls
                        .iter()
                        .map(|controls| controls[offset as usize])
                        .collect::<Vec<_>>()
                {
                    return Err(ExecutionProofErrorV1::History);
                }
            } else if tick < replay.frames.len() as u32 {
                return Err(ExecutionProofErrorV1::History);
            }
        }
        for slot in &batch.dnf_slots {
            if participants
                .get(usize::from(*slot))
                .and_then(|participant| participant.dnf_at_tick)
                != Some(batch.start_tick)
            {
                return Err(ExecutionProofErrorV1::History);
            }
        }
        tail = tail.saturating_add(6);
    }
    if tail < replay.frames.len() as u32 {
        return Err(ExecutionProofErrorV1::History);
    }
    let adapter = RaceAdapterV1::new(statement, &payload)?;
    verify_proof_managed_note_stark_v1(&adapter, &payload.stark_bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(players: u8) -> RaceProverRequestV1 {
        let replay = RaceReplayV1 {
            track: RaceTrackV1::NeonTokyo,
            player_count: players,
            frames: (0..6)
                .map(|tick| RaceInputFrameV1 {
                    tick,
                    controls: (0..players)
                        .map(|slot| 1 | if slot % 2 == 0 { 8 } else { 4 })
                        .collect(),
                })
                .collect(),
            dnf_events: vec![RaceDnfEventV1 {
                tick: 6,
                slots: (0..players).collect(),
            }],
        };
        let state = replay_race_v1(&replay).expect("reference replay");
        let network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(Hash::new(
            b"race-proof-test-network",
        )));
        RaceProverRequestV1 {
            statement: RacePublicInputsV1 {
                network_id,
                race_id: Hash::new(b"race"),
                roster_hash: Hash::new(b"roster"),
                rules_hash: race_rules_hash_v1(),
                track: replay.track,
                transcript_root: race_transcript_root_v1(&network_id, &replay),
                dispute_root: Hash::new(b"history"),
                result: race_result_v1(&state).expect("result"),
            },
            replay,
            checkpoint_state: None,
        }
    }
    #[test]
    fn funding_profile_is_fail_closed_before_release_qualification() {
        assert!(!race_profile_is_qualified_v1());
    }
    #[test]
    fn profile_has_closed_soundness_geometry() {
        assert_ne!(race_profile_id_v1(), race_rules_hash_v1());
        DOMAINS.validate().expect("execution domains are unique");
        let request = request(8);
        let payload = RaceProofPayloadV1 {
            final_state: replay_race_v1(&request.replay).expect("reference"),
            replay: request.replay,
            checkpoint_state: None,
            stark_bytes: vec![],
        };
        let adapter = RaceAdapterV1::new(&request.statement, &payload).expect("compiled adapter");
        adapter
            .protocol_v1()
            .validate()
            .expect("machine-checked security geometry");
    }

    #[test]
    fn complete_relation_has_exact_degree_four_on_arbitrary_field_lines() {
        let request = request(8);
        let payload = RaceProofPayloadV1 {
            final_state: replay_race_v1(&request.replay).expect("reference"),
            replay: request.replay,
            checkpoint_state: None,
            stark_bytes: vec![],
        };
        let adapter = RaceAdapterV1::new(&request.statement, &payload).expect("compiled adapter");
        let width = adapter.compiled.air.width();
        let fixed_width = adapter.profile_fixed_width_v1();
        let degree=crate::privacy_engines::proof_managed_note_stark::degree_audit::measured_maximum_affine_degree_v1([47;32],[width,width,0,0,fixed_width],8,4,|current,next,_,_,fixed|Ok::<_,()>(adapter.compiled.air.residues(current,next,fixed)));
        assert_eq!(degree, 4);
    }

    #[test]
    #[ignore = "expensive native cryptographic qualification; run explicitly and record peak RSS/proof bytes"]
    fn real_native_proof_roundtrip_and_statement_adversaries() {
        let start = std::time::Instant::now();
        let proof = prove_race_v1(request(2)).expect("genuine native execution proof");
        eprintln!(
            "race proof bytes={} proving_seconds={:.3}",
            proof.proof_bytes.len(),
            start.elapsed().as_secs_f64()
        );
        verify_race_proof_v1(&proof).expect("verify independent envelope");
        let mut corrupted = proof.clone();
        corrupted.proof_bytes.push(0);
        assert!(verify_race_proof_v1(&corrupted).is_err());
        let mut corrupted = proof.clone();
        corrupted.statement.race_id = Hash::new(b"foreign race");
        assert!(verify_race_proof_v1(&corrupted).is_err());
        let mut payload = decode_payload(&proof).expect("payload");
        payload.replay.frames[0].controls[0] ^= 32;
        let mut corrupted = proof.clone();
        corrupted.statement.transcript_root =
            race_transcript_root_v1(&corrupted.statement.network_id, &payload.replay);
        corrupted.proof_bytes = payload.encode();
        assert!(verify_race_proof_v1(&corrupted).is_err());
        let mut payload = decode_payload(&proof).expect("payload");
        payload.final_state.cars[0].progress_mm += 1;
        let mut corrupted = proof.clone();
        corrupted.statement.result =
            race_result_v1(&payload.final_state).expect("bounded forged result");
        corrupted.proof_bytes = payload.encode();
        assert!(verify_race_proof_v1(&corrupted).is_err());
        let mut corrupted = proof.clone();
        let middle = corrupted.proof_bytes.len() / 2;
        corrupted.proof_bytes[middle] ^= 1;
        assert!(verify_race_proof_v1(&corrupted).is_err());
    }
}
