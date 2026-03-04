//! Guard directory validation helpers used by the relay runtime.

#![allow(unexpected_cfgs)]

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ed25519_dalek::VerifyingKey;
use hex::{FromHexError, encode as hex_encode};
use iroha_crypto::soranet::{
    certificate::{CertificateError, CertificateValidationPhase, RelayCertificateBundleV2},
    directory::{GuardDirectoryRelayEntryV2, GuardDirectorySnapshotV2, decode_validation_phase},
};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use tempfile::NamedTempFile;
use thiserror::Error;

use crate::config::{ConfigError, GuardDirectoryConfig};

/// Result of resolving the relay entry from a guard directory snapshot.
#[derive(Debug)]
pub struct GuardDirectoryEntry {
    /// Verified bundle describing this relay.
    pub bundle: RelayCertificateBundleV2,
    /// Directory hash advertised in the snapshot.
    pub directory_hash: [u8; 32],
    /// Validation phase encoded in the snapshot metadata.
    pub validation_phase: CertificateValidationPhase,
}

/// Errors raised when resolving or validating guard directory metadata.
#[derive(Debug, Error)]
pub enum GuardDirectoryError {
    #[error("failed to read guard snapshot `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to decode guard snapshot `{path}`: {source}")]
    Decode {
        path: PathBuf,
        #[source]
        source: norito::Error,
    },
    #[error("guard snapshot `{path}` advertises unknown validation phase `{raw}`")]
    UnknownValidationPhase { path: PathBuf, raw: u8 },
    #[error("expected guard directory hash {expected} but snapshot `{path}` contained {found}")]
    DirectoryHashMismatch {
        path: PathBuf,
        expected: String,
        found: String,
    },
    #[error("relay entry not found in `{path}` for identity {identity_hex}")]
    RelayEntryMissing { path: PathBuf, identity_hex: String },
    #[error("relay certificate references issuer {fingerprint} that is not present in `{path}`")]
    IssuerMissing { path: PathBuf, fingerprint: String },
    #[error("issuer public key in `{path}` is invalid: {source}")]
    IssuerKey {
        path: PathBuf,
        #[source]
        source: ed25519_dalek::SignatureError,
    },
    #[error("relay certificate signature validation failed: {0}")]
    Certificate(#[from] CertificateError),
    #[error(
        "descriptor commit mismatch between guard snapshot ({found}) and local configuration ({expected})"
    )]
    DescriptorCommitMismatch { expected: String, found: String },
}

/// Errors raised when persisting guard pinning proofs.
#[derive(Debug, Error)]
pub enum GuardPinningProofError {
    #[error("failed to serialize guard pinning proof: {0}")]
    Serialize(json::Error),
    #[error("system clock is before UNIX epoch")]
    Clock,
    #[error("failed to create directory `{path}`: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to create temporary pinning proof in `{path}`: {source}")]
    CreateTemp {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write pinning proof to `{path}`: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to persist pinning proof `{path}`: {source}")]
    Persist {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Errors raised when validating persisted guard pinning proofs.
#[derive(Debug, Error)]
pub enum GuardPinningProofValidationError {
    #[error("unsupported guard pinning proof version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("invalid hex in field `{field}`: {source}")]
    Hex {
        field: &'static str,
        #[source]
        source: FromHexError,
    },
    #[error("field `{field}` must decode to {expected} bytes (got {found})")]
    HexLength {
        field: &'static str,
        expected: usize,
        found: usize,
    },
    #[error(
        "guard pinning proof directory hash `{found}` does not match snapshot hash `{expected}`"
    )]
    DirectoryHashMismatch { expected: String, found: String },
    #[error("relay `{relay_hex}` not found in guard directory snapshot")]
    RelayMissing { relay_hex: String },
    #[error(
        "descriptor commit mismatch for relay `{relay_hex}`: proof {found}, snapshot {expected}"
    )]
    DescriptorCommitMismatch {
        relay_hex: String,
        expected: String,
        found: String,
    },
    #[error(
        "issuer fingerprint mismatch for relay `{relay_hex}`: proof {found}, snapshot {expected}"
    )]
    IssuerFingerprintMismatch {
        relay_hex: String,
        expected: String,
        found: String,
    },
    #[error("validation phase `{found}` is not recognised")]
    InvalidValidationPhase { found: String },
    #[error("validation phase mismatch: proof `{found}`, snapshot `{expected}`")]
    ValidationPhaseMismatch { expected: String, found: String },
    #[error("{field} mismatch: proof `{found}`, snapshot `{expected}`")]
    FieldMismatch {
        field: &'static str,
        expected: String,
        found: String,
    },
    #[error("PQ KEM public key mismatch for relay `{relay_hex}`")]
    PqKemMismatch { relay_hex: String },
}

