//! Deterministic, resource-bounded SoraFS moderation model loading and inference.
//!
//! The runner executes only canonical [`ModerationModelArtifactV1`] data. Model
//! files are opened beneath an explicit artefact root without following symbolic
//! links, verified against their signed fingerprints, decoded with hard Norito
//! limits, and retained as immutable in-memory values before a service binds.

use std::{
    fs::{self, File, Metadata},
    io::{self, Read},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};

#[cfg(feature = "cli-orchestrator")]
use iroha_crypto::{KeyPair, PrivateKey, PublicKey, SignatureOf};
use iroha_data_model::sorafs::moderation::{
    MODERATION_MODEL_FEATURE_COUNT_V1, MODERATION_MODEL_MAX_ARTIFACT_BYTES_V1,
    MODERATION_MODEL_MAX_CALIBRATION_KNOTS_V1, MODERATION_MODEL_MAX_INPUT_BYTES_V1,
    MODERATION_REPRO_MAX_BPS, ModerationModelArtifactError, ModerationModelArtifactV1,
    ModerationModelFingerprintV1, ModerationModelScoreV1, ModerationReproManifestV1,
    is_canonical_moderation_artifact_path_v1,
};
#[cfg(feature = "cli-orchestrator")]
use iroha_data_model::sorafs::moderation::{
    MODERATION_SIGNED_RESULT_VERSION_V1, ModerationSignedResultError,
    ModerationSignedScreeningBodyV1, ModerationSignedScreeningResultV1, ModerationTrustPolicyError,
    ModerationTrustPolicyV1,
};
use norito::core::DecodeLimits;
use thiserror::Error;

const MODEL_DECODE_MAX_ELEMENTS: usize =
    MODERATION_MODEL_FEATURE_COUNT_V1 + MODERATION_MODEL_MAX_CALIBRATION_KNOTS_V1 * 2 + 32;
const MODEL_DECODE_MAX_DEPTH: usize = 16;

/// One model artefact and its signed combination weight, loaded into memory.
#[derive(Clone, Debug)]
struct LoadedModerationModelV1 {
    fingerprint: ModerationModelFingerprintV1,
    artifact: ModerationModelArtifactV1,
    weight_bps: u16,
}

#[cfg(unix)]
type FileIdentity = (u64, u64, u64, i64, i64);

#[cfg(not(unix))]
type ValidatedArtifactRoot = PathBuf;

#[cfg(unix)]
#[derive(Debug)]
struct ValidatedArtifactRoot {
    configured: PathBuf,
    canonical: PathBuf,
    identity: FileIdentity,
    _handle: File,
}

/// Immutable manifest-bound moderation engine shared by all runner transports.
#[derive(Clone, Debug)]
pub struct LoadedModerationRunnerV1 {
    manifest: ModerationReproManifestV1,
    models: Vec<LoadedModerationModelV1>,
    max_payload_bytes: u32,
}

/// Complete deterministic result returned by the shared model engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModerationInferenceV1 {
    /// Weighted aggregate score in basis points.
    pub combined_score_bps: u16,
    /// Per-model scores in canonical model-id order.
    pub model_scores: Vec<ModerationModelScoreV1>,
}

/// Verified deterministic runner plus externally anchored result-signing state.
#[cfg(feature = "cli-orchestrator")]
#[derive(Debug)]
pub struct LoadedModerationSigningRunnerV1 {
    runner: LoadedModerationRunnerV1,
    trust_policy: ModerationTrustPolicyV1,
    trust_anchors: std::collections::BTreeSet<PublicKey>,
    minimum_governance_quorum: u16,
    signer_public_key: PublicKey,
    signer_private_key: PrivateKey,
}

