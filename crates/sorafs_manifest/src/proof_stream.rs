//! Shared Norito schemas for streaming proof requests.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use norito::{
    derive::{NoritoDeserialize, NoritoSerialize},
    json::{self, Map, Value},
};
use thiserror::Error;

/// Maximum `sample_count` accepted for PoR proof-stream requests.
pub const MAX_PROOF_STREAM_SAMPLE_COUNT: u32 = 500;

/// Streaming proof request envelope (PoR / PDP / PoTR).
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq, Hash)]
pub struct ProofStreamRequestV1 {
    /// Canonical manifest digest (BLAKE3-256).
    pub manifest_digest: [u8; 32],
    /// Provider identifier authorised to serve proofs.
    pub provider_id: [u8; 32],
    /// Proof flavour requested by the client.
    pub proof_kind: ProofStreamKind,
    /// Existing governed challenge identifier (required for PDP).
    ///
    /// PDP sampling is fixed by the recorded challenge. A request without this
    /// binding must never be interpreted as permission to synthesize a new
    /// challenge from client-controlled sampling inputs.
    pub challenge_id: Option<[u8; 32]>,
    /// Requested sample count (required for PoR and forbidden for PDP/PoTR).
    pub sample_count: Option<u32>,
    /// Deadline in milliseconds (required for PoTR).
    pub deadline_ms: Option<u32>,
    /// Optional deterministic seed for sample selection.
    pub sample_seed: Option<u64>,
    /// Client-supplied nonce to guard against replay.
    pub nonce: [u8; 16],
    /// Orchestrator job identifier (UUID bytes).
    ///
    /// This is mandatory for PoTR because the manifest, provider, and job
    /// identifier derive the chain-authoritative request-scope identity.
    pub orchestrator_job_id: Option<[u8; 16]>,
    /// Tier hint for PDP/PoTR (hot/warm/archive).
    pub tier: Option<ProofStreamTier>,
}

impl ProofStreamRequestV1 {
    /// Validate request invariants.
    pub fn validate(&self) -> Result<(), ProofStreamRequestError> {
        if self.manifest_digest.iter().all(|&byte| byte == 0) {
            return Err(ProofStreamRequestError::InvalidManifestDigest);
        }
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(ProofStreamRequestError::InvalidProviderId);
        }
        if self.nonce.iter().all(|&byte| byte == 0) {
            return Err(ProofStreamRequestError::InvalidNonce);
        }
        if self
            .orchestrator_job_id
            .is_some_and(|job_id| job_id.iter().all(|&byte| byte == 0))
        {
            return Err(ProofStreamRequestError::InvalidOrchestratorJobId);
        }
        match self.proof_kind {
            ProofStreamKind::Por => {
                if self.challenge_id.is_some() {
                    return Err(ProofStreamRequestError::UnexpectedChallengeId);
                }
                if self.orchestrator_job_id.is_some() {
                    return Err(ProofStreamRequestError::UnexpectedOrchestratorJobId);
                }
                let count = self
                    .sample_count
                    .ok_or(ProofStreamRequestError::MissingSampleCount)?;
                if count == 0 {
                    return Err(ProofStreamRequestError::ZeroSampleCount);
                }
                if count > MAX_PROOF_STREAM_SAMPLE_COUNT {
                    return Err(ProofStreamRequestError::SampleCountTooLarge);
                }
                if self.deadline_ms.is_some() {
                    return Err(ProofStreamRequestError::UnexpectedDeadlineMs);
                }
            }
            ProofStreamKind::Pdp => {
                let challenge_id = self
                    .challenge_id
                    .ok_or(ProofStreamRequestError::MissingChallengeId)?;
                if challenge_id.iter().all(|&byte| byte == 0) {
                    return Err(ProofStreamRequestError::InvalidChallengeId);
                }
                if self.orchestrator_job_id.is_some() {
                    return Err(ProofStreamRequestError::UnexpectedOrchestratorJobId);
                }
                if self.sample_count.is_some() {
                    return Err(ProofStreamRequestError::UnexpectedSampleCount);
                }
                if self.deadline_ms.is_some() {
                    return Err(ProofStreamRequestError::UnexpectedDeadlineMs);
                }
                if self.sample_seed.is_some() {
                    return Err(ProofStreamRequestError::UnexpectedSampleSeed);
                }
            }
            ProofStreamKind::Potr => {
                if self.challenge_id.is_some() {
                    return Err(ProofStreamRequestError::UnexpectedChallengeId);
                }
                if self.orchestrator_job_id.is_none() {
                    return Err(ProofStreamRequestError::MissingOrchestratorJobId);
                }
                let deadline = self
                    .deadline_ms
                    .ok_or(ProofStreamRequestError::MissingDeadlineMs)?;
                if deadline == 0 {
                    return Err(ProofStreamRequestError::ZeroDeadlineMs);
                }
                if self.sample_count.is_some() {
                    return Err(ProofStreamRequestError::UnexpectedSampleCount);
                }
                if self.sample_seed.is_some() {
                    return Err(ProofStreamRequestError::UnexpectedSampleSeed);
                }
            }
        }
        Ok(())
    }
}

