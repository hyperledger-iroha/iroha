//! Durable execution routing context committed by a block header.

use crate::transaction::signed::TransactionEntrypoint;
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::{Hash, HashOf};
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
/// Current-only first-release block execution-context bundle layout.
pub const BLOCK_EXECUTION_CONTEXT_BUNDLE_VERSION_V1: u8 = 1;
/// Role of one route leg in an external execution plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[norito(tag = "role", content = "detail", rename_all = "snake_case")]
#[derive(DeriveJsonSerialize, DeriveJsonDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::execution_context::ExternalExecutionRouteRole")]
pub enum ExternalExecutionRouteRole {
    /// The route coordinates final admission and commit ordering for the plan.
    Coordinator,
    /// The route prepares or commits one dataspace-local leg of the plan.
    Participant,
}
/// Lane/dataspace leg committed as part of an external execution plan.
#[derive(
    Debug,
    Clone,
    Copy,
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
#[norito_schema(name = "iroha_data_model::block::execution_context::ExternalExecutionRouteLeg")]
pub struct ExternalExecutionRouteLeg {
    /// Lane selected for this leg.
    pub lane_id: LaneId,
    /// Dataspace selected for this leg.
    pub dataspace_id: DataSpaceId,
    /// Role assigned to this leg.
    pub role: ExternalExecutionRouteRole,
}
impl ExternalExecutionRouteLeg {
    /// Construct an execution route leg.
    #[must_use]
    pub const fn new(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        role: ExternalExecutionRouteRole,
    ) -> Self {
        Self {
            lane_id,
            dataspace_id,
            role,
        }
    }
}
/// Routing context used to execute one external block entrypoint.
#[derive(
    Debug,
    Clone,
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
#[norito_schema(name = "iroha_data_model::block::execution_context::ExternalExecutionContext")]
pub struct ExternalExecutionContext {
    /// Hash of the external entrypoint this context belongs to.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Lane selected for execution.
    pub lane_id: LaneId,
    /// Dataspace selected for execution.
    pub dataspace_id: DataSpaceId,
    /// Deterministic digest of the full routing plan used for execution.
    pub routing_plan_digest: Hash,
    /// Full coordinator/participant route plan used for execution.
    pub routing_plan_legs: Vec<ExternalExecutionRouteLeg>,
}
impl ExternalExecutionContext {
    /// Construct routing context for one external entrypoint.
    #[must_use]
    pub fn new(
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
    ) -> Self {
        let routing_plan_legs = vec![ExternalExecutionRouteLeg::new(
            lane_id,
            dataspace_id,
            ExternalExecutionRouteRole::Coordinator,
        )];
        let routing_plan_digest = single_route_plan_digest(lane_id, dataspace_id);
        Self {
            entrypoint_hash,
            lane_id,
            dataspace_id,
            routing_plan_digest,
            routing_plan_legs,
        }
    }
    /// Construct routing context with a committed full routing plan.
    #[must_use]
    pub fn with_routing_plan(
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        routing_plan_digest: Hash,
        routing_plan_legs: Vec<ExternalExecutionRouteLeg>,
    ) -> Self {
        Self {
            entrypoint_hash,
            lane_id,
            dataspace_id,
            routing_plan_digest,
            routing_plan_legs,
        }
    }
}
fn single_route_plan_digest(lane_id: LaneId, dataspace_id: DataSpaceId) -> Hash {
    let mut bytes = Vec::with_capacity(16 + 12);
    bytes.extend_from_slice(b"iroha:routing-plan:v1");
    bytes.extend_from_slice(&lane_id.as_u32().to_le_bytes());
    bytes.extend_from_slice(&dataspace_id.as_u64().to_le_bytes());
    Hash::new(bytes)
}
/// Ordered execution context for external entrypoints in a block payload.
#[derive(
    Debug,
    Clone,
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
#[norito_schema(name = "iroha_data_model::block::execution_context::BlockExecutionContextBundle")]
pub struct BlockExecutionContextBundle {
    /// Exact first-release bundle layout. Only version one is supported.
    pub version: u8,
    /// Routing context entries aligned with the block's external entrypoints.
    pub external: Vec<ExternalExecutionContext>,
    /// The lane blocks this global block merges (`specs/sumeragi_lanes.md` §4.2).
    #[norito(required)]
    pub lane_merge: Option<crate::sumeragi_lanes::SumeragiLaneMergeSection>,
}
impl BlockExecutionContextBundle {
    /// Current supported bundle layout.
    pub const VERSION: u8 = BLOCK_EXECUTION_CONTEXT_BUNDLE_VERSION_V1;
    /// Return whether this bundle advertises the current first-release layout.
    #[must_use]
    pub const fn has_current_version(&self) -> bool {
        self.version == Self::VERSION
    }
    /// Construct an ordered execution context bundle.
    #[must_use]
    pub const fn new(external: Vec<ExternalExecutionContext>) -> Self {
        Self {
            version: Self::VERSION,
            external,
            lane_merge: None,
        }
    }
    /// Returns true when the bundle carries no execution context.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.external.is_empty() && self.lane_merge.is_none()
    }
}
impl Default for BlockExecutionContextBundle {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn entrypoint_hash(label: &[u8]) -> HashOf<TransactionEntrypoint> {
        HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(label))
    }
    #[test]
    fn external_execution_context_new_commits_single_route_plan() {
        let lane_id = LaneId::new(3);
        let dataspace_id = DataSpaceId::new(7);
        let context =
            ExternalExecutionContext::new(entrypoint_hash(b"entrypoint"), lane_id, dataspace_id);
        assert_eq!(context.lane_id, lane_id);
        assert_eq!(context.dataspace_id, dataspace_id);
        assert_eq!(
            context.routing_plan_digest,
            single_route_plan_digest(lane_id, dataspace_id)
        );
        assert_eq!(
            context.routing_plan_legs,
            vec![ExternalExecutionRouteLeg::new(
                lane_id,
                dataspace_id,
                ExternalExecutionRouteRole::Coordinator,
            )]
        );
    }
    #[test]
    fn external_execution_context_with_routing_plan_preserves_full_plan() {
        let lane_id = LaneId::new(1);
        let dataspace_id = DataSpaceId::new(7);
        let routing_plan_digest = Hash::new(b"native-amx-plan");
        let routing_plan_legs = vec![
            ExternalExecutionRouteLeg::new(
                lane_id,
                dataspace_id,
                ExternalExecutionRouteRole::Coordinator,
            ),
            ExternalExecutionRouteLeg::new(
                LaneId::new(2),
                DataSpaceId::new(8),
                ExternalExecutionRouteRole::Participant,
            ),
        ];
        let context = ExternalExecutionContext::with_routing_plan(
            entrypoint_hash(b"native-entrypoint"),
            lane_id,
            dataspace_id,
            routing_plan_digest,
            routing_plan_legs.clone(),
        );
        assert_eq!(context.routing_plan_digest, routing_plan_digest);
        assert_eq!(context.routing_plan_legs, routing_plan_legs);
    }

    #[test]
    fn execution_context_json_requires_every_nullable_slot() {
        let external = ExternalExecutionContext::new(
            entrypoint_hash(b"explicit-route-leg"),
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        );
        let mut unknown_leg = norito::json::to_value(&external.routing_plan_legs[0])
            .expect("serialize external route leg");
        unknown_leg
            .as_object_mut()
            .expect("external route leg JSON object")
            .insert("pre_release_field".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<ExternalExecutionRouteLeg>(unknown_leg).is_err(),
            "the first-release external route leg must reject unknown fields"
        );

        let bundle = BlockExecutionContextBundle::new(Vec::new());
        let mut missing =
            norito::json::to_value(&bundle).expect("serialize block execution context bundle");
        missing
            .as_object_mut()
            .expect("block execution context JSON object")
            .remove("lane_merge");
        assert!(
            norito::json::from_value::<BlockExecutionContextBundle>(missing).is_err(),
            "the first-release block context must require its nullable lane-merge slot"
        );
    }
}

#[cfg(test)]
mod captured_execution_context_schema_tests;