/// Resolve and verify the relay entry referenced by the supplied configuration.
pub fn load_guard_entry(
    config: &GuardDirectoryConfig,
    identity_ed25519: &[u8; 32],
    expected_descriptor_commit: &[u8; 32],
) -> Result<GuardDirectoryEntry, GuardDirectoryError> {
    let path = config.snapshot_path().to_path_buf();
    let data = fs::read(&path).map_err(|source| GuardDirectoryError::Io {
        path: path.clone(),
        source,
    })?;
    let snapshot = GuardDirectorySnapshotV2::from_bytes(&data).map_err(|source| {
        GuardDirectoryError::Decode {
            path: path.clone(),
            source,
        }
    })?;

    if let Some(expected_hash) = config.expected_directory_hash()
        && snapshot.directory_hash != expected_hash
    {
        return Err(GuardDirectoryError::DirectoryHashMismatch {
            path: path.clone(),
            expected: hex_encode(expected_hash),
            found: hex_encode(snapshot.directory_hash),
        });
    }

    let validation_phase = decode_validation_phase(snapshot.validation_phase).ok_or(
        GuardDirectoryError::UnknownValidationPhase {
            path: path.clone(),
            raw: snapshot.validation_phase,
        },
    )?;

    let bundle = find_relay_entry(&snapshot.relays, identity_ed25519).ok_or_else(|| {
        GuardDirectoryError::RelayEntryMissing {
            path: path.clone(),
            identity_hex: hex_encode(identity_ed25519),
        }
    })?;

    if bundle.certificate.descriptor_commit != *expected_descriptor_commit {
        return Err(GuardDirectoryError::DescriptorCommitMismatch {
            expected: hex_encode(expected_descriptor_commit),
            found: hex_encode(bundle.certificate.descriptor_commit),
        });
    }

    let issuer = snapshot
        .issuers
        .iter()
        .find(|issuer| issuer.fingerprint == bundle.certificate.issuer_fingerprint)
        .ok_or_else(|| GuardDirectoryError::IssuerMissing {
            path: path.clone(),
            fingerprint: hex_encode(bundle.certificate.issuer_fingerprint),
        })?;
    let ed25519 = VerifyingKey::from_bytes(&issuer.ed25519_public).map_err(|source| {
        GuardDirectoryError::IssuerKey {
            path: path.clone(),
            source,
        }
    })?;
    bundle.verify(&ed25519, &issuer.mldsa65_public, validation_phase)?;

    Ok(GuardDirectoryEntry {
        bundle,
        directory_hash: snapshot.directory_hash,
        validation_phase,
    })
}

fn find_relay_entry(
    entries: &[GuardDirectoryRelayEntryV2],
    identity_ed25519: &[u8; 32],
) -> Option<RelayCertificateBundleV2> {
    entries.iter().find_map(|entry| {
        RelayCertificateBundleV2::from_cbor(&entry.certificate)
            .ok()
            .filter(|bundle| bundle.certificate.identity_ed25519 == *identity_ed25519)
    })
}

