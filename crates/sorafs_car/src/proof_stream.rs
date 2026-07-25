//! Shared helpers for SoraFS proof streaming clients.
//!
//! This module provides request/response representations that match the Torii
//! `/v1/sorafs/proof/stream` endpoint together with lightweight aggregation
//! utilities used by the CLI and SDK integrations.

use std::collections::BTreeMap;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use norito::{
    decode_from_bytes,
    json::{Map, Value, from_slice},
};
use sorafs_manifest::{PotrReceiptV1, PotrStatus};

use crate::{PorProof, por_json::proof_from_value};

/// Canonical proof flavour shared with the request schema.
pub use sorafs_manifest::ProofStreamKind as ProofKind;
/// Canonical storage tier shared with the request schema.
pub use sorafs_manifest::ProofStreamTier as ProofTier;

/// Verification status reported for a streaming item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationStatus {
    /// Item verified successfully.
    Success,
    /// Verification failed.
    Failure,
}

impl VerificationStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "success" => Ok(Self::Success),
            "failure" => Ok(Self::Failure),
            other => Err(format!(
                "unsupported proof result `{other}`; expected success or failure"
            )),
        }
    }

    #[must_use]
    pub fn is_failure(self) -> bool {
        matches!(self, Self::Failure)
    }
}

/// Streaming item reported by the gateway.
#[derive(Clone, Debug)]
pub struct ProofStreamItem {
    /// Manifest digest (hex).
    manifest_digest_hex: String,
    /// Provider identifier (hex).
    provider_id_hex: String,
    /// Chain-authoritative outcome identity (hex) for committed PDP/PoTR rows.
    outcome_identity_hex: Option<String>,
    /// Digest of the committed canonical archive or final signed receipt.
    outcome_digest_hex: Option<String>,
    /// Council-verified admission envelope bound to the committed outcome.
    admission_envelope_digest_hex: Option<String>,
    /// Finalized block height anchoring the committed outcome lookup.
    finalized_block_height: Option<u64>,
    /// Finalized block hash anchoring the committed outcome lookup.
    finalized_block_hash_hex: Option<String>,
    /// Committing block timestamp in milliseconds since Unix epoch.
    committed_at_ms: Option<u64>,
    /// Governed PDP challenge identifier (hex).
    challenge_id_hex: Option<String>,
    /// Proof kind.
    proof_kind: ProofKind,
    /// Verification status.
    status: VerificationStatus,
    /// Failure reason string (if provided).
    failure_reason: Option<String>,
    /// Reported latency in milliseconds.
    latency_ms: Option<u32>,
    /// Configured deadline in milliseconds (PoTR).
    deadline_ms: Option<u32>,
    /// Flat sample index (PoR).
    sample_index: Option<u32>,
    /// Chunk index (PoR).
    chunk_index: Option<u32>,
    /// Segment index (PoR).
    segment_index: Option<u32>,
    /// Leaf index within the segment (PoR).
    leaf_index: Option<u32>,
    /// Storage tier hint associated with the item.
    tier: Option<ProofTier>,
    /// Optional trace identifier.
    trace_id: Option<String>,
    /// Decoded PoR proof when supplied by the gateway.
    por_proof: Option<PorProof>,
    /// Canonical final signed PoTR receipt when supplied by the gateway.
    potr_receipt: Option<PotrReceiptV1>,
    /// Timestamp when the proof item was recorded (milliseconds since Unix epoch).
    recorded_at_ms: Option<u64>,
}

fn optional_u32_field(obj: &Map, key: &str) -> Result<Option<u32>, String> {
    let Some(value) = obj.get(key) else {
        return Ok(None);
    };
    let value = value
        .as_u64()
        .ok_or_else(|| format!("`{key}` must be an unsigned 32-bit integer when present"))?;
    u32::try_from(value)
        .map(Some)
        .map_err(|_| format!("`{key}` must fit in u32 (got {value})"))
}

fn optional_u64_field(obj: &Map, key: &str) -> Result<Option<u64>, String> {
    let Some(value) = obj.get(key) else {
        return Ok(None);
    };
    value
        .as_u64()
        .map(Some)
        .ok_or_else(|| format!("`{key}` must be an unsigned 64-bit integer when present"))
}

fn canonical_nonzero_hex<const N: usize>(raw: &str, field: &str) -> Result<String, String> {
    if raw.len() != N * 2 {
        return Err(format!(
            "`{field}` must contain exactly {} lowercase hexadecimal characters",
            N * 2
        ));
    }
    let bytes = hex::decode(raw).map_err(|error| format!("invalid `{field}`: {error}"))?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(format!("`{field}` must be non-zero"));
    }
    if hex::encode(&bytes) != raw {
        return Err(format!(
            "`{field}` must use canonical lowercase hexadecimal"
        ));
    }
    Ok(raw.to_owned())
}

fn canonical_failure_reason(raw: &str) -> Result<String, String> {
    const MAX_FAILURE_REASON_BYTES: usize = 64;
    if raw.is_empty() || raw.len() > MAX_FAILURE_REASON_BYTES {
        return Err(format!(
            "`failure_reason` must contain 1..={MAX_FAILURE_REASON_BYTES} bytes"
        ));
    }
    if !raw
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err("`failure_reason` must use canonical lowercase snake-case ASCII".to_string());
    }
    Ok(raw.to_owned())
}

fn decode_canonical_potr_receipt(raw: &str) -> Result<PotrReceiptV1, String> {
    const MAX_ENCODED_RECEIPT_BYTES: usize = 32 * 1024;
    if raw.is_empty() || raw.len() > MAX_ENCODED_RECEIPT_BYTES {
        return Err("`receipt_b64` exceeds the bounded PoTR receipt size".to_string());
    }
    let bytes = BASE64_STANDARD
        .decode(raw.as_bytes())
        .map_err(|error| format!("invalid `receipt_b64`: {error}"))?;
    if BASE64_STANDARD.encode(&bytes) != raw {
        return Err("`receipt_b64` must use canonical padded base64".to_string());
    }
    let receipt: PotrReceiptV1 = decode_from_bytes(&bytes)
        .map_err(|error| format!("failed to decode signed PoTR receipt: {error}"))?;
    receipt
        .validate()
        .map_err(|error| format!("invalid signed PoTR receipt: {error}"))?;
    let canonical = receipt
        .signed_receipt_bytes()
        .map_err(|error| format!("failed to re-encode signed PoTR receipt: {error}"))?;
    if canonical != bytes {
        return Err("`receipt_b64` must contain exact canonical Norito receipt bytes".to_string());
    }
    Ok(receipt)
}

impl ProofStreamItem {
    /// Return the canonical manifest digest encoding.
    #[must_use]
    pub fn manifest_digest_hex(&self) -> &str {
        &self.manifest_digest_hex
    }

