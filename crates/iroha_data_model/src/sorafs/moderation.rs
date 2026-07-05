//! Moderation reproducibility and ballot schemas (MINFO-1b / SFM-4b4).
//!
//! These types capture the governance-signed fingerprints that allow gateways
//! to verify moderation runners, model artefacts, and threshold parameters, and
//! the `SoraFS`-specific ballot context used by moderation panels. Validators use
//! explicit helpers to enforce schema versioning, signature coverage, and
//! commit/reveal binding before accepting moderation evidence.

use std::collections::BTreeSet;

use blake2::digest::Digest;
use iroha_crypto::{Algorithm, Blake2b256, PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

#[cfg(feature = "json")]
pub(crate) use crate::json_helpers::fixed_bytes::option as json_option_digest32;

/// Schema version for `ModerationReproManifestV1`.
pub const MODERATION_REPRO_MANIFEST_VERSION_V1: u16 = 1;
/// Required ONNX opset for first-release moderation model artefacts.
pub const MODERATION_REPRO_MODEL_OPSET_V1: u16 = 17;
/// Maximum model weight and threshold value in basis points.
pub const MODERATION_REPRO_MAX_BPS: u16 = 10_000;
/// Schema version for [`SoraFsModerationBallotContextV1`].
pub const SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1: u16 = 1;
/// Schema version for [`SoraFsModerationBallotCommitV1`].
pub const SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1: u16 = 1;
/// Schema version for [`SoraFsModerationBallotRevealV1`].
pub const SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1: u16 = 1;

/// Governance-signed moderation reproducibility manifest.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationReproManifestV1 {
    /// Canonical payload describing the runner, models, and thresholds.
    pub body: ModerationReproBodyV1,
    /// Signatures issued by the governance council / SRE leads.
    #[norito(default)]
    pub signatures: Vec<ModerationReproSignatureV1>,
}

/// Canonical payload hashed and signed in the reproducibility manifest.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationReproBodyV1 {
    /// Schema version; must equal [`MODERATION_REPRO_MANIFEST_VERSION_V1`].
    pub schema_version: u16,
    /// UUID of the moderation committee manifest this record attests to.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub manifest_id: [u8; 16],
    /// BLAKE3 digest of the manifest payload.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub manifest_digest: [u8; 32],
    /// BLAKE3 digest of the compiled runner binary.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub runner_hash: [u8; 32],
    /// Runner version string (e.g., `sorafs-ai-runner 0.4.0`).
    pub runtime_version: String,
    /// Unix timestamp (seconds) when the manifest was signed.
    pub issued_at_unix: u64,
    /// Seed/domain information used to derive deterministic RNG inputs.
    pub seed_material: ModerationSeedMaterialV1,
    /// Threshold configuration applied during calibration.
    pub thresholds: ModerationThresholdsV1,
    /// Digests for each moderated model artefact.
    #[norito(default)]
    pub models: Vec<ModerationModelFingerprintV1>,
    /// Optional governance notes included in the release artefact.
    #[norito(default)]
    pub notes: Option<String>,
}

/// Digest info for a single model artefact referenced by the moderation runner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationModelFingerprintV1 {
    /// Model UUID.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub model_id: [u8; 16],
    /// Digest of the container/image that bundles tokenizer + runner glue.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub artifact_digest: [u8; 32],
    /// Digest of the ONNX/safetensors weights blob.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub weights_digest: [u8; 32],
    /// Target opset enforced during calibration (e.g., `17`).
    pub opset: u16,
    /// Optional weight applied when combining model scores (basis points, 0-10_000).
    #[norito(default)]
    pub weight: Option<u16>,
}

/// Seed derivation metadata used to generate deterministic RNG inputs.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationSeedMaterialV1 {
    /// Domain separation label applied before hashing (`fastpq:v1`, etc.).
    pub domain_tag: String,
    /// Version of the seed derivation scheme.
    pub seed_version: u16,
    /// Governance-signed run nonce (BLAKE3 input) for this calibration.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub run_nonce: [u8; 32],
}

/// Threshold values used when aggregating moderation verdicts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationThresholdsV1 {
    /// Minimum combined score required to quarantine content (basis points, 0-10_000).
    pub quarantine: u16,
    /// Minimum combined score required to escalate content for review (basis points, 0-10_000).
    pub escalate: u16,
}

/// Signature and signer metadata for a reproducibility manifest.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationReproSignatureV1 {
    /// Governance role (e.g., `council`, `sre_lead`, `audit`).
    pub role: String,
    /// Public key of the signer.
    pub public_key: PublicKey,
    /// Typed signature covering [`ModerationReproBodyV1`].
    pub signature: SignatureOf<ModerationReproBodyV1>,
}

/// Validation summary returned after checking a reproducibility manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationReproManifestSummary {
    /// Referenced manifest UUID.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub manifest_id: [u8; 16],
    /// Unix timestamp (seconds) when the manifest was issued.
    pub issued_at_unix: u64,
    /// Number of model entries covered by the manifest.
    pub model_count: u32,
    /// Number of valid signatures present.
    pub signer_count: u32,
}

/// Validation errors surfaced when checking reproducibility manifests.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModerationReproValidationError {
    /// Manifest uses an unsupported schema version.
    #[error("unsupported reproducibility schema version {found}; expected {expected}")]
    UnsupportedVersion {
        /// Expected schema version for the manifest.
        expected: u16,
        /// Schema version discovered in the manifest payload.
        found: u16,
    },
    /// Manifest contains no model entries.
    #[error("reproducibility manifest lists no model digests")]
    MissingModels,
    /// Manifest digest is all zeros.
    #[error("reproducibility manifest field `{field}` must be non-zero")]
    MissingDigest {
        /// Name of the digest field.
        field: &'static str,
    },
    /// Model identifier is all zeros.
    #[error("reproducibility manifest includes a zero model id")]
    MissingModelId,
    /// Manifest contains duplicate model identifiers.
    #[error("reproducibility manifest repeats model id {model_id:?}")]
    DuplicateModelId {
        /// Repeated model identifier.
        model_id: [u8; 16],
    },
    /// Manifest contains duplicate model artefact digests.
    #[error("reproducibility manifest repeats artifact digest for model {model_id:?}")]
    DuplicateArtifactDigest {
        /// Model identifier carrying the repeated artifact digest.
        model_id: [u8; 16],
    },
    /// Manifest contains duplicate model weight digests.
    #[error("reproducibility manifest repeats weights digest for model {model_id:?}")]
    DuplicateWeightsDigest {
        /// Model identifier carrying the repeated weights digest.
        model_id: [u8; 16],
    },
    /// Model digest is all zeros.
    #[error("reproducibility manifest model {model_id:?} field `{field}` must be non-zero")]
    MissingModelDigest {
        /// Model identifier carrying the zero digest.
        model_id: [u8; 16],
        /// Name of the digest field.
        field: &'static str,
    },
    /// Model uses an unsupported opset.
    #[error("reproducibility manifest model {model_id:?} uses opset {found}; expected {expected}")]
    UnsupportedModelOpset {
        /// Model identifier carrying the unsupported opset.
        model_id: [u8; 16],
        /// Expected opset.
        expected: u16,
        /// Opset declared in the manifest.
        found: u16,
    },
    /// Model weight is outside the accepted basis-point range.
    #[error("reproducibility manifest model {model_id:?} weight {weight} exceeds 10000 bps")]
    InvalidModelWeight {
        /// Model identifier carrying the invalid weight.
        model_id: [u8; 16],
        /// Weight declared in the manifest.
        weight: u16,
    },
    /// Every model has zero weight.
    #[error("reproducibility manifest must include at least one positive model weight")]
    MissingPositiveModelWeight,
    /// Threshold is outside the accepted basis-point range.
    #[error("reproducibility manifest threshold `{field}` value {value} exceeds 10000 bps")]
    InvalidThresholdBps {
        /// Threshold field name.
        field: &'static str,
        /// Threshold value.
        value: u16,
    },
    /// Quarantine threshold exceeds the escalate threshold.
    #[error(
        "reproducibility manifest quarantine threshold {quarantine} exceeds escalate threshold {escalate}"
    )]
    InvalidThresholdOrder {
        /// Quarantine threshold in basis points.
        quarantine: u16,
        /// Escalate threshold in basis points.
        escalate: u16,
    },
    /// Manifest is missing signer entries.
    #[error("reproducibility manifest contains no signatures")]
    MissingSignatures,
    /// Manifest includes duplicate signer keys.
    #[error("reproducibility manifest includes duplicate signer keys")]
    DuplicateSigner,
    /// Signature verification failed.
    #[error("signature for role `{role}` failed verification: {source}")]
    BadSignature {
        /// Role label attached to the failing signature.
        role: String,
        /// Underlying crypto error.
        #[source]
        source: iroha_crypto::Error,
    },
}

