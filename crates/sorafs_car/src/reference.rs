//! Reference validation outcome adapter for manifest/CAR replay.
//!
//! The SoraFS manifest crate owns the stable `ValidationOutcomeV1` schema, while
//! this crate owns CAR parsing and trustless replay. Keeping the adapter here
//! avoids a dependency cycle and lets operators receive the same outcome shape
//! as the rest of the SF-11 reference validators.

use norito::decode_from_bytes;
use sorafs_manifest::{
    ManifestV1,
    reference::{ValidationContextFieldV1, ValidationInputV1, ValidationOutcomeV1},
    validation::{ManifestValidationError, PinPolicyConstraints, validate_manifest},
};

use crate::{TrustlessVerificationError, TrustlessVerificationOutcome, TrustlessVerifierConfig};

const CATEGORY_INTERNAL: &str = "internal";
const CATEGORY_NORITO: &str = "norito";
const CATEGORY_POLICY: &str = "policy";
const CATEGORY_VALIDATION: &str = "validation";
const TELEMETRY_MANIFEST_CAR: &str = "sorafs.reference.manifest_car";

/// Validates a Norito-encoded manifest and a full CAR stream as one replay unit.
#[must_use]
pub fn validate_manifest_car_replay_bytes(
    manifest_bytes: &[u8],
    car_bytes: &[u8],
    manifest_label: impl Into<String>,
    car_label: impl Into<String>,
    config: &TrustlessVerifierConfig,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let manifest_label = manifest_label.into();
    let car_label = car_label.into();
    let inputs = replay_inputs(&manifest_label, &car_label);

    let manifest = match decode_from_bytes::<ManifestV1>(manifest_bytes) {
        Ok(manifest) => manifest,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!("failed to decode ManifestV1 Norito payload: {error}"),
                "Re-encode the manifest with the canonical SoraFS Norito schema.",
                telemetry_tags("SFS-NORITO-001"),
                vec![ValidationContextFieldV1::new("schema", "ManifestV1")],
                inputs,
                generated_at,
            );
        }
    };

    validate_manifest_car_replay(
        &manifest,
        car_bytes,
        manifest_label,
        car_label,
        config,
        generated_at,
    )
}

