//! SoraCloud manifest schema for deterministic service hosting on Sora3.
//!
//! The initial SoraCloud release uses canonical Norito payloads:
//! [`SoraContainerManifestV1`], [`SoraServiceManifestV1`],
//! [`SoraStateBindingV1`], [`AgentApartmentManifestV1`], [`FheParamSetV1`],
//! and [`FheExecutionPolicyV1`]. Together they describe executable bundles,
//! deployment/routing policy, state mutation limits, agent-policy envelopes,
//! and deterministic confidential-compute policy in a form suitable for
//! validator admission and audit trails.

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
/// Schema version for [`AgentApartmentManifestV1`].
pub const AGENT_APARTMENT_MANIFEST_VERSION_V1: u16 = 1;
/// Schema version for [`FheParamSetV1`].
pub const FHE_PARAM_SET_VERSION_V1: u16 = 1;
/// Schema version for [`FheExecutionPolicyV1`].
pub const FHE_EXECUTION_POLICY_VERSION_V1: u16 = 1;
/// Schema version for [`FheGovernanceBundleV1`].
pub const FHE_GOVERNANCE_BUNDLE_VERSION_V1: u16 = 1;
/// Schema version for [`SecretEnvelopeV1`].
pub const SECRET_ENVELOPE_VERSION_V1: u16 = 1;
/// Schema version for [`CiphertextStateRecordV1`].
pub const CIPHERTEXT_STATE_RECORD_VERSION_V1: u16 = 1;
/// Schema version for [`FheJobSpecV1`].
pub const FHE_JOB_SPEC_VERSION_V1: u16 = 1;

/// Validation errors returned by `SoraCloud` manifest helpers.
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
    /// Agent apartment manifests cannot define duplicate tool capabilities.
    #[error("agent apartment manifest includes duplicate tool capability `{tool}`")]
    DuplicateToolCapability {
        /// Duplicate tool identifier.
        tool: String,
    },
    /// Agent apartment manifests cannot define duplicate policy capabilities.
    #[error("agent apartment manifest includes duplicate policy capability `{policy}`")]
    DuplicatePolicyCapability {
        /// Duplicate policy capability identifier.
        policy: Name,
    },
    /// Agent apartment manifests cannot define duplicate spend-limit assets.
    #[error("agent apartment manifest includes duplicate spend-limit asset `{asset}`")]
    DuplicateSpendLimitAsset {
        /// Duplicate spend-limit asset identifier.
        asset: String,
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

/// Canonical executable bundle manifest for `SoraCloud` workloads.
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
    /// Digest of the code bundle stored in `SoraFS`.
    pub bundle_hash: Hash,
    /// Path inside the `SoraFS` bundle with executable payload.
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
    /// Hostname assigned by `SoraDNS`.
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

/// Canonical deployment manifest for a routable `SoraCloud` service.
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

/// Upgrade mode for long-lived AI agent apartments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "upgrade_policy", content = "value"))]
pub enum AgentUpgradePolicyV1 {
    /// Apartments can only be upgraded through explicit governance actions.
    #[default]
    Governed,
    /// Apartments can be upgraded automatically once checks pass.
    Automatic,
    /// Apartment revision is pinned and cannot be upgraded.
    Pinned,
}

/// Tool-level execution cap for an agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AgentToolCapabilityV1 {
    /// Stable tool identifier.
    pub tool: String,
    /// Maximum invocations allowed per accounting epoch.
    pub max_invocations_per_epoch: NonZeroU32,
    /// Whether the tool may perform network egress.
    pub allow_network: bool,
    /// Whether the tool may write to local persistent files.
    pub allow_filesystem_write: bool,
}

/// Spend guardrail for a specific asset under apartment policy.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AgentSpendLimitV1 {
    /// Asset definition identifier (for example `xor#sora`).
    pub asset_definition: String,
    /// Maximum amount spendable per transaction, in nanos.
    pub max_per_tx_nanos: NonZeroU64,
    /// Maximum amount spendable per day, in nanos.
    pub max_per_day_nanos: NonZeroU64,
}

/// Deterministic policy manifest for a persistent AI agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AgentApartmentManifestV1 {
    /// Schema version; must equal [`AGENT_APARTMENT_MANIFEST_VERSION_V1`].
    pub schema_version: u16,
    /// Logical apartment identifier.
    pub apartment_name: Name,
    /// Reference to the executable container manifest.
    pub container: SoraContainerManifestRefV1,
    /// Tool-level capability policy.
    #[norito(default)]
    pub tool_capabilities: Vec<AgentToolCapabilityV1>,
    /// Additional high-level policy capability identifiers.
    #[norito(default)]
    pub policy_capabilities: Vec<Name>,
    /// Wallet spend limits across allowed assets.
    #[norito(default)]
    pub spend_limits: Vec<AgentSpendLimitV1>,
    /// Total state quota reserved for apartment memory.
    pub state_quota_bytes: NonZeroU64,
    /// Apartment-wide network egress policy.
    pub network_egress: SoraNetworkPolicyV1,
    /// Upgrade policy for apartment revisions.
    pub upgrade_policy: AgentUpgradePolicyV1,
}