impl ModerationReproManifestV1 {
    /// Validate the manifest signatures and schema constraints.
    ///
    /// Returns a summary containing the manifest identifier, timestamps, and counts on success.
    ///
    /// # Errors
    ///
    /// Returns [`ModerationReproValidationError`] if the schema version mismatches,
    /// models or signatures are missing, duplicate signer keys are present, or signature
    /// verification fails.
    pub fn validate(
        &self,
    ) -> Result<ModerationReproManifestSummary, ModerationReproValidationError> {
        validate_repro_body_header(&self.body)?;
        validate_repro_thresholds(self.body.thresholds)?;
        validate_repro_models(&self.body.models)?;
        validate_repro_signatures(&self.signatures, &self.body)?;
        moderation_repro_summary(&self.body, self.signatures.len())
    }
}

fn validate_repro_body_header(
    body: &ModerationReproBodyV1,
) -> Result<(), ModerationReproValidationError> {
    if body.schema_version != MODERATION_REPRO_MANIFEST_VERSION_V1 {
        return Err(ModerationReproValidationError::UnsupportedVersion {
            expected: MODERATION_REPRO_MANIFEST_VERSION_V1,
            found: body.schema_version,
        });
    }
    if body.models.is_empty() {
        return Err(ModerationReproValidationError::MissingModels);
    }
    if body.manifest_digest == [0; 32] {
        return Err(ModerationReproValidationError::MissingDigest {
            field: "manifest_digest",
        });
    }
    if body.runner_hash == [0; 32] {
        return Err(ModerationReproValidationError::MissingDigest {
            field: "runner_hash",
        });
    }
    if body.seed_material.run_nonce == [0; 32] {
        return Err(ModerationReproValidationError::MissingDigest { field: "run_nonce" });
    }
    Ok(())
}

fn validate_repro_thresholds(
    thresholds: ModerationThresholdsV1,
) -> Result<(), ModerationReproValidationError> {
    if thresholds.quarantine > MODERATION_REPRO_MAX_BPS {
        return Err(ModerationReproValidationError::InvalidThresholdBps {
            field: "quarantine",
            value: thresholds.quarantine,
        });
    }
    if thresholds.escalate > MODERATION_REPRO_MAX_BPS {
        return Err(ModerationReproValidationError::InvalidThresholdBps {
            field: "escalate",
            value: thresholds.escalate,
        });
    }
    if thresholds.quarantine > thresholds.escalate {
        return Err(ModerationReproValidationError::InvalidThresholdOrder {
            quarantine: thresholds.quarantine,
            escalate: thresholds.escalate,
        });
    }
    Ok(())
}

fn validate_repro_models(
    models: &[ModerationModelFingerprintV1],
) -> Result<(), ModerationReproValidationError> {
    let mut model_ids = BTreeSet::new();
    let mut artifact_digests = BTreeSet::new();
    let mut weights_digests = BTreeSet::new();
    let mut has_positive_model_weight = false;
    for model in models {
        validate_repro_model_uniqueness(model, &mut model_ids)?;
        validate_repro_model_digests(model, &mut artifact_digests, &mut weights_digests)?;
        validate_repro_model_shape(model)?;
        has_positive_model_weight |= model.weight.unwrap_or(MODERATION_REPRO_MAX_BPS) > 0;
    }
    if !has_positive_model_weight {
        return Err(ModerationReproValidationError::MissingPositiveModelWeight);
    }
    Ok(())
}

fn validate_repro_model_uniqueness(
    model: &ModerationModelFingerprintV1,
    model_ids: &mut BTreeSet<[u8; 16]>,
) -> Result<(), ModerationReproValidationError> {
    if model.model_id == [0; 16] {
        return Err(ModerationReproValidationError::MissingModelId);
    }
    if !model_ids.insert(model.model_id) {
        return Err(ModerationReproValidationError::DuplicateModelId {
            model_id: model.model_id,
        });
    }
    Ok(())
}

fn validate_repro_model_digests(
    model: &ModerationModelFingerprintV1,
    artifact_digests: &mut BTreeSet<[u8; 32]>,
    weights_digests: &mut BTreeSet<[u8; 32]>,
) -> Result<(), ModerationReproValidationError> {
    if model.artifact_digest == [0; 32] {
        return Err(ModerationReproValidationError::MissingModelDigest {
            model_id: model.model_id,
            field: "artifact_digest",
        });
    }
    if !artifact_digests.insert(model.artifact_digest) {
        return Err(ModerationReproValidationError::DuplicateArtifactDigest {
            model_id: model.model_id,
        });
    }
    if model.weights_digest == [0; 32] {
        return Err(ModerationReproValidationError::MissingModelDigest {
            model_id: model.model_id,
            field: "weights_digest",
        });
    }
    if !weights_digests.insert(model.weights_digest) {
        return Err(ModerationReproValidationError::DuplicateWeightsDigest {
            model_id: model.model_id,
        });
    }
    Ok(())
}

fn validate_repro_model_shape(
    model: &ModerationModelFingerprintV1,
) -> Result<(), ModerationReproValidationError> {
    if model.opset != MODERATION_REPRO_MODEL_OPSET_V1 {
        return Err(ModerationReproValidationError::UnsupportedModelOpset {
            model_id: model.model_id,
            expected: MODERATION_REPRO_MODEL_OPSET_V1,
            found: model.opset,
        });
    }
    let weight = model.weight.unwrap_or(MODERATION_REPRO_MAX_BPS);
    if weight > MODERATION_REPRO_MAX_BPS {
        return Err(ModerationReproValidationError::InvalidModelWeight {
            model_id: model.model_id,
            weight,
        });
    }
    Ok(())
}

