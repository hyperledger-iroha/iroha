//! Canonical stream-token gateway quota, sequencing, lease, and callback claims.
//!
//! These payload-free records bind one exact request and gateway policy. Runtime providers must
//! authenticate them against their authoritative state; a decoded claim alone grants no serving
//! permission.
use crate::sorafs::{
    capacity::ProviderId,
    reputation::{
        StreamTokenExcludedKindV1, StreamTokenRequestRouteV1, StreamTokenValidationOutcomeV1,
        StreamTokenValidationRequestContextV1, StreamTokenValidationStatusV1,
        StreamTokenViolationKindV1,
    },
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sorafs_manifest::token::{
    STREAM_TOKEN_MAX_RATE_LIMIT_BYTES_V1, STREAM_TOKEN_MAX_REQUESTS_PER_MINUTE_V1,
    STREAM_TOKEN_MAX_STREAMS_V1,
};
use thiserror::Error;

/// Consensus-owned gateway control, transition requests, and committed outcomes.
pub mod native;

/// Hard V1 ceiling for one reconciliation call.
pub const STREAM_TOKEN_GATEWAY_RECONCILE_MAX_ITEMS_V1: u32 = 1_024;
/// Exact public identity of a deployment-owned admission provider.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::StreamTokenGatewayAdmissionQualificationV1"
)]
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
pub struct StreamTokenGatewayAdmissionQualificationV1 {
    /// Stable identity derived from the chain and governed compliance gateway.
    pub gateway_id: [u8; 32],
    /// Non-zero adapter and public-policy revision.
    pub revision: u64,
    /// Non-zero digest of the provider's public policy.
    pub policy_digest: [u8; 32],
    /// Exact durable pending-row capacity enforced by the provider.
    pub max_pending: u32,
    /// Exact active token-window capacity enforced by the provider.
    pub max_tracked_tokens: u32,
    /// Exact maximum lifetime for one cross-replica concurrency lease.
    pub lease_ttl_ms: u64,
}
impl StreamTokenGatewayAdmissionQualificationV1 {
    /// Validate non-inert public qualification material.
    ///
    /// # Errors
    ///
    /// Returns [`StreamTokenGatewayAdmissionErrorV1::BindingMismatch`] for an
    /// inert gateway identity, revision, or policy digest.
    pub fn validate(self) -> Result<(), StreamTokenGatewayAdmissionErrorV1> {
        if self.gateway_id == [0; 32]
            || self.revision == 0
            || self.policy_digest == [0; 32]
            || self.max_pending == 0
            || self.max_pending > 1_000_000
            || self.max_tracked_tokens == 0
            || self.max_tracked_tokens > 1_000_000
            || self.lease_ttl_ms == 0
            || self.lease_ttl_ms > 300_000
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::BindingMismatch);
        }
        Ok(())
    }
}
/// Signed token quota inputs admitted atomically with one callback row.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::StreamTokenGatewayQuotaRequestV1"
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
)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenGatewayQuotaRequestV1 {
    /// Canonical 16-byte token identifier rendered as lowercase hexadecimal.
    pub token_id: String,
    /// Signed cross-replica concurrent-stream ceiling.
    pub max_streams: u16,
    /// Signed request budget per minute.
    pub requests_per_minute: u32,
    /// Signed byte budget per second.
    pub rate_limit_bytes: u64,
    /// Exact bytes selected by the canonical route.
    pub requested_bytes: u64,
    /// Signed token expiry in seconds since Unix epoch.
    pub expires_at_epoch: u64,
    /// Authenticated observation time in seconds since Unix epoch.
    pub observed_at_epoch: u64,
}
impl StreamTokenGatewayQuotaRequestV1 {
    fn validate(&self) -> Result<(), StreamTokenGatewayAdmissionErrorV1> {
        if self.token_id.len() != 32
            || !self
                .token_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || self.requests_per_minute == 0
            || self.requests_per_minute > STREAM_TOKEN_MAX_REQUESTS_PER_MINUTE_V1
            || self.max_streams == 0
            || self.max_streams > STREAM_TOKEN_MAX_STREAMS_V1
            || self.rate_limit_bytes == 0
            || self.rate_limit_bytes > STREAM_TOKEN_MAX_RATE_LIMIT_BYTES_V1
            || self.requested_bytes == 0
            || self.expires_at_epoch == 0
            || self.observed_at_epoch == 0
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest);
        }
        Ok(())
    }
}
/// Complete payload-free input to one external gateway admission transaction.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::StreamTokenGatewayAdmissionRequestV1"
)]
#[derive(IntoSchema, DeriveJsonSerialize, DeriveJsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenGatewayAdmissionRequestV1 {
    /// Fresh cryptographic identity of one gateway-owned HTTP serving attempt, retained on RPC retry.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub serving_attempt_id: [u8; 32],
    /// Exact canonical serving context.
    pub context: StreamTokenValidationRequestContextV1,
    /// Canonical signed token-body digest, present exactly after successful decode.
    pub token_body_digest: Option<[u8; 32]>,
    /// Signing-key version from the decoded token body.
    pub token_key_version: Option<u32>,
    /// Authenticated observation time in milliseconds since Unix epoch.
    pub validated_at_unix_ms: u64,
    /// Torii's terminal validation before deployment-owned quota admission.
    pub status: StreamTokenValidationStatusV1,
    /// Exact signed quota material, present when a canonical token body exists.
    pub quota: Option<StreamTokenGatewayQuotaRequestV1>,
}
impl StreamTokenGatewayAdmissionRequestV1 {
    /// Validate canonical request material before it crosses the provider boundary.
    ///
    /// # Errors
    ///
    /// Rejects malformed context, timestamp, token material, or quota bindings.
    pub fn validate(&self) -> Result<(), StreamTokenGatewayAdmissionErrorV1> {
        self.context
            .validate()
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::InvalidRequest)?;
        if self.serving_attempt_id == [0; 32]
            || self.validated_at_unix_ms == 0
            || self.token_body_digest == Some([0; 32])
            || self.token_key_version == Some(0)
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest);
        }
        let carries_body = matches!(
            self.status,
            StreamTokenValidationStatusV1::Accepted
                | StreamTokenValidationStatusV1::ProviderViolation(_)
                | StreamTokenValidationStatusV1::Excluded(
                    StreamTokenExcludedKindV1::InvalidSignature
                        | StreamTokenExcludedKindV1::UnsupportedKeyVersion
                        | StreamTokenExcludedKindV1::SignerAuthorityUnavailable
                )
        );
        if carries_body != self.token_body_digest.is_some()
            || carries_body != self.token_key_version.is_some()
            || carries_body != self.quota.is_some()
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest);
        }
        if let Some(quota) = &self.quota {
            quota.validate()?;
            let requested_bytes = match self.context.route() {
                StreamTokenRequestRouteV1::CarRange(range) => range
                    .byte_length()
                    .map_err(|_| StreamTokenGatewayAdmissionErrorV1::InvalidRequest)?,
                StreamTokenRequestRouteV1::Chunk(chunk) => chunk.stored_length,
            };
            if quota.observed_at_epoch != self.validated_at_unix_ms / 1_000
                || quota.expires_at_epoch.checked_mul(1_000).is_none()
                || quota.requested_bytes != requested_bytes
                || self.token_body_digest.is_none()
            {
                return Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest);
            }
        }
        Ok(())
    }
}
/// One externally committed, ordered callback row.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::StreamTokenGatewayAdmissionRecordV1"
)]
#[derive(IntoSchema, DeriveJsonSerialize, DeriveJsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenGatewayAdmissionRecordV1 {
    /// Exact original gateway-owned HTTP serving attempt; a new worker cannot adopt its lease.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub serving_attempt_id: [u8; 32],
    /// Original authenticated admission policy; current policy cannot reinterpret retained rows.
    pub admitted_under: StreamTokenGatewayAdmissionQualificationV1,
    /// Authoritative local serving provider.
    pub provider_id: ProviderId,
    /// Complete externally authenticated outcome.
    pub outcome: StreamTokenValidationOutcomeV1,
    /// Retry delay for quota violations, absent for every other terminal.
    pub retry_after_secs: Option<u32>,
    /// Opaque deployment-owned concurrency lease, present only when admitted.
    pub lease_id: Option<[u8; 32]>,
    /// Lease expiry in milliseconds since Unix epoch, present with `lease_id`.
    pub lease_expires_at_unix_ms: Option<u64>,
    /// Signed token expiry used to derive the lease deadline, present only with an accepted lease.
    pub lease_token_expires_at_epoch: Option<u64>,
}
impl StreamTokenGatewayAdmissionRecordV1 {
    /// Validate one retained pending/lease record against the live provider.
    ///
    /// Older authenticated policies retain their original lease bound. The provider must
    /// independently authenticate `admitted_under`; these shape checks do not prove policy history.
    ///
    /// # Errors
    ///
    /// Rejects inert, substituted, or internally inconsistent material.
    pub fn validate_shape(
        self,
        qualification: StreamTokenGatewayAdmissionQualificationV1,
    ) -> Result<(), StreamTokenGatewayAdmissionErrorV1> {
        qualification.validate()?;
        self.admitted_under
            .validate()
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)?;
        if self.serving_attempt_id == [0; 32]
            || self.admitted_under.gateway_id != qualification.gateway_id
            || self.admitted_under.revision > qualification.revision
            || (self.admitted_under.revision == qualification.revision
                && self.admitted_under != qualification)
            || self.provider_id.as_bytes() == &[0; 32]
            || self.outcome.binding.gateway_id != qualification.gateway_id
            || self.outcome.binding.gateway_sequence == 0
            || self.outcome.binding.request_context_digest == [0; 32]
            || self.outcome.token_body_digest == Some([0; 32])
            || self.outcome.token_key_version == Some(0)
            || self.outcome.validated_at_unix_ms == 0
            || self.lease_id == Some([0; 32])
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
        }
        let carries_body = matches!(
            self.outcome.status,
            StreamTokenValidationStatusV1::Accepted
                | StreamTokenValidationStatusV1::ProviderViolation(_)
                | StreamTokenValidationStatusV1::Excluded(
                    StreamTokenExcludedKindV1::InvalidSignature
                        | StreamTokenExcludedKindV1::UnsupportedKeyVersion
                        | StreamTokenExcludedKindV1::SignerAuthorityUnavailable
                )
        );
        if carries_body != self.outcome.token_body_digest.is_some()
            || carries_body != self.outcome.token_key_version.is_some()
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
        }
        let needs_retry = matches!(
            self.outcome.status,
            StreamTokenValidationStatusV1::ProviderViolation(
                StreamTokenViolationKindV1::RequestQuotaExceeded
                    | StreamTokenViolationKindV1::ByteRateLimitExceeded
            )
        );
        if needs_retry != self.retry_after_secs.is_some() || self.retry_after_secs == Some(0) {
            return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
        }
        let admitted = self.outcome.status == StreamTokenValidationStatusV1::Accepted;
        if admitted != self.lease_id.is_some()
            || admitted != self.lease_expires_at_unix_ms.is_some()
            || admitted != self.lease_token_expires_at_epoch.is_some()
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
        }
        if admitted {
            let expected = stream_token_gateway_lease_expiry_unix_ms_v1(
                self.outcome.validated_at_unix_ms,
                self.lease_token_expires_at_epoch
                    .ok_or(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)?,
                self.admitted_under.lease_ttl_ms,
            )
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)?;
            if self.lease_expires_at_unix_ms != Some(expected) {
                return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
            }
        }
        Ok(())
    }
    /// Verify that an external record is the exact result of `request` under `qualification`.
    ///
    /// # Errors
    ///
    /// Rejects substituted provider, context, gateway, token material,
    /// timestamp, status, or retry metadata.
    pub fn validate_for_request(
        self,
        request: &StreamTokenGatewayAdmissionRequestV1,
        qualification: StreamTokenGatewayAdmissionQualificationV1,
    ) -> Result<(), StreamTokenGatewayAdmissionErrorV1> {
        request.validate()?;
        qualification.validate()?;
        self.validate_shape(qualification)?;
        let request_context_digest = request
            .context
            .digest()
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::InvalidRequest)?;
        if self.serving_attempt_id != request.serving_attempt_id
            || self.provider_id != request.context.provider_id()
            || self.outcome.binding.gateway_id != qualification.gateway_id
            || self.outcome.binding.gateway_sequence == 0
            || self.outcome.binding.request_context_digest != request_context_digest
            || self.outcome.token_body_digest != request.token_body_digest
            || self.outcome.token_key_version != request.token_key_version
            || self.outcome.validated_at_unix_ms != request.validated_at_unix_ms
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
        }
        let status_is_valid = self.outcome.status == request.status
            || matches!(
                (request.status, self.outcome.status),
                (
                    StreamTokenValidationStatusV1::Accepted,
                    StreamTokenValidationStatusV1::ProviderViolation(
                        StreamTokenViolationKindV1::Expired
                            | StreamTokenViolationKindV1::ConcurrencyLimitExceeded
                            | StreamTokenViolationKindV1::RequestQuotaExceeded
                            | StreamTokenViolationKindV1::ByteRateLimitExceeded
                            | StreamTokenViolationKindV1::IdentifierPolicyConflict
                    )
                )
            );
        if !status_is_valid {
            return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
        }
        if self.outcome.status == StreamTokenValidationStatusV1::Accepted {
            let token_expiry = request
                .quota
                .as_ref()
                .ok_or(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)?
                .expires_at_epoch;
            if self.lease_token_expires_at_epoch != Some(token_expiry)
                || self.lease_expires_at_unix_ms
                    != Some(stream_token_gateway_lease_expiry_unix_ms_v1(
                        request.validated_at_unix_ms,
                        token_expiry,
                        self.admitted_under.lease_ttl_ms,
                    )?)
            {
                return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
            }
        }
        Ok(())
    }
}
/// Derive the exact exclusive lease expiry from the authenticated request and policy.
///
/// # Errors
/// Rejects timestamp overflow and leases already expired at the validation time.
pub fn stream_token_gateway_lease_expiry_unix_ms_v1(
    validated_at_unix_ms: u64,
    token_expires_at_epoch: u64,
    lease_ttl_ms: u64,
) -> Result<u64, StreamTokenGatewayAdmissionErrorV1> {
    let token_expires_at_unix_ms = token_expires_at_epoch
        .checked_mul(1_000)
        .ok_or(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)?;
    let ttl_expires_at_unix_ms = validated_at_unix_ms
        .checked_add(lease_ttl_ms)
        .ok_or(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)?;
    let expires_at_unix_ms = token_expires_at_unix_ms.min(ttl_expires_at_unix_ms);
    if expires_at_unix_ms <= validated_at_unix_ms {
        return Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest);
    }
    Ok(expires_at_unix_ms)
}
/// Provider-authenticated state of the exact row returned by `admit`.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::StreamTokenGatewayAdmissionDeliveryStateV1"
)]
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
#[norito(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum StreamTokenGatewayAdmissionDeliveryStateV1 {
    /// The row is pending after the exact immediately preceding sequence.
    Pending {
        /// Sealed high-water value before this row was allocated; zero means
        /// the row is the first gateway sequence.
        predecessor_sequence: u64,
    },
    /// This exact request was already acknowledged by another replica.
    AcknowledgedExactReplay {
        /// Authenticated contiguous acknowledgement high-water covering the returned row.
        acknowledged_through_sequence: u64,
    },
}
/// Exact atomic result of one deployment-owned admission transaction.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::StreamTokenGatewayAdmissionResultV1"
)]
#[derive(IntoSchema, DeriveJsonSerialize, DeriveJsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenGatewayAdmissionResultV1 {
    /// Byte-identical retained callback and optional lease record.
    pub record: StreamTokenGatewayAdmissionRecordV1,
    /// Provider-authenticated delivery state at the linearization point.
    pub delivery_state: StreamTokenGatewayAdmissionDeliveryStateV1,
}
impl StreamTokenGatewayAdmissionResultV1 {
    /// Validate the exact retained row and its authenticated delivery state.
    ///
    /// # Errors
    ///
    /// Rejects request substitution, sequence gaps, and false replay claims.
    pub fn validate_for_request(
        self,
        request: &StreamTokenGatewayAdmissionRequestV1,
        qualification: StreamTokenGatewayAdmissionQualificationV1,
    ) -> Result<(), StreamTokenGatewayAdmissionErrorV1> {
        self.record.validate_for_request(request, qualification)?;
        let sequence = self.record.outcome.binding.gateway_sequence;
        match self.delivery_state {
            StreamTokenGatewayAdmissionDeliveryStateV1::Pending {
                predecessor_sequence,
            } if predecessor_sequence.checked_add(1) == Some(sequence) => Ok(()),
            StreamTokenGatewayAdmissionDeliveryStateV1::AcknowledgedExactReplay {
                acknowledged_through_sequence,
            } if acknowledged_through_sequence >= sequence => Ok(()),
            _ => Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome),
        }
    }
}
/// Authenticated oldest-pending readback with contiguous sequence proofs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::StreamTokenGatewayAdmissionReadbackV1"
)]
#[derive(IntoSchema, DeriveJsonSerialize, DeriveJsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenGatewayAdmissionReadbackV1 {
    /// Highest gateway sequence durably acknowledged without a gap.
    pub acknowledged_through_sequence: u64,
    /// Highest gateway sequence durably allocated without a gap.
    pub high_water_sequence: u64,
    /// Oldest pending contiguous prefix after `acknowledged_through_sequence`.
    pub records: Vec<StreamTokenGatewayAdmissionRecordV1>,
}
impl StreamTokenGatewayAdmissionReadbackV1 {
    /// Validate an authenticated contiguous pending-prefix readback.
    ///
    /// # Errors
    ///
    /// Rejects oversized, gapped, reordered, omitted, or substituted rows.
    pub fn validate(
        &self,
        max_items: u32,
        qualification: StreamTokenGatewayAdmissionQualificationV1,
    ) -> Result<(), StreamTokenGatewayAdmissionErrorV1> {
        qualification.validate()?;
        if max_items == 0
            || max_items > STREAM_TOKEN_GATEWAY_RECONCILE_MAX_ITEMS_V1
            || self.records.len() > max_items as usize
            || self.acknowledged_through_sequence > self.high_water_sequence
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
        }
        if self.records.is_empty() {
            return if self.acknowledged_through_sequence == self.high_water_sequence {
                Ok(())
            } else {
                Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)
            };
        }
        let mut expected = self
            .acknowledged_through_sequence
            .checked_add(1)
            .ok_or(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)?;
        for record in &self.records {
            record.validate_shape(qualification)?;
            if record.outcome.binding.gateway_sequence != expected {
                return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
            }
            expected = expected
                .checked_add(1)
                .ok_or(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)?;
        }
        let last_returned = expected - 1;
        if last_returned > self.high_water_sequence
            || (self.records.len() < max_items as usize
                && last_returned != self.high_water_sequence)
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
        }
        Ok(())
    }
}
/// Durable acknowledgement result for one external callback row.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::StreamTokenGatewayAdmissionAckV1"
)]
#[derive(IntoSchema, DeriveJsonSerialize, DeriveJsonDeserialize)]
#[norito(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum StreamTokenGatewayAdmissionAckV1 {
    /// The pending row was durably acknowledged now.
    Acknowledged,
    /// The exact row was already acknowledged.
    ExactReplay,
}
/// Payload-free production provider failure.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum StreamTokenGatewayAdmissionErrorV1 {
    /// Configured and live public bindings differ or are test-marked.
    #[error("stream-token gateway admission provider binding mismatch")]
    BindingMismatch,
    /// Request material is malformed or exceeds the provider contract.
    #[error("stream-token gateway admission request is invalid")]
    InvalidRequest,
    /// The provider returned substituted or malformed outcome material.
    #[error("stream-token gateway admission outcome is substituted")]
    SubstitutedOutcome,
    /// Sealed state or the ordered outbox is temporarily unavailable.
    #[error("stream-token gateway admission provider is unavailable")]
    Unavailable,
    /// The provider rejected the requested state transition.
    #[error("stream-token gateway admission provider rejected the request")]
    Rejected,
    /// A compare-and-swap or replay identity conflicts with durable state.
    #[error("stream-token gateway admission provider reported a conflict")]
    Conflict,
    /// The provider qualification is stale or revoked.
    #[error("stream-token gateway admission provider is stale or revoked")]
    StaleOrRevoked,
    /// A mutating operation may have committed and must be reconciled.
    #[error("stream-token gateway admission outcome is ambiguous")]
    Ambiguous,
    /// The committed reputation callback is unavailable or rejected.
    #[error("stream-token reputation callback failed")]
    ReputationCallback,
}

#[cfg(test)]
#[path = "stream_token_gateway/types_tests.rs"]
mod types_tests;
