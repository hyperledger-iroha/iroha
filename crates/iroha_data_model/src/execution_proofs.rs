//! Canonical native execution-proof statements and the bounded RaceV1 simulation model.

use crate::NetworkId;

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Closed native execution catalog's maximum canonical proof-envelope size.
///
/// This includes public replay data and the native cryptographic proof. It is a
/// local type/admission ceiling, not permission to exceed transaction, block,
/// Torii, gossip or data-availability limits. A deployment must qualify those
/// independent limits before admitting funded execution profiles.
pub const EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1: usize = 4 * 1024 * 1024;

/// Simulation ticks per second in the immutable RaceV1 rules.
pub const RACE_TICKS_PER_SECOND_V1: u32 = 30;
/// Maximum duration of one RaceV1 run.
pub const RACE_MAX_TICKS_V1: u32 = 5_400;
/// Maximum authenticated racers in a RaceV1 run.
pub const RACE_MAX_PLAYERS_V1: u8 = 8;
/// Required completed laps.
pub const RACE_LAPS_V1: u32 = 3;
/// Ticks in one peer commitment/reveal batch.
pub const RACE_INPUT_BATCH_TICKS_V1: u32 = 6;
/// Bit mask of the six permitted controls.
pub const RACE_CONTROL_MASK_V1: u16 = 0x3f;
/// Number of visual car skins; all share the same simulation parameters.
pub const RACE_SKIN_COUNT_V1: u8 = 6;

/// The closed first-release track catalog.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RaceTrackV1 {
    /// Two-kilometre city circuit.
    NeonTokyo,
    /// 2.4-kilometre waterfront circuit.
    Harbor,
    /// 1.8-kilometre cherry-blossom circuit.
    Sakura,
}

impl RaceTrackV1 {
    /// Exact length of one lap in millimetres.
    #[must_use]
    pub const fn length_mm(self) -> i64 {
        match self {
            Self::NeonTokyo => 2_000_000,
            Self::Harbor => 2_400_000,
            Self::Sakura => 1_800_000,
        }
    }

    /// Twelve road-curvature cells, sampled with Euclidean wrapped progress.
    #[must_use]
    pub const fn curvature(self) -> [i32; 12] {
        match self {
            Self::NeonTokyo => [0, 1, 2, 1, 0, -1, -2, -1, 0, 2, -2, 0],
            Self::Harbor => [0, -2, -2, 0, 1, 3, 1, 0, -1, -3, -1, 0],
            Self::Sakura => [0, 1, 1, 0, -2, -1, 0, 2, 3, 1, -2, 0],
        }
    }
}

/// All players' authenticated controls at one exact tick.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
)]
pub struct RaceInputFrameV1 {
    /// Zero-based tick, without gaps or repeats.
    pub tick: u32,
    /// Slot-ordered masks: throttle, brake, left, right, drift, boost in bits 0..5.
    pub controls: Vec<u16>,
}

/// Consensus-authorized removals applied before one exact simulation tick.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::execution_proofs::RaceDnfEventV1")]
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
)]
pub struct RaceDnfEventV1 {
    /// Tick boundary at which these unfinished cars lose their collision bodies.
    pub tick: u32,
    /// Unique ascending participant slots.
    pub slots: Vec<u8>,
}

/// A complete deterministic replay from the prescribed starting grid.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::execution_proofs::RaceReplayV1")]
pub struct RaceReplayV1 {
    /// Immutable track selection.
    pub track: RaceTrackV1,
    /// Number of occupied slots, between one and eight for local replay.
    pub player_count: u8,
    /// Canonical consecutive frames.
    pub frames: Vec<RaceInputFrameV1>,
    /// Unique ascending exact-tick DNF events selected by consensus.
    pub dnf_events: Vec<RaceDnfEventV1>,
}