fn validate_repro_signatures(
    signatures: &[ModerationReproSignatureV1],
    body: &ModerationReproBodyV1,
) -> Result<(), ModerationReproValidationError> {
    if signatures.is_empty() {
        return Err(ModerationReproValidationError::MissingSignatures);
    }
    let mut seen = BTreeSet::new();
    for signer in signatures {
        if !seen.insert(signer.public_key.clone()) {
            return Err(ModerationReproValidationError::DuplicateSigner);
        }
        verify_repro_signature(&signer.signature, &signer.public_key, body).map_err(|source| {
            ModerationReproValidationError::BadSignature {
                role: signer.role.clone(),
                source,
            }
        })?;
    }
    Ok(())
}

fn moderation_repro_summary(
    body: &ModerationReproBodyV1,
    signature_count: usize,
) -> Result<ModerationReproManifestSummary, ModerationReproValidationError> {
    let model_count = u32::try_from(body.models.len())
        .map_err(|_| ModerationReproValidationError::MissingModels)?;
    let signer_count = u32::try_from(signature_count)
        .map_err(|_| ModerationReproValidationError::MissingSignatures)?;
    Ok(ModerationReproManifestSummary {
        manifest_id: body.manifest_id,
        issued_at_unix: body.issued_at_unix,
        model_count,
        signer_count,
    })
}

fn verify_repro_signature(
    signature: &SignatureOf<ModerationReproBodyV1>,
    public_key: &PublicKey,
    body: &ModerationReproBodyV1,
) -> Result<(), iroha_crypto::Error> {
    match public_key.try_algorithm() {
        Ok(Algorithm::Ed25519) => {
            iroha_crypto::ed25519_parse_signature(signature.payload())?;
        }
        Ok(Algorithm::MlDsa) => {
            iroha_crypto::mldsa65_parse_signature(signature.payload())?;
        }
        _ => {}
    }
    signature.verify(public_key, body)
}

/// `SoraFS` moderation-panel vote choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(tag = "choice", content = "value", rename_all = "kebab-case")
)]
pub enum SoraFsModerationVoteChoice {
    /// Keep the original moderation action.
    Uphold,
    /// Reverse the original moderation action.
    Overturn,
    /// Change the moderation action without fully reversing it.
    Modify,
    /// Escalate the case for another review path.
    Escalate,
}

impl SoraFsModerationVoteChoice {
    fn discriminant(self) -> u8 {
        match self {
            Self::Uphold => 1,
            Self::Overturn => 2,
            Self::Modify => 3,
            Self::Escalate => 4,
        }
    }
}

/// Immutable case scope that every moderation commit/reveal payload must bind.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SoraFsModerationBallotContextV1 {
    /// Schema version; must equal [`SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1`].
    pub version: u16,
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Digest of the evidence bundle reviewed by the panel.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub evidence_bundle_digest: [u8; 32],
    /// Appeal pricing/settlement config version used for this case.
    pub appeal_finance_config_version: String,
    /// Digest of the selected panel roster and failover policy.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub panel_roster_hash: [u8; 32],
    /// Moderation policy reference reviewed by the panel.
    pub policy_reference: String,
    /// Optional transparency or governance DAG reference for the evidence bundle.
    #[norito(default)]
    pub evidence_uri: Option<String>,
}

impl SoraFsModerationBallotContextV1 {
    /// Validate structural fields that bind a moderation ballot to one case scope.
    ///
    /// # Errors
    ///
    /// Returns [`SoraFsModerationBallotError`] when the context has an unsupported
    /// version or misses required case, evidence, finance, roster, or policy data.
    pub fn validate(&self) -> Result<(), SoraFsModerationBallotError> {
        if self.version != SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1 {
            return Err(SoraFsModerationBallotError::UnsupportedContextVersion {
                expected: SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
                found: self.version,
            });
        }
        if self.case_id.trim().is_empty() {
            return Err(SoraFsModerationBallotError::MissingCaseId);
        }
        if is_zero_digest(&self.evidence_bundle_digest) {
            return Err(SoraFsModerationBallotError::MissingEvidenceBundleDigest);
        }
        if self.appeal_finance_config_version.trim().is_empty() {
            return Err(SoraFsModerationBallotError::MissingAppealFinanceConfigVersion);
        }
        if is_zero_digest(&self.panel_roster_hash) {
            return Err(SoraFsModerationBallotError::MissingPanelRosterHash);
        }
        if self.policy_reference.trim().is_empty() {
            return Err(SoraFsModerationBallotError::MissingPolicyReference);
        }
        Ok(())
    }

    fn update_hash(&self, hasher: &mut Blake2b256) {
        hasher.update(self.version.to_le_bytes());
        update_hash_string(hasher, &self.case_id);
        hasher.update(self.evidence_bundle_digest);
        update_hash_string(hasher, &self.appeal_finance_config_version);
        hasher.update(self.panel_roster_hash);
        update_hash_string(hasher, &self.policy_reference);
        match self.evidence_uri.as_deref() {
            Some(uri) => {
                hasher.update([1u8]);
                update_hash_string(hasher, uri);
            }
            None => hasher.update([0u8]),
        }
    }
}

/// Juror commitment for a `SoraFS` moderation case.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SoraFsModerationBallotCommitV1 {
    /// Schema version; must equal [`SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1`].
    pub version: u16,
    /// Case scope this commitment is bound to.
    pub context: SoraFsModerationBallotContextV1,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Stable juror identifier or pseudonym.
    pub juror_id: String,
    /// Blake2b commitment over context, round, juror, choice, and nonce.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub commitment_blake2b_256: [u8; 32],
    /// UTC timestamp (milliseconds) when the commitment was recorded.
    pub committed_at_unix_ms: u64,
}

impl SoraFsModerationBallotCommitV1 {
    /// Validate this commitment and the supplied reveal against the shared `SoraFS` case context.
    ///
    /// # Errors
    ///
    /// Returns [`SoraFsModerationBallotError`] when either payload is malformed,
    /// references a different case context, or the reveal does not match the commitment.
    pub fn verify_reveal(
        &self,
        reveal: &SoraFsModerationBallotRevealV1,
    ) -> Result<(), SoraFsModerationBallotError> {
        self.validate()?;
        reveal.validate()?;
        if self.context != reveal.context {
            return Err(SoraFsModerationBallotError::ContextMismatch);
        }
        if self.round_id != reveal.round_id {
            return Err(SoraFsModerationBallotError::RoundMismatch {
                commit: self.round_id.clone(),
                reveal: reveal.round_id.clone(),
            });
        }
        if self.juror_id != reveal.juror_id {
            return Err(SoraFsModerationBallotError::JurorMismatch {
                commit: self.juror_id.clone(),
                reveal: reveal.juror_id.clone(),
            });
        }
        if self.commitment_blake2b_256 != reveal.compute_commitment() {
            return Err(SoraFsModerationBallotError::CommitmentMismatch);
        }
        Ok(())
    }

