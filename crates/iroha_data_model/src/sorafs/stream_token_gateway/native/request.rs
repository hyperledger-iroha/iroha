//! Bounded native gateway transition and observation claims; authority remains Core-owned.

use super::{StreamTokenGatewayCheckV1, StreamTokenGatewayPolicyV1};
use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize, NetworkId,
    sorafs::{
        reputation::StreamTokenValidationStatusV1,
        stream_token_gateway::{
            StreamTokenGatewayAdmissionErrorV1 as Error, StreamTokenGatewayAdmissionRecordV1,
            StreamTokenGatewayAdmissionRequestV1,
        },
    },
};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Maximum complete canonical frame of one native gateway request.
pub const STREAM_TOKEN_GATEWAY_MAX_REQUEST_BYTES_V1: usize = 32 * 1024;
/// Maximum ordered expiry markers consumed by one deterministic maintenance instruction.
pub const STREAM_TOKEN_GATEWAY_MAX_EXPIRY_ITEMS_V1: u32 = 256;

/// Sole native gateway action surface; a decoded action is not serving authorization.
#[expect(
    clippy::large_enum_variant,
    reason = "gateway actions retain their direct canonical V1 payloads without a second allocation envelope"
)]
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
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::native::StreamTokenGatewayActionV1"
)]
#[norito(
    tag = "action",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum StreamTokenGatewayActionV1 {
    /// Install the first policy or its adjacent replacement without resetting retained history.
    #[codec(index = 0)]
    Configure(StreamTokenGatewayPolicyV1),
    /// Apply one exact gateway-owned serving attempt and append its durable callback result.
    #[codec(index = 1)]
    Admit(StreamTokenGatewayAdmissionRequestV1),
    /// Acknowledge the exact retained record after its reputation callback succeeds.
    #[codec(index = 2)]
    Acknowledge(StreamTokenGatewayAdmissionRecordV1),
    /// Release the exact retained accepted lease without refunding consumed request or byte quota.
    #[codec(index = 3)]
    ReleaseLease(StreamTokenGatewayAdmissionRecordV1),
    /// Retire a bounded ordered prefix of due live indexes while retaining permanent history.
    #[codec(index = 4)]
    Expire {
        /// Nonzero maximum number of due expiry markers to consume, at most 256.
        max_items: u32,
    },
    /// Evaluate one exact fresh observer assertion without mutating gateway state.
    #[codec(index = 5)]
    Check(StreamTokenGatewayCheckV1),
    /// Permanently cancel an undelivered source under separate journal-manager authority.
    #[codec(index = 6)]
    CancelReputationDelivery {
        /// Exact original record; its original outcome remains unchanged.
        record: StreamTokenGatewayAdmissionRecordV1,
        /// Exact active recorder-policy digest independently checked in native State.
        #[norito(json = "crate::json_helpers::fixed_bytes")]
        expected_recorder_policy_digest: [u8; 32],
        /// Closed governed reason; a daemon failure alone grants no cancellation authority.
        reason: crate::sorafs::reputation::stream_token_delivery::StreamTokenReputationCancellationReasonV1,
    },
}

/// Exact network, stable gateway identity, current-policy CAS and native action.
///
/// The executor must compare this network to the signed transaction and derive the current
/// policy, authority and execution coordinates from State. Shape validation proves none of
/// those facts. Initial configuration alone uses zero CAS coordinates; all other actions bind
/// the exact nonzero current revision and policy digest.
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
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::native::StreamTokenGatewayRequestV1"
)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenGatewayRequestV1 {
    /// Genesis-derived native network identity.
    pub network_id: NetworkId,
    /// Stable network-derived gateway identity whose state this action addresses.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub gateway_id: [u8; 32],
    /// Exact current policy revision, or zero only for initial configuration.
    pub expected_policy_revision: u64,
    /// Exact current policy digest, or zero only for initial configuration.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub expected_policy_digest: [u8; 32],
    /// One bounded native action.
    pub action: StreamTokenGatewayActionV1,
}

impl StreamTokenGatewayRequestV1 {
    /// Validate bounded canonical shape and all self-contained identity relations.
    ///
    /// Core separately authenticates the current policy CAS, original policy history, exact
    /// signed instruction, account permissions and deterministic execution time. In particular,
    /// structurally valid diagnostic admissions retain their original token expiry material.
    ///
    /// # Errors
    /// Rejects oversized frames, inert identities, partial CAS coordinates, non-adjacent
    /// configuration, substituted record bindings and out-of-range maintenance limits.
    pub fn validate(&self) -> Result<(), Error> {
        // Bound the complete frame before nested validation or policy commitment construction.
        if norito::canonical_frame_len(self).map_err(|_| Error::InvalidRequest)?
            > STREAM_TOKEN_GATEWAY_MAX_REQUEST_BYTES_V1
        {
            return Err(Error::InvalidRequest);
        }
        if self.network_id.as_bytes() == &[0; 32]
            || self.gateway_id == [0; 32]
            || (self.expected_policy_revision == 0) != (self.expected_policy_digest == [0; 32])
        {
            return Err(Error::BindingMismatch);
        }
        if !matches!(self.action, StreamTokenGatewayActionV1::Configure(_))
            && self.expected_policy_revision == 0
        {
            return Err(Error::BindingMismatch);
        }
        match &self.action {
            StreamTokenGatewayActionV1::Configure(policy) => {
                policy.validate()?;
                if policy.network_id != self.network_id
                    || policy.qualification.gateway_id != self.gateway_id
                    || self.expected_policy_revision.checked_add(1)
                        != Some(policy.qualification.revision)
                {
                    return Err(Error::BindingMismatch);
                }
                Ok(())
            }
            StreamTokenGatewayActionV1::CancelReputationDelivery {
                record,
                expected_recorder_policy_digest,
                ..
            } => {
                record.validate_shape(record.admitted_under)?;
                if *expected_recorder_policy_digest == [0; 32]
                    || record.admitted_under.gateway_id != self.gateway_id
                    || record.admitted_under.revision > self.expected_policy_revision
                    || (record.admitted_under.revision == self.expected_policy_revision
                        && record.admitted_under.policy_digest != self.expected_policy_digest)
                    || !record.outcome.status.counts_for_provider()
                {
                    return Err(Error::BindingMismatch);
                }
                Ok(())
            }
            StreamTokenGatewayActionV1::Admit(request) => request.validate(),
            StreamTokenGatewayActionV1::Check(check) => check.validate_binding(
                self.gateway_id,
                self.expected_policy_revision,
                self.expected_policy_digest,
            ),
            StreamTokenGatewayActionV1::Acknowledge(record)
            | StreamTokenGatewayActionV1::ReleaseLease(record) => {
                record.validate_shape(record.admitted_under)?;
                if record.admitted_under.gateway_id != self.gateway_id
                    || record.admitted_under.revision > self.expected_policy_revision
                    || (record.admitted_under.revision == self.expected_policy_revision
                        && record.admitted_under.policy_digest != self.expected_policy_digest)
                    || (matches!(self.action, StreamTokenGatewayActionV1::ReleaseLease(_))
                        && record.outcome.status != StreamTokenValidationStatusV1::Accepted)
                {
                    return Err(Error::BindingMismatch);
                }
                Ok(())
            }
            StreamTokenGatewayActionV1::Expire { max_items }
                if (1..=STREAM_TOKEN_GATEWAY_MAX_EXPIRY_ITEMS_V1).contains(max_items) =>
            {
                Ok(())
            }
            StreamTokenGatewayActionV1::Expire { .. } => Err(Error::InvalidRequest),
        }
    }
}

#[cfg(test)]
mod tests;
