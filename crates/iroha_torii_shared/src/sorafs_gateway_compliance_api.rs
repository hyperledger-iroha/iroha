//! Canonical account-authenticated gateway-compliance control API projections and request bindings.
//!
//! Signed catalogs and acknowledgements remain owned by `sorafs_manifest::gateway_compliance`.
//! These DTOs describe runtime responses; decoding them grants no current native authority.
use norito::derive::{JsonDeserialize, JsonSerialize};
use sha2::{Digest as _, Sha256};

/// Exact action-response schema label.
pub const GATEWAY_COMPLIANCE_ACTION_SCHEMA_V1: &str = "sorafs.gateway.compliance.action.v1";
/// Exact checkpoint-status schema label.
pub const GATEWAY_COMPLIANCE_STATUS_SCHEMA_V1: &str = "sorafs.gateway.compliance.status.v1";
/// Exact public error schema label.
pub const GATEWAY_COMPLIANCE_ERROR_SCHEMA_V1: &str = "sorafs.gateway.compliance.error.v1";
/// Maximum decoded body for one payload-free control response.
pub const GATEWAY_COMPLIANCE_RESPONSE_MAX_BYTES_V1: usize = 16 * 1024;
/// Single canonical request-idempotency header name.
pub const GATEWAY_COMPLIANCE_IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";
const IDEMPOTENCY_BINDING_DOMAIN_V1: &[u8] = b"iroha.sorafs.gateway.compliance.idempotency.v1";