/// Supported proof kinds for streaming.
#[derive(Debug, Clone, Copy, Default, NoritoSerialize, NoritoDeserialize, PartialEq, Eq, Hash)]
pub enum ProofStreamKind {
    /// Proof-of-Retrievability samples.
    #[default]
    Por,
    /// Proofs of Data Possession requests.
    Pdp,
    /// Proof-of-Timed Retrieval receipt requests.
    Potr,
}

/// Tier hints used by PDP/PoTR schedulers.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq, Hash)]
pub enum ProofStreamTier {
    /// Hot tier (low latency).
    Hot,
    /// Warm tier (mid latency).
    Warm,
    /// Archive tier (cold storage).
    Archive,
}

impl ProofStreamKind {
    /// Canonical lowercase HTTP/JSON label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Por => "por",
            Self::Pdp => "pdp",
            Self::Potr => "potr",
        }
    }

    /// Parse an exact canonical HTTP/JSON label.
    pub fn parse(raw: &str) -> Result<Self, ProofStreamHttpRequestError> {
        match raw {
            "por" => Ok(Self::Por),
            "pdp" => Ok(Self::Pdp),
            "potr" => Ok(Self::Potr),
            _ => Err(ProofStreamHttpRequestError::UnsupportedProofKind),
        }
    }
}

impl ProofStreamTier {
    /// Canonical lowercase HTTP/JSON label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hot => "hot",
            Self::Warm => "warm",
            Self::Archive => "archive",
        }
    }

    /// Parse an exact canonical HTTP/JSON label.
    pub fn parse(raw: &str) -> Result<Self, ProofStreamHttpRequestError> {
        match raw {
            "hot" => Ok(Self::Hot),
            "warm" => Ok(Self::Warm),
            "archive" => Ok(Self::Archive),
            _ => Err(ProofStreamHttpRequestError::UnsupportedTier),
        }
    }
}

impl norito::json::JsonSerialize for ProofStreamKind {
    fn json_serialize(&self, out: &mut String) {
        let label = self.as_str();
        <&str as norito::json::JsonSerialize>::json_serialize(&label, out);
    }
}

impl norito::json::JsonSerialize for ProofStreamTier {
    fn json_serialize(&self, out: &mut String) {
        let label = self.as_str();
        <&str as norito::json::JsonSerialize>::json_serialize(&label, out);
    }
}