    /// Return the canonical provider identifier encoding.
    #[must_use]
    pub fn provider_id_hex(&self) -> &str {
        &self.provider_id_hex
    }

    /// Return the committed outcome identity when this is a chain-backed row.
    #[must_use]
    pub fn outcome_identity_hex(&self) -> Option<&str> {
        self.outcome_identity_hex.as_deref()
    }

    /// Return the committed outcome digest when this is a chain-backed row.
    #[must_use]
    pub fn outcome_digest_hex(&self) -> Option<&str> {
        self.outcome_digest_hex.as_deref()
    }

    /// Return the admission-envelope digest bound to a committed outcome.
    #[must_use]
    pub fn admission_envelope_digest_hex(&self) -> Option<&str> {
        self.admission_envelope_digest_hex.as_deref()
    }

    /// Return the finalized block height anchoring a committed outcome.
    #[must_use]
    pub const fn finalized_block_height(&self) -> Option<u64> {
        self.finalized_block_height
    }

    /// Return the finalized block hash anchoring a committed outcome.
    #[must_use]
    pub fn finalized_block_hash_hex(&self) -> Option<&str> {
        self.finalized_block_hash_hex.as_deref()
    }

    /// Return the committing block timestamp in milliseconds.
    #[must_use]
    pub const fn committed_at_ms(&self) -> Option<u64> {
        self.committed_at_ms
    }

    /// Return the governed PDP challenge identifier when present.
    #[must_use]
    pub fn challenge_id_hex(&self) -> Option<&str> {
        self.challenge_id_hex.as_deref()
    }

    /// Return the canonical proof kind.
    #[must_use]
    pub const fn proof_kind(&self) -> ProofKind {
        self.proof_kind
    }

    /// Return the terminal verification status.
    #[must_use]
    pub const fn status(&self) -> VerificationStatus {
        self.status
    }

    /// Return the canonical terminal failure reason when present.
    #[must_use]
    pub fn failure_reason(&self) -> Option<&str> {
        self.failure_reason.as_deref()
    }

    /// Return the observed verification latency in milliseconds.
    #[must_use]
    pub const fn latency_ms(&self) -> Option<u32> {
        self.latency_ms
    }

    /// Return the requested PoTR deadline in milliseconds.
    #[must_use]
    pub const fn deadline_ms(&self) -> Option<u32> {
        self.deadline_ms
    }

    /// Return the flat sample index when this is a PoR row.
    #[must_use]
    pub const fn sample_index(&self) -> Option<u32> {
        self.sample_index
    }

    /// Return the PoR chunk index when present.
    #[must_use]
    pub const fn chunk_index(&self) -> Option<u32> {
        self.chunk_index
    }

    /// Return the PoR segment index when present.
    #[must_use]
    pub const fn segment_index(&self) -> Option<u32> {
        self.segment_index
    }

    /// Return the PoR leaf index when present.
    #[must_use]
    pub const fn leaf_index(&self) -> Option<u32> {
        self.leaf_index
    }

    /// Return the canonical storage tier hint or signed PoTR tier.
    #[must_use]
    pub const fn tier(&self) -> Option<ProofTier> {
        self.tier
    }

    /// Return the signed PoTR trace identifier when present.
    #[must_use]
    pub fn trace_id(&self) -> Option<&str> {
        self.trace_id.as_deref()
    }

    /// Return the verified PoR witness when present.
    #[must_use]
    pub const fn por_proof(&self) -> Option<&PorProof> {
        self.por_proof.as_ref()
    }

    /// Return the exact final signed PoTR receipt when present.
    #[must_use]
    pub const fn potr_receipt(&self) -> Option<&PotrReceiptV1> {
        self.potr_receipt.as_ref()
    }

    /// Return the signed PoTR receipt recording timestamp.
    #[must_use]
    pub const fn recorded_at_ms(&self) -> Option<u64> {
        self.recorded_at_ms
    }