/// Integer state of one car; skins are intentionally absent.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
)]
pub struct RaceCarStateV1 {
    /// Unwrapped longitudinal progress, including the negative starting-grid offset.
    pub progress_mm: i64,
    /// Signed road-relative lateral position.
    pub lateral_mm: i32,
    /// Forward displacement per tick.
    pub speed_mm_per_tick: i32,
    /// Lateral displacement per tick before curvature force.
    pub lateral_velocity_mm_per_tick: i32,
    /// Boost reserve in the inclusive range 0..1000.
    pub boost_energy: u16,
    /// First completed simulation tick at which the car crossed the finish.
    pub finish_tick: Option<u32>,
    /// Consensus removal boundary; a previous finish remains historical after forfeiture.
    pub dnf_tick: Option<u32>,
}

/// Complete deterministic race state at a tick boundary.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::execution_proofs::RaceStateV1")]
pub struct RaceStateV1 {
    /// Number of completed ticks.
    pub tick: u32,
    /// Immutable circuit.
    pub track: RaceTrackV1,
    /// Cars in canonical on-chain participant-slot order.
    pub cars: Vec<RaceCarStateV1>,
}

/// One ranked entry in the result.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
)]
pub struct RaceStandingV1 {
    /// Original participant slot.
    pub slot: u8,
    /// Historical finish tick, if reached; forfeiture may subsequently remove eligibility.
    pub finish_tick: Option<u32>,
    /// Consensus removal boundary; a previous finish remains historical after forfeiture.
    pub dnf_tick: Option<u32>,
    /// Final unwrapped progress.
    pub progress_mm: i64,
}

/// Public outcome recomputed by the native execution relation.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::execution_proofs::RaceResultV1")]
pub struct RaceResultV1 {
    /// Exact number of simulated ticks.
    pub ticks: u32,
    /// Eligible racers before forfeits, then finish time and progress; slot breaks display ties.
    pub standings: Vec<RaceStandingV1>,
    /// Earliest finishers, a sole eligible survivor, or eligible distance leaders at timeout.
    /// Empty for an unfinished prefix or a terminal all-forfeit refund.
    pub winners: Vec<u8>,
}

/// Ledger-bound statement for a native RaceV1 execution proof.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
)]
pub struct RacePublicInputsV1 {
    /// Exact runtime network, preventing cross-network replay.
    pub network_id: NetworkId,
    /// One immutable race identifier.
    pub race_id: Hash,
    /// Commitment to the immutable participant, session-key, wager and equipment admission body.
    pub roster_hash: Hash,
    /// Commitment to the complete immutable physics and race rules.
    pub rules_hash: Hash,
    /// Track selected when the race was opened.
    pub track: RaceTrackV1,
    /// Authenticated cumulative input-transcript commitment.
    pub transcript_root: Hash,
    /// Commitment to all chain-resolved input and DNF events.
    pub dispute_root: Hash,
    /// Claimed outcome, checked by the execution relation.
    pub result: RaceResultV1,
}

/// Application-neutral statement for a compiled native execution relation.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
)]
pub struct ExecutionPublicInputsV1 {
    /// Exact runtime network.
    pub network_id: NetworkId,
    /// One immutable game session.
    pub session_id: Hash,
    /// Exact canonical application manifest commitment.
    pub manifest_hash: Hash,
    /// Exact immutable wallet, gameplay-key, wager and equipment admission commitment.
    pub roster_hash: Hash,
    /// Authenticated cumulative application input transcript commitment.
    pub transcript_root: Hash,
    /// Commitment to all consensus-resolved input and removal events.
    pub dispute_root: Hash,
    /// Exact typed generic outcome commitment, interpreted only after adapter verification.
    pub outcome_hash: Hash,
}

/// Exact first-release application-neutral native execution proof envelope.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::execution_proofs::ExecutionProofEnvelopeV1")]
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
)]
pub struct ExecutionProofEnvelopeV1 {
    /// Envelope version; exactly one.
    pub version: u16,
    /// Immutable compiled verifier/profile digest.
    pub profile_id: Hash,
    /// Exact typed public statement.
    pub statement: ExecutionPublicInputsV1,
    /// Canonical bounded native STARK proof payload.
    pub proof_bytes: Vec<u8>,
}