    /// Validate this commitment without requiring a reveal.
    ///
    /// # Errors
    ///
    /// Returns [`SoraFsModerationBallotError`] when the commitment has an
    /// unsupported version, malformed context, blank round id, or blank juror id.
    pub fn validate(&self) -> Result<(), SoraFsModerationBallotError> {
        if self.version != SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1 {
            return Err(SoraFsModerationBallotError::UnsupportedCommitVersion {
                expected: SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1,
                found: self.version,
            });
        }
        self.context.validate()?;
        if self.round_id.trim().is_empty() {
            return Err(SoraFsModerationBallotError::MissingRoundId);
        }
        if self.juror_id.trim().is_empty() {
            return Err(SoraFsModerationBallotError::MissingJurorId);
        }
        Ok(())
    }
}

/// Juror reveal for a `SoraFS` moderation case.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SoraFsModerationBallotRevealV1 {
    /// Schema version; must equal [`SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1`].
    pub version: u16,
    /// Case scope this reveal is bound to.
    pub context: SoraFsModerationBallotContextV1,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Stable juror identifier or pseudonym.
    pub juror_id: String,
    /// Moderation outcome selected by the juror.
    pub choice: SoraFsModerationVoteChoice,
    /// Random nonce used when generating the commitment.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub nonce: Vec<u8>,
    /// UTC timestamp (milliseconds) when the reveal was recorded.
    pub revealed_at_unix_ms: u64,
}

impl SoraFsModerationBallotRevealV1 {
    /// Compute the canonical commitment digest for this reveal.
    #[must_use]
    pub fn compute_commitment(&self) -> [u8; 32] {
        let mut hasher = Blake2b256::new();
        self.context.update_hash(&mut hasher);
        update_hash_string(&mut hasher, &self.round_id);
        update_hash_string(&mut hasher, &self.juror_id);
        hasher.update([self.choice.discriminant()]);
        hasher.update((self.nonce.len() as u64).to_le_bytes());
        hasher.update(&self.nonce);
        hasher.finalize().into()
    }

    /// Validate this reveal without requiring a stored commitment.
    ///
    /// # Errors
    ///
    /// Returns [`SoraFsModerationBallotError`] when the reveal has an unsupported
    /// version, malformed context, blank round or juror id, or a short nonce.
    pub fn validate(&self) -> Result<(), SoraFsModerationBallotError> {
        if self.version != SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1 {
            return Err(SoraFsModerationBallotError::UnsupportedRevealVersion {
                expected: SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1,
                found: self.version,
            });
        }
        self.context.validate()?;
        if self.round_id.trim().is_empty() {
            return Err(SoraFsModerationBallotError::MissingRoundId);
        }
        if self.juror_id.trim().is_empty() {
            return Err(SoraFsModerationBallotError::MissingJurorId);
        }
        if self.nonce.len() < 16 {
            return Err(SoraFsModerationBallotError::NonceTooShort {
                length: self.nonce.len(),
            });
        }
        Ok(())
    }
}

/// Errors surfaced while validating `SoraFS` moderation ballots.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SoraFsModerationBallotError {
    /// Context version mismatch.
    #[error("unsupported SoraFS moderation ballot context version `{found}` (expected {expected})")]
    UnsupportedContextVersion {
        /// Version expected by the verifier.
        expected: u16,
        /// Version observed in the payload.
        found: u16,
    },
    /// Commitment version mismatch.
    #[error("unsupported SoraFS moderation ballot commit version `{found}` (expected {expected})")]
    UnsupportedCommitVersion {
        /// Version expected by the verifier.
        expected: u16,
        /// Version observed in the payload.
        found: u16,
    },
    /// Reveal version mismatch.
    #[error("unsupported SoraFS moderation ballot reveal version `{found}` (expected {expected})")]
    UnsupportedRevealVersion {
        /// Version expected by the verifier.
        expected: u16,
        /// Version observed in the payload.
        found: u16,
    },
    /// Missing moderation case identifier.
    #[error("SoraFS moderation ballot case id is required")]
    MissingCaseId,
    /// Missing evidence bundle digest.
    #[error("SoraFS moderation ballot evidence bundle digest must be nonzero")]
    MissingEvidenceBundleDigest,
    /// Missing appeal finance configuration version.
    #[error("SoraFS moderation ballot appeal finance config version is required")]
    MissingAppealFinanceConfigVersion,
    /// Missing panel roster hash.
    #[error("SoraFS moderation ballot panel roster hash must be nonzero")]
    MissingPanelRosterHash,
    /// Missing moderation policy reference.
    #[error("SoraFS moderation ballot policy reference is required")]
    MissingPolicyReference,
    /// Missing ballot round identifier.
    #[error("SoraFS moderation ballot round id is required")]
    MissingRoundId,
    /// Missing juror identifier.
    #[error("SoraFS moderation ballot juror id is required")]
    MissingJurorId,
    /// Reveal nonce too short for a secure commitment.
    #[error("SoraFS moderation ballot reveal nonce must be >=16 bytes (found {length})")]
    NonceTooShort {
        /// Nonce length observed in the reveal.
        length: usize,
    },
    /// Commit and reveal are not bound to the same context.
    #[error("SoraFS moderation ballot context mismatch")]
    ContextMismatch,
    /// Reveal references a different round identifier.
    #[error("SoraFS moderation ballot round mismatch: commit `{commit}`, reveal `{reveal}`")]
    RoundMismatch {
        /// Round identifier stored in the commit.
        commit: String,
        /// Round identifier supplied by the reveal.
        reveal: String,
    },
    /// Reveal references a different juror identifier.
    #[error("SoraFS moderation ballot juror mismatch: commit `{commit}`, reveal `{reveal}`")]
    JurorMismatch {
        /// Juror identifier stored in the commit.
        commit: String,
        /// Juror identifier supplied by the reveal.
        reveal: String,
    },
    /// Revealed payload does not match the stored commitment.
    #[error("SoraFS moderation ballot commitment mismatch")]
    CommitmentMismatch,
}

fn update_hash_string(hasher: &mut Blake2b256, value: &str) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn is_zero_digest(digest: &[u8; 32]) -> bool {
    digest.iter().all(|byte| *byte == 0)
}

/// Schema version for [`AdversarialCorpusManifestV1`].
pub const ADVERSARIAL_CORPUS_VERSION_V1: u16 = 1;

/// Governance-signed registry describing adversarial corpus families.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AdversarialCorpusManifestV1 {
    /// Schema version; must equal [`ADVERSARIAL_CORPUS_VERSION_V1`].
    pub schema_version: u16,
    /// Unix timestamp (seconds) when the manifest was assembled.
    pub issued_at_unix: u64,
    /// Identifier describing the calibration window (e.g., `2026-Q1`).
    #[norito(default)]
    pub cohort_label: Option<String>,
    /// Families included in this corpus release.
    #[norito(default)]
    pub families: Vec<AdversarialPerceptualFamilyV1>,
}

