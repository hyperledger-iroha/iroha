//! Proof-of-Timed Retrieval (PoTR) receipt schemas.

use iroha_crypto::{Algorithm, PublicKey, Signature};
use norito::derive::{JsonSerialize, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

use crate::proof_stream::ProofStreamTier;

/// Current PoTR receipt schema version.
pub const POTR_RECEIPT_VERSION_V1: u8 = 1;

/// Receipt emitted after completing a timed retrieval probe.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PotrReceiptV1 {
    /// Schema version (`POTR_RECEIPT_VERSION_V1`).
    pub version: u8,
    /// Manifest digest (BLAKE3-256) associated with the retrieval.
    pub manifest_digest: [u8; 32],
    /// Provider identifier authorised for the manifest.
    pub provider_id: [u8; 32],
    /// Storage tier that serviced the retrieval.
    pub tier: ProofStreamTier,
    /// Deadline enforced by the probe (milliseconds).
    pub deadline_ms: u32,
    /// Observed latency for the retrieval (milliseconds).
    pub latency_ms: u32,
    /// Result of the timed retrieval.
    pub status: PotrStatus,
    /// Unix timestamp (milliseconds) when the request was issued.
    pub requested_at_ms: u64,
    /// Unix timestamp (milliseconds) when the response completed.
    pub responded_at_ms: u64,
    /// Unix timestamp (milliseconds) when the receipt was recorded.
    pub recorded_at_ms: u64,
    /// Inclusive byte offset where the ranged fetch started.
    pub range_start: u64,
    /// Inclusive byte offset where the ranged fetch ended.
    pub range_end: u64,
    /// Optional request identifier supplied by the orchestrator/gateway.
    pub request_id: Option<[u8; 16]>,
    /// Optional trace identifier associated with the probe.
    pub trace_id: Option<[u8; 16]>,
    /// Optional human-readable note describing failures.
    pub note: Option<String>,
    /// Gateway attestation over the receipt payload.
    pub gateway_signature: Option<PotrSignatureV1>,
    /// Provider attestation over the receipt payload.
    pub provider_signature: Option<PotrSignatureV1>,
}

/// Detached signature covering a PoTR receipt.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PotrSignatureV1 {
    /// Algorithm identifier for the signature.
    pub algorithm: PotrSignatureAlgorithm,
    /// Public key advertised by the signer.
    pub public_key: Vec<u8>,
    /// Raw signature bytes.
    pub signature: Vec<u8>,
}

impl PotrSignatureV1 {
    /// Validate signature lengths for the advertised algorithm.
    pub fn validate(&self, context: &'static str) -> Result<(), PotrReceiptValidationError> {
        match self.algorithm {
            PotrSignatureAlgorithm::Ed25519 => {
                if self.public_key.len() != 32 || self.signature.len() != 64 {
                    return Err(PotrReceiptValidationError::InvalidSignature {
                        context,
                        reason: "ed25519 signatures require 32-byte public keys and 64-byte signatures",
                    });
                }
            }
            PotrSignatureAlgorithm::Dilithium3 => {
                if self.public_key.is_empty() || self.signature.is_empty() {
                    return Err(PotrReceiptValidationError::InvalidSignature {
                        context,
                        reason: "dilithium3 signatures must include non-empty public key and signature buffers",
                    });
                }
            }
        }
        Ok(())
    }

    /// Verify the signature against the provided receipt payload.
    pub fn verify(
        &self,
        context: &'static str,
        payload: &[u8],
    ) -> Result<(), PotrReceiptValidationError> {
        self.validate(context)?;
        let algorithm = match self.algorithm {
            PotrSignatureAlgorithm::Ed25519 => Algorithm::Ed25519,
            PotrSignatureAlgorithm::Dilithium3 => Algorithm::MlDsa,
        };
        let public_key = PublicKey::from_bytes(algorithm, &self.public_key).map_err(|_| {
            PotrReceiptValidationError::InvalidSignature {
                context,
                reason: "invalid public key",
            }
        })?;
        let signature = Signature::from_bytes(&self.signature);
        signature.verify(&public_key, payload).map_err(|_| {
            PotrReceiptValidationError::InvalidSignature {
                context,
                reason: "signature verification failed",
            }
        })?;
        Ok(())
    }
}

/// Supported signature algorithms for PoTR receipts.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[norito(tag = "algorithm")]
#[repr(u8)]
pub enum PotrSignatureAlgorithm {
    /// Ed25519 attestation (current default).
    #[norito(rename = "ed25519")]
    Ed25519 = 1,
    /// ML-DSA (Dilithium3) attestation.
    #[norito(rename = "dilithium3")]
    Dilithium3 = 2,
}

impl norito::json::JsonSerialize for PotrSignatureAlgorithm {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            PotrSignatureAlgorithm::Ed25519 => "ed25519",
            PotrSignatureAlgorithm::Dilithium3 => "dilithium3",
        };
        norito::json::write_json_string(label, out);
    }
}

