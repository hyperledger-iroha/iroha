//! SoraCloud manifest schema for deterministic service hosting on Sora3.
//!
//! The initial SoraCloud release uses three canonical Norito payloads:
//! [`SoraContainerManifestV1`], [`SoraServiceManifestV1`], and
//! [`SoraStateBindingV1`]. Together they describe executable bundles,
//! deployment/routing policy, and state mutation limits in a deterministic form
//! suitable for validator admission and audit trails.

#![allow(clippy::module_name_repetitions)]

use std::{
    collections::BTreeSet,
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
};

use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::name::Name;

/// Schema version for [`SoraContainerManifestV1`].
pub const SORA_CONTAINER_MANIFEST_VERSION_V1: u16 = 1;
/// Schema version for [`SoraServiceManifestV1`].
pub const SORA_SERVICE_MANIFEST_VERSION_V1: u16 = 1;
/// Schema version for [`SoraStateBindingV1`].
pub const SORA_STATE_BINDING_VERSION_V1: u16 = 1;
/// Schema version for [`SoraDeploymentBundleV1`].
pub const SORA_DEPLOYMENT_BUNDLE_VERSION_V1: u16 = 1;

/// Validation errors returned by SoraCloud manifest helpers.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SoraCloudManifestError {
    /// The payload references an unsupported schema version.
    #[error("{manifest} schema version {found} is not supported (expected version {expected})")]
    UnsupportedVersion {
        /// Logical manifest type being validated.
        manifest: &'static str,
        /// Supported schema version.
        expected: u16,
        /// Encountered schema version.
        found: u16,
    },
    /// A required field was left empty.
    #[error("{manifest} field `{field}` must not be empty")]
    EmptyField {
        /// Logical manifest type being validated.
        manifest: &'static str,
        /// Name of the field that failed validation.
        field: &'static str,
    },
    /// A field violated deterministic or policy constraints.
    #[error("{manifest} field `{field}` is invalid: {reason}")]
    InvalidField {
        /// Logical manifest type being validated.
        manifest: &'static str,
        /// Name of the field that failed validation.
        field: &'static str,
        /// Human-readable reason.
        reason: String,
    },
    /// Service manifests cannot define duplicate state-binding names.
    #[error("sora service manifest includes duplicate state binding `{binding}`")]
    DuplicateStateBinding {
        /// Duplicate binding identifier.
        binding: Name,
    },
}

/// Runtime expected by the container manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "runtime", content = "value"))]
pub enum SoraContainerRuntimeV1 {
    /// Execute IVM bytecode entrypoints.
    #[default]
    Ivm,
    /// Execute a deterministic native process hosted by SCR.
    NativeProcess,
}

/// Network egress policy for a service container.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "mode", content = "value"))]
pub enum SoraNetworkPolicyV1 {
    /// No network egress is allowed.
    Isolated,
    /// Egress is allowed only to the listed hostnames.
    Allowlist(Vec<String>),
}

/// Capability policy enforced by the Sora Container Runtime.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SoraCapabilityPolicyV1 {
    /// Egress policy for outbound network access.
    pub network: SoraNetworkPolicyV1,
    /// Whether wallet/key signing capabilities are exposed to the service.
    pub allow_wallet_signing: bool,
    /// Whether deterministic key-value writes are allowed through bindings.
    pub allow_state_writes: bool,
    /// Whether model-training ops are allowed for this workload.
    pub allow_model_training: bool,
}

/// Resource limits for SCR process admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SoraResourceLimitsV1 {
    /// CPU budget in millicores.
    pub cpu_millis: NonZeroU32,
    /// Maximum resident memory in bytes.
    pub memory_bytes: NonZeroU64,
    /// Maximum ephemeral storage in bytes.
    pub ephemeral_storage_bytes: NonZeroU64,
    /// Maximum number of open file descriptors.
    pub max_open_files: NonZeroU32,
    /// Maximum number of cooperative tasks/threads.
    pub max_tasks: NonZeroU16,
}

/// Lifecycle hooks and probe settings used by SCR.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SoraLifecycleHooksV1 {
    /// Grace period allowed for service startup.
    pub start_grace_secs: NonZeroU32,
    /// Grace period allowed for service shutdown.
    pub stop_grace_secs: NonZeroU32,
    /// Optional HTTP health endpoint path.
    #[norito(default)]
    pub healthcheck_path: Option<String>,
}

