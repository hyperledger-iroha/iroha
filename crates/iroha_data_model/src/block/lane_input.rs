//! Canonical immutable admitted input shared by every route of one lane group.
//!
//! These wire values describe bytes and route slots. Core must authenticate the
//! first global carrier, certificate, exact current frozen contexts and all-route
//! head eligibility before granting body readiness or permission to vote. No
//! execution result or future economic-state assumption belongs in this payload.

use std::collections::BTreeMap;

use iroha_crypto::{Hash, HashOf};
use iroha_model_base::topology::{DataSpaceId, LaneId};
use norito::codec::{Decode, Encode};

use super::{
    BlockHeader, MAX_QUEUE_PLAN_ADMISSION_BYTES,
    lane_admission::{
        LaneAdmittedInputV1, MAX_QUEUE_PLAN_NATIVE_AMX_PARTICIPANTS_V1, RoutingDecision,
        RoutingPlan,
    },
    lane_consensus::{LaneDecisionV1, LanePhaseV1, LaneValueKindV1, QueuePlanAdmissionPriorityV1},
};

/// Single first-release immutable-input descriptor format.
pub const LANE_INPUT_VERSION_V1: u16 = 1;
/// One slot per distinct route, including the coordinator.
pub const MAX_LANE_INPUT_ROUTE_SLOTS: usize = MAX_QUEUE_PLAN_NATIVE_AMX_PARTICIPANTS_V1 + 1;
const DESCRIPTOR_DOMAIN: &[u8] = b"iroha:lane-consensus:input-descriptor:v1\0";

/// One route's exact frozen slot in the admitted atomic group.
///
/// A route that is both coordinator and participant has one slot here. All its
/// roles remain present in the input's single admission binding.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::block::lane_input::LaneInputRouteSlotV1")]
pub struct LaneInputRouteSlotV1 {
    /// Exact lane/dataspace identity from the immutable admission binding.
    pub route: RoutingDecision,
    /// Exact lifecycle incarnation of this route.
    pub lane_incarnation: Hash,
    /// Authenticated opening instance identity, without a live-authority grant.
    pub instance_id: Hash,
    /// Next lane frontier owned by this instance.
    pub lane_height: u64,
}

impl LaneInputRouteSlotV1 {
    /// Canonical distinct-route order, independent of coordinator/participant role.
    pub fn route_key(&self) -> (LaneId, DataSpaceId) {
        (self.route.lane_id, self.route.dataspace_id)
    }
}

/// Immutable source and complete route-slot list for one admitted group.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::block::lane_input::LaneInputDescriptorV1")]
pub struct LaneInputDescriptorV1 {
    /// Exact first-release descriptor version.
    pub version: u16,
    /// State-owned first global carrier position, preserved across admission replay.
    pub admission_priority: QueuePlanAdmissionPriorityV1,
    /// Canonical first carrier identity; the opening carrier may be later.
    pub admission_carrier_hash: HashOf<BlockHeader>,
    /// Hash of the exact canonical complete input retained at that carrier position.
    pub admitted_input_hash: Hash,
    /// Complete strict route order, with each distinct route represented once.
    pub slots: Vec<LaneInputRouteSlotV1>,
}

impl LaneInputDescriptorV1 {
    /// Check bounded shape only; arbitrary hashes do not authenticate authority.
    ///
    /// # Errors
    /// Rejects version, source position, zero identities and noncanonical slots.
    pub fn validate_structure(&self) -> Result<(), String> {
        if self.version != LANE_INPUT_VERSION_V1 {
            return Err("unsupported lane input descriptor version".to_owned());
        }
        self.admission_priority
            .validate()
            .map_err(|error| error.to_string())?;
        if zero(Hash::from(self.admission_carrier_hash)) || zero(self.admitted_input_hash) {
            return Err("lane input source identity is zero".to_owned());
        }
        if self.slots.is_empty() || self.slots.len() > MAX_LANE_INPUT_ROUTE_SLOTS {
            return Err("lane input distinct route count is outside its bound".to_owned());
        }
        if self
            .slots
            .windows(2)
            .any(|pair| pair[0].route_key() >= pair[1].route_key())
        {
            return Err("lane input slots are duplicated or unordered".to_owned());
        }
        for slot in &self.slots {
            if slot.lane_height == 0 || zero(slot.lane_incarnation) || zero(slot.instance_id) {
                return Err("lane input slot has an invalid frontier or identity".to_owned());
            }
        }
        Ok(())
    }

    /// Hash every canonical descriptor field under its distinct signing domain.
    ///
    /// # Errors
    /// Rejects malformed shape and canonical serialization errors.
    pub fn canonical_hash(&self) -> Result<Hash, String> {
        self.validate_structure()?;
        let bytes = norito::encode_canonical(self).map_err(|error| error.to_string())?;
        Ok(Hash::new_from_chunks(&[DESCRIPTOR_DOMAIN, &bytes]))
    }
}

/// Self-contained body whose hash and signed RS16 commitment enter lane votes.
///
/// It retains the exact first-carrier complete input. Each route certifies this
/// immutable input and slot list independently; the global execution carrier
/// joins route decisions and computes economic results against its actual base.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::block::lane_input::LaneInputPayloadV1")]
pub struct LaneInputPayloadV1 {
    /// Exact first source and all affected frozen route slots.
    pub descriptor: LaneInputDescriptorV1,
    /// Exact original entrypoint and its first-carrier admission certificate.
    pub input: LaneAdmittedInputV1,
}