/// Validates an already-decoded manifest and a full CAR stream as one replay unit.
#[must_use]
pub fn validate_manifest_car_replay(
    manifest: &ManifestV1,
    car_bytes: &[u8],
    manifest_label: impl Into<String>,
    car_label: impl Into<String>,
    config: &TrustlessVerifierConfig,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let manifest_label = manifest_label.into();
    let car_label = car_label.into();
    let inputs = replay_inputs(&manifest_label, &car_label);
    let mut context = manifest_context(manifest, config);

    if let Err(error) = validate_manifest(manifest, &PinPolicyConstraints::default()) {
        let (code, category) = manifest_validation_code_category(&error);
        context.push(ValidationContextFieldV1::new(
            "manifest_validation_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("manifest policy replay failed: {error}"),
            "Regenerate the manifest under the current SoraFS pin-registry policy and retry manifest/CAR replay.",
            telemetry_tags(code),
            context,
            inputs,
            generated_at,
        );
    }

    let verifier = crate::TrustlessVerifier::new(config.clone());
    match verifier.verify_full(manifest, car_bytes) {
        Ok(outcome) => {
            context.extend(outcome_context(&outcome));
            ValidationOutcomeV1::ok(
                "SFS-OK-000",
                "manifest/CAR replay accepted",
                telemetry_tags("SFS-OK-000"),
                context,
                inputs,
                generated_at,
            )
        }
        Err(error) => trustless_replay_error(error, context, inputs, generated_at),
    }
}

fn replay_inputs(manifest_label: &str, car_label: &str) -> Vec<ValidationInputV1> {
    vec![
        ValidationInputV1::new("manifest", manifest_label.to_owned()),
        ValidationInputV1::new("car", car_label.to_owned()),
    ]
}

fn manifest_context(
    manifest: &ManifestV1,
    config: &TrustlessVerifierConfig,
) -> Vec<ValidationContextFieldV1> {
    let mut context = vec![
        ValidationContextFieldV1::new("schema", "ManifestV1"),
        ValidationContextFieldV1::new("manifest_version", manifest.version.to_string()),
        ValidationContextFieldV1::new(
            "manifest_content_length",
            manifest.content_length.to_string(),
        ),
        ValidationContextFieldV1::new("manifest_car_size", manifest.car_size.to_string()),
        ValidationContextFieldV1::new("manifest_car_digest_hex", hex::encode(manifest.car_digest)),
        ValidationContextFieldV1::new("manifest_root_cid_hex", hex::encode(&manifest.root_cid)),
        ValidationContextFieldV1::new(
            "chunk_profile_handle",
            format!(
                "{}.{}@{}",
                manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
            ),
        ),
        ValidationContextFieldV1::new(
            "chunk_profile_id",
            manifest.chunking.profile_id.0.to_string(),
        ),
        ValidationContextFieldV1::new(
            "pin_min_replicas",
            manifest.pin_policy.min_replicas.to_string(),
        ),
        ValidationContextFieldV1::new(
            "pin_retention_epoch",
            manifest.pin_policy.retention_epoch.to_string(),
        ),
        ValidationContextFieldV1::new(
            "pin_storage_class",
            format!("{:?}", manifest.pin_policy.storage_class),
        ),
        ValidationContextFieldV1::new("trustless_config_version", config.version.to_string()),
    ];

    if let Ok(digest) = manifest.digest() {
        context.push(ValidationContextFieldV1::new(
            "manifest_digest_blake3_hex",
            hex::encode(digest.as_bytes()),
        ));
    }

    context
}

fn outcome_context(outcome: &TrustlessVerificationOutcome) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new(
            "verified_manifest_digest_blake3_hex",
            outcome.manifest_digest_hex(),
        ),
        ValidationContextFieldV1::new("car_digest_blake3_hex", outcome.car_digest_hex()),
        ValidationContextFieldV1::new("payload_digest_blake3_hex", outcome.payload_digest_hex()),
        ValidationContextFieldV1::new(
            "chunk_plan_digest_sha3_hex",
            outcome.chunk_plan_digest_hex(),
        ),
        ValidationContextFieldV1::new("por_root_hex", outcome.por_root_hex()),
        ValidationContextFieldV1::new("profile_handle", outcome.profile_handle().to_owned()),
        ValidationContextFieldV1::new(
            "chunk_count",
            outcome.report.chunk_store.chunks().len().to_string(),
        ),
        ValidationContextFieldV1::new(
            "payload_bytes",
            outcome.report.stats.payload_bytes.to_string(),
        ),
        ValidationContextFieldV1::new("car_size", outcome.report.stats.car_size.to_string()),
    ]
}

fn manifest_validation_code_category(
    error: &ManifestValidationError,
) -> (&'static str, &'static str) {
    match error {
        ManifestValidationError::UnsupportedVersion { .. } => ("SFS-VAL-002", CATEGORY_VALIDATION),
        ManifestValidationError::UnknownChunkerProfile { .. }
        | ManifestValidationError::ChunkerDescriptorMismatch { .. }
        | ManifestValidationError::UnknownChunkerAlias { .. }
        | ManifestValidationError::MissingCanonicalAlias { .. } => {
            ("SFS-VAL-003", CATEGORY_VALIDATION)
        }
        ManifestValidationError::MinReplicasTooLow { .. }
        | ManifestValidationError::MaxReplicasExceeded { .. }
        | ManifestValidationError::RetentionEpochExceeded { .. }
        | ManifestValidationError::StorageClassNotAllowed { .. }
        | ManifestValidationError::MissingCouncilSignature => ("SFS-POL-006", CATEGORY_POLICY),
    }
}

fn trustless_replay_error(
    error: TrustlessVerificationError,
    mut context: Vec<ValidationContextFieldV1>,
    inputs: Vec<ValidationInputV1>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    context.push(ValidationContextFieldV1::new(
        "replay_error",
        error.to_string(),
    ));

    let (code, category, action) = match &error {
        TrustlessVerificationError::Car(_) => (
            "SFS-CAR-001",
            CATEGORY_VALIDATION,
            "Regenerate or refetch the CAR stream so its roots, chunk plan, digest, and size match the manifest commitments.",
        ),
        TrustlessVerificationError::ConfigVersionMismatch { .. }
        | TrustlessVerificationError::ManifestDigest(_)
        | TrustlessVerificationError::MissingPorRoot => (
            "SFS-CAR-002",
            CATEGORY_INTERNAL,
            "Update the trustless verifier config or report the replay metadata derivation failure to maintainers.",
        ),
        TrustlessVerificationError::PinRecordInvalid(_)
        | TrustlessVerificationError::PinRecordManifestCidMismatch { .. }
        | TrustlessVerificationError::PinRecordProfileMismatch { .. }
        | TrustlessVerificationError::PinRecordChunkPlanMismatch { .. }
        | TrustlessVerificationError::PinRecordPorRootMismatch { .. } => (
            "SFS-CAR-001",
            CATEGORY_VALIDATION,
            "Regenerate the pin record from the verified manifest/CAR replay metadata.",
        ),
    };

    ValidationOutcomeV1::error(
        code,
        category,
        format!("manifest/CAR replay failed: {error}"),
        action,
        telemetry_tags(code),
        context,
        inputs,
        generated_at,
    )
}

