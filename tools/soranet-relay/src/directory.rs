//! Guard directory snapshot tooling (builder, rotation, inspection).
#![allow(unexpected_cfgs)]

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use ed25519_dalek::{SigningKey, VerifyingKey};
use hex::FromHexError;
use iroha_crypto::soranet::{
    certificate::{CertificateError, CertificateValidationPhase, RelayCertificateBundleV2},
    directory::{
        GUARD_DIRECTORY_VERSION_V2, GuardDirectoryIssuerV1, GuardDirectoryRelayEntryV2,
        GuardDirectorySnapshotV2, compute_issuer_fingerprint, decode_validation_phase,
        encode_validation_phase,
    },
};
use norito::json::{self, JsonDeserialize, JsonSerialize};
use rand::{CryptoRng, RngCore, SeedableRng, rngs::StdRng};
use soranet_pq::{MlDsaError, MlDsaKeyPair, MlDsaSuite, generate_mldsa_keypair};
use thiserror::Error;

use crate::guard::{
    GuardPinningProof, GuardPinningProofValidationError, verify_guard_pinning_proof,
};

/// Configuration used to build a directory snapshot.
#[derive(Debug, JsonDeserialize, JsonSerialize)]
pub struct DirectoryBuildConfig {
    /// Optional expected directory hash to embed in the snapshot.
    #[norito(default)]
    pub directory_hash_hex: Option<String>,
    /// Optional publication timestamp (UNIX seconds) to include.
    #[norito(default)]
    pub published_at_unix: Option<i64>,
    /// Optional validity window start (UNIX seconds).
    #[norito(default)]
    pub valid_after_unix: Option<i64>,
    /// Optional validity window end (UNIX seconds).
    #[norito(default)]
    pub valid_until_unix: Option<i64>,
    /// Optional validation phase label (phase1_allow_single|phase2_prefer_dual|phase3_require_dual).
    #[norito(default)]
    pub validation_phase: Option<String>,
    /// Issuer public keys included in the snapshot.
    pub issuers: Vec<IssuerConfig>,
    /// Certificate bundles included in the snapshot.
    pub bundles: Vec<BundleConfig>,
    /// Directory containing guard pinning proofs to ingest.
    #[norito(default)]
    pub guard_pinning_proofs_dir: Option<PathBuf>,
    /// Explicit guard pinning proofs to ingest.
    #[norito(default)]
    pub guard_pinning_proofs: Vec<PinningProofConfig>,
}

/// Optional build-time overrides for directory snapshots.
#[derive(Debug, Default)]
pub struct DirectoryBuildOptions<'a> {
    /// Directory containing guard pinning proofs to ingest.
    pub guard_pinning_proofs_dir: Option<&'a Path>,
}

/// Issuer entry supplied by the directory configuration.
#[derive(Debug, JsonDeserialize, JsonSerialize)]
pub struct IssuerConfig {
    /// Optional label rendered in summaries.
    #[norito(default)]
    pub label: Option<String>,
    /// Hex-encoded Ed25519 public key.
    pub ed25519_hex: String,
    /// Optional hex-encoded ML-DSA public key.
    #[norito(default)]
    pub mldsa_hex: Option<String>,
}

/// Certificate bundle path supplied by the configuration.
#[derive(Debug, JsonDeserialize, JsonSerialize)]
pub struct BundleConfig {
    /// Path to the Norito-encoded certificate bundle.
    pub path: PathBuf,
}

/// Guard pinning proof supplied by the configuration.
#[derive(Debug, JsonDeserialize, JsonSerialize)]
pub struct PinningProofConfig {
    /// Path to the proof file (JSON).
    pub path: PathBuf,
}

/// Snapshot plus extracted metadata.
#[derive(Debug)]
pub struct DirectorySnapshotBundle {
    pub snapshot: GuardDirectorySnapshotV2,
    pub metadata: DirectoryMetadata,
}

/// Snapshot metadata exposed by the tooling.
#[derive(Debug, Clone)]
pub struct DirectoryMetadata {
    /// Directory hash advertised in the snapshot.
    pub directory_hash_hex: String,
    /// Publication timestamp in UNIX seconds.
    pub published_at_unix: i64,
    /// Validity window start in UNIX seconds.
    pub valid_after_unix: i64,
    /// Validity window end in UNIX seconds.
    pub valid_until_unix: i64,
    /// Validation phase enforced by the snapshot.
    pub validation_phase: CertificateValidationPhase,
    /// Issuers embedded in the snapshot.
    pub issuers: Vec<IssuerSummary>,
    /// Certificates embedded in the snapshot.
    pub certificates: Vec<CertificateSummary>,
    /// Guard pinning proofs bundled alongside the snapshot.
    pub guard_pinning_proofs: Vec<GuardPinningProofSummary>,
}

/// Issuer summary rendered in CLI output.
#[derive(Debug, Clone)]
pub struct IssuerSummary {
    /// Optional label for display.
    pub label: Option<String>,
    /// Hex-encoded issuer fingerprint.
    pub fingerprint_hex: String,
    /// Hex-encoded Ed25519 public key.
    pub ed25519_hex: String,
    /// Whether an ML-DSA key was provided.
    pub has_mldsa: bool,
}

/// Certificate summary rendered in CLI output.
#[derive(Debug, Clone)]
pub struct CertificateSummary {
    /// Filesystem path where the certificate was loaded from (if any).
    pub path: Option<PathBuf>,
    /// Hex-encoded relay identifier.
    pub relay_id_hex: String,
    /// Guard weight advertised by the certificate.
    pub guard_weight: u32,
    /// Bandwidth allowance in bytes/sec.
    pub bandwidth_bytes_per_sec: u64,
    /// Reputation weight advertised by the certificate.
    pub reputation_weight: u32,
    /// Certificate validity window start in UNIX seconds.
    pub valid_after: i64,
    /// Certificate validity window end in UNIX seconds.
    pub valid_until: i64,
}

/// Guard pinning proof summary rendered in CLI output.
#[derive(Debug, Clone, JsonSerialize)]
pub struct GuardPinningProofSummary {
    /// Filesystem path of the proof.
    pub path: PathBuf,
    /// Hex-encoded relay identifier pinned by the proof.
    pub relay_id_hex: String,
    /// Hex-encoded directory hash pinned by the proof.
    pub directory_hash_hex: String,
    /// Hex-encoded descriptor commit pinned by the proof.
    pub descriptor_commit_hex: String,
    /// Hex-encoded issuer fingerprint associated with the proof.
    pub issuer_fingerprint_hex: String,
    /// Hex-encoded ML-KEM public key referenced by the proof.
    pub pq_kem_public_hex: String,
    /// Validation phase label recorded in the proof.
    pub validation_phase: String,
    /// Timestamp when the proof was recorded (UNIX seconds).
    pub recorded_at_unix: i64,
    /// Certificate validity window start (UNIX seconds).
    pub valid_after_unix: i64,
    /// Certificate validity window end (UNIX seconds).
    pub valid_until_unix: i64,
    /// Guard weight advertised by the certificate.
    pub guard_weight: u32,
    /// Bandwidth allowance in bytes/sec.
    pub bandwidth_bytes_per_sec: u64,
    /// Reputation weight advertised by the certificate.
    pub reputation_weight: u32,
}

