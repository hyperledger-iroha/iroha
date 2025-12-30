//! Shared Norito schemas for streaming proof requests.

use norito::derive::{JsonSerialize, NoritoDeserialize, NoritoSerialize};
#[allow(unused_imports)]
use norito::json::JsonSerialize as NoritoJsonSerialize;
use thiserror::Error;

/// Streaming proof request envelope (PoR / PDP / PoTR).
#[derive(
    Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq, Hash,
)]
pub struct ProofStreamRequestV1 {
    /// Canonical manifest digest (BLAKE3-256).
    pub manifest_digest: [u8; 32],
    /// Provider identifier authorised to serve proofs.
    pub provider_id: [u8; 32],
    /// Proof flavour requested by the client.
    pub proof_kind: ProofStreamKind,
    /// Requested sample count (required for PoR/PDP).
    pub sample_count: Option<u32>,
    /// Deadline in milliseconds (required for PoTR).
    pub deadline_ms: Option<u32>,
    /// Optional deterministic seed for sample selection.
    pub sample_seed: Option<u64>,
    /// Client-supplied nonce to guard against replay.
    pub nonce: [u8; 16],
    /// Orchestrator job identifier (UUID bytes).
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
        match self.proof_kind {
            ProofStreamKind::Por | ProofStreamKind::Pdp => {
                let count = self
                    .sample_count
                    .ok_or(ProofStreamRequestError::MissingSampleCount)?;
                if count == 0 {
                    return Err(ProofStreamRequestError::ZeroSampleCount);
                }
            }
            ProofStreamKind::Potr => {
                let deadline = self
                    .deadline_ms
                    .ok_or(ProofStreamRequestError::MissingDeadlineMs)?;
                if deadline == 0 {
                    return Err(ProofStreamRequestError::ZeroDeadlineMs);
                }
            }
        }
        Ok(())
    }
}

/// Supported proof kinds for streaming.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq, Hash)]
pub enum ProofStreamKind {
    /// Proof-of-Retrievability samples.
    Por,
    /// Proofs of Data Possession (future work).
    Pdp,
    /// Proof-of-Timed Retrieval receipts (future work).
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

impl norito::json::JsonSerialize for ProofStreamKind {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            ProofStreamKind::Por => "por",
            ProofStreamKind::Pdp => "pdp",
            ProofStreamKind::Potr => "potr",
        };
        <&str as norito::json::JsonSerialize>::json_serialize(&label, out);
    }
}

impl norito::json::JsonSerialize for ProofStreamTier {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            ProofStreamTier::Hot => "hot",
            ProofStreamTier::Warm => "warm",
            ProofStreamTier::Archive => "archive",
        };
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
    #[error("PoR/PDP requests require a sample count")]
    MissingSampleCount,
    #[error("PoTR requests require a deadline")]
    MissingDeadlineMs,
    #[error("sample count must be greater than zero")]
    ZeroSampleCount,
    #[error("deadline must be greater than zero milliseconds")]
    ZeroDeadlineMs,
}

#[cfg(test)]
mod tests {
    use norito::json::{Value, to_value};

    use super::*;

    fn base_request() -> ProofStreamRequestV1 {
        ProofStreamRequestV1 {
            manifest_digest: [0x11; 32],
            provider_id: [0x22; 32],
            proof_kind: ProofStreamKind::Por,
            sample_count: Some(16),
            deadline_ms: None,
            sample_seed: Some(42),
            nonce: [0x33; 16],
            orchestrator_job_id: Some([0x44; 16]),
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
    fn potr_requires_deadline() {
        let mut request = base_request();
        request.proof_kind = ProofStreamKind::Potr;
        request.sample_count = None;
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
    fn zero_nonce_rejected() {
        let mut request = base_request();
        request.nonce = [0u8; 16];
        assert_eq!(
            request.validate(),
            Err(ProofStreamRequestError::InvalidNonce)
        );
    }

    #[test]
    fn json_serialization_uses_lowercase_labels() {
        let request = base_request();
        let value = to_value(&request).expect("serialize to value");
        let obj = value
            .as_object()
            .expect("proof stream request should serialize to JSON object");
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
    }
}
