//! Guard directory snapshot helpers.

#![allow(unexpected_cfgs)]

use std::convert::TryFrom;

use blake3::Hasher as Blake3Hasher;
use norito::{NoritoDeserialize, NoritoSerialize, decode_from_bytes, to_bytes};

use crate::soranet::certificate::CertificateValidationPhase;

const SRC_V2_ISSUER_FINGERPRINT_DOMAIN: &[u8] = b"soranet.src.v2.issuer";

/// Schema version used by `GuardDirectorySnapshotV2`.
pub const GUARD_DIRECTORY_VERSION_V2: u8 = 2;

/// Norito-encoded guard directory snapshot.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct GuardDirectorySnapshotV2 {
    /// Snapshot schema version (`2`).
    pub version: u8,
    /// Consensus directory hash bound by certificates.
    pub directory_hash: [u8; 32],
    /// Publication timestamp (Unix seconds).
    pub published_at_unix: i64,
    /// Valid-after timestamp (Unix seconds).
    pub valid_after_unix: i64,
    /// Valid-until timestamp (Unix seconds).
    pub valid_until_unix: i64,
    /// Validation phase gate encoded as `u8`.
    pub validation_phase: u8,
    /// Governance issuer records.
    #[norito(default)]
    pub issuers: Vec<GuardDirectoryIssuerV1>,
    /// Relay certificate bundles.
    pub relays: Vec<GuardDirectoryRelayEntryV2>,
}

impl GuardDirectorySnapshotV2 {
    /// Encode the snapshot to Norito bytes.
    ///
    /// # Errors
    /// Returns an error if serialization fails.
    pub fn to_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        to_bytes(self)
    }

    /// Decode a snapshot from Norito bytes.
    ///
    /// # Errors
    /// Returns an error if decoding fails.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, norito::Error> {
        decode_from_bytes(bytes)
    }
}

/// Governance issuer record embedded in guard directory snapshots.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct GuardDirectoryIssuerV1 {
    /// Stable issuer fingerprint.
    pub fingerprint: [u8; 32],
    /// Ed25519 public key.
    pub ed25519_public: [u8; 32],
    /// Optional ML-DSA-65 public key (required for Phase 2+).
    #[norito(default)]
    pub mldsa65_public: Vec<u8>,
}

/// Relay entry embedded in guard directory snapshots.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct GuardDirectoryRelayEntryV2 {
    /// Serialized `RelayCertificateBundleV2` payload.
    pub certificate: Vec<u8>,
}

/// Compute the canonical issuer fingerprint used by SRC v2.
#[must_use]
pub fn compute_issuer_fingerprint(ed25519: &[u8; 32], mldsa_public: &[u8]) -> [u8; 32] {
    let mut hasher = Blake3Hasher::new();
    hasher.update(SRC_V2_ISSUER_FINGERPRINT_DOMAIN);
    hasher.update(ed25519);
    let length = u32::try_from(mldsa_public.len()).expect("ML-DSA key length must fit into u32");
    hasher.update(&length.to_be_bytes());
    hasher.update(mldsa_public);
    hasher.finalize().into()
}

/// Encode the validation phase to its wire representation.
#[must_use]
pub const fn encode_validation_phase(phase: CertificateValidationPhase) -> u8 {
    match phase {
        CertificateValidationPhase::Phase1AllowSingle => 1,
        CertificateValidationPhase::Phase2PreferDual => 2,
        CertificateValidationPhase::Phase3RequireDual => 3,
    }
}

/// Decode a validation phase from its wire representation.
#[must_use]
pub const fn decode_validation_phase(raw: u8) -> Option<CertificateValidationPhase> {
    match raw {
        1 => Some(CertificateValidationPhase::Phase1AllowSingle),
        2 => Some(CertificateValidationPhase::Phase2PreferDual),
        3 => Some(CertificateValidationPhase::Phase3RequireDual),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soranet::certificate::CertificateValidationPhase;

    #[test]
    fn encode_decode_validation_phase_roundtrip() {
        for phase in [
            CertificateValidationPhase::Phase1AllowSingle,
            CertificateValidationPhase::Phase2PreferDual,
            CertificateValidationPhase::Phase3RequireDual,
        ] {
            let raw = encode_validation_phase(phase);
            assert_eq!(decode_validation_phase(raw), Some(phase));
        }
        assert_eq!(decode_validation_phase(0), None);
        assert_eq!(decode_validation_phase(4), None);
    }

    #[test]
    fn compute_fingerprint_changes_with_keys() {
        let ed_a = [0x11; 32];
        let ed_b = [0x22; 32];
        let ml_a = vec![0xAA; 1952];
        let ml_b = vec![0xBB; 1952];

        let fingerprint_a = compute_issuer_fingerprint(&ed_a, &ml_a);
        let fingerprint_b = compute_issuer_fingerprint(&ed_b, &ml_a);
        let fingerprint_c = compute_issuer_fingerprint(&ed_a, &ml_b);

        assert_ne!(fingerprint_a, fingerprint_b);
        assert_ne!(fingerprint_a, fingerprint_c);
        assert_ne!(fingerprint_b, fingerprint_c);
    }

    #[test]
    fn snapshot_roundtrip() {
        let snapshot = GuardDirectorySnapshotV2 {
            version: GUARD_DIRECTORY_VERSION_V2,
            directory_hash: [0xAB; 32],
            published_at_unix: 1_734_000_000,
            valid_after_unix: 1_734_000_000,
            valid_until_unix: 1_734_086_400,
            validation_phase: encode_validation_phase(CertificateValidationPhase::Phase2PreferDual),
            issuers: vec![GuardDirectoryIssuerV1 {
                fingerprint: [0xCD; 32],
                ed25519_public: [0x11; 32],
                mldsa65_public: vec![0x44; 1952],
            }],
            relays: vec![GuardDirectoryRelayEntryV2 {
                certificate: vec![0x99, 0x00, 0x01],
            }],
        };

        let bytes = snapshot.to_bytes().expect("serialize");
        let decoded = GuardDirectorySnapshotV2::from_bytes(&bytes).expect("deserialize");
        assert_eq!(snapshot, decoded);
    }
}
