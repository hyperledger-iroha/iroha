//! Shared helpers for SoraFS proof streaming clients.
//!
//! This module provides request/response representations that match the Torii
//! `/v2/sorafs/proof/stream` endpoint together with lightweight aggregation
//!\//! utilities used by the CLI and SDK integrations.

use std::collections::BTreeMap;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use norito::json::{Map, Value, from_slice, to_vec};

use crate::{PorProof, por_json::proof_from_value};

/// Proof flavour emitted by the gateway.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ProofKind {
    /// Proof-of-Retrievability samples.
    #[default]
    Por,
    /// Proofs derived from commitment-based PDP checks.
    Pdp,
    /// Deadline proofs (PoTR-Lite).
    Potr,
}

impl ProofKind {
    /// Canonical lowercase label used on the wire.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Por => "por",
            Self::Pdp => "pdp",
            Self::Potr => "potr",
        }
    }

    /// Parses a wire label into a `ProofKind`.
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "por" => Ok(Self::Por),
            "pdp" => Ok(Self::Pdp),
            "potr" => Ok(Self::Potr),
            other => Err(format!("unsupported proof kind `{other}`")),
        }
    }
}

/// Tier hint associated with the streaming request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProofTier {
    /// Hot tier.
    Hot,
    /// Warm tier.
    Warm,
    /// Archive tier.
    Archive,
}

impl ProofTier {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hot => "hot",
            Self::Warm => "warm",
            Self::Archive => "archive",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "hot" => Ok(Self::Hot),
            "warm" => Ok(Self::Warm),
            "archive" => Ok(Self::Archive),
            other => Err(format!("unsupported proof tier `{other}`")),
        }
    }
}

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

    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "success" | "ok" | "passed" => Self::Success,
            "pending" => Self::Pending,
            // Treat unknown variants as failures so tooling errs on the safe side.
            _ => Self::Failure,
        }
    }

    #[must_use]
    pub fn is_failure(self) -> bool {
        matches!(self, Self::Failure)
    }
}

/// Request payload for `/v2/sorafs/proof/stream`.
#[derive(Clone, Debug)]
pub struct ProofStreamRequest {
    /// Manifest digest (BLAKE3-256) encoded as lowercase hex.
    pub manifest_digest_hex: String,
    /// Provider identifier encoded as lowercase hex (optional).
    pub provider_id_hex: Option<String>,
    /// Proof kind to request.
    pub proof_kind: ProofKind,
    /// Optional number of samples to request (PoR/PDP).
    pub sample_count: Option<u32>,
    /// Optional deterministic seed for sampling.
    pub sample_seed: Option<u64>,
    /// Optional deadline (milliseconds) for PoTR.
    pub deadline_ms: Option<u32>,
    /// Tier hint.
    pub tier: Option<ProofTier>,
    /// Caller-supplied nonce (16 bytes).
    pub nonce: [u8; 16],
    /// Optional orchestrator job identifier encoded as lowercase hex.
    pub orchestrator_job_id_hex: Option<String>,
}

impl ProofStreamRequest {
    /// Serialises the request into a Norito JSON value.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert(
            "manifest_digest_hex".into(),
            Value::from(self.manifest_digest_hex.clone()),
        );
        if let Some(provider) = &self.provider_id_hex {
            map.insert("provider_id_hex".into(), Value::from(provider.clone()));
        }
        map.insert(
            "proof_kind".into(),
            Value::from(self.proof_kind.as_str().to_string()),
        );
        if let Some(count) = self.sample_count {
            map.insert("sample_count".into(), Value::from(count));
        }
        if let Some(seed) = self.sample_seed {
            map.insert("sample_seed".into(), Value::from(seed));
        }
        if let Some(deadline) = self.deadline_ms {
            map.insert("deadline_ms".into(), Value::from(deadline));
        }
        if let Some(tier) = self.tier {
            map.insert("tier".into(), Value::from(tier.as_str().to_string()));
        }
        let nonce_b64 = BASE64_STANDARD.encode(self.nonce);
        map.insert("nonce_b64".into(), Value::from(nonce_b64));
        if let Some(job_id) = &self.orchestrator_job_id_hex {
            map.insert(
                "orchestrator_job_id_hex".into(),
                Value::from(job_id.clone()),
            );
        }
        Value::Object(map)
    }

    /// Serialises the request into bytes suitable for HTTP transport.
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, String> {
        to_vec(&self.to_json()).map_err(|err| format!("failed to encode request: {err}"))
    }
}

/// Streaming item reported by the gateway.
#[derive(Clone, Debug)]
pub struct ProofStreamItem {
    /// Manifest digest (hex).
    pub manifest_digest_hex: Option<String>,
    /// Provider identifier (hex).
    pub provider_id_hex: Option<String>,
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
    /// Timestamp when the proof item was recorded (milliseconds since Unix epoch).
    pub recorded_at_ms: Option<u64>,
}

