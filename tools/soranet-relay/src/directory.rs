//! Guard directory snapshot tooling (builder, rotation, inspection).
#![allow(unexpected_cfgs)]

use std::{
    collections::{HashMap, TryReserveError},
    fs,
    path::{Path, PathBuf},
};

use ed25519_dalek::{SigningKey, VerifyingKey};
use hex::FromHexError;
use iroha_crypto::soranet::{
    certificate::{
        CertificateError, CertificateValidationPhase, RelayCertificateBundleV2,
        SRC_V2_MAX_BUNDLE_BYTES,
    },
    directory::{
        GUARD_DIRECTORY_ISSUER_MLDSA65_MAX_BYTES_V1, GUARD_DIRECTORY_MAX_ISSUERS_V1,
        GUARD_DIRECTORY_MAX_RELAYS_V1, GUARD_DIRECTORY_VERSION_V2, GuardDirectoryIssuerV1,
        GuardDirectoryRelayEntryV2, GuardDirectorySnapshotV2, compute_issuer_fingerprint,
        decode_validation_phase, encode_validation_phase,
    },
};
use norito::{
    DecodeLimits,
    json::{self, JsonDeserialize, JsonSerialize},
};
use rand::{CryptoRng, RngCore, SeedableRng, rngs::StdRng};
use soranet_pq::{
    MlDsaError, MlDsaKeyPair, MlDsaSuite, generate_mldsa_keypair_from_os as generate_mldsa_keypair,
};
use thiserror::Error;

use crate::guard::{
    GuardPinningProof, GuardPinningProofValidationError, verify_guard_pinning_proof,
};
use crate::{checked_ed25519_verifying_key_from_bytes, config::read_bounded_direct_regular_file};

// The builder admits at most the encoded corpus that a full first-release
// 64-relay directory can reference. JSON escaping can expand the source while
// decoded strings remain under the separately audited 640 KiB budget.
const GUARD_DIRECTORY_BUNDLE_MAX_TOTAL_BYTES_V1: usize =
    GUARD_DIRECTORY_MAX_RELAYS_V1 * SRC_V2_MAX_BUNDLE_BYTES;
const DIRECTORY_BUILD_CONFIG_JSON_MAX_BYTES_V1: usize = GUARD_DIRECTORY_BUNDLE_MAX_TOTAL_BYTES_V1;
const DIRECTORY_BUILD_CONFIG_JSON_MAX_FIELD_BYTES_V1: usize = 4 * 1024;
const DIRECTORY_BUILD_CONFIG_JSON_MAX_TOTAL_STRING_BYTES_V1: usize = 640 * 1024;
const DIRECTORY_BUILD_CONFIG_JSON_MAX_SEQUENCE_ELEMENTS_V1: usize = GUARD_DIRECTORY_MAX_RELAYS_V1;
const DIRECTORY_BUILD_CONFIG_JSON_MAX_TOTAL_ELEMENTS_V1: usize = 512;
const DIRECTORY_BUILD_CONFIG_JSON_MAX_ALLOCATED_BYTES_V1: usize =
    2 * DIRECTORY_BUILD_CONFIG_JSON_MAX_BYTES_V1;
const DIRECTORY_BUILD_CONFIG_JSON_MAX_DEPTH_V1: usize = 8;
const DIRECTORY_BUILD_CONFIG_LABEL_MAX_BYTES_V1: usize = 256;
const DIRECTORY_BUILD_CONFIG_PATH_MAX_BYTES_V1: usize = 4 * 1024;

// A proof only restates bounded fields from one SRCv2 bundle plus its snapshot
// path. Capping it at the source bundle's 64 KiB ceiling admits the worst-case
// 4 KiB path and ML-KEM-1024 hex field even when JSON-escaped.
const GUARD_PINNING_PROOF_JSON_MAX_BYTES_V1: usize = SRC_V2_MAX_BUNDLE_BYTES;
const GUARD_PINNING_PROOF_JSON_MAX_FIELD_BYTES_V1: usize = 4 * 1024;
const GUARD_PINNING_PROOF_JSON_MAX_TOTAL_STRING_BYTES_V1: usize = 12 * 1024;
const GUARD_PINNING_PROOF_JSON_MAX_SEQUENCE_ELEMENTS_V1: usize = 32;
const GUARD_PINNING_PROOF_JSON_MAX_TOTAL_ELEMENTS_V1: usize = 32;
const GUARD_PINNING_PROOF_JSON_MAX_ALLOCATED_BYTES_V1: usize =
    2 * GUARD_PINNING_PROOF_JSON_MAX_BYTES_V1;
const GUARD_PINNING_PROOF_JSON_MAX_DEPTH_V1: usize = 4;

// One proof can exist for each first-release relay. Counting every directory
// entry, not only JSON candidates, bounds traversal work before collection and
// sorting.
const GUARD_PINNING_PROOF_DIRECTORY_MAX_ENTRIES_V1: usize = GUARD_DIRECTORY_MAX_RELAYS_V1;

const DIRECTORY_BUILD_CONFIG_JSON_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    DIRECTORY_BUILD_CONFIG_JSON_MAX_SEQUENCE_ELEMENTS_V1,
    DIRECTORY_BUILD_CONFIG_JSON_MAX_FIELD_BYTES_V1,
    DIRECTORY_BUILD_CONFIG_JSON_MAX_TOTAL_ELEMENTS_V1,
    DIRECTORY_BUILD_CONFIG_JSON_MAX_ALLOCATED_BYTES_V1,
    DIRECTORY_BUILD_CONFIG_JSON_MAX_DEPTH_V1,
);

const GUARD_PINNING_PROOF_JSON_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    GUARD_PINNING_PROOF_JSON_MAX_SEQUENCE_ELEMENTS_V1,
    GUARD_PINNING_PROOF_JSON_MAX_FIELD_BYTES_V1,
    GUARD_PINNING_PROOF_JSON_MAX_TOTAL_ELEMENTS_V1,
    GUARD_PINNING_PROOF_JSON_MAX_ALLOCATED_BYTES_V1,
    GUARD_PINNING_PROOF_JSON_MAX_DEPTH_V1,
);

const fn directory_build_config_json_preflight_limits_v1() -> json::JsonPreflightLimits {
    json::JsonPreflightLimits::new(
        DIRECTORY_BUILD_CONFIG_JSON_MAX_BYTES_V1,
        DIRECTORY_BUILD_CONFIG_JSON_MAX_TOTAL_ELEMENTS_V1.saturating_add(1),
        DIRECTORY_BUILD_CONFIG_JSON_MAX_BYTES_V1,
        DIRECTORY_BUILD_CONFIG_JSON_MAX_FIELD_BYTES_V1,
        DIRECTORY_BUILD_CONFIG_JSON_MAX_TOTAL_STRING_BYTES_V1,
        DIRECTORY_BUILD_CONFIG_JSON_MAX_SEQUENCE_ELEMENTS_V1,
        DIRECTORY_BUILD_CONFIG_JSON_MAX_TOTAL_ELEMENTS_V1,
        DIRECTORY_BUILD_CONFIG_JSON_MAX_TOTAL_ELEMENTS_V1,
        DIRECTORY_BUILD_CONFIG_JSON_MAX_TOTAL_ELEMENTS_V1,
        DIRECTORY_BUILD_CONFIG_JSON_MAX_DEPTH_V1,
    )
}