/// Perceptual hash/embedding family describing one moderated cluster.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AdversarialPerceptualFamilyV1 {
    /// Deterministic family identifier (UUID).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub family_id: [u8; 16],
    /// Free-form description for operator tooling.
    pub description: String,
    /// Variants that belong to this family.
    #[norito(default)]
    pub variants: Vec<AdversarialPerceptualVariantV1>,
}

/// Entry describing a single adversarial variant and its fingerprints.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AdversarialPerceptualVariantV1 {
    /// Variant identifier (UUID).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub variant_id: [u8; 16],
    /// Attack vector description (`jpeg_jitter`, `mosaic`, `zip_bomb`, …).
    pub attack_vector: String,
    /// Optional reference CID (base64) for operator previews.
    #[norito(default)]
    pub reference_cid_b64: Option<String>,
    /// Optional canonical perceptual hash (BLAKE3-domain separated, 256-bit).
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::sorafs::moderation::json_option_digest32")
    )]
    #[norito(default)]
    pub perceptual_hash: Option<[u8; 32]>,
    /// Maximum Hamming distance tolerated for perceptual hash matches.
    #[norito(default)]
    pub hamming_radius: u8,
    /// Optional embedding digest (BLAKE3 of quantised embedding vector).
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::sorafs::moderation::json_option_digest32")
    )]
    #[norito(default)]
    pub embedding_digest: Option<[u8; 32]>,
    /// Optional free-form notes captured during benchmarking.
    #[norito(default)]
    pub notes: Option<String>,
}

/// Validation errors surfaced when checking adversarial corpus manifests.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum AdversarialCorpusValidationError {
    /// Manifest uses an unsupported schema version.
    #[error("unsupported adversarial corpus schema version {found}; expected {expected}")]
    UnsupportedVersion {
        /// Expected schema version.
        expected: u16,
        /// Schema version discovered in the manifest.
        found: u16,
    },
    /// Manifest contains no families.
    #[error("adversarial corpus manifest lists no families")]
    MissingFamilies,
    /// Family contains no variants.
    #[error("adversarial corpus manifest lists no variants for family {family_id:?}")]
    MissingVariants {
        /// Identifier of the empty family.
        family_id: [u8; 16],
    },
    /// Manifest repeats a perceptual family identifier.
    #[error("adversarial corpus manifest repeats family id {family_id:?}")]
    DuplicateFamilyId {
        /// Repeated family identifier.
        family_id: [u8; 16],
    },
    /// Manifest repeats a perceptual variant identifier.
    #[error("adversarial corpus manifest repeats variant id {variant_id:?}")]
    DuplicateVariantId {
        /// Repeated variant identifier.
        variant_id: [u8; 16],
    },
    /// Manifest repeats a perceptual hash fingerprint.
    #[error("adversarial corpus manifest repeats perceptual hash for variant {variant_id:?}")]
    DuplicatePerceptualHash {
        /// Identifier of the variant carrying the repeated perceptual hash.
        variant_id: [u8; 16],
    },
    /// Manifest repeats an embedding digest fingerprint.
    #[error("adversarial corpus manifest repeats embedding digest for variant {variant_id:?}")]
    DuplicateEmbeddingDigest {
        /// Identifier of the variant carrying the repeated embedding digest.
        variant_id: [u8; 16],
    },
    /// Variant lacks perceptual hash and embedding fingerprints.
    #[error("variant {variant_id:?} must include a perceptual hash or embedding digest")]
    MissingMatchBasis {
        /// Identifier of the variant missing match information.
        variant_id: [u8; 16],
    },
    /// Declared Hamming radius exceeds the permitted bound.
    #[error("variant {variant_id:?} sets hamming radius {radius} above the 32-bit limit")]
    InvalidHammingRadius {
        /// Identifier of the variant with excessive radius.
        variant_id: [u8; 16],
        /// Radius declared in the manifest.
        radius: u8,
    },
}

