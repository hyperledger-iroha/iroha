//! Validation helpers for manifests destined for the Pin Registry.

#![allow(unexpected_cfgs)]

use std::collections::BTreeSet;

use crate::{
    GovernanceProofs, MANIFEST_VERSION_V1, ManifestV1, PinPolicy, ProfileId, StorageClass,
    chunker_registry,
};

/// Constraints applied to the pin policy during manifest validation.
#[derive(Debug, Clone)]
pub struct PinPolicyConstraints {
    /// Minimum number of replicas that governance policy requires.
    pub min_replicas_floor: u16,
    /// Optional maximum replicas ceiling.
    pub max_replicas_ceiling: Option<u16>,
    /// Optional upper bound for retention epoch (inclusive).
    pub max_retention_epoch: Option<u64>,
    /// Allowed storage classes. When omitted, any storage class is accepted.
    pub allowed_storage_classes: Option<BTreeSet<StorageClass>>,
}

impl Default for PinPolicyConstraints {
    fn default() -> Self {
        Self {
            min_replicas_floor: 1,
            max_replicas_ceiling: None,
            max_retention_epoch: None,
            allowed_storage_classes: None,
        }
    }
}

/// Errors surfaced while validating a manifest for registry submission.
#[derive(Debug, thiserror::Error)]
pub enum ManifestValidationError {
    #[error("unsupported manifest version {found}; expected {expected}")]
    UnsupportedVersion { expected: u8, found: u8 },
    #[error("chunker profile id {profile_id} is not registered")]
    UnknownChunkerProfile { profile_id: u32 },
    #[error("chunker descriptor mismatch for field {field}: expected {expected}, found {found}")]
    ChunkerDescriptorMismatch {
        field: &'static str,
        expected: String,
        found: String,
    },
    #[error("manifest advertises unexpected chunker alias `{alias}`")]
    UnknownChunkerAlias { alias: String },
    #[error("manifest chunker aliases are missing canonical handle `{canonical}`")]
    MissingCanonicalAlias { canonical: String },
    #[error("pin policy requires at least {required} replicas but manifest specifies {found}")]
    MinReplicasTooLow { required: u16, found: u16 },
    #[error("pin policy exceeds maximum replicas {maximum}; manifest specifies {found}")]
    MaxReplicasExceeded { maximum: u16, found: u16 },
    #[error("pin retention epoch must be <= {maximum}; manifest specifies {found}")]
    RetentionEpochExceeded { maximum: u64, found: u64 },
    #[error("storage class `{found:?}` is not permitted by policy")]
    StorageClassNotAllowed { found: StorageClass },
    #[error("manifest must include at least one council signature")]
    MissingCouncilSignature,
}

/// Validates the manifest according to registry policy.
pub fn validate_manifest(
    manifest: &ManifestV1,
    policy: &PinPolicyConstraints,
) -> Result<(), ManifestValidationError> {
    if manifest.version != MANIFEST_VERSION_V1 {
        return Err(ManifestValidationError::UnsupportedVersion {
            expected: MANIFEST_VERSION_V1,
            found: manifest.version,
        });
    }

    validate_chunker_handle(
        manifest.chunking.profile_id,
        &manifest.chunking.namespace,
        &manifest.chunking.name,
        &manifest.chunking.semver,
        manifest.chunking.multihash_code,
        Some(&manifest.chunking.aliases),
    )?;
    validate_pin_policy(&manifest.pin_policy, policy)?;
    validate_governance(&manifest.governance)?;

    Ok(())
}

/// Validates chunker metadata against the registry.
#[allow(clippy::needless_pass_by_value)]
pub fn validate_chunker_handle(
    profile_id: ProfileId,
    namespace: &str,
    name: &str,
    semver: &str,
    multihash_code: u64,
    aliases: Option<&[String]>,
) -> Result<&'static chunker_registry::ChunkerProfileDescriptor, ManifestValidationError> {
    let descriptor = chunker_registry::lookup(profile_id).ok_or(
        ManifestValidationError::UnknownChunkerProfile {
            profile_id: profile_id.0,
        },
    )?;

    if namespace != descriptor.namespace {
        return Err(ManifestValidationError::ChunkerDescriptorMismatch {
            field: "namespace",
            expected: descriptor.namespace.to_owned(),
            found: namespace.to_owned(),
        });
    }
    if name != descriptor.name {
        return Err(ManifestValidationError::ChunkerDescriptorMismatch {
            field: "name",
            expected: descriptor.name.to_owned(),
            found: name.to_owned(),
        });
    }
    if semver != descriptor.semver {
        return Err(ManifestValidationError::ChunkerDescriptorMismatch {
            field: "semver",
            expected: descriptor.semver.to_owned(),
            found: semver.to_owned(),
        });
    }
    if multihash_code != descriptor.multihash_code {
        return Err(ManifestValidationError::ChunkerDescriptorMismatch {
            field: "multihash_code",
            expected: descriptor.multihash_code.to_string(),
            found: multihash_code.to_string(),
        });
    }

    if let Some(aliases) = aliases {
        let expected_aliases: BTreeSet<String> = descriptor
            .aliases
            .iter()
            .map(|value| value.to_string())
            .collect();
        let provided_aliases: BTreeSet<String> = aliases.iter().cloned().collect();

        for alias in &provided_aliases {
            if !expected_aliases.contains(alias) {
                return Err(ManifestValidationError::UnknownChunkerAlias {
                    alias: alias.clone(),
                });
            }
        }

        let canonical = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        if !provided_aliases.contains(&canonical) {
            return Err(ManifestValidationError::MissingCanonicalAlias { canonical });
        }
    }

    Ok(descriptor)
}