/// Errors raised while preparing, loading, or executing model artefacts.
#[derive(Debug, Error)]
pub enum ModerationRunnerError {
    /// Signed manifest validation failed.
    #[error("invalid moderation reproducibility manifest: {0}")]
    InvalidManifest(#[from] iroha_data_model::sorafs::moderation::ModerationReproValidationError),
    /// Signed manifest digest is not derived from its canonical body.
    #[error("moderation manifest digest is not the canonical body digest")]
    ManifestDigestMismatch,
    /// Runner binary does not match the hash committed by the signed manifest.
    #[error("moderation runner binary hash does not match the signed runner_hash")]
    RunnerHashMismatch,
    /// Configured artefact root is unsafe or invalid.
    #[error("invalid moderation artefact root `{path}`: {reason}")]
    InvalidArtifactRoot {
        /// Configured root.
        path: PathBuf,
        /// Rejection reason.
        reason: String,
    },
    /// A model path escaped the configured artefact root or traversed a symlink.
    #[error("unsafe moderation model path `{path}`: {reason}")]
    UnsafeArtifactPath {
        /// Rejected path.
        path: PathBuf,
        /// Rejection reason.
        reason: String,
    },
    /// File system access failed.
    #[error("failed to access moderation model `{path}`: {source}")]
    ArtifactIo {
        /// Model path.
        path: PathBuf,
        /// Underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// File size does not exactly match the signed fingerprint.
    #[error("moderation model `{path}` has {found} bytes; fingerprint requires {expected}")]
    ArtifactSize {
        /// Model path.
        path: PathBuf,
        /// Signed byte length.
        expected: u64,
        /// Observed byte length.
        found: u64,
    },
    /// File changed identity while it was being opened or read.
    #[error("moderation model `{path}` changed while it was being loaded")]
    ArtifactChanged {
        /// Model path.
        path: PathBuf,
    },
    /// File digest does not match the signed fingerprint.
    #[error("moderation model `{path}` digest does not match its fingerprint")]
    ArtifactDigest {
        /// Model path.
        path: PathBuf,
    },
    /// Norito decoding failed within the fixed resource budget.
    #[error("failed to decode moderation model `{path}`: {reason}")]
    ArtifactDecode {
        /// Model path.
        path: PathBuf,
        /// Decoder failure.
        reason: String,
    },
    /// Artefact is not encoded in the single canonical Norito representation.
    #[error("moderation model `{path}` is not canonically encoded")]
    NonCanonicalArtifact {
        /// Model path.
        path: PathBuf,
    },
    /// Decoded artefact violates engine invariants.
    #[error("invalid moderation model `{path}`: {source}")]
    InvalidArtifact {
        /// Model path.
        path: PathBuf,
        /// Artefact validation failure.
        #[source]
        source: ModerationModelArtifactError,
    },
    /// Decoded artefact and signed fingerprint disagree.
    #[error("moderation model `{path}` does not match fingerprint field `{field}`")]
    FingerprintMismatch {
        /// Model path.
        path: PathBuf,
        /// Mismatched field.
        field: &'static str,
    },
    /// Payload is empty.
    #[error("moderation payload must not be empty")]
    EmptyPayload,
    /// Payload exceeds either the service or model limit.
    #[error("moderation payload has {found} bytes; maximum is {maximum}")]
    PayloadTooLarge {
        /// Observed payload size.
        found: usize,
        /// Effective hard limit.
        maximum: u32,
    },
    /// A checked integer operation failed during inference.
    #[error("moderation integer inference overflowed its validated bounds")]
    ArithmeticOverflow,
    /// Canonical artefact encoding failed.
    #[error("failed to encode moderation model artefact: {0}")]
    ArtifactEncode(String),
    /// Externally anchored moderation trust policy is invalid.
    #[cfg(feature = "cli-orchestrator")]
    #[error("invalid moderation trust policy: {0}")]
    InvalidTrustPolicy(#[from] ModerationTrustPolicyError),
    /// Result-signing private key could not be converted into a checked key pair.
    #[cfg(feature = "cli-orchestrator")]
    #[error("invalid moderation result-signing key: {0}")]
    InvalidSigningKey(String),
    /// Result expiry arithmetic overflowed.
    #[cfg(feature = "cli-orchestrator")]
    #[error("moderation signed-result expiry overflowed")]
    ResultExpiryOverflow,
    /// Canonical signed-result body encoding or signing failed.
    #[cfg(feature = "cli-orchestrator")]
    #[error("failed to sign moderation result: {0}")]
    ResultSigning(String),
    /// The freshly produced signed result did not pass its own trust checks.
    #[cfg(feature = "cli-orchestrator")]
    #[error("produced invalid signed moderation result: {0}")]
    InvalidSignedResult(#[from] ModerationSignedResultError),
}

impl LoadedModerationRunnerV1 {
    /// Load and fully verify every signed artefact below `artifact_root`.
    ///
    /// No listener should be bound until this function succeeds.
    pub fn load_verified(
        manifest: ModerationReproManifestV1,
        artifact_root: impl AsRef<Path>,
        observed_runner_hash: [u8; 32],
    ) -> Result<Self, ModerationRunnerError> {
        manifest.validate()?;
        let computed_manifest_digest = manifest.computed_manifest_digest().map_err(|error| {
            ModerationRunnerError::ArtifactDecode {
                path: PathBuf::from("<manifest>"),
                reason: error.to_string(),
            }
        })?;
        if computed_manifest_digest != manifest.body.manifest_digest {
            return Err(ModerationRunnerError::ManifestDigestMismatch);
        }
        if observed_runner_hash != manifest.body.runner_hash {
            return Err(ModerationRunnerError::RunnerHashMismatch);
        }
        let root = validate_artifact_root(artifact_root.as_ref())?;
        let mut models = Vec::new();
        models
            .try_reserve_exact(manifest.body.models.len())
            .map_err(|error| ModerationRunnerError::ArtifactDecode {
                path: PathBuf::from("<manifest>"),
                reason: format!("failed to reserve bounded model table: {error}"),
            })?;
        let mut max_payload_bytes = MODERATION_MODEL_MAX_INPUT_BYTES_V1;
        for fingerprint in &manifest.body.models {
            verify_artifact_root(&root)?;
            let artifact = load_model(&root, fingerprint)?;
            verify_artifact_root(&root)?;
            max_payload_bytes = max_payload_bytes.min(artifact.max_input_bytes);
            models.push(LoadedModerationModelV1 {
                fingerprint: fingerprint.clone(),
                artifact,
                weight_bps: fingerprint.weight.unwrap_or(MODERATION_REPRO_MAX_BPS),
            });
        }
        Ok(Self {
            manifest,
            models,
            max_payload_bytes,
        })
    }

    /// Return the verified signed manifest used by this engine.
    #[must_use]
    pub fn manifest(&self) -> &ModerationReproManifestV1 {
        &self.manifest
    }

    /// Return the strictest input bound across every loaded model.
    #[must_use]
    pub fn max_payload_bytes(&self) -> u32 {
        self.max_payload_bytes
    }

    /// Re-encode the immutable verified artefacts for supervised bundle output.
    ///
    /// Returning bytes from the in-memory values avoids reopening attacker-
    /// mutable source paths after verification. Every encoded row is checked
    /// again against its signed length and digest before it is returned.
    pub fn canonical_artifacts(&self) -> Result<Vec<(String, Vec<u8>)>, ModerationRunnerError> {
        let mut artifacts = Vec::new();
        artifacts
            .try_reserve_exact(self.models.len())
            .map_err(|error| ModerationRunnerError::ArtifactEncode(error.to_string()))?;
        for model in &self.models {
            let bytes = norito::to_bytes(&model.artifact)
                .map_err(|error| ModerationRunnerError::ArtifactEncode(error.to_string()))?;
            if u64::try_from(bytes.len()).ok() != Some(model.fingerprint.artifact_bytes) {
                return Err(ModerationRunnerError::ArtifactSize {
                    path: PathBuf::from(&model.fingerprint.artifact_path),
                    expected: model.fingerprint.artifact_bytes,
                    found: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                });
            }
            if blake3::hash(&bytes).as_bytes() != &model.fingerprint.artifact_digest {
                return Err(ModerationRunnerError::ArtifactDigest {
                    path: PathBuf::from(&model.fingerprint.artifact_path),
                });
            }
            artifacts.push((model.fingerprint.artifact_path.clone(), bytes));
        }
        Ok(artifacts)
    }

    /// Execute all models and combine their calibrated results deterministically.
    pub fn infer(
        &self,
        payload: &[u8],
        service_max_payload_bytes: u32,
    ) -> Result<ModerationInferenceV1, ModerationRunnerError> {
        if payload.is_empty() {
            return Err(ModerationRunnerError::EmptyPayload);
        }
        let effective_maximum = self
            .max_payload_bytes
            .min(service_max_payload_bytes)
            .min(MODERATION_MODEL_MAX_INPUT_BYTES_V1);
        let effective_maximum_usize = usize::try_from(effective_maximum)
            .map_err(|_| ModerationRunnerError::ArithmeticOverflow)?;
        if payload.len() > effective_maximum_usize {
            return Err(ModerationRunnerError::PayloadTooLarge {
                found: payload.len(),
                maximum: effective_maximum,
            });
        }

        let features = extract_features(payload)?;
        let mut model_scores = Vec::new();
        model_scores
            .try_reserve_exact(self.models.len())
            .map_err(|_| ModerationRunnerError::ArithmeticOverflow)?;
        let mut weighted_score = 0_u64;
        let mut total_weight = 0_u64;
        for model in &self.models {
            let score_bps = evaluate_model(&model.artifact, &features)?;
            weighted_score = weighted_score
                .checked_add(u64::from(score_bps) * u64::from(model.weight_bps))
                .ok_or(ModerationRunnerError::ArithmeticOverflow)?;
            total_weight = total_weight
                .checked_add(u64::from(model.weight_bps))
                .ok_or(ModerationRunnerError::ArithmeticOverflow)?;
            model_scores.push(ModerationModelScoreV1 {
                model_id: model.artifact.model_id,
                artifact_digest: model.fingerprint.artifact_digest,
                score_bps,
            });
        }
        let combined_score_bps = weighted_score_half_up(weighted_score, total_weight)?;
        Ok(ModerationInferenceV1 {
            combined_score_bps,
            model_scores,
        })
    }
}

#[cfg(feature = "cli-orchestrator")]
impl LoadedModerationSigningRunnerV1 {
    /// Bind a fully verified engine to an externally authenticated trust policy
    /// and one policy-authorized signing key.
    pub fn from_verified(
        runner: LoadedModerationRunnerV1,
        trust_policy: ModerationTrustPolicyV1,
        trust_anchors: std::collections::BTreeSet<PublicKey>,
        minimum_governance_quorum: u16,
        signer_private_key: PrivateKey,
        now_unix: u64,
    ) -> Result<Self, ModerationRunnerError> {
        trust_policy.validate_with_trust_anchors(
            runner.manifest(),
            &trust_anchors,
            minimum_governance_quorum,
            now_unix,
        )?;
        let signer_keypair = KeyPair::from_private_key(signer_private_key.clone())
            .map_err(|error| ModerationRunnerError::InvalidSigningKey(error.to_string()))?;
        let signer_public_key = signer_keypair.public_key().clone();
        let authorization = trust_policy
            .body
            .trusted_signers
            .iter()
            .find(|signer| signer.public_key == signer_public_key)
            .ok_or_else(|| {
                ModerationRunnerError::InvalidSigningKey(
                    "derived public key is absent from the trusted signer policy".to_string(),
                )
            })?;
        if now_unix < authorization.valid_from_unix
            || now_unix >= authorization.valid_until_unix
            || authorization
                .revoked_at_unix
                .is_some_and(|revoked| now_unix >= revoked)
        {
            return Err(ModerationRunnerError::InvalidSigningKey(
                "derived public key is not currently authorized".to_string(),
            ));
        }
        Ok(Self {
            runner,
            trust_policy,
            trust_anchors,
            minimum_governance_quorum,
            signer_public_key,
            signer_private_key,
        })
    }

    /// Return the verified deterministic engine.
    #[must_use]
    pub fn runner(&self) -> &LoadedModerationRunnerV1 {
        &self.runner
    }

    /// Return the externally authenticated trust policy.
    #[must_use]
    pub fn trust_policy(&self) -> &ModerationTrustPolicyV1 {
        &self.trust_policy
    }

    /// Return the public key used for signed results.
    #[must_use]
    pub fn signer_public_key(&self) -> &PublicKey {
        &self.signer_public_key
    }

    /// Infer and sign one manifest-, policy-, payload-, and timestamp-bound
    /// result. The caller must supply trusted wall-clock time, not a client-
    /// controlled timestamp.
    pub fn screen_signed(
        &self,
        payload: &[u8],
        service_max_payload_bytes: u32,
        subject: &str,
        notes: Option<String>,
        now_unix: u64,
    ) -> Result<ModerationSignedScreeningResultV1, ModerationRunnerError> {
        self.trust_policy.validate_with_trust_anchors(
            self.runner.manifest(),
            &self.trust_anchors,
            self.minimum_governance_quorum,
            now_unix,
        )?;
        let authorization = self
            .trust_policy
            .body
            .trusted_signers
            .iter()
            .find(|signer| signer.public_key == self.signer_public_key)
            .ok_or_else(|| {
                ModerationRunnerError::InvalidSigningKey(
                    "signer authorization disappeared from immutable policy".to_string(),
                )
            })?;
        let ttl_expiry = now_unix
            .checked_add(self.trust_policy.body.max_result_ttl_secs)
            .ok_or(ModerationRunnerError::ResultExpiryOverflow)?;
        let mut expires_at_unix = ttl_expiry
            .min(self.trust_policy.body.valid_until_unix)
            .min(authorization.valid_until_unix);
        if let Some(revoked_at_unix) = authorization.revoked_at_unix {
            expires_at_unix = expires_at_unix.min(revoked_at_unix);
        }
        if expires_at_unix <= now_unix {
            return Err(ModerationRunnerError::InvalidSigningKey(
                "signer authorization has no remaining result lifetime".to_string(),
            ));
        }

        let inference = self.runner.infer(payload, service_max_payload_bytes)?;
        let manifest = self.runner.manifest();
        let verdict = moderation_verdict(
            inference.combined_score_bps,
            manifest.body.thresholds.quarantine,
            manifest.body.thresholds.escalate,
        );
        let mut body = ModerationSignedScreeningBodyV1 {
            schema_version: MODERATION_SIGNED_RESULT_VERSION_V1,
            manifest_id: manifest.body.manifest_id,
            manifest_digest: manifest.body.manifest_digest,
            runner_hash: manifest.body.runner_hash,
            trust_policy_id: self.trust_policy.body.policy_id,
            trust_policy_digest: self.trust_policy.body.policy_digest,
            subject: subject.to_string(),
            subject_digest: *blake3::hash(payload).as_bytes(),
            model_scores: inference.model_scores,
            combined_score_bps: inference.combined_score_bps,
            verdict: verdict.to_string(),
            screened_at_unix: now_unix,
            expires_at_unix,
            policy_digest: manifest
                .body
                .computed_screening_policy_digest()
                .map_err(|error| ModerationRunnerError::ResultSigning(error.to_string()))?,
            evidence_digest: [0; 32],
            notes,
        };
        body.refresh_evidence_digest()
            .map_err(|error| ModerationRunnerError::ResultSigning(error.to_string()))?;
        let signature = SignatureOf::try_new(&self.signer_private_key, &body)
            .map_err(|error| ModerationRunnerError::ResultSigning(error.to_string()))?;
        let result = ModerationSignedScreeningResultV1 {
            body,
            signer_public_key: self.signer_public_key.clone(),
            signature,
        };
        result.validate(manifest, &self.trust_policy, now_unix)?;
        Ok(result)
    }
}

#[cfg(feature = "cli-orchestrator")]
fn moderation_verdict(score_bps: u16, quarantine_bps: u16, escalate_bps: u16) -> &'static str {
    if score_bps >= escalate_bps {
        "escalate"
    } else if score_bps >= quarantine_bps {
        "quarantine"
    } else {
        "pass"
    }
}

fn weighted_score_half_up(
    weighted_score: u64,
    total_weight: u64,
) -> Result<u16, ModerationRunnerError> {
    if total_weight == 0 {
        return Err(ModerationRunnerError::ArithmeticOverflow);
    }
    let rounded_score = weighted_score
        .checked_add(total_weight / 2)
        .ok_or(ModerationRunnerError::ArithmeticOverflow)?
        / total_weight;
    u16::try_from(rounded_score).map_err(|_| ModerationRunnerError::ArithmeticOverflow)
}

/// Canonically encode and validate an artefact for release tooling and tests.
pub fn canonical_model_artifact_bytes(
    artifact: &ModerationModelArtifactV1,
) -> Result<Vec<u8>, ModerationRunnerError> {
    artifact
        .validate()
        .map_err(|source| ModerationRunnerError::InvalidArtifact {
            path: PathBuf::from("<memory>"),
            source,
        })?;
    norito::to_bytes(artifact)
        .map_err(|error| ModerationRunnerError::ArtifactEncode(error.to_string()))
}

/// Build the exact signed fingerprint and canonical bytes for an artefact.
pub fn fingerprint_model_artifact(
    artifact_path: impl Into<String>,
    artifact: &ModerationModelArtifactV1,
    weight: Option<u16>,
) -> Result<(ModerationModelFingerprintV1, Vec<u8>), ModerationRunnerError> {
    let artifact_path = artifact_path.into();
    if !is_canonical_moderation_artifact_path_v1(&artifact_path) {
        return Err(ModerationRunnerError::UnsafeArtifactPath {
            path: PathBuf::from(artifact_path),
            reason: "path is not a canonical portable relative artefact path".to_owned(),
        });
    }
    if weight.is_some_and(|value| value > MODERATION_REPRO_MAX_BPS) {
        return Err(ModerationRunnerError::ArithmeticOverflow);
    }
    let bytes = canonical_model_artifact_bytes(artifact)?;
    let artifact_bytes =
        u64::try_from(bytes.len()).map_err(|_| ModerationRunnerError::ArithmeticOverflow)?;
    let fingerprint = ModerationModelFingerprintV1 {
        model_id: artifact.model_id,
        artifact_path,
        artifact_bytes,
        artifact_digest: *blake3::hash(&bytes).as_bytes(),
        weights_digest: artifact.behaviour_digest(),
        engine: artifact.engine,
        feature_profile: artifact.feature_profile,
        calibration_knot_count: u16::try_from(artifact.calibration.len())
            .map_err(|_| ModerationRunnerError::ArithmeticOverflow)?,
        max_input_bytes: artifact.max_input_bytes,
        max_operations: artifact.max_operations,
        working_memory_bytes: artifact.working_memory_bytes,
        weight,
    };
    Ok((fingerprint, bytes))
}

#[cfg(not(unix))]
fn validate_artifact_root(_root: &Path) -> Result<PathBuf, ModerationRunnerError> {
    Err(ModerationRunnerError::InvalidArtifactRoot {
        path: _root.to_path_buf(),
        reason: "secure no-follow artefact loading is unavailable on this platform".to_owned(),
    })
}

#[cfg(unix)]
fn validate_artifact_root(root: &Path) -> Result<ValidatedArtifactRoot, ModerationRunnerError> {
    let metadata = fs::symlink_metadata(root).map_err(|source| {
        ModerationRunnerError::InvalidArtifactRoot {
            path: root.to_path_buf(),
            reason: source.to_string(),
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ModerationRunnerError::InvalidArtifactRoot {
            path: root.to_path_buf(),
            reason: "root must be a real directory, not a symbolic link".to_owned(),
        });
    }
    let identity = file_identity(&metadata);
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    let handle =
        options
            .open(root)
            .map_err(|source| ModerationRunnerError::InvalidArtifactRoot {
                path: root.to_path_buf(),
                reason: source.to_string(),
            })?;
    let opened =
        handle
            .metadata()
            .map_err(|source| ModerationRunnerError::InvalidArtifactRoot {
                path: root.to_path_buf(),
                reason: source.to_string(),
            })?;
    if file_identity(&opened) != identity || !opened.is_dir() {
        return Err(ModerationRunnerError::InvalidArtifactRoot {
            path: root.to_path_buf(),
            reason: "root changed while it was being opened".to_owned(),
        });
    }
    let canonical =
        fs::canonicalize(root).map_err(|source| ModerationRunnerError::InvalidArtifactRoot {
            path: root.to_path_buf(),
            reason: source.to_string(),
        })?;
    let canonical_metadata = fs::symlink_metadata(&canonical).map_err(|source| {
        ModerationRunnerError::InvalidArtifactRoot {
            path: root.to_path_buf(),
            reason: source.to_string(),
        }
    })?;
    if canonical_metadata.file_type().is_symlink() || file_identity(&canonical_metadata) != identity
    {
        return Err(ModerationRunnerError::InvalidArtifactRoot {
            path: root.to_path_buf(),
            reason: "canonical root identity does not match the opened directory".to_owned(),
        });
    }
    Ok(ValidatedArtifactRoot {
        configured: root.to_path_buf(),
        canonical,
        identity,
        _handle: handle,
    })
}

#[cfg(unix)]
fn verify_artifact_root(root: &ValidatedArtifactRoot) -> Result<(), ModerationRunnerError> {
    for path in [&root.configured, &root.canonical] {
        let metadata = fs::symlink_metadata(path).map_err(|source| {
            ModerationRunnerError::InvalidArtifactRoot {
                path: path.clone(),
                reason: source.to_string(),
            }
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || file_identity(&metadata) != root.identity
        {
            return Err(ModerationRunnerError::InvalidArtifactRoot {
                path: path.clone(),
                reason: "root identity changed during artefact loading".to_owned(),
            });
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_artifact_root(_root: &PathBuf) -> Result<(), ModerationRunnerError> {
    unreachable!("non-Unix artifact roots fail closed during validation")
}

#[cfg(unix)]
fn load_model(
    root: &ValidatedArtifactRoot,
    fingerprint: &ModerationModelFingerprintV1,
) -> Result<ModerationModelArtifactV1, ModerationRunnerError> {
    load_model_with_post_open_hook(root, fingerprint, |_| {})
}

#[cfg(unix)]
fn load_model_with_post_open_hook<F>(
    root: &ValidatedArtifactRoot,
    fingerprint: &ModerationModelFingerprintV1,
    post_open_hook: F,
) -> Result<ModerationModelArtifactV1, ModerationRunnerError>
where
    F: FnOnce(&Path),
{
    let path = root.canonical.join(&fingerprint.artifact_path);
    reject_symlink_components(&root.canonical, &fingerprint.artifact_path)?;
    let canonical =
        fs::canonicalize(&path).map_err(|source| ModerationRunnerError::ArtifactIo {
            path: path.clone(),
            source,
        })?;
    if !canonical.starts_with(&root.canonical) {
        return Err(ModerationRunnerError::UnsafeArtifactPath {
            path,
            reason: "canonical path escapes the configured artefact root".to_owned(),
        });
    }

    let before =
        fs::symlink_metadata(&canonical).map_err(|source| ModerationRunnerError::ArtifactIo {
            path: canonical.clone(),
            source,
        })?;
    if !before.is_file() || before.file_type().is_symlink() {
        return Err(ModerationRunnerError::UnsafeArtifactPath {
            path: canonical,
            reason: "artefact must be a regular file".to_owned(),
        });
    }
    reject_linked_artifact(&canonical, &before)?;
    verify_size(&canonical, &before, fingerprint.artifact_bytes)?;
    let before_identity = file_identity(&before);

    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    let file = options
        .open(&canonical)
        .map_err(|source| ModerationRunnerError::ArtifactIo {
            path: canonical.clone(),
            source,
        })?;
    let opened = file
        .metadata()
        .map_err(|source| ModerationRunnerError::ArtifactIo {
            path: canonical.clone(),
            source,
        })?;
    if file_identity(&opened) != before_identity {
        return Err(ModerationRunnerError::ArtifactChanged { path: canonical });
    }
    reject_linked_artifact(&canonical, &opened)?;
    verify_size(&canonical, &opened, fingerprint.artifact_bytes)?;
    post_open_hook(&canonical);
    let bytes = read_exact_bounded(file, fingerprint.artifact_bytes, &canonical)?;
    let after =
        fs::symlink_metadata(&canonical).map_err(|source| ModerationRunnerError::ArtifactIo {
            path: canonical.clone(),
            source,
        })?;
    if after.file_type().is_symlink()
        || !after.is_file()
        || file_identity(&after) != before_identity
    {
        return Err(ModerationRunnerError::ArtifactChanged { path: canonical });
    }
    reject_linked_artifact(&canonical, &after)?;
    verify_artifact_root(root)?;
    if blake3::hash(&bytes).as_bytes() != &fingerprint.artifact_digest {
        return Err(ModerationRunnerError::ArtifactDigest { path: canonical });
    }

    let artifact_limit = usize::try_from(MODERATION_MODEL_MAX_ARTIFACT_BYTES_V1)
        .map_err(|_| ModerationRunnerError::ArithmeticOverflow)?;
    let limits = DecodeLimits::new(
        MODERATION_MODEL_FEATURE_COUNT_V1,
        artifact_limit,
        MODEL_DECODE_MAX_ELEMENTS,
        artifact_limit,
        MODEL_DECODE_MAX_DEPTH,
    );
    let artifact: ModerationModelArtifactV1 = norito::decode_from_bytes_with_limits(&bytes, limits)
        .map_err(|error| ModerationRunnerError::ArtifactDecode {
            path: canonical.clone(),
            reason: error.to_string(),
        })?;
    let canonical_bytes =
        norito::to_bytes(&artifact).map_err(|error| ModerationRunnerError::ArtifactDecode {
            path: canonical.clone(),
            reason: error.to_string(),
        })?;
    if canonical_bytes != bytes {
        return Err(ModerationRunnerError::NonCanonicalArtifact { path: canonical });
    }
    artifact
        .validate()
        .map_err(|source| ModerationRunnerError::InvalidArtifact {
            path: canonical.clone(),
            source,
        })?;
    verify_fingerprint(&canonical, fingerprint, &artifact)?;
    Ok(artifact)
}

#[cfg(not(unix))]
fn load_model(
    root: &ValidatedArtifactRoot,
    _fingerprint: &ModerationModelFingerprintV1,
) -> Result<ModerationModelArtifactV1, ModerationRunnerError> {
    Err(ModerationRunnerError::InvalidArtifactRoot {
        path: root.clone(),
        reason: "secure no-follow artefact loading is unavailable on this platform".to_owned(),
    })
}

fn reject_symlink_components(root: &Path, relative: &str) -> Result<(), ModerationRunnerError> {
    let mut current = root.to_path_buf();
    for component in relative.split('/') {
        current.push(component);
        let metadata =
            fs::symlink_metadata(&current).map_err(|source| ModerationRunnerError::ArtifactIo {
                path: current.clone(),
                source,
            })?;
        if metadata.file_type().is_symlink() {
            return Err(ModerationRunnerError::UnsafeArtifactPath {
                path: current,
                reason: "symbolic links are forbidden in artefact paths".to_owned(),
            });
        }
    }
    Ok(())
}

fn verify_size(
    path: &Path,
    metadata: &Metadata,
    expected: u64,
) -> Result<(), ModerationRunnerError> {
    if metadata.len() != expected {
        return Err(ModerationRunnerError::ArtifactSize {
            path: path.to_path_buf(),
            expected,
            found: metadata.len(),
        });
    }
    Ok(())
}

fn read_exact_bounded(
    file: File,
    expected: u64,
    path: &Path,
) -> Result<Vec<u8>, ModerationRunnerError> {
    let capacity = usize::try_from(expected).map_err(|_| ModerationRunnerError::ArtifactSize {
        path: path.to_path_buf(),
        expected,
        found: u64::MAX,
    })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|source| ModerationRunnerError::ArtifactIo {
            path: path.to_path_buf(),
            source: io::Error::new(
                io::ErrorKind::OutOfMemory,
                format!("failed to reserve bounded artefact buffer: {source}"),
            ),
        })?;
    file.take(expected.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| ModerationRunnerError::ArtifactIo {
            path: path.to_path_buf(),
            source,
        })?;
    if bytes.len() != capacity {
        return Err(ModerationRunnerError::ArtifactSize {
            path: path.to_path_buf(),
            expected,
            found: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        });
    }
    Ok(bytes)
}

#[cfg(unix)]
fn reject_linked_artifact(path: &Path, metadata: &Metadata) -> Result<(), ModerationRunnerError> {
    if metadata.nlink() != 1 {
        return Err(ModerationRunnerError::UnsafeArtifactPath {
            path: path.to_path_buf(),
            reason: format!(
                "artefact must have exactly one hard link; observed {}",
                metadata.nlink()
            ),
        });
    }
    Ok(())
}

fn verify_fingerprint(
    path: &Path,
    fingerprint: &ModerationModelFingerprintV1,
    artifact: &ModerationModelArtifactV1,
) -> Result<(), ModerationRunnerError> {
    let mismatch = if artifact.model_id != fingerprint.model_id {
        Some("model_id")
    } else if artifact.engine != fingerprint.engine {
        Some("engine")
    } else if artifact.feature_profile != fingerprint.feature_profile {
        Some("feature_profile")
    } else if usize::from(fingerprint.calibration_knot_count) != artifact.calibration.len() {
        Some("calibration_knot_count")
    } else if artifact.max_input_bytes != fingerprint.max_input_bytes {
        Some("max_input_bytes")
    } else if artifact.max_operations != fingerprint.max_operations {
        Some("max_operations")
    } else if artifact.working_memory_bytes != fingerprint.working_memory_bytes {
        Some("working_memory_bytes")
    } else if artifact.behaviour_digest() != fingerprint.weights_digest {
        Some("weights_digest")
    } else {
        None
    };
    if let Some(field) = mismatch {
        return Err(ModerationRunnerError::FingerprintMismatch {
            path: path.to_path_buf(),
            field,
        });
    }
    Ok(())
}

fn extract_features(
    payload: &[u8],
) -> Result<[u64; MODERATION_MODEL_FEATURE_COUNT_V1], ModerationRunnerError> {
    let mut counts = [0_u64; MODERATION_MODEL_FEATURE_COUNT_V1];
    for (index, byte) in payload.iter().copied().enumerate() {
        counts[usize::from(byte)] = counts[usize::from(byte)]
            .checked_add(1)
            .ok_or(ModerationRunnerError::ArithmeticOverflow)?;
        if index > 0 {
            let previous = usize::from(payload[index - 1]);
            let current = usize::from(byte);
            let bin = 256 + (previous * 251 + current * 17) % 256;
            counts[bin] = counts[bin]
                .checked_add(1)
                .ok_or(ModerationRunnerError::ArithmeticOverflow)?;
        }
    }
    let unigram_denominator =
        u64::try_from(payload.len()).map_err(|_| ModerationRunnerError::ArithmeticOverflow)?;
    let bigram_denominator = u64::try_from(payload.len().saturating_sub(1))
        .map_err(|_| ModerationRunnerError::ArithmeticOverflow)?;
    for (index, value) in counts.iter_mut().enumerate() {
        let denominator = if index < 256 {
            unigram_denominator
        } else {
            bigram_denominator
        };
        *value = if denominator == 0 {
            0
        } else {
            value
                .checked_mul(u64::from(MODERATION_REPRO_MAX_BPS))
                .ok_or(ModerationRunnerError::ArithmeticOverflow)?
                / denominator
        };
    }
    Ok(counts)
}

fn evaluate_model(
    artifact: &ModerationModelArtifactV1,
    features: &[u64; MODERATION_MODEL_FEATURE_COUNT_V1],
) -> Result<u16, ModerationRunnerError> {
    let raw = artifact.weights.iter().zip(features).try_fold(
        i128::from(artifact.bias),
        |total, (weight, feature)| {
            total
                .checked_add(i128::from(*weight) * i128::from(*feature))
                .ok_or(ModerationRunnerError::ArithmeticOverflow)
        },
    )?;
    let raw = i64::try_from(raw).map_err(|_| ModerationRunnerError::ArithmeticOverflow)?;
    calibrate(raw, &artifact.calibration)
}

fn calibrate(
    raw: i64,
    knots: &[iroha_data_model::sorafs::moderation::ModerationCalibrationKnotV1],
) -> Result<u16, ModerationRunnerError> {
    let first = knots
        .first()
        .ok_or(ModerationRunnerError::ArithmeticOverflow)?;
    if raw <= first.input {
        return Ok(first.score_bps);
    }
    for window in knots.windows(2) {
        let lower = window[0];
        let upper = window[1];
        if raw <= upper.input {
            let input_delta = i128::from(raw) - i128::from(lower.input);
            let input_span = i128::from(upper.input) - i128::from(lower.input);
            let score_delta = i128::from(upper.score_bps) - i128::from(lower.score_bps);
            let interpolated = i128::from(lower.score_bps)
                + input_delta
                    .checked_mul(score_delta)
                    .ok_or(ModerationRunnerError::ArithmeticOverflow)?
                    / input_span;
            return u16::try_from(interpolated)
                .map_err(|_| ModerationRunnerError::ArithmeticOverflow);
        }
    }
    Ok(knots
        .last()
        .ok_or(ModerationRunnerError::ArithmeticOverflow)?
        .score_bps)
}

#[cfg(unix)]
fn file_identity(metadata: &Metadata) -> (u64, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}

#[cfg(unix)]
fn set_no_follow_flag(options: &mut OpenOptions) {
    options.custom_flags(platform_no_follow_flag());
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const fn platform_no_follow_flag() -> i32 {
    0o400000
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
const fn platform_no_follow_flag() -> i32 {
    0x100
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
const fn platform_no_follow_flag() -> i32 {
    0
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{KeyPair, SignatureOf};
    use iroha_data_model::sorafs::moderation::{
        MODERATION_MODEL_ARTIFACT_VERSION_V1, MODERATION_MODEL_WORKING_MEMORY_BYTES_V1,
        MODERATION_TRUST_POLICY_VERSION_V1, ModerationCalibrationKnotV1,
        ModerationFeatureProfileV1, ModerationModelEngineV1, ModerationReproBodyV1,
        ModerationReproSignatureV1, ModerationSeedMaterialV1, ModerationThresholdsV1,
        ModerationTrustPolicyBodyV1, ModerationTrustPolicySignatureV1, ModerationTrustedSignerV1,
        moderation_model_required_operations_v1,
    };
    use tempfile::tempdir;

    use super::*;

    fn artifact(model_id: [u8; 16], weight: i32) -> ModerationModelArtifactV1 {
        let calibration = vec![
            ModerationCalibrationKnotV1 {
                input: -10_000,
                score_bps: 0,
            },
            ModerationCalibrationKnotV1 {
                input: 10_000,
                score_bps: 10_000,
            },
        ];
        let mut weights = vec![0; MODERATION_MODEL_FEATURE_COUNT_V1];
        weights[usize::from(b'a')] = weight;
        ModerationModelArtifactV1 {
            schema_version: MODERATION_MODEL_ARTIFACT_VERSION_V1,
            engine: ModerationModelEngineV1::DeterministicLinearV1,
            feature_profile: ModerationFeatureProfileV1::ByteHistogramAndBigramV1,
            model_id,
            max_input_bytes: 1024,
            max_operations: moderation_model_required_operations_v1(1024, calibration.len())
                .expect("operation budget"),
            working_memory_bytes: MODERATION_MODEL_WORKING_MEMORY_BYTES_V1,
            bias: 0,
            weights,
            calibration,
        }
    }

    fn signed_manifest(models: Vec<ModerationModelFingerprintV1>) -> ModerationReproManifestV1 {
        let mut body = ModerationReproBodyV1 {
            schema_version: 1,
            manifest_id: [1; 16],
            manifest_digest: [2; 32],
            runner_hash: [3; 32],
            runtime_version: "integer-runner-v1".to_owned(),
            issued_at_unix: 1,
            seed_material: ModerationSeedMaterialV1 {
                domain_tag: "sorafs:moderation:v1".to_owned(),
                seed_version: 1,
                run_nonce: [4; 32],
            },
            thresholds: ModerationThresholdsV1 {
                quarantine: 4_000,
                escalate: 7_000,
            },
            models,
            notes: None,
        };
        body.manifest_digest = ModerationReproManifestV1 {
            body: body.clone(),
            signatures: Vec::new(),
        }
        .computed_manifest_digest()
        .expect("manifest digest");
        let key = KeyPair::try_random().expect("key");
        let signature = SignatureOf::try_new(key.private_key(), &body).expect("signature");
        ModerationReproManifestV1 {
            body,
            signatures: vec![ModerationReproSignatureV1 {
                role: "test".to_owned(),
                public_key: key.public_key().clone(),
                signature,
            }],
        }
    }

    fn signed_trust_policy(
        manifest: &ModerationReproManifestV1,
        governance: &KeyPair,
        runner: &KeyPair,
        revoked_at_unix: Option<u64>,
    ) -> ModerationTrustPolicyV1 {
        let mut body = ModerationTrustPolicyBodyV1 {
            schema_version: MODERATION_TRUST_POLICY_VERSION_V1,
            policy_id: [8; 16],
            policy_digest: [0; 32],
            manifest_id: manifest.body.manifest_id,
            manifest_digest: manifest.body.manifest_digest,
            runner_hash: manifest.body.runner_hash,
            issued_at_unix: 10,
            valid_from_unix: 20,
            valid_until_unix: 1_000,
            result_quorum: 1,
            governance_quorum: 1,
            max_result_age_secs: 120,
            max_result_ttl_secs: 60,
            max_clock_skew_secs: 5,
            trusted_signers: vec![ModerationTrustedSignerV1 {
                role: "runner".to_string(),
                public_key: runner.public_key().clone(),
                valid_from_unix: 20,
                valid_until_unix: 1_000,
                revoked_at_unix,
            }],
            notes: None,
        };
        body.refresh_policy_digest().expect("trust policy digest");
        ModerationTrustPolicyV1 {
            signatures: vec![ModerationTrustPolicySignatureV1 {
                role: "governance".to_string(),
                public_key: governance.public_key().clone(),
                signature: SignatureOf::try_new(governance.private_key(), &body)
                    .expect("trust policy signature"),
            }],
            body,
        }
    }

    fn load_raw_artifact_error(
        invalid_artifact: &ModerationModelArtifactV1,
    ) -> ModerationRunnerError {
        let root = tempdir().expect("root");
        let valid_artifact = artifact([1; 16], 1);
        let (mut fingerprint, _) =
            fingerprint_model_artifact("model.norito", &valid_artifact, None)
                .expect("valid baseline fingerprint");
        let bytes = norito::to_bytes(invalid_artifact).expect("encode intentionally invalid model");
        fingerprint.artifact_bytes = u64::try_from(bytes.len()).expect("fixture size fits u64");
        fingerprint.artifact_digest = *blake3::hash(&bytes).as_bytes();
        fs::write(root.path().join("model.norito"), bytes).expect("write invalid model");
        LoadedModerationRunnerV1::load_verified(
            signed_manifest(vec![fingerprint]),
            root.path(),
            [3; 32],
        )
        .expect_err("invalid raw artefact must fail closed")
    }

    fn constant_artifact(model_id: [u8; 16], score_bps: u16) -> ModerationModelArtifactV1 {
        let calibration = vec![
            ModerationCalibrationKnotV1 {
                input: -1,
                score_bps,
            },
            ModerationCalibrationKnotV1 {
                input: 1,
                score_bps,
            },
        ];
        ModerationModelArtifactV1 {
            schema_version: MODERATION_MODEL_ARTIFACT_VERSION_V1,
            engine: ModerationModelEngineV1::DeterministicLinearV1,
            feature_profile: ModerationFeatureProfileV1::ByteHistogramAndBigramV1,
            model_id,
            max_input_bytes: 1024,
            max_operations: moderation_model_required_operations_v1(1024, calibration.len())
                .expect("operation budget"),
            working_memory_bytes: MODERATION_MODEL_WORKING_MEMORY_BYTES_V1,
            bias: 0,
            weights: vec![0; MODERATION_MODEL_FEATURE_COUNT_V1],
            calibration,
        }
    }

    #[test]
    fn loads_and_scores_canonical_models_deterministically() {
        let root = tempdir().expect("root");
        let artifact = artifact([1; 16], 1);
        let (fingerprint, bytes) =
            fingerprint_model_artifact("model.norito", &artifact, Some(10_000))
                .expect("fingerprint");
        fs::write(root.path().join("model.norito"), &bytes).expect("write model");
        let runner = LoadedModerationRunnerV1::load_verified(
            signed_manifest(vec![fingerprint]),
            root.path(),
            [3; 32],
        )
        .expect("load runner");

        let first = runner.infer(b"aaaaaaaa", 1024).expect("score");
        let second = runner.infer(b"aaaaaaaa", 1024).expect("score");
        assert_eq!(first, second);
        assert_eq!(first.model_scores.len(), 1);
        assert_eq!(first.model_scores[0].score_bps, 10_000);
        assert_eq!(first.combined_score_bps, 10_000);
        assert_eq!(
            runner.canonical_artifacts().expect("bundle artefacts"),
            vec![("model.norito".to_owned(), bytes)]
        );
    }

    #[test]
    fn signing_runner_emits_policy_bound_fresh_result() {
        let root = tempdir().expect("root");
        let model = constant_artifact([1; 16], 5_000);
        let (fingerprint, bytes) =
            fingerprint_model_artifact("model.norito", &model, Some(10_000)).expect("fingerprint");
        fs::write(root.path().join("model.norito"), bytes).expect("write model");
        let manifest = signed_manifest(vec![fingerprint]);
        let governance = KeyPair::try_random().expect("governance key");
        let signing_key = KeyPair::try_random().expect("runner key");
        let policy = signed_trust_policy(&manifest, &governance, &signing_key, None);
        let anchors = std::iter::once(governance.public_key().clone()).collect();
        let runner =
            LoadedModerationRunnerV1::load_verified(manifest.clone(), root.path(), [3; 32])
                .expect("load runner");
        let signing_runner = LoadedModerationSigningRunnerV1::from_verified(
            runner,
            policy.clone(),
            anchors,
            1,
            signing_key.private_key().clone(),
            100,
        )
        .expect("bind signer");

        let result = signing_runner
            .screen_signed(
                b"payload",
                1024,
                "cid:production-subject",
                Some("audited".to_string()),
                100,
            )
            .expect("signed screening");
        assert_eq!(result.signer_public_key, signing_key.public_key().clone());
        assert_eq!(result.body.screened_at_unix, 100);
        assert_eq!(result.body.expires_at_unix, 160);
        assert_eq!(result.body.combined_score_bps, 5_000);
        assert_eq!(result.body.verdict, "quarantine");
        assert_eq!(
            result.body.subject_digest,
            *blake3::hash(b"payload").as_bytes()
        );
        result
            .validate(&manifest, &policy, 100)
            .expect("result independently verifies");
    }

    #[test]
    fn signing_runner_rejects_untrusted_and_revoked_keys() {
        let root = tempdir().expect("root");
        let model = constant_artifact([1; 16], 5_000);
        let (fingerprint, bytes) =
            fingerprint_model_artifact("model.norito", &model, Some(10_000)).expect("fingerprint");
        fs::write(root.path().join("model.norito"), bytes).expect("write model");
        let manifest = signed_manifest(vec![fingerprint]);
        let governance = KeyPair::try_random().expect("governance key");
        let authorized = KeyPair::try_random().expect("authorized key");
        let unauthorized = KeyPair::try_random().expect("unauthorized key");
        let policy = signed_trust_policy(&manifest, &governance, &authorized, None);
        let anchors = std::iter::once(governance.public_key().clone()).collect();

        let runner =
            LoadedModerationRunnerV1::load_verified(manifest.clone(), root.path(), [3; 32])
                .expect("load runner");
        assert!(matches!(
            LoadedModerationSigningRunnerV1::from_verified(
                runner,
                policy,
                anchors,
                1,
                unauthorized.private_key().clone(),
                100,
            )
            .expect_err("key absent from policy must fail"),
            ModerationRunnerError::InvalidSigningKey(_)
        ));

        let revoked_policy = signed_trust_policy(&manifest, &governance, &authorized, Some(100));
        let anchors = std::iter::once(governance.public_key().clone()).collect();
        let runner = LoadedModerationRunnerV1::load_verified(manifest, root.path(), [3; 32])
            .expect("reload runner");
        assert!(matches!(
            LoadedModerationSigningRunnerV1::from_verified(
                runner,
                revoked_policy,
                anchors,
                1,
                authorized.private_key().clone(),
                100,
            )
            .expect_err("revoked key must fail at startup"),
            ModerationRunnerError::InvalidSigningKey(_)
        ));
    }

    #[test]
    fn signing_runner_clips_result_expiry_to_future_revocation() {
        let root = tempdir().expect("root");
        let model = constant_artifact([1; 16], 5_000);
        let (fingerprint, bytes) =
            fingerprint_model_artifact("model.norito", &model, Some(10_000)).expect("fingerprint");
        fs::write(root.path().join("model.norito"), bytes).expect("write model");
        let manifest = signed_manifest(vec![fingerprint]);
        let governance = KeyPair::try_random().expect("governance key");
        let signing_key = KeyPair::try_random().expect("runner key");
        let policy = signed_trust_policy(&manifest, &governance, &signing_key, Some(130));
        let anchors = std::iter::once(governance.public_key().clone()).collect();
        let runner =
            LoadedModerationRunnerV1::load_verified(manifest.clone(), root.path(), [3; 32])
                .expect("load runner");
        let signing_runner = LoadedModerationSigningRunnerV1::from_verified(
            runner,
            policy.clone(),
            anchors,
            1,
            signing_key.private_key().clone(),
            100,
        )
        .expect("bind signer before revocation");
        let result = signing_runner
            .screen_signed(b"payload", 1024, "cid:subject", None, 100)
            .expect("sign before revocation");
        assert_eq!(result.body.expires_at_unix, 130);
        result
            .validate(&manifest, &policy, 100)
            .expect("clipped result validates");
        assert!(result.validate(&manifest, &policy, 130).is_err());
    }

    #[test]
    fn weighted_ensemble_uses_half_up_rounding() {
        assert_eq!(weighted_score_half_up(1, 2).expect("round"), 1);
        assert_eq!(weighted_score_half_up(3, 2).expect("round"), 2);
        assert_eq!(weighted_score_half_up(2, 2).expect("round"), 1);
        assert!(weighted_score_half_up(0, 0).is_err());
    }

    #[test]
    fn fingerprint_verifier_binds_every_decoded_execution_field() {
        let artifact = artifact([1; 16], 1);
        let (fingerprint, _) = fingerprint_model_artifact("model.norito", &artifact, Some(9_999))
            .expect("fingerprint");
        let path = Path::new("model.norito");
        verify_fingerprint(path, &fingerprint, &artifact).expect("matching fingerprint");

        macro_rules! assert_mismatch {
            ($field:literal, $mutate:expr) => {{
                let mut changed = fingerprint.clone();
                $mutate(&mut changed);
                assert!(matches!(
                    verify_fingerprint(path, &changed, &artifact),
                    Err(ModerationRunnerError::FingerprintMismatch { field, .. })
                        if field == $field
                ));
            }};
        }

        assert_mismatch!("model_id", |value: &mut ModerationModelFingerprintV1| {
            value.model_id = [2; 16];
        });
        assert_mismatch!(
            "calibration_knot_count",
            |value: &mut ModerationModelFingerprintV1| {
                value.calibration_knot_count += 1;
            }
        );
        assert_mismatch!(
            "max_input_bytes",
            |value: &mut ModerationModelFingerprintV1| {
                value.max_input_bytes -= 1;
            }
        );
        assert_mismatch!(
            "max_operations",
            |value: &mut ModerationModelFingerprintV1| {
                value.max_operations += 1;
            }
        );
        assert_mismatch!(
            "working_memory_bytes",
            |value: &mut ModerationModelFingerprintV1| {
                value.working_memory_bytes += 1;
            }
        );
        assert_mismatch!(
            "weights_digest",
            |value: &mut ModerationModelFingerprintV1| {
                value.weights_digest[0] ^= 1;
            }
        );

        // Engine and feature-profile enums have exactly one first-release
        // variant. Unknown serialized discriminants are exercised by the
        // malformed-Norito loader test below; no second safe Rust value exists
        // with which to construct a semantic mismatch.
    }

    #[test]
    fn rejects_every_invalid_artifact_shape_and_resource_claim() {
        let mut invalid = artifact([1; 16], 1);
        invalid.schema_version += 1;
        assert!(matches!(
            load_raw_artifact_error(&invalid),
            ModerationRunnerError::InvalidArtifact {
                source: ModerationModelArtifactError::UnsupportedVersion { .. },
                ..
            }
        ));

        let mut invalid = artifact([1; 16], 1);
        invalid.model_id = [0; 16];
        assert!(matches!(
            load_raw_artifact_error(&invalid),
            ModerationRunnerError::InvalidArtifact {
                source: ModerationModelArtifactError::MissingModelId,
                ..
            }
        ));

        for max_input_bytes in [0, MODERATION_MODEL_MAX_INPUT_BYTES_V1 + 1] {
            let mut invalid = artifact([1; 16], 1);
            invalid.max_input_bytes = max_input_bytes;
            assert!(matches!(
                load_raw_artifact_error(&invalid),
                ModerationRunnerError::InvalidArtifact {
                    source: ModerationModelArtifactError::InvalidMaxInput { .. },
                    ..
                }
            ));
        }

        let mut invalid = artifact([1; 16], 1);
        invalid.weights.pop();
        assert!(matches!(
            load_raw_artifact_error(&invalid),
            ModerationRunnerError::InvalidArtifact {
                source: ModerationModelArtifactError::InvalidWeightCount { .. },
                ..
            }
        ));

        for calibration in [
            vec![ModerationCalibrationKnotV1 {
                input: 0,
                score_bps: 0,
            }],
            (0..=MODERATION_MODEL_MAX_CALIBRATION_KNOTS_V1)
                .map(|index| ModerationCalibrationKnotV1 {
                    input: i64::try_from(index).expect("fixture index fits i64"),
                    score_bps: 0,
                })
                .collect(),
        ] {
            let mut invalid = artifact([1; 16], 1);
            invalid.calibration = calibration;
            assert!(matches!(
                load_raw_artifact_error(&invalid),
                ModerationRunnerError::InvalidArtifact {
                    source: ModerationModelArtifactError::InvalidCalibrationCount { .. },
                    ..
                }
            ));
        }

        let mut invalid = artifact([1; 16], 1);
        invalid.calibration[1].input = invalid.calibration[0].input;
        assert!(matches!(
            load_raw_artifact_error(&invalid),
            ModerationRunnerError::InvalidArtifact {
                source: ModerationModelArtifactError::CalibrationInputOrder { .. },
                ..
            }
        ));

        for calibration in [
            vec![
                ModerationCalibrationKnotV1 {
                    input: -1,
                    score_bps: 5_000,
                },
                ModerationCalibrationKnotV1 {
                    input: 1,
                    score_bps: 4_999,
                },
            ],
            vec![
                ModerationCalibrationKnotV1 {
                    input: -1,
                    score_bps: 0,
                },
                ModerationCalibrationKnotV1 {
                    input: 1,
                    score_bps: MODERATION_REPRO_MAX_BPS + 1,
                },
            ],
        ] {
            let mut invalid = artifact([1; 16], 1);
            invalid.calibration = calibration;
            assert!(matches!(
                load_raw_artifact_error(&invalid),
                ModerationRunnerError::InvalidArtifact {
                    source: ModerationModelArtifactError::InvalidCalibrationScore { .. },
                    ..
                }
            ));
        }

        let mut invalid = artifact([1; 16], 1);
        invalid.working_memory_bytes += 1;
        assert!(matches!(
            load_raw_artifact_error(&invalid),
            ModerationRunnerError::InvalidArtifact {
                source: ModerationModelArtifactError::InvalidWorkingMemory { .. },
                ..
            }
        ));

        let mut invalid = artifact([1; 16], 1);
        invalid.max_operations += 1;
        assert!(matches!(
            load_raw_artifact_error(&invalid),
            ModerationRunnerError::InvalidArtifact {
                source: ModerationModelArtifactError::InvalidOperationBudget { .. },
                ..
            }
        ));

        let mut invalid = artifact([1; 16], 0);
        invalid.bias = i64::MAX;
        invalid.weights[0] = 1;
        assert!(matches!(
            load_raw_artifact_error(&invalid),
            ModerationRunnerError::InvalidArtifact {
                source: ModerationModelArtifactError::AccumulatorOverflow,
                ..
            }
        ));
    }

    #[test]
    fn exact_payload_bounds_and_extreme_valid_accumulator_are_deterministic() {
        let root = tempdir().expect("root");
        let mut model = artifact([1; 16], 0);
        model.bias = i64::MAX;
        let (fingerprint, bytes) =
            fingerprint_model_artifact("model.norito", &model, None).expect("fingerprint");
        fs::write(root.path().join("model.norito"), bytes).expect("write model");
        let runner = LoadedModerationRunnerV1::load_verified(
            signed_manifest(vec![fingerprint]),
            root.path(),
            [3; 32],
        )
        .expect("load runner");

        runner.infer(&[0], 1024).expect("one-byte lower bound");
        let maximum_payload = vec![0; 1024];
        let first = runner
            .infer(&maximum_payload, 1024)
            .expect("exact maximum payload");
        let second = runner
            .infer(&maximum_payload, 1024)
            .expect("repeat exact maximum payload");
        assert_eq!(first, second);
        assert!(matches!(
            runner.infer(&vec![0; 1025], 1025),
            Err(ModerationRunnerError::PayloadTooLarge { maximum: 1024, .. })
        ));
    }

    #[test]
    fn multi_model_weights_zero_weight_and_score_order_are_exact() {
        let root = tempdir().expect("root");
        let model_a = constant_artifact([1; 16], 1_000);
        let model_b = constant_artifact([2; 16], 9_000);
        let (fingerprint_a, bytes_a) =
            fingerprint_model_artifact("a.norito", &model_a, Some(1_000)).expect("fingerprint a");
        let (fingerprint_b, bytes_b) =
            fingerprint_model_artifact("b.norito", &model_b, Some(3_000)).expect("fingerprint b");
        fs::write(root.path().join("a.norito"), bytes_a).expect("write a");
        fs::write(root.path().join("b.norito"), bytes_b).expect("write b");
        let runner = LoadedModerationRunnerV1::load_verified(
            signed_manifest(vec![fingerprint_a.clone(), fingerprint_b.clone()]),
            root.path(),
            [3; 32],
        )
        .expect("load ensemble");
        let result = runner.infer(b"payload", 1024).expect("ensemble score");
        assert_eq!(result.combined_score_bps, 7_000);
        assert_eq!(
            result
                .model_scores
                .iter()
                .map(|score| score.model_id)
                .collect::<Vec<_>>(),
            vec![[1; 16], [2; 16]]
        );

        let root = tempdir().expect("zero-weight root");
        let (zero, bytes_a) =
            fingerprint_model_artifact("a.norito", &model_a, Some(0)).expect("zero fingerprint");
        let (positive, bytes_b) = fingerprint_model_artifact("b.norito", &model_b, Some(1))
            .expect("positive fingerprint");
        fs::write(root.path().join("a.norito"), bytes_a).expect("write a");
        fs::write(root.path().join("b.norito"), bytes_b).expect("write b");
        let runner = LoadedModerationRunnerV1::load_verified(
            signed_manifest(vec![zero.clone(), positive]),
            root.path(),
            [3; 32],
        )
        .expect("one positive model is sufficient");
        assert_eq!(
            runner
                .infer(b"payload", 1024)
                .expect("zero weight ignored")
                .combined_score_bps,
            9_000
        );

        let error = LoadedModerationRunnerV1::load_verified(
            signed_manifest(vec![zero]),
            root.path(),
            [3; 32],
        )
        .expect_err("all-zero weights must fail manifest validation");
        assert!(matches!(
            error,
            ModerationRunnerError::InvalidManifest(
                iroha_data_model::sorafs::moderation::ModerationReproValidationError::MissingPositiveModelWeight
            )
        ));
    }

    #[test]
    fn rejects_tampered_and_oversized_inputs() {
        let root = tempdir().expect("root");
        let artifact = artifact([1; 16], 1);
        let (fingerprint, mut bytes) =
            fingerprint_model_artifact("model.norito", &artifact, None).expect("fingerprint");
        bytes[0] ^= 1;
        fs::write(root.path().join("model.norito"), bytes).expect("write model");
        assert!(matches!(
            LoadedModerationRunnerV1::load_verified(
                signed_manifest(vec![fingerprint]),
                root.path(),
                [3; 32],
            ),
            Err(ModerationRunnerError::ArtifactDigest { .. })
        ));

        let root = tempdir().expect("root");
        let artifact = artifact([1; 16], 1);
        let (fingerprint, bytes) =
            fingerprint_model_artifact("model.norito", &artifact, None).expect("fingerprint");
        fs::write(root.path().join("model.norito"), bytes).expect("write model");
        let runner = LoadedModerationRunnerV1::load_verified(
            signed_manifest(vec![fingerprint]),
            root.path(),
            [3; 32],
        )
        .expect("load runner");
        assert!(matches!(
            runner.infer(&vec![0; 1025], 1024),
            Err(ModerationRunnerError::PayloadTooLarge { .. })
        ));
        assert!(matches!(
            runner.infer(&[], 1024),
            Err(ModerationRunnerError::EmptyPayload)
        ));
    }

    #[test]
    fn loader_rejects_signed_size_digest_trailing_and_decode_attacks() {
        let model = artifact([1; 16], 1);

        let root = tempdir().expect("size root");
        let (mut fingerprint, bytes) =
            fingerprint_model_artifact("model.norito", &model, None).expect("fingerprint");
        fingerprint.artifact_bytes += 1;
        fs::write(root.path().join("model.norito"), &bytes).expect("write model");
        assert!(matches!(
            LoadedModerationRunnerV1::load_verified(
                signed_manifest(vec![fingerprint]),
                root.path(),
                [3; 32],
            ),
            Err(ModerationRunnerError::ArtifactSize { .. })
        ));

        let root = tempdir().expect("digest root");
        let (mut fingerprint, bytes) =
            fingerprint_model_artifact("model.norito", &model, None).expect("fingerprint");
        fingerprint.artifact_digest[0] ^= 1;
        fs::write(root.path().join("model.norito"), &bytes).expect("write model");
        assert!(matches!(
            LoadedModerationRunnerV1::load_verified(
                signed_manifest(vec![fingerprint]),
                root.path(),
                [3; 32],
            ),
            Err(ModerationRunnerError::ArtifactDigest { .. })
        ));

        let root = tempdir().expect("trailing root");
        let (mut fingerprint, mut bytes) =
            fingerprint_model_artifact("model.norito", &model, None).expect("fingerprint");
        bytes.push(0xA5);
        fingerprint.artifact_bytes = u64::try_from(bytes.len()).expect("fixture size fits u64");
        fingerprint.artifact_digest = *blake3::hash(&bytes).as_bytes();
        fs::write(root.path().join("model.norito"), &bytes).expect("write model");
        assert!(matches!(
            LoadedModerationRunnerV1::load_verified(
                signed_manifest(vec![fingerprint]),
                root.path(),
                [3; 32],
            ),
            Err(ModerationRunnerError::ArtifactDecode { .. })
                | Err(ModerationRunnerError::NonCanonicalArtifact { .. })
        ));

        let root = tempdir().expect("decode root");
        let (mut fingerprint, _) =
            fingerprint_model_artifact("model.norito", &model, None).expect("fingerprint");
        let bytes = vec![
            0xFF;
            usize::try_from(MODERATION_MODEL_MAX_ARTIFACT_BYTES_V1)
                .expect("artifact cap fits usize")
        ];
        fingerprint.artifact_bytes = MODERATION_MODEL_MAX_ARTIFACT_BYTES_V1;
        fingerprint.artifact_digest = *blake3::hash(&bytes).as_bytes();
        fs::write(root.path().join("model.norito"), bytes).expect("write malformed model");
        assert!(matches!(
            LoadedModerationRunnerV1::load_verified(
                signed_manifest(vec![fingerprint]),
                root.path(),
                [3; 32],
            ),
            Err(ModerationRunnerError::ArtifactDecode { .. })
                | Err(ModerationRunnerError::InvalidArtifact { .. })
        ));
    }

    #[test]
    fn bounded_reader_rejects_short_long_and_impossible_capacities() {
        let root = tempdir().expect("root");
        let short = root.path().join("short");
        fs::write(&short, b"abc").expect("write short");
        assert!(matches!(
            read_exact_bounded(File::open(&short).expect("open short"), 4, &short),
            Err(ModerationRunnerError::ArtifactSize {
                expected: 4,
                found: 3,
                ..
            })
        ));

        let long = root.path().join("long");
        fs::write(&long, b"abcde").expect("write long");
        assert!(matches!(
            read_exact_bounded(File::open(&long).expect("open long"), 4, &long),
            Err(ModerationRunnerError::ArtifactSize {
                expected: 4,
                found: 5,
                ..
            })
        ));

        let impossible = root.path().join("impossible");
        fs::write(&impossible, []).expect("write empty");
        assert!(
            read_exact_bounded(
                File::open(&impossible).expect("open impossible"),
                u64::MAX,
                &impossible,
            )
            .is_err()
        );
    }

    #[test]
    fn calibration_interpolation_and_feature_extraction_cover_boundaries() {
        let knots = [
            ModerationCalibrationKnotV1 {
                input: -10,
                score_bps: 0,
            },
            ModerationCalibrationKnotV1 {
                input: 0,
                score_bps: 5_000,
            },
            ModerationCalibrationKnotV1 {
                input: 10,
                score_bps: 10_000,
            },
        ];
        assert_eq!(calibrate(-11, &knots).expect("lower clamp"), 0);
        assert_eq!(calibrate(-5, &knots).expect("lower interpolation"), 2_500);
        assert_eq!(calibrate(0, &knots).expect("exact knot"), 5_000);
        assert_eq!(calibrate(5, &knots).expect("upper interpolation"), 7_500);
        assert_eq!(calibrate(11, &knots).expect("upper clamp"), 10_000);

        let features = extract_features(b"aa").expect("features");
        assert_eq!(features[usize::from(b'a')], 10_000);
        let bigram_bin = 256 + (usize::from(b'a') * 251 + usize::from(b'a') * 17) % 256;
        assert_eq!(features[bigram_bin], 10_000);
        assert_eq!(
            features.iter().take(256).sum::<u64>(),
            u64::from(MODERATION_REPRO_MAX_BPS)
        );
        assert_eq!(
            features.iter().skip(256).sum::<u64>(),
            u64::from(MODERATION_REPRO_MAX_BPS)
        );
    }

    #[test]
    fn signed_seed_provenance_does_not_influence_integer_inference() {
        let root = tempdir().expect("root");
        let model = artifact([1; 16], 1);
        let (fingerprint, bytes) =
            fingerprint_model_artifact("model.norito", &model, None).expect("fingerprint");
        fs::write(root.path().join("model.norito"), bytes).expect("write model");
        let first_manifest = signed_manifest(vec![fingerprint.clone()]);
        let mut second_manifest = signed_manifest(vec![fingerprint]);
        second_manifest.body.seed_material.domain_tag = "other:calibration:provenance".to_owned();
        second_manifest.body.seed_material.seed_version = 9;
        second_manifest.body.seed_material.run_nonce = [0xA5; 32];
        second_manifest
            .body
            .refresh_manifest_digest()
            .expect("digest");
        let key = KeyPair::try_random().expect("key");
        second_manifest.signatures = vec![ModerationReproSignatureV1 {
            role: "test".to_owned(),
            public_key: key.public_key().clone(),
            signature: SignatureOf::try_new(key.private_key(), &second_manifest.body)
                .expect("signature"),
        }];

        let first = LoadedModerationRunnerV1::load_verified(first_manifest, root.path(), [3; 32])
            .expect("first runner");
        let second = LoadedModerationRunnerV1::load_verified(second_manifest, root.path(), [3; 32])
            .expect("second runner");
        assert_eq!(
            first.infer(b"seed-independent", 1024).expect("first score"),
            second
                .infer(b"seed-independent", 1024)
                .expect("second score")
        );
    }

    #[test]
    fn rejects_wrong_runner_hash_and_manifest_digest() {
        let root = tempdir().expect("root");
        let artifact = artifact([1; 16], 1);
        let (fingerprint, bytes) =
            fingerprint_model_artifact("model.norito", &artifact, None).expect("fingerprint");
        fs::write(root.path().join("model.norito"), bytes).expect("write model");
        let manifest = signed_manifest(vec![fingerprint]);
        assert!(matches!(
            LoadedModerationRunnerV1::load_verified(manifest.clone(), root.path(), [9; 32]),
            Err(ModerationRunnerError::RunnerHashMismatch)
        ));

        let mut changed = manifest;
        changed.body.manifest_digest[0] ^= 1;
        assert!(matches!(
            LoadedModerationRunnerV1::load_verified(changed, root.path(), [3; 32]),
            Err(ModerationRunnerError::InvalidManifest(
                iroha_data_model::sorafs::moderation::ModerationReproValidationError::ManifestDigestMismatch { .. }
            ))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_model() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("root");
        let outside = tempdir().expect("outside");
        let artifact = artifact([1; 16], 1);
        let (fingerprint, bytes) =
            fingerprint_model_artifact("model.norito", &artifact, None).expect("fingerprint");
        fs::write(outside.path().join("model.norito"), bytes).expect("write model");
        symlink(
            outside.path().join("model.norito"),
            root.path().join("model.norito"),
        )
        .expect("symlink");
        assert!(matches!(
            LoadedModerationRunnerV1::load_verified(
                signed_manifest(vec![fingerprint]),
                root.path(),
                [3; 32],
            ),
            Err(ModerationRunnerError::UnsafeArtifactPath { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_nested_symlinks_and_hard_link_aliases() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("root");
        let outside = tempdir().expect("outside");
        let model = artifact([1; 16], 1);
        let (fingerprint, bytes) =
            fingerprint_model_artifact("models/model.norito", &model, None).expect("fingerprint");
        fs::create_dir(outside.path().join("models")).expect("outside models");
        fs::write(outside.path().join("models/model.norito"), &bytes).expect("write outside");
        symlink(outside.path().join("models"), root.path().join("models"))
            .expect("nested directory symlink");
        assert!(matches!(
            LoadedModerationRunnerV1::load_verified(
                signed_manifest(vec![fingerprint]),
                root.path(),
                [3; 32],
            ),
            Err(ModerationRunnerError::UnsafeArtifactPath { .. })
        ));

        let root = tempdir().expect("hard-link root");
        let (fingerprint, bytes) =
            fingerprint_model_artifact("model.norito", &model, None).expect("fingerprint");
        let original = root.path().join("original.norito");
        fs::write(&original, bytes).expect("write original");
        fs::hard_link(&original, root.path().join("model.norito")).expect("create hard link");
        assert!(matches!(
            LoadedModerationRunnerV1::load_verified(
                signed_manifest(vec![fingerprint]),
                root.path(),
                [3; 32],
            ),
            Err(ModerationRunnerError::UnsafeArtifactPath { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn detects_file_replacement_after_the_verified_handle_is_open() {
        let root = tempdir().expect("root");
        let model = artifact([1; 16], 1);
        let (fingerprint, bytes) =
            fingerprint_model_artifact("model.norito", &model, None).expect("fingerprint");
        let model_path = root.path().join("model.norito");
        fs::write(&model_path, &bytes).expect("write model");
        let validated_root = validate_artifact_root(root.path()).expect("validate root");
        let replacement = root.path().join("replacement.norito");
        fs::write(&replacement, &bytes).expect("write replacement");
        let displaced = root.path().join("displaced.norito");

        let error = load_model_with_post_open_hook(&validated_root, &fingerprint, |canonical| {
            fs::rename(canonical, &displaced).expect("displace opened inode");
            fs::rename(&replacement, canonical).expect("replace artifact path");
        })
        .expect_err("identity replacement must fail");

        assert!(matches!(
            error,
            ModerationRunnerError::ArtifactChanged { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn detects_artifact_root_replacement_after_the_verified_handle_is_open() {
        let parent = tempdir().expect("parent");
        let configured = parent.path().join("configured");
        let displaced = parent.path().join("displaced");
        fs::create_dir(&configured).expect("create configured root");
        let model = artifact([1; 16], 1);
        let (fingerprint, bytes) =
            fingerprint_model_artifact("model.norito", &model, None).expect("fingerprint");
        fs::write(configured.join("model.norito"), &bytes).expect("write model");
        let validated_root = validate_artifact_root(&configured).expect("validate root");

        let error = load_model_with_post_open_hook(&validated_root, &fingerprint, |_| {
            fs::rename(&configured, &displaced).expect("displace opened root");
            fs::create_dir(&configured).expect("replace configured root");
            fs::write(configured.join("model.norito"), &bytes).expect("write replacement model");
        })
        .expect_err("root identity replacement must fail");

        assert!(matches!(
            error,
            ModerationRunnerError::ArtifactChanged { .. }
                | ModerationRunnerError::InvalidArtifactRoot { .. }
        ));
        assert!(matches!(
            verify_artifact_root(&validated_root),
            Err(ModerationRunnerError::InvalidArtifactRoot { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_or_replaced_artifact_root() {
        use std::os::unix::fs::symlink;

        let parent = tempdir().expect("parent");
        let real = parent.path().join("real");
        fs::create_dir(&real).expect("real root");
        let linked = parent.path().join("linked");
        symlink(&real, &linked).expect("root symlink");
        assert!(matches!(
            validate_artifact_root(&linked),
            Err(ModerationRunnerError::InvalidArtifactRoot { .. })
        ));

        let configured = parent.path().join("configured");
        let moved = parent.path().join("moved");
        fs::create_dir(&configured).expect("configured root");
        let validated = validate_artifact_root(&configured).expect("validate root");
        fs::rename(&configured, &moved).expect("move original root");
        fs::create_dir(&configured).expect("replace root path");
        assert!(matches!(
            verify_artifact_root(&validated),
            Err(ModerationRunnerError::InvalidArtifactRoot { .. })
        ));
    }
}
