//! Lanes of the global chain (`specs/sumeragi_lanes.md`).
//!
//! A lane incarnation is a Sumeragi instance with a committee and chain parameters pinned at
//! creation ([`SumeragiLaneRecord`]). Its blocks are admission-checked transaction batches; the
//! global chain merges certified lane blocks by reference ([`SumeragiLaneMerge`]) and is the only
//! place world state changes. The lane set, its autoscale history ([`SumeragiLaneState`]) and the
//! governed policy ([`SumeragiLanePolicy`]) are committed global-chain state.

use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};
use iroha_primitives::json::Json;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    parameter::{
        custom::{CustomParameter, CustomParameterId},
        system::SumeragiParameters,
    },
};

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
    /// The anchor freshness bound `A`, pinned at creation: a lane block anchored below
    /// `h - A` is stale when merged at global height `h` (§4.3).
    pub anchor_freshness: u64,
    /// The highest merged lane block.
    pub merged: SumeragiLaneFrontier,
    /// Global height at which the frontier last advanced (`active_from` before any merge).
    pub merged_at: u64,
    /// Transactions routed to this lane that the global chain executed directly since
    /// `merged_at` (§6.4): load the lane is not serving.
    pub rescued: u64,
}

impl SumeragiLaneRecord {
    /// Whether the lane admits blocks anchored at global height `anchor`: active, and not at or
    /// after its closing height.
    #[must_use]
    pub fn admits_anchor(&self, anchor: u64) -> bool {
        self.active_from <= anchor && self.closing.is_none_or(|closing| anchor < closing)
    }

    /// Global height at which a closing lane retires (`c + A + 1`), or `None` while the lane is
    /// not closing.
    #[must_use]
    pub fn retirement_height(&self) -> Option<u64> {
        self.closing.map(|closing| {
            closing
                .saturating_add(self.anchor_freshness)
                .saturating_add(1)
        })
    }