    /// Parses an item from a Norito JSON value.
    pub fn from_json(value: &Value) -> Result<Self, String> {
        let obj = value
            .as_object()
            .ok_or_else(|| "proof stream item must be a JSON object".to_string())?;

        for retired in [
            "verification_status",
            "provider_id",
            "latency",
            "sample_index",
        ] {
            if obj.contains_key(retired) {
                return Err(format!(
                    "proof stream item contains retired field `{retired}`"
                ));
            }
        }

        const CANONICAL_FIELDS: &[&str] = &[
            "manifest_digest_hex",
            "provider_id_hex",
            "outcome_identity_hex",
            "outcome_digest_hex",
            "admission_envelope_digest_hex",
            "finalized_block_height",
            "finalized_block_hash_hex",
            "committed_at_ms",
            "challenge_id_hex",
            "proof_kind",
            "result",
            "failure_reason",
            "latency_ms",
            "deadline_ms",
            "leaf_index_flat",
            "chunk_index",
            "segment_index",
            "leaf_index",
            "tier",
            "trace_id",
            "proof",
            "receipt_b64",
            "recorded_at_ms",
        ];
        for field in obj.keys() {
            if !CANONICAL_FIELDS.contains(&field.as_str()) {
                return Err(format!(
                    "proof stream item contains unknown field `{field}`"
                ));
            }
        }

        let proof_kind = match obj.get("proof_kind") {
            Some(Value::String(kind)) => {
                ProofKind::parse(kind).map_err(|error| error.to_string())?
            }
            Some(_) => return Err("`proof_kind` must be a string".to_string()),
            None => return Err("proof stream item missing `proof_kind` field".to_string()),
        };

        let status = match obj.get("result") {
            Some(Value::String(result)) => VerificationStatus::parse(result)?,
            Some(_) => return Err("`result` must be a string".to_string()),
            None => return Err("proof stream item missing `result` field".to_string()),
        };

        let manifest_digest_hex = match obj.get("manifest_digest_hex") {
            Some(Value::String(digest)) => {
                canonical_nonzero_hex::<32>(digest, "manifest_digest_hex")?
            }
            Some(_) => return Err("`manifest_digest_hex` must be a string".to_string()),
            None => {
                return Err("proof stream item missing `manifest_digest_hex` field".to_string());
            }
        };
        let provider_id_hex = match obj.get("provider_id_hex") {
            Some(Value::String(provider)) => {
                canonical_nonzero_hex::<32>(provider, "provider_id_hex")?
            }
            Some(_) => return Err("`provider_id_hex` must be a string".to_string()),
            None => return Err("proof stream item missing `provider_id_hex` field".to_string()),
        };
        let outcome_identity_hex = match obj.get("outcome_identity_hex") {
            Some(Value::String(identity)) => Some(canonical_nonzero_hex::<32>(
                identity,
                "outcome_identity_hex",
            )?),
            Some(_) => {
                return Err("`outcome_identity_hex` must be a string when present".to_string());
            }
            None => None,
        };
        let outcome_digest_hex = match obj.get("outcome_digest_hex") {
            Some(Value::String(digest)) => {
                Some(canonical_nonzero_hex::<32>(digest, "outcome_digest_hex")?)
            }
            Some(_) => {
                return Err("`outcome_digest_hex` must be a string when present".to_string());
            }
            None => None,
        };
        let admission_envelope_digest_hex = match obj.get("admission_envelope_digest_hex") {
            Some(Value::String(digest)) => Some(canonical_nonzero_hex::<32>(
                digest,
                "admission_envelope_digest_hex",
            )?),
            Some(_) => {
                return Err(
                    "`admission_envelope_digest_hex` must be a string when present".to_string(),
                );
            }
            None => None,
        };
        let finalized_block_height = optional_u64_field(obj, "finalized_block_height")?;
        let finalized_block_hash_hex = match obj.get("finalized_block_hash_hex") {
            Some(Value::String(hash)) => Some(canonical_nonzero_hex::<32>(
                hash,
                "finalized_block_hash_hex",
            )?),
            Some(_) => {
                return Err("`finalized_block_hash_hex` must be a string when present".to_string());
            }
            None => None,
        };
        let committed_at_ms = optional_u64_field(obj, "committed_at_ms")?;
        let challenge_id_hex = match obj.get("challenge_id_hex") {
            Some(Value::String(challenge)) => {
                Some(canonical_nonzero_hex::<32>(challenge, "challenge_id_hex")?)
            }
            Some(_) => {
                return Err("`challenge_id_hex` must be a string when present".to_string());
            }
            None => None,
        };

        let failure_reason = match obj.get("failure_reason") {
            Some(Value::String(reason)) => Some(canonical_failure_reason(reason)?),
            Some(_) => return Err("`failure_reason` must be a string when present".to_string()),
            None => None,
        };
        match (status, failure_reason.is_some()) {
            (VerificationStatus::Failure, false) => {
                return Err("failed proof stream item requires `failure_reason`".to_string());
            }
            (VerificationStatus::Success, true) => {
                return Err("non-failed proof stream item must omit `failure_reason`".to_string());
            }
            _ => {}
        }

        let latency_ms = optional_u32_field(obj, "latency_ms")?;
        let deadline_ms = optional_u32_field(obj, "deadline_ms")?;
        let sample_index = optional_u32_field(obj, "leaf_index_flat")?;
        let chunk_index = optional_u32_field(obj, "chunk_index")?;
        let segment_index = optional_u32_field(obj, "segment_index")?;
        let leaf_index = optional_u32_field(obj, "leaf_index")?;

        let tier = match obj.get("tier") {
            Some(Value::String(tier)) => {
                Some(ProofTier::parse(tier).map_err(|error| error.to_string())?)
            }
            Some(_) => return Err("`tier` must be a string when present".to_string()),
            None => None,
        };

        let por_proof = obj
            .get("proof")
            .map(proof_from_value)
            .transpose()
            .map_err(|err| format!("failed to decode proof payload: {err}"))?;
        let potr_receipt = match obj.get("receipt_b64") {
            Some(Value::String(encoded)) => Some(decode_canonical_potr_receipt(encoded)?),
            Some(_) => return Err("`receipt_b64` must be a string".to_string()),
            None => None,
        };
        let trace_id = match obj.get("trace_id") {
            Some(Value::String(trace_id)) => {
                Some(canonical_nonzero_hex::<16>(trace_id, "trace_id")?)
            }
            Some(_) => return Err("`trace_id` must be a string when present".to_string()),
            None => None,
        };
        let recorded_at_ms = match obj.get("recorded_at_ms") {
            Some(value) => Some(value.as_u64().ok_or_else(|| {
                "`recorded_at_ms` must be an unsigned integer when present".to_string()
            })?),
            None => None,
        };

        match proof_kind {
            ProofKind::Por => {
                if status != VerificationStatus::Success
                    || latency_ms.is_none()
                    || sample_index.is_none()
                    || chunk_index.is_none()
                    || segment_index.is_none()
                    || leaf_index.is_none()
                    || por_proof.is_none()
                {
                    return Err(
                        "PoR item requires a successful proof, latency, and complete sample indices"
                            .to_string(),
                    );
                }
                let proof = por_proof
                    .as_ref()
                    .expect("PoR required-field check guarantees a proof");
                let projected_indices = (
                    u32::try_from(proof.chunk_index).ok(),
                    u32::try_from(proof.segment_index).ok(),
                    u32::try_from(proof.leaf_index).ok(),
                );
                if projected_indices != (chunk_index, segment_index, leaf_index) {
                    return Err(
                        "PoR item indices do not match the canonical proof witness".to_string()
                    );
                }
                let derived_root = crate::hash_root(proof.payload_len, &proof.chunk_roots);
                if !proof.verify(&derived_root) {
                    return Err("PoR item contains an internally invalid proof witness".to_string());
                }
                if challenge_id_hex.is_some()
                    || deadline_ms.is_some()
                    || potr_receipt.is_some()
                    || trace_id.is_some()
                    || recorded_at_ms.is_some()
                    || outcome_identity_hex.is_some()
                    || outcome_digest_hex.is_some()
                    || admission_envelope_digest_hex.is_some()
                    || finalized_block_height.is_some()
                    || finalized_block_hash_hex.is_some()
                    || committed_at_ms.is_some()
                {
                    return Err(
                        "PoR item contains committed-outcome, PDP challenge, PoTR deadline, or signed-receipt fields"
                            .to_string(),
                    );
                }
            }
            ProofKind::Pdp => {
                let challenge = challenge_id_hex
                    .as_ref()
                    .ok_or_else(|| "PDP item requires `challenge_id_hex`".to_string())?;
                if outcome_identity_hex.as_ref() != Some(challenge) {
                    return Err(
                        "PDP committed outcome identity must equal its challenge id".to_string()
                    );
                }
                if outcome_digest_hex.is_none()
                    || admission_envelope_digest_hex.is_none()
                    || finalized_block_height.is_none_or(|height| height == 0)
                    || finalized_block_hash_hex.is_none()
                    || committed_at_ms.is_none_or(|timestamp| timestamp == 0)
                {
                    return Err(
                        "PDP item requires complete committed-outcome provenance".to_string()
                    );
                }
                if status == VerificationStatus::Failure
                    && !matches!(
                        failure_reason.as_deref(),
                        Some(
                            "deadline_expired"
                                | "submission_late"
                                | "future_timestamp"
                                | "invalid_proof"
                                | "admission_revoked"
                                | "admission_inactive"
                                | "storage_unavailable"
                        )
                    )
                {
                    return Err(
                        "PDP failure reason is not a canonical committed terminal status"
                            .to_string(),
                    );
                }
                if deadline_ms.is_some()
                    || latency_ms.is_some()
                    || sample_index.is_some()
                    || chunk_index.is_some()
                    || segment_index.is_some()
                    || leaf_index.is_some()
                    || por_proof.is_some()
                    || potr_receipt.is_some()
                    || trace_id.is_some()
                    || recorded_at_ms.is_some()
                {
                    return Err("PDP item contains fields reserved for PoR or PoTR".to_string());
                }
            }
            ProofKind::Potr => {
                if challenge_id_hex.is_some()
                    || sample_index.is_some()
                    || chunk_index.is_some()
                    || segment_index.is_some()
                    || leaf_index.is_some()
                    || por_proof.is_some()
                {
                    return Err("PoTR item contains fields reserved for PDP or PoR".to_string());
                }
                let receipt = potr_receipt
                    .as_ref()
                    .ok_or_else(|| "PoTR item requires final signed `receipt_b64`".to_string())?;
                let receipt_identity =
                    hex::encode(receipt.request_scope_digest().map_err(|error| {
                        format!("failed to scope signed PoTR receipt: {error}")
                    })?);
                let receipt_digest =
                    hex::encode(receipt.signed_receipt_digest().map_err(|error| {
                        format!("failed to digest signed PoTR receipt: {error}")
                    })?);
                if outcome_identity_hex.as_deref() != Some(receipt_identity.as_str())
                    || outcome_digest_hex.as_deref() != Some(receipt_digest.as_str())
                    || admission_envelope_digest_hex.is_none()
                    || finalized_block_height.is_none_or(|height| height == 0)
                    || finalized_block_hash_hex.is_none()
                    || committed_at_ms.is_none_or(|timestamp| timestamp == 0)
                {
                    return Err(
                        "PoTR item is missing or disagrees with committed outcome provenance"
                            .to_string(),
                    );
                }
                let receipt_manifest = hex::encode(receipt.manifest_digest);
                let receipt_provider = hex::encode(receipt.provider_id);
                if manifest_digest_hex != receipt_manifest || provider_id_hex != receipt_provider {
                    return Err(
                        "PoTR JSON projection identity does not match the signed receipt"
                            .to_string(),
                    );
                }
                if deadline_ms != Some(receipt.deadline_ms)
                    || latency_ms != Some(receipt.latency_ms)
                    || recorded_at_ms != Some(receipt.recorded_at_ms)
                {
                    return Err(
                        "PoTR JSON projection timing does not match the signed receipt".to_string(),
                    );
                }
                if committed_at_ms.is_some_and(|committed| receipt.recorded_at_ms > committed) {
                    return Err(
                        "PoTR signed receipt was recorded after its committing block".to_string(),
                    );
                }
                let receipt_tier = match receipt.tier {
                    sorafs_manifest::ProofStreamTier::Hot => "hot",
                    sorafs_manifest::ProofStreamTier::Warm => "warm",
                    sorafs_manifest::ProofStreamTier::Archive => "archive",
                };
                if tier.map(ProofTier::as_str) != Some(receipt_tier) {
                    return Err(
                        "PoTR JSON projection tier does not match the signed receipt".to_string(),
                    );
                }
                let receipt_trace = receipt.trace_id.map(hex::encode);
                if trace_id != receipt_trace {
                    return Err(
                        "PoTR JSON projection trace id does not match the signed receipt"
                            .to_string(),
                    );
                }
                let (expected_status, expected_reason) = match receipt.status {
                    PotrStatus::Success => (VerificationStatus::Success, None),
                    PotrStatus::MissedDeadline => {
                        (VerificationStatus::Failure, Some("missed_deadline"))
                    }
                    PotrStatus::ProviderError => {
                        (VerificationStatus::Failure, Some("provider_error"))
                    }
                    PotrStatus::GatewayError => {
                        (VerificationStatus::Failure, Some("gateway_error"))
                    }
                    PotrStatus::ClientCancelled => {
                        (VerificationStatus::Failure, Some("client_cancelled"))
                    }
                };
                if status != expected_status || failure_reason.as_deref() != expected_reason {
                    return Err(
                        "PoTR JSON projection result does not match the signed receipt".to_string(),
                    );
                }
            }
        }

        Ok(Self {
            manifest_digest_hex,
            provider_id_hex,
            outcome_identity_hex,
            outcome_digest_hex,
            admission_envelope_digest_hex,
            finalized_block_height,
            finalized_block_hash_hex,
            committed_at_ms,
            challenge_id_hex,
            proof_kind,
            status,
            failure_reason,
            latency_ms,
            deadline_ms,
            sample_index,
            chunk_index,
            segment_index,
            leaf_index,
            tier,
            trace_id,
            por_proof,
            potr_receipt,
            recorded_at_ms,
        })
    }