/// Canonical executable bundle manifest for SoraCloud workloads.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SoraContainerManifestV1 {
    /// Schema version; must equal [`SORA_CONTAINER_MANIFEST_VERSION_V1`].
    pub schema_version: u16,
    /// Runtime target for the bundle.
    pub runtime: SoraContainerRuntimeV1,
    /// Digest of the code bundle stored in SoraFS.
    pub bundle_hash: Hash,
    /// Path inside the SoraFS bundle with executable payload.
    pub bundle_path: String,
    /// Entrypoint symbol or executable path.
    pub entrypoint: String,
    /// Static arguments passed at process startup.
    #[norito(default)]
    pub args: Vec<String>,
    /// Environment variables supplied at launch.
    #[norito(default)]
    pub env: std::collections::BTreeMap<String, String>,
    /// Capability policy enforced by SCR.
    pub capabilities: SoraCapabilityPolicyV1,
    /// Resource limits used at admission/runtime.
    pub resources: SoraResourceLimitsV1,
    /// Lifecycle and health probe settings.
    pub lifecycle: SoraLifecycleHooksV1,
}

impl SoraContainerManifestV1 {
    /// Validate schema version and deterministic constraints.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when schema versions mismatch or
    /// required fields are empty.
    pub fn validate(&self) -> Result<(), SoraCloudManifestError> {
        if self.schema_version != SORA_CONTAINER_MANIFEST_VERSION_V1 {
            return Err(SoraCloudManifestError::UnsupportedVersion {
                manifest: "sora container manifest",
                expected: SORA_CONTAINER_MANIFEST_VERSION_V1,
                found: self.schema_version,
            });
        }

        if self.bundle_path.trim().is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "sora container manifest",
                field: "bundle_path",
            });
        }

        if self.entrypoint.trim().is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "sora container manifest",
                field: "entrypoint",
            });
        }

        if let Some(path) = self.lifecycle.healthcheck_path.as_ref()
            && !path.starts_with('/')
        {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "sora container manifest",
                field: "lifecycle.healthcheck_path",
                reason: "must start with '/'".to_string(),
            });
        }

        Ok(())
    }
}

/// Public exposure mode for a service route.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "visibility", content = "value"))]
pub enum SoraRouteVisibilityV1 {
    /// Route is externally reachable.
    #[default]
    Public,
    /// Route is cluster-internal only.
    Internal,
}

/// TLS requirements for service ingress.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "tls", content = "value"))]
pub enum SoraTlsModeV1 {
    /// TLS is mandatory.
    #[default]
    Required,
    /// TLS is optional and may be terminated upstream.
    Optional,
    /// TLS is disabled.
    Disabled,
}

/// Route definition for a deployed service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SoraRouteTargetV1 {
    /// Hostname assigned by SoraDNS.
    pub host: String,
    /// Path prefix exposed by the service.
    pub path_prefix: String,
    /// Internal service port.
    pub service_port: NonZeroU16,
    /// Exposure scope for the route.
    pub visibility: SoraRouteVisibilityV1,
    /// TLS requirements for ingress.
    pub tls_mode: SoraTlsModeV1,
}

/// Rollout/upgrade behavior for the service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SoraRolloutPolicyV1 {
    /// Canary percentage applied before full rollout.
    pub canary_percent: u8,
    /// Maximum replicas unavailable during rollout.
    pub max_unavailable_replicas: u16,
    /// Rolling health window duration in seconds.
    pub health_window_secs: NonZeroU32,
    /// Consecutive health failures before auto rollback.
    pub automatic_rollback_failures: NonZeroU32,
}

/// Reference to a previously admitted container manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SoraContainerManifestRefV1 {
    /// Hash of the referenced container manifest bytes.
    pub manifest_hash: Hash,
    /// Expected schema version for the referenced manifest.
    pub expected_schema_version: u16,
}

/// State namespace addressed by a service binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "scope", content = "value"))]
pub enum SoraStateScopeV1 {
    /// Account metadata namespace.
    AccountMetadata,
    /// Domain metadata namespace.
    DomainMetadata,
    /// Trigger-scoped state namespace.
    TriggerState,
    /// Service-local runtime namespace.
    #[default]
    ServiceState,
    /// Confidential namespace for sensitive records.
    ConfidentialState,
}

