//! Canonical routing and QueuePlan admission input values.
//!
//! These values bind immutable intent, route, roster and first-admission claim
//! bytes. They do not prove journal fsync, registry insertion, lifecycle activity
//! or permission to reserve/sign/apply. Those policies remain owned by Core.
//! Unreleased schema names intentionally use this sole model owner; semantic
//! request, binding and routing hash domains are unchanged. No legacy decoder.

use crate::{block::BlockHeader, consensus::MAX_LANE_CONSENSUS_VALIDATORS};
use iroha_crypto::{Hash, HashOf};
use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};
use norito::codec::{Decode, Encode};
use std::collections::BTreeSet;

/// Maximum participant legs represented by one admitted Native AMX claim.
/// Core statically checks this against the native protocol's existing bound.
pub const MAX_QUEUE_PLAN_NATIVE_AMX_PARTICIPANTS_V1: usize = 255;

/// Canonical lane/dataspace routing decision carried by an admitted input.
#[derive(
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_admission::RoutingDecision")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct RoutingDecision {
    /// Lane assigned to the transaction.
    pub lane_id: LaneId,
    /// Dataspace assigned to the transaction.
    pub dataspace_id: DataSpaceId,
}
impl RoutingDecision {
    /// Create a new routing decision.
    #[must_use]
    pub const fn new(lane_id: LaneId, dataspace_id: DataSpaceId) -> Self {
        Self {
            lane_id,
            dataspace_id,
        }
    }
}
impl Default for RoutingDecision {
    fn default() -> Self {
        Self::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
    }
}
/// Role of one route in a transaction routing plan.
#[derive(
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_admission::RouteLegRole")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(tag = "role", content = "value", deny_unknown_fields)]
pub enum RouteLegRole {
    /// The route coordinates final admission and commit ordering for the plan.
    Coordinator,
    /// The route prepares or commits one dataspace-local leg of the plan.
    Participant,
}
/// One lane/dataspace leg in a transaction routing plan.
#[derive(
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_admission::RouteLeg")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct RouteLeg {
    /// Lane and dataspace selected for this leg.
    pub route: RoutingDecision,
    /// Plan role assigned to the leg.
    pub role: RouteLegRole,
}
impl RouteLeg {
    /// Construct a new route leg.
    #[must_use]
    pub const fn new(route: RoutingDecision, role: RouteLegRole) -> Self {
        Self { route, role }
    }
}
/// Native AMX routing plan for a transaction that touches multiple dataspaces.
#[derive(
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_admission::NativeAmxRoutingPlan")]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct NativeAmxRoutingPlan {
    /// Stable digest of the coordinator and participant route set.
    pub plan_digest: Hash,
    /// Coordinator route for the native AMX plan.
    pub coordinator: RouteLeg,
    /// Dataspace-local participant routes sorted by dataspace and lane id.
    pub participants: Vec<RouteLeg>,
}
/// Complete routing plan for a transaction.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_admission::RoutingPlan")]
#[norito(tag = "kind", content = "value", deny_unknown_fields)]
pub enum RoutingPlan {
    /// The transaction executes on one lane/dataspace route.
    Single(RouteLeg),
    /// The transaction requires native AMX coordination across dataspaces.
    NativeAmx(NativeAmxRoutingPlan),
}
impl RoutingPlan {
    /// Construct a single-route coordinator plan.
    #[must_use]
    pub const fn single(route: RoutingDecision) -> Self {
        Self::Single(RouteLeg::new(route, RouteLegRole::Coordinator))
    }
    /// Construct a canonical native AMX plan.
    #[must_use]
    pub fn native_amx(coordinator: RoutingDecision, mut participants: Vec<RouteLeg>) -> Self {
        participants.sort_by_key(|leg| (leg.route.dataspace_id, leg.route.lane_id));
        participants.dedup_by_key(|leg| (leg.route.dataspace_id, leg.route.lane_id));
        for leg in &mut participants {
            leg.role = RouteLegRole::Participant;
        }
        let plan_digest = native_amx_plan_digest(coordinator, &participants);
        Self::NativeAmx(NativeAmxRoutingPlan {
            plan_digest,
            coordinator: RouteLeg::new(coordinator, RouteLegRole::Coordinator),
            participants,
        })
    }
    /// Return the route that existing single-route queue machinery should use as coordinator.
    #[must_use]
    pub const fn coordinator_route(&self) -> RoutingDecision {
        match self {
            Self::Single(leg) => leg.route,
            Self::NativeAmx(plan) => plan.coordinator.route,
        }
    }
    /// Return the coordinator leg.
    #[must_use]
    pub const fn coordinator_leg(&self) -> RouteLeg {
        match self {
            Self::Single(leg) => *leg,
            Self::NativeAmx(plan) => plan.coordinator,
        }
    }
    /// Return all plan legs in deterministic coordinator-first order.
    #[must_use]
    pub fn legs(&self) -> Vec<RouteLeg> {
        match self {
            Self::Single(leg) => vec![*leg],
            Self::NativeAmx(plan) => {
                let mut legs = Vec::with_capacity(plan.participants.len().saturating_add(1));
                legs.push(plan.coordinator);
                legs.extend(plan.participants.iter().copied());
                legs
            }
        }
    }
    /// Return the deterministic digest for the plan.
    #[must_use]
    pub fn digest(&self) -> Hash {
        match self {
            Self::Single(leg) => routing_plan_digest(&[leg.route]),
            Self::NativeAmx(plan) => plan.plan_digest,
        }
    }
}

