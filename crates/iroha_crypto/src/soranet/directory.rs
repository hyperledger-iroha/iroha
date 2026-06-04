//! Guard directory snapshot helpers.

#![allow(unexpected_cfgs)]

use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
};

use blake3::Hasher as Blake3Hasher;
use norito::{NoritoDeserialize, NoritoSerialize, decode_from_bytes, to_bytes};
use soranet_pq::MlDsaSuite;

use crate::{
    signature::ed25519::{Ed25519Sha512, PublicKey as Ed25519PublicKey},
    soranet::certificate::{
        CertificateValidationPhase, RelayCertificateBundleV2, RelayCertificateV2,
    },
};

const SRC_V2_ISSUER_FINGERPRINT_DOMAIN: &[u8] = b"soranet.src.v2.issuer";

type IssuersByFingerprint<'a> = HashMap<[u8; 32], (Ed25519PublicKey, &'a [u8])>;

/// Schema version used by `GuardDirectorySnapshotV2`.
pub const GUARD_DIRECTORY_VERSION_V2: u8 = 2;

/// Norito-encoded guard directory snapshot.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[norito(decode_from_slice)]
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
        let snapshot: Self = decode_from_bytes(bytes)?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    fn validate(&self) -> Result<(), norito::Error> {
        let validation_phase = self.validate_header()?;
        let issuers_by_fingerprint = self.validate_issuers(validation_phase)?;
        self.validate_relays(validation_phase, &issuers_by_fingerprint)
    }

    fn validate_header(&self) -> Result<CertificateValidationPhase, norito::Error> {
        if self.version != GUARD_DIRECTORY_VERSION_V2 {
            return Err(norito::Error::Message(format!(
                "guard directory snapshot version mismatch (expected {GUARD_DIRECTORY_VERSION_V2}, got {})",
                self.version
            )));
        }
        let validation_phase = decode_validation_phase(self.validation_phase).ok_or_else(|| {
            norito::Error::Message(format!(
                "guard directory snapshot validation_phase {} is not recognised",
                self.validation_phase
            ))
        })?;
        if self.published_at_unix < 0 || self.valid_after_unix < 0 || self.valid_until_unix < 0 {
            return Err(norito::Error::Message(
                "guard directory snapshot timestamps must be non-negative".to_string(),
            ));
        }
        if self.valid_after_unix >= self.valid_until_unix {
            return Err(norito::Error::Message(
                "guard directory snapshot valid_until_unix must be greater than valid_after_unix"
                    .to_string(),
            ));
        }
        if self.published_at_unix > self.valid_until_unix {
            return Err(norito::Error::Message(
                "guard directory snapshot published_at_unix exceeds valid_until_unix".to_string(),
            ));
        }
        Ok(validation_phase)
    }

    fn validate_issuers(
        &self,
        validation_phase: CertificateValidationPhase,
    ) -> Result<IssuersByFingerprint<'_>, norito::Error> {
        if self.issuers.is_empty() {
            return Err(norito::Error::Message(
                "guard directory snapshot must contain at least one issuer".to_string(),
            ));
        }
        let mut issuer_fingerprints = HashSet::with_capacity(self.issuers.len());
        let mut issuers_by_fingerprint = HashMap::with_capacity(self.issuers.len());
        for issuer in &self.issuers {
            if !issuer_fingerprints.insert(issuer.fingerprint) {
                return Err(norito::Error::Message(
                    "guard directory snapshot contains duplicate issuer fingerprint".to_string(),
                ));
            }
            let ed25519_public =
                Ed25519Sha512::parse_public_key(&issuer.ed25519_public).map_err(|err| {
                    norito::Error::Message(format!(
                        "guard directory issuer Ed25519 public key is invalid: {err}"
                    ))
                })?;
            Self::validate_issuer_mldsa65_public_key_len(validation_phase, &issuer.mldsa65_public)?;
            let computed =
                try_compute_issuer_fingerprint(&issuer.ed25519_public, &issuer.mldsa65_public)?;
            if computed != issuer.fingerprint {
                return Err(norito::Error::Message(
                    "guard directory issuer fingerprint does not match advertised keys".to_string(),
                ));
            }
            issuers_by_fingerprint.insert(
                issuer.fingerprint,
                (ed25519_public, issuer.mldsa65_public.as_slice()),
            );
        }
        Ok(issuers_by_fingerprint)
    }

    fn validate_issuer_mldsa65_public_key_len(
        validation_phase: CertificateValidationPhase,
        mldsa65_public: &[u8],
    ) -> Result<(), norito::Error> {
        if mldsa65_public.is_empty() {
            if validation_phase != CertificateValidationPhase::Phase1AllowSingle {
                return Err(norito::Error::Message(
                    "guard directory issuer ML-DSA-65 public key is required for validation phase"
                        .to_string(),
                ));
            }
            return Ok(());
        }
        let expected = MlDsaSuite::MlDsa65.public_key_len();
        if mldsa65_public.len() != expected {
            return Err(norito::Error::Message(format!(
                "guard directory issuer ML-DSA-65 public key must be {expected} bytes, got {}",
                mldsa65_public.len()
            )));
        }
        Ok(())
    }

    fn validate_relays(
        &self,
        validation_phase: CertificateValidationPhase,
        issuers_by_fingerprint: &IssuersByFingerprint<'_>,
    ) -> Result<(), norito::Error> {
        if self.relays.is_empty() {
            return Err(norito::Error::Message(
                "guard directory snapshot must contain at least one relay".to_string(),
            ));
        }
        let mut relay_ids = HashSet::with_capacity(self.relays.len());
        for relay in &self.relays {
            let bundle =
                RelayCertificateBundleV2::from_cbor(&relay.certificate).map_err(|err| {
                    norito::Error::Message(format!(
                        "guard directory relay certificate bundle is invalid: {err}"
                    ))
                })?;
            let issuer = issuers_by_fingerprint
                .get(&bundle.certificate.issuer_fingerprint)
                .ok_or_else(|| {
                    norito::Error::Message(
                        "guard directory relay certificate references unknown issuer fingerprint"
                            .to_string(),
                    )
                })?;
            if bundle.certificate.directory_hash != self.directory_hash {
                return Err(norito::Error::Message(
                    "guard directory relay certificate directory_hash does not match snapshot"
                        .to_string(),
                ));
            }
            if !relay_ids.insert(bundle.certificate.relay_id) {
                return Err(norito::Error::Message(
                    "guard directory snapshot contains duplicate relay id".to_string(),
                ));
            }
            self.validate_relay_certificate_window(&bundle.certificate)?;
            bundle
                .verify(&issuer.0, issuer.1, validation_phase)
                .map_err(|err| {
                    norito::Error::Message(format!(
                        "guard directory relay certificate signature verification failed: {err}"
                    ))
                })?;
        }
        Ok(())
    }

    fn validate_relay_certificate_window(
        &self,
        certificate: &RelayCertificateV2,
    ) -> Result<(), norito::Error> {
        if certificate.published_at > self.published_at_unix {
            return Err(norito::Error::Message(
                "guard directory relay certificate published_at is after snapshot publication"
                    .to_string(),
            ));
        }
        if certificate.valid_after > self.valid_after_unix {
            return Err(norito::Error::Message(
                "guard directory relay certificate is not valid at snapshot valid_after"
                    .to_string(),
            ));
        }
        if certificate.valid_until < self.valid_until_unix {
            return Err(norito::Error::Message(
                "guard directory relay certificate expires before snapshot valid_until".to_string(),
            ));
        }
        Ok(())
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
///
/// # Errors
/// Returns an error if the ML-DSA public-key length cannot be represented in
/// the fingerprint's fixed `u32` length field.
pub fn compute_issuer_fingerprint(
    ed25519: &[u8; 32],
    mldsa_public: &[u8],
) -> Result<[u8; 32], norito::Error> {
    compute_issuer_fingerprint_inner(ed25519, mldsa_public)
}

/// Compute the canonical issuer fingerprint used by SRC v2.
///
/// # Errors
/// Returns an error if the ML-DSA public-key length cannot be represented in
/// the fingerprint's fixed `u32` length field.
pub fn try_compute_issuer_fingerprint(
    ed25519: &[u8; 32],
    mldsa_public: &[u8],
) -> Result<[u8; 32], norito::Error> {
    compute_issuer_fingerprint_inner(ed25519, mldsa_public)
}

fn compute_issuer_fingerprint_inner(
    ed25519: &[u8; 32],
    mldsa_public: &[u8],
) -> Result<[u8; 32], norito::Error> {
    let mut hasher = Blake3Hasher::new();
    hasher.update(SRC_V2_ISSUER_FINGERPRINT_DOMAIN);
    hasher.update(ed25519);
    hasher.update(&issuer_fingerprint_len_bytes(mldsa_public.len())?);
    hasher.update(mldsa_public);
    Ok(hasher.finalize().into())
}

fn issuer_fingerprint_len_bytes(len: usize) -> Result<[u8; 4], norito::Error> {
    let len = u32::try_from(len).map_err(|_| {
        norito::Error::Message(format!(
            "guard directory issuer ML-DSA public key length {len} exceeds u32::MAX"
        ))
    })?;
    Ok(len.to_be_bytes())
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
    use crate::soranet::{
        certificate::{
            CapabilityToggle, CertificateValidationPhase, KemRotationModeV1, KemRotationPolicyV1,
            RelayCapabilityFlagsV1, RelayCertificateV2, RelayEndpointV2, RelayRolesV2,
        },
        handshake::HandshakeSuite,
    };
    use ed25519_dalek::{SECRET_KEY_LENGTH, SigningKey};
    use soranet_pq::{
        HedgedRngSeed, MlKemSuite, generate_mldsa_keypair_from_seed as generate_mldsa_keypair,
    };

    fn sample_issuer_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[0x11; SECRET_KEY_LENGTH])
    }

    fn sample_mldsa_keypair(personalization: &'static [u8]) -> soranet_pq::MlDsaKeyPair {
        generate_mldsa_keypair(
            MlDsaSuite::MlDsa65,
            HedgedRngSeed::from_entropy([0x44; 32]),
            personalization,
        )
        .expect("sample ML-DSA keypair")
    }

    fn sample_relay_certificate(
        directory_hash: [u8; 32],
        issuer_fingerprint: [u8; 32],
        relay_id: [u8; 32],
    ) -> RelayCertificateV2 {
        RelayCertificateV2 {
            relay_id,
            identity_ed25519: [0x22; 32],
            identity_mldsa65: vec![0x55; MlDsaSuite::MlDsa65.public_key_len()],
            descriptor_commit: [0x33; 32],
            roles: RelayRolesV2 {
                entry: true,
                middle: true,
                exit: false,
            },
            guard_weight: 100,
            bandwidth_bytes_per_sec: 1_000_000,
            reputation_weight: 50,
            endpoints: vec![RelayEndpointV2 {
                url: "quic://relay.example.test:443".to_string(),
                priority: 0,
                tags: vec!["nk3".to_string()],
            }],
            capability_flags: RelayCapabilityFlagsV1::new(
                CapabilityToggle::Enabled,
                CapabilityToggle::Disabled,
                CapabilityToggle::Enabled,
                CapabilityToggle::Disabled,
            ),
            kem_policy: KemRotationPolicyV1 {
                mode: KemRotationModeV1::Static,
                preferred_suite: MlKemSuite::MlKem768.kem_id(),
                fallback_suite: None,
                rotation_interval_hours: 0,
                grace_period_hours: 0,
            },
            handshake_suites: vec![
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            published_at: 1_734_000_000,
            valid_after: 1_734_000_000,
            valid_until: 1_734_086_400,
            directory_hash,
            issuer_fingerprint,
            pq_kem_public: vec![0x66; MlKemSuite::MlKem768.public_key_len()],
        }
    }

    fn sample_relay_bundle(
        directory_hash: [u8; 32],
        issuer_fingerprint: [u8; 32],
        relay_id: [u8; 32],
        issuer_signing_key: &SigningKey,
        issuer_mldsa_secret_key: &[u8],
        include_mldsa_signature: bool,
    ) -> RelayCertificateBundleV2 {
        let mut bundle = sample_relay_certificate(directory_hash, issuer_fingerprint, relay_id)
            .issue(issuer_signing_key, issuer_mldsa_secret_key)
            .expect("sample relay certificate issue");
        if !include_mldsa_signature {
            bundle.signatures.mldsa65 = None;
        }
        bundle
    }

    fn replace_first_relay_bundle(
        snapshot: &mut GuardDirectorySnapshotV2,
        bundle: &RelayCertificateBundleV2,
    ) {
        snapshot.relays[0].certificate = bundle.to_cbor();
    }

    fn mutate_first_relay_bundle(
        snapshot: &mut GuardDirectorySnapshotV2,
        mutate: impl FnOnce(&mut RelayCertificateBundleV2),
    ) {
        let mut bundle = RelayCertificateBundleV2::from_cbor(&snapshot.relays[0].certificate)
            .expect("sample relay bundle decodes");
        mutate(&mut bundle);
        replace_first_relay_bundle(snapshot, &bundle);
    }

    fn sample_snapshot() -> GuardDirectorySnapshotV2 {
        let issuer_signing_key = sample_issuer_signing_key();
        let issuer_mldsa = sample_mldsa_keypair(b"directory-snapshot-issuer");
        let ed25519_public = issuer_signing_key.verifying_key().to_bytes();
        let mldsa65_public = issuer_mldsa.public_key().to_vec();
        let fingerprint = compute_issuer_fingerprint(&ed25519_public, &mldsa65_public)
            .expect("sample issuer fingerprint should compute");
        let directory_hash = [0xAB; 32];
        GuardDirectorySnapshotV2 {
            version: GUARD_DIRECTORY_VERSION_V2,
            directory_hash,
            published_at_unix: 1_734_000_000,
            valid_after_unix: 1_734_000_000,
            valid_until_unix: 1_734_086_400,
            validation_phase: encode_validation_phase(CertificateValidationPhase::Phase2PreferDual),
            issuers: vec![GuardDirectoryIssuerV1 {
                fingerprint,
                ed25519_public,
                mldsa65_public,
            }],
            relays: vec![GuardDirectoryRelayEntryV2 {
                certificate: sample_relay_bundle(
                    directory_hash,
                    fingerprint,
                    [0x99; 32],
                    &issuer_signing_key,
                    issuer_mldsa.secret_key(),
                    true,
                )
                .to_cbor(),
            }],
        }
    }

    fn sample_phase1_single_signature_snapshot() -> GuardDirectorySnapshotV2 {
        let issuer_signing_key = sample_issuer_signing_key();
        let issuer_mldsa = sample_mldsa_keypair(b"directory-snapshot-phase1-signer");
        let ed25519_public = issuer_signing_key.verifying_key().to_bytes();
        let mldsa65_public = Vec::new();
        let fingerprint = compute_issuer_fingerprint(&ed25519_public, &mldsa65_public)
            .expect("sample phase-1 issuer fingerprint should compute");
        let directory_hash = [0xAB; 32];
        GuardDirectorySnapshotV2 {
            version: GUARD_DIRECTORY_VERSION_V2,
            directory_hash,
            published_at_unix: 1_734_000_000,
            valid_after_unix: 1_734_000_000,
            valid_until_unix: 1_734_086_400,
            validation_phase: encode_validation_phase(
                CertificateValidationPhase::Phase1AllowSingle,
            ),
            issuers: vec![GuardDirectoryIssuerV1 {
                fingerprint,
                ed25519_public,
                mldsa65_public,
            }],
            relays: vec![GuardDirectoryRelayEntryV2 {
                certificate: sample_relay_bundle(
                    directory_hash,
                    fingerprint,
                    [0x99; 32],
                    &issuer_signing_key,
                    issuer_mldsa.secret_key(),
                    false,
                )
                .to_cbor(),
            }],
        }
    }

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

        let fingerprint_a =
            compute_issuer_fingerprint(&ed_a, &ml_a).expect("fingerprint A should compute");
        let fingerprint_b =
            compute_issuer_fingerprint(&ed_b, &ml_a).expect("fingerprint B should compute");
        let fingerprint_c =
            compute_issuer_fingerprint(&ed_a, &ml_b).expect("fingerprint C should compute");

        assert_ne!(fingerprint_a, fingerprint_b);
        assert_ne!(fingerprint_a, fingerprint_c);
        assert_ne!(fingerprint_b, fingerprint_c);
    }

    #[test]
    fn compute_fingerprint_matches_try_helper() {
        let ed25519 = [0x11; 32];
        let mldsa_public = vec![0xAA; 1952];

        let via_try = try_compute_issuer_fingerprint(&ed25519, &mldsa_public)
            .expect("canonical issuer fingerprint should compute");
        let direct = compute_issuer_fingerprint(&ed25519, &mldsa_public)
            .expect("canonical issuer fingerprint should compute");

        assert_eq!(via_try, direct);
    }

    #[test]
    fn issuer_fingerprint_length_overflow_fails_closed() {
        let Some(too_long) = (u64::from(u32::MAX) + 1).try_into().ok() else {
            return;
        };

        let err = issuer_fingerprint_len_bytes(too_long)
            .expect_err("oversized issuer public-key length must fail closed");
        assert!(
            err.to_string().contains("exceeds u32::MAX"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn snapshot_roundtrip() {
        let snapshot = sample_snapshot();

        let bytes = snapshot.to_bytes().expect("serialize");
        let decoded = GuardDirectorySnapshotV2::from_bytes(&bytes).expect("deserialize");
        assert_eq!(snapshot, decoded);
    }

    #[test]
    fn snapshot_rejects_unknown_validation_phase() {
        let mut snapshot = sample_snapshot();
        snapshot.validation_phase = 0;
        let bytes = snapshot.to_bytes().expect("serialize");
        assert!(GuardDirectorySnapshotV2::from_bytes(&bytes).is_err());
    }

    #[test]
    fn snapshot_rejects_version_mismatch() {
        let mut snapshot = sample_snapshot();
        snapshot.version = 1;
        let bytes = snapshot.to_bytes().expect("serialize");
        assert!(GuardDirectorySnapshotV2::from_bytes(&bytes).is_err());
    }

    #[test]
    fn snapshot_rejects_empty_issuer_set() {
        let mut snapshot = sample_snapshot();
        snapshot.issuers.clear();
        let bytes = snapshot.to_bytes().expect("serialize");

        let err =
            GuardDirectorySnapshotV2::from_bytes(&bytes).expect_err("empty issuer set must fail");
        assert!(err.to_string().contains("at least one issuer"));
    }

    #[test]
    fn snapshot_rejects_empty_relay_set() {
        let mut snapshot = sample_snapshot();
        snapshot.relays.clear();
        let bytes = snapshot.to_bytes().expect("serialize");

        let err =
            GuardDirectorySnapshotV2::from_bytes(&bytes).expect_err("empty relay set must fail");
        assert!(err.to_string().contains("at least one relay"));
    }

    #[test]
    fn snapshot_rejects_invalid_time_window() {
        let mut snapshot = sample_snapshot();
        snapshot.valid_until_unix = snapshot.valid_after_unix;
        let bytes = snapshot.to_bytes().expect("serialize");
        assert!(GuardDirectorySnapshotV2::from_bytes(&bytes).is_err());

        let mut snapshot = sample_snapshot();
        snapshot.valid_after_unix = snapshot.valid_until_unix + 1;
        let bytes = snapshot.to_bytes().expect("serialize");
        assert!(GuardDirectorySnapshotV2::from_bytes(&bytes).is_err());
    }

    #[test]
    fn snapshot_rejects_issuer_fingerprint_mismatch() {
        let mut snapshot = sample_snapshot();
        snapshot.issuers[0].fingerprint[0] ^= 0xFF;
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect_err("fingerprint mismatch should fail");
        assert!(err.to_string().contains("fingerprint"));
    }

    #[test]
    fn snapshot_rejects_duplicate_issuer_fingerprints() {
        let mut snapshot = sample_snapshot();
        let duplicate = snapshot.issuers[0].clone();
        snapshot.issuers.push(duplicate);
        let bytes = snapshot.to_bytes().expect("serialize");
        let err =
            GuardDirectorySnapshotV2::from_bytes(&bytes).expect_err("duplicate issuer should fail");
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn snapshot_rejects_invalid_mldsa65_public_key_length() {
        let mut snapshot = sample_snapshot();
        snapshot.issuers[0].mldsa65_public.pop();
        snapshot.issuers[0].fingerprint = compute_issuer_fingerprint(
            &snapshot.issuers[0].ed25519_public,
            &snapshot.issuers[0].mldsa65_public,
        )
        .expect("sample issuer fingerprint should compute");
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect_err("invalid ML-DSA-65 public key length should fail");
        assert!(err.to_string().contains("ML-DSA-65 public key"));
    }

    #[test]
    fn snapshot_rejects_invalid_mldsa65_public_key_length_before_fingerprint() {
        let mut snapshot = sample_snapshot();
        snapshot.issuers[0].mldsa65_public.truncate(1);
        snapshot.issuers[0].fingerprint = [0xEE; 32];
        let bytes = snapshot.to_bytes().expect("serialize");

        let err = GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect_err("issuer ML-DSA-65 key shape must fail before fingerprint");
        let message = err.to_string();
        assert!(
            message.contains("ML-DSA-65 public key"),
            "unexpected message: {message}"
        );
        assert!(
            !message.contains("fingerprint"),
            "shape preflight should run before fingerprint comparison: {message}"
        );
    }

    #[test]
    fn snapshot_rejects_missing_mldsa65_public_key_after_phase1() {
        let mut snapshot = sample_snapshot();
        snapshot.issuers[0].mldsa65_public.clear();
        snapshot.issuers[0].fingerprint = compute_issuer_fingerprint(
            &snapshot.issuers[0].ed25519_public,
            &snapshot.issuers[0].mldsa65_public,
        )
        .expect("sample issuer fingerprint should compute");
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect_err("phase 2 requires ML-DSA-65 issuer key");
        assert!(err.to_string().contains("required"));
    }

    #[test]
    fn snapshot_allows_phase1_empty_mldsa65_public_key() {
        let snapshot = sample_phase1_single_signature_snapshot();
        let bytes = snapshot.to_bytes().expect("serialize");
        GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect("phase 1 may carry an issuer without ML-DSA-65 key");
    }

    #[test]
    fn snapshot_phase2_accepts_single_signature_relay() {
        let mut snapshot = sample_snapshot();
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            bundle.signatures.mldsa65 = None;
        });
        let bytes = snapshot.to_bytes().expect("serialize");
        GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect("phase 2 accepts Ed25519-only relay certificates during rollout");
    }

    #[test]
    fn snapshot_rejects_invalid_issuer_ed25519_public_key() {
        let mut snapshot = sample_snapshot();
        snapshot.issuers[0].ed25519_public = [0xFF; 32];
        snapshot.issuers[0].fingerprint = compute_issuer_fingerprint(
            &snapshot.issuers[0].ed25519_public,
            &snapshot.issuers[0].mldsa65_public,
        )
        .expect("sample issuer fingerprint should compute");
        let bytes = snapshot.to_bytes().expect("serialize");

        let err = GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect_err("invalid issuer Ed25519 public key should fail");
        assert!(err.to_string().contains("Ed25519 public key"));
    }

    #[test]
    fn snapshot_rejects_malformed_relay_certificate_bundle() {
        let mut snapshot = sample_snapshot();
        snapshot.relays[0].certificate = vec![0x99, 0x00, 0x01];
        let bytes = snapshot.to_bytes().expect("serialize");

        let err = GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect_err("malformed relay certificate bundle should fail");
        assert!(err.to_string().contains("relay certificate bundle"));
    }

    #[test]
    fn snapshot_rejects_relay_certificate_unknown_issuer() {
        let mut snapshot = sample_snapshot();
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            bundle.certificate.issuer_fingerprint = [0xEE; 32];
        });
        let bytes = snapshot.to_bytes().expect("serialize");

        let err = GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect_err("relay certificate with unknown issuer should fail");
        assert!(err.to_string().contains("unknown issuer"));
    }

    #[test]
    fn snapshot_rejects_relay_certificate_directory_hash_mismatch() {
        let mut snapshot = sample_snapshot();
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            bundle.certificate.directory_hash = [0xDD; 32];
        });
        let bytes = snapshot.to_bytes().expect("serialize");

        let err = GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect_err("relay certificate with mismatched directory hash should fail");
        assert!(err.to_string().contains("directory_hash"));
    }

    #[test]
    fn snapshot_rejects_relay_certificate_outside_snapshot_window() {
        let mut snapshot = sample_snapshot();
        let snapshot_valid_after = snapshot.valid_after_unix;
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            bundle.certificate.valid_after = snapshot_valid_after + 1;
        });
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect_err("relay certificate not valid at snapshot start should fail");
        assert!(err.to_string().contains("valid_after"));

        let mut snapshot = sample_snapshot();
        let snapshot_valid_until = snapshot.valid_until_unix;
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            bundle.certificate.valid_until = snapshot_valid_until - 1;
        });
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect_err("relay certificate expiring inside snapshot window should fail");
        assert!(err.to_string().contains("valid_until"));

        let mut snapshot = sample_snapshot();
        let snapshot_published_at = snapshot.published_at_unix;
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            bundle.certificate.published_at = snapshot_published_at + 1;
        });
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect_err("relay certificate published after snapshot should fail");
        assert!(err.to_string().contains("published_at"));
    }

    #[test]
    fn snapshot_rejects_duplicate_relay_ids() {
        let mut snapshot = sample_snapshot();
        snapshot.relays.push(snapshot.relays[0].clone());
        let bytes = snapshot.to_bytes().expect("serialize");

        let err = GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect_err("duplicate relay id should fail");
        assert!(err.to_string().contains("duplicate relay id"));
    }

    #[test]
    fn snapshot_rejects_bad_relay_certificate_ed25519_signature() {
        let mut snapshot = sample_snapshot();
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            bundle.signatures.ed25519[0] ^= 0xFF;
        });
        let bytes = snapshot.to_bytes().expect("serialize");

        let err = GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect_err("bad relay Ed25519 signature should fail");
        assert!(err.to_string().contains("signature verification"));
    }

    #[test]
    fn snapshot_rejects_bad_relay_certificate_mldsa65_signature() {
        let mut snapshot = sample_snapshot();
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            let signature = bundle
                .signatures
                .mldsa65
                .as_mut()
                .expect("ML-DSA signature");
            signature[0] ^= 0xFF;
        });
        let bytes = snapshot.to_bytes().expect("serialize");

        let err = GuardDirectorySnapshotV2::from_bytes(&bytes)
            .expect_err("bad relay ML-DSA signature should fail");
        assert!(err.to_string().contains("signature verification"));
    }
}