/// Mutation mode allowed by the binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "mutability", content = "value"))]
pub enum SoraStateMutabilityV1 {
    /// Binding is read-only.
    #[default]
    ReadOnly,
    /// Binding supports append-only writes.
    AppendOnly,
    /// Binding supports full read/write updates.
    ReadWrite,
}

/// Encryption policy expected for values in the binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "encryption", content = "value"))]
pub enum SoraStateEncryptionV1 {
    /// Values are stored in plaintext.
    #[default]
    Plaintext,
    /// Values are client-encrypted before submission.
    ClientCiphertext,
    /// Values are FHE ciphertexts.
    FheCiphertext,
}

/// Deterministic state binding contract for an SCR service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SoraStateBindingV1 {
    /// Schema version; must equal [`SORA_STATE_BINDING_VERSION_V1`].
    pub schema_version: u16,
    /// Human-readable binding identifier.
    pub binding_name: Name,
    /// Canonical namespace targeted by this binding.
    pub scope: SoraStateScopeV1,
    /// Mutation class allowed by the binding.
    pub mutability: SoraStateMutabilityV1,
    /// Encryption policy for values under this binding.
    pub encryption: SoraStateEncryptionV1,
    /// Prefix that scopes all allowed keys.
    pub key_prefix: String,
    /// Maximum bytes per item written through this binding.
    pub max_item_bytes: NonZeroU64,
    /// Maximum cumulative bytes for the binding namespace.
    pub max_total_bytes: NonZeroU64,
}

impl SoraStateBindingV1 {
    /// Validate schema version and namespace limits.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when schema versions mismatch or
    /// binding fields violate deterministic constraints.
    pub fn validate(&self) -> Result<(), SoraCloudManifestError> {
        if self.schema_version != SORA_STATE_BINDING_VERSION_V1 {
            return Err(SoraCloudManifestError::UnsupportedVersion {
                manifest: "sora state binding",
                expected: SORA_STATE_BINDING_VERSION_V1,
                found: self.schema_version,
            });
        }

        if self.key_prefix.trim().is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "sora state binding",
                field: "key_prefix",
            });
        }

        if !self.key_prefix.starts_with('/') {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "sora state binding",
                field: "key_prefix",
                reason: "must start with '/'".to_string(),
            });
        }

        if self.max_item_bytes > self.max_total_bytes {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "sora state binding",
                field: "max_item_bytes",
                reason: "cannot exceed max_total_bytes".to_string(),
            });
        }

        if self.scope == SoraStateScopeV1::ConfidentialState
            && self.encryption == SoraStateEncryptionV1::Plaintext
        {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "sora state binding",
                field: "encryption",
                reason: "confidential state requires ciphertext encryption".to_string(),
            });
        }

        Ok(())
    }
}

/// Canonical deployment manifest for a routable SoraCloud service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SoraServiceManifestV1 {
    /// Schema version; must equal [`SORA_SERVICE_MANIFEST_VERSION_V1`].
    pub schema_version: u16,
    /// Logical service identifier.
    pub service_name: Name,
    /// Human-readable service version label.
    pub service_version: String,
    /// Reference to the executable container manifest.
    pub container: SoraContainerManifestRefV1,
    /// Desired replica count.
    pub replicas: NonZeroU16,
    /// Optional route exposure metadata.
    #[norito(default)]
    pub route: Option<SoraRouteTargetV1>,
    /// Rollout and rollback policy.
    pub rollout: SoraRolloutPolicyV1,
    /// State bindings exposed to the service.
    #[norito(default)]
    pub state_bindings: Vec<SoraStateBindingV1>,
}

