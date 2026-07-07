//! Durable execution routing context committed by a block header.

use iroha_crypto::{Hash, HashOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    block::consensus::{NativeAmxReceipt, SumeragiLanePayloadOwnership},
    nexus::{DataSpaceId, LaneId},
    transaction::signed::TransactionEntrypoint,
};

/// Role of one route leg in an external execution plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[norito(tag = "role", content = "detail", rename_all = "snake_case")]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub enum ExternalExecutionRouteRole {
    /// The route coordinates final admission and commit ordering for the plan.
    Coordinator,
    /// The route prepares or commits one dataspace-local leg of the plan.
    Participant,
}

/// Lane/dataspace leg committed as part of an external execution plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// Native AMX receipt collected for this routed entrypoint, when the plan spans dataspaces.
    #[norito(default)]
    pub native_amx_receipt: Option<NativeAmxReceipt>,
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
            native_amx_receipt: None,
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
            native_amx_receipt: None,
        }
    }

    /// Attach a native AMX receipt to this execution context.
    #[must_use]
    pub fn with_native_amx_receipt(mut self, receipt: NativeAmxReceipt) -> Self {
        self.native_amx_receipt = Some(receipt);
        self
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
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct BlockExecutionContextBundle {
    /// Routing context entries aligned with the block's external entrypoints.
    pub external: Vec<ExternalExecutionContext>,
    /// Lane-local payload ownership and RBC instance identities aligned by block entrypoint index.
    #[norito(default)]
    pub lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
}

impl BlockExecutionContextBundle {
    /// Construct an ordered execution context bundle.
    #[must_use]
    pub const fn new(external: Vec<ExternalExecutionContext>) -> Self {
        Self {
            external,
            lane_payload_ownerships: Vec::new(),
        }
    }

    /// Attach lane-local payload ownership identities to this bundle.
    #[must_use]
    pub fn with_lane_payload_ownerships(
        mut self,
        lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
    ) -> Self {
        self.lane_payload_ownerships = lane_payload_ownerships;
        self
    }

    /// Returns true when the bundle carries no execution context.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.external.is_empty() && self.lane_payload_ownerships.is_empty()
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
    fn block_execution_context_bundle_roundtrips_lane_payload_ownerships() {
        let lane_id = LaneId::new(2);
        let dataspace_id = DataSpaceId::new(9);
        let ownership = SumeragiLanePayloadOwnership {
            proposal_height: 7,
            proposal_view: 3,
            lane_id,
            dataspace_id,
            lane_block_height: 5,
            lane_block_view: 1,
            subject_hash: Hash::new(b"lane-subject"),
            qc_mode_tag: "test-lane-qc-mode".to_string(),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::new(b"entrypoint")],
            previous_lane_block_height: 4,
            previous_lane_block_descriptor_hash: Some(Hash::new(b"previous-lane-block-descriptor")),
            lane_block_descriptor_hash: Some(Hash::new(b"lane-block-descriptor")),
            lane_block_descriptor_validator_set: Vec::new(),
            lane_block_descriptor_validator_count: 0,
            lane_block_descriptor_min_quorum: 0,
            payload_ownership_hash: Hash::new(b"lane-payload-ownership"),
            rbc_instance_hash: Hash::new(b"lane-rbc-instance"),
        };
        let bundle = BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
            entrypoint_hash(b"entrypoint"),
            lane_id,
            dataspace_id,
        )])
        .with_lane_payload_ownerships(vec![ownership.clone()]);

        let encoded = norito::to_bytes(&bundle).expect("bundle encodes");
        let decoded: BlockExecutionContextBundle =
            norito::decode_from_bytes(&encoded).expect("bundle decodes with lane ownership");

        assert_eq!(decoded.lane_payload_ownerships, vec![ownership]);
        assert!(!decoded.is_empty());
    }

    #[test]
    fn block_execution_context_bundle_with_only_ownership_is_not_empty() {
        let ownership = SumeragiLanePayloadOwnership {
            proposal_height: 1,
            proposal_view: 0,
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            lane_block_height: 1,
            lane_block_view: 0,
            subject_hash: Hash::new(b"subject"),
            qc_mode_tag: "test-lane-qc-mode".to_string(),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::new(b"entrypoint")],
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_descriptor_hash: Some(Hash::new(b"descriptor")),
            lane_block_descriptor_validator_set: Vec::new(),
            lane_block_descriptor_validator_count: 0,
            lane_block_descriptor_min_quorum: 0,
            payload_ownership_hash: Hash::new(b"payload"),
            rbc_instance_hash: Hash::new(b"rbc"),
        };

        assert!(
            !BlockExecutionContextBundle::new(Vec::new())
                .with_lane_payload_ownerships(vec![ownership])
                .is_empty()
        );
    }
}
