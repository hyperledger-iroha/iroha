//! Trustless verification helpers for SoraNet gateway responses.
//!
//! The verifier consumes a manifest and CAR stream, rebuilds the chunk plan
//! and PoR tree, and optionally cross-checks the results against a
//! registry-ready [`PinRecordV1`]. Config is sourced from the gateway
//! verifier TOML used in the SNNet-15 pack so operators and CI share the
//! same thresholds.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use hex::encode as hex_encode;
use norito::{
    Error as NoritoError,
    json::{Map, Value},
};
use sha3::{Digest, Sha3_256};
use sorafs_manifest::{
    ManifestV1,
    pin_registry::{PinRecordV1, PinRecordValidationError},
};
use thiserror::Error;
use toml::Value as TomlValue;

use crate::{CarVerificationReport, CarVerifier, StoredChunk};

/// Configuration used to guide trustless verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustlessVerifierConfig {
    /// Schema version for the config file.
    pub version: u32,
    /// Chunk window used when streaming Merkle verification.
    pub merkle_chunk_window: u32,
    /// Maximum concurrent chunk streams.
    pub merkle_max_parallel_streams: u32,
    /// Location of the trusted setup parameters for KZG proofs.
    pub kzg_trusted_setup: String,
    /// Cache directory for KZG proof data.
    pub kzg_proof_cache: String,
    /// Maximum tolerated KZG gap in milliseconds.
    pub kzg_max_gap_ms: u32,
    /// Directory holding SDR receipts.
    pub sdr_receipt_dir: String,
    /// Maximum age for SDR receipts (seconds).
    pub sdr_max_lag_seconds: u32,
    /// Whether hybrid manifests are permitted.
    pub pipeline_allow_hybrid_manifest: bool,
    /// Whether stale cache versions must be rejected.
    pub pipeline_reject_stale_cache_versions: bool,
    /// Whether cache binding headers must be present.
    pub pipeline_verify_cache_binding_header: bool,
    /// Logging level for the verifier.
    pub logging_level: String,
    /// Toggle for emitting metrics.
    pub logging_emit_metrics: bool,
}

impl TrustlessVerifierConfig {
    /// Parse a verifier config from TOML text.
    pub fn from_toml_str(input: &str) -> Result<Self, TrustlessConfigError> {
        let value: TomlValue = toml::from_str(input)?;
        let root = value
            .as_table()
            .ok_or(TrustlessConfigError::MissingField("version"))?;
        let version = read_u32(root, "version")?;
        let merkle = read_table(root, "merkle")?;
        let kzg = read_table(root, "kzg")?;
        let sdr = read_table(root, "sdr")?;
        let pipeline = read_table(root, "pipeline")?;
        let logging = read_table(root, "logging")?;

        Ok(Self {
            version,
            merkle_chunk_window: read_u32(merkle, "chunk_window")?,
            merkle_max_parallel_streams: read_u32(merkle, "max_parallel_streams")?,
            kzg_trusted_setup: read_string(kzg, "trusted_setup")?,
            kzg_proof_cache: read_string(kzg, "proof_cache")?,
            kzg_max_gap_ms: read_u32(kzg, "max_gap_ms")?,
            sdr_receipt_dir: read_string(sdr, "receipt_dir")?,
            sdr_max_lag_seconds: read_u32(sdr, "max_lag_seconds")?,
            pipeline_allow_hybrid_manifest: read_bool(pipeline, "allow_hybrid_manifest")?,
            pipeline_reject_stale_cache_versions: read_bool(
                pipeline,
                "reject_stale_cache_versions",
            )?,
            pipeline_verify_cache_binding_header: read_bool(
                pipeline,
                "verify_cache_binding_header",
            )?,
            logging_level: read_string(logging, "level")?,
            logging_emit_metrics: read_bool(logging, "emit_metrics")?,
        })
    }

    /// Load a verifier config from the provided path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, TrustlessConfigError> {
        let path_ref = path.as_ref();
        let contents = fs::read_to_string(path_ref).map_err(|source| TrustlessConfigError::Io {
            path: path_ref.to_path_buf(),
            source,
        })?;
        Self::from_toml_str(&contents)
    }
}