impl SoraServiceManifestV1 {
    /// Validate schema version, routing constraints, and binding invariants.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when schema versions mismatch, route
    /// fields are invalid, or binding constraints fail.
    pub fn validate(&self) -> Result<(), SoraCloudManifestError> {
        if self.schema_version != SORA_SERVICE_MANIFEST_VERSION_V1 {
            return Err(SoraCloudManifestError::UnsupportedVersion {
                manifest: "sora service manifest",
                expected: SORA_SERVICE_MANIFEST_VERSION_V1,
                found: self.schema_version,
            });
        }

        if self.service_version.trim().is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "sora service manifest",
                field: "service_version",
            });
        }

        if self.container.expected_schema_version != SORA_CONTAINER_MANIFEST_VERSION_V1 {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "sora service manifest",
                field: "container.expected_schema_version",
                reason: format!(
                    "must equal {SORA_CONTAINER_MANIFEST_VERSION_V1}, found {}",
                    self.container.expected_schema_version
                ),
            });
        }

        if self.rollout.canary_percent > 100 {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "sora service manifest",
                field: "rollout.canary_percent",
                reason: "must be within 0..=100".to_string(),
            });
        }

        if let Some(route) = self.route.as_ref() {
            if route.host.trim().is_empty() {
                return Err(SoraCloudManifestError::EmptyField {
                    manifest: "sora service manifest",
                    field: "route.host",
                });
            }

            if !route.path_prefix.starts_with('/') {
                return Err(SoraCloudManifestError::InvalidField {
                    manifest: "sora service manifest",
                    field: "route.path_prefix",
                    reason: "must start with '/'".to_string(),
                });
            }
        }

        let mut seen = BTreeSet::new();
        for binding in &self.state_bindings {
            binding.validate()?;
            if !seen.insert(binding.binding_name.clone()) {
                return Err(SoraCloudManifestError::DuplicateStateBinding {
                    binding: binding.binding_name.clone(),
                });
            }
        }

        Ok(())
    }
}

/// Admission bundle coupling container + service manifests for deterministic checks.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SoraDeploymentBundleV1 {
    /// Schema version; must equal [`SORA_DEPLOYMENT_BUNDLE_VERSION_V1`].
    pub schema_version: u16,
    /// Container manifest referenced by the service.
    pub container: SoraContainerManifestV1,
    /// Routable service manifest.
    pub service: SoraServiceManifestV1,
}

impl SoraDeploymentBundleV1 {
    /// Compute the canonical hash of the container manifest.
    #[must_use]
    pub fn container_manifest_hash(&self) -> Hash {
        Hash::new(Encode::encode(&self.container))
    }

    /// Compute the canonical hash of the service manifest.
    #[must_use]
    pub fn service_manifest_hash(&self) -> Hash {
        Hash::new(Encode::encode(&self.service))
    }

    /// Validate deterministic admission constraints across container + service manifests.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when schema versions mismatch, internal
    /// manifest validation fails, manifest references are inconsistent, or
    /// capability/binding combinations are invalid.
    pub fn validate_for_admission(&self) -> Result<(), SoraCloudManifestError> {
        if self.schema_version != SORA_DEPLOYMENT_BUNDLE_VERSION_V1 {
            return Err(SoraCloudManifestError::UnsupportedVersion {
                manifest: "sora deployment bundle",
                expected: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
                found: self.schema_version,
            });
        }

        self.container.validate()?;
        self.service.validate()?;

        if self.service.container.expected_schema_version != self.container.schema_version {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "sora deployment bundle",
                field: "service.container.expected_schema_version",
                reason: format!(
                    "expected service container schema {} but container manifest declares {}",
                    self.service.container.expected_schema_version, self.container.schema_version
                ),
            });
        }

        let computed_hash = self.container_manifest_hash();
        if self.service.container.manifest_hash != computed_hash {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "sora deployment bundle",
                field: "service.container.manifest_hash",
                reason: format!(
                    "expected {}, found {}",
                    computed_hash, self.service.container.manifest_hash
                ),
            });
        }

        if !self.container.capabilities.allow_state_writes {
            for binding in &self.service.state_bindings {
                if binding.mutability != SoraStateMutabilityV1::ReadOnly {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "sora deployment bundle",
                        field: "container.capabilities.allow_state_writes",
                        reason: format!(
                            "binding `{}` requires mutable writes (`{:?}`)",
                            binding.binding_name, binding.mutability
                        ),
                    });
                }
            }
        }

        if matches!(
            self.service.route.as_ref().map(|route| route.visibility),
            Some(SoraRouteVisibilityV1::Public)
        ) && self.container.lifecycle.healthcheck_path.is_none()
        {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "sora deployment bundle",
                field: "container.lifecycle.healthcheck_path",
                reason: "public routes require an explicit healthcheck path".to_string(),
            });
        }

        Ok(())
    }
}