pub fn validate_pin_policy(
    pin_policy: &PinPolicy,
    constraints: &PinPolicyConstraints,
) -> Result<(), ManifestValidationError> {
    if pin_policy.min_replicas < constraints.min_replicas_floor {
        return Err(ManifestValidationError::MinReplicasTooLow {
            required: constraints.min_replicas_floor,
            found: pin_policy.min_replicas,
        });
    }
    if let Some(maximum) = constraints.max_replicas_ceiling
        && pin_policy.min_replicas > maximum
    {
        return Err(ManifestValidationError::MaxReplicasExceeded {
            maximum,
            found: pin_policy.min_replicas,
        });
    }
    if let Some(maximum) = constraints.max_retention_epoch
        && pin_policy.retention_epoch > maximum
    {
        return Err(ManifestValidationError::RetentionEpochExceeded {
            maximum,
            found: pin_policy.retention_epoch,
        });
    }
    if constraints
        .allowed_storage_classes
        .as_ref()
        .is_some_and(|allowed| !allowed.contains(&pin_policy.storage_class))
    {
        return Err(ManifestValidationError::StorageClassNotAllowed {
            found: pin_policy.storage_class,
        });
    }

    Ok(())
}

fn validate_governance(proofs: &GovernanceProofs) -> Result<(), ManifestValidationError> {
    if proofs.council_signatures.is_empty() {
        return Err(ManifestValidationError::MissingCouncilSignature);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CouncilSignature, GovernanceProofs, ManifestBuilder};

    fn manifest_with_defaults() -> ManifestV1 {
        ManifestBuilder::new()
            .root_cid(vec![0x01, 0x55, 0xaa])
            .dag_codec(crate::DagCodecId(chunker_registry::MANIFEST_DAG_CODEC))
            .chunking_from_registry(chunker_registry::default_descriptor().id)
            .content_length(1_048_576)
            .car_digest([0xAB; 32])
            .car_size(1_100_000)
            .pin_policy(PinPolicy {
                min_replicas: 3,
                storage_class: StorageClass::Hot,
                retention_epoch: 10,
            })
            .governance(GovernanceProofs {
                council_signatures: vec![CouncilSignature {
                    signer: [0x11; 32],
                    signature: vec![0x22; 64],
                }],
            })
            .build()
            .expect("manifest")
    }

    fn default_constraints() -> PinPolicyConstraints {
        PinPolicyConstraints {
            min_replicas_floor: 1,
            max_replicas_ceiling: Some(5),
            max_retention_epoch: Some(20),
            allowed_storage_classes: None,
        }
    }

    #[test]
    fn validates_manifest_successfully() {
        let manifest = manifest_with_defaults();
        let constraints = default_constraints();

        let result = validate_manifest(&manifest, &constraints);

        assert!(result.is_ok());
    }

    #[test]
    fn rejects_unknown_chunker_profile() {
        let mut manifest = manifest_with_defaults();
        manifest.chunking.profile_id = crate::ProfileId(u32::MAX);

        let err = validate_manifest(&manifest, &default_constraints()).expect_err("should fail");

        matches!(
            err,
            ManifestValidationError::UnknownChunkerProfile { profile_id }
            if profile_id == u32::MAX
        );
    }

    #[test]
    fn rejects_chunker_metadata_mismatch() {
        let mut manifest = manifest_with_defaults();
        manifest.chunking.semver = "2.0.0".to_string();

        let err = validate_manifest(&manifest, &default_constraints()).expect_err("should fail");

        matches!(
            err,
            ManifestValidationError::ChunkerDescriptorMismatch {
                field,
                ..
            } if field == "semver"
        );
    }

    #[test]
    fn rejects_alias_without_canonical() {
        let mut manifest = manifest_with_defaults();
        manifest.chunking.aliases.clear();
        manifest.chunking.aliases.push("sorafs.sf1".to_string());

        let err = validate_manifest(&manifest, &default_constraints()).expect_err("should fail");

        matches!(err, ManifestValidationError::MissingCanonicalAlias { .. });
    }

    #[test]
    fn enforces_pin_policy_constraints() {
        let mut manifest = manifest_with_defaults();
        manifest.pin_policy.min_replicas = 0;

        let err = validate_manifest(
            &manifest,
            &PinPolicyConstraints {
                min_replicas_floor: 1,
                max_replicas_ceiling: Some(5),
                max_retention_epoch: Some(20),
                allowed_storage_classes: None,
            },
        )
        .expect_err("should fail");
        matches!(
            err,
            ManifestValidationError::MinReplicasTooLow { required, found }
            if required == 1 && found == 0
        );
    }

    #[test]
    fn rejects_missing_signatures() {
        let mut manifest = manifest_with_defaults();
        manifest.governance.council_signatures.clear();

        let err = validate_manifest(&manifest, &default_constraints()).expect_err("should fail");

        matches!(err, ManifestValidationError::MissingCouncilSignature);
    }

    #[test]
    fn chunker_handle_valid_without_aliases() {
        let descriptor = chunker_registry::default_descriptor();
        validate_chunker_handle(
            descriptor.id,
            descriptor.namespace,
            descriptor.name,
            descriptor.semver,
            descriptor.multihash_code,
            None,
        )
        .expect("handle validates");
    }

    #[test]
    fn pin_policy_helper_enforces_constraints() {
        let policy = PinPolicy {
            min_replicas: 0,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        };
        let err = validate_pin_policy(
            &policy,
            &PinPolicyConstraints {
                min_replicas_floor: 1,
                max_replicas_ceiling: Some(5),
                max_retention_epoch: None,
                allowed_storage_classes: None,
            },
        )
        .expect_err("policy should fail");
        matches!(err, ManifestValidationError::MinReplicasTooLow { .. });
    }
}