/// Persist guard pinning metadata for directory publisher consumption.
pub fn persist_guard_pinning_proof(
    path: &Path,
    snapshot_path: &Path,
    entry: &GuardDirectoryEntry,
    relay_id: &[u8; 32],
    recorded_at: SystemTime,
) -> Result<(), GuardPinningProofError> {
    let recorded_secs = recorded_at
        .duration_since(UNIX_EPOCH)
        .map_err(|_| GuardPinningProofError::Clock)?
        .as_secs();
    let recorded_at_unix =
        i64::try_from(recorded_secs).map_err(|_| GuardPinningProofError::Clock)?;
    let certificate = &entry.bundle.certificate;
    let proof = GuardPinningProof {
        version: 1,
        recorded_at_unix,
        snapshot_path: snapshot_path.to_string_lossy().into_owned(),
        validation_phase: validation_phase_label(entry.validation_phase).to_string(),
        relay_id_hex: hex_encode(relay_id),
        directory_hash_hex: hex_encode(entry.directory_hash),
        descriptor_commit_hex: hex_encode(certificate.descriptor_commit),
        issuer_fingerprint_hex: hex_encode(certificate.issuer_fingerprint),
        valid_after_unix: certificate.valid_after,
        valid_until_unix: certificate.valid_until,
        guard_weight: certificate.guard_weight,
        bandwidth_bytes_per_sec: certificate.bandwidth_bytes_per_sec,
        reputation_weight: certificate.reputation_weight,
        pq_kem_public_hex: hex_encode(certificate.pq_kem_public.clone()),
    };
    let rendered = json::to_json_pretty(&proof).map_err(GuardPinningProofError::Serialize)?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|source| GuardPinningProofError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let temp_parent = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut temp = NamedTempFile::new_in(&temp_parent).map_err(|source| {
        GuardPinningProofError::CreateTemp {
            path: temp_parent.clone(),
            source,
        }
    })?;
    temp.write_all(rendered.as_bytes())
        .map_err(|source| GuardPinningProofError::Write {
            path: temp.path().to_path_buf(),
            source,
        })?;
    temp.flush()
        .map_err(|source| GuardPinningProofError::Write {
            path: temp.path().to_path_buf(),
            source,
        })?;
    temp.into_temp_path()
        .persist(path)
        .map_err(|source| GuardPinningProofError::Persist {
            path: path.to_path_buf(),
            source: source.into(),
        })?;
    Ok(())
}

/// Validate a guard pinning proof against the supplied directory snapshot.
pub fn verify_guard_pinning_proof(
    snapshot: &GuardDirectorySnapshotV2,
    proof: &GuardPinningProof,
) -> Result<(), GuardPinningProofValidationError> {
    if proof.version != 1 {
        return Err(GuardPinningProofValidationError::UnsupportedVersion {
            found: proof.version,
        });
    }

    let proof_hash = hex_array_from_str::<32>(&proof.directory_hash_hex, "directory_hash_hex")?;
    if snapshot.directory_hash != proof_hash {
        return Err(GuardPinningProofValidationError::DirectoryHashMismatch {
            expected: hex_encode(snapshot.directory_hash),
            found: proof.directory_hash_hex.clone(),
        });
    }

    let relay_id = hex_array_from_str::<32>(&proof.relay_id_hex, "relay_id_hex")?;
    let descriptor_commit =
        hex_array_from_str::<32>(&proof.descriptor_commit_hex, "descriptor_commit_hex")?;
    let fingerprint =
        hex_array_from_str::<32>(&proof.issuer_fingerprint_hex, "issuer_fingerprint_hex")?;
    let pq_kem_public = hex::decode(&proof.pq_kem_public_hex).map_err(|source| {
        GuardPinningProofValidationError::Hex {
            field: "pq_kem_public_hex",
            source,
        }
    })?;

    let proof_phase = parse_validation_phase_label(&proof.validation_phase).ok_or_else(|| {
        GuardPinningProofValidationError::InvalidValidationPhase {
            found: proof.validation_phase.clone(),
        }
    })?;
    let snapshot_phase = decode_validation_phase(snapshot.validation_phase).ok_or_else(|| {
        GuardPinningProofValidationError::ValidationPhaseMismatch {
            expected: format!("raw({})", snapshot.validation_phase),
            found: proof.validation_phase.clone(),
        }
    })?;
    if snapshot_phase != proof_phase {
        return Err(GuardPinningProofValidationError::ValidationPhaseMismatch {
            expected: validation_phase_label(snapshot_phase).to_string(),
            found: proof.validation_phase.clone(),
        });
    }

    let bundle = find_relay_entry(&snapshot.relays, &relay_id).ok_or_else(|| {
        GuardPinningProofValidationError::RelayMissing {
            relay_hex: proof.relay_id_hex.clone(),
        }
    })?;
    let certificate = bundle.certificate;

    if certificate.descriptor_commit != descriptor_commit {
        return Err(GuardPinningProofValidationError::DescriptorCommitMismatch {
            relay_hex: proof.relay_id_hex.clone(),
            expected: hex_encode(certificate.descriptor_commit),
            found: proof.descriptor_commit_hex.clone(),
        });
    }
    if certificate.issuer_fingerprint != fingerprint {
        return Err(
            GuardPinningProofValidationError::IssuerFingerprintMismatch {
                relay_hex: proof.relay_id_hex.clone(),
                expected: hex_encode(certificate.issuer_fingerprint),
                found: proof.issuer_fingerprint_hex.clone(),
            },
        );
    }
    if certificate.valid_after != proof.valid_after_unix {
        return Err(GuardPinningProofValidationError::FieldMismatch {
            field: "valid_after_unix",
            expected: certificate.valid_after.to_string(),
            found: proof.valid_after_unix.to_string(),
        });
    }
    if certificate.valid_until != proof.valid_until_unix {
        return Err(GuardPinningProofValidationError::FieldMismatch {
            field: "valid_until_unix",
            expected: certificate.valid_until.to_string(),
            found: proof.valid_until_unix.to_string(),
        });
    }
    if certificate.guard_weight != proof.guard_weight {
        return Err(GuardPinningProofValidationError::FieldMismatch {
            field: "guard_weight",
            expected: certificate.guard_weight.to_string(),
            found: proof.guard_weight.to_string(),
        });
    }
    if certificate.bandwidth_bytes_per_sec != proof.bandwidth_bytes_per_sec {
        return Err(GuardPinningProofValidationError::FieldMismatch {
            field: "bandwidth_bytes_per_sec",
            expected: certificate.bandwidth_bytes_per_sec.to_string(),
            found: proof.bandwidth_bytes_per_sec.to_string(),
        });
    }
    if certificate.reputation_weight != proof.reputation_weight {
        return Err(GuardPinningProofValidationError::FieldMismatch {
            field: "reputation_weight",
            expected: certificate.reputation_weight.to_string(),
            found: proof.reputation_weight.to_string(),
        });
    }
    if certificate.pq_kem_public.as_slice() != pq_kem_public.as_slice() {
        return Err(GuardPinningProofValidationError::PqKemMismatch {
            relay_hex: proof.relay_id_hex.clone(),
        });
    }

    Ok(())
}