/// Errors surfaced while parsing the trustless verifier config.
#[derive(Debug, Error)]
pub enum TrustlessConfigError {
    /// Failed to parse TOML.
    #[error("failed to parse trustless verifier config: {0}")]
    Parse(#[from] toml::de::Error),
    /// Required field missing from the config.
    #[error("missing required `{0}` in trustless verifier config")]
    MissingField(&'static str),
    /// Encountered an invalid value.
    #[error("invalid `{field}` value: {reason}")]
    InvalidField {
        /// Field name being parsed.
        field: &'static str,
        /// Reason why parsing failed.
        reason: String,
    },
    /// Failed to read the config file.
    #[error("failed to read trustless verifier config at `{path}`: {source}")]
    Io {
        /// Path that could not be read.
        path: PathBuf,
        /// Source IO error.
        source: io::Error,
    },
}

/// Errors surfaced while verifying trustless gateway payloads.
#[derive(Debug, Error)]
pub enum TrustlessVerificationError {
    /// Config version mismatched expected schema.
    #[error("unsupported config version {found} (expected {expected})")]
    ConfigVersionMismatch { expected: u32, found: u32 },
    /// CAR verification failed.
    #[error("CAR verification failed: {0}")]
    Car(#[from] crate::verifier::CarVerifyError),
    /// Manifest hashing failed.
    #[error("failed to hash manifest: {0}")]
    ManifestDigest(#[from] NoritoError),
    /// PoR tree was empty for a non-empty payload.
    #[error("PoR root missing from verified payload")]
    MissingPorRoot,
    /// Pin record failed validation.
    #[error("pin record invalid: {0}")]
    PinRecordInvalid(#[from] PinRecordValidationError),
    /// Pin record points to a different manifest CID.
    #[error("pin record manifest CID mismatch (expected {expected}, found {found})")]
    PinRecordManifestCidMismatch { expected: String, found: String },
    /// Pin record profile handle mismatched the manifest.
    #[error("pin record profile handle mismatch (expected {expected}, found {found})")]
    PinRecordProfileMismatch { expected: String, found: String },
    /// Pin record chunk plan digest mismatched the reconstructed plan.
    #[error("pin record chunk plan digest mismatch (expected {expected}, found {found})")]
    PinRecordChunkPlanMismatch { expected: String, found: String },
    /// Pin record PoR root mismatched the reconstructed tree.
    #[error("pin record PoR root mismatch (expected {expected}, found {found})")]
    PinRecordPorRootMismatch { expected: String, found: String },
}

/// Output of a trustless verification run.
#[derive(Debug)]
pub struct TrustlessVerificationOutcome {
    manifest_digest: [u8; 32],
    manifest_cid: Vec<u8>,
    chunk_plan_digest: [u8; 32],
    por_root: [u8; 32],
    profile_handle: String,
    /// Underlying CAR verification output.
    pub report: CarVerificationReport,
}

impl TrustlessVerificationOutcome {
    /// Hex-encoded manifest digest.
    #[must_use]
    pub fn manifest_digest_hex(&self) -> String {
        hex_encode(self.manifest_digest)
    }

    /// Hex-encoded CAR archive digest (already validated against the manifest).
    #[must_use]
    pub fn car_digest_hex(&self) -> String {
        hex_encode(self.report.stats.car_archive_digest.as_bytes())
    }

    /// Hex-encoded payload digest.
    #[must_use]
    pub fn payload_digest_hex(&self) -> String {
        hex_encode(self.report.chunk_store.payload_digest().as_bytes())
    }

    /// Hex-encoded chunk plan digest (SHA3-256).
    #[must_use]
    pub fn chunk_plan_digest_hex(&self) -> String {
        hex_encode(self.chunk_plan_digest)
    }

    /// Hex-encoded PoR root.
    #[must_use]
    pub fn por_root_hex(&self) -> String {
        hex_encode(self.por_root)
    }

    /// Canonical chunk profile handle (namespace.name@semver).
    #[must_use]
    pub fn profile_handle(&self) -> &str {
        &self.profile_handle
    }

    /// Serialize a short JSON summary of the verification outcome.
    #[must_use]
    pub fn to_summary_json(&self) -> Value {
        let mut root = Map::new();
        root.insert(
            "manifest_digest_blake3_hex".into(),
            Value::from(self.manifest_digest_hex()),
        );
        root.insert(
            "car_digest_blake3_hex".into(),
            Value::from(self.car_digest_hex()),
        );
        root.insert(
            "payload_digest_blake3_hex".into(),
            Value::from(self.payload_digest_hex()),
        );
        root.insert(
            "chunk_plan_digest_sha3_hex".into(),
            Value::from(self.chunk_plan_digest_hex()),
        );
        root.insert("por_root_hex".into(), Value::from(self.por_root_hex()));
        root.insert(
            "profile_handle".into(),
            Value::from(self.profile_handle.clone()),
        );
        root.insert(
            "chunk_count".into(),
            Value::from(self.report.chunk_store.chunks().len() as u64),
        );
        root.insert(
            "payload_bytes".into(),
            Value::from(self.report.stats.payload_bytes),
        );
        root.insert("car_size".into(), Value::from(self.report.stats.car_size));
        Value::Object(root)
    }

    /// Validate the outcome against a registry pin record.
    pub fn validate_pin_record(&self, pin: &PinRecordV1) -> Result<(), TrustlessVerificationError> {
        pin.validate()?;

        let expected_cid_hex = hex_encode(&self.manifest_cid);
        let found_cid_hex = hex_encode(&pin.manifest_cid);
        if pin.manifest_cid != self.manifest_cid {
            return Err(TrustlessVerificationError::PinRecordManifestCidMismatch {
                expected: expected_cid_hex,
                found: found_cid_hex,
            });
        }

        if pin.profile_handle != self.profile_handle {
            return Err(TrustlessVerificationError::PinRecordProfileMismatch {
                expected: self.profile_handle.clone(),
                found: pin.profile_handle.clone(),
            });
        }

        let expected_plan_hex = self.chunk_plan_digest_hex();
        let found_plan_hex = hex_encode(pin.chunk_plan_digest);
        if pin.chunk_plan_digest != self.chunk_plan_digest {
            return Err(TrustlessVerificationError::PinRecordChunkPlanMismatch {
                expected: expected_plan_hex,
                found: found_plan_hex,
            });
        }

        let expected_root_hex = self.por_root_hex();
        let found_root_hex = hex_encode(pin.por_root);
        if pin.por_root != self.por_root {
            return Err(TrustlessVerificationError::PinRecordPorRootMismatch {
                expected: expected_root_hex,
                found: found_root_hex,
            });
        }

        Ok(())
    }
}

/// Trustless CAR verifier wrapper that enforces the gateway config.
#[derive(Debug)]
pub struct TrustlessVerifier {
    config: TrustlessVerifierConfig,
}

impl TrustlessVerifier {
    /// Construct a verifier using the supplied config.
    #[must_use]
    pub fn new(config: TrustlessVerifierConfig) -> Self {
        Self { config }
    }

    /// Verify a full CAR stream against the given manifest.
    pub fn verify_full(
        &self,
        manifest: &ManifestV1,
        car_bytes: &[u8],
    ) -> Result<TrustlessVerificationOutcome, TrustlessVerificationError> {
        if self.config.version != 1 {
            return Err(TrustlessVerificationError::ConfigVersionMismatch {
                expected: 1,
                found: self.config.version,
            });
        }

        let manifest_digest = manifest.digest()?;
        let report = CarVerifier::verify_full_car(manifest, car_bytes)?;
        let por_root = *report.chunk_store.por_tree().root();
        if por_root.iter().all(|&byte| byte == 0) {
            return Err(TrustlessVerificationError::MissingPorRoot);
        }

        let chunk_plan_digest = chunk_plan_digest_sha3(report.chunk_store.chunks());
        let profile_handle = format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        );

        Ok(TrustlessVerificationOutcome {
            manifest_digest: *manifest_digest.as_bytes(),
            manifest_cid: manifest.root_cid.clone(),
            chunk_plan_digest,
            por_root,
            profile_handle,
            report,
        })
    }
}

fn chunk_plan_digest_sha3(chunks: &[StoredChunk]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    for chunk in chunks {
        hasher.update(chunk.offset.to_le_bytes());
        hasher.update(u64::from(chunk.length).to_le_bytes());
        hasher.update(chunk.blake3);
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(digest.as_ref());
    out
}

fn read_table<'a>(
    table: &'a toml::map::Map<String, TomlValue>,
    key: &'static str,
) -> Result<&'a toml::map::Map<String, TomlValue>, TrustlessConfigError> {
    table
        .get(key)
        .and_then(TomlValue::as_table)
        .ok_or(TrustlessConfigError::MissingField(key))
}

fn read_u32(
    table: &toml::map::Map<String, TomlValue>,
    key: &'static str,
) -> Result<u32, TrustlessConfigError> {
    table
        .get(key)
        .and_then(TomlValue::as_integer)
        .ok_or(TrustlessConfigError::MissingField(key))
        .and_then(|value| {
            u32::try_from(value).map_err(|err| TrustlessConfigError::InvalidField {
                field: key,
                reason: err.to_string(),
            })
        })
}

fn read_bool(
    table: &toml::map::Map<String, TomlValue>,
    key: &'static str,
) -> Result<bool, TrustlessConfigError> {
    table
        .get(key)
        .and_then(TomlValue::as_bool)
        .ok_or(TrustlessConfigError::MissingField(key))
}

fn read_string(
    table: &toml::map::Map<String, TomlValue>,
    key: &'static str,
) -> Result<String, TrustlessConfigError> {
    let value = table
        .get(key)
        .and_then(TomlValue::as_str)
        .ok_or(TrustlessConfigError::MissingField(key))?;
    if value.trim().is_empty() {
        return Err(TrustlessConfigError::InvalidField {
            field: key,
            reason: "value must not be empty".to_owned(),
        });
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use sorafs_manifest::pin_registry::PinRecordValidationError;

    use super::*;

    #[test]
    fn parses_gateway_config() {
        let config = TrustlessVerifierConfig::from_toml_str(
            r#"
version = 1

[merkle]
chunk_window = 16
max_parallel_streams = 4

[kzg]
trusted_setup = "/tmp/kzg.params"
proof_cache = "/tmp/cache"
max_gap_ms = 100

[sdr]
receipt_dir = "/tmp/sdr"
max_lag_seconds = 8

[pipeline]
allow_hybrid_manifest = false
reject_stale_cache_versions = true
verify_cache_binding_header = true

[logging]
level = "info"
emit_metrics = true
"#,
        )
        .expect("config parses");

        assert_eq!(config.version, 1);
        assert_eq!(config.merkle_chunk_window, 16);
        assert_eq!(config.merkle_max_parallel_streams, 4);
        assert_eq!(config.kzg_trusted_setup, "/tmp/kzg.params");
        assert_eq!(config.kzg_proof_cache, "/tmp/cache");
        assert_eq!(config.kzg_max_gap_ms, 100);
        assert_eq!(config.sdr_receipt_dir, "/tmp/sdr");
        assert_eq!(config.sdr_max_lag_seconds, 8);
        assert!(!config.pipeline_allow_hybrid_manifest);
        assert!(config.pipeline_reject_stale_cache_versions);
        assert!(config.pipeline_verify_cache_binding_header);
        assert_eq!(config.logging_level, "info");
        assert!(config.logging_emit_metrics);
    }

    #[test]
    fn pin_record_validation_reports_mismatch() {
        let manifest_cid = vec![0xAA, 0xBB];
        let outcome = TrustlessVerificationOutcome {
            manifest_digest: [0x11; 32],
            manifest_cid: manifest_cid.clone(),
            chunk_plan_digest: [0x22; 32],
            por_root: [0x33; 32],
            profile_handle: "sorafs.sf1@1.0.0".to_string(),
            report: CarVerificationReport {
                stats: crate::CarWriteStats {
                    payload_bytes: 0,
                    chunk_count: 0,
                    car_size: 0,
                    car_payload_digest: blake3::hash(&[]),
                    car_archive_digest: blake3::hash(&[]),
                    car_cid: Vec::new(),
                    root_cids: Vec::new(),
                    dag_codec: 0,
                    chunk_profile: sorafs_chunker::ChunkProfile::DEFAULT,
                },
                chunk_store: crate::ChunkStore::new(),
            },
        };

        let mut pin = PinRecordV1 {
            manifest_cid,
            chunk_plan_digest: [0x22; 32],
            por_root: [0x33; 32],
            profile_handle: "sorafs.sf1@1.0.0".to_string(),
            approved_at: 1,
            retention_epoch: 2,
            pin_policy: sorafs_manifest::PinPolicy::default(),
            successor_of: None,
            governance_envelope_hash: None,
        };

        // Manifest CID mismatch should be surfaced first.
        pin.manifest_cid = vec![0xCC];
        let err = outcome.validate_pin_record(&pin).expect_err("cid mismatch");
        assert!(matches!(
            err,
            TrustlessVerificationError::PinRecordManifestCidMismatch { .. }
        ));

        // Fix manifest, break profile handle.
        pin.manifest_cid = outcome.manifest_cid.clone();
        pin.profile_handle = "sorafs.sf2@1.0.0".to_string();
        let err = outcome
            .validate_pin_record(&pin)
            .expect_err("profile mismatch");
        assert!(matches!(
            err,
            TrustlessVerificationError::PinRecordProfileMismatch { .. }
        ));

        // Fix profile, break chunk plan digest.
        pin.profile_handle = outcome.profile_handle.clone();
        pin.chunk_plan_digest = [0x55; 32];
        let err = outcome
            .validate_pin_record(&pin)
            .expect_err("plan mismatch");
        assert!(matches!(
            err,
            TrustlessVerificationError::PinRecordChunkPlanMismatch { .. }
        ));

        // Fix plan, break PoR.
        pin.chunk_plan_digest = outcome.chunk_plan_digest;
        pin.por_root = [0x99; 32];
        let err = outcome.validate_pin_record(&pin).expect_err("por mismatch");
        assert!(matches!(
            err,
            TrustlessVerificationError::PinRecordPorRootMismatch { .. }
        ));

        // Empty profile handle should be caught by PinRecord validation.
        pin.por_root = outcome.por_root;
        pin.profile_handle.clear();
        let err = outcome
            .validate_pin_record(&pin)
            .expect_err("pin record validation failure");
        assert!(matches!(
            err,
            TrustlessVerificationError::PinRecordInvalid(
                PinRecordValidationError::UnknownProfileHandle { .. }
            )
        ));

        // Restore and ensure the happy path succeeds.
        pin.profile_handle = outcome.profile_handle.clone();
        let result = outcome.validate_pin_record(&pin);
        assert!(result.is_ok(), "expected successful validation");
    }
}