impl PotrReceiptV1 {
    fn signing_payload_bytes(&self) -> Result<Vec<u8>, PotrReceiptValidationError> {
        let mut unsigned = self.clone();
        unsigned.gateway_signature = None;
        unsigned.provider_signature = None;
        norito::to_bytes(&unsigned).map_err(|_| PotrReceiptValidationError::InvalidSignature {
            context: "receipt",
            reason: "failed to encode receipt for signature verification",
        })
    }

    /// Validates invariants required for a PoTR receipt.
    pub fn validate(&self) -> Result<(), PotrReceiptValidationError> {
        if self.version != POTR_RECEIPT_VERSION_V1 {
            return Err(PotrReceiptValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.manifest_digest.iter().all(|&byte| byte == 0) {
            return Err(PotrReceiptValidationError::InvalidManifestDigest);
        }
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(PotrReceiptValidationError::InvalidProviderId);
        }
        if self.deadline_ms == 0 {
            return Err(PotrReceiptValidationError::ZeroDeadline);
        }
        if self.range_start > self.range_end {
            return Err(PotrReceiptValidationError::RangeStartExceedsEnd {
                start: self.range_start,
                end: self.range_end,
            });
        }
        if self.responded_at_ms < self.requested_at_ms {
            return Err(PotrReceiptValidationError::ResponseBeforeRequest {
                requested_at_ms: self.requested_at_ms,
                responded_at_ms: self.responded_at_ms,
            });
        }
        if self.recorded_at_ms < self.responded_at_ms {
            return Err(PotrReceiptValidationError::RecordedBeforeResponse {
                recorded_at_ms: self.recorded_at_ms,
                responded_at_ms: self.responded_at_ms,
            });
        }
        if matches!(self.status, PotrStatus::Success) && self.latency_ms > self.deadline_ms {
            return Err(PotrReceiptValidationError::LatencyExceedsDeadline {
                latency_ms: self.latency_ms,
                deadline_ms: self.deadline_ms,
            });
        }
        let observed_latency = self.responded_at_ms.saturating_sub(self.requested_at_ms);
        if observed_latency <= u64::from(u32::MAX) {
            let observed_ms = observed_latency as u32;
            let latency_delta = i128::from(self.latency_ms) - i128::from(observed_ms);
            if latency_delta.abs() > 5 {
                return Err(PotrReceiptValidationError::LatencyMismatch {
                    latency_ms: self.latency_ms,
                    observed_ms: u64::from(observed_ms),
                });
            }
        }
        if self.gateway_signature.is_some() || self.provider_signature.is_some() {
            let payload = self.signing_payload_bytes()?;
            if let Some(signature) = self.gateway_signature.as_ref() {
                signature.verify("gateway", &payload)?;
            }
            if let Some(signature) = self.provider_signature.as_ref() {
                signature.verify("provider", &payload)?;
            }
        }
        Ok(())
    }
}

/// Outcome classification for PoTR receipts.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq, Hash)]
pub enum PotrStatus {
    /// Retrieval completed within the configured deadline.
    Success,
    /// Retrieval exceeded the configured deadline.
    MissedDeadline,
    /// Provider returned an explicit error before completing the fetch.
    ProviderError,
    /// Gateway failed to complete the request.
    GatewayError,
    /// Client aborted the request before completion.
    ClientCancelled,
}

impl norito::json::JsonSerialize for PotrStatus {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            PotrStatus::Success => "success",
            PotrStatus::MissedDeadline => "missed_deadline",
            PotrStatus::ProviderError => "provider_error",
            PotrStatus::GatewayError => "gateway_error",
            PotrStatus::ClientCancelled => "client_cancelled",
        };
        norito::json::write_json_string(label, out);
    }
}