/// Public replay availability and the exact native cryptographic execution proof.
/// Replay data is intentionally public, allowing any replaceable worker to prove the race.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::execution_proofs::RaceProofPayloadV1")]
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
)]
pub struct RaceProofPayloadV1 {
    /// Exact generic session manifest; this adapter decodes only its compiled parameters.
    pub manifest: crate::game::GameManifestV1,
    /// Public immutable admission body whose canonical commitment must match the statement.
    pub admission: crate::game::GameAdmissionBodyV1,
    /// Typed generic payout outcome, derived from the proof-bound RaceV1 final state.
    pub outcome: crate::game::GameOutcomeV1,
    /// Race-specific relation statement; it is not part of any generic native instruction.
    pub relation_inputs: RacePublicInputsV1,
    /// Complete input and consensus DNF transcript.
    pub replay: RaceReplayV1,
    /// Claimed terminal state, constrained by the final AIR boundary.
    pub final_state: RaceStateV1,
    /// Optional retained checkpoint state, constrained by an intermediate AIR boundary.
    pub checkpoint_state: Option<RaceStateV1>,
    /// Exact bounded native STARK wire, with no alternate verifier modes.
    pub stark_bytes: Vec<u8>,
}

/// Portable input to a local native prover or an untrusted proof worker.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::execution_proofs::RaceProverRequestV1")]
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
)]
pub struct RaceProverRequestV1 {
    /// Complete network and ledger-bound claim.
    pub statement: ExecutionPublicInputsV1,
    /// Exact generic session manifest containing the compiled race parameters.
    pub manifest: crate::game::GameManifestV1,
    /// Public immutable participants and NFT authorizations; contains no custody signing keys.
    pub admission: crate::game::GameAdmissionBodyV1,
    /// Available canonical input transcript.
    pub replay: RaceReplayV1,
    /// Claimed state at the retained signed checkpoint, when present.
    pub checkpoint_state: Option<RaceStateV1>,
}

/// A compiled, versioned execution relation; arbitrary circuit or executable installation is absent.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ExecutionProofRelationV1 {
    /// The complete bounded deterministic arcade-race transition relation.
    RaceV1,
}

/// Exact native descriptor retained by the compiled execution-profile registry.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
)]
pub struct ExecutionProofProfileV1 {
    /// Descriptor schema version, exactly one.
    pub version: u16,
    /// Digest of the complete compiled verifier and profile parameters.
    pub profile_id: Hash,
    /// Commitment to the immutable relation rules.
    pub rules_hash: Hash,
    /// Closed native relation identity.
    pub relation: ExecutionProofRelationV1,
    /// Release target, not a claim that qualification has already passed.
    pub target_soundness_bits: u16,
    /// Maximum canonical execution envelope, including public replay and proof bytes.
    /// Transport and transaction bounds are separate admission conditions.
    pub maximum_proof_bytes: u32,
    /// True only after native correctness, cryptographic, and resource release gates pass.
    pub qualified: bool,
}

/// Compact receipt for mathematical execution validity; it does not authorize a race payout.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::execution_proofs::ExecutionProofVerificationV1")]
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
)]

