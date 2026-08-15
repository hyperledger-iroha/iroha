//! Deployment-owned stream-token quota, sequencing, and reputation-callback admission.
//!
//! Production gateways must inject an implementation of [`StreamTokenGatewayAdmissionProviderV1`].
//! The provider owns the atomic quota decision, sealed monotonic gateway sequence, and durable
//! ordered callback outbox. Torii never reconstructs or rewrites the returned typed outcome;
//! [`StreamTokenAdmissionCaptureV1`] passes it unchanged to the committed reputation runtime and
//! acknowledges the external row only after that callback succeeds.
use iroha_config::parameters::is_production_runtime_handle;
use iroha_data_model::sorafs::{
    capacity::ProviderId,
    reputation::{
        StreamTokenExcludedKindV1, StreamTokenRequestRouteV1, StreamTokenValidationOutcomeV1,
        StreamTokenValidationRequestContextV1, StreamTokenValidationStatusV1,
        StreamTokenViolationKindV1,
    },
};
use norito::codec::{Decode, Encode};
use sorafs_node::reputation::runtime::{
    ReputationNativeOutcomeAdmissionApiV1, ReputationNativeOutcomeAdmissionStateV1,
    StreamTokenReputationAdmissionOutcomeV1,
};
use std::{fmt, sync::Arc};
use thiserror::Error;
/// Hard V1 ceiling for one reconciliation call.
pub const STREAM_TOKEN_GATEWAY_RECONCILE_MAX_ITEMS_V1: u32 = 1_024;
/// Exact public identity of a deployment-owned admission provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
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
            || self.max_streams == 0
            || self.rate_limit_bytes == 0
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct StreamTokenGatewayAdmissionRequestV1 {
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
        if self.validated_at_unix_ms == 0
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct StreamTokenGatewayAdmissionRecordV1 {
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
    /// # Errors
    ///
    /// Rejects inert, substituted, or internally inconsistent material.
    pub fn validate_shape(
        self,
        qualification: StreamTokenGatewayAdmissionQualificationV1,
    ) -> Result<(), StreamTokenGatewayAdmissionErrorV1> {
        qualification.validate()?;
        if self.provider_id.as_bytes() == &[0; 32]
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
            let expected = exact_lease_expiry_unix_ms(
                self.outcome.validated_at_unix_ms,
                self.lease_token_expires_at_epoch
                    .ok_or(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)?,
                qualification.lease_ttl_ms,
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
        if self.provider_id != request.context.provider_id()
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
                    != Some(exact_lease_expiry_unix_ms(
                        request.validated_at_unix_ms,
                        token_expiry,
                        qualification.lease_ttl_ms,
                    )?)
            {
                return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
            }
        }
        Ok(())
    }
}
fn exact_lease_expiry_unix_ms(
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
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
        if max_items == 0
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
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
/// Deployment-owned quota, sealed sequence, and ordered-outbox boundary.
pub trait StreamTokenGatewayAdmissionProviderV1: Send + Sync + fmt::Debug {
    /// Return the stable credential-free provider handle.
    fn handle(&self) -> &str;
    /// Return the live public qualification.
    ///
    /// # Errors
    ///
    /// Fails when the provider is unavailable, stale, revoked, or malformed.
    fn qualification(
        &self,
    ) -> Result<StreamTokenGatewayAdmissionQualificationV1, StreamTokenGatewayAdmissionErrorV1>;
    /// Atomically apply quota, allocate a sealed monotonic sequence, and append
    /// one ordered pending callback row.
    ///
    /// Exact replay must return the byte-identical retained record. Substituted
    /// material for the same request context must fail closed.
    fn admit(
        &self,
        request: &StreamTokenGatewayAdmissionRequestV1,
    ) -> Result<StreamTokenGatewayAdmissionResultV1, StreamTokenGatewayAdmissionErrorV1>;
    /// Return the oldest pending callback rows in gateway-sequence order.
    fn pending(
        &self,
        max_items: u32,
    ) -> Result<StreamTokenGatewayAdmissionReadbackV1, StreamTokenGatewayAdmissionErrorV1>;
    /// Durably acknowledge one callback only after reputation admission succeeds.
    fn acknowledge(
        &self,
        record: StreamTokenGatewayAdmissionRecordV1,
    ) -> Result<StreamTokenGatewayAdmissionAckV1, StreamTokenGatewayAdmissionErrorV1>;
    /// Idempotently release one accepted cross-replica concurrency lease.
    ///
    /// A crashed caller need not run this method: the deployment provider must expire the lease at
    /// `lease_expires_at_unix_ms` before admitting another stream against the signed ceiling.
    fn release_lease(
        &self,
        record: StreamTokenGatewayAdmissionRecordV1,
    ) -> Result<StreamTokenGatewayAdmissionAckV1, StreamTokenGatewayAdmissionErrorV1>;
}
/// Qualified Torii capture boundary combining external admission with the
/// committed reputation callback.
#[derive(Clone)]
pub struct StreamTokenAdmissionCaptureV1 {
    provider: Arc<dyn StreamTokenGatewayAdmissionProviderV1>,
    reputation: Arc<dyn ReputationNativeOutcomeAdmissionApiV1>,
    expected_handle: Arc<str>,
    expected_qualification: StreamTokenGatewayAdmissionQualificationV1,
    reconcile_max_items: u32,
}
impl fmt::Debug for StreamTokenAdmissionCaptureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StreamTokenAdmissionCaptureV1")
            .field("expected_handle", &self.expected_handle)
            .field("expected_qualification", &self.expected_qualification)
            .field("reconcile_max_items", &self.reconcile_max_items)
            .finish_non_exhaustive()
    }
}
impl StreamTokenAdmissionCaptureV1 {
    /// Construct and live-qualify the strict production capture boundary.
    ///
    /// # Errors
    ///
    /// Rejects missing, substituted, stale, revoked, inert, or test-marked
    /// public bindings and an inactive reputation admission runtime.
    pub fn try_new(
        expected_handle: impl Into<String>,
        expected_qualification: StreamTokenGatewayAdmissionQualificationV1,
        reconcile_max_items: u32,
        provider: Arc<dyn StreamTokenGatewayAdmissionProviderV1>,
        reputation: Arc<dyn ReputationNativeOutcomeAdmissionApiV1>,
    ) -> Result<Self, StreamTokenGatewayAdmissionErrorV1> {
        let expected_handle = expected_handle.into();
        expected_qualification.validate()?;
        if !is_production_runtime_handle(&expected_handle)
            || provider.handle() != expected_handle
            || provider.qualification()? != expected_qualification
            || reconcile_max_items == 0
            || reconcile_max_items > STREAM_TOKEN_GATEWAY_RECONCILE_MAX_ITEMS_V1
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::BindingMismatch);
        }
        match reputation
            .activation_state()
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::ReputationCallback)?
        {
            ReputationNativeOutcomeAdmissionStateV1::Active => {}
            ReputationNativeOutcomeAdmissionStateV1::Deferred => {
                return Err(StreamTokenGatewayAdmissionErrorV1::ReputationCallback);
            }
        }
        let capture = Self {
            provider,
            reputation,
            expected_handle: Arc::from(expected_handle),
            expected_qualification,
            reconcile_max_items,
        };
        capture.ensure_binding()?;
        Ok(capture)
    }
    /// Revalidate this capture against an independently derived launch binding.
    ///
    /// # Errors
    ///
    /// Rejects substituted launch inputs or live provider drift.
    pub fn validate_expected_binding(
        &self,
        expected_handle: &str,
        expected_qualification: StreamTokenGatewayAdmissionQualificationV1,
        expected_reconcile_max_items: u32,
    ) -> Result<(), StreamTokenGatewayAdmissionErrorV1> {
        expected_qualification.validate()?;
        if !is_production_runtime_handle(expected_handle)
            || self.expected_handle.as_ref() != expected_handle
            || self.expected_qualification != expected_qualification
            || self.reconcile_max_items != expected_reconcile_max_items
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::BindingMismatch);
        }
        self.ensure_binding()
    }
    /// Commit one admission and synchronously deliver its exact typed outcome.
    ///
    /// The external row remains pending if reputation admission or durable
    /// acknowledgement fails, so a restart can replay it exactly.
    pub fn admit(
        &self,
        request: &StreamTokenGatewayAdmissionRequestV1,
    ) -> Result<StreamTokenGatewayAdmissionRecordV1, StreamTokenGatewayAdmissionErrorV1> {
        request.validate()?;
        self.reconcile_until(None)?;
        self.ensure_binding()?;
        let admission = self.provider.admit(request)?;
        admission.validate_for_request(request, self.expected_qualification)?;
        self.ensure_binding()?;
        match admission.delivery_state {
            StreamTokenGatewayAdmissionDeliveryStateV1::Pending { .. } => {
                self.reconcile_until(Some(admission.record))?;
            }
            StreamTokenGatewayAdmissionDeliveryStateV1::AcknowledgedExactReplay { .. } => {
                self.deliver_acknowledged_replay(admission.record)?;
            }
        }
        Ok(admission.record)
    }
    /// Replay the oldest durable callback suffix after a crash or outage.
    ///
    /// # Errors
    ///
    /// Fails closed on provider drift, unordered/substituted pending rows,
    /// reputation rejection, or acknowledgement failure.
    pub fn reconcile_pending(&self) -> Result<u32, StreamTokenGatewayAdmissionErrorV1> {
        self.reconcile_one_batch(None)
            .map(|outcome| outcome.delivered)
    }
    /// Release an accepted external concurrency lease idempotently.
    ///
    /// # Errors
    ///
    /// Rejects substituted records or a drifting/unavailable provider. A
    /// failed release remains bounded by the externally authenticated expiry.
    pub fn release_lease(
        &self,
        record: StreamTokenGatewayAdmissionRecordV1,
    ) -> Result<StreamTokenGatewayAdmissionAckV1, StreamTokenGatewayAdmissionErrorV1> {
        record.validate_shape(self.expected_qualification)?;
        if record.outcome.status != StreamTokenValidationStatusV1::Accepted {
            return Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest);
        }
        self.ensure_binding()?;
        let released = self.provider.release_lease(record)?;
        self.ensure_binding()?;
        Ok(released)
    }
    fn reconcile_until(
        &self,
        required_record: Option<StreamTokenGatewayAdmissionRecordV1>,
    ) -> Result<u32, StreamTokenGatewayAdmissionErrorV1> {
        let mut delivered = 0_u32;
        loop {
            let batch = self.reconcile_one_batch(required_record)?;
            delivered = delivered
                .checked_add(batch.delivered)
                .ok_or(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)?;
            if required_record.is_none() && batch.delivered == 0 {
                return Ok(delivered);
            }
            if required_record.is_some() && batch.required_record_delivered {
                return Ok(delivered);
            }
            if batch.delivered == 0 || delivered > self.expected_qualification.max_pending {
                return Err(StreamTokenGatewayAdmissionErrorV1::Unavailable);
            }
        }
    }
    fn reconcile_one_batch(
        &self,
        required_record: Option<StreamTokenGatewayAdmissionRecordV1>,
    ) -> Result<StreamTokenReconcileBatchV1, StreamTokenGatewayAdmissionErrorV1> {
        self.ensure_binding()?;
        let pending = self.provider.pending(self.reconcile_max_items)?;
        pending.validate(self.reconcile_max_items, self.expected_qualification)?;
        self.ensure_binding()?;
        let mut delivery_count = pending.records.len();
        let mut required_record_delivered = false;
        if let Some(required) = required_record {
            let required_sequence = required.outcome.binding.gateway_sequence;
            if pending.acknowledged_through_sequence >= required_sequence {
                self.deliver_acknowledged_replay(required)?;
                return Ok(StreamTokenReconcileBatchV1 {
                    delivered: 0,
                    required_record_delivered: true,
                });
            }
            if pending.high_water_sequence < required_sequence {
                return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
            }
            match pending
                .records
                .iter()
                .position(|record| record.outcome.binding.gateway_sequence == required_sequence)
            {
                Some(position) => {
                    if pending.records[position] != required {
                        return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
                    }
                    delivery_count = position + 1;
                    required_record_delivered = true;
                }
                None if pending.records.len() == self.reconcile_max_items as usize => {}
                None => return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome),
            }
        }
        for record in pending.records.iter().take(delivery_count) {
            self.deliver_record(*record)?;
        }
        self.ensure_binding()?;
        Ok(StreamTokenReconcileBatchV1 {
            delivered: u32::try_from(delivery_count)
                .map_err(|_| StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)?,
            required_record_delivered,
        })
    }
    fn deliver_acknowledged_replay(
        &self,
        record: StreamTokenGatewayAdmissionRecordV1,
    ) -> Result<(), StreamTokenGatewayAdmissionErrorV1> {
        self.reputation
            .record_authenticated_stream_token_validation(record.provider_id, record.outcome)
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::ReputationCallback)?;
        if self.provider.acknowledge(record)? != StreamTokenGatewayAdmissionAckV1::ExactReplay {
            return Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome);
        }
        self.ensure_binding()
    }
    fn deliver_record(
        &self,
        record: StreamTokenGatewayAdmissionRecordV1,
    ) -> Result<StreamTokenReputationAdmissionOutcomeV1, StreamTokenGatewayAdmissionErrorV1> {
        let outcome = record.outcome;
        let admitted = self
            .reputation
            .record_authenticated_stream_token_validation(record.provider_id, outcome)
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::ReputationCallback)?;
        self.provider.acknowledge(record)?;
        self.ensure_binding()?;
        Ok(admitted)
    }
    fn ensure_binding(&self) -> Result<(), StreamTokenGatewayAdmissionErrorV1> {
        if self.provider.handle() != self.expected_handle.as_ref()
            || self.provider.qualification()? != self.expected_qualification
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::StaleOrRevoked);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StreamTokenReconcileBatchV1 {
    delivered: u32,
    required_record_delivered: bool,
}
#[cfg(test)]
#[path = "stream_token_admission/tests.rs"]
mod tests;