/// Bounded runtime acknowledgement of one exact control request; not native serving evidence.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct GatewayComplianceActionResponseV1 {
    /// Exact V1 response schema label.
    pub schema: String,
    /// Exact control action name.
    pub action: String,
    /// Lowercase 32-byte canonical catalog digest.
    pub catalog_digest_hex: String,
    /// Lowercase exact request idempotency digest.
    pub idempotency_key: String,
    /// Original durable action time in Unix seconds.
    pub operation_timestamp_unix: u64,
}
/// Payload-free projection of one retained runtime catalog.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct GatewayComplianceCatalogStatusV1 {
    /// Lowercase 32-byte canonical catalog digest.
    pub digest_hex: String,
    /// Positive original catalog sequence.
    pub sequence: u64,
    /// Original catalog issue time in Unix seconds.
    pub generated_at_unix: u64,
    /// Exclusive catalog expiry in Unix seconds.
    pub valid_until_unix: u64,
}
/// Payload-free projection of the last durable promotion or rollback.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct GatewayComplianceLatestActionStatusV1 {
    /// Lowercase exact durable operation identifier.
    pub operation_id_hex: String,
    /// Exact control action name.
    pub action: String,
    /// Previous serving catalog digest, if present.
    #[norito(required)]
    pub previous_serving_digest_hex: Option<String>,
    /// Resulting serving catalog digest.
    pub serving_digest_hex: String,
    /// Original action time in Unix seconds.
    pub recorded_at_unix: u64,
    /// Bounded canonical durable reason code.
    pub reason_code: String,
}
/// Runtime checkpoint observation; the serving flag is a node claim, not native eligibility.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct GatewayComplianceStatusResponseV1 {
    /// Exact V1 response schema label.
    pub schema: String,
    /// Original controller checkpoint version.
    pub checkpoint_version: u8,
    /// Lowercase canonical trust-policy digest.
    pub policy_digest_hex: String,
    /// Node observation time in Unix seconds.
    pub observed_at_unix: u64,
    /// Node-reported runtime readiness; not a verified native capability.
    pub serving_ready: bool,
    /// Current retained chain head, if any.
    #[norito(required)]
    pub chain_head: Option<GatewayComplianceCatalogStatusV1>,
    /// Current serving catalog, if any.
    #[norito(required)]
    pub serving: Option<GatewayComplianceCatalogStatusV1>,
    /// Previous serving catalog, if any.
    #[norito(required)]
    pub previous_serving: Option<GatewayComplianceCatalogStatusV1>,
    /// Staged candidate, if any.
    #[norito(required)]
    pub candidate: Option<GatewayComplianceCatalogStatusV1>,
    /// Total retained acknowledgements.
    pub acknowledgement_count: u64,
    /// Retained positive acknowledgements.
    pub accepted_acknowledgement_count: u64,
    /// Retained negative acknowledgements.
    pub rejected_acknowledgement_count: u64,
    /// Retained durable history entry count.
    pub history_count: u64,
    /// Retained idempotency record count.
    pub idempotency_record_count: u64,
    /// Latest durable promotion or rollback, if any.
    #[norito(required)]
    pub latest_action: Option<GatewayComplianceLatestActionStatusV1>,
}
/// Closed public HTTP failure envelope; no signing credentials or catalog bodies.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct GatewayComplianceErrorResponseV1 {
    /// Exact V1 response schema label.
    pub schema: String,
    /// Closed public failure code.
    pub code: String,
    /// Bounded public failure explanation.
    pub message: String,
}
/// Exact catalog identity selected for the canonical promotion request target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct GatewayCompliancePromoteExpectationV1 {
    /// Exact requested catalog digest.
    pub catalog_digest: [u8; 32],
    /// Positive original catalog sequence.
    pub sequence: u64,
}
/// Bind the exact action, HTTP path-and-query and body to one deterministic request key.
///
/// The caller supplies the request's original path-and-query, including its canonical query order.
/// This pure digest neither signs the request nor authorizes any runtime mutation.
pub fn request_idempotency_binding(action: &str, request_target: &str, body: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(IDEMPOTENCY_BINDING_DOMAIN_V1);
    hasher.update((action.len() as u64).to_be_bytes());
    hasher.update(action.as_bytes());
    hasher.update((request_target.len() as u64).to_be_bytes());
    hasher.update(request_target.as_bytes());
    hasher.update((body.len() as u64).to_be_bytes());
    hasher.update(body);
    hasher.finalize().into()
}
/// Decode exactly 64 lowercase hexadecimal characters without accepting alternate spellings.
pub fn decode_lower_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64
        || value.bytes().any(|byte| !byte.is_ascii_hexdigit())
        || value.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return None;
    }
    let decoded = hex::decode(value).ok()?;
    decoded.try_into().ok()
}
/// Invalid control representation; the error never contains supplied payloads or credentials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GatewayComplianceApiErrorV1;
impl std::fmt::Display for GatewayComplianceApiErrorV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid gateway-compliance control representation")
    }
}
impl std::error::Error for GatewayComplianceApiErrorV1 {}
fn require(valid: bool) -> Result<(), GatewayComplianceApiErrorV1> {
    valid.then_some(()).ok_or(GatewayComplianceApiErrorV1)
}
impl GatewayCompliancePromoteExpectationV1 {
    /// Parse the sole accepted digest-first canonical query spelling.
    /// # Errors
    /// Refuses missing, reordered, duplicated, escaped or noncanonical fields, or zero sequence.
    pub fn parse_query(query: &str) -> Result<Self, GatewayComplianceApiErrorV1> {
        let mut fields = query.split('&');
        let digest_field = fields.next().unwrap_or_default();
        let sequence_field = fields.next().unwrap_or_default();
        require(fields.next().is_none())?;
        let digest_hex = digest_field
            .strip_prefix("expected_catalog_digest=")
            .ok_or(GatewayComplianceApiErrorV1)?;
        let sequence_text = sequence_field
            .strip_prefix("expected_sequence=")
            .ok_or(GatewayComplianceApiErrorV1)?;
        let catalog_digest = decode_lower_hex_32(digest_hex).ok_or(GatewayComplianceApiErrorV1)?;
        let sequence = sequence_text
            .parse::<u64>()
            .ok()
            .filter(|n| *n != 0 && n.to_string() == sequence_text)
            .ok_or(GatewayComplianceApiErrorV1)?;
        let result = Self {
            catalog_digest,
            sequence,
        };
        require(result.canonical_query()? == query)?;
        Ok(result)
    }
    /// Encode the sole request query accepted by the controller.
    /// # Errors
    /// Refuses zero sequence. Native promotion separately checks the exact staged digest.
    pub fn canonical_query(&self) -> Result<String, GatewayComplianceApiErrorV1> {
        require(self.sequence != 0)?;
        Ok(format!(
            "expected_catalog_digest={}&expected_sequence={}",
            hex::encode(self.catalog_digest),
            self.sequence
        ))
    }
}
impl GatewayComplianceCatalogStatusV1 {
    /// Validate the bounded digest and finite positive catalog interval.
    /// # Errors
    /// Refuses a malformed digest, sequence or interval.
    pub fn validate(&self) -> Result<(), GatewayComplianceApiErrorV1> {
        require(
            decode_lower_hex_32(&self.digest_hex).is_some()
                && self.sequence > 0
                && self.generated_at_unix > 0
                && self.valid_until_unix > self.generated_at_unix,
        )
    }
}
impl GatewayComplianceActionResponseV1 {
    /// Validate the exact response schema and closed action representation.
    /// # Errors
    /// Refuses an unknown schema/action, malformed digest or zero durable timestamp.
    pub fn validate(&self) -> Result<(), GatewayComplianceApiErrorV1> {
        require(
            self.schema == GATEWAY_COMPLIANCE_ACTION_SCHEMA_V1
                && matches!(
                    self.action.as_str(),
                    "stage" | "acknowledge" | "promote" | "rollback"
                )
                && decode_lower_hex_32(&self.catalog_digest_hex).is_some()
                && decode_lower_hex_32(&self.idempotency_key).is_some()
                && self.operation_timestamp_unix > 0,
        )
    }
}
impl GatewayComplianceLatestActionStatusV1 {
    /// Validate exact digest spellings and a bounded durable action projection.
    /// # Errors
    /// Refuses malformed digests, an unknown action or an oversized reason code.
    pub fn validate(&self) -> Result<(), GatewayComplianceApiErrorV1> {
        require(
            decode_lower_hex_32(&self.operation_id_hex).is_some()
                && decode_lower_hex_32(&self.serving_digest_hex).is_some()
                && self
                    .previous_serving_digest_hex
                    .as_ref()
                    .is_none_or(|s| decode_lower_hex_32(s).is_some())
                && matches!(self.action.as_str(), "promotion" | "rollback")
                && self.recorded_at_unix > 0
                && !self.reason_code.is_empty()
                && self.reason_code.len() <= 128,
        )
    }
}
impl GatewayComplianceStatusResponseV1 {
    /// Validate exact V1 shape and internally consistent bounded projection fields.
    ///
    /// This validates representation only; `serving_ready` remains a runtime node assertion.
    /// # Errors
    /// Refuses a wrong schema/version, malformed digest/catalog or inconsistent counters.
    pub fn validate(&self) -> Result<(), GatewayComplianceApiErrorV1> {
        require(
            self.schema == GATEWAY_COMPLIANCE_STATUS_SCHEMA_V1
                && self.checkpoint_version == 1
                && self.observed_at_unix > 0
                && decode_lower_hex_32(&self.policy_digest_hex).is_some()
                && self
                    .accepted_acknowledgement_count
                    .checked_add(self.rejected_acknowledgement_count)
                    == Some(self.acknowledgement_count)
                && (self.serving_ready
                    == self.serving.as_ref().is_some_and(|catalog| {
                        catalog.generated_at_unix <= self.observed_at_unix
                            && self.observed_at_unix < catalog.valid_until_unix
                    })),
        )?;
        for value in [
            &self.chain_head,
            &self.serving,
            &self.previous_serving,
            &self.candidate,
        ]
        .into_iter()
        .flatten()
        {
            value.validate()?;
        }
        if let Some(value) = &self.latest_action {
            value.validate()?;
        }
        Ok(())
    }
}
impl GatewayComplianceErrorResponseV1 {
    /// Validate the exact closed failure schema and bounded public diagnostic fields.
    /// # Errors
    /// Refuses unknown schema or unbounded/empty public fields.
    pub fn validate(&self) -> Result<(), GatewayComplianceApiErrorV1> {
        require(
            self.schema == GATEWAY_COMPLIANCE_ERROR_SCHEMA_V1
                && !self.code.is_empty()
                && self.code.len() <= 128
                && !self.message.is_empty()
                && self.message.len() <= 4096,
        )
    }
}
#[cfg(test)]
mod tests;