impl AgentApartmentManifestV1 {
    /// Validate schema version and deterministic policy constraints.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when schema versions mismatch or
    /// policy fields violate deterministic constraints.
    pub fn validate(&self) -> Result<(), SoraCloudManifestError> {
        if self.schema_version != AGENT_APARTMENT_MANIFEST_VERSION_V1 {
            return Err(SoraCloudManifestError::UnsupportedVersion {
                manifest: "agent apartment manifest",
                expected: AGENT_APARTMENT_MANIFEST_VERSION_V1,
                found: self.schema_version,
            });
        }

        if self.container.expected_schema_version != SORA_CONTAINER_MANIFEST_VERSION_V1 {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "agent apartment manifest",
                field: "container.expected_schema_version",
                reason: format!(
                    "must equal {SORA_CONTAINER_MANIFEST_VERSION_V1}, found {}",
                    self.container.expected_schema_version
                ),
            });
        }

        let mut seen_tools = BTreeSet::new();
        for tool_capability in &self.tool_capabilities {
            let tool = tool_capability.tool.trim();
            if tool.is_empty() {
                return Err(SoraCloudManifestError::EmptyField {
                    manifest: "agent apartment manifest",
                    field: "tool_capabilities.tool",
                });
            }
            if !seen_tools.insert(tool.to_owned()) {
                return Err(SoraCloudManifestError::DuplicateToolCapability {
                    tool: tool.to_owned(),
                });
            }
        }

        let mut seen_policies = BTreeSet::new();
        for policy in &self.policy_capabilities {
            if !seen_policies.insert(policy.clone()) {
                return Err(SoraCloudManifestError::DuplicatePolicyCapability {
                    policy: policy.clone(),
                });
            }
        }

        let mut seen_spend_assets = BTreeSet::new();
        for limit in &self.spend_limits {
            let asset = limit.asset_definition.trim();
            if asset.is_empty() {
                return Err(SoraCloudManifestError::EmptyField {
                    manifest: "agent apartment manifest",
                    field: "spend_limits.asset_definition",
                });
            }
            if limit.max_per_tx_nanos > limit.max_per_day_nanos {
                return Err(SoraCloudManifestError::InvalidField {
                    manifest: "agent apartment manifest",
                    field: "spend_limits.max_per_tx_nanos",
                    reason: "cannot exceed max_per_day_nanos".to_string(),
                });
            }
            if !seen_spend_assets.insert(asset.to_owned()) {
                return Err(SoraCloudManifestError::DuplicateSpendLimitAsset {
                    asset: asset.to_owned(),
                });
            }
        }

        if let SoraNetworkPolicyV1::Allowlist(hosts) = &self.network_egress {
            if hosts.is_empty() {
                return Err(SoraCloudManifestError::InvalidField {
                    manifest: "agent apartment manifest",
                    field: "network_egress",
                    reason: "allowlist must include at least one host".to_string(),
                });
            }
            let mut seen_hosts = BTreeSet::new();
            for host in hosts {
                let normalized = host.trim();
                if normalized.is_empty() {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "agent apartment manifest",
                        field: "network_egress",
                        reason: "allowlist host entries must be non-empty".to_string(),
                    });
                }
                if !seen_hosts.insert(normalized.to_owned()) {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "agent apartment manifest",
                        field: "network_egress",
                        reason: format!("duplicate allowlist host `{normalized}`"),
                    });
                }
            }
        }

        Ok(())
    }
}

/// Fully homomorphic encryption scheme family used by a parameter set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "scheme", content = "value"))]
pub enum FheSchemeV1 {
    /// Brakerski/Fan-Vercauteren integer arithmetic scheme.
    #[default]
    Bfv,
    /// Brakerski-Gentry-Vaikuntanathan integer arithmetic scheme.
    Bgv,
    /// Approximate arithmetic CKKS scheme.
    Ckks,
}

/// Governance lifecycle state for a registered FHE parameter set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "lifecycle", content = "value"))]
pub enum FheParamLifecycleV1 {
    /// Parameter set is published and awaiting activation.
    #[default]
    Proposed,
    /// Parameter set is active and may be used for job admission.
    Active,
    /// Parameter set is still valid but scheduled for migration/retirement.
    Deprecated,
    /// Parameter set is withdrawn and must be rejected for new jobs.
    Withdrawn,
}

/// Governance-managed FHE parameter-set descriptor for SoraCloud workloads.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FheParamSetV1 {
    /// Schema version; must equal [`FHE_PARAM_SET_VERSION_V1`].
    pub schema_version: u16,
    /// Stable on-chain identifier for the parameter family.
    pub param_set: Name,
    /// Monotonic version number under the same `param_set` name.
    pub version: NonZeroU32,
    /// Backend profile identifier (`fhe/bfv-rns/v1`, etc.).
    pub backend: String,
    /// Cryptosystem family used by this parameter set.
    pub scheme: FheSchemeV1,
    /// RNS modulus chain in bits, canonical order from highest to lowest level.
    #[norito(default)]
    pub ciphertext_modulus_bits: Vec<NonZeroU16>,
    /// Plaintext modulus size in bits.
    pub plaintext_modulus_bits: NonZeroU16,
    /// Polynomial modulus degree.
    pub polynomial_modulus_degree: NonZeroU32,
    /// Number of plaintext slots exposed by this profile.
    pub slot_count: NonZeroU32,
    /// Minimum targeted security level in bits.
    pub security_level_bits: NonZeroU16,
    /// Maximum admissible multiplication depth under this chain.
    pub max_multiplicative_depth: NonZeroU16,
    /// Governance lifecycle state.
    pub lifecycle: FheParamLifecycleV1,
    /// First block where this set can be admitted.
    #[norito(default)]
    pub activation_height: Option<u64>,
    /// Optional block where this set enters deprecation.
    #[norito(default)]
    pub deprecation_height: Option<u64>,
    /// Optional block where this set is fully withdrawn.
    #[norito(default)]
    pub withdraw_height: Option<u64>,
    /// Canonical digest of backend parameter bytes.
    pub parameter_digest: Hash,
}