impl ProofStreamItem {
    /// Parses an item from a Norito JSON value.
    pub fn from_json(value: &Value) -> Result<Self, String> {
        let obj = value
            .as_object()
            .ok_or_else(|| "proof stream item must be a JSON object".to_string())?;

        let proof_kind = match obj.get("proof_kind") {
            Some(Value::String(kind)) => ProofKind::parse(kind)?,
            Some(_) => return Err("`proof_kind` must be a string".to_string()),
            None => ProofKind::Por,
        };

        let status = obj
            .get("result")
            .or_else(|| obj.get("verification_status"))
            .and_then(Value::as_str)
            .map(VerificationStatus::parse)
            .ok_or_else(|| "proof stream item missing `result` field".to_string())?;

        let failure_reason = obj
            .get("failure_reason")
            .and_then(Value::as_str)
            .map(|reason| reason.trim().to_string())
            .filter(|reason| !reason.is_empty());

        let latency_ms = obj
            .get("latency_ms")
            .or_else(|| obj.get("latency"))
            .and_then(Value::as_u64)
            .map(|value| value as u32);
        let deadline_ms = obj
            .get("deadline_ms")
            .and_then(Value::as_u64)
            .map(|value| value as u32);

        let tier = obj
            .get("tier")
            .and_then(Value::as_str)
            .map(ProofTier::parse)
            .transpose()?;

        let por_proof = obj
            .get("proof")
            .map(proof_from_value)
            .transpose()
            .map_err(|err| format!("failed to decode proof payload: {err}"))?;

        Ok(Self {
            manifest_digest_hex: obj
                .get("manifest_digest_hex")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            provider_id_hex: obj
                .get("provider_id_hex")
                .or_else(|| obj.get("provider_id"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            proof_kind,
            status,
            failure_reason,
            latency_ms,
            deadline_ms,
            sample_index: obj
                .get("leaf_index_flat")
                .or_else(|| obj.get("sample_index"))
                .and_then(Value::as_u64)
                .map(|value| value as u32),
            chunk_index: obj
                .get("chunk_index")
                .and_then(Value::as_u64)
                .map(|value| value as u32),
            segment_index: obj
                .get("segment_index")
                .and_then(Value::as_u64)
                .map(|value| value as u32),
            leaf_index: obj
                .get("leaf_index")
                .and_then(Value::as_u64)
                .map(|value| value as u32),
            tier,
            trace_id: obj
                .get("trace_id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            por_proof,
            recorded_at_ms: obj.get("recorded_at_ms").and_then(Value::as_u64),
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
        if let Some(digest) = &self.manifest_digest_hex {
            map.insert("manifest_digest_hex".into(), Value::from(digest.clone()));
        }
        if let Some(provider) = &self.provider_id_hex {
            map.insert("provider_id_hex".into(), Value::from(provider.clone()));
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

    #[test]
    fn request_serialises_to_expected_shape() {
        let request = ProofStreamRequest {
            manifest_digest_hex: "deadbeef".into(),
            provider_id_hex: Some("abcd".into()),
            proof_kind: ProofKind::Por,
            sample_count: Some(8),
            sample_seed: Some(42),
            deadline_ms: Some(90_000),
            tier: Some(ProofTier::Hot),
            nonce: [0x11; 16],
            orchestrator_job_id_hex: Some("ff00".into()),
        };
        let value = request.to_json();
        let obj = value.as_object().expect("json object");
        assert_eq!(
            obj.get("manifest_digest_hex").and_then(Value::as_str),
            Some("deadbeef")
        );
        assert_eq!(
            obj.get("provider_id_hex").and_then(Value::as_str),
            Some("abcd")
        );
        assert_eq!(obj.get("proof_kind").and_then(Value::as_str), Some("por"));
        assert_eq!(obj.get("sample_count").and_then(Value::as_u64), Some(8));
        assert_eq!(obj.get("sample_seed").and_then(Value::as_u64), Some(42));
        assert_eq!(obj.get("deadline_ms").and_then(Value::as_u64), Some(90_000));
        assert_eq!(obj.get("tier").and_then(Value::as_str), Some("hot"));
        assert!(obj.get("nonce_b64").and_then(Value::as_str).is_some());
        assert_eq!(
            obj.get("orchestrator_job_id_hex").and_then(Value::as_str),
            Some("ff00")
        );
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
        let map = norito::json!({
            "manifest_digest_hex": "aa",
            "provider_id_hex": "bb",
            "proof_kind": "por",
            "result": "success",
            "latency_ms": 42,
            "deadline_ms": 90_000,
            "recorded_at_ms": 1_700_000_000_000u64,
            "leaf_index_flat": 1,
            "chunk_index": 0,
            "segment_index": 0,
            "leaf_index": 0,
            "proof": proof,
        });
        let line = norito::json::to_string(&map).expect("serialize map");
        let item = ProofStreamItem::from_ndjson(line.as_bytes()).expect("parse item");
        assert_eq!(item.manifest_digest_hex.as_deref(), Some("aa"));
        assert_eq!(item.provider_id_hex.as_deref(), Some("bb"));
        assert_eq!(item.sample_index, Some(1));
        assert!(matches!(item.status, VerificationStatus::Success));
        assert!(item.por_proof.is_some());
        assert_eq!(item.deadline_ms, Some(90_000));
        assert_eq!(item.recorded_at_ms, Some(1_700_000_000_000));
    }

    #[test]
    fn metrics_collect_failure_breakdown() {
        let mut metrics = ProofStreamMetrics::default();
        metrics.record(&ProofStreamItem {
            manifest_digest_hex: None,
            provider_id_hex: None,
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
            recorded_at_ms: None,
        });
        metrics.record(&ProofStreamItem {
            manifest_digest_hex: None,
            provider_id_hex: None,
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