/// Validation failures for [`ProofStreamRequestV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ProofStreamRequestError {
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    #[error("nonce must be non-zero")]
    InvalidNonce,
    #[error("orchestrator job id must be non-zero when supplied")]
    InvalidOrchestratorJobId,
    #[error("PoTR requests require a non-zero orchestrator job id")]
    MissingOrchestratorJobId,
    #[error("orchestrator job id is reserved for PoTR requests")]
    UnexpectedOrchestratorJobId,
    #[error("PDP requests require a non-zero governed challenge id")]
    MissingChallengeId,
    #[error("challenge id must be non-zero")]
    InvalidChallengeId,
    #[error("challenge id is only valid for PDP requests")]
    UnexpectedChallengeId,
    #[error("PoR requests require a sample count")]
    MissingSampleCount,
    #[error("sample count is only valid for PoR requests")]
    UnexpectedSampleCount,
    #[error("PoTR requests require a deadline")]
    MissingDeadlineMs,
    #[error("deadline is only valid for PoTR requests")]
    UnexpectedDeadlineMs,
    #[error("sample seed is only valid for PoR requests")]
    UnexpectedSampleSeed,
    #[error("sample count must be greater than zero")]
    ZeroSampleCount,
    #[error("sample count exceeds maximum")]
    SampleCountTooLarge,
    #[error("deadline must be greater than zero milliseconds")]
    ZeroDeadlineMs,
}

/// Validated canonical Norito-JSON envelope for the proof-stream HTTP API.
///
/// The inner binary request is kept private so callers cannot serialize an
/// unchecked string DTO. Optional fields are omitted on the wire; explicit
/// `null`, aliases, and unknown fields are rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProofStreamHttpRequestV1 {
    request: ProofStreamRequestV1,
}

impl ProofStreamHttpRequestV1 {
    /// Construct a wire envelope after validating all request invariants.
    pub fn new(request: ProofStreamRequestV1) -> Result<Self, ProofStreamHttpRequestError> {
        request.validate()?;
        Ok(Self { request })
    }

    /// Borrow the validated canonical binary request.
    #[must_use]
    pub const fn request(&self) -> &ProofStreamRequestV1 {
        &self.request
    }

    /// Consume the envelope and return the validated canonical binary request.
    #[must_use]
    pub const fn into_request(self) -> ProofStreamRequestV1 {
        self.request
    }

    /// Render the exact canonical HTTP object.
    #[must_use]
    pub fn to_json_value(&self) -> Value {
        let request = self.request;
        let mut map = Map::new();
        map.insert(
            "manifest_digest_hex".into(),
            Value::from(hex::encode(request.manifest_digest)),
        );
        map.insert(
            "provider_id_hex".into(),
            Value::from(hex::encode(request.provider_id)),
        );
        map.insert(
            "proof_kind".into(),
            Value::from(request.proof_kind.as_str()),
        );
        if let Some(challenge_id) = request.challenge_id {
            map.insert(
                "challenge_id_hex".into(),
                Value::from(hex::encode(challenge_id)),
            );
        }
        if let Some(sample_count) = request.sample_count {
            map.insert("sample_count".into(), Value::from(sample_count));
        }
        if let Some(deadline_ms) = request.deadline_ms {
            map.insert("deadline_ms".into(), Value::from(deadline_ms));
        }
        if let Some(sample_seed) = request.sample_seed {
            map.insert("sample_seed".into(), Value::from(sample_seed));
        }
        map.insert(
            "nonce_b64".into(),
            Value::from(BASE64_STANDARD.encode(request.nonce)),
        );
        if let Some(job_id) = request.orchestrator_job_id {
            map.insert(
                "orchestrator_job_id_hex".into(),
                Value::from(hex::encode(job_id)),
            );
        }
        if let Some(tier) = request.tier {
            map.insert("tier".into(), Value::from(tier.as_str()));
        }
        Value::Object(map)
    }

