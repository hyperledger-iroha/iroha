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
    /// Item is pending verification.
    Pending,
}

impl VerificationStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Pending => "pending",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "success" => Ok(Self::Success),
            "failure" => Ok(Self::Failure),
            "pending" => Ok(Self::Pending),
            other => Err(format!(
                "unsupported proof result `{other}`; expected success, failure, or pending"
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
    pub manifest_digest_hex: String,
    /// Provider identifier (hex).
    pub provider_id_hex: String,
    /// Governed PDP challenge identifier (hex).
    pub challenge_id_hex: Option<String>,
    /// Proof kind.
    pub proof_kind: ProofKind,
    /// Verification status.
    pub status: VerificationStatus,
    /// Failure reason string (if provided).
    pub failure_reason: Option<String>,
    /// Reported latency in milliseconds.
    pub latency_ms: Option<u32>,
    /// Configured deadline in milliseconds (PoTR).
    pub deadline_ms: Option<u32>,
    /// Flat sample index (PoR).
    pub sample_index: Option<u32>,
    /// Chunk index (PoR).
    pub chunk_index: Option<u32>,
    /// Segment index (PoR).
    pub segment_index: Option<u32>,
    /// Leaf index within the segment (PoR).
    pub leaf_index: Option<u32>,
    /// Storage tier hint associated with the item.
    pub tier: Option<ProofTier>,
    /// Optional trace identifier.
    pub trace_id: Option<String>,
    /// Decoded PoR proof when supplied by the gateway.
    pub por_proof: Option<PorProof>,
    /// Canonical final signed PoTR receipt when supplied by the gateway.
    pub potr_receipt: Option<PotrReceiptV1>,
    /// Timestamp when the proof item was recorded (milliseconds since Unix epoch).
    pub recorded_at_ms: Option<u64>,
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
    Ok(receipt)
}

impl ProofStreamItem {
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
            (VerificationStatus::Success | VerificationStatus::Pending, true) => {
                return Err("non-failed proof stream item must omit `failure_reason`".to_string());
            }
            _ => {}
        }

        let latency_ms = optional_u32_field(obj, "latency_ms")?;
        let deadline_ms = optional_u32_field(obj, "deadline_ms")?;

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
                if challenge_id_hex.is_some() || deadline_ms.is_some() || potr_receipt.is_some() {
                    return Err(
                        "PoR item contains a PDP challenge, PoTR deadline, or signed receipt"
                            .to_string(),
                    );
                }
            }
            ProofKind::Pdp => {
                if challenge_id_hex.is_none() {
                    return Err("PDP item requires `challenge_id_hex`".to_string());
                }
                if deadline_ms.is_some() || por_proof.is_some() || potr_receipt.is_some() {
                    return Err("PDP item contains fields reserved for PoR or PoTR".to_string());
                }
            }
            ProofKind::Potr => {
                if challenge_id_hex.is_some() || por_proof.is_some() {
                    return Err("PoTR item contains fields reserved for PDP or PoR".to_string());
                }
                let receipt = potr_receipt
                    .as_ref()
                    .ok_or_else(|| "PoTR item requires final signed `receipt_b64`".to_string())?;
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
            challenge_id_hex,
            proof_kind,
            status,
            failure_reason,
            latency_ms,
            deadline_ms,
            sample_index: optional_u32_field(obj, "leaf_index_flat")?,
            chunk_index: optional_u32_field(obj, "chunk_index")?,
            segment_index: optional_u32_field(obj, "segment_index")?,
            leaf_index: optional_u32_field(obj, "leaf_index")?,
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

    fn canonical_item_map() -> Map {
        let mut map = Map::new();
        map.insert("manifest_digest_hex".into(), Value::from("aa".repeat(32)));
        map.insert("provider_id_hex".into(), Value::from("bb".repeat(32)));
        map.insert("proof_kind".into(), Value::from("por"));
        map.insert("result".into(), Value::from("success"));
        map
    }

    #[test]
    fn item_parses_from_ndjson() {
        let digest_01 = "0101010101010101010101010101010101010101010101010101010101010101";
        let digest_02 = "0202020202020202020202020202020202020202020202020202020202020202";
        let digest_03 = "0303030303030303030303030303030303030303030303030303030303030303";
        let digest_04 = "0404040404040404040404040404040404040404040404040404040404040404";
        let proof = norito::json!({
            "payload_len": 1024,
            "chunk_index": 0,
            "chunk_offset": 0,
            "chunk_length": 1024,
            "chunk_digest_hex": digest_01,
            "chunk_root_hex": digest_02,
            "segment_index": 0,
            "segment_offset": 0,
            "segment_length": 1024,
            "segment_digest_hex": digest_03,
            "leaf_index": 0,
            "leaf_offset": 0,
            "leaf_length": 1024,
            "leaf_bytes_hex": "",
            "leaf_digest_hex": digest_04,
            "segment_leaves_hex": [digest_04],
            "chunk_segments_hex": [digest_03],
            "chunk_roots_hex": [digest_02],
        });
        let manifest_digest_hex = "aa".repeat(32);
        let provider_id_hex = "bb".repeat(32);
        let map = norito::json!({
            "manifest_digest_hex": (manifest_digest_hex.clone()),
            "provider_id_hex": (provider_id_hex.clone()),
            "proof_kind": "por",
            "result": "success",
            "latency_ms": 42,
            "recorded_at_ms": 1_700_000_000_000u64,
            "leaf_index_flat": 1,
            "chunk_index": 0,
            "segment_index": 0,
            "leaf_index": 0,
            "proof": proof,
        });
        let line = norito::json::to_string(&map).expect("serialize map");
        let item = ProofStreamItem::from_ndjson(line.as_bytes()).expect("parse item");
        assert_eq!(item.manifest_digest_hex, manifest_digest_hex);
        assert_eq!(item.provider_id_hex, provider_id_hex);
        assert_eq!(item.sample_index, Some(1));
        assert!(matches!(item.status, VerificationStatus::Success));
        assert!(item.por_proof.is_some());
        assert_eq!(item.deadline_ms, None);
        assert_eq!(item.recorded_at_ms, Some(1_700_000_000_000));
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
        for field in [
            "latency_ms",
            "leaf_index_flat",
            "chunk_index",
            "segment_index",
            "leaf_index",
        ] {
            let mut map = canonical_item_map();
            map.insert(field.into(), Value::from(u32::MAX));

            let item = ProofStreamItem::from_json(&Value::Object(map))
                .expect("u32::MAX must remain representable");
            let parsed = match field {
                "latency_ms" => item.latency_ms,
                "leaf_index_flat" => item.sample_index,
                "chunk_index" => item.chunk_index,
                "segment_index" => item.segment_index,
                "leaf_index" => item.leaf_index,
                _ => unreachable!("field list is exhaustive"),
            };
            assert_eq!(parsed, Some(u32::MAX), "wrong value for {field}");
        }
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

        for result in ["success", "pending"] {
            let mut map = canonical_item_map();
            map.insert("result".into(), Value::from(result));
            map.insert("failure_reason".into(), Value::from("provider_error"));
            let error = ProofStreamItem::from_json(&Value::Object(map))
                .expect_err("non-failed item must not carry a failure reason");
            assert!(error.contains("must omit `failure_reason`"));
        }
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

        for invalid_result in ["ok", "passed", "SUCCESS", " success"] {
            let mut map = canonical_item_map();
            map.insert("result".into(), Value::from(invalid_result));
            let error = ProofStreamItem::from_json(&Value::Object(map))
                .expect_err("noncanonical result must fail closed");
            assert!(error.contains("unsupported proof result"));
        }
    }

    #[test]
    fn metrics_collect_failure_breakdown() {
        let mut metrics = ProofStreamMetrics::default();
        metrics.record(&ProofStreamItem {
            manifest_digest_hex: "aa".repeat(32),
            provider_id_hex: "bb".repeat(32),
            challenge_id_hex: None,
            proof_kind: ProofKind::Por,
            status: VerificationStatus::Success,
            failure_reason: None,
            latency_ms: Some(10),
            deadline_ms: None,
            sample_index: None,
            chunk_index: None,
            segment_index: None,
            leaf_index: None,
            tier: None,
            trace_id: None,
            por_proof: None,
            potr_receipt: None,
            recorded_at_ms: None,
        });
        metrics.record(&ProofStreamItem {
            manifest_digest_hex: "aa".repeat(32),
            provider_id_hex: "bb".repeat(32),
            challenge_id_hex: None,
            proof_kind: ProofKind::Por,
            status: VerificationStatus::Failure,
            failure_reason: Some("timeout".into()),
            latency_ms: Some(50),
            deadline_ms: None,
            sample_index: None,
            chunk_index: None,
            segment_index: None,
            leaf_index: None,
            tier: None,
            trace_id: None,
            por_proof: None,
            potr_receipt: None,
            recorded_at_ms: None,
        });

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
    }
}
