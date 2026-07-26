//! Trustless verification helpers for SoraNet gateway responses.
//!
//! The verifier consumes a manifest and CAR stream, rebuilds the chunk plan
//! and PoR tree, and optionally cross-checks the results against a
//! finalized native [`PinManifestFinalizedRecordV1`]. Config is sourced from
//! the gateway verifier TOML used in the SNNet-15 pack so operators and CI
//! share the same thresholds.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use hex::encode as hex_encode;
use iroha_data_model::sorafs::pin_registry::{PinManifestFinalizedRecordV1, PinStatus};
use norito::{
    Error as NoritoError,
    json::{Map, Value},
};
use sorafs_manifest::ManifestV1;
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
    /// Reconstructed chunk plan disagreed with the mandatory manifest commitment.
    #[error("manifest chunk plan digest mismatch (expected {expected}, found {found})")]
    ManifestChunkPlanMismatch { expected: String, found: String },
    /// Reconstructed PoR tree disagreed with the mandatory manifest commitment.
    #[error("manifest PoR root mismatch (expected {expected}, found {found})")]
    ManifestPorRootMismatch { expected: String, found: String },
    /// Finalized pin query returned an inert cursor.
    #[error("finalized pin cursor is inert (height {height}, block hash {block_hash_hex})")]
    FinalizedPinCursorInvalid { height: u64, block_hash_hex: String },
    /// Finalized pin is not approved for serving.
    #[error("finalized pin status is not approved (found {found:?})")]
    FinalizedPinStatusInvalid { found: PinStatus },
    /// Finalized pin points to a different manifest envelope digest.
    #[error("finalized pin manifest digest mismatch (expected {expected}, found {found})")]
    FinalizedPinManifestDigestMismatch { expected: String, found: String },
    /// Finalized pin points to a different manifest CID.
    #[error("finalized pin manifest CID mismatch (expected {expected}, found {found})")]
    FinalizedPinManifestCidMismatch { expected: String, found: String },
    /// Finalized pin profile handle mismatched the manifest.
    #[error("finalized pin profile handle mismatch (expected {expected}, found {found})")]
    FinalizedPinProfileMismatch { expected: String, found: String },
    /// Finalized pin chunk plan digest mismatched the reconstructed plan.
    #[error("finalized pin chunk plan digest mismatch (expected {expected}, found {found})")]
    FinalizedPinChunkPlanMismatch { expected: String, found: String },
    /// Finalized pin PoR root mismatched the reconstructed tree.
    #[error("finalized pin PoR root mismatch (expected {expected}, found {found})")]
    FinalizedPinPorRootMismatch { expected: String, found: String },
    /// Finalized pin payload length mismatched the verified CAR.
    #[error("finalized pin content length mismatch (expected {expected}, found {found})")]
    FinalizedPinContentLengthMismatch { expected: u64, found: u64 },
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

    /// Validate the outcome against an approved, finalized native pin record.
    ///
    /// This comparison does not authenticate record provenance. Callers must
    /// obtain `pin` from an authenticated ledger query or a verified state
    /// proof; arbitrary local files are not trust anchors.
    pub fn validate_finalized_pin(
        &self,
        pin: &PinManifestFinalizedRecordV1,
    ) -> Result<(), TrustlessVerificationError> {
        if pin.finalized_cursor.height == 0
            || pin
                .finalized_cursor
                .block_hash
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(TrustlessVerificationError::FinalizedPinCursorInvalid {
                height: pin.finalized_cursor.height,
                block_hash_hex: hex_encode(pin.finalized_cursor.block_hash),
            });
        }
        if !matches!(pin.manifest.status, PinStatus::Approved(_)) {
            return Err(TrustlessVerificationError::FinalizedPinStatusInvalid {
                found: pin.manifest.status,
            });
        }

        let expected_manifest_digest_hex = self.manifest_digest_hex();
        let found_manifest_digest_hex = hex_encode(pin.manifest.digest.as_bytes());
        if pin.manifest.digest.as_bytes() != &self.manifest_digest {
            return Err(
                TrustlessVerificationError::FinalizedPinManifestDigestMismatch {
                    expected: expected_manifest_digest_hex,
                    found: found_manifest_digest_hex,
                },
            );
        }

        let expected_cid_hex = hex_encode(&self.manifest_cid);
        let found_cid_hex = hex_encode(pin.manifest.root_cid.as_bytes());
        if pin.manifest.root_cid.as_bytes().as_slice() != self.manifest_cid {
            return Err(
                TrustlessVerificationError::FinalizedPinManifestCidMismatch {
                    expected: expected_cid_hex,
                    found: found_cid_hex,
                },
            );
        }

        let found_profile_handle = pin.manifest.chunker.to_handle();
        if found_profile_handle != self.profile_handle {
            return Err(TrustlessVerificationError::FinalizedPinProfileMismatch {
                expected: self.profile_handle.clone(),
                found: found_profile_handle,
            });
        }

        let expected_plan_hex = self.chunk_plan_digest_hex();
        let found_plan_hex = hex_encode(pin.manifest.chunk_digest_sha3_256);
        if pin.manifest.chunk_digest_sha3_256 != self.chunk_plan_digest {
            return Err(TrustlessVerificationError::FinalizedPinChunkPlanMismatch {
                expected: expected_plan_hex,
                found: found_plan_hex,
            });
        }

        let expected_root_hex = self.por_root_hex();
        let found_root_hex = hex_encode(pin.manifest.por_root);
        if pin.manifest.por_root != self.por_root {
            return Err(TrustlessVerificationError::FinalizedPinPorRootMismatch {
                expected: expected_root_hex,
                found: found_root_hex,
            });
        }

        if pin.manifest.content_length != self.report.stats.payload_bytes {
            return Err(
                TrustlessVerificationError::FinalizedPinContentLengthMismatch {
                    expected: self.report.stats.payload_bytes,
                    found: pin.manifest.content_length,
                },
            );
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
        if chunk_plan_digest != manifest.chunk_digest_sha3_256 {
            return Err(TrustlessVerificationError::ManifestChunkPlanMismatch {
                expected: hex_encode(manifest.chunk_digest_sha3_256),
                found: hex_encode(chunk_plan_digest),
            });
        }
        if por_root != manifest.por_root {
            return Err(TrustlessVerificationError::ManifestPorRootMismatch {
                expected: hex_encode(manifest.por_root),
                found: hex_encode(por_root),
            });
        }
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
    sorafs_chunker::compute_chunk_plan_digest_sha3(
        chunks
            .iter()
            .map(|chunk| (chunk.offset, u64::from(chunk.length), chunk.blake3)),
    )
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
    use iroha_data_model::{
        account::AccountId,
        metadata::Metadata,
        sorafs::pin_registry::{
            ChunkerProfileHandle, ManifestDigest, ManifestRootCid, PinManifestFinalizedCursorV1,
            PinManifestRecord, PinPolicy,
        },
    };

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
    fn finalized_pin_validation_reports_mismatch() {
        let manifest_cid = sorafs_manifest::canonical_manifest_root_cid([0xAA; 32]);
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

        let mut manifest = PinManifestRecord::new(
            ManifestDigest::new(outcome.manifest_digest),
            ManifestRootCid::try_from(manifest_cid).expect("canonical root CID"),
            ChunkerProfileHandle {
                profile_id: 1,
                namespace: "sorafs".into(),
                name: "sf1".into(),
                semver: "1.0.0".into(),
                multihash_code: 0x1f,
            },
            [0x22; 32],
            [0x33; 32],
            0,
            PinPolicy::default(),
            AccountId::new(
                "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
                    .parse()
                    .expect("public key"),
            ),
            1,
            None,
            None,
            Metadata::default(),
        );
        manifest.approve(2, None);
        let mut pin = PinManifestFinalizedRecordV1 {
            finalized_cursor: PinManifestFinalizedCursorV1 {
                height: 7,
                block_hash: [0x44; 32],
            },
            manifest,
        };

        pin.finalized_cursor.height = 0;
        let err = outcome
            .validate_finalized_pin(&pin)
            .expect_err("zero cursor height");
        assert!(matches!(
            err,
            TrustlessVerificationError::FinalizedPinCursorInvalid { .. }
        ));
        pin.finalized_cursor.height = 7;

        pin.manifest.status = PinStatus::Pending;
        let err = outcome
            .validate_finalized_pin(&pin)
            .expect_err("pending pin");
        assert!(matches!(
            err,
            TrustlessVerificationError::FinalizedPinStatusInvalid { .. }
        ));
        pin.manifest.approve(2, None);

        pin.manifest.digest = ManifestDigest::new([0xCC; 32]);
        let err = outcome
            .validate_finalized_pin(&pin)
            .expect_err("manifest digest mismatch");
        assert!(matches!(
            err,
            TrustlessVerificationError::FinalizedPinManifestDigestMismatch { .. }
        ));
        pin.manifest.digest = ManifestDigest::new(outcome.manifest_digest);

        // Manifest CID mismatch should be surfaced first.
        pin.manifest.root_cid =
            ManifestRootCid::try_from(sorafs_manifest::canonical_manifest_root_cid([0xCC; 32]))
                .expect("canonical root CID");
        let err = outcome
            .validate_finalized_pin(&pin)
            .expect_err("cid mismatch");
        assert!(matches!(
            err,
            TrustlessVerificationError::FinalizedPinManifestCidMismatch { .. }
        ));

        // Fix manifest, break profile handle.
        pin.manifest.root_cid =
            ManifestRootCid::try_from(outcome.manifest_cid.clone()).expect("canonical root CID");
        pin.manifest.chunker.name = "sf2".to_owned();
        let err = outcome
            .validate_finalized_pin(&pin)
            .expect_err("profile mismatch");
        assert!(matches!(
            err,
            TrustlessVerificationError::FinalizedPinProfileMismatch { .. }
        ));

        // Fix profile, break chunk plan digest.
        pin.manifest.chunker.name = "sf1".to_owned();
        pin.manifest.chunk_digest_sha3_256 = [0x55; 32];
        let err = outcome
            .validate_finalized_pin(&pin)
            .expect_err("plan mismatch");
        assert!(matches!(
            err,
            TrustlessVerificationError::FinalizedPinChunkPlanMismatch { .. }
        ));

        // Fix plan, break PoR.
        pin.manifest.chunk_digest_sha3_256 = outcome.chunk_plan_digest;
        pin.manifest.por_root = [0x99; 32];
        let err = outcome
            .validate_finalized_pin(&pin)
            .expect_err("por mismatch");
        assert!(matches!(
            err,
            TrustlessVerificationError::FinalizedPinPorRootMismatch { .. }
        ));

        // Content length remains bound to the verified CAR.
        pin.manifest.por_root = outcome.por_root;
        pin.manifest.content_length = 1;
        let err = outcome
            .validate_finalized_pin(&pin)
            .expect_err("content length mismatch");
        assert!(matches!(
            err,
            TrustlessVerificationError::FinalizedPinContentLengthMismatch { .. }
        ));

        // Restore and ensure the happy path succeeds.
        pin.manifest.content_length = outcome.report.stats.payload_bytes;
        let result = outcome.validate_finalized_pin(&pin);
        assert!(result.is_ok(), "expected successful validation");
    }
}