    /// Parses an NDJSON line emitted by the gateway.
    pub fn from_ndjson(bytes: &[u8]) -> Result<Self, String> {
        let value: Value = from_slice(bytes)
            .map_err(|err| format!("failed to parse proof stream item JSON: {err}"))?;
        Self::from_json(&value)
    }

    /// Serialises the item into a JSON value suitable for summaries.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert(
            "manifest_digest_hex".into(),
            Value::from(self.manifest_digest_hex.clone()),
        );
        map.insert(
            "provider_id_hex".into(),
            Value::from(self.provider_id_hex.clone()),
        );
        if let Some(identity) = &self.outcome_identity_hex {
            map.insert("outcome_identity_hex".into(), Value::from(identity.clone()));
        }
        if let Some(digest) = &self.outcome_digest_hex {
            map.insert("outcome_digest_hex".into(), Value::from(digest.clone()));
        }
        if let Some(digest) = &self.admission_envelope_digest_hex {
            map.insert(
                "admission_envelope_digest_hex".into(),
                Value::from(digest.clone()),
            );
        }
        if let Some(height) = self.finalized_block_height {
            map.insert("finalized_block_height".into(), Value::from(height));
        }
        if let Some(hash) = &self.finalized_block_hash_hex {
            map.insert("finalized_block_hash_hex".into(), Value::from(hash.clone()));
        }
        if let Some(timestamp) = self.committed_at_ms {
            map.insert("committed_at_ms".into(), Value::from(timestamp));
        }
        if let Some(challenge) = &self.challenge_id_hex {
            map.insert("challenge_id_hex".into(), Value::from(challenge.clone()));
        }
        map.insert("proof_kind".into(), Value::from(self.proof_kind.as_str()));
        map.insert("result".into(), Value::from(self.status.as_str()));
        if let Some(reason) = &self.failure_reason {
            map.insert("failure_reason".into(), Value::from(reason.clone()));
        }
        if let Some(latency) = self.latency_ms {
            map.insert("latency_ms".into(), Value::from(latency as u64));
        }
        if let Some(deadline) = self.deadline_ms {
            map.insert("deadline_ms".into(), Value::from(deadline as u64));
        }
        if let Some(index) = self.sample_index {
            map.insert("leaf_index_flat".into(), Value::from(index as u64));
        }
        if let Some(index) = self.chunk_index {
            map.insert("chunk_index".into(), Value::from(index as u64));
        }
        if let Some(index) = self.segment_index {
            map.insert("segment_index".into(), Value::from(index as u64));
        }
        if let Some(index) = self.leaf_index {
            map.insert("leaf_index".into(), Value::from(index as u64));
        }
        if let Some(tier) = self.tier {
            map.insert("tier".into(), Value::from(tier.as_str()));
        }
        if let Some(trace) = &self.trace_id {
            map.insert("trace_id".into(), Value::from(trace.clone()));
        }
        if let Some(recorded) = self.recorded_at_ms {
            map.insert("recorded_at_ms".into(), Value::from(recorded));
        }
        if let Some(proof) = &self.por_proof {
            map.insert("proof".into(), crate::por_json::proof_to_value(proof));
        }
        if let Some(receipt) = &self.potr_receipt {
            let encoded = norito::to_bytes(receipt)
                .expect("validated signed PoTR receipt must remain canonically encodable");
            map.insert(
                "receipt_b64".into(),
                Value::from(BASE64_STANDARD.encode(encoded)),
            );
        }
        Value::Object(map)
    }
}