pub struct ExecutionProofVerificationV1 {
    /// Exact compiled native verifier profile.
    pub profile_id: Hash,
    /// Domain-separated commitment to the verified public statement and profile.
    pub statement_hash: Hash,
    /// Exact canonical envelope retained in the finalized transaction body.
    pub proof_hash: Hash,
    /// Consensus block height at which native verification succeeded.
    pub verified_at_height: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_receipt_frame_binds_profile_statement_proof_and_height() {
        let receipt = ExecutionProofVerificationV1 {
            profile_id: Hash::new(b"verification-profile"),
            statement_hash: Hash::new(b"verification-statement"),
            proof_hash: Hash::new(b"verification-proof"),
            verified_at_height: 42,
        };
        assert_eq!(
            <ExecutionProofVerificationV1 as norito::NoritoSchema>::nominal_name(),
            "iroha_data_model::execution_proofs::ExecutionProofVerificationV1",
        );
        let frame = norito::encode_canonical(&receipt).expect("verification frame");
        assert_eq!(
            frame[6..22],
            norito::schema::identity::frame_hash::<ExecutionProofVerificationV1>()
        );
        assert_eq!(
            norito::decode_canonical::<ExecutionProofVerificationV1>(&frame).unwrap(),
            receipt
        );
        let mut changed = receipt;
        changed.verified_at_height += 1;
        assert_ne!(norito::encode_canonical(&changed).unwrap(), frame);
        let mut wrong_owner = frame.clone();
        wrong_owner[6] ^= 1;
        assert!(matches!(
            norito::decode_canonical::<ExecutionProofVerificationV1>(&wrong_owner),
            Err(norito::Error::SchemaMismatch)
        ));
        assert!(
            norito::decode_canonical::<ExecutionProofVerificationV1>(&frame[..frame.len() - 1])
                .is_err()
        );
        let mut trailing = frame;
        trailing.push(0);
        assert!(norito::decode_canonical::<ExecutionProofVerificationV1>(&trailing).is_err());
    }
    #[test]
    fn replay_norito_roundtrip() {
        let replay = RaceReplayV1 {
            track: RaceTrackV1::NeonTokyo,
            player_count: 2,
            frames: vec![RaceInputFrameV1 {
                tick: 0,
                controls: vec![1, 33],
            }],
            dnf_events: vec![],
        };
        let bytes = norito::encode_canonical(&replay).expect("encode replay");
        assert_eq!(
            norito::decode_canonical::<RaceReplayV1>(&bytes).expect("decode replay"),
            replay
        );
    }

    #[test]
    fn catalog_lengths_and_curve_bounds() {
        for track in [
            RaceTrackV1::NeonTokyo,
            RaceTrackV1::Harbor,
            RaceTrackV1::Sakura,
        ] {
            assert!(track.length_mm() > 0);
            assert!(
                track
                    .curvature()
                    .into_iter()
                    .all(|value| (-3..=3).contains(&value))
            );
        }
    }

    #[test]
    fn state_and_result_preserve_exact_tick_zero_removal() {
        let state = RaceStateV1 {
            tick: 6,
            track: RaceTrackV1::Harbor,
            cars: vec![RaceCarStateV1 {
                progress_mm: -4000,
                lateral_mm: -1800,
                speed_mm_per_tick: 0,
                lateral_velocity_mm_per_tick: -3,
                boost_energy: 975,
                finish_tick: None,
                dnf_tick: Some(0),
            }],
        };
        let result = RaceResultV1 {
            ticks: 6,
            standings: vec![RaceStandingV1 {
                slot: 0,
                finish_tick: None,
                dnf_tick: Some(0),
                progress_mm: -4000,
            }],
            winners: vec![],
        };
        let state_bytes = norito::encode_canonical(&state).expect("canonical state");
        let result_bytes = norito::encode_canonical(&result).expect("canonical result");
        assert_eq!(
            norito::decode_canonical::<RaceStateV1>(&state_bytes).expect("decode state"),
            state
        );
        assert_eq!(
            norito::decode_canonical::<RaceResultV1>(&result_bytes).expect("decode result"),
            result
        );

        {
            let state_json = norito::json::to_json(&state).expect("state JSON");
            let result_json = norito::json::to_json(&result).expect("result JSON");
            assert_eq!(
                norito::json::from_json::<RaceStateV1>(&state_json).expect("read state JSON"),
                state
            );
            assert_eq!(
                norito::json::from_json::<RaceResultV1>(&result_json).expect("read result JSON"),
                result
            );
        }
    }
}

#[cfg(test)]
mod additional_frame_owner_identity_tests {
    //! Typed frame contracts observed with the original codec.

    #[test]
    fn captured_additional_frame_owner_identities() {
        crate::frame_owner_identity_tests::assert_bidirectional::<
            crate::execution_proofs::RaceReplayV1,
        >("iroha_data_model::execution_proofs::RaceReplayV1");
        crate::frame_owner_identity_tests::assert_bidirectional::<
            crate::execution_proofs::RaceResultV1,
        >("iroha_data_model::execution_proofs::RaceResultV1");
        crate::frame_owner_identity_tests::assert_bidirectional::<
            crate::execution_proofs::RaceStateV1,
        >("iroha_data_model::execution_proofs::RaceStateV1");
    }
}