impl AdversarialCorpusManifestV1 {
    /// Validate manifest consistency before distributing it to gateways.
    ///
    /// # Errors
    ///
    /// Returns [`AdversarialCorpusValidationError`] when the schema version mismatches,
    /// family/variant identifiers are missing or duplicated, or fingerprint metadata is
    /// incomplete.
    pub fn validate(&self) -> Result<(), AdversarialCorpusValidationError> {
        if self.schema_version != ADVERSARIAL_CORPUS_VERSION_V1 {
            return Err(AdversarialCorpusValidationError::UnsupportedVersion {
                expected: ADVERSARIAL_CORPUS_VERSION_V1,
                found: self.schema_version,
            });
        }
        if self.families.is_empty() {
            return Err(AdversarialCorpusValidationError::MissingFamilies);
        }
        let mut family_ids = BTreeSet::new();
        let mut variant_ids = BTreeSet::new();
        let mut perceptual_hashes = BTreeSet::new();
        let mut embedding_digests = BTreeSet::new();
        for family in &self.families {
            if !family_ids.insert(family.family_id) {
                return Err(AdversarialCorpusValidationError::DuplicateFamilyId {
                    family_id: family.family_id,
                });
            }
            if family.variants.is_empty() {
                return Err(AdversarialCorpusValidationError::MissingVariants {
                    family_id: family.family_id,
                });
            }
            for variant in &family.variants {
                if !variant_ids.insert(variant.variant_id) {
                    return Err(AdversarialCorpusValidationError::DuplicateVariantId {
                        variant_id: variant.variant_id,
                    });
                }
                let has_hash = variant.perceptual_hash.is_some();
                let has_embedding = variant.embedding_digest.is_some();
                if !has_hash && !has_embedding {
                    return Err(AdversarialCorpusValidationError::MissingMatchBasis {
                        variant_id: variant.variant_id,
                    });
                }
                if let Some(perceptual_hash) = variant.perceptual_hash
                    && !perceptual_hashes.insert(perceptual_hash)
                {
                    return Err(AdversarialCorpusValidationError::DuplicatePerceptualHash {
                        variant_id: variant.variant_id,
                    });
                }
                if let Some(embedding_digest) = variant.embedding_digest
                    && !embedding_digests.insert(embedding_digest)
                {
                    return Err(AdversarialCorpusValidationError::DuplicateEmbeddingDigest {
                        variant_id: variant.variant_id,
                    });
                }
                if variant.hamming_radius > 32 {
                    return Err(AdversarialCorpusValidationError::InvalidHammingRadius {
                        variant_id: variant.variant_id,
                        radius: variant.hamming_radius,
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, Signature};

    use super::*;

    fn sample_body() -> ModerationReproBodyV1 {
        ModerationReproBodyV1 {
            schema_version: MODERATION_REPRO_MANIFEST_VERSION_V1,
            manifest_id: [0xAA; 16],
            manifest_digest: [0x11; 32],
            runner_hash: [0x22; 32],
            runtime_version: "sorafs-ai-runner 0.4.0".to_string(),
            issued_at_unix: 1_706_000_000,
            seed_material: ModerationSeedMaterialV1 {
                domain_tag: "fastpq:v1:moderation".to_string(),
                seed_version: 1,
                run_nonce: [0x33; 32],
            },
            thresholds: ModerationThresholdsV1 {
                quarantine: 4_200,
                escalate: 7_800,
            },
            models: vec![ModerationModelFingerprintV1 {
                model_id: [0x44; 16],
                artifact_digest: [0x55; 32],
                weights_digest: [0x66; 32],
                opset: 17,
                weight: Some(10_000),
            }],
            notes: Some("calibration=2026-02".to_string()),
        }
    }

    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked moderation fixture keypair")
    }

    fn checked_random_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm).unwrap_or_else(|err| {
            panic!("{algorithm:?} moderation fixture key generation should succeed: {err}")
        })
    }

    fn checked_signature(
        keypair: &KeyPair,
        body: &ModerationReproBodyV1,
    ) -> SignatureOf<ModerationReproBodyV1> {
        let signature = SignatureOf::try_new(keypair.private_key(), body)
            .expect("sign checked moderation reproducibility fixture");
        signature
            .verify(keypair.public_key(), body)
            .expect("checked moderation reproducibility fixture verifies");
        signature
    }

    fn sign_manifest(body: ModerationReproBodyV1, roles: &[&str]) -> ModerationReproManifestV1 {
        let mut signatures = Vec::new();
        for &role in roles {
            let keypair = checked_random_keypair();
            let signature = checked_signature(&keypair, &body);
            signatures.push(ModerationReproSignatureV1 {
                role: role.to_string(),
                public_key: keypair.public_key().clone(),
                signature,
            });
        }

        ModerationReproManifestV1 { body, signatures }
    }

    fn sign_manifest_with_keypair(
        body: ModerationReproBodyV1,
        role: &str,
        keypair: &KeyPair,
    ) -> ModerationReproManifestV1 {
        let signature = checked_signature(keypair, &body);
        ModerationReproManifestV1 {
            body,
            signatures: vec![ModerationReproSignatureV1 {
                role: role.to_string(),
                public_key: keypair.public_key().clone(),
                signature,
            }],
        }
    }

    const SMALL_ORDER_ED25519_SIGNATURE_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    const NONCANONICAL_ED25519_SIGNATURE_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    fn signature_with_malformed_ed25519_r(
        signature: &SignatureOf<ModerationReproBodyV1>,
        replacement_r: &[u8; 32],
    ) -> SignatureOf<ModerationReproBodyV1> {
        let mut payload = signature.payload().to_vec();
        payload[..replacement_r.len()].copy_from_slice(replacement_r);
        SignatureOf::from_signature(Signature::from_bytes(&payload))
    }

    #[test]
    fn validate_happy_path() {
        let manifest = sign_manifest(sample_body(), &["council", "sre"]);
        let summary = manifest.validate().expect("manifest valid");
        assert_eq!(summary.model_count, 1);
        assert_eq!(summary.signer_count, 2);
        assert_eq!(summary.manifest_id, [0xAA; 16]);
    }

    #[test]
    fn validate_rejects_duplicate_signer() {
        let body = sample_body();
        let keypair = checked_random_keypair();
        let signature = checked_signature(&keypair, &body);
        let manifest = ModerationReproManifestV1 {
            body,
            signatures: vec![
                ModerationReproSignatureV1 {
                    role: "council".to_string(),
                    public_key: keypair.public_key().clone(),
                    signature: signature.clone(),
                },
                ModerationReproSignatureV1 {
                    role: "sre".to_string(),
                    public_key: keypair.public_key().clone(),
                    signature,
                },
            ],
        };

        let err = manifest.validate().expect_err("duplicate signer must fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::DuplicateSigner
        ));
    }

    #[test]
    fn validate_rejects_malformed_ed25519_signature_r() {
        let manifest = sign_manifest(sample_body(), &["council"]);

        for (label, replacement_r) in [
            ("small-order", SMALL_ORDER_ED25519_SIGNATURE_R),
            ("noncanonical", NONCANONICAL_ED25519_SIGNATURE_R),
        ] {
            let mut invalid_manifest = manifest.clone();
            invalid_manifest.signatures[0].signature = signature_with_malformed_ed25519_r(
                &manifest.signatures[0].signature,
                &replacement_r,
            );

            let err = invalid_manifest
                .validate()
                .expect_err("malformed moderation signature R must fail admission");
            let ModerationReproValidationError::BadSignature { source, .. } = err else {
                panic!("expected bad moderation signature error: {err:?}");
            };
            assert_eq!(
                source,
                iroha_crypto::Error::BadSignature,
                "{label} moderation signature R produced unexpected error"
            );
        }
    }

    #[test]
    fn validate_rejects_malformed_mldsa_signature_lengths() {
        let keypair = checked_random_keypair_with_algorithm(Algorithm::MlDsa);
        let manifest = sign_manifest_with_keypair(sample_body(), "council", &keypair);
        manifest
            .validate()
            .expect("valid ML-DSA moderation manifest verifies");
        let valid_signature = manifest.signatures[0].signature.payload().to_vec();

        for (label, replacement_signature) in [
            (
                "short",
                valid_signature[..valid_signature.len() - 1].to_vec(),
            ),
            ("overlong", {
                let mut payload = valid_signature.clone();
                payload.push(0x5E);
                payload
            }),
        ] {
            let mut invalid_manifest = manifest.clone();
            invalid_manifest.signatures[0].signature =
                SignatureOf::from_signature(Signature::from_bytes(&replacement_signature));

            let err = invalid_manifest
                .validate()
                .expect_err("malformed moderation ML-DSA signature length must fail admission");
            let ModerationReproValidationError::BadSignature { source, .. } = err else {
                panic!("expected bad moderation signature error: {err:?}");
            };
            assert_eq!(
                source,
                iroha_crypto::Error::BadSignature,
                "{label} moderation ML-DSA signature length produced unexpected error"
            );
        }
    }

    #[test]
    fn validate_rejects_missing_models() {
        let mut body = sample_body();
        body.models.clear();
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest.validate().expect_err("missing models should fail");
        assert!(matches!(err, ModerationReproValidationError::MissingModels));
    }

    #[test]
    fn validate_rejects_zero_manifest_digests() {
        let mut body = sample_body();
        body.manifest_digest = [0; 32];
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest
            .validate()
            .expect_err("zero manifest digest should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::MissingDigest {
                field: "manifest_digest"
            }
        ));