impl LaneInputPayloadV1 {
    /// Check the exact descriptor/input relationship and derive the value kind.
    ///
    /// This checks no signatures, finality, current membership or economic policy.
    /// Core must authenticate those boundaries before using the body.
    ///
    /// # Errors
    /// Rejects malformed, oversized, substituted or incomplete route/input claims.
    pub fn validate_structure(&self) -> Result<LaneValueKindV1, String> {
        self.descriptor.validate_structure()?;
        let binding = &self.input.certificate.binding;
        binding.validate_structure()?;
        let plan = self.input.routing_plan()?;
        let length = norito::canonical_frame_len(&self.input).map_err(|error| error.to_string())?;
        if length > MAX_QUEUE_PLAN_ADMISSION_BYTES {
            return Err("lane input exceeds the complete admission control bound".to_owned());
        }
        let bytes = norito::encode_canonical(&self.input).map_err(|error| error.to_string())?;
        if Hash::new(bytes) != self.descriptor.admitted_input_hash {
            return Err("lane input differs from its immutable source hash".to_owned());
        }
        let mut routes = BTreeMap::new();
        for bound in &binding.admission_context.route_incarnations {
            let key = (bound.leg.route.lane_id, bound.leg.route.dataspace_id);
            if let Some(previous) = routes.insert(key, bound.lane_incarnation) {
                if previous != bound.lane_incarnation {
                    return Err("one admitted route claims different incarnations".to_owned());
                }
            }
        }
        if routes.len() != self.descriptor.slots.len()
            || routes
                .iter()
                .zip(&self.descriptor.slots)
                .any(|((route, incarnation), slot)| {
                    *route != slot.route_key() || *incarnation != slot.lane_incarnation
                })
        {
            return Err("lane input slots differ from the complete admitted route set".to_owned());
        }
        Ok(match plan {
            RoutingPlan::Single(_) => LaneValueKindV1::Execution,
            RoutingPlan::NativeAmx(_) => LaneValueKindV1::AtomicGroup,
        })
    }
}

/// One immutable input and the independently certified decision of each route.
///
/// The input is transmitted once, including when a route has both coordinator
/// and participant roles. Decisions use the descriptor's strict route order;
/// their origin and voting views need not agree. This is untrusted wire data.
/// A live consumer must authenticate the first carrier and current frozen
/// contexts. Historical replay requires inclusion in the exact finalized global
/// execution carrier; these bytes alone never grant historical or live authority.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::block::lane_input::LaneDecisionGroupV1")]
pub struct LaneDecisionGroupV1 {
    /// Exact first-carrier input and complete distinct route-slot descriptor.
    pub payload: LaneInputPayloadV1,
    /// One native Commit decision per slot, in the descriptor's route order.
    pub decisions: Vec<LaneDecisionV1>,
}

impl LaneDecisionGroupV1 {
    /// Check bounded group shape and exact input references, without authority.
    ///
    /// Native quorum sizes, signatures, origin leaders, frozen layouts and RS16
    /// codewords must still be checked against independently authenticated
    /// contexts. A valid structure neither releases reservations nor executes.
    ///
    /// # Errors
    /// Rejects missing, duplicated, reordered or substituted route decisions.
    pub fn validate_structure(&self) -> Result<(), String> {
        let kind = self.payload.validate_structure()?;
        let slots = &self.payload.descriptor.slots;
        if self.decisions.len() != slots.len() {
            return Err(
                "native decision group does not contain exactly one decision per route".into(),
            );
        }
        let descriptor_hash = self.payload.descriptor.canonical_hash()?;
        let payload_bytes =
            norito::encode_canonical(&self.payload).map_err(|error| error.to_string())?;
        let payload_hash = Hash::new(&payload_bytes);
        let binding_hash = self.payload.input.certificate.binding.canonical_hash();
        for (slot, decision) in slots.iter().zip(&self.decisions) {
            let value = decision.value();
            let statement = &decision.commit_qc.statement;
            if value.instance_id != slot.instance_id
                || value.admitted_binding_hash != binding_hash
                || value.kind != kind
                || value.descriptor_hash != descriptor_hash
                || value.payload_hash != payload_hash
                || statement.phase != LanePhaseV1::Commit
                || statement.value != *value
                || statement.round.instance_id != slot.instance_id
                || statement.round.lane_height != slot.lane_height
                || statement.round.voting_view < value.origin_view
                || decision.manifest.byte_len != payload_bytes.len() as u64
            {
                return Err(
                    "native decision group differs from its exact input or ordered route slots"
                        .into(),
                );
            }
            decision
                .manifest
                .validate_availability()
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// Decode canonical framing under the enclosing carrier's actual byte cap.
    ///
    /// Use a frame-derived allocation budget for the single complete input;
    /// transaction decoding must not inherit a certificate-only allocation cap.
    /// A stricter enclosing Norito budget remains effective. This checks no
    /// native signatures, current membership or global finality.
    ///
    /// # Errors
    /// Rejects empty, oversized, noncanonical or structurally invalid groups.
    pub fn decode_canonical(bytes: &[u8], maximum_bytes: usize) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > maximum_bytes {
            return Err("native decision group exceeds its enclosing carrier bound".into());
        }
        let frame_bytes = bytes.len();
        let allocation = norito::canonical_decode_limits(frame_bytes).max_total_allocated_bytes();
        let value: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::DecodeLimits::new(frame_bytes, frame_bytes, frame_bytes, allocation, 64),
        )
        .map_err(|error| error.to_string())?;
        value.validate_structure()?;
        Ok(value)
    }
}

fn zero(hash: Hash) -> bool {
    hash == Hash::prehashed([0; Hash::LENGTH])
}

#[cfg(test)]
#[path = "lane_input_tests.rs"]
mod tests;
