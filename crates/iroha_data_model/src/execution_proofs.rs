//! Canonical native execution-proof statements and the bounded RaceV1 simulation model.

use crate::NetworkId;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonDeserialize, DeriveJsonSerialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", rename_all = "snake_case")
)]
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonDeserialize, DeriveJsonSerialize))]
pub struct RaceInputFrameV1 {
    /// Zero-based tick, without gaps or repeats.
    pub tick: u32,
    /// Slot-ordered masks: throttle, brake, left, right, drift, boost in bits 0..5.
    pub controls: Vec<u16>,
}

/// Consensus-authorized removals applied before one exact simulation tick.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonDeserialize, DeriveJsonSerialize))]
pub struct RaceDnfEventV1 {
    /// Tick boundary at which these unfinished cars lose their collision bodies.
    pub tick: u32,
    /// Unique ascending participant slots.
    pub slots: Vec<u8>,
}

/// A complete deterministic replay from the prescribed starting grid.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonDeserialize, DeriveJsonSerialize))]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonDeserialize, DeriveJsonSerialize))]
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
    /// Exact pre-tick removal boundary; finished cars cannot be disqualified.
    pub dnf_tick: Option<u32>,
}

/// Complete deterministic race state at a tick boundary.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonDeserialize, DeriveJsonSerialize))]
pub struct RaceStateV1 {
    /// Number of completed ticks.
    pub tick: u32,
    /// Immutable circuit.
    pub track: RaceTrackV1,
    /// Cars in canonical on-chain participant-slot order.
    pub cars: Vec<RaceCarStateV1>,
}

/// One ranked entry in the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonDeserialize, DeriveJsonSerialize))]
pub struct RaceStandingV1 {
    /// Original participant slot.
    pub slot: u8,
    /// Finish tick, or None for a DNF.
    pub finish_tick: Option<u32>,
    /// Exact pre-tick removal boundary; finished cars cannot be disqualified.
    pub dnf_tick: Option<u32>,
    /// Final unwrapped progress.
    pub progress_mm: i64,
}

/// Public outcome recomputed by the native execution relation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonDeserialize, DeriveJsonSerialize))]
pub struct RaceResultV1 {
    /// Exact number of simulated ticks.
    pub ticks: u32,
    /// Finishers by ascending finish tick, then DNFs by descending progress; slot breaks display ties.
    pub standings: Vec<RaceStandingV1>,
    /// Every slot tied for the earliest finish; empty when nobody finished.
    pub winners: Vec<u8>,
}

/// Ledger-bound statement for a native RaceV1 execution proof.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonDeserialize, DeriveJsonSerialize))]
pub struct RacePublicInputsV1 {
    /// Exact runtime network, preventing cross-network replay.
    pub network_id: NetworkId,
    /// One immutable race identifier.
    pub race_id: Hash,
    /// Commitment to the canonical participant and session-key roster.
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

/// Exact first-release native execution proof envelope.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonDeserialize, DeriveJsonSerialize))]
pub struct ExecutionProofEnvelopeV1 {
    /// Envelope version; exactly one.
    pub version: u16,
    /// Immutable compiled verifier/profile digest.
    pub profile_id: Hash,
    /// Exact typed public statement.
    pub statement: RacePublicInputsV1,
    /// Canonical bounded native STARK proof payload.
    pub proof_bytes: Vec<u8>,
}

/// Public replay availability and the exact native cryptographic execution proof.
/// Replay data is intentionally public, allowing any replaceable worker to prove the race.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonDeserialize, DeriveJsonSerialize))]
pub struct RaceProofPayloadV1 {
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonDeserialize, DeriveJsonSerialize))]
pub struct RaceProverRequestV1 {
    /// Complete network and ledger-bound claim.
    pub statement: RacePublicInputsV1,
    /// Available canonical input transcript.
    pub replay: RaceReplayV1,
    /// Claimed state at the retained signed checkpoint, when present.
    pub checkpoint_state: Option<RaceStateV1>,
}

/// A compiled, versioned execution relation; arbitrary circuit or executable installation is absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonDeserialize, DeriveJsonSerialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum ExecutionProofRelationV1 {
    /// The complete bounded deterministic arcade-race transition relation.
    RaceV1,
}

/// Exact native descriptor retained by the compiled execution-profile registry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonDeserialize, DeriveJsonSerialize))]
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
    /// Absolute cryptographic wire bound; transport bounds are separate admission conditions.
    pub maximum_proof_bytes: u32,
    /// True only after native correctness, cryptographic, and resource release gates pass.
    pub qualified: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