/// Errors raised while collecting guard pinning proofs for directory publishers.
#[derive(Debug, Error)]
pub enum GuardPinningCollectError {
    #[error("guard pinning proof directory `{path}` does not exist or is not a directory")]
    NotDirectory { path: PathBuf },
    #[error("failed to list guard pinning proofs under `{path}`: {source}")]
    ReadDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to inspect guard pinning proof candidate `{path}`: {source}")]
    EntryIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to canonicalize guard pinning proof `{path}`: {source}")]
    Canonicalize {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("no guard pinning proofs were found under `{path}`")]
    NoProofs { path: PathBuf },
    #[error(transparent)]
    Build(#[from] Box<DirectoryBuildError>),
}

/// Error raised while building a snapshot from configuration.
#[derive(Debug, Error)]
pub enum DirectoryBuildError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse JSON config {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: json::Error,
    },
    #[error("no issuers supplied in configuration")]
    NoIssuers,
    #[error("no certificate bundles supplied in configuration")]
    NoBundles,
    #[error(
        "invalid validation phase `{value}` (expected phase1_allow_single, phase2_prefer_dual, or phase3_require_dual)"
    )]
    InvalidPhase { value: String },
    #[error("invalid hex in {field}: {source}")]
    Hex {
        field: String,
        #[source]
        source: FromHexError,
    },
    #[error("{field} must decode to {expected} bytes (got {found})")]
    InvalidHexLength {
        field: String,
        expected: usize,
        found: usize,
    },
    #[error("issuer {label} missing ML-DSA public key for validation phase {phase:?}")]
    IssuerMissingMlDsa {
        label: String,
        phase: CertificateValidationPhase,
    },
    #[error("duplicate issuer fingerprint {fingerprint}")]
    DuplicateIssuer { fingerprint: String },
    #[error("issuer {label} contained an invalid Ed25519 public key: {source}")]
    InvalidIssuerEd25519 {
        label: String,
        #[source]
        source: ed25519_dalek::SignatureError,
    },
    #[error("certificate references unknown issuer {fingerprint} ({path})")]
    UnknownIssuerForCertificate { fingerprint: String, path: PathBuf },
    #[error("certificate verification failed for {path}: {source}")]
    CertificateVerify {
        path: PathBuf,
        #[source]
        source: CertificateError,
    },
    #[error("certificate directory hash mismatch in {path}: expected {expected}, got {found}")]
    DirectoryHashMismatch {
        path: PathBuf,
        expected: String,
        found: String,
    },
    #[error("certificate {field} mismatch in {path}: expected {expected}, got {found}")]
    CertificateFieldMismatch {
        path: PathBuf,
        field: &'static str,
        expected: i64,
        found: i64,
    },
    #[error("failed to read guard pinning proof {path}: {source}")]
    GuardPinningProofIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to decode guard pinning proof {path}: {source}")]
    GuardPinningProofDecode {
        path: PathBuf,
        #[source]
        source: json::Error,
    },
    #[error("guard pinning proof validation failed for {path}: {source}")]
    GuardPinningProofValidation {
        path: PathBuf,
        #[source]
        source: GuardPinningProofValidationError,
    },
    #[error("duplicate guard pinning proof for relay {relay_id_hex} ({path})")]
    DuplicateGuardPinningProof { relay_id_hex: String, path: PathBuf },
    #[error("failed to collect guard pinning proofs from {path}: {source}")]
    GuardPinningCollect {
        path: PathBuf,
        #[source]
        source: Box<GuardPinningCollectError>,
    },
    #[error("failed to derive {field} from supplied certificates")]
    MissingDerivedField { field: &'static str },
}

/// Error raised while rotating issuer keys.
#[derive(Debug, Error)]
pub enum DirectoryRotateError {
    #[error("failed to decode guard directory snapshot: {source}")]
    Decode {
        #[source]
        source: norito::Error,
    },
    #[error("snapshot missing issuer records")]
    NoIssuers,
    #[error("rotation currently supports single-issuer snapshots (found {found})")]
    MultipleIssuers { found: usize },
    #[error("snapshot validation phase {phase} is not recognised")]
    UnknownPhase { phase: u8 },
    #[error("snapshot contained no relay certificates to rotate")]
    NoCertificates,
    #[error("issuer public key invalid: {source}")]
    InvalidIssuerKey {
        #[source]
        source: ed25519_dalek::SignatureError,
    },
    #[error("certificate decode failed at index {index}: {source}")]
    CertificateDecode {
        index: usize,
        #[source]
        source: CertificateError,
    },
    #[error("certificate verification failed at index {index}: {source}")]
    CertificateVerify {
        index: usize,
        #[source]
        source: CertificateError,
    },
    #[error("failed to reissue certificate at index {index}: {source}")]
    CertificateReissue {
        index: usize,
        #[source]
        source: CertificateError,
    },
    #[error("failed to generate {suite:?} keypair: {source}")]
    KeyGeneration {
        suite: MlDsaSuite,
        #[source]
        source: MlDsaError,
    },
}

/// Result returned when rotating issuer material.
#[derive(Debug)]
pub struct RotationOutput {
    pub bundle: DirectorySnapshotBundle,
    pub keys: RotationKeys,
}

/// Key material produced during rotation.
#[derive(Debug, Clone)]
pub struct RotationKeys {
    pub ed25519_secret: [u8; 32],
    pub ed25519_public: [u8; 32],
    pub mldsa_public: Vec<u8>,
    pub mldsa_secret: Vec<u8>,
    pub fingerprint: [u8; 32],
}

/// Build a guard directory snapshot from configuration.
///
/// # Errors
/// Returns [`DirectoryBuildError`] when configuration parsing, certificate verification,
/// or metadata reconciliation fails.
pub fn build_snapshot_from_config(
    path: &Path,
) -> Result<DirectorySnapshotBundle, DirectoryBuildError> {
    build_snapshot_from_config_with_options(path, DirectoryBuildOptions::default())
}