fn validation_phase_label(phase: CertificateValidationPhase) -> &'static str {
    match phase {
        CertificateValidationPhase::Phase1AllowSingle => "phase1_allow_single",
        CertificateValidationPhase::Phase2PreferDual => "phase2_prefer_dual",
        CertificateValidationPhase::Phase3RequireDual => "phase3_require_dual",
    }
}

fn parse_validation_phase_label(label: &str) -> Option<CertificateValidationPhase> {
    match label {
        "phase1_allow_single" => Some(CertificateValidationPhase::Phase1AllowSingle),
        "phase2_prefer_dual" => Some(CertificateValidationPhase::Phase2PreferDual),
        "phase3_require_dual" => Some(CertificateValidationPhase::Phase3RequireDual),
        _ => None,
    }
}

fn hex_array_from_str<const N: usize>(
    hex_value: &str,
    field: &'static str,
) -> Result<[u8; N], GuardPinningProofValidationError> {
    let decoded = hex::decode(hex_value)
        .map_err(|source| GuardPinningProofValidationError::Hex { field, source })?;
    if decoded.len() != N {
        return Err(GuardPinningProofValidationError::HexLength {
            field,
            expected: N,
            found: decoded.len(),
        });
    }
    let mut bytes = [0u8; N];
    bytes.copy_from_slice(&decoded);
    Ok(bytes)
}

/// Guard pinning proof persisted by relays for directory publisher ingestion.
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct GuardPinningProof {
    /// Schema version of the proof.
    version: u8,
    /// UNIX timestamp when the proof was recorded.
    recorded_at_unix: i64,
    /// Path to the guard directory snapshot this proof references.
    snapshot_path: String,
    /// Validation phase label used during verification.
    validation_phase: String,
    /// Relay identifier (hex).
    relay_id_hex: String,
    /// Directory hash (hex) advertised in the snapshot.
    directory_hash_hex: String,
    /// Descriptor commit (hex) pinned by the relay.
    descriptor_commit_hex: String,
    /// Issuer fingerprint (hex) associated with the certificate.
    issuer_fingerprint_hex: String,
    /// Certificate validity window start (UNIX seconds).
    valid_after_unix: i64,
    /// Certificate validity window end (UNIX seconds).
    valid_until_unix: i64,
    /// Guard weight advertised in the certificate.
    guard_weight: u32,
    /// Bandwidth allowance in bytes/sec.
    bandwidth_bytes_per_sec: u64,
    /// Reputation weight advertised in the certificate.
    reputation_weight: u32,
    /// ML-KEM public key (hex) advertised by the relay certificate.
    pq_kem_public_hex: String,
}