/// Re-export commonly used SoraCloud schema types.
pub mod prelude {
    pub use super::{
        SORA_CONTAINER_MANIFEST_VERSION_V1, SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        SORA_SERVICE_MANIFEST_VERSION_V1, SORA_STATE_BINDING_VERSION_V1, SoraCapabilityPolicyV1,
        SoraCloudManifestError, SoraContainerManifestRefV1, SoraContainerManifestV1,
        SoraContainerRuntimeV1, SoraDeploymentBundleV1, SoraLifecycleHooksV1, SoraNetworkPolicyV1,
        SoraResourceLimitsV1, SoraRolloutPolicyV1, SoraRouteTargetV1, SoraRouteVisibilityV1,
        SoraServiceManifestV1, SoraStateBindingV1, SoraStateEncryptionV1, SoraStateMutabilityV1,
        SoraStateScopeV1, SoraTlsModeV1,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample_hash(seed: u8) -> Hash {
        let mut bytes = [0u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = seed.wrapping_add(u8::try_from(index).expect("index fits in u8"));
        }
        Hash::prehashed(bytes)
    }

    fn sample_binding(name: &str) -> SoraStateBindingV1 {
        SoraStateBindingV1 {
            schema_version: SORA_STATE_BINDING_VERSION_V1,
            binding_name: name.parse().expect("valid name"),
            scope: SoraStateScopeV1::ServiceState,
            mutability: SoraStateMutabilityV1::ReadWrite,
            encryption: SoraStateEncryptionV1::ClientCiphertext,
            key_prefix: "/state/demo".to_string(),
            max_item_bytes: NonZeroU64::new(4_096).expect("nonzero"),
            max_total_bytes: NonZeroU64::new(65_536).expect("nonzero"),
        }
    }

    fn sample_container() -> SoraContainerManifestV1 {
        SoraContainerManifestV1 {
            schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
            runtime: SoraContainerRuntimeV1::Ivm,
            bundle_hash: sample_hash(7),
            bundle_path: "/bundles/site.to".to_string(),
            entrypoint: "main".to_string(),
            args: vec!["--http".to_string()],
            env: BTreeMap::from([("APP_ENV".to_string(), "prod".to_string())]),
            capabilities: SoraCapabilityPolicyV1 {
                network: SoraNetworkPolicyV1::Allowlist(vec![
                    "api.sora.internal".to_string(),
                    "rpc.sora.internal".to_string(),
                ]),
                allow_wallet_signing: true,
                allow_state_writes: true,
                allow_model_training: false,
            },
            resources: SoraResourceLimitsV1 {
                cpu_millis: NonZeroU32::new(750).expect("nonzero"),
                memory_bytes: NonZeroU64::new(536_870_912).expect("nonzero"),
                ephemeral_storage_bytes: NonZeroU64::new(2_147_483_648).expect("nonzero"),
                max_open_files: NonZeroU32::new(512).expect("nonzero"),
                max_tasks: NonZeroU16::new(64).expect("nonzero"),
            },
            lifecycle: SoraLifecycleHooksV1 {
                start_grace_secs: NonZeroU32::new(30).expect("nonzero"),
                stop_grace_secs: NonZeroU32::new(15).expect("nonzero"),
                healthcheck_path: Some("/healthz".to_string()),
            },
        }
    }

    fn sample_service(state_bindings: Vec<SoraStateBindingV1>) -> SoraServiceManifestV1 {
        SoraServiceManifestV1 {
            schema_version: SORA_SERVICE_MANIFEST_VERSION_V1,
            service_name: "portal".parse().expect("valid name"),
            service_version: "2026.1".to_string(),
            container: SoraContainerManifestRefV1 {
                manifest_hash: sample_hash(23),
                expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
            },
            replicas: NonZeroU16::new(3).expect("nonzero"),
            route: Some(SoraRouteTargetV1 {
                host: "portal.sora".to_string(),
                path_prefix: "/app".to_string(),
                service_port: NonZeroU16::new(8081).expect("nonzero"),
                visibility: SoraRouteVisibilityV1::Public,
                tls_mode: SoraTlsModeV1::Required,
            }),
            rollout: SoraRolloutPolicyV1 {
                canary_percent: 20,
                max_unavailable_replicas: 1,
                health_window_secs: NonZeroU32::new(60).expect("nonzero"),
                automatic_rollback_failures: NonZeroU32::new(2).expect("nonzero"),
            },
            state_bindings,
        }
    }

    #[test]
    fn state_binding_validate_rejects_plaintext_confidential_scope() {
        let mut binding = sample_binding("private_state");
        binding.scope = SoraStateScopeV1::ConfidentialState;
        binding.encryption = SoraStateEncryptionV1::Plaintext;
        let error = binding
            .validate()
            .expect_err("plaintext confidential state must be rejected");
        assert!(matches!(
            error,
            SoraCloudManifestError::InvalidField {
                field: "encryption",
                ..
            }
        ));
    }

    #[test]
    fn container_validate_rejects_invalid_healthcheck_path() {
        let mut container = sample_container();
        container.lifecycle.healthcheck_path = Some("healthz".to_string());
        let error = container
            .validate()
            .expect_err("healthcheck path must start with slash");
        assert!(matches!(
            error,
            SoraCloudManifestError::InvalidField {
                field: "lifecycle.healthcheck_path",
                ..
            }
        ));
    }

    #[test]
    fn service_validate_rejects_duplicate_binding_names() {
        let binding = sample_binding("session_store");
        let manifest = SoraServiceManifestV1 {
            schema_version: SORA_SERVICE_MANIFEST_VERSION_V1,
            service_name: "wallet".parse().expect("valid name"),
            service_version: "1.0.0".to_string(),
            container: SoraContainerManifestRefV1 {
                manifest_hash: sample_hash(13),
                expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
            },
            replicas: NonZeroU16::new(2).expect("nonzero"),
            route: Some(SoraRouteTargetV1 {
                host: "wallet.sora".to_string(),
                path_prefix: "/".to_string(),
                service_port: NonZeroU16::new(8080).expect("nonzero"),
                visibility: SoraRouteVisibilityV1::Public,
                tls_mode: SoraTlsModeV1::Required,
            }),
            rollout: SoraRolloutPolicyV1 {
                canary_percent: 10,
                max_unavailable_replicas: 1,
                health_window_secs: NonZeroU32::new(45).expect("nonzero"),
                automatic_rollback_failures: NonZeroU32::new(3).expect("nonzero"),
            },
            state_bindings: vec![binding.clone(), binding],
        };

        let error = manifest
            .validate()
            .expect_err("duplicate state binding names must fail");
        assert!(matches!(
            error,
            SoraCloudManifestError::DuplicateStateBinding { .. }
        ));
    }

    #[test]
    fn service_validate_accepts_valid_manifest() {
        let manifest = sample_service(vec![sample_binding("session"), sample_binding("profiles")]);

        assert!(manifest.validate().is_ok(), "valid manifest should pass");
    }

    #[test]
    fn deployment_bundle_validate_rejects_container_hash_mismatch() {
        let container = sample_container();
        let mut service = sample_service(vec![sample_binding("session")]);
        service.container.manifest_hash = sample_hash(99);
        let bundle = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        };
        let error = bundle
            .validate_for_admission()
            .expect_err("mismatched container hash must fail admission");
        assert!(matches!(
            error,
            SoraCloudManifestError::InvalidField {
                field: "service.container.manifest_hash",
                ..
            }
        ));
    }