    /// Whether a lane block anchored at `anchor` is stale when merged at global height
    /// `height` (`anchor < height - A`).
    #[must_use]
    pub fn is_stale(&self, anchor: u64, height: u64) -> bool {
        anchor.saturating_add(self.anchor_freshness) < height
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

/// The lane merge of a global block (`specs/sumeragi_lanes.md` §4): the merged lane ranges, the
/// time floor they impose on the block and, in the executed block, how many trailing entrypoints
/// came from them.
///
/// A proposal carries `merged_count = 0`; execution appends the admitted transactions of the
/// merged lane blocks to the block's entrypoints and records their number, so the executed block
/// runs through the ordinary execution pipeline and the proposal is recovered by removing that
/// suffix. The block's canonical time is at least `time_floor_ms`, so every merged transaction
/// precedes the block that executes it.
#[derive(
    Clone,
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
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneMergeSection")]
pub struct SumeragiLaneMergeSection {
    /// Merged lane ranges, lanes ascending.
    pub merges: Vec<SumeragiLaneMerge>,
    /// One millisecond after the latest creation time among the transactions of the fresh
    /// merged lane blocks (`0` when they carry none).
    pub time_floor_ms: u64,
    /// Number of trailing block entrypoints that come from the merged lane blocks.
    pub merged_count: u32,
}

/// One global block's load sample (§6.1): the transactions the default-route lanes carried
/// and the capacity they offered.
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
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneSample")]
pub struct SumeragiLaneSample {
    /// Global height of the block.
    pub height: u64,
    /// Its creation time (ms since the Unix epoch).
    pub time_ms: u64,
    /// Transactions it executed from lane `0` and the elastic lanes.
    pub transactions: u64,
    /// Default-route lanes (lane `0` and the admitted elastic lanes) at its height.
    pub lanes: u32,
}

/// The committed lane set of the global chain and its autoscale history.
#[derive(
    Clone,
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
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneState")]
pub struct SumeragiLaneState {
    /// Lane records, lanes ascending; retired lanes are removed.
    pub lanes: Vec<SumeragiLaneRecord>,
    /// Recent load samples, oldest first.
    pub samples: Vec<SumeragiLaneSample>,
    /// Global height of the last autoscale transition (`0`: none yet).
    pub last_transition: u64,
    /// Incarnations created so far; the next one derives from this counter, so none repeats.
    pub incarnations: u64,
}

impl SumeragiLaneState {
    /// The record of `lane`.
    #[must_use]
    pub fn lane(&self, lane: LaneId) -> Option<&SumeragiLaneRecord> {
        self.lanes
            .binary_search_by_key(&lane, |record| record.lane)
            .ok()
            .map(|index| &self.lanes[index])
    }

    /// The record of `lane`, mutably.
    pub fn lane_mut(&mut self, lane: LaneId) -> Option<&mut SumeragiLaneRecord> {
        self.lanes
            .binary_search_by_key(&lane, |record| record.lane)
            .ok()
            .map(|index| &mut self.lanes[index])
    }

    /// Insert or replace a record, keeping lanes ascending.
    pub fn upsert(&mut self, record: SumeragiLaneRecord) {
        match self
            .lanes
            .binary_search_by_key(&record.lane, |existing| existing.lane)
        {
            Ok(index) => self.lanes[index] = record,
            Err(index) => self.lanes.insert(index, record),
        }
    }
}

/// A lane incarnation as the node serves it (`specs/sumeragi_lanes.md` §8): the committed record
/// and the status of the node's instance of it.
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
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneStatus")]
pub struct SumeragiLaneStatus {
    /// The committed lifecycle record.
    pub record: SumeragiLaneRecord,
    /// The node's instance (`None` while it does not run one: before activation, or when its
    /// instance failed to start).
    #[norito(required)]
    pub instance: Option<crate::sumeragi::SumeragiStatus>,
}

/// A fixed lane of the policy: created with this committee, recreated after it retires while
/// the policy still lists it, closed once the policy no longer lists it (§2.1).
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
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiFixedLane")]
pub struct SumeragiFixedLane {
    /// The lane (never `0`, never in the autoscale range).
    pub lane: LaneId,
    /// The dataspace owning the lane.
    pub dataspace: DataSpaceId,
    /// The committee pinned into each incarnation.
    pub committee: Vec<SumeragiLaneMember>,
}

/// An explicit routing rule (§5.1): a transaction matching every present matcher goes to `lane`
/// (lane `0` or a fixed lane).
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
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneRoute")]
pub struct SumeragiLaneRoute {
    /// The target lane.
    pub lane: LaneId,
    /// Authority matcher: an account id, an encoded account id or an account alias.
    #[norito(required)]
    pub account: Option<String>,
    /// Instruction matcher: an instruction type name or a `Type::...` path prefix.
    #[norito(required)]
    pub instruction: Option<String>,
}

/// Elastic lane autoscaling (§6): utilization of the default-route lanes over a window of
/// committed samples opens or closes elastic lanes.
#[derive(
    Clone,
    Copy,
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
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneAutoscale")]
pub struct SumeragiLaneAutoscale {
    /// Lowest elastic lane id (inclusive, at least `1`).
    pub min_lane: LaneId,
    /// Elastic lane ids end here (exclusive).
    pub max_lane_exclusive: LaneId,
    /// The dataspace owning elastic lanes.
    pub dataspace: DataSpaceId,
    /// Members of an elastic lane committee, drawn from the global validators.
    pub committee_size: u32,
    /// Transactions per second one default-route lane is expected to carry.
    pub per_lane_target_tps: u32,
    /// Samples (global blocks) a decision reads.
    pub window: u32,
    /// Open a lane at or above this utilization (per mille).
    pub scale_out_permille: u32,
    /// Close the highest elastic lane below this utilization (per mille).
    pub scale_in_permille: u32,
    /// Global blocks between two transitions.
    pub cooldown: u64,
}

/// The governed lane policy of the global chain, a custom chain parameter
/// ([`Self::PARAMETER_ID_STR`]). Without it the chain has no lanes besides lane `0`.
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
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLanePolicy")]
pub struct SumeragiLanePolicy {
    /// Anchor freshness bound `A` pinned into new incarnations.
    pub anchor_freshness: u64,
    /// Lane blocks one global block merges per lane at most.
    pub max_merge_blocks: u32,
    /// Global blocks after which a lane with unserved load and no merge is closed (§6.4).
    pub stall_window: u64,
    /// Chain parameters pinned into new incarnations.
    pub lane_params: SumeragiParameters,
    /// Fixed lanes, lanes ascending.
    pub fixed: Vec<SumeragiFixedLane>,
    /// Explicit routing rules, in priority order.
    pub routes: Vec<SumeragiLaneRoute>,
    /// Elastic lane autoscaling, if enabled.
    #[norito(required)]
    pub autoscale: Option<SumeragiLaneAutoscale>,
}

/// Why a [`SumeragiLanePolicy`] is not valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SumeragiLanePolicyError {
    /// A bound that must be positive is zero.
    #[error("`{0}` must be positive")]
    Zero(&'static str),
    /// Fixed lanes are not strictly ascending, or one is lane `0`.
    #[error("fixed lanes must be strictly ascending and never lane 0")]
    FixedOrder,
    /// A fixed lane has no committee or repeats a member.
    #[error("fixed lane {0} needs a committee of distinct members")]
    FixedCommittee(LaneId),
    /// A fixed lane lies in the elastic range.
    #[error("fixed lane {0} lies in the elastic lane range")]
    FixedElastic(LaneId),
    /// A route targets a lane that is neither `0` nor fixed.
    #[error("route {0} targets a lane that is neither 0 nor fixed")]
    RouteTarget(usize),
    /// A route has no matcher.
    #[error("route {0} matches every transaction")]
    RouteMatcher(usize),
    /// The elastic range is empty or includes lane `0`.
    #[error("the elastic lane range must be non-empty and above lane 0")]
    ElasticRange,
    /// Scale-in does not stay below scale-out.
    #[error("scale_in_permille must be below scale_out_permille")]
    Hysteresis,
}

impl SumeragiLanePolicy {
    /// Initial governed lane policy for a chain adding its first physical dataspace.
    ///
    /// Existing policies retain their reviewed bounds. A chain without one starts with a
    /// 16-block anchor window, at most 16 merged blocks per lane, and a 64-block stall
    /// window. New lane instances inherit the chain's committed consensus parameters.
    #[must_use]
    pub fn for_chain(lane_params: SumeragiParameters) -> Self {
        Self {
            anchor_freshness: 16,
            max_merge_blocks: 16,
            stall_window: 64,
            lane_params,
            fixed: Vec::new(),
            routes: Vec::new(),
            autoscale: None,
        }
    }

    /// Identifier of the custom parameter holding the policy.
    pub const PARAMETER_ID_STR: &'static str = "sumeragi_lane_policy";

    /// The [`CustomParameterId`] of the policy.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("valid sumeragi lane policy parameter identifier")
    }

    /// The policy as a custom parameter.
    #[must_use]
    pub fn into_custom_parameter(self) -> CustomParameter {
        CustomParameter::new(Self::parameter_id(), Json::new(self))
    }

    /// Decode and validate the policy from `custom`; `None` when `custom` is another parameter.
    ///
    /// # Errors
    /// The payload does not decode or the policy is not valid.
    pub fn from_custom_parameter(custom: &CustomParameter) -> Option<Result<Self, String>> {
        if custom.id != Self::parameter_id() {
            return None;
        }
        Some(
            norito::json::from_str::<Self>(custom.payload().get())
                .map_err(|error| error.to_string())
                .and_then(|policy| {
                    policy.validate().map_err(|error| error.to_string())?;
                    Ok(policy)
                }),
        )
    }

    /// Whether `lane` is in the elastic range.
    #[must_use]
    pub fn is_elastic(&self, lane: LaneId) -> bool {
        self.autoscale.as_ref().is_some_and(|autoscale| {
            autoscale.min_lane <= lane && lane < autoscale.max_lane_exclusive
        })
    }

    /// The fixed lane entry of `lane`.
    #[must_use]
    pub fn fixed_lane(&self, lane: LaneId) -> Option<&SumeragiFixedLane> {
        self.fixed
            .binary_search_by_key(&lane, |fixed| fixed.lane)
            .ok()
            .map(|index| &self.fixed[index])
    }

    /// Check the policy's structural rules (the pinned chain parameters are checked by the
    /// consensus layer).
    ///
    /// # Errors
    /// See [`SumeragiLanePolicyError`].
    pub fn validate(&self) -> Result<(), SumeragiLanePolicyError> {
        if self.anchor_freshness == 0 {
            return Err(SumeragiLanePolicyError::Zero("anchor_freshness"));
        }
        if self.max_merge_blocks == 0 {
            return Err(SumeragiLanePolicyError::Zero("max_merge_blocks"));
        }
        if self.stall_window == 0 {
            return Err(SumeragiLanePolicyError::Zero("stall_window"));
        }
        if let Some(autoscale) = &self.autoscale {
            if autoscale.min_lane.as_u32() == 0
                || autoscale.min_lane >= autoscale.max_lane_exclusive
            {
                return Err(SumeragiLanePolicyError::ElasticRange);
            }
            for (value, name) in [
                (autoscale.committee_size, "committee_size"),
                (autoscale.per_lane_target_tps, "per_lane_target_tps"),
                (autoscale.window, "window"),
                (autoscale.scale_out_permille, "scale_out_permille"),
            ] {
                if value == 0 {
                    return Err(SumeragiLanePolicyError::Zero(name));
                }
            }
            if autoscale.scale_in_permille >= autoscale.scale_out_permille {
                return Err(SumeragiLanePolicyError::Hysteresis);
            }
        }
        let mut previous = LaneId::new(0);
        for fixed in &self.fixed {
            if fixed.lane <= previous {
                return Err(SumeragiLanePolicyError::FixedOrder);
            }
            previous = fixed.lane;
            if self.is_elastic(fixed.lane) {
                return Err(SumeragiLanePolicyError::FixedElastic(fixed.lane));
            }
            let mut peers = fixed
                .committee
                .iter()
                .map(|member| &member.peer)
                .collect::<Vec<_>>();
            peers.sort();
            peers.dedup();
            if peers.is_empty() || peers.len() != fixed.committee.len() {
                return Err(SumeragiLanePolicyError::FixedCommittee(fixed.lane));
            }
        }
        for (index, route) in self.routes.iter().enumerate() {
            if route.lane.as_u32() != 0 && self.fixed_lane(route.lane).is_none() {
                return Err(SumeragiLanePolicyError::RouteTarget(index));
            }
            if route.account.is_none() && route.instruction.is_none() {
                return Err(SumeragiLanePolicyError::RouteMatcher(index));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_dataspace_policy_inherits_chain_parameters_and_roundtrips() {
        let params = SumeragiParameters::default();
        let policy = SumeragiLanePolicy::for_chain(params.clone());
        assert_eq!(policy.lane_params, params);
        assert_eq!(
            (
                policy.anchor_freshness,
                policy.max_merge_blocks,
                policy.stall_window
            ),
            (16, 16, 64)
        );
        assert!(policy.fixed.is_empty() && policy.routes.is_empty() && policy.autoscale.is_none());
        policy.validate().unwrap();
        let encoded = norito::encode_canonical(&policy).unwrap();
        let decoded: SumeragiLanePolicy = norito::decode_canonical(&encoded).unwrap();
        assert_eq!(decoded, policy);
        assert_eq!(
            SumeragiLanePolicy::from_custom_parameter(&policy.clone().into_custom_parameter())
                .unwrap()
                .unwrap(),
            policy
        );
    }

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
            anchor_freshness: 16,
            merged: SumeragiLaneFrontier::default(),
            merged_at: 12,
            rescued: 0,
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
        assert_eq!(closing.retirement_height(), Some(37));
        assert_eq!(open.retirement_height(), None);
        // A block anchored at 4 is fresh when merged at 20 (4 + 16), stale at 21.
        assert!(!open.is_stale(4, 20));
        assert!(open.is_stale(4, 21));
    }

    fn member(seed: u8) -> SumeragiLaneMember {
        let pair =
            iroha_crypto::KeyPair::from_seed(vec![seed; 32], iroha_crypto::Algorithm::BlsNormal);
        SumeragiLaneMember {
            peer: PeerId::new(pair.public_key().clone()),
            pop: Vec::new(),
        }
    }

    fn policy() -> SumeragiLanePolicy {
        SumeragiLanePolicy {
            anchor_freshness: 16,
            max_merge_blocks: 32,
            stall_window: 256,
            lane_params: SumeragiParameters::default(),
            fixed: vec![SumeragiFixedLane {
                lane: LaneId::new(2),
                dataspace: DataSpaceId::new(0),
                committee: vec![member(1), member(2)],
            }],
            routes: vec![SumeragiLaneRoute {
                lane: LaneId::new(2),
                account: None,
                instruction: Some("Log".to_owned()),
            }],
            autoscale: Some(SumeragiLaneAutoscale {
                min_lane: LaneId::new(16),
                max_lane_exclusive: LaneId::new(32),
                dataspace: DataSpaceId::new(0),
                committee_size: 4,
                per_lane_target_tps: 500,
                window: 32,
                scale_out_permille: 750,
                scale_in_permille: 300,
                cooldown: 64,
            }),
        }
    }

    #[test]
    fn policies_validate_their_structure() {
        let valid = policy();
        assert_eq!(valid.validate(), Ok(()));
        assert!(valid.is_elastic(LaneId::new(16)));
        assert!(!valid.is_elastic(LaneId::new(32)));
        let mut elastic_fixed = valid.clone();
        elastic_fixed.fixed[0].lane = LaneId::new(20);
        elastic_fixed.routes.clear();
        assert_eq!(
            elastic_fixed.validate(),
            Err(SumeragiLanePolicyError::FixedElastic(LaneId::new(20)))
        );
        let mut repeated = valid.clone();
        repeated.fixed[0].committee.push(member(1));
        assert_eq!(
            repeated.validate(),
            Err(SumeragiLanePolicyError::FixedCommittee(LaneId::new(2)))
        );
        let mut stray_route = valid.clone();
        stray_route.routes[0].lane = LaneId::new(3);
        assert_eq!(
            stray_route.validate(),
            Err(SumeragiLanePolicyError::RouteTarget(0))
        );
        let mut inverted = valid.clone();
        inverted.autoscale.as_mut().unwrap().scale_in_permille = 800;
        assert_eq!(
            inverted.validate(),
            Err(SumeragiLanePolicyError::Hysteresis)
        );
        let mut zero = valid;
        zero.anchor_freshness = 0;
        assert_eq!(
            zero.validate(),
            Err(SumeragiLanePolicyError::Zero("anchor_freshness"))
        );
    }

    #[test]
    fn policies_roundtrip_through_their_custom_parameter() {
        let value = policy();
        let custom = value.clone().into_custom_parameter();
        assert_eq!(
            SumeragiLanePolicy::from_custom_parameter(&custom),
            Some(Ok(value))
        );
    }

    #[test]
    fn lane_statuses_roundtrip_with_and_without_an_instance() {
        let running = SumeragiLaneStatus {
            record: record(None),
            instance: Some(crate::sumeragi::SumeragiStatus {
                protocol_version: crate::sumeragi::PROTOCOL_VERSION,
                config_fingerprint: iroha_crypto::Hash::new(b"lane status fixture config"),
                beacon_horizon: None,
                instance: [5; 32],
                height: 4,
                view: 0,
                stage: 0,
                leader: None,
                proxy_tail: None,
                high_qc_view: None,
                level: 0,
                start_level: 0,
                t_retx_ms: 100,
                committed_height: 3,
                applied_height: 3,
                awaiting: false,
                signer: None,
                unanchored: false,
                abstaining: true,
                halted: None,
                footprint: crate::sumeragi::SumeragiFootprint::default(),
            }),
        };
        for status in [
            running.clone(),
            SumeragiLaneStatus {
                instance: None,
                ..running
            },
        ] {
            let bytes = status.encode();
            let decoded =
                <SumeragiLaneStatus as norito::codec::DecodeAll>::decode_all(&mut bytes.as_slice())
                    .expect("decode");
            assert_eq!(decoded, status);
            let json = norito::json::to_json(&status).expect("json");
            assert_eq!(
                norito::json::from_json::<SumeragiLaneStatus>(&json).expect("from json"),
                status
            );
        }
    }

    #[test]
    fn lane_state_keeps_lanes_ascending() {
        let mut state = SumeragiLaneState::default();
        for lane in [5, 2, 9, 2] {
            state.upsert(SumeragiLaneRecord {
                lane: LaneId::new(lane),
                ..record(None)
            });
        }
        let lanes = state
            .lanes
            .iter()
            .map(|r| r.lane.as_u32())
            .collect::<Vec<_>>();
        assert_eq!(lanes, vec![2, 5, 9]);
        assert!(state.lane(LaneId::new(5)).is_some());
        assert!(state.lane(LaneId::new(4)).is_none());
        state.lane_mut(LaneId::new(9)).unwrap().rescued = 3;
        assert_eq!(state.lane(LaneId::new(9)).unwrap().rescued, 3);
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