impl GuardPinningProof {
    /// Returns the on-disk path to the snapshot this proof references.
    pub fn snapshot_path(&self) -> &str {
        &self.snapshot_path
    }

    /// Relay identifier encoded as lowercase hex.
    pub fn relay_id_hex(&self) -> &str {
        &self.relay_id_hex
    }

    /// Directory hash advertised by this proof.
    pub fn directory_hash_hex(&self) -> &str {
        &self.directory_hash_hex
    }

    /// UNIX timestamp recorded when the proof was generated.
    pub fn recorded_at_unix(&self) -> i64 {
        self.recorded_at_unix
    }

    /// Validation phase recorded by the relay when the proof was created.
    pub fn validation_phase(&self) -> &str {
        &self.validation_phase
    }

    /// Descriptor commit hex encoded string.
    pub fn descriptor_commit_hex(&self) -> &str {
        &self.descriptor_commit_hex
    }

    /// Issuer fingerprint hex encoded string.
    pub fn issuer_fingerprint_hex(&self) -> &str {
        &self.issuer_fingerprint_hex
    }

    /// Relay PQ KEM public key recorded by the proof.
    pub fn pq_kem_public_hex(&self) -> &str {
        &self.pq_kem_public_hex
    }

    /// Valid-after timestamp advertised by the certificate.
    pub fn valid_after_unix(&self) -> i64 {
        self.valid_after_unix
    }

    /// Valid-until timestamp advertised by the certificate.
    pub fn valid_until_unix(&self) -> i64 {
        self.valid_until_unix
    }

    /// Guard weight advertised in the proof.
    pub fn guard_weight(&self) -> u32 {
        self.guard_weight
    }

    /// Bandwidth weight advertised in the proof.
    pub fn bandwidth_bytes_per_sec(&self) -> u64 {
        self.bandwidth_bytes_per_sec
    }

    /// Reputation weight advertised in the proof.
    pub fn reputation_weight(&self) -> u32 {
        self.reputation_weight
    }
}