impl FheParamSetV1 {
    /// Validate schema version and deterministic lifecycle constraints.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when schema versions mismatch or
    /// parameter/lifecycle fields violate deterministic governance rules.
    pub fn validate(&self) -> Result<(), SoraCloudManifestError> {
        if self.schema_version != FHE_PARAM_SET_VERSION_V1 {
            return Err(SoraCloudManifestError::UnsupportedVersion {
                manifest: "fhe parameter set",
                expected: FHE_PARAM_SET_VERSION_V1,
                found: self.schema_version,
            });
        }

        if self.backend.trim().is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "fhe parameter set",
                field: "backend",
            });
        }

        if self.ciphertext_modulus_bits.is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "fhe parameter set",
                field: "ciphertext_modulus_bits",
            });
        }

        let mut previous_bits = u16::MAX;
        for modulus_bits in &self.ciphertext_modulus_bits {
            let current = modulus_bits.get();
            if !(2..=120).contains(&current) {
                return Err(SoraCloudManifestError::InvalidField {
                    manifest: "fhe parameter set",
                    field: "ciphertext_modulus_bits",
                    reason: format!("value {current} must be within 2..=120"),
                });
            }
            if current > previous_bits {
                return Err(SoraCloudManifestError::InvalidField {
                    manifest: "fhe parameter set",
                    field: "ciphertext_modulus_bits",
                    reason: "chain must be non-increasing".to_string(),
                });
            }
            previous_bits = current;
        }

        let largest_modulus = self
            .ciphertext_modulus_bits
            .first()
            .expect("ciphertext modulus chain is non-empty due prior check")
            .get();
        if self.plaintext_modulus_bits.get() >= largest_modulus {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe parameter set",
                field: "plaintext_modulus_bits",
                reason: format!(
                    "must be smaller than the largest ciphertext modulus ({largest_modulus})"
                ),
            });
        }

        if self.slot_count > self.polynomial_modulus_degree {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe parameter set",
                field: "slot_count",
                reason: "cannot exceed polynomial_modulus_degree".to_string(),
            });
        }

        let chain_len = u16::try_from(self.ciphertext_modulus_bits.len()).map_err(|_| {
            SoraCloudManifestError::InvalidField {
                manifest: "fhe parameter set",
                field: "ciphertext_modulus_bits",
                reason: "chain length exceeds supported u16 range".to_string(),
            }
        })?;
        if self.max_multiplicative_depth.get() >= chain_len {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe parameter set",
                field: "max_multiplicative_depth",
                reason: format!(
                    "must be smaller than ciphertext modulus chain length ({chain_len})"
                ),
            });
        }

        if let Some(deprecation_height) = self.deprecation_height {
            let Some(activation_height) = self.activation_height else {
                return Err(SoraCloudManifestError::InvalidField {
                    manifest: "fhe parameter set",
                    field: "deprecation_height",
                    reason: "requires activation_height".to_string(),
                });
            };
            if deprecation_height <= activation_height {
                return Err(SoraCloudManifestError::InvalidField {
                    manifest: "fhe parameter set",
                    field: "deprecation_height",
                    reason: "must be strictly greater than activation_height".to_string(),
                });
            }
        }

        if let Some(withdraw_height) = self.withdraw_height {
            let Some(activation_height) = self.activation_height else {
                return Err(SoraCloudManifestError::InvalidField {
                    manifest: "fhe parameter set",
                    field: "withdraw_height",
                    reason: "requires activation_height".to_string(),
                });
            };
            if withdraw_height <= activation_height {
                return Err(SoraCloudManifestError::InvalidField {
                    manifest: "fhe parameter set",
                    field: "withdraw_height",
                    reason: "must be strictly greater than activation_height".to_string(),
                });
            }
        }

        if let (Some(deprecation_height), Some(withdraw_height)) =
            (self.deprecation_height, self.withdraw_height)
            && withdraw_height <= deprecation_height
        {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe parameter set",
                field: "withdraw_height",
                reason: "must be strictly greater than deprecation_height".to_string(),
            });
        }

        match self.lifecycle {
            FheParamLifecycleV1::Proposed => {
                if self.deprecation_height.is_some() || self.withdraw_height.is_some() {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe parameter set",
                        field: "lifecycle",
                        reason: "proposed sets cannot define deprecation/withdraw heights"
                            .to_string(),
                    });
                }
            }
            FheParamLifecycleV1::Active => {
                if self.activation_height.is_none() {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe parameter set",
                        field: "lifecycle",
                        reason: "active sets require activation_height".to_string(),
                    });
                }
            }
            FheParamLifecycleV1::Deprecated => {
                if self.activation_height.is_none() || self.deprecation_height.is_none() {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe parameter set",
                        field: "lifecycle",
                        reason: "deprecated sets require activation_height and deprecation_height"
                            .to_string(),
                    });
                }
            }
            FheParamLifecycleV1::Withdrawn => {
                if self.activation_height.is_none() || self.withdraw_height.is_none() {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe parameter set",
                        field: "lifecycle",
                        reason: "withdrawn sets require activation_height and withdraw_height"
                            .to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}

/// Rounding mode used for deterministic ciphertext arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "rounding_mode", content = "value"))]
pub enum FheDeterministicRoundingModeV1 {
    /// Always round toward negative infinity.
    Floor,
    /// Round to nearest value; ties resolve to even.
    #[default]
    NearestTiesToEven,
}

/// Deterministic execution policy for validator-side ciphertext operations.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FheExecutionPolicyV1 {
    /// Schema version; must equal [`FHE_EXECUTION_POLICY_VERSION_V1`].
    pub schema_version: u16,
    /// Stable policy identifier.
    pub policy_name: Name,
    /// Referenced parameter-set name.
    pub param_set: Name,
    /// Referenced parameter-set version.
    pub param_set_version: NonZeroU32,
    /// Maximum admitted ciphertext size in bytes.
    pub max_ciphertext_bytes: NonZeroU64,
    /// Maximum admitted plaintext input size in bytes.
    pub max_plaintext_bytes: NonZeroU64,
    /// Maximum ciphertext inputs per operation.
    pub max_input_ciphertexts: NonZeroU16,
    /// Maximum ciphertext outputs per operation.
    pub max_output_ciphertexts: NonZeroU16,
    /// Maximum multiplication depth requested by an admitted job.
    pub max_multiplication_depth: NonZeroU16,
    /// Maximum homomorphic rotations per job.
    pub max_rotation_count: NonZeroU32,
    /// Maximum bootstrap operations per job.
    pub max_bootstrap_count: u16,
    /// Canonical rounding mode used by evaluators.
    pub rounding_mode: FheDeterministicRoundingModeV1,
}

impl FheExecutionPolicyV1 {
    /// Validate schema version and deterministic policy constraints.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when schema versions mismatch or
    /// execution limits violate deterministic admission rules.
    pub fn validate(&self) -> Result<(), SoraCloudManifestError> {
        if self.schema_version != FHE_EXECUTION_POLICY_VERSION_V1 {
            return Err(SoraCloudManifestError::UnsupportedVersion {
                manifest: "fhe execution policy",
                expected: FHE_EXECUTION_POLICY_VERSION_V1,
                found: self.schema_version,
            });
        }

        if self.max_plaintext_bytes > self.max_ciphertext_bytes {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "max_plaintext_bytes",
                reason: "cannot exceed max_ciphertext_bytes".to_string(),
            });
        }

        if self.max_output_ciphertexts > self.max_input_ciphertexts {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "max_output_ciphertexts",
                reason: "cannot exceed max_input_ciphertexts".to_string(),
            });
        }

        Ok(())
    }

    /// Validate this policy against an admitted FHE parameter set.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when parameter identifiers do not
    /// match, policy depth exceeds the parameter budget, or the parameter set
    /// lifecycle is not admissible for new job execution.
    pub fn validate_for_param_set(
        &self,
        param_set: &FheParamSetV1,
    ) -> Result<(), SoraCloudManifestError> {
        self.validate()?;
        param_set.validate()?;

        if self.param_set != param_set.param_set {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "param_set",
                reason: format!(
                    "policy references `{}` but parameter set is `{}`",
                    self.param_set, param_set.param_set
                ),
            });
        }

        if self.param_set_version != param_set.version {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "param_set_version",
                reason: format!(
                    "policy references version {} but parameter set is version {}",
                    self.param_set_version, param_set.version
                ),
            });
        }

        if self.max_multiplication_depth > param_set.max_multiplicative_depth {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "max_multiplication_depth",
                reason: format!(
                    "cannot exceed parameter-set maximum ({})",
                    param_set.max_multiplicative_depth
                ),
            });
        }

        match param_set.lifecycle {
            FheParamLifecycleV1::Proposed => Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "param_set.lifecycle",
                reason: "parameter set is not active yet".to_string(),
            }),
            FheParamLifecycleV1::Active | FheParamLifecycleV1::Deprecated => Ok(()),
            FheParamLifecycleV1::Withdrawn => Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "param_set.lifecycle",
                reason: "parameter set is withdrawn".to_string(),
            }),
        }
    }
}