        let mut body = sample_body();
        body.runner_hash = [0; 32];
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest
            .validate()
            .expect_err("zero runner hash should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::MissingDigest {
                field: "runner_hash"
            }
        ));

        let mut body = sample_body();
        body.seed_material.run_nonce = [0; 32];
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest.validate().expect_err("zero run nonce should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::MissingDigest { field: "run_nonce" }
        ));
    }

    #[test]
    fn validate_rejects_bad_thresholds() {
        let mut body = sample_body();
        body.thresholds.quarantine = MODERATION_REPRO_MAX_BPS + 1;
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest
            .validate()
            .expect_err("oversized quarantine threshold should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::InvalidThresholdBps {
                field: "quarantine",
                value: 10_001
            }
        ));

        let mut body = sample_body();
        body.thresholds.escalate = MODERATION_REPRO_MAX_BPS + 1;
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest
            .validate()
            .expect_err("oversized escalate threshold should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::InvalidThresholdBps {
                field: "escalate",
                value: 10_001
            }
        ));

        let mut body = sample_body();
        body.thresholds = ModerationThresholdsV1 {
            quarantine: 8_000,
            escalate: 7_000,
        };
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest
            .validate()
            .expect_err("inverted thresholds should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::InvalidThresholdOrder {
                quarantine: 8_000,
                escalate: 7_000
            }
        ));
    }

    #[test]
    fn validate_rejects_duplicate_model_ids_and_digests() {
        let mut body = sample_body();
        let mut duplicate = body.models[0];
        duplicate.artifact_digest = [0x77; 32];
        duplicate.weights_digest = [0x88; 32];
        body.models.push(duplicate);
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest
            .validate()
            .expect_err("duplicate model id should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::DuplicateModelId { .. }
        ));

        let mut body = sample_body();
        let mut duplicate = body.models[0];
        duplicate.model_id = [0x45; 16];
        duplicate.weights_digest = [0x88; 32];
        body.models.push(duplicate);
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest
            .validate()
            .expect_err("duplicate artifact digest should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::DuplicateArtifactDigest { .. }
        ));

        let mut body = sample_body();
        let mut duplicate = body.models[0];
        duplicate.model_id = [0x45; 16];
        duplicate.artifact_digest = [0x77; 32];
        body.models.push(duplicate);
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest
            .validate()
            .expect_err("duplicate weights digest should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::DuplicateWeightsDigest { .. }
        ));
    }

    #[test]
    fn validate_rejects_missing_model_identity_and_digests() {
        let mut body = sample_body();
        body.models[0].model_id = [0; 16];
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest.validate().expect_err("zero model id should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::MissingModelId
        ));

        let mut body = sample_body();
        body.models[0].artifact_digest = [0; 32];
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest
            .validate()
            .expect_err("zero artifact digest should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::MissingModelDigest {
                field: "artifact_digest",
                ..
            }
        ));

        let mut body = sample_body();
        body.models[0].weights_digest = [0; 32];
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest
            .validate()
            .expect_err("zero weights digest should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::MissingModelDigest {
                field: "weights_digest",
                ..
            }
        ));
    }

    #[test]
    fn validate_rejects_unsupported_opset_and_bad_weights() {
        let mut body = sample_body();
        body.models[0].opset = MODERATION_REPRO_MODEL_OPSET_V1 + 1;
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest
            .validate()
            .expect_err("unsupported opset should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::UnsupportedModelOpset {
                expected: MODERATION_REPRO_MODEL_OPSET_V1,
                found: 18,
                ..
            }
        ));

        let mut body = sample_body();
        body.models[0].weight = Some(MODERATION_REPRO_MAX_BPS + 1);
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest
            .validate()
            .expect_err("oversized model weight should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::InvalidModelWeight { weight: 10_001, .. }
        ));

        let mut body = sample_body();
        body.models[0].weight = Some(0);
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest
            .validate()
            .expect_err("all-zero model weights should fail");
        assert!(matches!(
            err,
            ModerationReproValidationError::MissingPositiveModelWeight
        ));
    }

    fn sample_ballot_context() -> SoraFsModerationBallotContextV1 {
        SoraFsModerationBallotContextV1 {
            version: SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
            case_id: "SFM-CASE-2026-0007".to_string(),
            evidence_bundle_digest: [0xA7; 32],
            appeal_finance_config_version: "baseline-v1".to_string(),
            panel_roster_hash: [0xB7; 32],
            policy_reference: "gar:moderation:2026-q1".to_string(),
            evidence_uri: Some("sorafs://governance/evidence/case-0007".to_string()),
        }
    }

    fn sample_ballot_reveal(choice: SoraFsModerationVoteChoice) -> SoraFsModerationBallotRevealV1 {
        SoraFsModerationBallotRevealV1 {
            version: SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1,
            context: sample_ballot_context(),
            round_id: "SFM-PANEL-2026-02".to_string(),
            juror_id: "juror:pop:7".to_string(),
            choice,
            nonce: vec![0x42; 32],
            revealed_at_unix_ms: 1_738_001_000_000,
        }
    }

    fn sample_ballot_commit(choice: SoraFsModerationVoteChoice) -> SoraFsModerationBallotCommitV1 {
        let reveal = sample_ballot_reveal(choice);
        SoraFsModerationBallotCommitV1 {
            version: SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1,
            context: reveal.context.clone(),
            round_id: reveal.round_id.clone(),
            juror_id: reveal.juror_id.clone(),
            commitment_blake2b_256: reveal.compute_commitment(),
            committed_at_unix_ms: 1_738_000_000_000,
        }
    }

    #[test]
    fn sorafs_moderation_ballot_commit_reveal_roundtrip() {
        let commit = sample_ballot_commit(SoraFsModerationVoteChoice::Overturn);
        let reveal = sample_ballot_reveal(SoraFsModerationVoteChoice::Overturn);

        commit
            .verify_reveal(&reveal)
            .expect("SoraFS moderation reveal matches commitment");
    }

    #[test]
    fn sorafs_moderation_ballot_binds_evidence_and_finance_context() {
        let commit = sample_ballot_commit(SoraFsModerationVoteChoice::Modify);
        let mut reveal = sample_ballot_reveal(SoraFsModerationVoteChoice::Modify);
        reveal.context.evidence_bundle_digest = [0xC7; 32];
        let err = commit
            .verify_reveal(&reveal)
            .expect_err("evidence digest mismatch must fail");
        assert!(matches!(err, SoraFsModerationBallotError::ContextMismatch));

        let mut reveal = sample_ballot_reveal(SoraFsModerationVoteChoice::Modify);
        reveal.context.appeal_finance_config_version = "baseline-v2".to_string();
        let err = commit
            .verify_reveal(&reveal)
            .expect_err("finance config version mismatch must fail");
        assert!(matches!(err, SoraFsModerationBallotError::ContextMismatch));

        let mut reveal = sample_ballot_reveal(SoraFsModerationVoteChoice::Modify);
        reveal.context.panel_roster_hash = [0xD7; 32];
        let err = commit
            .verify_reveal(&reveal)
            .expect_err("panel roster hash mismatch must fail");
        assert!(matches!(err, SoraFsModerationBallotError::ContextMismatch));
    }

    #[test]
    fn sorafs_moderation_ballot_rejects_mismatched_choice_and_short_nonce() {
        let commit = sample_ballot_commit(SoraFsModerationVoteChoice::Uphold);
        let reveal = sample_ballot_reveal(SoraFsModerationVoteChoice::Escalate);
        let err = commit
            .verify_reveal(&reveal)
            .expect_err("choice mismatch must fail");
        assert!(matches!(
            err,
            SoraFsModerationBallotError::CommitmentMismatch
        ));

        let mut reveal = sample_ballot_reveal(SoraFsModerationVoteChoice::Uphold);
        reveal.nonce = vec![0x01; 8];
        let err = commit
            .verify_reveal(&reveal)
            .expect_err("short nonce must fail");
        assert!(matches!(
            err,
            SoraFsModerationBallotError::NonceTooShort { length: 8 }
        ));
    }

    #[test]
    fn sorafs_moderation_ballot_requires_case_policy_and_roster_scope() {
        let mut reveal = sample_ballot_reveal(SoraFsModerationVoteChoice::Uphold);
        reveal.context.case_id = "  ".to_string();
        let commit = SoraFsModerationBallotCommitV1 {
            version: SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1,
            context: reveal.context.clone(),
            round_id: reveal.round_id.clone(),
            juror_id: reveal.juror_id.clone(),
            commitment_blake2b_256: reveal.compute_commitment(),
            committed_at_unix_ms: 1_738_000_000_000,
        };
        let err = commit
            .verify_reveal(&reveal)
            .expect_err("blank case id must fail");
        assert!(matches!(err, SoraFsModerationBallotError::MissingCaseId));

        let mut reveal = sample_ballot_reveal(SoraFsModerationVoteChoice::Uphold);
        reveal.context.policy_reference.clear();
        let commit = SoraFsModerationBallotCommitV1 {
            version: SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1,
            context: reveal.context.clone(),
            round_id: reveal.round_id.clone(),
            juror_id: reveal.juror_id.clone(),
            commitment_blake2b_256: reveal.compute_commitment(),
            committed_at_unix_ms: 1_738_000_000_000,
        };
        let err = commit
            .verify_reveal(&reveal)
            .expect_err("blank policy reference must fail");
        assert!(matches!(
            err,
            SoraFsModerationBallotError::MissingPolicyReference
        ));

        let mut reveal = sample_ballot_reveal(SoraFsModerationVoteChoice::Uphold);
        reveal.context.panel_roster_hash = [0; 32];
        let commit = SoraFsModerationBallotCommitV1 {
            version: SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1,
            context: reveal.context.clone(),
            round_id: reveal.round_id.clone(),
            juror_id: reveal.juror_id.clone(),
            commitment_blake2b_256: reveal.compute_commitment(),
            committed_at_unix_ms: 1_738_000_000_000,
        };
        let err = commit
            .verify_reveal(&reveal)
            .expect_err("zero roster hash must fail");
        assert!(matches!(
            err,
            SoraFsModerationBallotError::MissingPanelRosterHash
        ));
    }

    fn sample_family_manifest() -> AdversarialCorpusManifestV1 {
        AdversarialCorpusManifestV1 {
            schema_version: ADVERSARIAL_CORPUS_VERSION_V1,
            issued_at_unix: 1_706_000_000,
            cohort_label: Some("2026-Q1".to_string()),
            families: vec![AdversarialPerceptualFamilyV1 {
                family_id: [0x01; 16],
                description: "jpeg jitter corpus".to_string(),
                variants: vec![AdversarialPerceptualVariantV1 {
                    variant_id: [0x02; 16],
                    attack_vector: "jpeg_jitter".to_string(),
                    reference_cid_b64: Some("YmFzZTY0LWNpZA==".to_string()),
                    perceptual_hash: Some([0xAA; 32]),
                    hamming_radius: 4,
                    embedding_digest: None,
                    notes: Some("delta=2".to_string()),
                }],
            }],
        }
    }

    #[test]
    fn adversarial_manifest_validates() {
        let manifest = sample_family_manifest();
        manifest.validate().expect("manifest valid");
    }

    #[test]
    fn adversarial_manifest_rejects_missing_variants() {
        let mut manifest = sample_family_manifest();
        manifest.families[0].variants.clear();
        let err = manifest.validate().expect_err("missing variants");
        assert!(matches!(
            err,
            AdversarialCorpusValidationError::MissingVariants { .. }
        ));
    }

    #[test]
    fn adversarial_manifest_rejects_duplicate_family_ids() {
        let mut manifest = sample_family_manifest();
        let mut duplicate = manifest.families[0].clone();
        duplicate.description = "same family id with different rows".to_string();
        duplicate.variants[0].variant_id = [0x03; 16];
        manifest.families.push(duplicate);

        let err = manifest.validate().expect_err("duplicate family id");
        assert!(matches!(
            err,
            AdversarialCorpusValidationError::DuplicateFamilyId { .. }
        ));
    }

    #[test]
    fn adversarial_manifest_rejects_duplicate_variant_ids_across_families() {
        let mut manifest = sample_family_manifest();
        let mut second_family = manifest.families[0].clone();
        second_family.family_id = [0x04; 16];
        second_family.description = "same variant id in another family".to_string();
        manifest.families.push(second_family);

        let err = manifest.validate().expect_err("duplicate variant id");
        assert!(matches!(
            err,
            AdversarialCorpusValidationError::DuplicateVariantId { .. }
        ));
    }

    #[test]
    fn adversarial_manifest_rejects_duplicate_variant_ids_within_family() {
        let mut manifest = sample_family_manifest();
        let mut duplicate = manifest.families[0].variants[0].clone();
        duplicate.attack_vector = "mosaic".to_string();
        duplicate.perceptual_hash = Some([0xBB; 32]);
        manifest.families[0].variants.push(duplicate);

        let err = manifest.validate().expect_err("duplicate variant id");
        assert!(matches!(
            err,
            AdversarialCorpusValidationError::DuplicateVariantId { .. }
        ));
    }

    #[test]
    fn adversarial_manifest_rejects_duplicate_perceptual_hashes() {
        let mut manifest = sample_family_manifest();
        let mut duplicate = manifest.families[0].variants[0].clone();
        duplicate.variant_id = [0x05; 16];
        duplicate.attack_vector = "crop_jitter".to_string();
        manifest.families[0].variants.push(duplicate);

        let err = manifest.validate().expect_err("duplicate perceptual hash");
        assert!(matches!(
            err,
            AdversarialCorpusValidationError::DuplicatePerceptualHash { .. }
        ));
    }

    #[test]
    fn adversarial_manifest_rejects_duplicate_embedding_digests() {
        let mut manifest = sample_family_manifest();
        manifest.families[0].variants[0].perceptual_hash = None;
        manifest.families[0].variants[0].embedding_digest = Some([0xCC; 32]);
        let mut duplicate = manifest.families[0].variants[0].clone();
        duplicate.variant_id = [0x06; 16];
        duplicate.attack_vector = "embedding_collision".to_string();
        manifest.families[0].variants.push(duplicate);

        let err = manifest.validate().expect_err("duplicate embedding digest");
        assert!(matches!(
            err,
            AdversarialCorpusValidationError::DuplicateEmbeddingDigest { .. }
        ));
    }

    #[test]
    fn adversarial_manifest_requires_match_basis() {
        let mut manifest = sample_family_manifest();
        manifest.families[0].variants[0].perceptual_hash = None;
        manifest.families[0].variants[0].embedding_digest = None;
        let err = manifest.validate().expect_err("missing fingerprints");
        assert!(matches!(
            err,
            AdversarialCorpusValidationError::MissingMatchBasis { .. }
        ));
    }
}