/// Aggregated metrics derived from a proof stream.
#[derive(Debug, Clone, Default)]
pub struct ProofStreamMetrics {
    /// Total number of items processed.
    pub item_total: u64,
    /// Number of successful items.
    pub success_total: u64,
    /// Number of failed items.
    pub failure_total: u64,
    /// Failure counts grouped by reason.
    pub failure_by_reason: BTreeMap<String, u64>,
    latencies_ms: Vec<u32>,
    latency_count: u64,
    latency_sum_ms: u128,
    latency_min_ms: Option<u32>,
    latency_max_ms: Option<u32>,
    latency_truncated: bool,
}

impl ProofStreamMetrics {
    /// Records a streaming item into the aggregated metrics.
    pub fn record(&mut self, item: &ProofStreamItem) {
        self.item_total += 1;
        if let Some(latency) = item.latency_ms {
            self.latency_count += 1;
            self.latency_sum_ms = self.latency_sum_ms.saturating_add(u128::from(latency));
            self.latency_min_ms = Some(match self.latency_min_ms {
                Some(current) => current.min(latency),
                None => latency,
            });
            self.latency_max_ms = Some(match self.latency_max_ms {
                Some(current) => current.max(latency),
                None => latency,
            });
            if self.latencies_ms.len() < LATENCY_SAMPLE_LIMIT {
                self.latencies_ms.push(latency);
            } else {
                self.latency_truncated = true;
            }
        }
        if item.status.is_failure() {
            self.failure_total += 1;
            let reason = item
                .failure_reason
                .clone()
                .unwrap_or_else(|| "unspecified".to_string());
            *self.failure_by_reason.entry(reason).or_insert(0) += 1;
        } else if matches!(item.status, VerificationStatus::Success) {
            self.success_total += 1;
        }
    }

    fn latency_stats(&self) -> Option<LatencyStats> {
        if self.latency_count == 0 || self.latencies_ms.is_empty() {
            return None;
        }
        let mut sorted = self.latencies_ms.clone();
        sorted.sort_unstable();
        let sample_count = sorted.len() as u64;
        let min = self.latency_min_ms.unwrap_or(0);
        let max = self.latency_max_ms.unwrap_or(0);
        let average = if self.latency_count == 0 {
            0.0
        } else {
            self.latency_sum_ms as f64 / self.latency_count as f64
        };
        let percentile = |p: f64| -> u32 {
            if sample_count == 0 {
                return 0;
            }
            let rank = ((p / 100.0) * (sample_count as f64 - 1.0)).round() as usize;
            sorted
                .get(rank.min(sorted.len().saturating_sub(1)))
                .copied()
                .unwrap_or(0)
        };
        Some(LatencyStats {
            min,
            max,
            average,
            p50: percentile(50.0),
            p95: percentile(95.0),
            sampled_count: sample_count,
        })
    }

    /// Serialises the metrics into a JSON value.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert("item_total".into(), Value::from(self.item_total));
        map.insert("success_total".into(), Value::from(self.success_total));
        map.insert("failure_total".into(), Value::from(self.failure_total));

        let mut reasons = Map::new();
        for (reason, count) in &self.failure_by_reason {
            reasons.insert(reason.clone(), Value::from(*count));
        }
        map.insert("failure_by_reason".into(), Value::Object(reasons));

        if let Some(stats) = self.latency_stats() {
            let mut latency = Map::new();
            latency.insert("count".into(), Value::from(self.latency_count));
            latency.insert("sampled_count".into(), Value::from(stats.sampled_count));
            latency.insert("min_ms".into(), Value::from(stats.min as u64));
            latency.insert("max_ms".into(), Value::from(stats.max as u64));
            latency.insert("p50_ms".into(), Value::from(stats.p50 as u64));
            latency.insert("p95_ms".into(), Value::from(stats.p95 as u64));
            latency.insert("average_ms".into(), Value::from(stats.average));
            latency.insert("truncated".into(), Value::from(self.latency_truncated));
            map.insert("latency_ms".into(), Value::Object(latency));
        }

        Value::Object(map)
    }
}

#[derive(Clone, Debug)]
struct LatencyStats {
    min: u32,
    max: u32,
    average: f64,
    p50: u32,
    p95: u32,
    sampled_count: u64,
}

const LATENCY_SAMPLE_LIMIT: usize = 4096;

/// Final summary returned after processing a stream.
#[derive(Debug, Clone)]
pub struct ProofStreamSummary {
    /// Aggregated metrics.
    pub metrics: ProofStreamMetrics,
    /// Sampled failures (first few entries for troubleshooting).
    pub failure_samples: Vec<ProofStreamItem>,
}

impl ProofStreamSummary {
    /// Creates a new summary from metrics and failure samples.
    #[must_use]
    pub fn new(metrics: ProofStreamMetrics, failure_samples: Vec<ProofStreamItem>) -> Self {
        Self {
            metrics,
            failure_samples,
        }
    }