/// Governance admission bundle coupling an FHE parameter set and execution policy.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FheGovernanceBundleV1 {
    /// Schema version; must equal [`FHE_GOVERNANCE_BUNDLE_VERSION_V1`].
    pub schema_version: u16,
    /// Governance-authored parameter set descriptor.
    pub param_set: FheParamSetV1,
    /// Deterministic execution policy bound to the parameter set.
    pub execution_policy: FheExecutionPolicyV1,
}

impl FheGovernanceBundleV1 {
    /// Validate deterministic admission constraints across FHE governance records.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when schema versions mismatch or
    /// policy/parameter references are inconsistent.
    pub fn validate_for_admission(&self) -> Result<(), SoraCloudManifestError> {
        if self.schema_version != FHE_GOVERNANCE_BUNDLE_VERSION_V1 {
            return Err(SoraCloudManifestError::UnsupportedVersion {
                manifest: "fhe governance bundle",
                expected: FHE_GOVERNANCE_BUNDLE_VERSION_V1,
                found: self.schema_version,
            });
        }
        self.execution_policy
            .validate_for_param_set(&self.param_set)
    }
}

/// Encryption class for an opaque secret envelope payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "encryption", content = "value"))]
pub enum SecretEnvelopeEncryptionV1 {
    /// Payload is client-encrypted and opaque to validators.
    ClientCiphertext,
    /// Payload is FHE ciphertext and may be operated on homomorphically.
    FheCiphertext,
}

/// Opaque encrypted payload with commitment used by ciphertext-native state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SecretEnvelopeV1 {
    /// Schema version; must equal [`SECRET_ENVELOPE_VERSION_V1`].
    pub schema_version: u16,
    /// Encryption class used by the payload.
    pub encryption: SecretEnvelopeEncryptionV1,
    /// Key material identifier (KMS alias / threshold key id / FHE key tag).
    pub key_id: String,
    /// Key version under the same `key_id`.
    pub key_version: NonZeroU32,
    /// Deterministic nonce/IV bytes supplied by the producer.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub nonce: Vec<u8>,
    /// Opaque encrypted payload bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub ciphertext: Vec<u8>,
    /// Commitment hash for verifying payload integrity against metadata.
    pub commitment: Hash,
    /// Optional digest over associated public metadata.
    #[norito(default)]
    pub aad_digest: Option<Hash>,
}

impl SecretEnvelopeV1 {
    const MAX_NONCE_BYTES: usize = 256;
    const MAX_CIPHERTEXT_BYTES: usize = 33_554_432;

    /// Validate schema version and ciphertext envelope constraints.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when schema versions mismatch or
    /// encrypted payload fields violate deterministic bounds.
    pub fn validate(&self) -> Result<(), SoraCloudManifestError> {
        if self.schema_version != SECRET_ENVELOPE_VERSION_V1 {
            return Err(SoraCloudManifestError::UnsupportedVersion {
                manifest: "secret envelope",
                expected: SECRET_ENVELOPE_VERSION_V1,
                found: self.schema_version,
            });
        }

        if self.key_id.trim().is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "secret envelope",
                field: "key_id",
            });
        }

        if self.nonce.is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "secret envelope",
                field: "nonce",
            });
        }
        if self.nonce.len() > Self::MAX_NONCE_BYTES {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "secret envelope",
                field: "nonce",
                reason: format!(
                    "length {} exceeds max {} bytes",
                    self.nonce.len(),
                    Self::MAX_NONCE_BYTES
                ),
            });
        }

        if self.ciphertext.is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "secret envelope",
                field: "ciphertext",
            });
        }
        if self.ciphertext.len() > Self::MAX_CIPHERTEXT_BYTES {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "secret envelope",
                field: "ciphertext",
                reason: format!(
                    "length {} exceeds max {} bytes",
                    self.ciphertext.len(),
                    Self::MAX_CIPHERTEXT_BYTES
                ),
            });
        }

        Ok(())
    }
}

/// Public metadata attached to ciphertext-native state records.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CiphertextStateMetadataV1 {
    /// MIME-style content hint for encrypted payload decoding.
    pub content_type: String,
    /// Ciphertext payload size in bytes.
    pub payload_bytes: NonZeroU64,
    /// Commitment hash mirrored from the secret envelope.
    pub commitment: Hash,
    /// Optional governance policy tag for access/disclosure controls.
    #[norito(default)]
    pub policy_tag: Option<String>,
    /// Optional deterministic labels for index/query routing.
    #[norito(default)]
    pub tags: Vec<String>,
}

impl CiphertextStateMetadataV1 {
    /// Validate metadata fields for deterministic ciphertext state indexing.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when metadata fields are empty or
    /// include duplicate tag entries.
    pub fn validate(&self) -> Result<(), SoraCloudManifestError> {
        if self.content_type.trim().is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "ciphertext state metadata",
                field: "content_type",
            });
        }

        if let Some(policy_tag) = self.policy_tag.as_ref()
            && policy_tag.trim().is_empty()
        {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "ciphertext state metadata",
                field: "policy_tag",
                reason: "must not be empty when provided".to_string(),
            });
        }

        let mut seen = BTreeSet::new();
        for tag in &self.tags {
            let normalized = tag.trim();
            if normalized.is_empty() {
                return Err(SoraCloudManifestError::InvalidField {
                    manifest: "ciphertext state metadata",
                    field: "tags",
                    reason: "tag entries must be non-empty".to_string(),
                });
            }
            if !seen.insert(normalized.to_owned()) {
                return Err(SoraCloudManifestError::InvalidField {
                    manifest: "ciphertext state metadata",
                    field: "tags",
                    reason: format!("duplicate tag `{normalized}`"),
                });
            }
        }

        Ok(())
    }
}

/// Ciphertext-native key-value record with public metadata and secret payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CiphertextStateRecordV1 {
    /// Schema version; must equal [`CIPHERTEXT_STATE_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Binding that governs this encrypted state key.
    pub binding_name: Name,
    /// Canonical key path scoped under a state binding prefix.
    pub state_key: String,
    /// Publicly visible metadata used for indexing, policy, and audit.
    pub metadata: CiphertextStateMetadataV1,
    /// Encrypted payload envelope.
    pub secret: SecretEnvelopeV1,
}