fn routing_plan_digest(routes: &[RoutingDecision]) -> Hash {
    let mut bytes = Vec::with_capacity(16 + routes.len() * 12);
    bytes.extend_from_slice(b"iroha:routing-plan:v1");
    for route in routes {
        bytes.extend_from_slice(&route.lane_id.as_u32().to_le_bytes());
        bytes.extend_from_slice(&route.dataspace_id.as_u64().to_le_bytes());
    }
    Hash::new(bytes)
}
fn native_amx_plan_digest(coordinator: RoutingDecision, participants: &[RouteLeg]) -> Hash {
    let mut bytes = Vec::with_capacity(24 + participants.len() * 13);
    bytes.extend_from_slice(b"iroha:native-amx-plan:v1");
    bytes.push(0);
    bytes.extend_from_slice(&coordinator.lane_id.as_u32().to_le_bytes());
    bytes.extend_from_slice(&coordinator.dataspace_id.as_u64().to_le_bytes());
    for participant in participants {
        bytes.push(1);
        bytes.extend_from_slice(&participant.route.lane_id.as_u32().to_le_bytes());
        bytes.extend_from_slice(&participant.route.dataspace_id.as_u64().to_le_bytes());
    }
    Hash::new(bytes)
}

/// Version of the queue-plan lifecycle context embedded in durable admission claims.
pub const QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V1: u16 = 1;
/// Version of the global admission identity embedded in a strict queue-plan journal record.
pub const QUEUE_PLAN_GLOBAL_ADMISSION_IDENTITY_VERSION_V1: u16 = 1;
/// Exact-network/request identity chosen once by ingress before any authority acquires queue ownership.
///
/// This identity is persisted inside the exact journal record. Together with the record's
/// canonical enqueue timestamp and claim digest it lets restart recovery reconstruct the same
/// global admission binding that every authority attested.
#[derive(
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(
    name = "iroha_data_model::block::lane_admission::QueuePlanGlobalAdmissionIdentityV1"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct QueuePlanGlobalAdmissionIdentityV1 {
    /// Identity layout version.
    pub version: u16,
    /// Domain-separated digest of the exact network identifier.
    pub network_id_digest: Hash,
    /// Deterministic `QueuePlanSynced` proxy request identity.
    pub request_id: Hash,
}
/// One routing leg paired with the exact active lane incarnation that admitted it.
#[derive(
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_admission::QueuePlanRouteIncarnationV1")]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct QueuePlanRouteIncarnationV1 {
    /// Coordinator or participant route in canonical routing-plan order.
    pub leg: RouteLeg,
    /// Non-zero lane incarnation active for this route at `proposal_height`.
    pub lane_incarnation: Hash,
    /// Version of the canonical ordered validator-set hash.
    pub validator_set_hash_version: u16,
    /// Typed digest of the ordered authoritative roster at `proposal_height`.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Exact ordered authoritative roster at `proposal_height`.
    pub validator_set: Vec<PeerId>,
    /// Number of distinct identities in the authoritative roster.
    pub validator_count: u16,
    /// Minimum distinct durable attestations needed to include at least one honest copy.
    pub durability_threshold: u16,
}
/// Generation-stable lifecycle context for one queue-plan admission attempt.
#[derive(
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::lane_admission::QueuePlanAdmissionContextV1")]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct QueuePlanAdmissionContextV1 {
    /// Context layout version.
    pub version: u16,
    /// Canonical committed height used to resolve the routing plan.
    pub authority_height: u64,
    /// Contiguous next proposal height at which all route incarnations are active.
    pub proposal_height: u64,
    /// Exact committed tip that is the predecessor of `proposal_height`.
    #[norito(required)]
    pub predecessor_block_hash: Option<HashOf<BlockHeader>>,
    /// Digest of the complete coordinator/participant routing plan.
    pub routing_plan_digest: Hash,
    /// Coordinator-first route/incarnation pairs for the complete plan.
    pub route_incarnations: Vec<QueuePlanRouteIncarnationV1>,
}
impl QueuePlanAdmissionContextV1 {
    /// Reconstruct the complete canonical routing plan carried by this context.
    ///
    /// # Errors
    /// Returns an error when the leg vector is empty or its roles/order cannot encode one
    /// canonical single-route or Native AMX plan.
    pub fn routing_plan(&self) -> Result<RoutingPlan, String> {
        let Some(coordinator) = self.route_incarnations.first() else {
            return Err("queue-plan admission context has no coordinator leg".to_owned());
        };
        if coordinator.leg.role != RouteLegRole::Coordinator {
            return Err("queue-plan admission context first leg is not the coordinator".to_owned());
        }
        if self.route_incarnations.len() == 1 {
            return Ok(RoutingPlan::single(coordinator.leg.route));
        }
        let participants = self
            .route_incarnations
            .iter()
            .skip(1)
            .map(|bound| {
                if bound.leg.role != RouteLegRole::Participant {
                    return Err(
                        "queue-plan admission context contains a non-participant trailing leg"
                            .to_owned(),
                    );
                }
                Ok(bound.leg)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RoutingPlan::native_amx(coordinator.leg.route, participants))
    }
    /// Validate this context against an exact canonical routing plan.
    ///
    /// # Errors
    /// Returns the first structural, lifecycle, roster, or redundant-field mismatch.
    pub fn validate_for_routing_plan(&self, routing_plan: &RoutingPlan) -> Result<(), String> {
        let canonical_plan = match routing_plan {
            RoutingPlan::Single(leg) => RoutingPlan::single(leg.route),
            RoutingPlan::NativeAmx(plan) => {
                if plan.participants.is_empty()
                    || plan.participants.len() > MAX_QUEUE_PLAN_NATIVE_AMX_PARTICIPANTS_V1
                {
                    return Err(format!(
                        "queue-plan Native AMX participant count {} is outside 1..={}",
                        plan.participants.len(),
                        MAX_QUEUE_PLAN_NATIVE_AMX_PARTICIPANTS_V1
                    ));
                }
                RoutingPlan::native_amx(plan.coordinator.route, plan.participants.clone())
            }
        };
        if &canonical_plan != routing_plan {
            return Err(
                "queue-plan admission context is paired with a noncanonical routing plan"
                    .to_owned(),
            );
        }
        if self.version != QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V1 {
            return Err(format!(
                "unsupported queue-plan admission context version {}; expected {}",
                self.version, QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V1
            ));
        }
        let Some(expected_proposal_height) = self.authority_height.checked_add(1) else {
            return Err(
                "queue-plan admission authority height overflows proposal height".to_owned(),
            );
        };
        if self.proposal_height != expected_proposal_height {
            return Err(
                "queue-plan admission proposal height is not contiguous with authority height"
                    .to_owned(),
            );
        }
        if (self.authority_height == 0) != self.predecessor_block_hash.is_none() {
            return Err(
                "queue-plan admission predecessor hash presence does not match authority height"
                    .to_owned(),
            );
        }
        if self
            .predecessor_block_hash
            .is_some_and(|hash| hash_is_zero(Hash::from(hash)))
        {
            return Err(
                "queue-plan admission context contains a zero predecessor block hash".to_owned(),
            );
        }
        if self.routing_plan_digest != routing_plan.digest() {
            return Err(
                "queue-plan admission context digest does not match the exact routing plan"
                    .to_owned(),
            );
        }
        let legs = routing_plan.legs();
        if self.route_incarnations.len() != legs.len() {
            return Err(
                "queue-plan admission context does not bind every routing leg exactly once"
                    .to_owned(),
            );
        }
        for (bound, expected_leg) in self.route_incarnations.iter().zip(legs) {
            validate_route_incarnation(bound, expected_leg)?;
        }
        Ok(())
    }
}

fn validate_route_incarnation(
    bound: &QueuePlanRouteIncarnationV1,
    expected_leg: RouteLeg,
) -> Result<(), String> {
    if bound.leg != expected_leg {
        return Err(
            "queue-plan admission context legs are missing, reordered, or role-mismatched"
                .to_owned(),
        );
    }
    if hash_is_zero(bound.lane_incarnation) {
        return Err("queue-plan admission context contains a zero lane incarnation".to_owned());
    }
    if bound.validator_set_hash_version != crate::consensus::VALIDATOR_SET_HASH_VERSION_V1 {
        return Err(format!(
            "queue-plan admission validator-set hash version {} is unsupported",
            bound.validator_set_hash_version
        ));
    }
    if hash_is_zero(Hash::from(bound.validator_set_hash)) {
        return Err("queue-plan admission context contains a zero validator-set hash".to_owned());
    }
    let validator_count = bound.validator_set.len();
    if validator_count == 0 || validator_count > MAX_LANE_CONSENSUS_VALIDATORS {
        return Err(format!(
            "queue-plan admission validator count {validator_count} is outside 1..={MAX_LANE_CONSENSUS_VALIDATORS}"
        ));
    }
    if usize::from(bound.validator_count) != validator_count {
        return Err(format!(
            "queue-plan admission validator count {} does not equal exact roster length {validator_count}",
            bound.validator_count
        ));
    }
    if bound.validator_set.iter().collect::<BTreeSet<_>>().len() != validator_count {
        return Err(
            "queue-plan admission validator roster contains duplicate identities".to_owned(),
        );
    }
    if bound.validator_set_hash != HashOf::new(&bound.validator_set) {
        return Err(
            "queue-plan admission validator-set hash does not match the exact ordered roster"
                .to_owned(),
        );
    }
    let expected_threshold = validator_count.div_ceil(3);
    if usize::from(bound.durability_threshold) != expected_threshold {
        return Err(format!(
            "queue-plan admission durability threshold {} does not equal ceil({validator_count}/3)",
            bound.durability_threshold
        ));
    }
    Ok(())
}

fn hash_is_zero(hash: Hash) -> bool {
    hash == Hash::prehashed([0; Hash::LENGTH])
}

#[cfg(test)]
#[path = "lane_admission_tests.rs"]
mod tests;