    /// Serialises the summary into a Norito JSON object.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert("metrics".into(), self.metrics.to_json());
        if !self.failure_samples.is_empty() {
            let samples = self
                .failure_samples
                .iter()
                .map(ProofStreamItem::to_json)
                .collect::<Vec<_>>();
            map.insert("failure_samples".into(), Value::Array(samples));
        }
        Value::Object(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical_por_sample() -> (usize, PorProof) {
        let payload = (0_u16..512)
            .map(|value| u8::try_from(value % 251).expect("fixture byte"))
            .collect::<Vec<_>>();
        let mut store = crate::ChunkStore::new();
        store
            .ingest_bytes(&payload)
            .expect("ingest canonical PoR fixture payload");
        store
            .sample_leaves(1, 7, &payload)
            .expect("sample canonical PoR fixture")
            .into_iter()
            .next()
            .expect("one canonical PoR sample")
    }

    fn canonical_item_map() -> Map {
        let (flat_index, proof) = canonical_por_sample();
        let mut map = crate::por_json::sample_to_map(flat_index, &proof);
        map.insert("manifest_digest_hex".into(), Value::from("aa".repeat(32)));
        map.insert("provider_id_hex".into(), Value::from("bb".repeat(32)));
        map.insert("proof_kind".into(), Value::from("por"));
        map.insert("result".into(), Value::from("success"));
        map.insert("latency_ms".into(), Value::from(42));
        map
    }

    fn canonical_pdp_item_map() -> Map {
        let challenge_id = "11".repeat(32);
        let mut map = Map::new();
        map.insert("manifest_digest_hex".into(), Value::from("22".repeat(32)));
        map.insert("provider_id_hex".into(), Value::from("33".repeat(32)));
        map.insert(
            "outcome_identity_hex".into(),
            Value::from(challenge_id.clone()),
        );
        map.insert("outcome_digest_hex".into(), Value::from("44".repeat(32)));
        map.insert(
            "admission_envelope_digest_hex".into(),
            Value::from("55".repeat(32)),
        );
        map.insert("finalized_block_height".into(), Value::from(17_u64));
        map.insert(
            "finalized_block_hash_hex".into(),
            Value::from("66".repeat(32)),
        );
        map.insert("committed_at_ms".into(), Value::from(1_700_000_010_000_u64));
        map.insert("challenge_id_hex".into(), Value::from(challenge_id));
        map.insert("proof_kind".into(), Value::from("pdp"));
        map.insert("result".into(), Value::from("failure"));
        map.insert("failure_reason".into(), Value::from("invalid_proof"));
        map
    }

    fn canonical_potr_item_map() -> (Map, PotrReceiptV1) {
        let canonical_receipt =
            include_bytes!("../../../fixtures/sorafs_manifest/potr/receipt_v1.to");
        let receipt: PotrReceiptV1 =
            decode_from_bytes(canonical_receipt).expect("decode canonical signed PoTR fixture");
        receipt
            .validate()
            .expect("canonical signed PoTR fixture validates");
        assert_eq!(
            receipt
                .signed_receipt_bytes()
                .expect("re-encode signed PoTR fixture"),
            canonical_receipt
        );
        let identity = receipt
            .request_scope_digest()
            .expect("scope canonical signed PoTR fixture");
        let digest = receipt
            .signed_receipt_digest()
            .expect("digest canonical signed PoTR fixture");

        let mut map = Map::new();
        map.insert(
            "manifest_digest_hex".into(),
            Value::from(hex::encode(receipt.manifest_digest)),
        );
        map.insert(
            "provider_id_hex".into(),
            Value::from(hex::encode(receipt.provider_id)),
        );
        map.insert(
            "outcome_identity_hex".into(),
            Value::from(hex::encode(identity)),
        );
        map.insert(
            "outcome_digest_hex".into(),
            Value::from(hex::encode(digest)),
        );
        map.insert(
            "admission_envelope_digest_hex".into(),
            Value::from("77".repeat(32)),
        );
        map.insert("finalized_block_height".into(), Value::from(23_u64));
        map.insert(
            "finalized_block_hash_hex".into(),
            Value::from("88".repeat(32)),
        );
        map.insert(
            "committed_at_ms".into(),
            Value::from(receipt.recorded_at_ms + 1),
        );
        map.insert("proof_kind".into(), Value::from("potr"));
        map.insert("result".into(), Value::from("success"));
        map.insert(
            "latency_ms".into(),
            Value::from(u64::from(receipt.latency_ms)),
        );
        map.insert(
            "deadline_ms".into(),
            Value::from(u64::from(receipt.deadline_ms)),
        );
        map.insert("tier".into(), Value::from(receipt.tier.as_str()));
        map.insert(
            "trace_id".into(),
            Value::from(hex::encode(receipt.trace_id.expect("fixture trace id"))),
        );
        map.insert(
            "receipt_b64".into(),
            Value::from(BASE64_STANDARD.encode(canonical_receipt)),
        );
        map.insert("recorded_at_ms".into(), Value::from(receipt.recorded_at_ms));
        (map, receipt)
    }

    #[test]
    fn item_parses_from_ndjson() {
        let manifest_digest_hex = "aa".repeat(32);
        let provider_id_hex = "bb".repeat(32);
        let map = canonical_item_map();
        let expected_sample_index = map
            .get("leaf_index_flat")
            .and_then(Value::as_u64)
            .expect("canonical flat sample index");
        let line =
            norito::json::to_string(&Value::Object(map)).expect("serialize canonical PoR item");
        let item = ProofStreamItem::from_ndjson(line.as_bytes()).expect("parse item");
        assert_eq!(item.manifest_digest_hex, manifest_digest_hex);
        assert_eq!(item.provider_id_hex, provider_id_hex);
        assert_eq!(item.sample_index, Some(expected_sample_index as u32));
        assert!(matches!(item.status, VerificationStatus::Success));
        assert!(item.por_proof.is_some());
        assert_eq!(item.deadline_ms, None);
        assert_eq!(item.recorded_at_ms, None);
    }

    #[test]
    fn item_rejects_u32_field_overflow_instead_of_wrapping() {
        for field in [
            "latency_ms",
            "deadline_ms",
            "leaf_index_flat",
            "chunk_index",
            "segment_index",
            "leaf_index",
        ] {
            let mut map = canonical_item_map();
            map.insert(field.into(), Value::from(u64::from(u32::MAX) + 1));

            let error = ProofStreamItem::from_json(&Value::Object(map))
                .expect_err("overflowing u32 field must be rejected");
            assert!(
                error.contains(field) && error.contains("must fit in u32"),
                "unexpected error for {field}: {error}"
            );
        }
    }

    #[test]
    fn item_accepts_u32_max_for_every_bounded_field() {
        let mut map = canonical_item_map();
        map.insert("latency_ms".into(), Value::from(u32::MAX));

        let item = ProofStreamItem::from_json(&Value::Object(map))
            .expect("u32::MAX latency must remain representable");
        assert_eq!(item.latency_ms, Some(u32::MAX));
    }

    #[test]
    fn item_rejects_present_non_integer_u32_field() {
        let mut map = canonical_item_map();
        map.insert("latency_ms".into(), Value::from("42"));

        let error = ProofStreamItem::from_json(&Value::Object(map))
            .expect_err("present non-integer bounded field must be rejected");
        assert!(error.contains("`latency_ms` must be an unsigned 32-bit integer"));
    }

    #[test]
    fn item_rejects_unknown_fields_and_explicit_null_optionals() {
        let mut map = canonical_item_map();
        map.insert("manifest_cid_hex".into(), Value::from("aa".repeat(32)));
        let error = ProofStreamItem::from_json(&Value::Object(map))
            .expect_err("unknown response fields must fail closed");
        assert!(error.contains("unknown field `manifest_cid_hex`"));

        for field in [
            "outcome_identity_hex",
            "outcome_digest_hex",
            "admission_envelope_digest_hex",
            "finalized_block_height",
            "finalized_block_hash_hex",
            "committed_at_ms",
            "challenge_id_hex",
            "failure_reason",
            "latency_ms",
            "deadline_ms",
            "leaf_index_flat",
            "chunk_index",
            "segment_index",
            "leaf_index",
            "tier",
            "trace_id",
            "proof",
            "receipt_b64",
            "recorded_at_ms",
        ] {
            let mut map = canonical_item_map();
            map.insert(field.into(), Value::Null);
            let error = ProofStreamItem::from_json(&Value::Object(map))
                .expect_err("optional fields must be omitted instead of encoded as null");
            assert!(
                error.contains(field) || field == "proof",
                "unexpected error for `{field}`: {error}"
            );
        }
    }

    #[test]
    fn item_rejects_non_string_optional_text_fields() {
        for field in [
            "outcome_identity_hex",
            "outcome_digest_hex",
            "admission_envelope_digest_hex",
            "finalized_block_hash_hex",
            "challenge_id_hex",
            "failure_reason",
            "tier",
            "trace_id",
            "receipt_b64",
        ] {
            let mut map = canonical_item_map();
            map.insert(field.into(), Value::from(7));
            let error = ProofStreamItem::from_json(&Value::Object(map))
                .expect_err("present optional text fields must be strings");
            assert!(
                error.contains(field),
                "unexpected error for `{field}`: {error}"
            );
        }
    }

    #[test]
    fn item_requires_failure_reason_exactly_for_failed_results() {
        let mut failed = canonical_item_map();
        failed.insert("result".into(), Value::from("failure"));
        let error = ProofStreamItem::from_json(&Value::Object(failed))
            .expect_err("failed item without a reason must fail closed");
        assert!(error.contains("requires `failure_reason`"));

        let mut success = canonical_item_map();
        success.insert("failure_reason".into(), Value::from("provider_error"));
        let error = ProofStreamItem::from_json(&Value::Object(success))
            .expect_err("successful item must not carry a failure reason");
        assert!(error.contains("must omit `failure_reason`"));
    }

    #[test]
    fn item_rejects_retired_aliases_and_noncanonical_labels() {
        for retired in [
            "verification_status",
            "provider_id",
            "latency",
            "sample_index",
        ] {
            let mut map = canonical_item_map();
            map.insert(retired.into(), Value::from("retired"));
            let error = ProofStreamItem::from_json(&Value::Object(map))
                .expect_err("retired response alias must fail closed");
            assert!(error.contains("retired field") && error.contains(retired));
        }

        for invalid_result in ["ok", "passed", "pending", "SUCCESS", " success"] {
            let mut map = canonical_item_map();
            map.insert("result".into(), Value::from(invalid_result));
            let error = ProofStreamItem::from_json(&Value::Object(map))
                .expect_err("noncanonical result must fail closed");
            assert!(error.contains("unsupported proof result"));
        }
    }

    #[test]
    fn item_rejects_por_outer_index_or_witness_tampering() {
        for field in ["chunk_index", "segment_index", "leaf_index"] {
            let mut wrong_index = canonical_item_map();
            let current = wrong_index
                .get(field)
                .and_then(Value::as_u64)
                .expect("canonical proof index");
            wrong_index.insert(field.into(), Value::from(current + 1));
            let error = ProofStreamItem::from_json(&Value::Object(wrong_index))
                .expect_err("outer PoR index contradiction must fail closed");
            assert!(error.contains("indices do not match"));
        }

        let mut wrong_witness = canonical_item_map();
        let proof = wrong_witness
            .get_mut("proof")
            .and_then(Value::as_object_mut)
            .expect("canonical proof object");
        let leaf_bytes = proof
            .get("leaf_bytes_hex")
            .and_then(Value::as_str)
            .expect("canonical leaf bytes");
        let (first_byte, remaining_bytes) = leaf_bytes.split_at(2);
        let replacement_prefix = if first_byte == "00" { "ff" } else { "00" };
        let replacement = format!("{replacement_prefix}{remaining_bytes}");
        proof.insert("leaf_bytes_hex".into(), Value::from(replacement));
        let error = ProofStreamItem::from_json(&Value::Object(wrong_witness))
            .expect_err("internally invalid PoR witness must fail closed");
        assert!(error.contains("internally invalid proof witness"));
    }

    #[test]
    fn item_accepts_only_terminal_chain_backed_pdp_projection() {
        let failed = ProofStreamItem::from_json(&Value::Object(canonical_pdp_item_map()))
            .expect("canonical committed PDP failure");
        assert_eq!(failed.proof_kind, ProofKind::Pdp);
        assert_eq!(failed.status, VerificationStatus::Failure);
        assert_eq!(failed.failure_reason.as_deref(), Some("invalid_proof"));
        assert_eq!(
            failed.outcome_identity_hex.as_deref(),
            failed.challenge_id_hex.as_deref()
        );
        ProofStreamItem::from_json(&failed.to_json()).expect("PDP JSON roundtrip");

        let mut accepted = canonical_pdp_item_map();
        accepted.insert("result".into(), Value::from("success"));
        accepted.remove("failure_reason");
        let accepted = ProofStreamItem::from_json(&Value::Object(accepted))
            .expect("canonical committed PDP success");
        assert_eq!(accepted.status, VerificationStatus::Success);

        for reason in [
            "provider_error",
            "missed_deadline",
            "timeout",
            "invalid-proof",
            "INVALID_PROOF",
        ] {
            let mut invalid = canonical_pdp_item_map();
            invalid.insert("failure_reason".into(), Value::from(reason));
            let error = ProofStreamItem::from_json(&Value::Object(invalid))
                .expect_err("non-ledger PDP terminal reason must fail closed");
            assert!(
                error.contains("canonical committed terminal status")
                    || error.contains("canonical lowercase snake-case"),
                "unexpected error for `{reason}`: {error}"
            );
        }
    }

    #[test]
    fn item_rejects_incomplete_or_contradictory_pdp_provenance() {
        for required in [
            "outcome_identity_hex",
            "outcome_digest_hex",
            "admission_envelope_digest_hex",
            "finalized_block_height",
            "finalized_block_hash_hex",
            "committed_at_ms",
            "challenge_id_hex",
        ] {
            let mut invalid = canonical_pdp_item_map();
            invalid.remove(required);
            let error = ProofStreamItem::from_json(&Value::Object(invalid))
                .expect_err("incomplete PDP provenance must fail closed");
            assert!(
                error.contains("PDP") || error.contains(required),
                "unexpected error after removing `{required}`: {error}"
            );
        }

        let mut wrong_identity = canonical_pdp_item_map();
        wrong_identity.insert("outcome_identity_hex".into(), Value::from("99".repeat(32)));
        let error = ProofStreamItem::from_json(&Value::Object(wrong_identity))
            .expect_err("PDP challenge/outcome identity mismatch must fail closed");
        assert!(error.contains("identity must equal"));

        for field in ["finalized_block_height", "committed_at_ms"] {
            let mut zero = canonical_pdp_item_map();
            zero.insert(field.into(), Value::from(0_u64));
            let error = ProofStreamItem::from_json(&Value::Object(zero))
                .expect_err("zero finalized provenance must fail closed");
            assert!(error.contains("complete committed-outcome provenance"));
        }
    }

    #[test]
    fn item_accepts_exact_chain_backed_signed_potr_projection() {
        let (map, receipt) = canonical_potr_item_map();
        let item = ProofStreamItem::from_json(&Value::Object(map))
            .expect("canonical committed PoTR projection");
        assert_eq!(item.proof_kind, ProofKind::Potr);
        assert_eq!(item.status, VerificationStatus::Success);
        assert_eq!(item.potr_receipt.as_ref(), Some(&receipt));
        assert_eq!(
            item.outcome_identity_hex.as_deref(),
            Some(
                hex::encode(
                    receipt
                        .request_scope_digest()
                        .expect("scope signed receipt")
                )
                .as_str()
            )
        );
        ProofStreamItem::from_json(&item.to_json()).expect("PoTR JSON roundtrip");
    }

    #[test]
    fn item_rejects_any_signed_potr_projection_contradiction() {
        let (canonical, receipt) = canonical_potr_item_map();
        let cases: [(&str, Value); 9] = [
            ("manifest_digest_hex", Value::from("91".repeat(32))),
            ("provider_id_hex", Value::from("92".repeat(32))),
            ("outcome_identity_hex", Value::from("93".repeat(32))),
            ("outcome_digest_hex", Value::from("94".repeat(32))),
            (
                "deadline_ms",
                Value::from(u64::from(receipt.deadline_ms) + 1),
            ),
            ("latency_ms", Value::from(u64::from(receipt.latency_ms) + 1)),
            ("tier", Value::from("archive")),
            ("trace_id", Value::from("95".repeat(16))),
            ("recorded_at_ms", Value::from(receipt.recorded_at_ms + 1)),
        ];
        for (field, value) in cases {
            let mut invalid = canonical.clone();
            invalid.insert(field.into(), value);
            let error = ProofStreamItem::from_json(&Value::Object(invalid))
                .expect_err("signed PoTR contradiction must fail closed");
            assert!(
                error.contains("PoTR"),
                "unexpected error for `{field}`: {error}"
            );
        }

        let mut wrong_status = canonical.clone();
        wrong_status.insert("result".into(), Value::from("failure"));
        wrong_status.insert("failure_reason".into(), Value::from("gateway_error"));
        let error = ProofStreamItem::from_json(&Value::Object(wrong_status))
            .expect_err("signed receipt status contradiction must fail closed");
        assert!(error.contains("result does not match"));

        let mut impossible_commit = canonical;
        impossible_commit.insert(
            "committed_at_ms".into(),
            Value::from(receipt.recorded_at_ms - 1),
        );
        let error = ProofStreamItem::from_json(&Value::Object(impossible_commit))
            .expect_err("receipt recorded after commit must fail closed");
        assert!(error.contains("after its committing block"));
    }

    #[test]
    fn item_rejects_missing_potr_provenance_and_noncanonical_receipt_base64() {
        let (canonical, _) = canonical_potr_item_map();
        for required in [
            "outcome_identity_hex",
            "outcome_digest_hex",
            "admission_envelope_digest_hex",
            "finalized_block_height",
            "finalized_block_hash_hex",
            "committed_at_ms",
            "receipt_b64",
        ] {
            let mut invalid = canonical.clone();
            invalid.remove(required);
            let error = ProofStreamItem::from_json(&Value::Object(invalid))
                .expect_err("incomplete PoTR provenance must fail closed");
            assert!(
                error.contains("PoTR"),
                "unexpected error after removing `{required}`: {error}"
            );
        }

        let encoded = canonical
            .get("receipt_b64")
            .and_then(Value::as_str)
            .expect("canonical base64")
            .to_owned();
        let mut unpadded = canonical;
        unpadded.insert(
            "receipt_b64".into(),
            Value::from(encoded.trim_end_matches('=')),
        );
        let error = ProofStreamItem::from_json(&Value::Object(unpadded))
            .expect_err("noncanonical receipt base64 must fail closed");
        assert!(error.contains("receipt_b64"));

        let mut noncanonical_norito = canonical_potr_item_map().0;
        let mut receipt_bytes = BASE64_STANDARD
            .decode(
                noncanonical_norito
                    .get("receipt_b64")
                    .and_then(Value::as_str)
                    .expect("canonical fixture receipt"),
            )
            .expect("decode canonical fixture receipt");
        receipt_bytes.push(0);
        noncanonical_norito.insert(
            "receipt_b64".into(),
            Value::from(BASE64_STANDARD.encode(receipt_bytes)),
        );
        let error = ProofStreamItem::from_json(&Value::Object(noncanonical_norito))
            .expect_err("noncanonical Norito receipt bytes must fail closed");
        assert!(
            error.contains("receipt") || error.contains("decode"),
            "unexpected noncanonical receipt error: {error}"
        );
    }

    #[test]
    fn metrics_collect_failure_breakdown() {
        let mut por = canonical_item_map();
        por.insert("latency_ms".into(), Value::from(10_u64));
        let por = ProofStreamItem::from_json(&Value::Object(por))
            .expect("canonical successful PoR metrics fixture");
        let pdp = ProofStreamItem::from_json(&Value::Object(canonical_pdp_item_map()))
            .expect("canonical failed PDP metrics fixture");

        let mut metrics = ProofStreamMetrics::default();
        metrics.record(&por);
        metrics.record(&pdp);

        let json = metrics.to_json();
        let obj = json.as_object().expect("metrics json");
        assert_eq!(
            obj.get("item_total").and_then(Value::as_u64),
            Some(2),
            "total items"
        );
        assert_eq!(
            obj.get("failure_total").and_then(Value::as_u64),
            Some(1),
            "failure items"
        );
        assert_eq!(
            obj.get("failure_by_reason")
                .and_then(Value::as_object)
                .and_then(|reasons| reasons.get("invalid_proof"))
                .and_then(Value::as_u64),
            Some(1),
            "canonical PDP failure reason"
        );
    }
}