impl CiphertextStateRecordV1 {
    /// Validate schema version and metadata/secret consistency constraints.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when schema versions mismatch,
    /// key paths are invalid, or metadata does not match secret payload state.
    pub fn validate(&self) -> Result<(), SoraCloudManifestError> {
        if self.schema_version != CIPHERTEXT_STATE_RECORD_VERSION_V1 {
            return Err(SoraCloudManifestError::UnsupportedVersion {
                manifest: "ciphertext state record",
                expected: CIPHERTEXT_STATE_RECORD_VERSION_V1,
                found: self.schema_version,
            });
        }

        if self.state_key.trim().is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "ciphertext state record",
                field: "state_key",
            });
        }
        if !self.state_key.starts_with('/') {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "ciphertext state record",
                field: "state_key",
                reason: "must start with '/'".to_string(),
            });
        }

        self.metadata.validate()?;
        self.secret.validate()?;

        let ciphertext_len = u64::try_from(self.secret.ciphertext.len())
            .expect("ciphertext length always fits in u64");
        if self.metadata.payload_bytes.get() != ciphertext_len {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "ciphertext state record",
                field: "metadata.payload_bytes",
                reason: format!(
                    "metadata declares {} bytes but envelope has {} bytes",
                    self.metadata.payload_bytes, ciphertext_len
                ),
            });
        }

        if self.metadata.commitment != self.secret.commitment {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "ciphertext state record",
                field: "metadata.commitment",
                reason: "must match secret.commitment".to_string(),
            });
        }

        Ok(())
    }
}

/// Deterministic FHE operation class admitted for Soracloud ciphertext jobs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "operation", content = "value"))]
pub enum FheJobOperationV1 {
    /// Element-wise homomorphic addition over two or more inputs.
    Add,
    /// Element-wise homomorphic multiplication over two or more inputs.
    Multiply,
    /// Deterministic left-rotation over one ciphertext input.
    RotateLeft,
    /// Deterministic bootstrap/relinearization refresh over one input.
    Bootstrap,
}

/// Input ciphertext reference for deterministic FHE job admission.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FheJobInputRefV1 {
    /// Canonical state key of the ciphertext input.
    pub state_key: String,
    /// Input payload size in bytes.
    pub payload_bytes: NonZeroU64,
    /// Input commitment hash bound to the ciphertext payload.
    pub commitment: Hash,
}

impl FheJobInputRefV1 {
    /// Validate deterministic input reference constraints.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when state keys are empty or outside
    /// canonical path formatting rules.
    pub fn validate(&self) -> Result<(), SoraCloudManifestError> {
        if self.state_key.trim().is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "fhe job spec",
                field: "inputs.state_key",
            });
        }
        if !self.state_key.starts_with('/') {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "inputs.state_key",
                reason: "must start with '/'".to_string(),
            });
        }
        Ok(())
    }
}

/// Deterministic FHE admission/execution job descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FheJobSpecV1 {
    /// Schema version; must equal [`FHE_JOB_SPEC_VERSION_V1`].
    pub schema_version: u16,
    /// Stable deterministic job identifier.
    pub job_id: String,
    /// Referenced deterministic execution policy identifier.
    pub policy_name: Name,
    /// Referenced parameter-set identifier.
    pub param_set: Name,
    /// Referenced parameter-set version.
    pub param_set_version: NonZeroU32,
    /// Homomorphic operation class.
    pub operation: FheJobOperationV1,
    /// Ordered ciphertext inputs for deterministic replay.
    #[norito(default)]
    pub inputs: Vec<FheJobInputRefV1>,
    /// Output ciphertext state key.
    pub output_state_key: String,
    /// Requested multiplicative depth consumed by this job.
    pub requested_multiplication_depth: u16,
    /// Number of deterministic ciphertext rotations requested.
    pub rotation_count: u32,
    /// Number of bootstrap/refresh operations requested.
    pub bootstrap_count: u16,
}

