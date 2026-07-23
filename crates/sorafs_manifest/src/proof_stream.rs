//! Shared Norito schemas for streaming proof requests.

use norito::derive::{JsonSerialize, NoritoDeserialize, NoritoSerialize};
#[allow(unused_imports)]
use norito::json::JsonSerialize as NoritoJsonSerialize;
use thiserror::Error;

/// Maximum `sample_count` accepted for PoR/PDP proof-stream requests.
pub const MAX_PROOF_STREAM_SAMPLE_COUNT: u32 = 500;

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
            ProofStreamKind::Por => {
                if self.challenge_id.is_some() {
                    return Err(ProofStreamRequestError::UnexpectedChallengeId);
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
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq, Hash)]
pub enum ProofStreamKind {
    /// Proof-of-Retrievability samples.
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

#[cfg(test)]
mod tests {
    use norito::json::{Value, to_value};

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
