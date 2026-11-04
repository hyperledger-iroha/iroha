//! Corrected stock racing proof with eligibility-first awards and one first-release identity.
//!
//! The verifier rebuilds controls, weather and removals as public columns, but never replays vehicle
//! physics. Transition correctness is checked by the RaceV1 bounded-integer circuit
//! (`iroha_execution_stark::games::race_v1`) on the `iroha_execution_stark` engine: a Goldilocks/Fp4
//! DEEP-ALI/FRI STARK with BLAKE2b-384 commitments and transcript and no masking (replays are
//! public). Every proof and verification first checks the standard parameters against this
//! profile's 128-bit security target for the exact circuit. Payout admission additionally binds
//! authenticated checkpoints and every consensus-selected forced control and DNF event.

use super::{
    error::ExecutionProofErrorV1,
    race::{
        apply_race_dnf_v1, initial_race_state_v1, race_result_v1, replay_race_v1, step_race_v1,
    },
};
use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    execution_proofs::*,
    game::{
        GameAdmissionBodyV1, GameCheckpointV1, GameDnfEventV1, GameForcedBatchV1, GameManifestV1,
        GameOutcomeV1, GameParticipantV1, GameTranscriptAnchorV1, GameTranscriptBatchV1,
        GameTranscriptV1, game_message_hash_v1, game_roster_hash_v1,
    },
};
use iroha_execution_stark::{
    Checkpoint, Circuit, Params, Proof, Statement,
    games::race_v1 as race_circuit,
    hash::{Digest as ProfileDigest, FramedHash},
};
use norito::codec::{Decode, Encode};