impl FheJobSpecV1 {
    /// Validate schema version and deterministic FHE job constraints.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when job identifiers, key paths,
    /// inputs, or operation-specific constraints are invalid.
    pub fn validate(&self) -> Result<(), SoraCloudManifestError> {
        if self.schema_version != FHE_JOB_SPEC_VERSION_V1 {
            return Err(SoraCloudManifestError::UnsupportedVersion {
                manifest: "fhe job spec",
                expected: FHE_JOB_SPEC_VERSION_V1,
                found: self.schema_version,
            });
        }

        if self.job_id.trim().is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "fhe job spec",
                field: "job_id",
            });
        }

        if self.output_state_key.trim().is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "fhe job spec",
                field: "output_state_key",
            });
        }
        if !self.output_state_key.starts_with('/') {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "output_state_key",
                reason: "must start with '/'".to_string(),
            });
        }

        if self.inputs.is_empty() {
            return Err(SoraCloudManifestError::EmptyField {
                manifest: "fhe job spec",
                field: "inputs",
            });
        }

        let mut seen_inputs = BTreeSet::new();
        for input in &self.inputs {
            input.validate()?;
            if !seen_inputs.insert(input.state_key.clone()) {
                return Err(SoraCloudManifestError::InvalidField {
                    manifest: "fhe job spec",
                    field: "inputs.state_key",
                    reason: format!("duplicate input key `{}`", input.state_key),
                });
            }
        }

        match self.operation {
            FheJobOperationV1::Add => {
                if self.inputs.len() < 2 {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "inputs",
                        reason: "add operation requires at least two inputs".to_string(),
                    });
                }
                if self.requested_multiplication_depth != 0 {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "requested_multiplication_depth",
                        reason: "add operation must use depth 0".to_string(),
                    });
                }
                if self.rotation_count != 0 || self.bootstrap_count != 0 {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "operation",
                        reason: "add operation cannot request rotation/bootstrap".to_string(),
                    });
                }
            }
            FheJobOperationV1::Multiply => {
                if self.inputs.len() < 2 {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "inputs",
                        reason: "multiply operation requires at least two inputs".to_string(),
                    });
                }
                if self.requested_multiplication_depth == 0 {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "requested_multiplication_depth",
                        reason: "multiply operation requires non-zero depth".to_string(),
                    });
                }
                if self.rotation_count != 0 || self.bootstrap_count != 0 {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "operation",
                        reason: "multiply operation cannot request rotation/bootstrap".to_string(),
                    });
                }
            }
            FheJobOperationV1::RotateLeft => {
                if self.inputs.len() != 1 {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "inputs",
                        reason: "rotate operation requires exactly one input".to_string(),
                    });
                }
                if self.rotation_count == 0 {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "rotation_count",
                        reason: "rotate operation requires non-zero rotation_count".to_string(),
                    });
                }
                if self.requested_multiplication_depth != 0 || self.bootstrap_count != 0 {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "operation",
                        reason: "rotate operation cannot request depth/bootstrap".to_string(),
                    });
                }
            }
            FheJobOperationV1::Bootstrap => {
                if self.inputs.len() != 1 {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "inputs",
                        reason: "bootstrap operation requires exactly one input".to_string(),
                    });
                }
                if self.bootstrap_count == 0 {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "bootstrap_count",
                        reason: "bootstrap operation requires non-zero bootstrap_count".to_string(),
                    });
                }
                if self.requested_multiplication_depth != 0 || self.rotation_count != 0 {
                    return Err(SoraCloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "operation",
                        reason: "bootstrap operation cannot request depth/rotation".to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Validate job admission against deterministic policy + parameter constraints.
    ///
    /// # Errors
    /// Returns [`SoraCloudManifestError`] when policy linkage mismatches, input
    /// bounds exceed policy limits, or deterministic output bounds are violated.
    pub fn validate_for_execution(
        &self,
        policy: &FheExecutionPolicyV1,
        param_set: &FheParamSetV1,
    ) -> Result<(), SoraCloudManifestError> {
        self.validate()?;
        policy.validate_for_param_set(param_set)?;

        if self.policy_name != policy.policy_name {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "policy_name",
                reason: format!(
                    "job references `{}` but policy is `{}`",
                    self.policy_name, policy.policy_name
                ),
            });
        }
        if self.param_set != policy.param_set {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "param_set",
                reason: format!(
                    "job references `{}` but policy is `{}`",
                    self.param_set, policy.param_set
                ),
            });
        }
        if self.param_set_version != policy.param_set_version {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "param_set_version",
                reason: format!(
                    "job references version {} but policy is version {}",
                    self.param_set_version, policy.param_set_version
                ),
            });
        }

        let input_count =
            u16::try_from(self.inputs.len()).map_err(|_| SoraCloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "inputs",
                reason: "input count exceeds supported u16 range".to_string(),
            })?;
        if input_count > policy.max_input_ciphertexts.get() {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "inputs",
                reason: format!(
                    "input count {} exceeds policy max_input_ciphertexts {}",
                    input_count, policy.max_input_ciphertexts
                ),
            });
        }

        if self.requested_multiplication_depth > policy.max_multiplication_depth.get() {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "requested_multiplication_depth",
                reason: format!(
                    "requested depth {} exceeds policy max_multiplication_depth {}",
                    self.requested_multiplication_depth, policy.max_multiplication_depth
                ),
            });
        }
        if self.rotation_count > policy.max_rotation_count.get() {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "rotation_count",
                reason: format!(
                    "rotation_count {} exceeds policy max_rotation_count {}",
                    self.rotation_count, policy.max_rotation_count
                ),
            });
        }
        if self.bootstrap_count > policy.max_bootstrap_count {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "bootstrap_count",
                reason: format!(
                    "bootstrap_count {} exceeds policy max_bootstrap_count {}",
                    self.bootstrap_count, policy.max_bootstrap_count
                ),
            });
        }

        for input in &self.inputs {
            if input.payload_bytes > policy.max_ciphertext_bytes {
                return Err(SoraCloudManifestError::InvalidField {
                    manifest: "fhe job spec",
                    field: "inputs.payload_bytes",
                    reason: format!(
                        "input payload {} exceeds policy max_ciphertext_bytes {}",
                        input.payload_bytes, policy.max_ciphertext_bytes
                    ),
                });
            }
        }

        let output_bytes = self.deterministic_output_payload_bytes();
        if output_bytes > policy.max_ciphertext_bytes.get() {
            return Err(SoraCloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "output_state_key",
                reason: format!(
                    "deterministic output size {} exceeds policy max_ciphertext_bytes {}",
                    output_bytes, policy.max_ciphertext_bytes
                ),
            });
        }

        Ok(())
    }

    /// Deterministic projected output payload size in bytes for admission checks.
    #[must_use]
    pub fn deterministic_output_payload_bytes(&self) -> u64 {
        let max_input = self
            .inputs
            .iter()
            .map(|input| input.payload_bytes.get())
            .max()
            .unwrap_or(0);
        let op_overhead = match self.operation {
            FheJobOperationV1::Add => 16,
            FheJobOperationV1::Multiply => {
                u64::from(self.requested_multiplication_depth).saturating_mul(64)
            }
            FheJobOperationV1::RotateLeft => u64::from(self.rotation_count).min(1_024),
            FheJobOperationV1::Bootstrap => u64::from(self.bootstrap_count).saturating_mul(128),
        };
        max_input.saturating_add(op_overhead).max(1)
    }

    /// Deterministic output commitment derived from operation + input commitments.
    #[must_use]
    pub fn deterministic_output_commitment(&self) -> Hash {
        let input_commitments = self
            .inputs
            .iter()
            .map(|input| input.commitment)
            .collect::<Vec<_>>();
        Hash::new(Encode::encode(&(
            self.job_id.clone(),
            self.policy_name.clone(),
            self.param_set.clone(),
            self.param_set_version,
            self.operation,
            self.requested_multiplication_depth,
            self.rotation_count,
            self.bootstrap_count,
            input_commitments,
        )))
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

/// Re-export commonly used `SoraCloud` schema types.
pub mod prelude {
    pub use super::{
        AGENT_APARTMENT_MANIFEST_VERSION_V1, AgentApartmentManifestV1, AgentSpendLimitV1,
        AgentToolCapabilityV1, AgentUpgradePolicyV1, CIPHERTEXT_STATE_RECORD_VERSION_V1,
        CiphertextStateMetadataV1, CiphertextStateRecordV1, FHE_EXECUTION_POLICY_VERSION_V1,
        FHE_GOVERNANCE_BUNDLE_VERSION_V1, FHE_JOB_SPEC_VERSION_V1, FHE_PARAM_SET_VERSION_V1,
        FheDeterministicRoundingModeV1, FheExecutionPolicyV1, FheGovernanceBundleV1,
        FheJobInputRefV1, FheJobOperationV1, FheJobSpecV1, FheParamLifecycleV1, FheParamSetV1,
        FheSchemeV1, SECRET_ENVELOPE_VERSION_V1, SORA_CONTAINER_MANIFEST_VERSION_V1,
        SORA_DEPLOYMENT_BUNDLE_VERSION_V1, SORA_SERVICE_MANIFEST_VERSION_V1,
        SORA_STATE_BINDING_VERSION_V1, SecretEnvelopeEncryptionV1, SecretEnvelopeV1,
        SoraCapabilityPolicyV1, SoraCloudManifestError, SoraContainerManifestRefV1,
        SoraContainerManifestV1, SoraContainerRuntimeV1, SoraDeploymentBundleV1,
        SoraLifecycleHooksV1, SoraNetworkPolicyV1, SoraResourceLimitsV1, SoraRolloutPolicyV1,
        SoraRouteTargetV1, SoraRouteVisibilityV1, SoraServiceManifestV1, SoraStateBindingV1,
        SoraStateEncryptionV1, SoraStateMutabilityV1, SoraStateScopeV1, SoraTlsModeV1,
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

    fn sample_agent_apartment_manifest() -> AgentApartmentManifestV1 {
        AgentApartmentManifestV1 {
            schema_version: AGENT_APARTMENT_MANIFEST_VERSION_V1,
            apartment_name: "ops_agent".parse().expect("valid name"),
            container: SoraContainerManifestRefV1 {
                manifest_hash: sample_hash(41),
                expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
            },
            tool_capabilities: vec![
                AgentToolCapabilityV1 {
                    tool: "soracloud.deploy".to_string(),
                    max_invocations_per_epoch: NonZeroU32::new(128).expect("nonzero"),
                    allow_network: true,
                    allow_filesystem_write: false,
                },
                AgentToolCapabilityV1 {
                    tool: "wallet.transfer".to_string(),
                    max_invocations_per_epoch: NonZeroU32::new(32).expect("nonzero"),
                    allow_network: false,
                    allow_filesystem_write: false,
                },
            ],
            policy_capabilities: vec![
                "wallet.sign".parse().expect("valid name"),
                "governance.audit".parse().expect("valid name"),
            ],
            spend_limits: vec![AgentSpendLimitV1 {
                asset_definition: "xor#sora".to_string(),
                max_per_tx_nanos: NonZeroU64::new(5_000_000).expect("nonzero"),
                max_per_day_nanos: NonZeroU64::new(20_000_000).expect("nonzero"),
            }],
            state_quota_bytes: NonZeroU64::new(134_217_728).expect("nonzero"),
            network_egress: SoraNetworkPolicyV1::Allowlist(vec![
                "rpc.sora.internal".to_string(),
                "torii.sora.internal".to_string(),
            ]),
            upgrade_policy: AgentUpgradePolicyV1::Governed,
        }
    }

    fn sample_fhe_param_set() -> FheParamSetV1 {
        FheParamSetV1 {
            schema_version: FHE_PARAM_SET_VERSION_V1,
            param_set: "fhe_bfv_med".parse().expect("valid name"),
            version: NonZeroU32::new(2).expect("nonzero"),
            backend: "fhe/bfv-rns/v1".to_string(),
            scheme: FheSchemeV1::Bfv,
            ciphertext_modulus_bits: vec![
                NonZeroU16::new(60).expect("nonzero"),
                NonZeroU16::new(50).expect("nonzero"),
                NonZeroU16::new(40).expect("nonzero"),
            ],
            plaintext_modulus_bits: NonZeroU16::new(20).expect("nonzero"),
            polynomial_modulus_degree: NonZeroU32::new(8_192).expect("nonzero"),
            slot_count: NonZeroU32::new(4_096).expect("nonzero"),
            security_level_bits: NonZeroU16::new(128).expect("nonzero"),
            max_multiplicative_depth: NonZeroU16::new(2).expect("nonzero"),
            lifecycle: FheParamLifecycleV1::Active,
            activation_height: Some(10_000),
            deprecation_height: Some(20_000),
            withdraw_height: Some(40_000),
            parameter_digest: sample_hash(77),
        }
    }

    fn sample_fhe_execution_policy() -> FheExecutionPolicyV1 {
        FheExecutionPolicyV1 {
            schema_version: FHE_EXECUTION_POLICY_VERSION_V1,
            policy_name: "fhe_policy_med".parse().expect("valid name"),
            param_set: "fhe_bfv_med".parse().expect("valid name"),
            param_set_version: NonZeroU32::new(2).expect("nonzero"),
            max_ciphertext_bytes: NonZeroU64::new(131_072).expect("nonzero"),
            max_plaintext_bytes: NonZeroU64::new(16_384).expect("nonzero"),
            max_input_ciphertexts: NonZeroU16::new(8).expect("nonzero"),
            max_output_ciphertexts: NonZeroU16::new(4).expect("nonzero"),
            max_multiplication_depth: NonZeroU16::new(2).expect("nonzero"),
            max_rotation_count: NonZeroU32::new(128).expect("nonzero"),
            max_bootstrap_count: 1,
            rounding_mode: FheDeterministicRoundingModeV1::NearestTiesToEven,
        }
    }

    fn sample_fhe_job_spec() -> FheJobSpecV1 {
        FheJobSpecV1 {
            schema_version: FHE_JOB_SPEC_VERSION_V1,
            job_id: "job-add-001".to_string(),
            policy_name: "fhe_policy_med".parse().expect("valid name"),
            param_set: "fhe_bfv_med".parse().expect("valid name"),
            param_set_version: NonZeroU32::new(2).expect("nonzero"),
            operation: FheJobOperationV1::Add,
            inputs: vec![
                FheJobInputRefV1 {
                    state_key: "/state/health/patient-1".to_string(),
                    payload_bytes: NonZeroU64::new(2_048).expect("nonzero"),
                    commitment: sample_hash(121),
                },
                FheJobInputRefV1 {
                    state_key: "/state/health/patient-2".to_string(),
                    payload_bytes: NonZeroU64::new(2_048).expect("nonzero"),
                    commitment: sample_hash(122),
                },
            ],
            output_state_key: "/state/health/result-1".to_string(),
            requested_multiplication_depth: 0,
            rotation_count: 0,
            bootstrap_count: 0,
        }
    }

    fn sample_secret_envelope() -> SecretEnvelopeV1 {
        SecretEnvelopeV1 {
            schema_version: SECRET_ENVELOPE_VERSION_V1,
            encryption: SecretEnvelopeEncryptionV1::FheCiphertext,
            key_id: "kms/fhe/team-a".to_string(),
            key_version: NonZeroU32::new(3).expect("nonzero"),
            nonce: vec![1, 2, 3, 4, 5, 6, 7, 8],
            ciphertext: vec![11, 12, 13, 14, 15, 16, 17, 18, 19, 20],
            commitment: sample_hash(91),
            aad_digest: Some(sample_hash(99)),
        }
    }

    fn sample_ciphertext_state_record() -> CiphertextStateRecordV1 {
        let secret = sample_secret_envelope();
        let payload_bytes = u64::try_from(secret.ciphertext.len()).expect("fits in u64");
        CiphertextStateRecordV1 {
            schema_version: CIPHERTEXT_STATE_RECORD_VERSION_V1,
            binding_name: "private_state".parse().expect("valid name"),
            state_key: "/state/private/patient-1".to_string(),
            metadata: CiphertextStateMetadataV1 {
                content_type: "application/vnd.sora.secret+norito".to_string(),
                payload_bytes: NonZeroU64::new(payload_bytes).expect("nonzero"),
                commitment: secret.commitment,
                policy_tag: Some("health.phi.minimum".to_string()),
                tags: vec!["phi".to_string(), "tenant:alpha".to_string()],
            },
            secret,
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

    #[test]
    fn agent_apartment_manifest_validate_rejects_duplicate_tool_capabilities() {
        let mut manifest = sample_agent_apartment_manifest();
        manifest.tool_capabilities.push(AgentToolCapabilityV1 {
            tool: "soracloud.deploy".to_string(),
            max_invocations_per_epoch: NonZeroU32::new(1).expect("nonzero"),
            allow_network: false,
            allow_filesystem_write: false,
        });
        let error = manifest
            .validate()
            .expect_err("duplicate tool capabilities must be rejected");
        assert!(matches!(
            error,
            SoraCloudManifestError::DuplicateToolCapability { .. }
        ));
    }

    #[test]
    fn agent_apartment_manifest_validate_rejects_excessive_per_tx_limit() {
        let mut manifest = sample_agent_apartment_manifest();
        manifest.spend_limits[0].max_per_tx_nanos = NonZeroU64::new(50_000_000).expect("nonzero");
        let error = manifest
            .validate()
            .expect_err("per-tx spend limit above daily limit must fail");
        assert!(matches!(
            error,
            SoraCloudManifestError::InvalidField {
                field: "spend_limits.max_per_tx_nanos",
                ..
            }
        ));
    }

    #[test]
    fn agent_apartment_manifest_validate_accepts_consistent_policy() {
        let manifest = sample_agent_apartment_manifest();
        assert!(
            manifest.validate().is_ok(),
            "valid agent apartment manifest must pass"
        );
    }

    #[test]
    fn fhe_param_set_validate_rejects_empty_modulus_chain() {
        let mut param_set = sample_fhe_param_set();
        param_set.ciphertext_modulus_bits.clear();
        let error = param_set
            .validate()
            .expect_err("empty modulus chain must be rejected");
        assert!(matches!(
            error,
            SoraCloudManifestError::EmptyField {
                field: "ciphertext_modulus_bits",
                ..
            }
        ));
    }

    #[test]
    fn fhe_param_set_validate_rejects_invalid_lifecycle_order() {
        let mut param_set = sample_fhe_param_set();
        param_set.deprecation_height = Some(8_000);
        let error = param_set
            .validate()
            .expect_err("deprecation height before activation must be rejected");
        assert!(matches!(
            error,
            SoraCloudManifestError::InvalidField {
                field: "deprecation_height",
                ..
            }
        ));
    }

    #[test]
    fn fhe_execution_policy_validate_rejects_output_overflow() {
        let mut policy = sample_fhe_execution_policy();
        policy.max_output_ciphertexts = NonZeroU16::new(16).expect("nonzero");
        let error = policy
            .validate()
            .expect_err("output ciphertext count above input count must fail");
        assert!(matches!(
            error,
            SoraCloudManifestError::InvalidField {
                field: "max_output_ciphertexts",
                ..
            }
        ));
    }

    #[test]
    fn fhe_execution_policy_validate_for_param_set_rejects_withdrawn_param_set() {
        let mut param_set = sample_fhe_param_set();
        param_set.lifecycle = FheParamLifecycleV1::Withdrawn;
        let policy = sample_fhe_execution_policy();
        let error = policy
            .validate_for_param_set(&param_set)
            .expect_err("withdrawn parameter sets must reject new execution policy admission");
        assert!(matches!(
            error,
            SoraCloudManifestError::InvalidField {
                field: "param_set.lifecycle",
                ..
            }
        ));
    }

    #[test]
    fn fhe_governance_bundle_validate_accepts_consistent_payload() {
        let bundle = FheGovernanceBundleV1 {
            schema_version: FHE_GOVERNANCE_BUNDLE_VERSION_V1,
            param_set: sample_fhe_param_set(),
            execution_policy: sample_fhe_execution_policy(),
        };
        assert!(
            bundle.validate_for_admission().is_ok(),
            "consistent FHE governance bundle must pass validation"
        );
    }

    #[test]
    fn fhe_job_spec_validate_rejects_duplicate_input_keys() {
        let mut job = sample_fhe_job_spec();
        job.inputs.push(FheJobInputRefV1 {
            state_key: "/state/health/patient-1".to_string(),
            payload_bytes: NonZeroU64::new(64).expect("nonzero"),
            commitment: sample_hash(123),
        });
        let error = job
            .validate()
            .expect_err("duplicate input state keys must be rejected");
        assert!(matches!(
            error,
            SoraCloudManifestError::InvalidField {
                field: "inputs.state_key",
                ..
            }
        ));
    }

    #[test]
    fn fhe_job_spec_validate_for_execution_rejects_policy_mismatch() {
        let mut job = sample_fhe_job_spec();
        job.policy_name = "fhe_policy_other".parse().expect("valid name");
        let error = job
            .validate_for_execution(&sample_fhe_execution_policy(), &sample_fhe_param_set())
            .expect_err("job with policy mismatch must fail execution admission");
        assert!(matches!(
            error,
            SoraCloudManifestError::InvalidField {
                field: "policy_name",
                ..
            }
        ));
    }

    #[test]
    fn fhe_job_spec_validate_for_execution_accepts_consistent_job() {
        let job = sample_fhe_job_spec();
        let policy = sample_fhe_execution_policy();
        let param_set = sample_fhe_param_set();
        assert!(
            job.validate_for_execution(&policy, &param_set).is_ok(),
            "consistent FHE job should pass execution admission checks"
        );
        assert!(
            job.deterministic_output_payload_bytes() > 0,
            "deterministic output size must be non-zero"
        );
    }

    #[test]
    fn secret_envelope_validate_rejects_empty_ciphertext() {
        let mut envelope = sample_secret_envelope();
        envelope.ciphertext.clear();
        let error = envelope
            .validate()
            .expect_err("secret envelope without ciphertext must fail");
        assert!(matches!(
            error,
            SoraCloudManifestError::EmptyField {
                field: "ciphertext",
                ..
            }
        ));
    }

    #[test]
    fn ciphertext_state_record_validate_rejects_payload_size_mismatch() {
        let mut record = sample_ciphertext_state_record();
        record.metadata.payload_bytes = NonZeroU64::new(1).expect("nonzero");
        let error = record
            .validate()
            .expect_err("payload size mismatch must be rejected");
        assert!(matches!(
            error,
            SoraCloudManifestError::InvalidField {
                field: "metadata.payload_bytes",
                ..
            }
        ));
    }

    #[test]
    fn ciphertext_state_record_validate_accepts_consistent_record() {
        let record = sample_ciphertext_state_record();
        assert!(
            record.validate().is_ok(),
            "consistent ciphertext state record must validate"
        );
    }
}
