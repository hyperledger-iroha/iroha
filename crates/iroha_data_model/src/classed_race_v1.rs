//! Distinct canonical model for the first equal-performance upgraded racing class.
//!
//! These types do not reinterpret RaceV1 data and do not authorize NFT ownership or custody.
//! This application model is separate from generic execution envelopes. Its compiled proof
//! profile and native equipment admission are not registered yet.

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Closed upgraded spec-class catalog; every participant uses the same selected class.
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
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ClassedRaceClassV1 {
    /// Touring S1: 10% higher acceleration and normal/boost speed limits than stock.
    TouringS1,
}

/// Versioned track catalog for this computation, independent of RaceV1's Rust implementation.
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
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ClassedRaceTrackV1 {
    /// Two-kilometre city circuit.
    NeonTokyo,
    /// 2.4-kilometre waterfront circuit.
    Harbor,
    /// 1.8-kilometre cherry-blossom circuit.
    Sakura,
}

/// All participants' six-bit controls at one consecutive simulation tick.
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
#[norito(deny_unknown_fields)]
pub struct ClassedRaceInputFrameV1 {
    /// Zero-based tick without gaps or repetitions.
    pub tick: u32,
    /// Permanent slot order; throttle, brake, left, right, drift, boost in bits 0..5.
    pub controls: Vec<u16>,
}

/// One consensus-authorized removal set applied before its exact simulation tick.
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
#[norito(deny_unknown_fields)]
pub struct ClassedRaceDnfEventV1 {
    /// Number of ticks completed before removal.
    pub tick: u32,
    /// Nonempty unique ascending permanent slots.
    pub slots: Vec<u8>,
}

/// Public complete or prefix replay; NFT authorization belongs to native admission.
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
#[norito(deny_unknown_fields)]
pub struct ClassedRaceReplayV1 {
    /// Wire/rules generation, exactly one.
    pub version: u16,
    /// One fixed performance vector for the entire grid; per-car tuning is absent.
    pub class_id: ClassedRaceClassV1,
    /// Immutable circuit.
    pub track: ClassedRaceTrackV1,
    /// One to eight for practice/replay; multiplayer admission requires two to eight.
    pub player_count: u8,
    /// Consecutive frames starting at zero, at most 5,400.
    pub frames: Vec<ClassedRaceInputFrameV1>,
    /// Unique ascending event ticks, bounded by the replay length.
    pub dnf_events: Vec<ClassedRaceDnfEventV1>,
}

/// State of one car. There is no cosmetic, NFT or caller-supplied performance multiplier.
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
#[norito(deny_unknown_fields)]
pub struct ClassedRaceCarStateV1 {
    /// Unwrapped longitudinal progress, including negative starting-grid offsets.
    pub progress_mm: i64,
    /// Signed road-relative lateral position after ordered contacts.
    pub lateral_mm: i32,
    /// Forward displacement per tick, bounded by the selected compiled class.
    pub speed_mm_per_tick: i32,
    /// Lateral displacement per tick before curvature.
    pub lateral_velocity_mm_per_tick: i32,
    /// Inclusive 0..1000 boost reserve for Touring S1.
    pub boost_energy: u16,
    /// First completed tick crossing the finish; preserved after a later key removal.
    pub finish_tick: Option<u32>,
    /// Exact pre-tick input-key removal boundary; freezes an unfinished car.
    pub dnf_tick: Option<u32>,
}

/// Exact bounded state at a tick boundary for one fixed class and track.
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
#[norito(deny_unknown_fields)]
pub struct ClassedRaceStateV1 {
    /// Number of completed simulation ticks.
    pub tick: u32,
    /// Performance selection shared by every car.
    pub class_id: ClassedRaceClassV1,
    /// Immutable track.
    pub track: ClassedRaceTrackV1,
    /// Permanent participant-slot order.
    pub cars: Vec<ClassedRaceCarStateV1>,
}

/// One display-ranked entry; slot breaks display ties only.
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
#[norito(deny_unknown_fields)]
pub struct ClassedRaceStandingV1 {
    /// Original permanent participant slot.
    pub slot: u8,
    /// Preserved finish tick, if any.
    pub finish_tick: Option<u32>,
    /// Consensus removal tick, if any.
    pub dnf_tick: Option<u32>,
    /// Exact final progress used for timeout ties.
    pub progress_mm: i64,
}

/// Native result; a nonterminal prefix is explicitly distinguished from a settleable outcome.
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
#[norito(deny_unknown_fields)]
pub struct ClassedRaceResultV1 {
    /// Exact class interpreted by this result.
    pub class_id: ClassedRaceClassV1,
    /// Exact circuit.
    pub track: ClassedRaceTrackV1,
    /// Completed simulation ticks.
    pub ticks: u32,
    /// Whether the reference reached its immutable terminal boundary.
    pub terminal: bool,
    /// Eligible finishers first, eligible unfinished racers next, then every forfeit.
    pub standings: Vec<ClassedRaceStandingV1>,
    /// Every exact tied winner in ascending permanent slot order; empty for a prefix or refund.
    pub winners: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classed_replay_has_its_own_exact_canonical_roundtrip() {
        let replay = ClassedRaceReplayV1 {
            version: 1,
            class_id: ClassedRaceClassV1::TouringS1,
            track: ClassedRaceTrackV1::Harbor,
            player_count: 2,
            frames: vec![ClassedRaceInputFrameV1 {
                tick: 0,
                controls: vec![33, 17],
            }],
            dnf_events: vec![ClassedRaceDnfEventV1 {
                tick: 1,
                slots: vec![1],
            }],
        };
        let bytes = norito::encode_canonical(&replay).expect("canonical classed replay");
        assert_eq!(
            norito::decode_canonical::<ClassedRaceReplayV1>(&bytes).unwrap(),
            replay
        );
        let bare = replay.encode();
        let mut input = bare.as_slice();
        assert_eq!(ClassedRaceReplayV1::decode(&mut input).unwrap(), replay);
        assert!(input.is_empty());
    }

    #[test]
    fn classed_state_and_terminal_result_roundtrip_all_optional_fields() {
        let state = ClassedRaceStateV1 {
            tick: 6,
            class_id: ClassedRaceClassV1::TouringS1,
            track: ClassedRaceTrackV1::NeonTokyo,
            cars: vec![ClassedRaceCarStateV1 {
                progress_mm: 6_000_000,
                lateral_mm: -1_800,
                speed_mm_per_tick: 3_300,
                lateral_velocity_mm_per_tick: -320,
                boost_energy: 0,
                finish_tick: Some(5),
                dnf_tick: Some(6),
            }],
        };
        let bytes = norito::encode_canonical(&state).unwrap();
        assert_eq!(
            norito::decode_canonical::<ClassedRaceStateV1>(&bytes).unwrap(),
            state
        );
        let result = ClassedRaceResultV1 {
            class_id: state.class_id,
            track: state.track,
            ticks: 6,
            terminal: true,
            standings: vec![ClassedRaceStandingV1 {
                slot: 0,
                finish_tick: Some(5),
                dnf_tick: Some(6),
                progress_mm: 6_000_000,
            }],
            winners: vec![0],
        };
        let bytes = norito::encode_canonical(&result).unwrap();
        assert_eq!(
            norito::decode_canonical::<ClassedRaceResultV1>(&bytes).unwrap(),
            result
        );
    }
}