impl From<GuardDirectoryError> for ConfigError {
    fn from(err: GuardDirectoryError) -> Self {
        ConfigError::GuardDirectory(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use iroha_crypto::soranet::{
        certificate::{
            CapabilityToggle, RelayCapabilityFlagsV1, RelayCertificateV2, RelayEndpointV2,
            RelayRolesV2,
        },
        directory::{
            GUARD_DIRECTORY_VERSION_V2, GuardDirectoryIssuerV1, GuardDirectoryRelayEntryV2,
            compute_issuer_fingerprint, encode_validation_phase,
        },
        handshake::HandshakeSuite,
    };
    use rand::{RngCore, SeedableRng, rngs::StdRng};
    use soranet_pq::{MlDsaSuite, generate_mldsa_keypair};
    use tempfile::NamedTempFile;

    use super::*;
    use crate::config::GuardDirectoryConfig;

    struct SnapshotFixture {
        config: GuardDirectoryConfig,
        relay_id: [u8; 32],
        descriptor_commit: [u8; 32],
        _temp: NamedTempFile,
    }

    #[test]
    fn load_guard_entry_validates_snapshot() {
        let fixture = snapshot_fixture();
        let entry = load_guard_entry(
            &fixture.config,
            &fixture.relay_id,
            &fixture.descriptor_commit,
        )
        .expect("guard snapshot validates");
        assert_eq!(
            entry.bundle.certificate.descriptor_commit,
            fixture.descriptor_commit
        );
    }

    #[test]
    fn load_guard_entry_detects_descriptor_mismatch() {
        let fixture = snapshot_fixture();
        let mut wrong_commit = fixture.descriptor_commit;
        wrong_commit[0] ^= 0xFF;
        let err = load_guard_entry(&fixture.config, &fixture.relay_id, &wrong_commit)
            .expect_err("mismatched descriptor commit should fail validation");
        matches!(err, GuardDirectoryError::DescriptorCommitMismatch { .. });
    }

    #[test]
    fn persist_guard_pinning_proof_serializes_metadata() {
        use std::{fs, time::Duration};

        let fixture = snapshot_fixture();
        let entry = load_guard_entry(
            &fixture.config,
            &fixture.relay_id,
            &fixture.descriptor_commit,
        )
        .expect("guard snapshot validates");
        let dir = tempfile::tempdir().expect("temp dir");
        let proof_path = dir.path().join("guard_proof.json");
        persist_guard_pinning_proof(
            &proof_path,
            fixture.config.snapshot_path(),
            &entry,
            &fixture.relay_id,
            SystemTime::UNIX_EPOCH + Duration::from_secs(42),
        )
        .expect("proof serializes");
        let contents = fs::read_to_string(&proof_path).expect("proof contents");
        let value: json::Value = json::from_str(&contents).expect("json proof");
        assert_eq!(
            value
                .get("relay_id_hex")
                .and_then(json::Value::as_str)
                .map(|s| s.to_string()),
            Some(hex_encode(fixture.relay_id)),
        );
        assert_eq!(
            value
                .get("descriptor_commit_hex")
                .and_then(json::Value::as_str)
                .map(|s| s.to_string()),
            Some(hex_encode(fixture.descriptor_commit))
        );
        assert_eq!(
            value.get("recorded_at_unix").and_then(json::Value::as_i64),
            Some(42)
        );
    }

    #[test]
    fn verify_guard_pinning_proof_accepts_valid_metadata() {
        use std::{fs, time::Duration};

        let fixture = snapshot_fixture();
        let entry = load_guard_entry(
            &fixture.config,
            &fixture.relay_id,
            &fixture.descriptor_commit,
        )
        .expect("guard snapshot validates");
        let dir = tempfile::tempdir().expect("temp dir");
        let proof_path = dir.path().join("guard_proof.json");
        persist_guard_pinning_proof(
            &proof_path,
            fixture.config.snapshot_path(),
            &entry,
            &fixture.relay_id,
            SystemTime::UNIX_EPOCH + Duration::from_secs(7),
        )
        .expect("proof serializes");
        let proof_bytes = fs::read(&proof_path).expect("load proof");
        let proof: GuardPinningProof = json::from_slice(&proof_bytes).expect("decode guard proof");
        let snapshot_bytes = fs::read(fixture.config.snapshot_path()).expect("snapshot contents");
        let snapshot =
            GuardDirectorySnapshotV2::from_bytes(&snapshot_bytes).expect("snapshot decodes");
        verify_guard_pinning_proof(&snapshot, &proof).expect("proof validates");
    }

    #[test]
    fn guard_pinning_proof_accessors_surface_metadata() {
        use std::{fs, time::Duration};

        let fixture = snapshot_fixture();
        let entry = load_guard_entry(
            &fixture.config,
            &fixture.relay_id,
            &fixture.descriptor_commit,
        )
        .expect("guard snapshot validates");
        let dir = tempfile::tempdir().expect("temp dir");
        let proof_path = dir.path().join("guard_proof.json");
        persist_guard_pinning_proof(
            &proof_path,
            fixture.config.snapshot_path(),
            &entry,
            &fixture.relay_id,
            SystemTime::UNIX_EPOCH + Duration::from_secs(7),
        )
        .expect("proof serializes");
        let proof_bytes = fs::read(&proof_path).expect("load proof");
        let proof: GuardPinningProof = json::from_slice(&proof_bytes).expect("decode guard proof");

        let expected_snapshot = fixture
            .config
            .snapshot_path()
            .to_string_lossy()
            .into_owned();
        assert_eq!(proof.snapshot_path(), expected_snapshot);
        assert_eq!(proof.relay_id_hex(), hex_encode(fixture.relay_id));
        assert_eq!(proof.directory_hash_hex(), hex_encode(entry.directory_hash));
        assert_eq!(proof.recorded_at_unix(), 7);
    }

    #[test]
    fn verify_guard_pinning_proof_detects_descriptor_mismatch() {
        use std::{fs, time::Duration};

        let fixture = snapshot_fixture();
        let entry = load_guard_entry(
            &fixture.config,
            &fixture.relay_id,
            &fixture.descriptor_commit,
        )
        .expect("guard snapshot validates");
        let dir = tempfile::tempdir().expect("temp dir");
        let proof_path = dir.path().join("guard_proof.json");
        persist_guard_pinning_proof(
            &proof_path,
            fixture.config.snapshot_path(),
            &entry,
            &fixture.relay_id,
            SystemTime::UNIX_EPOCH + Duration::from_secs(7),
        )
        .expect("proof serializes");
        let proof_bytes = fs::read(&proof_path).expect("load proof");
        let mut proof: GuardPinningProof =
            json::from_slice(&proof_bytes).expect("decode guard proof");
        proof.descriptor_commit_hex = "00".repeat(32);
        let snapshot_bytes = fs::read(fixture.config.snapshot_path()).expect("snapshot contents");
        let snapshot =
            GuardDirectorySnapshotV2::from_bytes(&snapshot_bytes).expect("snapshot decodes");
        let err = verify_guard_pinning_proof(&snapshot, &proof)
            .expect_err("descriptor mismatch must be rejected");
        matches!(
            err,
            GuardPinningProofValidationError::DescriptorCommitMismatch { .. }
        );
    }

    fn snapshot_fixture() -> SnapshotFixture {
        let mut rng = StdRng::seed_from_u64(7);
        let issuer_seed = {
            let mut bytes = [0u8; 32];
            rng.fill_bytes(&mut bytes);
            bytes
        };
        let issuer_signing = SigningKey::from_bytes(&issuer_seed);
        let issuer_public = issuer_signing.verifying_key().to_bytes();
        let issuer_mldsa = generate_mldsa_keypair(MlDsaSuite::MlDsa65).expect("issuer keypair");
        let issuer_fingerprint =
            compute_issuer_fingerprint(&issuer_public, issuer_mldsa.public_key());

        let relay_id = issuer_public;
        let descriptor_commit = [0xAB; 32];
        let bundle = build_bundle(
            relay_id,
            descriptor_commit,
            issuer_fingerprint,
            &issuer_signing,
            issuer_mldsa.secret_key(),
        );

        let snapshot = GuardDirectorySnapshotV2 {
            version: GUARD_DIRECTORY_VERSION_V2,
            directory_hash: [0x44; 32],
            published_at_unix: 1_734_000_000,
            valid_after_unix: 1_734_000_000,
            valid_until_unix: 1_734_086_400,
            validation_phase: encode_validation_phase(CertificateValidationPhase::Phase2PreferDual),
            issuers: vec![GuardDirectoryIssuerV1 {
                fingerprint: issuer_fingerprint,
                ed25519_public: issuer_public,
                mldsa65_public: issuer_mldsa.public_key().to_vec(),
            }],
            relays: vec![GuardDirectoryRelayEntryV2 {
                certificate: bundle.to_cbor(),
            }],
        };
        let bytes = snapshot
            .to_bytes()
            .expect("guard directory snapshot encodes");
        let mut temp = NamedTempFile::new().expect("temp file");
        temp.write_all(&bytes).expect("write snapshot");
        let config = GuardDirectoryConfig {
            snapshot_path: temp.path().to_path_buf(),
            expected_directory_hash_hex: Some(hex_encode(snapshot.directory_hash)),
            allow_missing_entry: false,
            pinning_proof_path: None,
        };

        SnapshotFixture {
            config,
            relay_id,
            descriptor_commit,
            _temp: temp,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_bundle(
        relay_id: [u8; 32],
        descriptor_commit: [u8; 32],
        issuer_fingerprint: [u8; 32],
        issuer_signing: &SigningKey,
        issuer_mldsa_secret: &[u8],
    ) -> RelayCertificateBundleV2 {
        let certificate = RelayCertificateV2 {
            relay_id,
            identity_ed25519: relay_id,
            identity_mldsa65: vec![0x33; 1952],
            descriptor_commit,
            roles: RelayRolesV2 {
                entry: true,
                middle: true,
                exit: false,
            },
            guard_weight: 200,
            bandwidth_bytes_per_sec: 1_000_000,
            reputation_weight: 42,
            endpoints: vec![RelayEndpointV2 {
                url: "soranet://relay.test:443".into(),
                priority: 1,
                tags: vec!["norito-stream".into()],
            }],
            capability_flags: RelayCapabilityFlagsV1::new(
                CapabilityToggle::Enabled,
                CapabilityToggle::Enabled,
                CapabilityToggle::Enabled,
                CapabilityToggle::Disabled,
            ),
            kem_policy: iroha_crypto::soranet::certificate::KemRotationPolicyV1 {
                mode: iroha_crypto::soranet::certificate::KemRotationModeV1::Static,
                preferred_suite: 1,
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
            directory_hash: [0x44; 32],
            issuer_fingerprint,
            pq_kem_public: vec![0x55; 1_184],
        };

        certificate
            .issue(issuer_signing, issuer_mldsa_secret)
            .expect("issue certificate")
    }
}