/// Maximum canonical execution envelope, including this adapter's complete replay.
/// This does not raise any deployment's independent transaction or transport bounds.
pub const RACE_MAX_PROOF_BYTES_V1: usize = EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1;
/// Rounded bound on the cryptographic wire alone, excluding replay and envelope. Every RaceV1
/// circuit's computed worst case (`iroha_execution_stark::max_proof_bytes`, 2,044,445 bytes for
/// eight cars) is below it; actual eight-car proofs are about 1.66 MB.
pub const RACE_MAX_STARK_BYTES_V1: usize = 3_250_000;
const RULES:&[u8]=b"iroha-race-rules-v1:ticks=30:max=5400:players=1..8:multiplayer=2..8:laps=3:batch=6:skins=6:grid-progress=-floor(slot/2)*4000:grid-x=even?-1800:1800:grid-speed=0:grid-vx=0:grid-energy=1000:controls=throttle,brake,left,right,drift,boost:boost=bit5&&energy>=25:energy=boost?-25:min(1000,+4):top=boost?3000:2400:accel=brake?-100:throttle?40:-12:speed=clamp(speed+accel,0,top):steer=right-left:vx=clamp(trunc((vx+steer*(drift?28:18))*7/8),-320,320):curve-cell=floor(remEuclid(oldProgress,length)*12/length):force=trunc(curve*speed/120):x=clamp(x+vx+force,-9000,9000):abs(x)>6000=>speed=max(0,speed-90):progress+=speed:contacts=ascending-i-j,abs(dp)<3600&&abs(dx)<1800,push=ceil((1800-abs(dx))/2),low-x-or-low-slot-tie-goes-left,speed=max(0,speed-120):finish=after-contacts,progress>=3*length,clamp-and-freeze:dnf=before-exact-tick,record-key-inactivity-even-after-finish,preserve-earlier-finish-as-history-only,freeze-and-ghost:finished-ghost:terminal=(tick%6==0&&(all-finished-or-dnf||active-keys<2))||tick5400:ranking=eligible-before-all-dnf,then-finish-time,then-distance,slot-display-only:winners=eligible-earliest-finish-ties-else-sole-eligible-survivor-else-tick5400-max-eligible-progress-ties-else-all-forfeit-refund:environment=12-cell-center-floor((2i+1)*length/24):kinds-tree0-sign1-oil2:Tokyo-kinds=0,1,0,2,0,1,2,0,1,0,2,1:Harbor-kinds=1,2,0,1,0,2,1,0,2,1,0,2:Sakura-kinds=0,0,2,1,0,2,0,1,0,2,0,1:object-x=even-negative-odd-positive,magnitudes7400,5600,1800:rain-patterns=Tokyo0010,Harbor0110,Sakura0001:wind-pattern=0,1,2,1,0,-1,-2,-1:wind-strengths=Tokyo4,Harbor16,Sakura8:tree-radius=1400:tree-loss=600:tree-push=1800-clamp9000:sign-radius=1300:sign-loss=260:oil-half-length=9000:oil-half-width=1400:oil-steer=trunc(steer/2):rain-steer=trunc(steer*3/4):slippery-damping=15/16:rain-vx-limit=280:wind=trunc(public90tick8table*speed/2400):rain=public300tick4table:impact=swept-progress-before-ordered-contacts:tracks=NeonTokyo/2000000/[0,1,2,1,0,-1,-2,-1,0,2,-2,0];Harbor/2400000/[0,-2,-2,0,1,3,1,0,-1,-3,-1,0];Sakura/1800000/[0,1,1,0,-2,-1,0,2,3,1,-2,0]";
const PROFILE:&[u8]=b"iroha-native-execution-race-v1:wire=IEB1/1:engine=iroha_execution_stark:bounded-integer-air:goldilocks-fp4-deep-ali-fri:blake2b-384-merkle-and-transcript:digest-bytes=48:public-replay:no-masking:trace=2^13:one-row-per-tick:state=progress,x,speed,vx,energy,finished,finish-tick:dnf=public-columns:boundaries=initial,checkpoint,final:logup=16-bit-verifier-summed-table,4-lookups-per-helper:blowup=8:queries=87:fri-arity=8:final-degree=16:grinding=20:round-target=129:security-target=128:envelope-max=4194304:stark-max=3250000:all-car-dynamics:all-ordered-contacts:deferred-contact-slowing";
/// Classical soundness target of this profile, enforced by every proof and verification against
/// the exact circuit. Meeting it is necessary for, but separate from, release qualification.
const RACE_TARGET_SECURITY_BITS_V1: u16 = 128;

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
        target_soundness_bits: RACE_TARGET_SECURITY_BITS_V1,
        maximum_proof_bytes: u32::try_from(RACE_MAX_PROOF_BYTES_V1).expect("bounded circuit index"),
        qualified: race_profile_is_qualified_v1(),
    }
}
/// Commitment to every immutable gameplay parameter and operation order.
#[must_use]
pub fn race_rules_hash_v1() -> Hash {
    Hash::new(RULES)
}
/// Exact repository-relative source inventory committed by the stock verifier.
/// The browser exporter uses this same inventory to reject stale native binaries even when the
/// driving arithmetic is unchanged but the public admission or proof format has changed.
pub(super) const RACE_PROFILE_SOURCES_V1: &[(&str, &[u8])] = &[
    (
        "crates/iroha_core_privacy/src/execution_proofs/race.rs",
        include_bytes!("race.rs"),
    ),
    (
        "crates/iroha_core_privacy/src/execution_proofs/environment.rs",
        include_bytes!("environment.rs"),
    ),
    (
        "crates/iroha_core_privacy/src/execution_proofs/proof.rs",
        include_bytes!("proof.rs"),
    ),
    (
        "crates/iroha_data_model/src/game.rs",
        include_bytes!("../../../iroha_data_model/src/game.rs"),
    ),
    (
        "crates/iroha_data_model/src/game_resources.rs",
        include_bytes!("../../../iroha_data_model/src/game_resources.rs"),
    ),
    (
        "crates/iroha_data_model/src/execution_proofs.rs",
        include_bytes!("../../../iroha_data_model/src/execution_proofs.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/lib.rs",
        include_bytes!("../../../iroha_execution_stark/src/lib.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/air.rs",
        include_bytes!("../../../iroha_execution_stark/src/air.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/circuit.rs",
        include_bytes!("../../../iroha_execution_stark/src/circuit.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/field.rs",
        include_bytes!("../../../iroha_execution_stark/src/field.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/fri.rs",
        include_bytes!("../../../iroha_execution_stark/src/fri.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/hash.rs",
        include_bytes!("../../../iroha_execution_stark/src/hash.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/merkle.rs",
        include_bytes!("../../../iroha_execution_stark/src/merkle.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/ntt.rs",
        include_bytes!("../../../iroha_execution_stark/src/ntt.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/params.rs",
        include_bytes!("../../../iroha_execution_stark/src/params.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/proof.rs",
        include_bytes!("../../../iroha_execution_stark/src/proof.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/prover.rs",
        include_bytes!("../../../iroha_execution_stark/src/prover.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/trace.rs",
        include_bytes!("../../../iroha_execution_stark/src/trace.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/transcript.rs",
        include_bytes!("../../../iroha_execution_stark/src/transcript.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/verifier.rs",
        include_bytes!("../../../iroha_execution_stark/src/verifier.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/games/mod.rs",
        include_bytes!("../../../iroha_execution_stark/src/games/mod.rs"),
    ),
    (
        "crates/iroha_execution_stark/src/games/race_v1.rs",
        include_bytes!("../../../iroha_execution_stark/src/games/race_v1.rs"),
    ),
];
/// Commitment to the complete frozen `RaceV1` verifier and native outcome semantics.
///
/// The source commitment binds the compiled transition, outcome and verifier implementation.
/// This first release has one implementation; obsolete development drafts have no dispatch entry.
/// The registry and exporters are outside the commitment so unrelated compiled relations do not
/// change this identity. Any change to these committed sources produces a different profile ID.
#[must_use]
pub fn race_profile_id_v1() -> Hash {
    static ID: std::sync::OnceLock<Hash> = std::sync::OnceLock::new();
    *ID.get_or_init(|| {
        let mut chunks: Vec<&[u8]> =
            vec![b"iroha:execution:profile:v1\0".as_slice(), PROFILE, RULES];
        chunks.extend(RACE_PROFILE_SOURCES_V1.iter().map(|(_, bytes)| *bytes));
        Hash::new_from_chunks(&chunks)
    })
}
/// Full-strength commitment to the compiled profile: the descriptor, the rules and every committed
/// source with its path, as length-framed items under the engine's BLAKE2b-384 suite. The 255-bit
/// ledger identifier `race_profile_id_v1` stays the registry and envelope key; every proof
/// statement also binds this commitment.
fn race_profile_commitment_v1() -> ProfileDigest {
    static COMMITMENT: std::sync::OnceLock<ProfileDigest> = std::sync::OnceLock::new();
    *COMMITMENT.get_or_init(|| profile_commitment(PROFILE, RULES, RACE_PROFILE_SOURCES_V1))
}

fn profile_commitment(profile: &[u8], rules: &[u8], sources: &[(&str, &[u8])]) -> ProfileDigest {
    let mut hasher = FramedHash::new(b"iroha:execution:race-v1:profile-commitment:v1");
    hasher.update(profile);
    hasher.update(rules);
    hasher.update(&(sources.len() as u64).to_le_bytes());
    for (path, bytes) in sources {
        hasher.update(path.as_bytes());
        hasher.update(bytes);
    }
    hasher.finalize()
}
/// Exact canonical public replay commitment used by peer and ledger certificates.
#[must_use]
pub fn race_transcript_root_v1(network: &NetworkId, replay: &RaceReplayV1) -> Hash {
    game_message_hash_v1(
        network,
        "input-transcript",
        &race_game_transcript_v1(replay),
    )
}
/// Convert the application replay to the generic, opaque canonical session transcript.
#[must_use]
pub fn race_game_transcript_v1(replay: &RaceReplayV1) -> GameTranscriptV1 {
    GameTranscriptV1 {
        batches: replay
            .frames
            .chunks(6)
            .map(|frames| {
                let start_tick = frames[0].tick;
                GameTranscriptBatchV1 {
                    start_tick,
                    inputs: (0..replay.player_count)
                        .map(|slot| {
                            if replay.dnf_events.iter().any(|event| {
                                event.tick <= start_tick && event.slots.contains(&slot)
                            }) {
                                vec![]
                            } else {
                                frames
                                    .iter()
                                    .flat_map(|frame| {
                                        frame.controls[usize::from(slot)].to_le_bytes()
                                    })
                                    .collect()
                            }
                        })
                        .collect(),
                }
            })
            .collect(),
        dnf_events: replay
            .dnf_events
            .iter()
            .map(|event| GameDnfEventV1 {
                tick: event.tick,
                slots: event.slots.clone(),
            })
            .collect(),
    }
}
/// Exact canonical state commitment used by peer and ledger certificates.
#[must_use]
pub fn race_state_root_v1(network: &NetworkId, state: &RaceStateV1) -> Hash {
    game_message_hash_v1(network, "simulation-state", &state.encode())
}

/// Validate application-neutral manifest bounds against its exact compiled relation.
pub fn validate_race_manifest_v1(manifest: &GameManifestV1) -> Result<(), ExecutionProofErrorV1> {
    if manifest.version != 1
        || manifest.profile_id != race_profile_id_v1()
        || manifest.min_participants != 2
        || !(2..=8).contains(&manifest.max_participants)
        || manifest.batch_ticks != 6
        || manifest.max_ticks != 5400
        || manifest.max_input_bytes != 12
        || manifest.max_participant_data_bytes != 1
    {
        return Err(ExecutionProofErrorV1::Statement);
    }
    decode_track(manifest)?;
    Ok(())
}
fn decode_track(manifest: &GameManifestV1) -> Result<RaceTrackV1, ExecutionProofErrorV1> {
    let mut bytes = manifest.application_parameters.as_slice();
    let track = RaceTrackV1::decode(&mut bytes).map_err(|_| ExecutionProofErrorV1::Statement)?;
    if !bytes.is_empty() || track.encode() != manifest.application_parameters {
        return Err(ExecutionProofErrorV1::Statement);
    }
    Ok(track)
}
/// Validate opaque application participant data through the compiled adapter.
pub fn validate_race_participant_v1(
    manifest: &GameManifestV1,
    data: &[u8],
) -> Result<(), ExecutionProofErrorV1> {
    validate_race_manifest_v1(manifest)?;
    if data.len() != 1 || data[0] >= RACE_SKIN_COUNT_V1 {
        return Err(ExecutionProofErrorV1::Statement);
    }
    Ok(())
}
/// Validate the complete immutable admission projection before constructing any AIR trace.
/// Stock racing accepts cosmetic data only; resource-enabled adapters need their own compiled
/// relation and exact entitlement checks. NFT metadata never chooses stock physics parameters.
fn validate_race_admission_v1(
    manifest: &GameManifestV1,
    admission: &GameAdmissionBodyV1,
    player_count: u8,
) -> Result<(), ExecutionProofErrorV1> {
    admission
        .validate()
        .map_err(|_| ExecutionProofErrorV1::Statement)?;
    if admission.participants.len() != usize::from(player_count)
        || player_count < manifest.min_participants
        || player_count > manifest.max_participants
        || !admission.resources.is_empty()
    {
        return Err(ExecutionProofErrorV1::Statement);
    }
    for participant in &admission.participants {
        validate_race_participant_v1(manifest, &participant.application_data)?;
    }
    Ok(())
}
/// Validate opaque input batches through the compiled adapter, before accepting commitments/reveals.
pub fn validate_race_input_v1(
    manifest: &GameManifestV1,
    input: &[u8],
) -> Result<(), ExecutionProofErrorV1> {
    validate_race_manifest_v1(manifest)?;
    if input.len() != 12
        || input
            .as_chunks::<2>()
            .0
            .iter()
            .any(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]) & !RACE_CONTROL_MASK_V1 != 0)
    {
        return Err(ExecutionProofErrorV1::Replay);
    }
    Ok(())
}
/// Compute the compiled application's prescribed initial state commitment.
pub fn initial_race_manifest_state_root_v1(
    network: &NetworkId,
    manifest: &GameManifestV1,
    player_count: u8,
) -> Result<Hash, ExecutionProofErrorV1> {
    validate_race_manifest_v1(manifest)?;
    if player_count < 2 || player_count > manifest.max_participants {
        return Err(ExecutionProofErrorV1::Replay);
    }
    let state = initial_race_state_v1(decode_track(manifest)?, player_count)
        .map_err(|_| ExecutionProofErrorV1::Replay)?;
    Ok(race_state_root_v1(network, &state))
}
fn race_game_outcome_v1(state: &RaceStateV1) -> Result<GameOutcomeV1, ExecutionProofErrorV1> {
    let result = race_result_v1(state).map_err(|_| ExecutionProofErrorV1::Replay)?;
    Ok(GameOutcomeV1 {
        terminal_tick: state.tick,
        winner_slots: result.winners.clone(),
        result: result.encode(),
    })
}
fn race_payload_from_request_v1(
    request: &RaceProverRequestV1,
) -> Result<RaceProofPayloadV1, ExecutionProofErrorV1> {
    let final_state = replay_race_v1(&request.replay).map_err(|_| ExecutionProofErrorV1::Replay)?;
    let outcome = race_game_outcome_v1(&final_state)?;
    let relation_inputs = RacePublicInputsV1 {
        network_id: request.statement.network_id,
        race_id: request.statement.session_id,
        roster_hash: request.statement.roster_hash,
        rules_hash: race_rules_hash_v1(),
        track: request.replay.track,
        transcript_root: request.statement.transcript_root,
        dispute_root: request.statement.dispute_root,
        result: race_result_v1(&final_state).map_err(|_| ExecutionProofErrorV1::Replay)?,
    };
    Ok(RaceProofPayloadV1 {
        manifest: request.manifest.clone(),
        admission: request.admission.clone(),
        outcome,
        relation_inputs,
        replay: request.replay.clone(),
        final_state,
        checkpoint_state: request.checkpoint_state.clone(),
        stark_bytes: vec![],
    })
}
/// Verify a compiled execution envelope and return only the adapter-authenticated generic outcome.
pub fn verify_race_outcome_v1(
    envelope: &ExecutionProofEnvelopeV1,
) -> Result<GameOutcomeV1, ExecutionProofErrorV1> {
    let payload = decode_payload(envelope)?;
    validate_payload(&envelope.statement, &payload)?;
    verify_engine_proof(&envelope.statement, &payload)?;
    Ok(payload.outcome)
}

fn validate_payload(
    statement: &ExecutionPublicInputsV1,
    payload: &RaceProofPayloadV1,
) -> Result<(), ExecutionProofErrorV1> {
    validate_race_manifest_v1(&payload.manifest)?;
    validate_race_admission_v1(
        &payload.manifest,
        &payload.admission,
        payload.replay.player_count,
    )?;
    if payload.replay.player_count < 2
        || payload.replay.player_count > 8
        || payload.replay.frames.len() > 5400
        || payload
            .replay
            .frames
            .iter()
            .any(|frame| frame.controls.len() != usize::from(payload.replay.player_count))
    {
        return Err(ExecutionProofErrorV1::Replay);
    }
    let relation = &payload.relation_inputs;
    let expected_outcome = race_game_outcome_v1(&payload.final_state)?;
    let race_session_id = relation.race_id;
    if statement.network_id != relation.network_id
        || statement.session_id != race_session_id
        || statement.roster_hash != relation.roster_hash
        || statement.roster_hash
            != game_roster_hash_v1(
                &statement.network_id,
                &statement.session_id,
                &payload.admission,
            )
        || statement.transcript_root != relation.transcript_root
        || statement.dispute_root != relation.dispute_root
        || statement.manifest_hash
            != game_message_hash_v1(&statement.network_id, "session-manifest", &payload.manifest)
        || statement.outcome_hash
            != game_message_hash_v1(&statement.network_id, "session-outcome", &payload.outcome)
        || payload.outcome != expected_outcome
        || payload.replay.track != decode_track(&payload.manifest)?
        || payload.replay.player_count > payload.manifest.max_participants
    {
        return Err(ExecutionProofErrorV1::Statement);
    }
    let replay = &payload.replay;
    let final_state = &payload.final_state;
    if !(1..=8).contains(&replay.player_count)
        || replay.frames.len() > RACE_MAX_TICKS_V1 as usize
        || replay.frames.iter().enumerate().any(|(tick, frame)| {
            frame.tick != u32::try_from(tick).expect("bounded circuit index")
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
            event.tick > u32::try_from(replay.frames.len()).expect("validated replay duration")
                || event.tick % 6 != 0
                || event.slots.is_empty()
                || event.slots.windows(2).any(|slots| slots[0] >= slots[1])
                || event.slots.iter().any(|slot| *slot >= replay.player_count)
        })
        || !final_state.tick.is_multiple_of(6)
        || final_state.cars.len() != usize::from(replay.player_count)
        || final_state.track != replay.track
        || final_state.tick
            != u32::try_from(replay.frames.len()).expect("validated replay duration")
        || (final_state.tick != RACE_MAX_TICKS_V1
            && final_state
                .cars
                .iter()
                .filter(|car| car.dnf_tick.is_none())
                .count()
                >= 2
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
    if replay.frames.iter().any(|frame| {
        replay
            .dnf_events
            .iter()
            .filter(|event| event.tick <= frame.tick)
            .any(|event| {
                event
                    .slots
                    .iter()
                    .any(|slot| frame.controls[usize::from(*slot)] != 0)
            })
    }) {
        return Err(ExecutionProofErrorV1::Replay);
    }
    if relation.rules_hash != race_rules_hash_v1()
        || relation.track != replay.track
        || relation.transcript_root != race_transcript_root_v1(&statement.network_id, replay)
        || relation.result
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

fn engine_track(track: RaceTrackV1) -> race_circuit::Track {
    match track {
        RaceTrackV1::NeonTokyo => race_circuit::Track::NeonTokyo,
        RaceTrackV1::Harbor => race_circuit::Track::Harbor,
        RaceTrackV1::Sakura => race_circuit::Track::Sakura,
    }
}

fn engine_replay(replay: &RaceReplayV1) -> race_circuit::Replay {
    race_circuit::Replay {
        track: engine_track(replay.track),
        players: usize::from(replay.player_count),
        frames: replay
            .frames
            .iter()
            .map(|frame| frame.controls.clone())
            .collect(),
        dnf_events: replay
            .dnf_events
            .iter()
            .map(|event| (event.tick, event.slots.clone()))
            .collect(),
    }
}

/// Circuit registers of a state, in `race_circuit::registers` order. Removals are public columns.
fn engine_registers(state: &RaceStateV1) -> Vec<i64> {
    state
        .cars
        .iter()
        .flat_map(|car| {
            [
                car.progress_mm,
                i64::from(car.lateral_mm),
                i64::from(car.speed_mm_per_tick),
                i64::from(car.lateral_velocity_mm_per_tick),
                i64::from(car.boost_energy),
                i64::from(car.finish_tick.is_some()),
                i64::from(car.finish_tick.unwrap_or(0)),
            ]
        })
        .collect()
}

/// Removal ticks must match the public schedule exactly: the final state records every removal
/// (those at the last tick included) and a checkpoint records those strictly before its tick.
fn removals_match_states(payload: &RaceProofPayloadV1) -> bool {
    let removed = race_circuit::removal_ticks(&engine_replay(&payload.replay));
    payload.final_state.cars.len() == removed.len()
        && payload
            .final_state
            .cars
            .iter()
            .zip(&removed)
            .all(|(car, tick)| car.dnf_tick == *tick)
        && payload.checkpoint_state.as_ref().is_none_or(|checkpoint| {
            checkpoint.cars.len() == removed.len()
                && checkpoint
                    .cars
                    .iter()
                    .zip(&removed)
                    .all(|(car, tick)| car.dnf_tick == tick.filter(|tick| *tick < checkpoint.tick))
        })
}

/// Transcript binding of the engine statement: the exact envelope statement and every payload
/// field except the proof bytes themselves, as length-prefixed canonical encodings. The engine
/// absorbs these bytes unhashed, so the binding rests on the transcript hash alone rather than on
/// the 255-bit ledger `Hash`. It includes the full-strength profile commitment as well as the
/// ledger profile identifier.
fn engine_binding(statement: &ExecutionPublicInputsV1, payload: &RaceProofPayloadV1) -> Vec<u8> {
    let profile = race_profile_id_v1();
    let commitment = race_profile_commitment_v1();
    let fields: [&[u8]; 11] = [
        b"iroha:execution:race-v1:engine-binding:v1",
        profile.as_ref(),
        &commitment,
        &statement.encode(),
        &payload.manifest.encode(),
        &payload.admission.encode(),
        &payload.outcome.encode(),
        &payload.relation_inputs.encode(),
        &payload.replay.encode(),
        &payload.final_state.encode(),
        &payload.checkpoint_state.encode(),
    ];
    let mut binding = Vec::with_capacity(fields.iter().map(|field| 8 + field.len()).sum());
    for field in fields {
        binding.extend_from_slice(&(field.len() as u64).to_le_bytes());
        binding.extend_from_slice(field);
    }
    binding
}

/// The engine circuit, statement and public columns of a validated payload. Checks the replay's
/// public structure (including the public half of the six-tick terminal rule, which the circuit
/// does not prove) and the removal ticks of the final and checkpoint states.
fn engine_statement(
    statement: &ExecutionPublicInputsV1,
    payload: &RaceProofPayloadV1,
) -> Result<(Circuit, Statement, Vec<Vec<i64>>), ExecutionProofErrorV1> {
    let replay = engine_replay(&payload.replay);
    race_circuit::validate_replay(&replay).map_err(|_| ExecutionProofErrorV1::Replay)?;
    if !removals_match_states(payload) {
        return Err(ExecutionProofErrorV1::Replay);
    }
    let initial = initial_race_state_v1(payload.replay.track, payload.replay.player_count)
        .map_err(|_| ExecutionProofErrorV1::Replay)?;
    let public =
        race_circuit::public_columns(&replay).map_err(|_| ExecutionProofErrorV1::Replay)?;
    let engine_statement = Statement {
        steps: payload.replay.frames.len(),
        initial: engine_registers(&initial),
        final_state: engine_registers(&payload.final_state),
        checkpoint: payload.checkpoint_state.as_ref().map(|state| Checkpoint {
            step: state.tick as usize,
            registers: engine_registers(state),
        }),
        binding: engine_binding(statement, payload),
    };
    Ok((
        race_circuit::circuit(replay.track, replay.players),
        engine_statement,
        public,
    ))
}

/// The standard parameters after checking them, for this exact circuit, against the profile's
/// immutable security target. Every proof and every verification goes through this check.
fn engine_params(circuit: &Circuit) -> Result<Params, ExecutionProofErrorV1> {
    let params = Params::standard();
    params.check_security(circuit, f64::from(RACE_TARGET_SECURITY_BITS_V1))?;
    Ok(params)
}

/// Verify the payload's proof bytes against the engine statement rebuilt from public data.
fn verify_engine_proof(
    statement: &ExecutionPublicInputsV1,
    payload: &RaceProofPayloadV1,
) -> Result<(), ExecutionProofErrorV1> {
    let (circuit, engine_statement, public) = engine_statement(statement, payload)?;
    let params = engine_params(&circuit)?;
    let proof =
        Proof::from_bytes(&payload.stark_bytes).map_err(|_| ExecutionProofErrorV1::Envelope)?;
    if proof.to_bytes() != payload.stark_bytes {
        return Err(ExecutionProofErrorV1::Envelope);
    }
    iroha_execution_stark::verify(&circuit, &params, &engine_statement, &public, &proof)?;
    Ok(())
}

/// Register vectors before every tick and after the last, from the node's own simulation.
fn race_register_states(replay: &RaceReplayV1) -> Result<Vec<Vec<i64>>, ExecutionProofErrorV1> {
    let mut state = initial_race_state_v1(replay.track, replay.player_count)
        .map_err(|_| ExecutionProofErrorV1::Replay)?;
    let mut events = replay.dnf_events.iter().peekable();
    let mut states = Vec::with_capacity(replay.frames.len() + 1);
    for frame in &replay.frames {
        if events.peek().is_some_and(|event| event.tick == state.tick) {
            let event = events.next().expect("peeked event");
            apply_race_dnf_v1(&mut state, &event.slots)
                .map_err(|_| ExecutionProofErrorV1::Replay)?;
        }
        states.push(engine_registers(&state));
        step_race_v1(&mut state, frame).map_err(|_| ExecutionProofErrorV1::Replay)?;
    }
    // Removals at the last tick change no register.
    states.push(engine_registers(&state));
    Ok(states)
}

/// Generate and self-verify a genuine native execution proof. Proving is deterministic.
///
/// # Errors
/// Rejects inconsistent admission, manifest, replay or statement bindings; propagates proving,
/// size-limit and self-verification failures.
pub fn prove_race_v1(
    request: &RaceProverRequestV1,
) -> Result<ExecutionProofEnvelopeV1, ExecutionProofErrorV1> {
    let mut payload = race_payload_from_request_v1(request)?;
    validate_payload(&request.statement, &payload)?;
    let (circuit, engine_statement, public) = engine_statement(&request.statement, &payload)?;
    let params = engine_params(&circuit)?;
    let states = race_register_states(&payload.replay)?;
    let proof =
        iroha_execution_stark::prove(&circuit, &params, &engine_statement, &public, &states)?;
    payload.stark_bytes = proof.to_bytes();
    if payload.stark_bytes.len() > RACE_MAX_STARK_BYTES_V1 {
        return Err(ExecutionProofErrorV1::Envelope);
    }
    let envelope = ExecutionProofEnvelopeV1 {
        version: 1,
        profile_id: race_profile_id_v1(),
        statement: request.statement,
        proof_bytes: payload.encode(),
    };
    if envelope.encode().len() > RACE_MAX_PROOF_BYTES_V1 {
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
        || envelope.encode().len() > RACE_MAX_PROOF_BYTES_V1
    {
        return Err(ExecutionProofErrorV1::Envelope);
    }
    let mut bytes = envelope.proof_bytes.as_slice();
    let payload =
        RaceProofPayloadV1::decode(&mut bytes).map_err(|_| ExecutionProofErrorV1::Envelope)?;
    if !bytes.is_empty()
        || payload.stark_bytes.len() > RACE_MAX_STARK_BYTES_V1
        || payload.encode() != envelope.proof_bytes
    {
        return Err(ExecutionProofErrorV1::Envelope);
    }
    Ok(payload)
}
/// Verify the complete semantic execution relation, with no replay-only or binding-only mode.
///
/// # Errors
/// Rejects malformed or oversized envelopes, inconsistent semantic bindings, and invalid proofs.
pub fn verify_race_proof_v1(
    envelope: &ExecutionProofEnvelopeV1,
) -> Result<(), ExecutionProofErrorV1> {
    let payload = decode_payload(envelope)?;
    validate_payload(&envelope.statement, &payload)?;
    verify_engine_proof(&envelope.statement, &payload)
}

/// Retained chain history against which an execution proof is authenticated.
#[derive(Clone, Copy)]
pub struct GameProofHistoryV1<'a> {
    /// Latest certified checkpoint, when present.
    pub checkpoint: Option<&'a GameCheckpointV1>,
    /// Certified transcript prefixes and participant removals.
    pub anchors: &'a [GameTranscriptAnchorV1],
    /// Retained forced input batches in chain order.
    pub batches: &'a [GameForcedBatchV1],
    /// Current slot-ordered participant roster.
    pub participants: &'a [GameParticipantV1],
    /// Session epoch selected by committed state.
    pub epoch: u64,
}

/// Verify semantic execution and exact retained checkpoint, forced inputs, and removals.
///
/// # Errors
/// Rejects invalid proofs and mismatches with the retained session history.
pub fn verify_race_proof_for_history_v1(
    envelope: &ExecutionProofEnvelopeV1,
    manifest: &GameManifestV1,
    outcome: &GameOutcomeV1,
    history: GameProofHistoryV1<'_>,
) -> Result<(), ExecutionProofErrorV1> {
    let GameProofHistoryV1 {
        checkpoint,
        anchors,
        batches,
        participants,
        epoch,
    } = history;
    let payload = decode_payload(envelope)?;
    validate_payload(&envelope.statement, &payload)?;
    if &payload.manifest != manifest || &payload.outcome != outcome {
        return Err(ExecutionProofErrorV1::History);
    }
    let statement = &envelope.statement;
    let replay = &payload.replay;
    if participants.len() != usize::from(replay.player_count)
        || statement.dispute_root
            != game_message_hash_v1(
                &statement.network_id,
                "dispute-history",
                &(
                    statement.session_id,
                    epoch,
                    checkpoint.copied(),
                    anchors.to_vec(),
                    batches.to_vec(),
                ),
            )
    {
        return Err(ExecutionProofErrorV1::History);
    }
    // A removal changes the set of future checkpoint signers. Every prefix authenticated before
    // that change remains immutable even when the current checkpoint is later replaced.
    if anchors.len() > 32 {
        return Err(ExecutionProofErrorV1::History);
    }
    for anchor in anchors {
        if anchor.tick > u32::try_from(replay.frames.len()).expect("validated replay duration")
            || anchor.tick % 6 != 0
        {
            return Err(ExecutionProofErrorV1::History);
        }
        let prefix = RaceReplayV1 {
            track: replay.track,
            player_count: replay.player_count,
            frames: replay.frames[..anchor.tick as usize].to_vec(),
            dnf_events: replay
                .dnf_events
                .iter()
                .filter(|event| event.tick < anchor.tick)
                .cloned()
                .collect(),
        };
        if race_transcript_root_v1(&statement.network_id, &prefix) != anchor.transcript_root {
            return Err(ExecutionProofErrorV1::History);
        }
    }
    let mut expected_dnfs = std::collections::BTreeMap::<u32, Vec<u8>>::new();
    for (slot, participant) in participants.iter().enumerate() {
        if let Some(tick) = participant
            .dnf_at_tick
            .filter(|tick| *tick <= payload.final_state.tick)
        {
            expected_dnfs
                .entry(tick)
                .or_default()
                .push(u8::try_from(slot).expect("validated player or port count"));
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
            if checkpoint.session_id != statement.session_id
                || checkpoint.tick
                    > u32::try_from(replay.frames.len()).expect("validated replay duration")
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
    if batches
        .windows(2)
        .any(|pair| pair[0].start_tick.saturating_add(6) > pair[1].start_tick)
    {
        return Err(ExecutionProofErrorV1::History);
    }
    for batch in batches {
        let extends_tail = batch.start_tick >= tail;
        if (extends_tail && batch.start_tick != tail)
            || batch.inputs.len() != participants.len()
            || batch.epoch > epoch
        {
            return Err(ExecutionProofErrorV1::History);
        }
        let mut controls = Vec::with_capacity(participants.len());
        for (slot, input) in batch.inputs.iter().enumerate() {
            let removed = participants[slot]
                .dnf_at_tick
                .is_some_and(|tick| tick <= batch.start_tick);
            if removed {
                if !input.is_empty() {
                    return Err(ExecutionProofErrorV1::History);
                }
                controls.push([0_u16; 6]);
            } else {
                validate_race_input_v1(manifest, input)?;
                controls.push(std::array::from_fn(|offset| {
                    u16::from_le_bytes([input[offset * 2], input[offset * 2 + 1]])
                }));
            }
        }
        for offset in 0..6 {
            let tick = batch.start_tick + offset;
            if let Some(frame) = replay.frames.get(tick as usize)
                && frame.controls
                    != controls
                        .iter()
                        .map(|controls| controls[offset as usize])
                        .collect::<Vec<_>>()
            {
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
        if extends_tail {
            tail = tail.saturating_add(6);
        }
    }
    if tail < u32::try_from(replay.frames.len()).expect("validated replay duration") {
        return Err(ExecutionProofErrorV1::History);
    }
    verify_engine_proof(statement, &payload)
}

#[cfg(test)]
#[path = "proof_tests.rs"]
mod tests;
