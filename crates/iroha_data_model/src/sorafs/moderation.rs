//! Moderation reproducibility schema (MINFO-1b).
//!
//! These types capture the governance-signed fingerprints that allow gateways
//! to verify that moderation runners, model artefacts, and threshold
//! parameters match the canonical committee manifest. Validators use the
//! `validate` helper to enforce schema versioning and signature coverage before
//! accepting a new reproducibility package.

use std::collections::BTreeSet;

use iroha_crypto::{PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

#[cfg(feature = "json")]
pub(crate) use crate::json_helpers::fixed_bytes::option as json_option_digest32;

/// Schema version for `ModerationReproManifestV1`.
pub const MODERATION_REPRO_MANIFEST_VERSION_V1: u16 = 1;

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
        if self.body.schema_version != MODERATION_REPRO_MANIFEST_VERSION_V1 {
            return Err(ModerationReproValidationError::UnsupportedVersion {
                expected: MODERATION_REPRO_MANIFEST_VERSION_V1,
                found: self.body.schema_version,
            });
        }
        if self.body.models.is_empty() {
            return Err(ModerationReproValidationError::MissingModels);
        }
        if self.signatures.is_empty() {
            return Err(ModerationReproValidationError::MissingSignatures);
        }

        let mut seen = BTreeSet::new();
        for signer in &self.signatures {
            if !seen.insert(signer.public_key.clone()) {
                return Err(ModerationReproValidationError::DuplicateSigner);
            }
            if let Err(source) = signer.signature.verify(&signer.public_key, &self.body) {
                return Err(ModerationReproValidationError::BadSignature {
                    role: signer.role.clone(),
                    source,
                });
            }
        }

        let model_count = u32::try_from(self.body.models.len())
            .map_err(|_| ModerationReproValidationError::MissingModels)?;
        let signer_count = u32::try_from(self.signatures.len())
            .map_err(|_| ModerationReproValidationError::MissingSignatures)?;

        Ok(ModerationReproManifestSummary {
            manifest_id: self.body.manifest_id,
            issued_at_unix: self.body.issued_at_unix,
            model_count,
            signer_count,
        })
    }
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
    /// families/variants are missing, or fingerprint metadata is incomplete.
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
        for family in &self.families {
            if family.variants.is_empty() {
                return Err(AdversarialCorpusValidationError::MissingVariants {
                    family_id: family.family_id,
                });
            }
            for variant in &family.variants {
                let has_hash = variant.perceptual_hash.is_some();
                let has_embedding = variant.embedding_digest.is_some();
                if !has_hash && !has_embedding {
                    return Err(AdversarialCorpusValidationError::MissingMatchBasis {
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
    use iroha_crypto::KeyPair;

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

    fn sign_manifest(body: ModerationReproBodyV1, roles: &[&str]) -> ModerationReproManifestV1 {
        let mut signatures = Vec::new();
        for &role in roles {
            let keypair = KeyPair::random();
            let signature = SignatureOf::new(keypair.private_key(), &body);
            signatures.push(ModerationReproSignatureV1 {
                role: role.to_string(),
                public_key: keypair.public_key().clone(),
                signature,
            });
        }

        ModerationReproManifestV1 { body, signatures }
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
        let keypair = KeyPair::random();
        let signature = SignatureOf::new(keypair.private_key(), &body);
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
    fn validate_rejects_missing_models() {
        let mut body = sample_body();
        body.models.clear();
        let manifest = sign_manifest(body, &["council"]);
        let err = manifest.validate().expect_err("missing models should fail");
        assert!(matches!(err, ModerationReproValidationError::MissingModels));
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
