//! Lanes of the global chain (`specs/sumeragi_lanes.md`).
//!
//! A lane incarnation is a Sumeragi instance with a committee and chain parameters pinned at
//! creation ([`SumeragiLaneRecord`]). Its blocks are admission-checked transaction batches; the
//! global chain merges certified lane blocks by reference ([`SumeragiLaneMerge`]) and is the only
//! place world state changes.

use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, parameter::system::SumeragiParameters};

/// One pinned member of a lane committee: its peer (BLS-normal consensus key) and the proof of
/// possession admitted when the lane was created.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneMember")]
pub struct SumeragiLaneMember {
    /// The member's peer identity (its consensus key).
    pub peer: PeerId,
    /// The member's proof of possession of that key.
    #[norito(
        with = "crate::json_helpers::base64_vec",
        bounded_with = "crate::json_helpers::base64_vec::serialize_bounded"
    )]
    pub pop: Vec<u8>,
}

/// The highest lane block the global chain has merged.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneFrontier")]
pub struct SumeragiLaneFrontier {
    /// Lane height of the highest merged block (`0`: nothing merged yet, the lane genesis).
    pub height: u64,
    /// Core block hash of that block.
    pub block_hash: [u8; 32],
    /// Its certified execution result `R`.
    pub result: [u8; 32],
}

/// The lifecycle record of one lane incarnation, kept in the global chain's world state.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneRecord")]
pub struct SumeragiLaneRecord {
    /// The lane.
    pub lane: LaneId,
    /// The dataspace owning the lane.
    pub dataspace: DataSpaceId,
    /// The incarnation: never reused, so a recreated lane is a new instance.
    pub incarnation: [u8; 32],
    /// Chain parameters of the lane instance, pinned for the whole incarnation.
    pub params: SumeragiParameters,
    /// The committee, in canonical order, pinned for the whole incarnation.
    pub committee: Vec<SumeragiLaneMember>,
    /// Global height of the block that created the record.
    pub created_at: u64,
    /// First global height at which the lane is active (`created_at + 2`).
    pub active_from: u64,
    /// Closing height `c`, once the lane is closing.
    #[norito(required)]
    pub closing: Option<u64>,
    /// The highest merged lane block.
    pub merged: SumeragiLaneFrontier,
}

impl SumeragiLaneRecord {
    /// Whether the lane admits blocks anchored at global height `anchor`: active, and not at or
    /// after its closing height.
    #[must_use]
    pub fn admits_anchor(&self, anchor: u64) -> bool {
        self.active_from <= anchor && self.closing.is_none_or(|closing| anchor < closing)
    }

    /// Global height at which a closing lane retires (`c + A + 1`, `A` = the anchor freshness
    /// bound), or `None` while the lane is not closing.
    #[must_use]
    pub fn retirement_height(&self, freshness: u64) -> Option<u64> {
        self.closing
            .map(|closing| closing.saturating_add(freshness).saturating_add(1))
    }
}

/// A global block's reference to the next contiguous certified blocks of one lane.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneMerge")]
pub struct SumeragiLaneMerge {
    /// The lane.
    pub lane: LaneId,
    /// Its incarnation.
    pub incarnation: [u8; 32],
    /// First merged lane height (the lane's merged frontier + 1).
    pub from: u64,
    /// Last merged lane height.
    pub to: u64,
    /// Core block hash of lane height `to`.
    pub tip_hash: [u8; 32],
    /// Certified result `R` of lane height `to`.
    pub tip_result: [u8; 32],
}

impl SumeragiLaneMerge {
    /// Number of lane blocks merged.
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.to.saturating_sub(self.from).saturating_add(1)
    }

    /// Whether the range is empty (`to < from`), which no valid merge has.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.to < self.from
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(closing: Option<u64>) -> SumeragiLaneRecord {
        SumeragiLaneRecord {
            lane: LaneId::new(3),
            dataspace: DataSpaceId::new(0),
            incarnation: [7; 32],
            params: SumeragiParameters::default(),
            committee: Vec::new(),
            created_at: 10,
            active_from: 12,
            closing,
            merged: SumeragiLaneFrontier::default(),
        }
    }

    #[test]
    fn anchors_are_admitted_from_activation_until_closing() {
        let open = record(None);
        assert!(!open.admits_anchor(11));
        assert!(open.admits_anchor(12));
        assert!(open.admits_anchor(u64::MAX));
        let closing = record(Some(20));
        assert!(closing.admits_anchor(19));
        assert!(!closing.admits_anchor(20));
        assert_eq!(closing.retirement_height(16), Some(37));
        assert_eq!(open.retirement_height(16), None);
    }

    #[test]
    fn merge_ranges_count_blocks() {
        let merge = SumeragiLaneMerge {
            lane: LaneId::new(1),
            incarnation: [1; 32],
            from: 4,
            to: 6,
            tip_hash: [2; 32],
            tip_result: [3; 32],
        };
        assert_eq!(merge.len(), 3);
        assert!(!merge.is_empty());
        assert!(SumeragiLaneMerge { to: 3, ..merge }.is_empty());
    }

    #[test]
    fn records_roundtrip_through_norito_and_json() {
        let value = record(Some(5));
        let bytes = value.encode();
        let decoded =
            <SumeragiLaneRecord as norito::codec::DecodeAll>::decode_all(&mut bytes.as_slice())
                .expect("decode");
        assert_eq!(decoded, value);
        let json = norito::json::to_json(&value).expect("json");
        assert_eq!(
            norito::json::from_json::<SumeragiLaneRecord>(&json).expect("from json"),
            value
        );
    }
}