    #[test]
    fn deployment_bundle_validate_rejects_mutable_binding_without_write_capability() {
        let mut container = sample_container();
        container.capabilities.allow_state_writes = false;
        let container_hash = Hash::new(Encode::encode(&container));
        let mut service = sample_service(vec![sample_binding("session")]);
        service.container.manifest_hash = container_hash;
        let bundle = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        };
        let error = bundle
            .validate_for_admission()
            .expect_err("mutable bindings require state-write capability");
        assert!(matches!(
            error,
            SoraCloudManifestError::InvalidField {
                field: "container.capabilities.allow_state_writes",
                ..
            }
        ));
    }

    #[test]
    fn deployment_bundle_validate_accepts_consistent_bundle() {
        let container = sample_container();
        let container_hash = Hash::new(Encode::encode(&container));
        let mut service = sample_service(vec![
            sample_binding("session"),
            SoraStateBindingV1 {
                schema_version: SORA_STATE_BINDING_VERSION_V1,
                binding_name: "read_only_profile".parse().expect("valid name"),
                scope: SoraStateScopeV1::AccountMetadata,
                mutability: SoraStateMutabilityV1::ReadOnly,
                encryption: SoraStateEncryptionV1::ClientCiphertext,
                key_prefix: "/state/profile".to_string(),
                max_item_bytes: NonZeroU64::new(2_048).expect("nonzero"),
                max_total_bytes: NonZeroU64::new(65_536).expect("nonzero"),
            },
        ]);
        service.container.manifest_hash = container_hash;
        let bundle = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        };
        assert!(
            bundle.validate_for_admission().is_ok(),
            "consistent deployment bundle must pass"
        );
    }
}