    /// Parse and validate an exact canonical HTTP object.
    pub fn from_json_value(value: &Value) -> Result<Self, ProofStreamHttpRequestError> {
        let object = value
            .as_object()
            .ok_or(ProofStreamHttpRequestError::ExpectedObject)?;
        const FIELDS: &[&str] = &[
            "manifest_digest_hex",
            "provider_id_hex",
            "proof_kind",
            "challenge_id_hex",
            "sample_count",
            "deadline_ms",
            "sample_seed",
            "nonce_b64",
            "orchestrator_job_id_hex",
            "tier",
        ];
        if object.keys().any(|field| !FIELDS.contains(&field.as_str())) {
            return Err(ProofStreamHttpRequestError::UnknownField);
        }

        let manifest_digest = parse_canonical_hex::<32>(
            required_string(object, "manifest_digest_hex")?,
            "manifest_digest_hex",
        )?;
        let provider_id = parse_canonical_hex::<32>(
            required_string(object, "provider_id_hex")?,
            "provider_id_hex",
        )?;
        let proof_kind = ProofStreamKind::parse(required_string(object, "proof_kind")?)?;
        let challenge_id = optional_string(object, "challenge_id_hex")?
            .map(|raw| parse_canonical_hex::<32>(raw, "challenge_id_hex"))
            .transpose()?;
        let sample_count = optional_u64(object, "sample_count")?
            .map(|value| {
                u32::try_from(value)
                    .map_err(|_| ProofStreamHttpRequestError::IntegerOutOfRange("sample_count"))
            })
            .transpose()?;
        let deadline_ms = optional_u64(object, "deadline_ms")?
            .map(|value| {
                u32::try_from(value)
                    .map_err(|_| ProofStreamHttpRequestError::IntegerOutOfRange("deadline_ms"))
            })
            .transpose()?;
        let sample_seed = optional_u64(object, "sample_seed")?;
        let nonce = parse_canonical_base64_16(required_string(object, "nonce_b64")?)?;
        let orchestrator_job_id = optional_string(object, "orchestrator_job_id_hex")?
            .map(|raw| parse_canonical_hex::<16>(raw, "orchestrator_job_id_hex"))
            .transpose()?;
        let tier = optional_string(object, "tier")?
            .map(ProofStreamTier::parse)
            .transpose()?;

        Self::new(ProofStreamRequestV1 {
            manifest_digest,
            provider_id,
            proof_kind,
            challenge_id,
            sample_count,
            deadline_ms,
            sample_seed,
            nonce,
            orchestrator_job_id,
            tier,
        })
    }
}

impl TryFrom<ProofStreamRequestV1> for ProofStreamHttpRequestV1 {
    type Error = ProofStreamHttpRequestError;

    fn try_from(request: ProofStreamRequestV1) -> Result<Self, Self::Error> {
        Self::new(request)
    }
}

impl json::JsonSerialize for ProofStreamHttpRequestV1 {
    fn json_serialize(&self, out: &mut String) {
        self.to_json_value().json_serialize(out);
    }
}

impl json::JsonDeserialize for ProofStreamHttpRequestV1 {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = Value::json_deserialize(parser)?;
        Self::json_from_value(&value)
    }

    fn json_from_value(value: &Value) -> Result<Self, json::Error> {
        Self::from_json_value(value).map_err(|error| json::Error::Message(error.to_string()))
    }
}