fn telemetry_tags(code: &str) -> Vec<String> {
    vec![
        TELEMETRY_MANIFEST_CAR.to_owned(),
        format!("sorafs.reference.code.{code}"),
    ]
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use norito::decode_from_bytes;

    use super::*;

    fn workspace_path(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .join(relative)
    }

    fn gateway_inputs() -> (Vec<u8>, Vec<u8>, TrustlessVerifierConfig) {
        let manifest_bytes = fs::read(workspace_path(
            "fixtures/sorafs_gateway/1.0.0/manifest_v1.to",
        ))
        .expect("manifest bytes");
        let car_bytes = fs::read(workspace_path("fixtures/sorafs_gateway/1.0.0/gateway.car"))
            .expect("gateway CAR bytes");
        let config = TrustlessVerifierConfig::from_file(workspace_path(
            "configs/soranet/gateway_m0/gateway_trustless_verifier.toml",
        ))
        .expect("gateway config");
        (manifest_bytes, car_bytes, config)
    }

    fn context_value<'a>(outcome: &'a ValidationOutcomeV1, key: &str) -> Option<&'a str> {
        outcome
            .context
            .iter()
            .find(|field| field.key == key)
            .map(|field| field.value.as_str())
    }

    #[test]
    fn manifest_car_replay_accepts_gateway_fixture() {
        let (manifest_bytes, car_bytes, config) = gateway_inputs();
        let outcome = validate_manifest_car_replay_bytes(
            &manifest_bytes,
            &car_bytes,
            "manifest_v1.to",
            "gateway.car",
            &config,
            123,
        );

        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(outcome.code, "SFS-OK-000");
        assert_eq!(outcome.generated_at, 123);
        assert_eq!(
            context_value(&outcome, "profile_handle"),
            Some("sorafs.sf1@1.0.0")
        );
        assert_eq!(
            context_value(&outcome, "car_digest_blake3_hex"),
            Some("ce50a9aadf84e57559208d39201621262fd1b1887ae490ca54470e2a00153f27")
        );
    }

    #[test]
    fn manifest_car_replay_rejects_manifest_car_digest_mismatch() {
        let (manifest_bytes, car_bytes, config) = gateway_inputs();
        let mut manifest: ManifestV1 = decode_from_bytes(&manifest_bytes).expect("manifest");
        manifest.car_digest[0] ^= 0xFF;
        let tampered_manifest = norito::to_bytes(&manifest).expect("tampered manifest bytes");

        let outcome = validate_manifest_car_replay_bytes(
            &tampered_manifest,
            &car_bytes,
            "tampered_manifest.to",
            "gateway.car",
            &config,
            123,
        );

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-CAR-001");
        assert_eq!(outcome.category, "validation");
        assert!(
            context_value(&outcome, "replay_error")
                .expect("replay error")
                .contains("manifest car digest mismatch")
        );
    }

    #[test]
    fn manifest_car_replay_rejects_malformed_manifest_norito() {
        let (_, car_bytes, config) = gateway_inputs();
        let outcome = validate_manifest_car_replay_bytes(
            b"not norito",
            &car_bytes,
            "bad_manifest.to",
            "gateway.car",
            &config,
            123,
        );

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-NORITO-001");
        assert_eq!(outcome.category, "norito");
    }

    #[test]
    fn manifest_car_replay_maps_manifest_policy_failure() {
        let (manifest_bytes, car_bytes, config) = gateway_inputs();
        let mut manifest: ManifestV1 = decode_from_bytes(&manifest_bytes).expect("manifest");
        manifest.version = manifest.version.saturating_add(1);
        let invalid_manifest = norito::to_bytes(&manifest).expect("invalid manifest bytes");

        let outcome = validate_manifest_car_replay_bytes(
            &invalid_manifest,
            &car_bytes,
            "invalid_manifest.to",
            "gateway.car",
            &config,
            123,
        );

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-VAL-002");
        assert_eq!(outcome.category, "validation");
    }
}