/// Build a guard directory snapshot from configuration with optional overrides.
///
/// # Errors
/// Returns [`DirectoryBuildError`] when configuration parsing, certificate verification,
/// or metadata reconciliation fails.
pub fn build_snapshot_from_config_with_options(
    path: &Path,
    options: DirectoryBuildOptions<'_>,
) -> Result<DirectorySnapshotBundle, DirectoryBuildError> {
    let bytes = fs::read(path).map_err(|source| DirectoryBuildError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut config: DirectoryBuildConfig =
        json::from_slice(&bytes).map_err(|source| DirectoryBuildError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    if let Some(dir) = options.guard_pinning_proofs_dir {
        config.guard_pinning_proofs_dir = Some(dir.to_path_buf());
    }
    build_snapshot(config, path.parent().unwrap_or_else(|| Path::new(".")))
}

/// Rotate issuer material for an existing snapshot using OS randomness.
///
/// # Errors
/// Returns [`DirectoryRotateError`] when the snapshot fails validation or certificates
/// cannot be reissued.
pub fn rotate_snapshot_with_os_rng(
    snapshot_bytes: &[u8],
) -> Result<RotationOutput, DirectoryRotateError> {
    let mut entropy = rand::rng();
    let mut rng = <StdRng as SeedableRng>::from_rng(&mut entropy);
    rotate_snapshot(snapshot_bytes, &mut rng)
}

/// Rotate issuer material for an existing snapshot with the provided RNG.
///
/// # Errors
/// Returns [`DirectoryRotateError`] when the snapshot fails validation or certificates
/// cannot be reissued.
pub fn rotate_snapshot<R: RngCore + CryptoRng>(
    snapshot_bytes: &[u8],
    rng: &mut R,
) -> Result<RotationOutput, DirectoryRotateError> {
    let snapshot = GuardDirectorySnapshotV2::from_bytes(snapshot_bytes)
        .map_err(|source| DirectoryRotateError::Decode { source })?;
    rotate_snapshot_struct(snapshot, rng)
}

/// Produce metadata for an existing snapshot.
///
/// # Errors
/// Returns [`DirectoryRotateError`] when the snapshot fails to decode or certificates cannot be parsed.
pub fn inspect_snapshot(
    snapshot_bytes: &[u8],
) -> Result<DirectorySnapshotBundle, DirectoryRotateError> {
    let snapshot = GuardDirectorySnapshotV2::from_bytes(snapshot_bytes)
        .map_err(|source| DirectoryRotateError::Decode { source })?;
    let metadata = summarize_snapshot(&snapshot)?;
    Ok(DirectorySnapshotBundle { snapshot, metadata })
}

fn build_snapshot(
    config: DirectoryBuildConfig,
    base_dir: &Path,
) -> Result<DirectorySnapshotBundle, DirectoryBuildError> {
    if config.issuers.is_empty() {
        return Err(DirectoryBuildError::NoIssuers);
    }
    if config.bundles.is_empty() {
        return Err(DirectoryBuildError::NoBundles);
    }

    let validation_phase = parse_validation_phase(config.validation_phase.as_deref())?;
    let issuers = load_issuers(&config.issuers, validation_phase)?;

    let mut issuer_map: HashMap<[u8; 32], LoadedIssuer> = HashMap::new();
    for issuer in &issuers {
        if issuer_map
            .insert(issuer.fingerprint, issuer.clone())
            .is_some()
        {
            return Err(DirectoryBuildError::DuplicateIssuer {
                fingerprint: hex::encode(issuer.fingerprint),
            });
        }
    }

    let mut parsed_bundles: Vec<(PathBuf, RelayCertificateBundleV2)> = Vec::new();
    let mut certificate_summaries: Vec<CertificateSummary> = Vec::new();

    let mut directory_hash = parse_optional_hash(config.directory_hash_hex.as_deref())?;
    let mut published_at = config.published_at_unix;
    let mut valid_after = config.valid_after_unix;
    let mut valid_until = config.valid_until_unix;

    for bundle_config in &config.bundles {
        let absolute_path = if bundle_config.path.is_absolute() {
            bundle_config.path.clone()
        } else {
            base_dir.join(&bundle_config.path)
        };
        let bytes = fs::read(&absolute_path).map_err(|source| DirectoryBuildError::Io {
            path: absolute_path.clone(),
            source,
        })?;
        let bundle = RelayCertificateBundleV2::from_cbor(&bytes).map_err(|source| {
            DirectoryBuildError::CertificateVerify {
                path: absolute_path.clone(),
                source,
            }
        })?;

        let fingerprint = bundle.certificate.issuer_fingerprint;
        let issuer = issuer_map.get(&fingerprint).ok_or_else(|| {
            DirectoryBuildError::UnknownIssuerForCertificate {
                fingerprint: hex::encode(fingerprint),
                path: absolute_path.clone(),
            }
        })?;

        bundle
            .verify(
                &issuer.verifying_key,
                &issuer.mldsa_public,
                validation_phase,
            )
            .map_err(|source| DirectoryBuildError::CertificateVerify {
                path: absolute_path.clone(),
                source,
            })?;

        update_directory_hash(
            &mut directory_hash,
            bundle.certificate.directory_hash,
            &absolute_path,
        )?;
        update_field(
            &mut published_at,
            bundle.certificate.published_at,
            "published_at_unix",
            &absolute_path,
        )?;
        update_field(
            &mut valid_after,
            bundle.certificate.valid_after,
            "valid_after_unix",
            &absolute_path,
        )?;
        update_field(
            &mut valid_until,
            bundle.certificate.valid_until,
            "valid_until_unix",
            &absolute_path,
        )?;

        certificate_summaries.push(CertificateSummary {
            path: Some(absolute_path.clone()),
            relay_id_hex: hex::encode(bundle.certificate.relay_id),
            guard_weight: bundle.certificate.guard_weight,
            bandwidth_bytes_per_sec: bundle.certificate.bandwidth_bytes_per_sec,
            reputation_weight: bundle.certificate.reputation_weight,
            valid_after: bundle.certificate.valid_after,
            valid_until: bundle.certificate.valid_until,
        });
        parsed_bundles.push((absolute_path, bundle));
    }

    let directory_hash = directory_hash.ok_or(DirectoryBuildError::MissingDerivedField {
        field: "directory_hash",
    })?;
    let published_at = published_at.ok_or(DirectoryBuildError::MissingDerivedField {
        field: "published_at_unix",
    })?;
    let valid_after = valid_after.ok_or(DirectoryBuildError::MissingDerivedField {
        field: "valid_after_unix",
    })?;
    let valid_until = valid_until.ok_or(DirectoryBuildError::MissingDerivedField {
        field: "valid_until_unix",
    })?;

    let issuer_list: Vec<GuardDirectoryIssuerV1> = issuers
        .iter()
        .map(|issuer| GuardDirectoryIssuerV1 {
            fingerprint: issuer.fingerprint,
            ed25519_public: issuer.ed25519_bytes,
            mldsa65_public: issuer.mldsa_public.clone(),
        })
        .collect();
    let issuer_summaries: Vec<IssuerSummary> = issuers
        .iter()
        .map(|issuer| IssuerSummary {
            label: issuer.label.clone(),
            fingerprint_hex: hex::encode(issuer.fingerprint),
            ed25519_hex: hex::encode(issuer.ed25519_bytes),
            has_mldsa: !issuer.mldsa_public.is_empty(),
        })
        .collect();

    let relays = parsed_bundles
        .into_iter()
        .map(|(_, bundle)| GuardDirectoryRelayEntryV2 {
            certificate: bundle.to_cbor(),
        })
        .collect::<Vec<_>>();

    let snapshot = GuardDirectorySnapshotV2 {
        version: GUARD_DIRECTORY_VERSION_V2,
        directory_hash,
        published_at_unix: published_at,
        valid_after_unix: valid_after,
        valid_until_unix: valid_until,
        validation_phase: encode_validation_phase(validation_phase),
        issuers: issuer_list,
        relays,
    };

    let mut guard_pinning_proofs =
        load_guard_pinning_proofs(&config.guard_pinning_proofs, base_dir, &snapshot)?;
    if let Some(dir) = config.guard_pinning_proofs_dir.as_ref() {
        let resolved = if dir.is_absolute() {
            dir.clone()
        } else {
            base_dir.join(dir)
        };
        let mut collected = collect_guard_pinning_proofs_from_directory(&resolved, &snapshot)
            .map_err(|source| DirectoryBuildError::GuardPinningCollect {
                path: resolved.clone(),
                source: Box::new(source),
            })?;
        if guard_pinning_proofs.is_empty() {
            guard_pinning_proofs = collected;
        } else {
            let mut seen: HashSet<String> = guard_pinning_proofs
                .iter()
                .map(|proof| proof.relay_id_hex.clone())
                .collect();
            for proof in collected.drain(..) {
                if !seen.insert(proof.relay_id_hex.clone()) {
                    return Err(DirectoryBuildError::DuplicateGuardPinningProof {
                        relay_id_hex: proof.relay_id_hex,
                        path: proof.path.clone(),
                    });
                }
                guard_pinning_proofs.push(proof);
            }
        }
    }

    let metadata = DirectoryMetadata {
        directory_hash_hex: hex::encode(directory_hash),
        published_at_unix: published_at,
        valid_after_unix: valid_after,
        valid_until_unix: valid_until,
        validation_phase,
        issuers: issuer_summaries,
        certificates: certificate_summaries,
        guard_pinning_proofs,
    };

    Ok(DirectorySnapshotBundle { snapshot, metadata })
}

fn rotate_snapshot_struct<R: RngCore + CryptoRng>(
    snapshot: GuardDirectorySnapshotV2,
    rng: &mut R,
) -> Result<RotationOutput, DirectoryRotateError> {
    if snapshot.issuers.is_empty() {
        return Err(DirectoryRotateError::NoIssuers);
    }
    if snapshot.issuers.len() != 1 {
        return Err(DirectoryRotateError::MultipleIssuers {
            found: snapshot.issuers.len(),
        });
    }
    if snapshot.relays.is_empty() {
        return Err(DirectoryRotateError::NoCertificates);
    }
    let validation_phase = decode_validation_phase(snapshot.validation_phase).ok_or(
        DirectoryRotateError::UnknownPhase {
            phase: snapshot.validation_phase,
        },
    )?;

    let issuer = &snapshot.issuers[0];
    let verifying_key = VerifyingKey::from_bytes(&issuer.ed25519_public)
        .map_err(|source| DirectoryRotateError::InvalidIssuerKey { source })?;

    let mut parsed_bundles: Vec<RelayCertificateBundleV2> =
        Vec::with_capacity(snapshot.relays.len());
    let mut certificate_summaries: Vec<CertificateSummary> =
        Vec::with_capacity(snapshot.relays.len());

    for (index, relay_entry) in snapshot.relays.iter().enumerate() {
        let bundle = RelayCertificateBundleV2::from_cbor(&relay_entry.certificate)
            .map_err(|source| DirectoryRotateError::CertificateDecode { index, source })?;
        bundle
            .verify(&verifying_key, &issuer.mldsa65_public, validation_phase)
            .map_err(|source| DirectoryRotateError::CertificateVerify { index, source })?;
        certificate_summaries.push(CertificateSummary {
            path: None,
            relay_id_hex: hex::encode(bundle.certificate.relay_id),
            guard_weight: bundle.certificate.guard_weight,
            bandwidth_bytes_per_sec: bundle.certificate.bandwidth_bytes_per_sec,
            reputation_weight: bundle.certificate.reputation_weight,
            valid_after: bundle.certificate.valid_after,
            valid_until: bundle.certificate.valid_until,
        });
        parsed_bundles.push(bundle);
    }

    let mut ed_seed = [0u8; 32];
    rng.fill_bytes(&mut ed_seed);
    let signing_key = SigningKey::from_bytes(&ed_seed);
    let ed_public = signing_key.verifying_key().to_bytes();

    let mldsa_keys: MlDsaKeyPair =
        generate_mldsa_keypair(MlDsaSuite::MlDsa65).map_err(|source| {
            DirectoryRotateError::KeyGeneration {
                suite: MlDsaSuite::MlDsa65,
                source,
            }
        })?;
    let mldsa_public = mldsa_keys.public_key().to_vec();
    let mldsa_secret = mldsa_keys.secret_key().to_vec();

    let fingerprint = compute_issuer_fingerprint(&ed_public, &mldsa_public);

    let mut relays: Vec<GuardDirectoryRelayEntryV2> = Vec::with_capacity(parsed_bundles.len());
    for (index, bundle) in parsed_bundles.into_iter().enumerate() {
        let mut certificate = bundle.certificate;
        certificate.issuer_fingerprint = fingerprint;
        let reissued = certificate
            .issue(&signing_key, mldsa_keys.secret_key())
            .map_err(|source| DirectoryRotateError::CertificateReissue { index, source })?;
        relays.push(GuardDirectoryRelayEntryV2 {
            certificate: reissued.to_cbor(),
        });
    }

    let snapshot = GuardDirectorySnapshotV2 {
        version: snapshot.version,
        directory_hash: snapshot.directory_hash,
        published_at_unix: snapshot.published_at_unix,
        valid_after_unix: snapshot.valid_after_unix,
        valid_until_unix: snapshot.valid_until_unix,
        validation_phase: snapshot.validation_phase,
        issuers: vec![GuardDirectoryIssuerV1 {
            fingerprint,
            ed25519_public: ed_public,
            mldsa65_public: mldsa_public.clone(),
        }],
        relays,
    };

    let metadata = DirectoryMetadata {
        directory_hash_hex: hex::encode(snapshot.directory_hash),
        published_at_unix: snapshot.published_at_unix,
        valid_after_unix: snapshot.valid_after_unix,
        valid_until_unix: snapshot.valid_until_unix,
        validation_phase,
        issuers: vec![IssuerSummary {
            label: None,
            fingerprint_hex: hex::encode(fingerprint),
            ed25519_hex: hex::encode(ed_public),
            has_mldsa: true,
        }],
        certificates: certificate_summaries,
        guard_pinning_proofs: Vec::new(),
    };

    let keys = RotationKeys {
        ed25519_secret: ed_seed,
        ed25519_public: ed_public,
        mldsa_public,
        mldsa_secret,
        fingerprint,
    };

    Ok(RotationOutput {
        bundle: DirectorySnapshotBundle { snapshot, metadata },
        keys,
    })
}

fn summarize_snapshot(
    snapshot: &GuardDirectorySnapshotV2,
) -> Result<DirectoryMetadata, DirectoryRotateError> {
    let validation_phase = decode_validation_phase(snapshot.validation_phase).ok_or(
        DirectoryRotateError::UnknownPhase {
            phase: snapshot.validation_phase,
        },
    )?;

    let mut issuer_summaries: Vec<IssuerSummary> = Vec::with_capacity(snapshot.issuers.len());
    let mut issuer_records: HashMap<[u8; 32], (VerifyingKey, Vec<u8>)> =
        HashMap::with_capacity(snapshot.issuers.len());

    for issuer in &snapshot.issuers {
        let verifying_key = VerifyingKey::from_bytes(&issuer.ed25519_public)
            .map_err(|source| DirectoryRotateError::InvalidIssuerKey { source })?;
        issuer_records.insert(
            issuer.fingerprint,
            (verifying_key, issuer.mldsa65_public.clone()),
        );
        issuer_summaries.push(IssuerSummary {
            label: None,
            fingerprint_hex: hex::encode(issuer.fingerprint),
            ed25519_hex: hex::encode(issuer.ed25519_public),
            has_mldsa: !issuer.mldsa65_public.is_empty(),
        });
    }

    let mut certificate_summaries: Vec<CertificateSummary> =
        Vec::with_capacity(snapshot.relays.len());

    for (index, relay_entry) in snapshot.relays.iter().enumerate() {
        let bundle = RelayCertificateBundleV2::from_cbor(&relay_entry.certificate)
            .map_err(|source| DirectoryRotateError::CertificateDecode { index, source })?;
        if let Some((issuer_key, mldsa_public)) =
            issuer_records.get(&bundle.certificate.issuer_fingerprint)
        {
            bundle
                .verify(issuer_key, mldsa_public, validation_phase)
                .map_err(|source| DirectoryRotateError::CertificateVerify { index, source })?;
        }
        certificate_summaries.push(CertificateSummary {
            path: None,
            relay_id_hex: hex::encode(bundle.certificate.relay_id),
            guard_weight: bundle.certificate.guard_weight,
            bandwidth_bytes_per_sec: bundle.certificate.bandwidth_bytes_per_sec,
            reputation_weight: bundle.certificate.reputation_weight,
            valid_after: bundle.certificate.valid_after,
            valid_until: bundle.certificate.valid_until,
        });
    }

    Ok(DirectoryMetadata {
        directory_hash_hex: hex::encode(snapshot.directory_hash),
        published_at_unix: snapshot.published_at_unix,
        valid_after_unix: snapshot.valid_after_unix,
        valid_until_unix: snapshot.valid_until_unix,
        validation_phase,
        issuers: issuer_summaries,
        certificates: certificate_summaries,
        guard_pinning_proofs: Vec::new(),
    })
}

#[derive(Clone)]
struct LoadedIssuer {
    label: Option<String>,
    verifying_key: VerifyingKey,
    ed25519_bytes: [u8; 32],
    mldsa_public: Vec<u8>,
    fingerprint: [u8; 32],
}

fn load_issuers(
    configs: &[IssuerConfig],
    phase: CertificateValidationPhase,
) -> Result<Vec<LoadedIssuer>, DirectoryBuildError> {
    let mut loaded = Vec::with_capacity(configs.len());
    for config in configs {
        let ed_bytes = decode_hex_array::<32>(&config.ed25519_hex, "issuers[].ed25519_hex")?;
        let label_display = config
            .label
            .clone()
            .unwrap_or_else(|| "<unknown issuer>".to_string());
        let verifying_key = VerifyingKey::from_bytes(&ed_bytes).map_err(|source| {
            DirectoryBuildError::InvalidIssuerEd25519 {
                label: label_display.clone(),
                source,
            }
        })?;
        let mldsa_public =
            decode_mldsa_bytes(config.mldsa_hex.as_deref(), phase, label_display.clone())?;
        let fingerprint = compute_issuer_fingerprint(&ed_bytes, &mldsa_public);
        loaded.push(LoadedIssuer {
            label: config.label.clone(),
            verifying_key,
            ed25519_bytes: ed_bytes,
            mldsa_public,
            fingerprint,
        });
    }
    Ok(loaded)
}

fn load_guard_pinning_proofs(
    configs: &[PinningProofConfig],
    base_dir: &Path,
    snapshot: &GuardDirectorySnapshotV2,
) -> Result<Vec<GuardPinningProofSummary>, DirectoryBuildError> {
    if configs.is_empty() {
        return Ok(Vec::new());
    }

    let mut summaries = Vec::with_capacity(configs.len());
    let mut seen_relays = HashSet::with_capacity(configs.len());

    for proof_config in configs {
        let absolute_path = if proof_config.path.is_absolute() {
            proof_config.path.clone()
        } else {
            base_dir.join(&proof_config.path)
        };
        let bytes = fs::read(&absolute_path).map_err(|source| {
            DirectoryBuildError::GuardPinningProofIo {
                path: absolute_path.clone(),
                source,
            }
        })?;
        let proof: GuardPinningProof = json::from_slice(&bytes).map_err(|source| {
            DirectoryBuildError::GuardPinningProofDecode {
                path: absolute_path.clone(),
                source,
            }
        })?;
        verify_guard_pinning_proof(snapshot, &proof).map_err(|source| {
            DirectoryBuildError::GuardPinningProofValidation {
                path: absolute_path.clone(),
                source,
            }
        })?;

        let relay_id_hex = proof.relay_id_hex().to_string();
        if !seen_relays.insert(relay_id_hex.clone()) {
            return Err(DirectoryBuildError::DuplicateGuardPinningProof {
                relay_id_hex,
                path: absolute_path.clone(),
            });
        }

        summaries.push(GuardPinningProofSummary::from_proof(absolute_path, &proof));
    }

    Ok(summaries)
}

impl GuardPinningProofSummary {
    fn from_proof(path: PathBuf, proof: &GuardPinningProof) -> Self {
        Self {
            path,
            relay_id_hex: proof.relay_id_hex().to_string(),
            directory_hash_hex: proof.directory_hash_hex().to_string(),
            descriptor_commit_hex: proof.descriptor_commit_hex().to_string(),
            issuer_fingerprint_hex: proof.issuer_fingerprint_hex().to_string(),
            pq_kem_public_hex: proof.pq_kem_public_hex().to_string(),
            validation_phase: proof.validation_phase().to_string(),
            recorded_at_unix: proof.recorded_at_unix(),
            valid_after_unix: proof.valid_after_unix(),
            valid_until_unix: proof.valid_until_unix(),
            guard_weight: proof.guard_weight(),
            bandwidth_bytes_per_sec: proof.bandwidth_bytes_per_sec(),
            reputation_weight: proof.reputation_weight(),
        }
    }
}

/// Collect and validate guard pinning proofs stored in the supplied directory.
///
/// Returns the verified summaries that directory publishers can staple into the
/// governance evidence bundle for a guard snapshot.
///
/// # Errors
/// Returns [`GuardPinningCollectError`] when the directory is missing, no proofs
/// are present, or any proof fails to validate against the snapshot.
pub fn collect_guard_pinning_proofs_from_directory(
    directory: &Path,
    snapshot: &GuardDirectorySnapshotV2,
) -> Result<Vec<GuardPinningProofSummary>, GuardPinningCollectError> {
    if !directory.is_dir() {
        return Err(GuardPinningCollectError::NotDirectory {
            path: directory.to_path_buf(),
        });
    }

    let mut configs = Vec::new();
    let entries = fs::read_dir(directory).map_err(|source| GuardPinningCollectError::ReadDir {
        path: directory.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| GuardPinningCollectError::EntryIo {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|source| GuardPinningCollectError::EntryIo {
                path: path.clone(),
                source,
            })?;
        if !metadata.is_file() {
            continue;
        }
        if !path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("json"))
            .unwrap_or(false)
        {
            continue;
        }
        let absolute_path =
            path.canonicalize()
                .map_err(|source| GuardPinningCollectError::Canonicalize {
                    path: path.clone(),
                    source,
                })?;
        configs.push(PinningProofConfig {
            path: absolute_path,
        });
    }

    if configs.is_empty() {
        return Err(GuardPinningCollectError::NoProofs {
            path: directory.to_path_buf(),
        });
    }

    configs.sort_by(|a, b| a.path.cmp(&b.path));
    load_guard_pinning_proofs(&configs, Path::new(""), snapshot)
        .map_err(|source| GuardPinningCollectError::Build(Box::new(source)))
}

fn decode_mldsa_bytes(
    value: Option<&str>,
    phase: CertificateValidationPhase,
    label: String,
) -> Result<Vec<u8>, DirectoryBuildError> {
    match value {
        Some(hex_value) => hex::decode(hex_value).map_err(|source| DirectoryBuildError::Hex {
            field: "issuers[].mldsa_hex".to_string(),
            source,
        }),
        None => {
            if matches!(
                phase,
                CertificateValidationPhase::Phase2PreferDual
                    | CertificateValidationPhase::Phase3RequireDual
            ) {
                Err(DirectoryBuildError::IssuerMissingMlDsa { label, phase })
            } else {
                Ok(Vec::new())
            }
        }
    }
}

fn parse_validation_phase(
    value: Option<&str>,
) -> Result<CertificateValidationPhase, DirectoryBuildError> {
    let Some(raw) = value else {
        return Ok(CertificateValidationPhase::Phase2PreferDual);
    };
    match raw.to_ascii_lowercase().as_str() {
        "phase1_allow_single" | "phase1" | "allow_single" => {
            Ok(CertificateValidationPhase::Phase1AllowSingle)
        }
        "phase2_prefer_dual" | "phase2" | "prefer_dual" => {
            Ok(CertificateValidationPhase::Phase2PreferDual)
        }
        "phase3_require_dual" | "phase3" | "require_dual" => {
            Ok(CertificateValidationPhase::Phase3RequireDual)
        }
        other => Err(DirectoryBuildError::InvalidPhase {
            value: other.to_string(),
        }),
    }
}

fn parse_optional_hash(value: Option<&str>) -> Result<Option<[u8; 32]>, DirectoryBuildError> {
    value
        .map(|hex_value| decode_hex_array::<32>(hex_value, "directory_hash_hex"))
        .transpose()
}

fn decode_hex_array<const N: usize>(
    value: &str,
    field: &str,
) -> Result<[u8; N], DirectoryBuildError> {
    let bytes = hex::decode(value).map_err(|source| DirectoryBuildError::Hex {
        field: field.to_string(),
        source,
    })?;
    if bytes.len() != N {
        return Err(DirectoryBuildError::InvalidHexLength {
            field: field.to_string(),
            expected: N,
            found: bytes.len(),
        });
    }
    let mut array = [0u8; N];
    array.copy_from_slice(&bytes);
    Ok(array)
}

fn update_directory_hash(
    current: &mut Option<[u8; 32]>,
    candidate: [u8; 32],
    path: &Path,
) -> Result<(), DirectoryBuildError> {
    match current {
        Some(expected) => {
            if expected != &candidate {
                return Err(DirectoryBuildError::DirectoryHashMismatch {
                    path: path.to_path_buf(),
                    expected: hex::encode(expected),
                    found: hex::encode(candidate),
                });
            }
            Ok(())
        }
        None => {
            *current = Some(candidate);
            Ok(())
        }
    }
}

fn update_field(
    current: &mut Option<i64>,
    candidate: i64,
    field: &'static str,
    path: &Path,
) -> Result<(), DirectoryBuildError> {
    match current {
        Some(expected) => {
            if *expected != candidate {
                return Err(DirectoryBuildError::CertificateFieldMismatch {
                    path: path.to_path_buf(),
                    field,
                    expected: *expected,
                    found: candidate,
                });
            }
            Ok(())
        }
        None => {
            *current = Some(candidate);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use iroha_crypto::soranet::{
        certificate::{
            CapabilityToggle, KemRotationModeV1, KemRotationPolicyV1, RelayCapabilityFlagsV1,
            RelayCertificateV2, RelayEndpointV2, RelayRolesV2,
        },
        handshake::HandshakeSuite,
    };
    use rand::{SeedableRng, rngs::StdRng};
    use soranet_pq::generate_mldsa_keypair;
    use tempfile::tempdir;

    use super::*;
    use crate::guard::{GuardDirectoryEntry, persist_guard_pinning_proof};

    #[test]
    fn build_snapshot_from_config_roundtrip() {
        let issuer_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");
        let mut rng = StdRng::seed_from_u64(0xA55A55);
        let mut ed_seed = [0u8; 32];
        rng.fill_bytes(&mut ed_seed);
        let signing_key = SigningKey::from_bytes(&ed_seed);
        let ed_public = signing_key.verifying_key().to_bytes();
        let fingerprint = compute_issuer_fingerprint(&ed_public, issuer_keys.public_key());

        let certificate = sample_certificate(fingerprint);
        let bundle = certificate
            .clone()
            .issue(&signing_key, issuer_keys.secret_key())
            .expect("issue");
        let dir = tempdir().expect("tempdir");
        let bundle_path = dir.path().join("alpha.cbor");
        fs::write(&bundle_path, bundle.to_cbor()).expect("write bundle");

        let config_path = dir.path().join("directory.json");
        let config = DirectoryBuildConfig {
            directory_hash_hex: Some(hex::encode(bundle.certificate.directory_hash)),
            published_at_unix: Some(bundle.certificate.published_at),
            valid_after_unix: Some(bundle.certificate.valid_after),
            valid_until_unix: Some(bundle.certificate.valid_until),
            validation_phase: Some("phase3_require_dual".to_string()),
            issuers: vec![IssuerConfig {
                label: Some("governance".to_string()),
                ed25519_hex: hex::encode(ed_public),
                mldsa_hex: Some(hex::encode(issuer_keys.public_key())),
            }],
            bundles: vec![BundleConfig {
                path: bundle_path.clone(),
            }],
            guard_pinning_proofs_dir: None,
            guard_pinning_proofs: Vec::new(),
        };
        write_directory_config(&config_path, &config);

        let bundle = build_snapshot_from_config(&config_path).expect("build snapshot");
        assert_eq!(bundle.snapshot.directory_hash, certificate.directory_hash);
        assert_eq!(bundle.metadata.certificates.len(), 1);
        assert_eq!(bundle.metadata.issuers.len(), 1);
        assert_eq!(
            bundle.metadata.validation_phase,
            CertificateValidationPhase::Phase3RequireDual
        );
        assert_eq!(
            bundle.metadata.certificates[0].relay_id_hex,
            hex::encode(certificate.relay_id)
        );
        assert_eq!(
            bundle.metadata.issuers[0].fingerprint_hex,
            hex::encode(fingerprint)
        );
    }

    #[test]
    fn build_snapshot_ingests_guard_pinning_proofs() {
        let issuer_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");
        let mut rng = StdRng::seed_from_u64(0xBAD5EED);
        let mut ed_seed = [0u8; 32];
        rng.fill_bytes(&mut ed_seed);
        let signing_key = SigningKey::from_bytes(&ed_seed);
        let ed_public = signing_key.verifying_key().to_bytes();
        let fingerprint = compute_issuer_fingerprint(&ed_public, issuer_keys.public_key());

        let certificate = sample_certificate(fingerprint);
        let bundle = certificate
            .clone()
            .issue(&signing_key, issuer_keys.secret_key())
            .expect("issue");

        let dir = tempdir().expect("tempdir");
        let bundle_path = dir.path().join("relay.cbor");
        fs::write(&bundle_path, bundle.to_cbor()).expect("write bundle");

        let config_path = dir.path().join("directory.json");
        let snapshot_path = dir.path().join("snapshots/current.norito");
        let proof_rel_path = PathBuf::from("evidence/entry.json");
        let proof_abs_path = dir.path().join(&proof_rel_path);

        let mut config = DirectoryBuildConfig {
            directory_hash_hex: Some(hex::encode(bundle.certificate.directory_hash)),
            published_at_unix: Some(bundle.certificate.published_at),
            valid_after_unix: Some(bundle.certificate.valid_after),
            valid_until_unix: Some(bundle.certificate.valid_until),
            validation_phase: Some("phase3_require_dual".to_string()),
            issuers: vec![IssuerConfig {
                label: Some("governance".to_string()),
                ed25519_hex: hex::encode(ed_public),
                mldsa_hex: Some(hex::encode(issuer_keys.public_key())),
            }],
            bundles: vec![BundleConfig {
                path: bundle_path.clone(),
            }],
            guard_pinning_proofs_dir: None,
            guard_pinning_proofs: Vec::new(),
        };
        write_directory_config(&config_path, &config);

        // Build once to derive the snapshot bytes used by relays in production.
        let initial_bundle =
            build_snapshot_from_config(&config_path).expect("initial build succeeds");
        let snapshot_bytes = initial_bundle.snapshot.to_bytes().expect("encode snapshot");
        fs::create_dir_all(snapshot_path.parent().expect("snapshot dir")).expect("create dir");
        fs::write(&snapshot_path, snapshot_bytes).expect("write snapshot");

        let entry = GuardDirectoryEntry {
            bundle: bundle.clone(),
            directory_hash: certificate.directory_hash,
            validation_phase: CertificateValidationPhase::Phase3RequireDual,
        };
        persist_guard_pinning_proof(
            &proof_abs_path,
            &snapshot_path,
            &entry,
            &certificate.relay_id,
            SystemTime::UNIX_EPOCH + Duration::from_secs(42),
        )
        .expect("persist proof");

        // Rebuild with guard pinning proofs so the metadata captures the evidence.
        config.guard_pinning_proofs = vec![PinningProofConfig {
            path: proof_rel_path.clone(),
        }];
        write_directory_config(&config_path, &config);
        let bundle_with_proof =
            build_snapshot_from_config(&config_path).expect("build with proof succeeds");
        assert_eq!(
            bundle_with_proof.metadata.guard_pinning_proofs.len(),
            1,
            "proof summary included in metadata"
        );
        let summary = &bundle_with_proof.metadata.guard_pinning_proofs[0];
        assert_eq!(summary.relay_id_hex, hex::encode(certificate.relay_id));
        assert_eq!(summary.path, proof_abs_path);
        assert_eq!(
            summary.directory_hash_hex,
            hex::encode(certificate.directory_hash)
        );
        assert_eq!(summary.validation_phase, "phase3_require_dual");
        assert_eq!(summary.guard_weight, certificate.guard_weight);
        assert_eq!(
            summary.bandwidth_bytes_per_sec,
            certificate.bandwidth_bytes_per_sec
        );
    }

    #[test]
    fn build_snapshot_collects_guard_pinning_directory() {
        let issuer_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");
        let mut rng = StdRng::seed_from_u64(0xA11ECE44);
        let mut ed_seed = [0u8; 32];
        rng.fill_bytes(&mut ed_seed);
        let signing_key = SigningKey::from_bytes(&ed_seed);
        let ed_public = signing_key.verifying_key().to_bytes();
        let fingerprint = compute_issuer_fingerprint(&ed_public, issuer_keys.public_key());

        let certificate = sample_certificate(fingerprint);
        let bundle = certificate
            .clone()
            .issue(&signing_key, issuer_keys.secret_key())
            .expect("issue");

        let dir = tempdir().expect("tempdir");
        let bundle_path = dir.path().join("relay.cbor");
        fs::write(&bundle_path, bundle.to_cbor()).expect("write bundle");

        let config_path = dir.path().join("directory.json");
        let snapshot_path = dir.path().join("snapshots/current.norito");
        let evidence_dir = dir.path().join("evidence");
        fs::create_dir_all(&evidence_dir).expect("create evidence dir");
        let proof_path = evidence_dir.join("entry.json");

        let mut config = DirectoryBuildConfig {
            directory_hash_hex: Some(hex::encode(bundle.certificate.directory_hash)),
            published_at_unix: Some(bundle.certificate.published_at),
            valid_after_unix: Some(bundle.certificate.valid_after),
            valid_until_unix: Some(bundle.certificate.valid_until),
            validation_phase: Some("phase3_require_dual".to_string()),
            issuers: vec![IssuerConfig {
                label: Some("governance".to_string()),
                ed25519_hex: hex::encode(ed_public),
                mldsa_hex: Some(hex::encode(issuer_keys.public_key())),
            }],
            bundles: vec![BundleConfig {
                path: bundle_path.clone(),
            }],
            guard_pinning_proofs_dir: None,
            guard_pinning_proofs: Vec::new(),
        };
        write_directory_config(&config_path, &config);

        let snapshot_bundle =
            build_snapshot_from_config(&config_path).expect("initial snapshot build succeeds");
        let snapshot_bytes = snapshot_bundle
            .snapshot
            .to_bytes()
            .expect("encode snapshot");
        fs::create_dir_all(snapshot_path.parent().expect("snapshot dir")).expect("create dir");
        fs::write(&snapshot_path, snapshot_bytes).expect("write snapshot");

        let entry = GuardDirectoryEntry {
            bundle,
            directory_hash: certificate.directory_hash,
            validation_phase: CertificateValidationPhase::Phase3RequireDual,
        };
        persist_guard_pinning_proof(
            &proof_path,
            &snapshot_path,
            &entry,
            &certificate.relay_id,
            SystemTime::UNIX_EPOCH + Duration::from_secs(42),
        )
        .expect("persist proof");

        // Rebuild with guard_pinning_proofs_dir so the builder discovers evidence automatically.
        config.guard_pinning_proofs_dir = Some(PathBuf::from("evidence"));
        write_directory_config(&config_path, &config);

        let bundle_with_proof = build_snapshot_from_config(&config_path).expect("rebuild snapshot");
        assert_eq!(
            bundle_with_proof.metadata.guard_pinning_proofs.len(),
            1,
            "expected guard pinning proof collected from directory"
        );
        let summary = &bundle_with_proof.metadata.guard_pinning_proofs[0];
        assert_eq!(
            summary.path,
            proof_path.canonicalize().expect("canonicalize proof path"),
            "proof summary path should canonicalize the discovered file"
        );
    }

    #[test]
    fn collect_guard_pinning_proofs_from_directory_verifies_entries() {
        let issuer_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");
        let mut rng = StdRng::seed_from_u64(0xA11ECE55);
        let mut ed_seed = [0u8; 32];
        rng.fill_bytes(&mut ed_seed);
        let signing_key = SigningKey::from_bytes(&ed_seed);
        let ed_public = signing_key.verifying_key().to_bytes();
        let fingerprint = compute_issuer_fingerprint(&ed_public, issuer_keys.public_key());

        let certificate = sample_certificate(fingerprint);
        let bundle = certificate
            .clone()
            .issue(&signing_key, issuer_keys.secret_key())
            .expect("issue");

        let dir = tempdir().expect("tempdir");
        let bundle_path = dir.path().join("relay.cbor");
        fs::write(&bundle_path, bundle.to_cbor()).expect("write bundle");

        let config_path = dir.path().join("directory.json");
        let snapshot_path = dir.path().join("snapshots/current.norito");
        let evidence_dir = dir.path().join("evidence");
        fs::create_dir_all(&evidence_dir).expect("create evidence dir");
        let proof_path = evidence_dir.join("entry.json");

        let config = DirectoryBuildConfig {
            directory_hash_hex: Some(hex::encode(bundle.certificate.directory_hash)),
            published_at_unix: Some(bundle.certificate.published_at),
            valid_after_unix: Some(bundle.certificate.valid_after),
            valid_until_unix: Some(bundle.certificate.valid_until),
            validation_phase: Some("phase3_require_dual".to_string()),
            issuers: vec![IssuerConfig {
                label: Some("governance".to_string()),
                ed25519_hex: hex::encode(ed_public),
                mldsa_hex: Some(hex::encode(issuer_keys.public_key())),
            }],
            bundles: vec![BundleConfig {
                path: bundle_path.clone(),
            }],
            guard_pinning_proofs_dir: None,
            guard_pinning_proofs: Vec::new(),
        };
        write_directory_config(&config_path, &config);

        let snapshot_bundle =
            build_snapshot_from_config(&config_path).expect("initial snapshot build succeeds");
        let snapshot_bytes = snapshot_bundle
            .snapshot
            .to_bytes()
            .expect("encode snapshot");
        fs::create_dir_all(snapshot_path.parent().expect("snapshot dir")).expect("create dir");
        fs::write(&snapshot_path, snapshot_bytes).expect("write snapshot");

        let entry = GuardDirectoryEntry {
            bundle,
            directory_hash: certificate.directory_hash,
            validation_phase: CertificateValidationPhase::Phase3RequireDual,
        };
        persist_guard_pinning_proof(
            &proof_path,
            &snapshot_path,
            &entry,
            &certificate.relay_id,
            SystemTime::UNIX_EPOCH + Duration::from_secs(7),
        )
        .expect("persist proof");

        let summaries =
            collect_guard_pinning_proofs_from_directory(&evidence_dir, &snapshot_bundle.snapshot)
                .expect("collect summaries");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].relay_id_hex, hex::encode(certificate.relay_id));
        assert_eq!(
            summaries[0].path,
            proof_path.canonicalize().expect("canonicalize proof path")
        );
    }

    #[test]
    fn rotate_snapshot_reissues_certificates() {
        let issuer_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");
        let mut rng = StdRng::seed_from_u64(0xA55A56);
        let mut ed_seed = [0u8; 32];
        rng.fill_bytes(&mut ed_seed);
        let signing_key = SigningKey::from_bytes(&ed_seed);
        let ed_public = signing_key.verifying_key().to_bytes();
        let fingerprint = compute_issuer_fingerprint(&ed_public, issuer_keys.public_key());

        let certificate = sample_certificate(fingerprint);
        let bundle = certificate
            .clone()
            .issue(&signing_key, issuer_keys.secret_key())
            .expect("issue");

        let snapshot = GuardDirectorySnapshotV2 {
            version: GUARD_DIRECTORY_VERSION_V2,
            directory_hash: certificate.directory_hash,
            published_at_unix: certificate.published_at,
            valid_after_unix: certificate.valid_after,
            valid_until_unix: certificate.valid_until,
            validation_phase: encode_validation_phase(
                CertificateValidationPhase::Phase3RequireDual,
            ),
            issuers: vec![GuardDirectoryIssuerV1 {
                fingerprint,
                ed25519_public: ed_public,
                mldsa65_public: issuer_keys.public_key().to_vec(),
            }],
            relays: vec![GuardDirectoryRelayEntryV2 {
                certificate: bundle.to_cbor(),
            }],
        };

        let bytes = snapshot.to_bytes().expect("encode snapshot");
        let mut rng = StdRng::seed_from_u64(0xDEADBEEF);
        let output = rotate_snapshot(&bytes, &mut rng).expect("rotate snapshot");

        assert_eq!(
            output.bundle.metadata.certificates.len(),
            snapshot.relays.len()
        );
        assert_ne!(output.keys.fingerprint, fingerprint);
        assert_ne!(output.keys.ed25519_public, ed_public);

        let new_verifying = VerifyingKey::from_bytes(&output.keys.ed25519_public).expect("key");
        for entry in &output.bundle.snapshot.relays {
            let bundle = RelayCertificateBundleV2::from_cbor(&entry.certificate).expect("bundle");
            bundle
                .verify(
                    &new_verifying,
                    &output.keys.mldsa_public,
                    output.bundle.metadata.validation_phase,
                )
                .expect("verify");
        }
    }

    fn sample_certificate(fingerprint: [u8; 32]) -> RelayCertificateV2 {
        let identity = [0x22; 32];
        RelayCertificateV2 {
            relay_id: identity,
            identity_ed25519: identity,
            identity_mldsa65: vec![0x33; 1952],
            descriptor_commit: [0x44; 32],
            roles: RelayRolesV2 {
                entry: true,
                middle: true,
                exit: false,
            },
            guard_weight: 180,
            bandwidth_bytes_per_sec: 2_500_000,
            reputation_weight: 90,
            endpoints: vec![RelayEndpointV2 {
                url: "soranet://relay.example:443".to_string(),
                priority: 0,
                tags: vec!["norito_stream".to_string()],
            }],
            capability_flags: RelayCapabilityFlagsV1::new(
                CapabilityToggle::Enabled,
                CapabilityToggle::Disabled,
                CapabilityToggle::Enabled,
                CapabilityToggle::Disabled,
            ),
            kem_policy: KemRotationPolicyV1 {
                mode: KemRotationModeV1::Static,
                preferred_suite: 2,
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
            directory_hash: [0x55; 32],
            issuer_fingerprint: fingerprint,
            pq_kem_public: vec![0x66; 1184],
        }
    }

    fn write_directory_config(path: &Path, config: &DirectoryBuildConfig) {
        let json_value = norito::json::to_value(config).expect("serialize config");
        let json = norito::json::to_string(&json_value).expect("encode config");
        fs::write(path, json).expect("write config");
    }
}