/// Canonical proof-stream HTTP envelope failures.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ProofStreamHttpRequestError {
    /// Top-level JSON value was not an object.
    #[error("proof stream request must be a JSON object")]
    ExpectedObject,
    /// An unrecognized field was present.
    #[error("proof stream request contains an unknown field")]
    UnknownField,
    /// A required field was absent.
    #[error("proof stream request is missing required field `{0}`")]
    MissingField(&'static str),
    /// A field used the wrong JSON type.
    #[error("proof stream request field `{0}` has the wrong JSON type")]
    WrongType(&'static str),
    /// An optional field was explicitly set to null instead of omitted.
    #[error("proof stream request field `{0}` must be omitted instead of null")]
    ExplicitNull(&'static str),
    /// A fixed-width integer exceeded its wire range.
    #[error("proof stream request field `{0}` is outside its integer range")]
    IntegerOutOfRange(&'static str),
    /// A hexadecimal field had the wrong exact length.
    #[error("proof stream request field `{0}` has the wrong hexadecimal length")]
    InvalidHexLength(&'static str),
    /// A hexadecimal field contained invalid characters.
    #[error("proof stream request field `{0}` is not hexadecimal")]
    InvalidHex(&'static str),
    /// A hexadecimal field was not exact lowercase canonical encoding.
    #[error("proof stream request field `{0}` is not canonical lowercase hexadecimal")]
    NonCanonicalHex(&'static str),
    /// The proof-kind label was not canonical.
    #[error("proof stream request uses an unsupported proof kind")]
    UnsupportedProofKind,
    /// The tier label was not canonical.
    #[error("proof stream request uses an unsupported proof tier")]
    UnsupportedTier,
    /// The nonce was not canonical padded base64.
    #[error("proof stream request nonce is not canonical padded base64")]
    InvalidNonceBase64,
    /// The decoded nonce did not contain exactly 16 bytes.
    #[error("proof stream request nonce must decode to exactly 16 bytes")]
    InvalidNonceLength,
    /// The canonical binary request failed semantic validation.
    #[error(transparent)]
    InvalidRequest(#[from] ProofStreamRequestError),
}

fn required_string<'a>(
    object: &'a Map,
    field: &'static str,
) -> Result<&'a str, ProofStreamHttpRequestError> {
    match object.get(field) {
        Some(Value::String(value)) => Ok(value),
        Some(_) => Err(ProofStreamHttpRequestError::WrongType(field)),
        None => Err(ProofStreamHttpRequestError::MissingField(field)),
    }
}

fn optional_string<'a>(
    object: &'a Map,
    field: &'static str,
) -> Result<Option<&'a str>, ProofStreamHttpRequestError> {
    match object.get(field) {
        Some(Value::String(value)) => Ok(Some(value)),
        Some(Value::Null) => Err(ProofStreamHttpRequestError::ExplicitNull(field)),
        Some(_) => Err(ProofStreamHttpRequestError::WrongType(field)),
        None => Ok(None),
    }
}

fn optional_u64(
    object: &Map,
    field: &'static str,
) -> Result<Option<u64>, ProofStreamHttpRequestError> {
    match object.get(field) {
        Some(Value::Null) => Err(ProofStreamHttpRequestError::ExplicitNull(field)),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or(ProofStreamHttpRequestError::WrongType(field)),
        None => Ok(None),
    }
}

fn parse_canonical_hex<const N: usize>(
    raw: &str,
    field: &'static str,
) -> Result<[u8; N], ProofStreamHttpRequestError> {
    if raw.len() != N * 2 {
        return Err(ProofStreamHttpRequestError::InvalidHexLength(field));
    }
    let bytes = hex::decode(raw).map_err(|_| ProofStreamHttpRequestError::InvalidHex(field))?;
    if hex::encode(&bytes) != raw {
        return Err(ProofStreamHttpRequestError::NonCanonicalHex(field));
    }
    bytes
        .try_into()
        .map_err(|_| ProofStreamHttpRequestError::InvalidHexLength(field))
}

fn parse_canonical_base64_16(raw: &str) -> Result<[u8; 16], ProofStreamHttpRequestError> {
    let bytes = BASE64_STANDARD
        .decode(raw.as_bytes())
        .map_err(|_| ProofStreamHttpRequestError::InvalidNonceBase64)?;
    if BASE64_STANDARD.encode(&bytes) != raw {
        return Err(ProofStreamHttpRequestError::InvalidNonceBase64);
    }
    bytes
        .try_into()
        .map_err(|_| ProofStreamHttpRequestError::InvalidNonceLength)
}

#[cfg(test)]
mod tests {
    use norito::json::{Value, from_slice, to_vec};

    use super::*;

    fn base_request() -> ProofStreamRequestV1 {
        ProofStreamRequestV1 {
            manifest_digest: [0x11; 32],
            provider_id: [0x22; 32],
            proof_kind: ProofStreamKind::Por,
            challenge_id: None,
            sample_count: Some(16),
            deadline_ms: None,
            sample_seed: Some(42),
            nonce: [0x33; 16],
            orchestrator_job_id: None,
            tier: Some(ProofStreamTier::Hot),
        }
    }

    #[test]
    fn por_request_validates() {
        let request = base_request();
        assert_eq!(request.validate(), Ok(()));
    }

    #[test]
    fn missing_sample_count_rejected() {
        let mut request = base_request();
        request.sample_count = None;
        assert_eq!(
            request.validate(),
            Err(ProofStreamRequestError::MissingSampleCount)
        );
    }

    #[test]
    fn zero_orchestrator_job_id_is_rejected() {
        let mut request = base_request();
        request.orchestrator_job_id = Some([0; 16]);
        assert_eq!(
            request.validate(),
            Err(ProofStreamRequestError::InvalidOrchestratorJobId)
        );
    }

    #[test]
    fn oversized_sample_count_rejected() {
        let mut request = base_request();
        request.sample_count = Some(MAX_PROOF_STREAM_SAMPLE_COUNT + 1);
        assert_eq!(
            request.validate(),
            Err(ProofStreamRequestError::SampleCountTooLarge)
        );
    }

    #[test]
    fn potr_requires_deadline() {
        let mut request = base_request();
        request.proof_kind = ProofStreamKind::Potr;
        request.sample_count = None;
        request.sample_seed = None;
        request.orchestrator_job_id = Some([0x44; 16]);
        assert_eq!(
            request.validate(),
            Err(ProofStreamRequestError::MissingDeadlineMs)
        );
        request.deadline_ms = Some(0);
        assert_eq!(
            request.validate(),
            Err(ProofStreamRequestError::ZeroDeadlineMs)
        );
        request.deadline_ms = Some(90_000);
        assert_eq!(request.validate(), Ok(()));
    }

    #[test]
    fn potr_requires_an_exact_request_scope_job_id() {
        let mut request = base_request();
        request.proof_kind = ProofStreamKind::Potr;
        request.sample_count = None;
        request.sample_seed = None;
        request.deadline_ms = Some(90_000);
        request.orchestrator_job_id = None;
        assert_eq!(
            request.validate(),
            Err(ProofStreamRequestError::MissingOrchestratorJobId)
        );
        request.orchestrator_job_id = Some([0; 16]);
        assert_eq!(
            request.validate(),
            Err(ProofStreamRequestError::InvalidOrchestratorJobId)
        );
        request.orchestrator_job_id = Some([0x44; 16]);
        assert_eq!(request.validate(), Ok(()));
    }

    #[test]
    fn pdp_requires_a_non_zero_governed_challenge_id() {
        let mut request = base_request();
        request.proof_kind = ProofStreamKind::Pdp;
        request.sample_count = None;
        request.sample_seed = None;
        assert_eq!(
            request.validate(),
            Err(ProofStreamRequestError::MissingChallengeId)
        );
        request.challenge_id = Some([0; 32]);
        assert_eq!(
            request.validate(),
            Err(ProofStreamRequestError::InvalidChallengeId)
        );
        request.challenge_id = Some([0x55; 32]);
        assert_eq!(request.validate(), Ok(()));
    }

    #[test]
    fn non_pdp_requests_reject_challenge_ids() {
        let mut request = base_request();
        request.challenge_id = Some([0x55; 32]);
        assert_eq!(
            request.validate(),
            Err(ProofStreamRequestError::UnexpectedChallengeId)
        );
        request.proof_kind = ProofStreamKind::Potr;
        request.sample_count = None;
        request.sample_seed = None;
        request.deadline_ms = Some(90_000);
        assert_eq!(
            request.validate(),
            Err(ProofStreamRequestError::UnexpectedChallengeId)
        );
    }

    #[test]
    fn por_and_pdp_reject_potr_request_scope_ids() {
        let mut por = base_request();
        por.orchestrator_job_id = Some([0x44; 16]);
        assert_eq!(
            por.validate(),
            Err(ProofStreamRequestError::UnexpectedOrchestratorJobId)
        );

        let mut pdp = base_request();
        pdp.proof_kind = ProofStreamKind::Pdp;
        pdp.challenge_id = Some([0x55; 32]);
        pdp.sample_count = None;
        pdp.sample_seed = None;
        pdp.orchestrator_job_id = Some([0x44; 16]);
        assert_eq!(
            pdp.validate(),
            Err(ProofStreamRequestError::UnexpectedOrchestratorJobId)
        );
    }

    #[test]
    fn zero_nonce_rejected() {
        let mut request = base_request();
        request.nonce = [0u8; 16];
        assert_eq!(
            request.validate(),
            Err(ProofStreamRequestError::InvalidNonce)
        );
    }

    #[test]
    fn canonical_http_request_roundtrips_without_null_optionals() {
        let request = base_request();
        let envelope =
            ProofStreamHttpRequestV1::new(request).expect("valid canonical HTTP envelope");
        let bytes = to_vec(&envelope).expect("serialize canonical HTTP envelope");
        let value: Value = from_slice(&bytes).expect("parse canonical HTTP JSON");
        let obj = value
            .as_object()
            .expect("proof stream request should serialize to JSON object");
        assert_eq!(obj.len(), 7);
        assert_eq!(
            obj.get("proof_kind").and_then(Value::as_str),
            Some("por"),
            "proof_kind should serialize to lowercase label"
        );
        assert_eq!(
            obj.get("tier").and_then(Value::as_str),
            Some("hot"),
            "tier should serialize to lowercase label"
        );
        assert_eq!(
            obj.get("manifest_digest_hex").and_then(Value::as_str),
            Some("11".repeat(32).as_str())
        );
        assert_eq!(
            obj.get("nonce_b64").and_then(Value::as_str),
            Some(BASE64_STANDARD.encode([0x33; 16]).as_str())
        );
        assert!(!obj.contains_key("challenge_id_hex"));
        assert!(!obj.contains_key("deadline_ms"));
        assert!(obj.values().all(|value| !matches!(value, Value::Null)));

        let decoded: ProofStreamHttpRequestV1 =
            from_slice(&bytes).expect("decode canonical HTTP envelope");
        assert_eq!(decoded.into_request(), request);
    }

    #[test]
    fn canonical_http_request_rejects_unknown_alias_and_null_fields() {
        let base = ProofStreamHttpRequestV1::new(base_request())
            .expect("valid envelope")
            .to_json_value();
        let base = base.as_object().expect("object");

        for field in ["provider_id", "verification_status", "unexpected"] {
            let mut object = base.clone();
            object.insert(field.into(), Value::from("retired"));
            assert_eq!(
                ProofStreamHttpRequestV1::from_json_value(&Value::Object(object)),
                Err(ProofStreamHttpRequestError::UnknownField)
            );
        }

        for field in [
            "challenge_id_hex",
            "sample_count",
            "deadline_ms",
            "sample_seed",
            "orchestrator_job_id_hex",
            "tier",
        ] {
            let mut object = base.clone();
            object.insert(field.into(), Value::Null);
            assert_eq!(
                ProofStreamHttpRequestV1::from_json_value(&Value::Object(object)),
                Err(ProofStreamHttpRequestError::ExplicitNull(field))
            );
        }
    }

    #[test]
    fn canonical_http_request_rejects_noncanonical_identity_and_nonce_encodings() {
        let base = ProofStreamHttpRequestV1::new(base_request())
            .expect("valid envelope")
            .to_json_value();
        let base = base.as_object().expect("object");

        let mut uppercase = base.clone();
        uppercase.insert("manifest_digest_hex".into(), Value::from("AB".repeat(32)));
        assert_eq!(
            ProofStreamHttpRequestV1::from_json_value(&Value::Object(uppercase)),
            Err(ProofStreamHttpRequestError::NonCanonicalHex(
                "manifest_digest_hex"
            ))
        );

        let mut zero_provider = base.clone();
        zero_provider.insert("provider_id_hex".into(), Value::from("00".repeat(32)));
        assert_eq!(
            ProofStreamHttpRequestV1::from_json_value(&Value::Object(zero_provider)),
            Err(ProofStreamHttpRequestError::InvalidRequest(
                ProofStreamRequestError::InvalidProviderId
            ))
        );

        let canonical_nonce = BASE64_STANDARD.encode([0x33; 16]);
        let mut unpadded_nonce = base.clone();
        unpadded_nonce.insert(
            "nonce_b64".into(),
            Value::from(canonical_nonce.trim_end_matches('=')),
        );
        assert_eq!(
            ProofStreamHttpRequestV1::from_json_value(&Value::Object(unpadded_nonce)),
            Err(ProofStreamHttpRequestError::InvalidNonceBase64)
        );

        let mut zero_nonce = base.clone();
        zero_nonce.insert(
            "nonce_b64".into(),
            Value::from(BASE64_STANDARD.encode([0; 16])),
        );
        assert_eq!(
            ProofStreamHttpRequestV1::from_json_value(&Value::Object(zero_nonce)),
            Err(ProofStreamHttpRequestError::InvalidRequest(
                ProofStreamRequestError::InvalidNonce
            ))
        );
    }

    #[test]
    fn canonical_http_request_rejects_wrong_types_ranges_and_kind_fields() {
        let base = ProofStreamHttpRequestV1::new(base_request())
            .expect("valid envelope")
            .to_json_value();
        let base = base.as_object().expect("object");

        let mut wrong_type = base.clone();
        wrong_type.insert("tier".into(), Value::from(7));
        assert_eq!(
            ProofStreamHttpRequestV1::from_json_value(&Value::Object(wrong_type)),
            Err(ProofStreamHttpRequestError::WrongType("tier"))
        );

        let mut overflow = base.clone();
        overflow.insert("sample_count".into(), Value::from(u64::from(u32::MAX) + 1));
        assert_eq!(
            ProofStreamHttpRequestV1::from_json_value(&Value::Object(overflow)),
            Err(ProofStreamHttpRequestError::IntegerOutOfRange(
                "sample_count"
            ))
        );

        let mut wrong_kind = base.clone();
        wrong_kind.insert("proof_kind".into(), Value::from("pdp"));
        assert_eq!(
            ProofStreamHttpRequestV1::from_json_value(&Value::Object(wrong_kind)),
            Err(ProofStreamHttpRequestError::InvalidRequest(
                ProofStreamRequestError::MissingChallengeId
            ))
        );

        let mut noncanonical_tier = base.clone();
        noncanonical_tier.insert("tier".into(), Value::from("HOT"));
        assert_eq!(
            ProofStreamHttpRequestV1::from_json_value(&Value::Object(noncanonical_tier)),
            Err(ProofStreamHttpRequestError::UnsupportedTier)
        );
    }

    #[test]
    fn canonical_http_request_rejects_duplicate_json_keys() {
        let raw = format!(
            concat!(
                "{{",
                "\"manifest_digest_hex\":\"{}\",",
                "\"provider_id_hex\":\"{}\",",
                "\"proof_kind\":\"por\",",
                "\"proof_kind\":\"pdp\",",
                "\"sample_count\":1,",
                "\"nonce_b64\":\"{}\"",
                "}}"
            ),
            "11".repeat(32),
            "22".repeat(32),
            BASE64_STANDARD.encode([0x33; 16])
        );
        let error = from_slice::<ProofStreamHttpRequestV1>(raw.as_bytes())
            .expect_err("duplicate JSON keys must fail closed");
        assert!(matches!(error, json::Error::DuplicateField { .. }));
    }
}