const fn guard_pinning_proof_json_preflight_limits_v1() -> json::JsonPreflightLimits {
    json::JsonPreflightLimits::new(
        GUARD_PINNING_PROOF_JSON_MAX_BYTES_V1,
        GUARD_PINNING_PROOF_JSON_MAX_TOTAL_ELEMENTS_V1.saturating_add(1),
        GUARD_PINNING_PROOF_JSON_MAX_BYTES_V1,
        GUARD_PINNING_PROOF_JSON_MAX_FIELD_BYTES_V1,
        GUARD_PINNING_PROOF_JSON_MAX_TOTAL_STRING_BYTES_V1,
        GUARD_PINNING_PROOF_JSON_MAX_SEQUENCE_ELEMENTS_V1,
        0,
        GUARD_PINNING_PROOF_JSON_MAX_TOTAL_ELEMENTS_V1,
        GUARD_PINNING_PROOF_JSON_MAX_TOTAL_ELEMENTS_V1,
        GUARD_PINNING_PROOF_JSON_MAX_DEPTH_V1,
    )
}

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
    /// Hex-encoded ML-DSA-65 public key.
    #[norito(default)]
    pub mldsa_hex: String,
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
    #[error(
        "guard pinning proof directory `{path}` contains at least {found} entries; first-release traversal limit is {maximum}"
    )]
    TooManyEntries {
        path: PathBuf,
        found: usize,
        maximum: usize,
    },
    #[error(
        "guard pinning proof directory `{path}` contains at least {found} JSON proofs; remaining first-release relay capacity is {maximum}"
    )]
    TooManyProofs {
        path: PathBuf,
        found: usize,
        maximum: usize,
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
    #[error("JSON config admission failed for {path}: {source}")]
    JsonAdmission {
        path: PathBuf,
        #[source]
        source: json::JsonPreflightError,
    },
    #[error("{field} contains {found} entries; first-release limit is {maximum}")]
    TooManyEntries {
        field: &'static str,
        found: usize,
        maximum: usize,
    },
    #[error("{field} is {found} bytes; first-release limit is {maximum} bytes")]
    FieldTooLong {
        field: &'static str,
        found: usize,
        maximum: usize,
    },
    #[error("{artifact} total is {found} bytes; first-release limit is {maximum} bytes")]
    AggregateBytes {
        artifact: &'static str,
        found: usize,
        maximum: usize,
    },
    #[error("failed to reserve bounded storage for {artifact}: {source}")]
    Allocation {
        artifact: &'static str,
        #[source]
        source: TryReserveError,
    },
    #[error("no issuers supplied in configuration")]
    NoIssuers,
    #[error("no certificate bundles supplied in configuration")]
    NoBundles,
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
    #[error("issuer {label} missing ML-DSA-65 public key required by the first-release policy")]
    IssuerMissingMlDsa { label: String },
    #[error("duplicate issuer fingerprint {fingerprint}")]
    DuplicateIssuer { fingerprint: String },
    #[error("issuer {label} contained an invalid Ed25519 public key: {source}")]
    InvalidIssuerEd25519 {
        label: String,
        #[source]
        source: ed25519_dalek::SignatureError,
    },
    #[error("issuer {label} contained invalid Ed25519 public key material: {reason}")]
    InvalidIssuerEd25519Material { label: String, reason: String },
    #[error("issuer {label} fingerprint could not be computed: {source}")]
    IssuerFingerprint {
        label: String,
        #[source]
        source: norito::Error,
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
    #[error("guard pinning proof JSON admission failed for {path}: {source}")]
    GuardPinningProofAdmission {
        path: PathBuf,
        #[source]
        source: json::JsonPreflightError,
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
    #[error(
        "snapshot validation phase {phase:?} is not accepted; the first release requires phase 3 dual signatures"
    )]
    UnsupportedReleasePhase { phase: CertificateValidationPhase },
    #[error("snapshot contained no relay certificates to rotate")]
    NoCertificates,
    #[error("issuer public key invalid: {source}")]
    InvalidIssuerKey {
        #[source]
        source: ed25519_dalek::SignatureError,
    },
    #[error("issuer public key has invalid material: {reason}")]
    InvalidIssuerKeyMaterial { reason: String },
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
    #[error("generated Ed25519 issuer seed material is invalid: {reason}")]
    InvalidGeneratedIssuerKeyMaterial { reason: String },
    #[error("rotated issuer fingerprint could not be computed: {source}")]
    IssuerFingerprint {
        #[source]
        source: norito::Error,
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
    let bytes = read_bounded_direct_regular_file(
        path,
        DIRECTORY_BUILD_CONFIG_JSON_MAX_BYTES_V1,
        "guard directory build configuration JSON",
    )
    .map_err(|source| DirectoryBuildError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    json::preflight_slice(&bytes, directory_build_config_json_preflight_limits_v1()).map_err(
        |source| DirectoryBuildError::JsonAdmission {
            path: path.to_path_buf(),
            source,
        },
    )?;
    let mut config: DirectoryBuildConfig =
        norito::with_decode_limits_scope(DIRECTORY_BUILD_CONFIG_JSON_DECODE_LIMITS_V1, || {
            json::from_slice(&bytes)
        })
        .map_err(|source| DirectoryBuildError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    if let Some(dir) = options.guard_pinning_proofs_dir {
        config.guard_pinning_proofs_dir = Some(dir.to_path_buf());
    }
    build_snapshot(config, path.parent().unwrap_or_else(|| Path::new(".")))
}

/// Read and decode one first-release guard pinning proof from a stable direct
/// regular file.
///
/// The file is capped at 64 KiB (the maximum encoded SRCv2 bundle size), its
/// JSON is lexically admitted without allocation, and the owned decode runs
/// under explicit field, element, allocation, and nesting limits.
///
/// # Errors
/// Returns [`DirectoryBuildError`] when the file is not a stable direct regular
/// file, exceeds the first-release limit, or fails JSON admission or decoding.
pub fn read_guard_pinning_proof_file(
    path: &Path,
) -> Result<GuardPinningProof, DirectoryBuildError> {
    let bytes = read_bounded_direct_regular_file(
        path,
        GUARD_PINNING_PROOF_JSON_MAX_BYTES_V1,
        "guard pinning proof JSON",
    )
    .map_err(|source| DirectoryBuildError::GuardPinningProofIo {
        path: path.to_path_buf(),
        source,
    })?;
    json::preflight_slice(&bytes, guard_pinning_proof_json_preflight_limits_v1()).map_err(
        |source| DirectoryBuildError::GuardPinningProofAdmission {
            path: path.to_path_buf(),
            source,
        },
    )?;
    norito::with_decode_limits_scope(GUARD_PINNING_PROOF_JSON_DECODE_LIMITS_V1, || {
        json::from_slice(&bytes)
    })
    .map_err(|source| DirectoryBuildError::GuardPinningProofDecode {
        path: path.to_path_buf(),
        source,
    })
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
    let snapshot = GuardDirectorySnapshotV2::inspect_bytes(snapshot_bytes)
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
    let snapshot = GuardDirectorySnapshotV2::inspect_bytes(snapshot_bytes)
        .map_err(|source| DirectoryRotateError::Decode { source })?;
    let metadata = summarize_snapshot(&snapshot)?;
    Ok(DirectorySnapshotBundle { snapshot, metadata })
}

fn build_snapshot(
    config: DirectoryBuildConfig,
    base_dir: &Path,
) -> Result<DirectorySnapshotBundle, DirectoryBuildError> {
    validate_directory_build_config_limits(&config)?;
    if config.issuers.is_empty() {
        return Err(DirectoryBuildError::NoIssuers);
    }
    if config.bundles.is_empty() {
        return Err(DirectoryBuildError::NoBundles);
    }

    let validation_phase = CertificateValidationPhase::Phase3RequireDual;
    let issuers = load_issuers(&config.issuers)?;

    let mut issuer_map: HashMap<[u8; 32], usize> = HashMap::new();
    issuer_map
        .try_reserve(issuers.len())
        .map_err(|source| DirectoryBuildError::Allocation {
            artifact: "guard directory issuer index",
            source,
        })?;
    for (index, issuer) in issuers.iter().enumerate() {
        if issuer_map.insert(issuer.fingerprint, index).is_some() {
            return Err(DirectoryBuildError::DuplicateIssuer {
                fingerprint: hex::encode(issuer.fingerprint),
            });
        }
    }

    let mut parsed_bundles: Vec<(PathBuf, RelayCertificateBundleV2)> = Vec::new();
    parsed_bundles
        .try_reserve_exact(config.bundles.len())
        .map_err(|source| DirectoryBuildError::Allocation {
            artifact: "parsed relay certificate bundles",
            source,
        })?;
    let mut certificate_summaries: Vec<CertificateSummary> = Vec::new();
    certificate_summaries
        .try_reserve_exact(config.bundles.len())
        .map_err(|source| DirectoryBuildError::Allocation {
            artifact: "relay certificate summaries",
            source,
        })?;
    let mut retained_bundle_bytes = 0_usize;

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
        let bytes = read_bounded_direct_regular_file(
            &absolute_path,
            SRC_V2_MAX_BUNDLE_BYTES,
            "SRCv2 relay certificate bundle",
        )
        .map_err(|source| DirectoryBuildError::Io {
            path: absolute_path.clone(),
            source,
        })?;
        retained_bundle_bytes = account_retained_bundle_bytes(retained_bundle_bytes, bytes.len())?;
        let bundle = RelayCertificateBundleV2::from_cbor(&bytes).map_err(|source| {
            DirectoryBuildError::CertificateVerify {
                path: absolute_path.clone(),
                source,
            }
        })?;

        let fingerprint = bundle.certificate.issuer_fingerprint;
        let issuer_index = issuer_map.get(&fingerprint).ok_or_else(|| {
            DirectoryBuildError::UnknownIssuerForCertificate {
                fingerprint: hex::encode(fingerprint),
                path: absolute_path.clone(),
            }
        })?;
        let issuer = &issuers[*issuer_index];

        bundle
            .verify_signatures(
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

    let mut issuer_list = Vec::new();
    issuer_list
        .try_reserve_exact(issuers.len())
        .map_err(|source| DirectoryBuildError::Allocation {
            artifact: "guard directory issuer records",
            source,
        })?;
    let mut issuer_summaries = Vec::new();
    issuer_summaries
        .try_reserve_exact(issuers.len())
        .map_err(|source| DirectoryBuildError::Allocation {
            artifact: "guard directory issuer summaries",
            source,
        })?;
    for issuer in issuers {
        issuer_summaries.push(IssuerSummary {
            label: issuer.label,
            fingerprint_hex: hex::encode(issuer.fingerprint),
            ed25519_hex: hex::encode(issuer.ed25519_bytes),
            has_mldsa: !issuer.mldsa_public.is_empty(),
        });
        issuer_list.push(GuardDirectoryIssuerV1 {
            fingerprint: issuer.fingerprint,
            ed25519_public: issuer.ed25519_bytes,
            mldsa65_public: issuer.mldsa_public,
        });
    }

    let mut relays = Vec::new();
    relays
        .try_reserve_exact(parsed_bundles.len())
        .map_err(|source| DirectoryBuildError::Allocation {
            artifact: "guard directory relay records",
            source,
        })?;
    for (path, bundle) in parsed_bundles {
        let certificate =
            bundle
                .try_to_cbor()
                .map_err(|source| DirectoryBuildError::CertificateVerify {
                    path: path.clone(),
                    source,
                })?;
        relays.push(GuardDirectoryRelayEntryV2 { certificate });
    }

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
        let remaining = GUARD_DIRECTORY_MAX_RELAYS_V1
            .checked_sub(guard_pinning_proofs.len())
            .ok_or(DirectoryBuildError::TooManyEntries {
                field: "guard_pinning_proofs",
                found: guard_pinning_proofs.len(),
                maximum: GUARD_DIRECTORY_MAX_RELAYS_V1,
            })?;
        let mut collected =
            collect_guard_pinning_proofs_from_directory_with_limit(&resolved, &snapshot, remaining)
                .map_err(|source| DirectoryBuildError::GuardPinningCollect {
                    path: resolved.clone(),
                    source: Box::new(source),
                })?;
        if guard_pinning_proofs.is_empty() {
            guard_pinning_proofs = collected;
        } else {
            guard_pinning_proofs
                .try_reserve_exact(collected.len())
                .map_err(|source| DirectoryBuildError::Allocation {
                    artifact: "merged guard pinning proof summaries",
                    source,
                })?;
            for proof in collected.drain(..) {
                if guard_pinning_proofs
                    .iter()
                    .any(|retained| retained.relay_id_hex == proof.relay_id_hex)
                {
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
    if validation_phase != CertificateValidationPhase::Phase3RequireDual {
        return Err(DirectoryRotateError::UnsupportedReleasePhase {
            phase: validation_phase,
        });
    }

    let issuer = &snapshot.issuers[0];
    let verifying_key = checked_ed25519_verifying_key_from_bytes(&issuer.ed25519_public)
        .map_err(|reason| DirectoryRotateError::InvalidIssuerKeyMaterial { reason })?;

    let mut parsed_bundles: Vec<RelayCertificateBundleV2> =
        Vec::with_capacity(snapshot.relays.len());
    let mut certificate_summaries: Vec<CertificateSummary> =
        Vec::with_capacity(snapshot.relays.len());

    for (index, relay_entry) in snapshot.relays.iter().enumerate() {
        let bundle = RelayCertificateBundleV2::from_cbor(&relay_entry.certificate)
            .map_err(|source| DirectoryRotateError::CertificateDecode { index, source })?;
        bundle
            .verify_signatures(&verifying_key, &issuer.mldsa65_public, validation_phase)
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
    if ed_seed.iter().all(|byte| *byte == 0) {
        return Err(DirectoryRotateError::InvalidGeneratedIssuerKeyMaterial {
            reason: "ed25519 private key seed material must not be all zero".to_string(),
        });
    }
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

    let fingerprint = compute_issuer_fingerprint(&ed_public, &mldsa_public)
        .map_err(|source| DirectoryRotateError::IssuerFingerprint { source })?;

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
        let verifying_key = checked_ed25519_verifying_key_from_bytes(&issuer.ed25519_public)
            .map_err(|reason| DirectoryRotateError::InvalidIssuerKeyMaterial { reason })?;
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
                .verify_signatures(issuer_key, mldsa_public, validation_phase)
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

struct LoadedIssuer {
    label: Option<String>,
    verifying_key: VerifyingKey,
    ed25519_bytes: [u8; 32],
    mldsa_public: Vec<u8>,
    fingerprint: [u8; 32],
}

fn validate_directory_build_config_limits(
    config: &DirectoryBuildConfig,
) -> Result<(), DirectoryBuildError> {
    validate_entry_count(
        "issuers",
        config.issuers.len(),
        GUARD_DIRECTORY_MAX_ISSUERS_V1,
    )?;
    validate_entry_count(
        "bundles",
        config.bundles.len(),
        GUARD_DIRECTORY_MAX_RELAYS_V1,
    )?;
    validate_entry_count(
        "guard_pinning_proofs",
        config.guard_pinning_proofs.len(),
        GUARD_DIRECTORY_MAX_RELAYS_V1,
    )?;

    for issuer in &config.issuers {
        if let Some(label) = issuer.label.as_deref() {
            validate_field_len(
                "issuers[].label",
                label.len(),
                DIRECTORY_BUILD_CONFIG_LABEL_MAX_BYTES_V1,
            )?;
        }
    }
    for bundle in &config.bundles {
        validate_config_path("bundles[].path", &bundle.path)?;
    }
    for proof in &config.guard_pinning_proofs {
        validate_config_path("guard_pinning_proofs[].path", &proof.path)?;
    }
    if let Some(path) = config.guard_pinning_proofs_dir.as_deref() {
        validate_config_path("guard_pinning_proofs_dir", path)?;
    }
    Ok(())
}

fn validate_entry_count(
    field: &'static str,
    found: usize,
    maximum: usize,
) -> Result<(), DirectoryBuildError> {
    if found > maximum {
        return Err(DirectoryBuildError::TooManyEntries {
            field,
            found,
            maximum,
        });
    }
    Ok(())
}

fn validate_field_len(
    field: &'static str,
    found: usize,
    maximum: usize,
) -> Result<(), DirectoryBuildError> {
    if found > maximum {
        return Err(DirectoryBuildError::FieldTooLong {
            field,
            found,
            maximum,
        });
    }
    Ok(())
}

fn validate_config_path(field: &'static str, path: &Path) -> Result<(), DirectoryBuildError> {
    validate_field_len(
        field,
        path.as_os_str().len(),
        DIRECTORY_BUILD_CONFIG_PATH_MAX_BYTES_V1,
    )
}

fn account_retained_bundle_bytes(
    retained: usize,
    additional: usize,
) -> Result<usize, DirectoryBuildError> {
    let found = retained
        .checked_add(additional)
        .ok_or(DirectoryBuildError::AggregateBytes {
            artifact: "SRCv2 relay certificate corpus",
            found: usize::MAX,
            maximum: GUARD_DIRECTORY_BUNDLE_MAX_TOTAL_BYTES_V1,
        })?;
    if found > GUARD_DIRECTORY_BUNDLE_MAX_TOTAL_BYTES_V1 {
        return Err(DirectoryBuildError::AggregateBytes {
            artifact: "SRCv2 relay certificate corpus",
            found,
            maximum: GUARD_DIRECTORY_BUNDLE_MAX_TOTAL_BYTES_V1,
        });
    }
    Ok(found)
}

fn try_clone_bounded_string(
    value: &str,
    artifact: &'static str,
) -> Result<String, DirectoryBuildError> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|source| DirectoryBuildError::Allocation { artifact, source })?;
    owned.push_str(value);
    Ok(owned)
}

fn load_issuers(configs: &[IssuerConfig]) -> Result<Vec<LoadedIssuer>, DirectoryBuildError> {
    let mut loaded = Vec::new();
    loaded
        .try_reserve_exact(configs.len())
        .map_err(|source| DirectoryBuildError::Allocation {
            artifact: "decoded guard directory issuers",
            source,
        })?;
    for config in configs {
        let ed_bytes = decode_hex_array::<32>(&config.ed25519_hex, "issuers[].ed25519_hex")?;
        let label_display = config
            .label
            .clone()
            .unwrap_or_else(|| "<unknown issuer>".to_string());
        let verifying_key =
            checked_ed25519_verifying_key_from_bytes(&ed_bytes).map_err(|reason| {
                DirectoryBuildError::InvalidIssuerEd25519Material {
                    label: label_display.clone(),
                    reason,
                }
            })?;
        let mldsa_public = decode_mldsa_bytes(&config.mldsa_hex, label_display.clone())?;
        let fingerprint =
            compute_issuer_fingerprint(&ed_bytes, &mldsa_public).map_err(|source| {
                DirectoryBuildError::IssuerFingerprint {
                    label: label_display.clone(),
                    source,
                }
            })?;
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

    let mut summaries = Vec::new();
    summaries
        .try_reserve_exact(configs.len())
        .map_err(|source| DirectoryBuildError::Allocation {
            artifact: "guard pinning proof summaries",
            source,
        })?;

    for proof_config in configs {
        let absolute_path = if proof_config.path.is_absolute() {
            proof_config.path.clone()
        } else {
            base_dir.join(&proof_config.path)
        };
        let proof = read_guard_pinning_proof_file(&absolute_path)?;
        verify_guard_pinning_proof(snapshot, &proof).map_err(|source| {
            DirectoryBuildError::GuardPinningProofValidation {
                path: absolute_path.clone(),
                source,
            }
        })?;

        if summaries
            .iter()
            .any(|summary: &GuardPinningProofSummary| summary.relay_id_hex == proof.relay_id_hex())
        {
            return Err(DirectoryBuildError::DuplicateGuardPinningProof {
                relay_id_hex: proof.relay_id_hex().to_string(),
                path: absolute_path.clone(),
            });
        }

        summaries.push(GuardPinningProofSummary::try_from_proof(
            absolute_path,
            &proof,
        )?);
    }

    Ok(summaries)
}

impl GuardPinningProofSummary {
    fn try_from_proof(
        path: PathBuf,
        proof: &GuardPinningProof,
    ) -> Result<Self, DirectoryBuildError> {
        Ok(Self {
            path,
            relay_id_hex: try_clone_bounded_string(
                proof.relay_id_hex(),
                "guard proof relay identifier summary",
            )?,
            directory_hash_hex: try_clone_bounded_string(
                proof.directory_hash_hex(),
                "guard proof directory hash summary",
            )?,
            descriptor_commit_hex: try_clone_bounded_string(
                proof.descriptor_commit_hex(),
                "guard proof descriptor commitment summary",
            )?,
            issuer_fingerprint_hex: try_clone_bounded_string(
                proof.issuer_fingerprint_hex(),
                "guard proof issuer fingerprint summary",
            )?,
            pq_kem_public_hex: try_clone_bounded_string(
                proof.pq_kem_public_hex(),
                "guard proof ML-KEM public key summary",
            )?,
            validation_phase: try_clone_bounded_string(
                proof.validation_phase(),
                "guard proof validation phase summary",
            )?,
            recorded_at_unix: proof.recorded_at_unix(),
            valid_after_unix: proof.valid_after_unix(),
            valid_until_unix: proof.valid_until_unix(),
            guard_weight: proof.guard_weight(),
            bandwidth_bytes_per_sec: proof.bandwidth_bytes_per_sec(),
            reputation_weight: proof.reputation_weight(),
        })
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
    collect_guard_pinning_proofs_from_directory_with_limit(
        directory,
        snapshot,
        GUARD_DIRECTORY_MAX_RELAYS_V1,
    )
}

fn collect_guard_pinning_proofs_from_directory_with_limit(
    directory: &Path,
    snapshot: &GuardDirectorySnapshotV2,
    maximum_proofs: usize,
) -> Result<Vec<GuardPinningProofSummary>, GuardPinningCollectError> {
    let directory_metadata =
        fs::symlink_metadata(directory).map_err(|_| GuardPinningCollectError::NotDirectory {
            path: directory.to_path_buf(),
        })?;
    if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
        return Err(GuardPinningCollectError::NotDirectory {
            path: directory.to_path_buf(),
        });
    }

    let mut configs = Vec::new();
    configs
        .try_reserve_exact(maximum_proofs)
        .map_err(|source| {
            GuardPinningCollectError::Build(Box::new(DirectoryBuildError::Allocation {
                artifact: "guard pinning proof directory candidates",
                source,
            }))
        })?;
    let absolute_directory =
        directory
            .canonicalize()
            .map_err(|source| GuardPinningCollectError::Canonicalize {
                path: directory.to_path_buf(),
                source,
            })?;
    let entries =
        fs::read_dir(&absolute_directory).map_err(|source| GuardPinningCollectError::ReadDir {
            path: directory.to_path_buf(),
            source,
        })?;

    let mut entry_count = 0_usize;
    for entry in entries {
        let entry = entry.map_err(|source| GuardPinningCollectError::EntryIo {
            path: directory.to_path_buf(),
            source,
        })?;
        entry_count =
            entry_count
                .checked_add(1)
                .ok_or_else(|| GuardPinningCollectError::TooManyEntries {
                    path: directory.to_path_buf(),
                    found: usize::MAX,
                    maximum: GUARD_PINNING_PROOF_DIRECTORY_MAX_ENTRIES_V1,
                })?;
        if entry_count > GUARD_PINNING_PROOF_DIRECTORY_MAX_ENTRIES_V1 {
            return Err(GuardPinningCollectError::TooManyEntries {
                path: directory.to_path_buf(),
                found: entry_count,
                maximum: GUARD_PINNING_PROOF_DIRECTORY_MAX_ENTRIES_V1,
            });
        }
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| GuardPinningCollectError::EntryIo {
                path: path.clone(),
                source,
            })?;
        if !file_type.is_file() {
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
        if configs.len() == maximum_proofs {
            return Err(GuardPinningCollectError::TooManyProofs {
                path: directory.to_path_buf(),
                found: configs.len().saturating_add(1),
                maximum: maximum_proofs,
            });
        }
        configs.push(PinningProofConfig {
            path: absolute_directory.join(entry.file_name()),
        });
    }

    if configs.is_empty() {
        return Err(GuardPinningCollectError::NoProofs {
            path: directory.to_path_buf(),
        });
    }

    configs.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    load_guard_pinning_proofs(&configs, Path::new(""), snapshot)
        .map_err(|source| GuardPinningCollectError::Build(Box::new(source)))
}

fn decode_mldsa_bytes(value: &str, label: String) -> Result<Vec<u8>, DirectoryBuildError> {
    if value.is_empty() {
        return Err(DirectoryBuildError::IssuerMissingMlDsa { label });
    }
    let bytes = hex::decode(value).map_err(|source| DirectoryBuildError::Hex {
        field: "issuers[].mldsa_hex".to_string(),
        source,
    })?;
    if bytes.len() != GUARD_DIRECTORY_ISSUER_MLDSA65_MAX_BYTES_V1 {
        return Err(DirectoryBuildError::InvalidHexLength {
            field: "issuers[].mldsa_hex".to_string(),
            expected: GUARD_DIRECTORY_ISSUER_MLDSA65_MAX_BYTES_V1,
            found: bytes.len(),
        });
    }
    Ok(bytes)
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

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use iroha_crypto::soranet::{
        certificate::{
            CapabilityToggle, KemRotationModeV1, KemRotationPolicyV1, RelayCapabilityFlagsV1,
            RelayCertificateV2, RelayEndpointV2, RelayRolesV2,
        },
        handshake::HandshakeSuite,
    };
    use rand::{SeedableRng, rngs::StdRng};
    use soranet_pq::{MlKemSuite, generate_mldsa_keypair_from_os as generate_mldsa_keypair};
    use tempfile::tempdir;

    use super::*;
    use crate::guard::{GuardDirectoryEntry, persist_guard_pinning_proof};

    const SMALL_ORDER_ED25519_POINT: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    #[test]
    fn directory_build_config_file_limit_accepts_exact_and_rejects_plus_one() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("directory.json");
        let config = config_with_counts(0, 0, 0);
        let json_value = json::to_value(&config).expect("serialize config");
        let mut bytes = json::to_string(&json_value)
            .expect("encode config")
            .into_bytes();
        assert!(bytes.len() < DIRECTORY_BUILD_CONFIG_JSON_MAX_BYTES_V1);

        bytes.resize(DIRECTORY_BUILD_CONFIG_JSON_MAX_BYTES_V1, b' ');
        fs::write(&path, &bytes).expect("write exact config");
        assert!(matches!(
            build_snapshot_from_config(&path),
            Err(DirectoryBuildError::NoIssuers)
        ));

        bytes.push(b' ');
        fs::write(&path, bytes).expect("write oversized config");
        let err = build_snapshot_from_config(&path).expect_err("max + 1 must fail at file read");
        match err {
            DirectoryBuildError::Io { source, .. } => {
                assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
            }
            other => panic!("unexpected oversized config error: {other:?}"),
        }
    }

    #[test]
    fn guard_pinning_proof_file_limit_accepts_exact_and_rejects_plus_one() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("proof.json");
        let mut bytes = minimal_guard_pinning_proof_json("");
        assert!(bytes.len() < GUARD_PINNING_PROOF_JSON_MAX_BYTES_V1);

        bytes.resize(GUARD_PINNING_PROOF_JSON_MAX_BYTES_V1, b' ');
        fs::write(&path, &bytes).expect("write exact proof");
        let proof = read_guard_pinning_proof_file(&path).expect("exact proof size is admitted");
        assert!(proof.snapshot_path().is_empty());

        bytes.push(b' ');
        fs::write(&path, bytes).expect("write oversized proof");
        let err =
            read_guard_pinning_proof_file(&path).expect_err("proof max + 1 must fail at file read");
        match err {
            DirectoryBuildError::GuardPinningProofIo { source, .. } => {
                assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
            }
            other => panic!("unexpected oversized proof error: {other:?}"),
        }
    }

    #[test]
    fn guard_pinning_proof_field_limit_accepts_exact_and_rejects_plus_one() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("proof.json");
        let exact_path = "p".repeat(GUARD_PINNING_PROOF_JSON_MAX_FIELD_BYTES_V1);
        fs::write(&path, minimal_guard_pinning_proof_json(&exact_path))
            .expect("write exact-field proof");
        let proof = read_guard_pinning_proof_file(&path).expect("exact field size is admitted");
        assert_eq!(proof.snapshot_path().len(), exact_path.len());

        let oversized_path = "p".repeat(GUARD_PINNING_PROOF_JSON_MAX_FIELD_BYTES_V1 + 1);
        fs::write(&path, minimal_guard_pinning_proof_json(&oversized_path))
            .expect("write oversized-field proof");
        assert!(matches!(
            read_guard_pinning_proof_file(&path),
            Err(DirectoryBuildError::GuardPinningProofAdmission { .. })
        ));
    }

    #[test]
    fn directory_build_source_counts_accept_exact_and_reject_plus_one() {
        validate_directory_build_config_limits(&config_with_counts(
            GUARD_DIRECTORY_MAX_ISSUERS_V1,
            GUARD_DIRECTORY_MAX_RELAYS_V1,
            GUARD_DIRECTORY_MAX_RELAYS_V1,
        ))
        .expect("exact source counts are admitted");

        assert_too_many_config_entries(
            config_with_counts(GUARD_DIRECTORY_MAX_ISSUERS_V1 + 1, 0, 0),
            "issuers",
            GUARD_DIRECTORY_MAX_ISSUERS_V1 + 1,
            GUARD_DIRECTORY_MAX_ISSUERS_V1,
        );
        assert_too_many_config_entries(
            config_with_counts(0, GUARD_DIRECTORY_MAX_RELAYS_V1 + 1, 0),
            "bundles",
            GUARD_DIRECTORY_MAX_RELAYS_V1 + 1,
            GUARD_DIRECTORY_MAX_RELAYS_V1,
        );
        assert_too_many_config_entries(
            config_with_counts(0, 0, GUARD_DIRECTORY_MAX_RELAYS_V1 + 1),
            "guard_pinning_proofs",
            GUARD_DIRECTORY_MAX_RELAYS_V1 + 1,
            GUARD_DIRECTORY_MAX_RELAYS_V1,
        );
    }

    #[test]
    fn directory_config_json_sequence_preflight_accepts_exact_and_rejects_plus_one() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("directory.json");
        write_directory_config(
            &path,
            &config_with_counts(0, GUARD_DIRECTORY_MAX_RELAYS_V1, 0),
        );
        assert!(matches!(
            build_snapshot_from_config(&path),
            Err(DirectoryBuildError::NoIssuers)
        ));

        write_directory_config(
            &path,
            &config_with_counts(0, GUARD_DIRECTORY_MAX_RELAYS_V1 + 1, 0),
        );
        assert!(matches!(
            build_snapshot_from_config(&path),
            Err(DirectoryBuildError::JsonAdmission { .. })
        ));
    }

    #[test]
    fn retained_bundle_byte_accounting_accepts_exact_and_rejects_plus_one() {
        assert_eq!(
            account_retained_bundle_bytes(GUARD_DIRECTORY_BUNDLE_MAX_TOTAL_BYTES_V1 - 1, 1)
                .expect("exact aggregate is admitted"),
            GUARD_DIRECTORY_BUNDLE_MAX_TOTAL_BYTES_V1
        );
        assert!(matches!(
            account_retained_bundle_bytes(GUARD_DIRECTORY_BUNDLE_MAX_TOTAL_BYTES_V1, 1),
            Err(DirectoryBuildError::AggregateBytes {
                found,
                maximum: GUARD_DIRECTORY_BUNDLE_MAX_TOTAL_BYTES_V1,
                ..
            }) if found == GUARD_DIRECTORY_BUNDLE_MAX_TOTAL_BYTES_V1 + 1
        ));
    }

    #[test]
    fn relay_bundle_file_limit_accepts_exact_and_rejects_plus_one() {
        let dir = tempdir().expect("tempdir");
        let bundle_path = dir.path().join("relay.cbor");
        let config_path = dir.path().join("directory.json");
        write_directory_config(&config_path, &config_for_bundle(&bundle_path));

        fs::write(&bundle_path, vec![0_u8; SRC_V2_MAX_BUNDLE_BYTES]).expect("write exact bundle");
        assert!(matches!(
            build_snapshot_from_config(&config_path),
            Err(DirectoryBuildError::CertificateVerify { .. })
        ));

        fs::write(&bundle_path, vec![0_u8; SRC_V2_MAX_BUNDLE_BYTES + 1])
            .expect("write oversized bundle");
        let err = build_snapshot_from_config(&config_path)
            .expect_err("bundle max + 1 must fail at file read");
        match err {
            DirectoryBuildError::Io { source, .. } => {
                assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
            }
            other => panic!("unexpected oversized bundle error: {other:?}"),
        }
    }

    #[test]
    fn proof_directory_inventory_limit_accepts_exact_and_rejects_plus_one() {
        let dir = tempdir().expect("tempdir");
        for index in 0..GUARD_PINNING_PROOF_DIRECTORY_MAX_ENTRIES_V1 {
            fs::write(
                dir.path().join(format!("ignored-{index:03}.txt")),
                b"ignored",
            )
            .expect("write ignored entry");
        }
        assert!(matches!(
            collect_guard_pinning_proofs_from_directory(dir.path(), &empty_snapshot()),
            Err(GuardPinningCollectError::NoProofs { .. })
        ));

        fs::write(dir.path().join("overflow.txt"), b"ignored").expect("write overflow entry");
        assert!(matches!(
            collect_guard_pinning_proofs_from_directory(dir.path(), &empty_snapshot()),
            Err(GuardPinningCollectError::TooManyEntries {
                found,
                maximum: GUARD_PINNING_PROOF_DIRECTORY_MAX_ENTRIES_V1,
                ..
            }) if found == GUARD_PINNING_PROOF_DIRECTORY_MAX_ENTRIES_V1 + 1
        ));
    }

    #[test]
    fn proof_directory_respects_remaining_relay_capacity_before_collection() {
        let dir = tempdir().expect("tempdir");
        fs::write(
            dir.path().join("proof.json"),
            minimal_guard_pinning_proof_json(""),
        )
        .expect("write proof candidate");
        assert!(matches!(
            collect_guard_pinning_proofs_from_directory_with_limit(
                dir.path(),
                &empty_snapshot(),
                0,
            ),
            Err(GuardPinningCollectError::TooManyProofs {
                found: 1,
                maximum: 0,
                ..
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn builder_inputs_and_discovered_proofs_reject_symbolic_links() {
        let dir = tempdir().expect("tempdir");

        let config_target = dir.path().join("config-target.json");
        write_directory_config(&config_target, &config_with_counts(0, 0, 0));
        let config_link = dir.path().join("config-link.json");
        symlink(&config_target, &config_link).expect("symlink config");
        assert!(matches!(
            build_snapshot_from_config(&config_link),
            Err(DirectoryBuildError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::InvalidData
        ));

        let bundle_target = dir.path().join("bundle-target.cbor");
        fs::write(&bundle_target, [0_u8]).expect("write bundle target");
        let bundle_link = dir.path().join("bundle-link.cbor");
        symlink(&bundle_target, &bundle_link).expect("symlink bundle");
        let bundle_config_path = dir.path().join("bundle-config.json");
        write_directory_config(&bundle_config_path, &config_for_bundle(&bundle_link));
        assert!(matches!(
            build_snapshot_from_config(&bundle_config_path),
            Err(DirectoryBuildError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::InvalidData
        ));

        let proof_target = dir.path().join("proof-target.json");
        fs::write(&proof_target, minimal_guard_pinning_proof_json("")).expect("write proof target");
        let proof_link = dir.path().join("proof-link.json");
        symlink(&proof_target, &proof_link).expect("symlink proof");
        assert!(matches!(
            read_guard_pinning_proof_file(&proof_link),
            Err(DirectoryBuildError::GuardPinningProofIo { source, .. })
                if source.kind() == std::io::ErrorKind::InvalidData
        ));

        let evidence = dir.path().join("evidence");
        fs::create_dir(&evidence).expect("create evidence directory");
        symlink(&proof_target, evidence.join("discovered.json")).expect("symlink discovered proof");
        assert!(matches!(
            collect_guard_pinning_proofs_from_directory(&evidence, &empty_snapshot()),
            Err(GuardPinningCollectError::NoProofs { .. })
        ));

        let evidence_link = dir.path().join("evidence-link");
        symlink(&evidence, &evidence_link).expect("symlink evidence directory");
        assert!(matches!(
            collect_guard_pinning_proofs_from_directory(&evidence_link, &empty_snapshot()),
            Err(GuardPinningCollectError::NotDirectory { .. })
        ));
    }

    #[test]
    fn build_snapshot_rejects_all_zero_issuer_ed25519_key_material() {
        let dir = tempdir().expect("tempdir");
        let config_path = dir.path().join("directory.json");
        let config = DirectoryBuildConfig {
            directory_hash_hex: None,
            published_at_unix: None,
            valid_after_unix: None,
            valid_until_unix: None,
            issuers: vec![IssuerConfig {
                label: Some("zero-issuer".to_string()),
                ed25519_hex: hex::encode([0u8; 32]),
                mldsa_hex: String::new(),
            }],
            bundles: vec![BundleConfig {
                path: PathBuf::from("unused.cbor"),
            }],
            guard_pinning_proofs_dir: None,
            guard_pinning_proofs: Vec::new(),
        };
        write_directory_config(&config_path, &config);

        let err = build_snapshot_from_config(&config_path)
            .expect_err("all-zero issuer key must fail before certificate reads");
        match err {
            DirectoryBuildError::InvalidIssuerEd25519Material { label, reason } => {
                assert_eq!(label, "zero-issuer");
                assert!(
                    reason.contains("all zero"),
                    "unexpected issuer key material error: {reason}"
                );
            }
            other => panic!("unexpected directory build error: {other:?}"),
        }
    }

    #[test]
    fn build_snapshot_requires_mldsa65_issuer_key() {
        let dir = tempdir().expect("tempdir");
        let config_path = dir.path().join("directory.json");
        let issuer_ed25519 = SigningKey::from_bytes(&[0x45; 32])
            .verifying_key()
            .to_bytes();
        let config = DirectoryBuildConfig {
            directory_hash_hex: None,
            published_at_unix: None,
            valid_after_unix: None,
            valid_until_unix: None,
            issuers: vec![IssuerConfig {
                label: Some("governance".to_string()),
                ed25519_hex: hex::encode(issuer_ed25519),
                mldsa_hex: String::new(),
            }],
            bundles: vec![BundleConfig {
                path: PathBuf::from("unused.cbor"),
            }],
            guard_pinning_proofs_dir: None,
            guard_pinning_proofs: Vec::new(),
        };
        write_directory_config(&config_path, &config);

        let err = build_snapshot_from_config(&config_path)
            .expect_err("the first-release directory builder must require ML-DSA-65");
        assert!(
            matches!(
                &err,
                DirectoryBuildError::IssuerMissingMlDsa { label }
                    if label == "governance"
            ),
            "unexpected directory build error: {err:?}"
        );
    }

    #[test]
    fn build_snapshot_from_config_roundtrip() {
        let issuer_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");
        let mut rng = StdRng::seed_from_u64(0xA55A55);
        let mut ed_seed = [0u8; 32];
        rng.fill_bytes(&mut ed_seed);
        let signing_key = SigningKey::from_bytes(&ed_seed);
        let ed_public = signing_key.verifying_key().to_bytes();
        let fingerprint = compute_issuer_fingerprint(&ed_public, issuer_keys.public_key())
            .expect("sample issuer fingerprint should compute");

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
            issuers: vec![IssuerConfig {
                label: Some("governance".to_string()),
                ed25519_hex: hex::encode(ed_public),
                mldsa_hex: hex::encode(issuer_keys.public_key()),
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
    fn build_snapshot_rejects_small_order_issuer_ed25519_key_material() {
        let dir = tempdir().expect("tempdir");
        let config_path = dir.path().join("directory.json");
        let config = DirectoryBuildConfig {
            directory_hash_hex: None,
            published_at_unix: None,
            valid_after_unix: None,
            valid_until_unix: None,
            issuers: vec![IssuerConfig {
                label: Some("weak-issuer".to_string()),
                ed25519_hex: hex::encode(SMALL_ORDER_ED25519_POINT),
                mldsa_hex: String::new(),
            }],
            bundles: vec![BundleConfig {
                path: PathBuf::from("unused.cbor"),
            }],
            guard_pinning_proofs_dir: None,
            guard_pinning_proofs: Vec::new(),
        };
        write_directory_config(&config_path, &config);

        let err = build_snapshot_from_config(&config_path)
            .expect_err("weak issuer key must fail before certificate reads");
        match err {
            DirectoryBuildError::InvalidIssuerEd25519Material { label, reason } => {
                assert_eq!(label, "weak-issuer");
                assert!(
                    reason.contains("small-order"),
                    "unexpected issuer key material error: {reason}"
                );
            }
            other => panic!("unexpected directory build error: {other:?}"),
        }
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
        let fingerprint = compute_issuer_fingerprint(&ed_public, issuer_keys.public_key())
            .expect("sample issuer fingerprint should compute");

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
            issuers: vec![IssuerConfig {
                label: Some("governance".to_string()),
                ed25519_hex: hex::encode(ed_public),
                mldsa_hex: hex::encode(issuer_keys.public_key()),
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
            snapshot_valid_until_unix: initial_bundle.snapshot.valid_until_unix,
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
        let fingerprint = compute_issuer_fingerprint(&ed_public, issuer_keys.public_key())
            .expect("sample issuer fingerprint should compute");

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
            issuers: vec![IssuerConfig {
                label: Some("governance".to_string()),
                ed25519_hex: hex::encode(ed_public),
                mldsa_hex: hex::encode(issuer_keys.public_key()),
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
            snapshot_valid_until_unix: snapshot_bundle.snapshot.valid_until_unix,
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
        let fingerprint = compute_issuer_fingerprint(&ed_public, issuer_keys.public_key())
            .expect("sample issuer fingerprint should compute");

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
            issuers: vec![IssuerConfig {
                label: Some("governance".to_string()),
                ed25519_hex: hex::encode(ed_public),
                mldsa_hex: hex::encode(issuer_keys.public_key()),
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
            snapshot_valid_until_unix: snapshot_bundle.snapshot.valid_until_unix,
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
        let fingerprint = compute_issuer_fingerprint(&ed_public, issuer_keys.public_key())
            .expect("sample issuer fingerprint should compute");

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
                .verify_signatures(
                    &new_verifying,
                    &output.keys.mldsa_public,
                    output.bundle.metadata.validation_phase,
                )
                .expect("verify");
        }
    }

    #[test]
    fn rotate_snapshot_rejects_pre_release_validation_phase() {
        let issuer_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");
        let signing_key = SigningKey::from_bytes(&[0x57; 32]);
        let ed_public = signing_key.verifying_key().to_bytes();
        let fingerprint = compute_issuer_fingerprint(&ed_public, issuer_keys.public_key())
            .expect("sample issuer fingerprint should compute");
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
            validation_phase: encode_validation_phase(CertificateValidationPhase::Phase2PreferDual),
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
        let mut rng = StdRng::seed_from_u64(0xBAD_CAFE);

        let err = rotate_snapshot(&bytes, &mut rng)
            .expect_err("rotation must not preserve a pre-release validation policy");
        assert!(
            matches!(
                &err,
                DirectoryRotateError::UnsupportedReleasePhase {
                    phase: CertificateValidationPhase::Phase2PreferDual
                }
            ),
            "unexpected directory rotation error: {err:?}"
        );
    }

    #[test]
    fn rotate_snapshot_rejects_all_zero_generated_ed25519_seed() {
        struct ZeroRng;

        impl RngCore for ZeroRng {
            fn next_u32(&mut self) -> u32 {
                0
            }

            fn next_u64(&mut self) -> u64 {
                0
            }

            fn fill_bytes(&mut self, dest: &mut [u8]) {
                dest.fill(0);
            }
        }

        impl CryptoRng for ZeroRng {}

        let issuer_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");
        let mut rng = StdRng::seed_from_u64(0xA55A56);
        let mut ed_seed = [0u8; 32];
        rng.fill_bytes(&mut ed_seed);
        let signing_key = SigningKey::from_bytes(&ed_seed);
        let ed_public = signing_key.verifying_key().to_bytes();
        let fingerprint = compute_issuer_fingerprint(&ed_public, issuer_keys.public_key())
            .expect("sample issuer fingerprint should compute");

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
        let mut rng = ZeroRng;
        let err = rotate_snapshot(&bytes, &mut rng)
            .expect_err("all-zero generated Ed25519 issuer seed must fail");

        match err {
            DirectoryRotateError::InvalidGeneratedIssuerKeyMaterial { reason } => {
                assert!(reason.contains("all zero"), "unexpected reason: {reason}");
            }
            other => panic!("unexpected directory rotation error: {other:?}"),
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
                quic_multiaddr: "/dns/relay.example/udp/443/quic".to_string(),
                tls_server_name: "relay.example".to_string(),
                tls_spki_sha256: [0xA5; 32],
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
            pq_kem_public: vec![0x66; MlKemSuite::MlKem1024.public_key_len()],
        }
    }

    fn config_with_counts(
        issuer_count: usize,
        bundle_count: usize,
        proof_count: usize,
    ) -> DirectoryBuildConfig {
        DirectoryBuildConfig {
            directory_hash_hex: None,
            published_at_unix: None,
            valid_after_unix: None,
            valid_until_unix: None,
            issuers: (0..issuer_count)
                .map(|index| IssuerConfig {
                    label: Some(format!("issuer-{index}")),
                    ed25519_hex: hex::encode([1_u8; 32]),
                    mldsa_hex: hex::encode(vec![1_u8; GUARD_DIRECTORY_ISSUER_MLDSA65_MAX_BYTES_V1]),
                })
                .collect(),
            bundles: (0..bundle_count)
                .map(|index| BundleConfig {
                    path: PathBuf::from(format!("bundle-{index}.cbor")),
                })
                .collect(),
            guard_pinning_proofs_dir: None,
            guard_pinning_proofs: (0..proof_count)
                .map(|index| PinningProofConfig {
                    path: PathBuf::from(format!("proof-{index}.json")),
                })
                .collect(),
        }
    }

    fn config_for_bundle(path: &Path) -> DirectoryBuildConfig {
        let signing_key = SigningKey::from_bytes(&[0x45; 32]);
        DirectoryBuildConfig {
            directory_hash_hex: None,
            published_at_unix: None,
            valid_after_unix: None,
            valid_until_unix: None,
            issuers: vec![IssuerConfig {
                label: Some("bounded-input-test".to_string()),
                ed25519_hex: hex::encode(signing_key.verifying_key().to_bytes()),
                mldsa_hex: hex::encode(vec![1_u8; GUARD_DIRECTORY_ISSUER_MLDSA65_MAX_BYTES_V1]),
            }],
            bundles: vec![BundleConfig {
                path: path.to_path_buf(),
            }],
            guard_pinning_proofs_dir: None,
            guard_pinning_proofs: Vec::new(),
        }
    }

    fn empty_snapshot() -> GuardDirectorySnapshotV2 {
        GuardDirectorySnapshotV2 {
            version: GUARD_DIRECTORY_VERSION_V2,
            directory_hash: [0_u8; 32],
            published_at_unix: 0,
            valid_after_unix: 0,
            valid_until_unix: 1,
            validation_phase: encode_validation_phase(
                CertificateValidationPhase::Phase3RequireDual,
            ),
            issuers: Vec::new(),
            relays: Vec::new(),
        }
    }

    fn minimal_guard_pinning_proof_json(snapshot_path: &str) -> Vec<u8> {
        format!(
            concat!(
                "{{",
                "\"version\":1,",
                "\"recorded_at_unix\":0,",
                "\"snapshot_path\":\"{}\",",
                "\"validation_phase\":\"phase3_require_dual\",",
                "\"relay_id_hex\":\"{}\",",
                "\"directory_hash_hex\":\"{}\",",
                "\"descriptor_commit_hex\":\"{}\",",
                "\"issuer_fingerprint_hex\":\"{}\",",
                "\"valid_after_unix\":0,",
                "\"valid_until_unix\":1,",
                "\"guard_weight\":1,",
                "\"bandwidth_bytes_per_sec\":1,",
                "\"reputation_weight\":1,",
                "\"pq_kem_public_hex\":\"\"",
                "}}"
            ),
            snapshot_path,
            "00".repeat(32),
            "00".repeat(32),
            "00".repeat(32),
            "00".repeat(32),
        )
        .into_bytes()
    }

    fn assert_too_many_config_entries(
        config: DirectoryBuildConfig,
        expected_field: &'static str,
        expected_found: usize,
        expected_maximum: usize,
    ) {
        assert!(matches!(
            validate_directory_build_config_limits(&config),
            Err(DirectoryBuildError::TooManyEntries {
                field,
                found,
                maximum,
            }) if field == expected_field
                && found == expected_found
                && maximum == expected_maximum
        ));
    }

    fn write_directory_config(path: &Path, config: &DirectoryBuildConfig) {
        let json_value = norito::json::to_value(config).expect("serialize config");
        let json = norito::json::to_string(&json_value).expect("encode config");
        fs::write(path, json).expect("write config");
    }
}