/// Validation errors for [`PotrReceiptV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PotrReceiptValidationError {
    #[error("unsupported PoTR receipt version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    #[error("deadline must be greater than zero milliseconds")]
    ZeroDeadline,
    #[error("range start {start} exceeds range end {end}")]
    RangeStartExceedsEnd { start: u64, end: u64 },
    #[error("response timestamp {responded_at_ms}ms precedes request {requested_at_ms}ms")]
    ResponseBeforeRequest {
        requested_at_ms: u64,
        responded_at_ms: u64,
    },
    #[error("recorded timestamp {recorded_at_ms}ms precedes response {responded_at_ms}ms")]
    RecordedBeforeResponse {
        recorded_at_ms: u64,
        responded_at_ms: u64,
    },
    #[error("latency {latency_ms}ms exceeds success deadline {deadline_ms}ms")]
    LatencyExceedsDeadline { latency_ms: u32, deadline_ms: u32 },
    #[error("reported latency {latency_ms}ms diverges from observed {observed_ms}ms")]
    LatencyMismatch { latency_ms: u32, observed_ms: u64 },
    #[error("{context} signature invalid: {reason}")]
    InvalidSignature {
        context: &'static str,
        reason: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;
    use crate::proof_stream::ProofStreamTier;

    fn base_receipt() -> PotrReceiptV1 {
        PotrReceiptV1 {
            version: POTR_RECEIPT_VERSION_V1,
            manifest_digest: [0x11; 32],
            provider_id: [0x22; 32],
            tier: ProofStreamTier::Hot,
            deadline_ms: 90_000,
            latency_ms: 42_000,
            status: PotrStatus::Success,
            requested_at_ms: 1_700_000_000_000,
            responded_at_ms: 1_700_000_042_000,
            recorded_at_ms: 1_700_000_042_100,
            range_start: 0,
            range_end: 1_048_575,
            request_id: Some([0x44; 16]),
            trace_id: Some([0x33; 16]),
            note: Some("ok".to_string()),
            gateway_signature: None,
            provider_signature: None,
        }
    }

    fn sign_receipt(receipt: &PotrReceiptV1, signing_key: &SigningKey) -> PotrSignatureV1 {
        let payload = receipt.signing_payload_bytes().expect("payload bytes");
        let signature = signing_key.sign(&payload);
        PotrSignatureV1 {
            algorithm: PotrSignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signature.to_bytes().to_vec(),
        }
    }

    #[test]
    fn receipt_validates() {
        let receipt = base_receipt();
        assert_eq!(receipt.validate(), Ok(()));
    }

    #[test]
    fn zero_deadline_rejected() {
        let mut receipt = base_receipt();
        receipt.deadline_ms = 0;
        assert_eq!(
            receipt.validate(),
            Err(PotrReceiptValidationError::ZeroDeadline)
        );
    }

    #[test]
    fn latency_beyond_deadline_rejected_for_success() {
        let mut receipt = base_receipt();
        receipt.latency_ms = receipt.deadline_ms + 1;
        receipt.responded_at_ms = receipt
            .requested_at_ms
            .saturating_add(u64::from(receipt.latency_ms));
        receipt.recorded_at_ms = receipt.responded_at_ms + 100;
        assert_eq!(
            receipt.validate(),
            Err(PotrReceiptValidationError::LatencyExceedsDeadline {
                latency_ms: receipt.latency_ms,
                deadline_ms: receipt.deadline_ms,
            })
        );

        receipt.status = PotrStatus::MissedDeadline;
        assert_eq!(receipt.validate(), Ok(()));
    }

    #[test]
    fn range_start_beyond_end_rejected() {
        let mut receipt = base_receipt();
        receipt.range_start = 100;
        receipt.range_end = 50;
        assert_eq!(
            receipt.validate(),
            Err(PotrReceiptValidationError::RangeStartExceedsEnd {
                start: 100,
                end: 50
            })
        );
    }

    #[test]
    fn response_before_request_rejected() {
        let mut receipt = base_receipt();
        receipt.responded_at_ms = receipt.requested_at_ms - 1;
        assert_eq!(
            receipt.validate(),
            Err(PotrReceiptValidationError::ResponseBeforeRequest {
                requested_at_ms: receipt.requested_at_ms,
                responded_at_ms: receipt.responded_at_ms,
            })
        );
    }

    #[test]
    fn latency_mismatch_rejected() {
        let mut receipt = base_receipt();
        receipt.latency_ms = receipt.latency_ms.saturating_add(10);
        assert_eq!(
            receipt.validate(),
            Err(PotrReceiptValidationError::LatencyMismatch {
                latency_ms: 42_010,
                observed_ms: 42_000,
            })
        );
    }

    #[test]
    fn gateway_signature_verifies() {
        let mut receipt = base_receipt();
        let signing_key = SigningKey::from_bytes(&[0x11; 32]);
        receipt.gateway_signature = Some(sign_receipt(&receipt, &signing_key));
        assert_eq!(receipt.validate(), Ok(()));
    }

    #[test]
    fn gateway_signature_rejects_invalid_signature() {
        let mut receipt = base_receipt();
        let signing_key = SigningKey::from_bytes(&[0x22; 32]);
        let mut signature = sign_receipt(&receipt, &signing_key);
        signature.signature[0] ^= 0x01;
        receipt.gateway_signature = Some(signature);
        assert!(matches!(
            receipt.validate(),
            Err(PotrReceiptValidationError::InvalidSignature {
                context: "gateway",
                ..
            })
        ));
    }

    #[test]
    fn gateway_signature_rejects_invalid_length() {
        let mut receipt = base_receipt();
        receipt.gateway_signature = Some(PotrSignatureV1 {
            algorithm: PotrSignatureAlgorithm::Ed25519,
            public_key: vec![0u8; 16],
            signature: vec![0u8; 32],
        });
        assert!(matches!(
            receipt.validate(),
            Err(PotrReceiptValidationError::InvalidSignature {
                context: "gateway",
                ..
            })
        ));
    }

    #[test]
    fn manifest_and_provider_must_be_non_zero() {
        let mut receipt = base_receipt();
        receipt.manifest_digest = [0u8; 32];
        assert_eq!(
            receipt.validate(),
            Err(PotrReceiptValidationError::InvalidManifestDigest)
        );
        receipt.manifest_digest = [0x11; 32];
        receipt.provider_id = [0u8; 32];
        assert_eq!(
            receipt.validate(),
            Err(PotrReceiptValidationError::InvalidProviderId)
        );
    }
}
