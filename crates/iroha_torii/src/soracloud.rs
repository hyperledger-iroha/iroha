//! App-facing Soracloud control-plane shim.
//!
//! This module provides a deterministic in-memory control-plane surface for
//! `deploy`/`upgrade`/`rollback` workflows while SCR host integration is in
//! progress. Requests must carry signed payloads so admission can verify
//! manifest provenance before mutating registry state.

use std::collections::{BTreeMap, BTreeSet};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use iroha_crypto::Hash;
use iroha_data_model::{
    name::Name,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        SoraDeploymentBundleV1, SoraStateBindingV1, SoraStateEncryptionV1, SoraStateMutabilityV1,
    },
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use tokio::sync::RwLock;

use crate::{JsonBody, NoritoJson, NoritoQuery, SharedAppState};

const REGISTRY_SCHEMA_VERSION: u16 = 1;
const DEFAULT_AUDIT_LIMIT: usize = 20;
const MAX_AUDIT_LIMIT: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "action", content = "value")]
pub(crate) enum SoracloudAction {
    Deploy,
    Upgrade,
    Rollback,
    StateMutation,
    Rollout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MutationMode {
    Deploy,
    Upgrade,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedBundleRequest {
    pub bundle: SoraDeploymentBundleV1,
    pub provenance: ManifestProvenance,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct RollbackPayload {
    pub service_name: String,
    #[norito(default)]
    pub target_version: Option<String>,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedRollbackRequest {
    pub payload: RollbackPayload,
    pub provenance: ManifestProvenance,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "operation", content = "value")]
pub(crate) enum StateMutationOperation {
    Upsert,
    Delete,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct StateMutationRequest {
    pub service_name: String,
    pub binding_name: String,
    pub key: String,
    pub operation: StateMutationOperation,
    #[norito(default)]
    pub value_size_bytes: Option<u64>,
    pub encryption: SoraStateEncryptionV1,
    pub governance_tx_hash: Hash,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedStateMutationRequest {
    pub payload: StateMutationRequest,
    pub provenance: ManifestProvenance,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "stage", content = "value")]
pub(crate) enum RolloutStage {
    Canary,
    Promoted,
    RolledBack,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct RolloutAdvancePayload {
    pub service_name: String,
    pub rollout_handle: String,
    pub healthy: bool,
    #[norito(default)]
    pub promote_to_percent: Option<u8>,
    pub governance_tx_hash: Hash,
}

#[derive(Clone, Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub(crate) struct SignedRolloutAdvanceRequest {
    pub payload: RolloutAdvancePayload,
    pub provenance: ManifestProvenance,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct RolloutResponse {
    pub action: SoracloudAction,
    pub service_name: String,
    pub rollout_handle: String,
    pub stage: RolloutStage,
    pub current_version: String,
    pub traffic_percent: u8,
    pub health_failures: u32,
    pub max_health_failures: u32,
    pub sequence: u64,
    pub governance_tx_hash: Hash,
    pub audit_event_count: u32,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct StateMutationResponse {
    pub action: SoracloudAction,
    pub service_name: String,
    pub binding_name: String,
    pub key: String,
    pub operation: StateMutationOperation,
    pub sequence: u64,
    pub governance_tx_hash: Hash,
    pub current_version: String,
    pub binding_total_bytes: u64,
    pub binding_key_count: u32,
    pub audit_event_count: u32,
    pub signed_by: String,
}

#[derive(Clone, Debug, Default, JsonDeserialize)]
pub(crate) struct RegistryStatusQuery {
    #[norito(default)]
    pub service_name: Option<String>,
    #[norito(default)]
    pub audit_limit: Option<u32>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct MutationResponse {
    pub action: SoracloudAction,
    pub service_name: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub previous_version: Option<String>,
    pub current_version: String,
    pub sequence: u64,
    pub service_manifest_hash: Hash,
    pub container_manifest_hash: Hash,
    pub revision_count: u32,
    pub audit_event_count: u32,
    pub signed_by: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rollout_handle: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rollout_stage: Option<RolloutStage>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rollout_percent: Option<u8>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct RegistrySnapshot {
    pub schema_version: u16,
    pub service_count: u32,
    pub audit_event_count: u32,
    #[norito(default)]
    pub services: Vec<ServiceStatusSnapshot>,
    #[norito(default)]
    pub recent_audit_events: Vec<RegistryAuditEvent>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ServiceStatusSnapshot {
    pub service_name: String,
    pub current_version: String,
    pub revision_count: u32,
    #[norito(default)]
    pub latest_revision: Option<RegistryServiceRevision>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub active_rollout: Option<RolloutRuntimeState>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_rollout: Option<RolloutRuntimeState>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct RegistryState {
    schema_version: u16,
    next_sequence: u64,
    #[norito(default)]
    services: BTreeMap<String, RegistryServiceEntry>,
    #[norito(default)]
    audit_log: Vec<RegistryAuditEvent>,
}

impl Default for RegistryState {
    fn default() -> Self {
        Self {
            schema_version: REGISTRY_SCHEMA_VERSION,
            next_sequence: 1,
            services: BTreeMap::new(),
            audit_log: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct RegistryServiceEntry {
    current_version: String,
    #[norito(default)]
    revisions: Vec<RegistryServiceRevision>,
    #[norito(default)]
    binding_states: BTreeMap<String, BindingRuntimeState>,
    #[norito(default)]
    active_rollout: Option<RolloutRuntimeState>,
    #[norito(default)]
    last_rollout: Option<RolloutRuntimeState>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct RegistryServiceRevision {
    pub sequence: u64,
    pub action: SoracloudAction,
    pub service_version: String,
    pub service_manifest_hash: Hash,
    pub container_manifest_hash: Hash,
    pub replicas: u16,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub route_host: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub route_path_prefix: Option<String>,
    pub state_binding_count: u32,
    #[norito(default)]
    pub state_bindings: Vec<SoraStateBindingV1>,
    pub signed_by: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct RegistryAuditEvent {
    pub sequence: u64,
    pub action: SoracloudAction,
    pub service_name: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub from_version: Option<String>,
    pub to_version: String,
    pub service_manifest_hash: Hash,
    pub container_manifest_hash: Hash,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub binding_name: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub state_key: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub governance_tx_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rollout_handle: Option<String>,
    pub signed_by: String,
}

#[derive(Clone, Debug, Default, JsonSerialize, JsonDeserialize)]
struct BindingRuntimeState {
    total_bytes: u64,
    #[norito(default)]
    key_sizes: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct RolloutRuntimeState {
    pub rollout_handle: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub baseline_version: Option<String>,
    pub candidate_version: String,
    pub canary_percent: u8,
    pub traffic_percent: u8,
    pub stage: RolloutStage,
    pub health_failures: u32,
    pub max_health_failures: u32,
    pub health_window_secs: u32,
    pub created_sequence: u64,
    pub updated_sequence: u64,
}

#[derive(Debug, Default)]
pub(crate) struct Registry {
    state: RwLock<RegistryState>,
}

impl Registry {
    pub(crate) async fn snapshot(
        &self,
        service_name: Option<&str>,
        audit_limit: usize,
    ) -> RegistrySnapshot {
        let state = self.state.read().await;
        let mut services = Vec::new();
        for (name, entry) in &state.services {
            if service_name.is_some_and(|filter| filter != name) {
                continue;
            }
            services.push(ServiceStatusSnapshot {
                service_name: name.clone(),
                current_version: entry.current_version.clone(),
                revision_count: u32::try_from(entry.revisions.len()).unwrap_or(u32::MAX),
                latest_revision: entry.revisions.last().cloned(),
                active_rollout: entry.active_rollout.clone(),
                last_rollout: entry.last_rollout.clone(),
            });
        }

        let limit = audit_limit.max(1).min(MAX_AUDIT_LIMIT);
        let recent_audit_events = state
            .audit_log
            .iter()
            .rev()
            .filter(|event| service_name.is_none_or(|filter| filter == event.service_name.as_str()))
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();

        RegistrySnapshot {
            schema_version: state.schema_version,
            service_count: u32::try_from(services.len()).unwrap_or(u32::MAX),
            audit_event_count: u32::try_from(state.audit_log.len()).unwrap_or(u32::MAX),
            services,
            recent_audit_events,
        }
    }

    pub(crate) async fn apply_deploy(
        &self,
        request: SignedBundleRequest,
    ) -> Result<MutationResponse, SoracloudError> {
        self.apply_bundle_mutation(MutationMode::Deploy, request)
            .await
    }

    pub(crate) async fn apply_upgrade(
        &self,
        request: SignedBundleRequest,
    ) -> Result<MutationResponse, SoracloudError> {
        self.apply_bundle_mutation(MutationMode::Upgrade, request)
            .await
    }

    pub(crate) async fn apply_rollback(
        &self,
        request: SignedRollbackRequest,
    ) -> Result<MutationResponse, SoracloudError> {
        verify_rollback_signature(&request)?;

        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        let service_name = service_name.to_string();
        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;

        let sequence = state.next_sequence;
        let signer = request.provenance.signer.to_string();
        let target_version = request.payload.target_version.clone();
        let (previous_version, target, revision_count) = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;
            let previous_version = Some(entry.current_version.clone());
            let target = if let Some(target_version) = target_version.as_deref() {
                entry
                    .revisions
                    .iter()
                    .rev()
                    .find(|revision| revision.service_version == target_version)
                    .cloned()
                    .ok_or_else(|| {
                        SoracloudError::not_found(format!(
                            "service `{service_name}` has no deployed revision for version `{target_version}`"
                        ))
                    })?
            } else {
                entry
                    .revisions
                    .iter()
                    .rev()
                    .find(|revision| revision.service_version != entry.current_version)
                    .cloned()
                    .ok_or_else(|| {
                        SoracloudError::conflict(format!(
                            "service `{service_name}` has no previous revision to roll back to"
                        ))
                    })?
            };

            let revision = RegistryServiceRevision {
                sequence,
                action: SoracloudAction::Rollback,
                service_version: target.service_version.clone(),
                service_manifest_hash: target.service_manifest_hash,
                container_manifest_hash: target.container_manifest_hash,
                replicas: target.replicas,
                route_host: target.route_host.clone(),
                route_path_prefix: target.route_path_prefix.clone(),
                state_binding_count: target.state_binding_count,
                state_bindings: target.state_bindings.clone(),
                signed_by: signer.clone(),
            };

            entry.current_version = target.service_version.clone();
            sync_binding_states(entry, &target.state_bindings);
            entry.revisions.push(revision);
            entry.active_rollout = None;
            entry.last_rollout = None;
            let revision_count = u32::try_from(entry.revisions.len()).unwrap_or(u32::MAX);
            (previous_version, target, revision_count)
        };

        state.audit_log.push(RegistryAuditEvent {
            sequence,
            action: SoracloudAction::Rollback,
            service_name: service_name.clone(),
            from_version: previous_version.clone(),
            to_version: target.service_version.clone(),
            service_manifest_hash: target.service_manifest_hash,
            container_manifest_hash: target.container_manifest_hash,
            binding_name: None,
            state_key: None,
            governance_tx_hash: None,
            rollout_handle: None,
            signed_by: signer.clone(),
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        let audit_event_count = u32::try_from(state.audit_log.len()).unwrap_or(u32::MAX);

        Ok(MutationResponse {
            action: SoracloudAction::Rollback,
            service_name,
            previous_version,
            current_version: target.service_version,
            sequence,
            service_manifest_hash: target.service_manifest_hash,
            container_manifest_hash: target.container_manifest_hash,
            revision_count,
            audit_event_count,
            signed_by: signer,
            rollout_handle: None,
            rollout_stage: None,
            rollout_percent: None,
        })
    }

    pub(crate) async fn apply_state_mutation(
        &self,
        request: SignedStateMutationRequest,
    ) -> Result<StateMutationResponse, SoracloudError> {
        verify_state_mutation_signature(&request)?;

        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        let binding_name: Name =
            request.payload.binding_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid binding_name: {err}"))
            })?;
        if request.payload.key.trim().is_empty() {
            return Err(SoracloudError::bad_request(
                "state mutation key must not be empty",
            ));
        }

        let service_name = service_name.to_string();
        let binding_name = binding_name.to_string();
        let signer = request.provenance.signer.to_string();
        let operation = request.payload.operation;
        let key = request.payload.key.clone();
        let governance_tx_hash = request.payload.governance_tx_hash;

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;

        let sequence = state.next_sequence;
        let (
            current_version,
            service_manifest_hash,
            container_manifest_hash,
            binding_total_bytes,
            binding_key_count,
        ) = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;
            let current_revision = entry.revisions.last().cloned().ok_or_else(|| {
                SoracloudError::conflict(format!("service `{service_name}` has no active revision"))
            })?;

            let binding = current_revision
                .state_bindings
                .iter()
                .find(|binding| binding.binding_name.as_ref() == binding_name.as_str())
                .cloned()
                .ok_or_else(|| {
                    SoracloudError::not_found(format!(
                        "binding `{binding_name}` is not declared for service `{service_name}`"
                    ))
                })?;

            if request.payload.encryption != binding.encryption {
                return Err(SoracloudError::conflict(format!(
                    "binding `{binding_name}` requires {:?} encryption",
                    binding.encryption
                )));
            }
            if !key.starts_with(&binding.key_prefix) {
                return Err(SoracloudError::conflict(format!(
                    "key `{key}` is outside binding prefix `{}`",
                    binding.key_prefix
                )));
            }

            let runtime_state = entry
                .binding_states
                .entry(binding_name.clone())
                .or_insert_with(BindingRuntimeState::default);
            match operation {
                StateMutationOperation::Upsert => {
                    if binding.mutability == SoraStateMutabilityV1::ReadOnly {
                        return Err(SoracloudError::conflict(format!(
                            "binding `{binding_name}` is read-only"
                        )));
                    }
                    let value_size = request.payload.value_size_bytes.ok_or_else(|| {
                        SoracloudError::bad_request(
                            "value_size_bytes is required for upsert mutations",
                        )
                    })?;
                    if value_size > binding.max_item_bytes.get() {
                        return Err(SoracloudError::conflict(format!(
                            "value_size_bytes {value_size} exceeds binding max_item_bytes {}",
                            binding.max_item_bytes
                        )));
                    }

                    let existing_size = runtime_state.key_sizes.get(&key).copied().unwrap_or(0);
                    if binding.mutability == SoraStateMutabilityV1::AppendOnly && existing_size > 0
                    {
                        return Err(SoracloudError::conflict(format!(
                            "binding `{binding_name}` is append-only; key `{key}` already exists"
                        )));
                    }
                    let tentative_total = runtime_state
                        .total_bytes
                        .saturating_sub(existing_size)
                        .saturating_add(value_size);
                    if tentative_total > binding.max_total_bytes.get() {
                        return Err(SoracloudError::conflict(format!(
                            "binding `{binding_name}` max_total_bytes {} would be exceeded",
                            binding.max_total_bytes
                        )));
                    }

                    runtime_state.total_bytes = tentative_total;
                    runtime_state.key_sizes.insert(key.clone(), value_size);
                }
                StateMutationOperation::Delete => {
                    if binding.mutability != SoraStateMutabilityV1::ReadWrite {
                        return Err(SoracloudError::conflict(format!(
                            "binding `{binding_name}` does not allow deletes"
                        )));
                    }
                    if let Some(existing_size) = runtime_state.key_sizes.remove(&key) {
                        runtime_state.total_bytes =
                            runtime_state.total_bytes.saturating_sub(existing_size);
                    }
                }
            }

            (
                entry.current_version.clone(),
                current_revision.service_manifest_hash,
                current_revision.container_manifest_hash,
                runtime_state.total_bytes,
                u32::try_from(runtime_state.key_sizes.len()).unwrap_or(u32::MAX),
            )
        };

        state.audit_log.push(RegistryAuditEvent {
            sequence,
            action: SoracloudAction::StateMutation,
            service_name: service_name.clone(),
            from_version: None,
            to_version: current_version.clone(),
            service_manifest_hash,
            container_manifest_hash,
            binding_name: Some(binding_name.clone()),
            state_key: Some(key.clone()),
            governance_tx_hash: Some(governance_tx_hash),
            rollout_handle: None,
            signed_by: signer.clone(),
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        let audit_event_count = u32::try_from(state.audit_log.len()).unwrap_or(u32::MAX);

        Ok(StateMutationResponse {
            action: SoracloudAction::StateMutation,
            service_name,
            binding_name,
            key,
            operation,
            sequence,
            governance_tx_hash,
            current_version,
            binding_total_bytes,
            binding_key_count,
            audit_event_count,
            signed_by: signer,
        })
    }

    pub(crate) async fn apply_rollout(
        &self,
        request: SignedRolloutAdvanceRequest,
    ) -> Result<RolloutResponse, SoracloudError> {
        verify_rollout_signature(&request)?;

        let service_name: Name =
            request.payload.service_name.parse().map_err(|err| {
                SoracloudError::bad_request(format!("invalid service_name: {err}"))
            })?;
        if request.payload.rollout_handle.trim().is_empty() {
            return Err(SoracloudError::bad_request(
                "rollout_handle must not be empty",
            ));
        }
        if request
            .payload
            .promote_to_percent
            .is_some_and(|value| value > 100)
        {
            return Err(SoracloudError::bad_request(
                "promote_to_percent must be within 0..=100",
            ));
        }

        let service_name = service_name.to_string();
        let signer = request.provenance.signer.to_string();
        let rollout_handle = request.payload.rollout_handle.clone();
        let governance_tx_hash = request.payload.governance_tx_hash;
        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;
        let sequence = state.next_sequence;

        let (mut response, audit_event) = {
            let entry = state.services.get_mut(&service_name).ok_or_else(|| {
                SoracloudError::not_found(format!(
                    "service `{service_name}` not found in control-plane registry"
                ))
            })?;

            let mut rollout = entry.active_rollout.clone().ok_or_else(|| {
                SoracloudError::conflict(format!(
                    "service `{service_name}` has no active rollout to advance"
                ))
            })?;
            if rollout.rollout_handle != rollout_handle {
                return Err(SoracloudError::conflict(format!(
                    "service `{service_name}` active rollout handle mismatch (expected `{}`)",
                    rollout.rollout_handle
                )));
            }

            if request.payload.healthy {
                let promote_to = request.payload.promote_to_percent.unwrap_or(100);
                if promote_to < rollout.traffic_percent {
                    return Err(SoracloudError::conflict(format!(
                        "rollout traffic cannot decrease from {} to {promote_to}",
                        rollout.traffic_percent
                    )));
                }
                if promote_to < rollout.canary_percent {
                    return Err(SoracloudError::conflict(format!(
                        "rollout traffic cannot be below canary_percent {}",
                        rollout.canary_percent
                    )));
                }
                rollout.traffic_percent = promote_to;
                rollout.stage = if promote_to >= 100 {
                    RolloutStage::Promoted
                } else {
                    RolloutStage::Canary
                };
                rollout.health_failures = 0;
                rollout.updated_sequence = sequence;

                if rollout.stage == RolloutStage::Promoted {
                    entry.active_rollout = None;
                } else {
                    entry.active_rollout = Some(rollout.clone());
                }
                entry.last_rollout = Some(rollout.clone());

                let current_version = entry.current_version.clone();
                let current_revision = entry.revisions.last().cloned().ok_or_else(|| {
                    SoracloudError::conflict(format!(
                        "service `{service_name}` has no active revision"
                    ))
                })?;
                let audit_event = RegistryAuditEvent {
                    sequence,
                    action: SoracloudAction::Rollout,
                    service_name: service_name.clone(),
                    from_version: Some(current_version.clone()),
                    to_version: current_version.clone(),
                    service_manifest_hash: current_revision.service_manifest_hash,
                    container_manifest_hash: current_revision.container_manifest_hash,
                    binding_name: None,
                    state_key: None,
                    governance_tx_hash: Some(governance_tx_hash),
                    rollout_handle: Some(rollout_handle.clone()),
                    signed_by: signer.clone(),
                };
                let response = RolloutResponse {
                    action: SoracloudAction::Rollout,
                    service_name: service_name.clone(),
                    rollout_handle: rollout_handle.clone(),
                    stage: rollout.stage,
                    current_version,
                    traffic_percent: rollout.traffic_percent,
                    health_failures: rollout.health_failures,
                    max_health_failures: rollout.max_health_failures,
                    sequence,
                    governance_tx_hash,
                    audit_event_count: 0,
                    signed_by: signer.clone(),
                };
                (response, audit_event)
            } else {
                rollout.health_failures = rollout.health_failures.saturating_add(1);
                rollout.updated_sequence = sequence;

                if rollout.health_failures >= rollout.max_health_failures {
                    let baseline_version = rollout.baseline_version.clone().ok_or_else(|| {
                        SoracloudError::conflict(format!(
                            "service `{service_name}` has no baseline version for auto rollback"
                        ))
                    })?;
                    let previous_version = entry.current_version.clone();
                    let target = entry
                        .revisions
                        .iter()
                        .rev()
                        .find(|revision| revision.service_version == baseline_version)
                        .cloned()
                        .ok_or_else(|| {
                            SoracloudError::not_found(format!(
                                "service `{service_name}` missing baseline revision `{baseline_version}`"
                            ))
                        })?;

                    let rollback_revision = RegistryServiceRevision {
                        sequence,
                        action: SoracloudAction::Rollback,
                        service_version: target.service_version.clone(),
                        service_manifest_hash: target.service_manifest_hash,
                        container_manifest_hash: target.container_manifest_hash,
                        replicas: target.replicas,
                        route_host: target.route_host.clone(),
                        route_path_prefix: target.route_path_prefix.clone(),
                        state_binding_count: target.state_binding_count,
                        state_bindings: target.state_bindings.clone(),
                        signed_by: signer.clone(),
                    };
                    entry.current_version = target.service_version.clone();
                    sync_binding_states(entry, &target.state_bindings);
                    entry.revisions.push(rollback_revision);

                    rollout.stage = RolloutStage::RolledBack;
                    rollout.traffic_percent = 0;
                    entry.active_rollout = None;
                    entry.last_rollout = Some(rollout.clone());

                    let audit_event = RegistryAuditEvent {
                        sequence,
                        action: SoracloudAction::Rollback,
                        service_name: service_name.clone(),
                        from_version: Some(previous_version),
                        to_version: target.service_version.clone(),
                        service_manifest_hash: target.service_manifest_hash,
                        container_manifest_hash: target.container_manifest_hash,
                        binding_name: None,
                        state_key: None,
                        governance_tx_hash: Some(governance_tx_hash),
                        rollout_handle: Some(rollout_handle.clone()),
                        signed_by: signer.clone(),
                    };
                    let response = RolloutResponse {
                        action: SoracloudAction::Rollback,
                        service_name: service_name.clone(),
                        rollout_handle: rollout_handle.clone(),
                        stage: rollout.stage,
                        current_version: target.service_version,
                        traffic_percent: rollout.traffic_percent,
                        health_failures: rollout.health_failures,
                        max_health_failures: rollout.max_health_failures,
                        sequence,
                        governance_tx_hash,
                        audit_event_count: 0,
                        signed_by: signer.clone(),
                    };
                    (response, audit_event)
                } else {
                    entry.active_rollout = Some(rollout.clone());
                    entry.last_rollout = Some(rollout.clone());
                    let current_version = entry.current_version.clone();
                    let current_revision = entry.revisions.last().cloned().ok_or_else(|| {
                        SoracloudError::conflict(format!(
                            "service `{service_name}` has no active revision"
                        ))
                    })?;
                    let audit_event = RegistryAuditEvent {
                        sequence,
                        action: SoracloudAction::Rollout,
                        service_name: service_name.clone(),
                        from_version: Some(current_version.clone()),
                        to_version: current_version.clone(),
                        service_manifest_hash: current_revision.service_manifest_hash,
                        container_manifest_hash: current_revision.container_manifest_hash,
                        binding_name: None,
                        state_key: None,
                        governance_tx_hash: Some(governance_tx_hash),
                        rollout_handle: Some(rollout_handle.clone()),
                        signed_by: signer.clone(),
                    };
                    let response = RolloutResponse {
                        action: SoracloudAction::Rollout,
                        service_name: service_name.clone(),
                        rollout_handle: rollout_handle.clone(),
                        stage: rollout.stage,
                        current_version,
                        traffic_percent: rollout.traffic_percent,
                        health_failures: rollout.health_failures,
                        max_health_failures: rollout.max_health_failures,
                        sequence,
                        governance_tx_hash,
                        audit_event_count: 0,
                        signed_by: signer.clone(),
                    };
                    (response, audit_event)
                }
            }
        };

        state.audit_log.push(audit_event);
        state.next_sequence = state.next_sequence.saturating_add(1);
        response.audit_event_count = u32::try_from(state.audit_log.len()).unwrap_or(u32::MAX);
        Ok(response)
    }

    async fn apply_bundle_mutation(
        &self,
        mode: MutationMode,
        request: SignedBundleRequest,
    ) -> Result<MutationResponse, SoracloudError> {
        verify_bundle_signature(&request)?;
        request.bundle.validate_for_admission().map_err(|err| {
            SoracloudError::bad_request(format!("deployment bundle failed admission checks: {err}"))
        })?;

        let mut state = self.state.write().await;
        ensure_registry_schema(&state)?;

        let service_name = request.bundle.service.service_name.to_string();
        let service_version = request.bundle.service.service_version.clone();
        let sequence = state.next_sequence;
        let signer = request.provenance.signer.to_string();
        let previous_version = state
            .services
            .get(&service_name)
            .map(|entry| entry.current_version.clone());
        let action = match (mode, previous_version.is_some()) {
            (MutationMode::Deploy, false) => SoracloudAction::Deploy,
            (MutationMode::Deploy, true) => {
                return Err(SoracloudError::conflict(format!(
                    "service `{service_name}` already deployed; use upgrade instead"
                )));
            }
            (MutationMode::Upgrade, false) => {
                return Err(SoracloudError::not_found(format!(
                    "service `{service_name}` not found; deploy it before upgrading"
                )));
            }
            (MutationMode::Upgrade, true) => SoracloudAction::Upgrade,
        };

        if let Some(existing) = state.services.get(&service_name)
            && existing.current_version == service_version
        {
            return Err(SoracloudError::conflict(format!(
                "service `{service_name}` is already at version `{service_version}`"
            )));
        }

        let container_manifest_hash = request.bundle.container_manifest_hash();
        let service_manifest_hash = request.bundle.service_manifest_hash();
        let revision = RegistryServiceRevision {
            sequence,
            action,
            service_version: service_version.clone(),
            service_manifest_hash,
            container_manifest_hash,
            replicas: request.bundle.service.replicas.get(),
            route_host: request
                .bundle
                .service
                .route
                .as_ref()
                .map(|route| route.host.clone()),
            route_path_prefix: request
                .bundle
                .service
                .route
                .as_ref()
                .map(|route| route.path_prefix.clone()),
            state_binding_count: u32::try_from(request.bundle.service.state_bindings.len())
                .unwrap_or(u32::MAX),
            state_bindings: request.bundle.service.state_bindings.clone(),
            signed_by: signer.clone(),
        };

        let mut response_rollout_handle = None;
        let mut response_rollout_stage = None;
        let mut response_rollout_percent = None;
        let revision_count = {
            let entry = state
                .services
                .entry(service_name.clone())
                .or_insert_with(|| RegistryServiceEntry {
                    current_version: service_version.clone(),
                    revisions: Vec::new(),
                    binding_states: BTreeMap::new(),
                    active_rollout: None,
                    last_rollout: None,
                });
            entry.current_version = service_version.clone();
            sync_binding_states(entry, &request.bundle.service.state_bindings);
            entry.revisions.push(revision);

            if action == SoracloudAction::Upgrade {
                let canary_percent = request.bundle.service.rollout.canary_percent.min(100);
                let traffic_percent = if canary_percent == 0 {
                    100
                } else {
                    canary_percent
                };
                let rollout_state = RolloutRuntimeState {
                    rollout_handle: rollout_handle(&service_name, sequence),
                    baseline_version: previous_version.clone(),
                    candidate_version: service_version.clone(),
                    canary_percent,
                    traffic_percent,
                    stage: if traffic_percent >= 100 {
                        RolloutStage::Promoted
                    } else {
                        RolloutStage::Canary
                    },
                    health_failures: 0,
                    max_health_failures: request
                        .bundle
                        .service
                        .rollout
                        .automatic_rollback_failures
                        .get(),
                    health_window_secs: request.bundle.service.rollout.health_window_secs.get(),
                    created_sequence: sequence,
                    updated_sequence: sequence,
                };
                response_rollout_handle = Some(rollout_state.rollout_handle.clone());
                response_rollout_stage = Some(rollout_state.stage);
                response_rollout_percent = Some(rollout_state.traffic_percent);
                if rollout_state.stage == RolloutStage::Promoted {
                    entry.active_rollout = None;
                } else {
                    entry.active_rollout = Some(rollout_state.clone());
                }
                entry.last_rollout = Some(rollout_state);
            } else {
                entry.active_rollout = None;
                entry.last_rollout = None;
            }
            u32::try_from(entry.revisions.len()).unwrap_or(u32::MAX)
        };

        state.audit_log.push(RegistryAuditEvent {
            sequence,
            action,
            service_name: service_name.clone(),
            from_version: previous_version.clone(),
            to_version: service_version.clone(),
            service_manifest_hash,
            container_manifest_hash,
            binding_name: None,
            state_key: None,
            governance_tx_hash: None,
            rollout_handle: response_rollout_handle.clone(),
            signed_by: signer.clone(),
        });
        state.next_sequence = state.next_sequence.saturating_add(1);
        let audit_event_count = u32::try_from(state.audit_log.len()).unwrap_or(u32::MAX);

        Ok(MutationResponse {
            action,
            service_name,
            previous_version,
            current_version: service_version,
            sequence,
            service_manifest_hash,
            container_manifest_hash,
            revision_count,
            audit_event_count,
            signed_by: signer,
            rollout_handle: response_rollout_handle,
            rollout_stage: response_rollout_stage,
            rollout_percent: response_rollout_percent,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SoracloudErrorKind {
    BadRequest,
    Unauthorized,
    NotFound,
    Conflict,
    Internal,
}

#[derive(Debug, JsonSerialize)]
struct SoracloudErrorBody {
    code: &'static str,
    message: String,
}

#[derive(Debug)]
pub(crate) struct SoracloudError {
    kind: SoracloudErrorKind,
    message: String,
}

impl SoracloudError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            kind: SoracloudErrorKind::BadRequest,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            kind: SoracloudErrorKind::Unauthorized,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            kind: SoracloudErrorKind::NotFound,
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            kind: SoracloudErrorKind::Conflict,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: SoracloudErrorKind::Internal,
            message: message.into(),
        }
    }

    fn code(&self) -> &'static str {
        match self.kind {
            SoracloudErrorKind::BadRequest => "bad_request",
            SoracloudErrorKind::Unauthorized => "invalid_signature",
            SoracloudErrorKind::NotFound => "not_found",
            SoracloudErrorKind::Conflict => "conflict",
            SoracloudErrorKind::Internal => "internal",
        }
    }

    fn status(&self) -> StatusCode {
        match self.kind {
            SoracloudErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            SoracloudErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            SoracloudErrorKind::NotFound => StatusCode::NOT_FOUND,
            SoracloudErrorKind::Conflict => StatusCode::CONFLICT,
            SoracloudErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for SoracloudError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = SoracloudErrorBody {
            code: self.code(),
            message: self.message,
        };
        (status, JsonBody(body)).into_response()
    }
}

fn sync_binding_states(entry: &mut RegistryServiceEntry, bindings: &[SoraStateBindingV1]) {
    let active_bindings = bindings
        .iter()
        .map(|binding| binding.binding_name.to_string())
        .collect::<BTreeSet<_>>();
    entry
        .binding_states
        .retain(|name, _| active_bindings.contains(name));
    for name in active_bindings {
        entry.binding_states.entry(name).or_default();
    }
}

fn ensure_registry_schema(state: &RegistryState) -> Result<(), SoracloudError> {
    if state.schema_version != REGISTRY_SCHEMA_VERSION {
        return Err(SoracloudError::internal(format!(
            "unsupported registry schema {} (expected {REGISTRY_SCHEMA_VERSION})",
            state.schema_version
        )));
    }
    Ok(())
}

fn rollout_handle(service_name: &str, sequence: u64) -> String {
    format!("{service_name}:rollout:{sequence}")
}

fn verify_bundle_signature(request: &SignedBundleRequest) -> Result<(), SoracloudError> {
    let payload = norito::to_bytes(&request.bundle).map_err(|err| {
        SoracloudError::internal(format!("failed to encode bundle payload: {err}"))
    })?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("bundle provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_rollback_signature(request: &SignedRollbackRequest) -> Result<(), SoracloudError> {
    let payload = norito::to_bytes(&request.payload).map_err(|err| {
        SoracloudError::internal(format!("failed to encode rollback payload: {err}"))
    })?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("rollback provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_state_mutation_signature(
    request: &SignedStateMutationRequest,
) -> Result<(), SoracloudError> {
    let payload = norito::to_bytes(&request.payload).map_err(|err| {
        SoracloudError::internal(format!("failed to encode state mutation payload: {err}"))
    })?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("state mutation provenance signature verification failed")
        })?;
    Ok(())
}

fn verify_rollout_signature(request: &SignedRolloutAdvanceRequest) -> Result<(), SoracloudError> {
    let payload = norito::to_bytes(&request.payload).map_err(|err| {
        SoracloudError::internal(format!("failed to encode rollout payload: {err}"))
    })?;
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .map_err(|_| {
            SoracloudError::unauthorized("rollout provenance signature verification failed")
        })?;
    Ok(())
}

pub(crate) async fn handle_deploy(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedBundleRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/deploy").await {
        return err.into_response();
    }

    match app.soracloud_registry.apply_deploy(request).await {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_upgrade(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedBundleRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/upgrade").await {
        return err.into_response();
    }

    match app.soracloud_registry.apply_upgrade(request).await {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_rollback(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedRollbackRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/rollback").await {
        return err.into_response();
    }

    match app.soracloud_registry.apply_rollback(request).await {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_rollout(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedRolloutAdvanceRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/rollout").await {
        return err.into_response();
    }

    match app.soracloud_registry.apply_rollout(request).await {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_state_mutation(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<SignedStateMutationRequest>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/state/mutate").await {
        return err.into_response();
    }

    match app.soracloud_registry.apply_state_mutation(request).await {
        Ok(response) => JsonBody(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub(crate) async fn handle_registry_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoQuery(query): NoritoQuery<RegistryStatusQuery>,
) -> Response {
    if let Err(err) = crate::check_access(&app, &headers, None, "v1/soracloud/registry").await {
        return err.into_response();
    }

    let audit_limit = query
        .audit_limit
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(DEFAULT_AUDIT_LIMIT)
        .max(1);
    let snapshot = app
        .soracloud_registry
        .snapshot(query.service_name.as_deref(), audit_limit)
        .await;
    JsonBody(snapshot).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use iroha_crypto::{KeyPair, Signature};
    use iroha_data_model::{
        Encode,
        soracloud::{
            SORA_DEPLOYMENT_BUNDLE_VERSION_V1, SoraContainerManifestV1, SoraServiceManifestV1,
        },
    };

    fn workspace_fixture(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(path)
    }

    fn load_json<T>(path: &Path) -> T
    where
        T: norito::json::JsonDeserialize,
    {
        let bytes = fs::read(path).expect("read fixture");
        norito::json::from_slice(&bytes).expect("decode fixture")
    }

    fn fixture_bundle(version: &str) -> SoraDeploymentBundleV1 {
        let container: SoraContainerManifestV1 = load_json(&workspace_fixture(
            "fixtures/soracloud/sora_container_manifest_v1.json",
        ));
        let mut service: SoraServiceManifestV1 = load_json(&workspace_fixture(
            "fixtures/soracloud/sora_service_manifest_v1.json",
        ));
        service.service_version = version.to_string();
        service.container.manifest_hash = Hash::new(Encode::encode(&container));
        SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        }
    }

    fn signed_bundle_request(
        bundle: SoraDeploymentBundleV1,
        key_pair: &KeyPair,
    ) -> SignedBundleRequest {
        let payload = norito::to_bytes(&bundle).expect("encode bundle");
        let signature = Signature::new(key_pair.private_key(), &payload);
        SignedBundleRequest {
            bundle,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
        }
    }

    fn signed_rollback_request(
        service_name: &str,
        target_version: Option<&str>,
        key_pair: &KeyPair,
    ) -> SignedRollbackRequest {
        let payload = RollbackPayload {
            service_name: service_name.to_string(),
            target_version: target_version.map(ToOwned::to_owned),
        };
        let encoded = norito::to_bytes(&payload).expect("encode rollback payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedRollbackRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
        }
    }

    fn signed_state_mutation_request(
        payload: StateMutationRequest,
        key_pair: &KeyPair,
    ) -> SignedStateMutationRequest {
        let encoded = norito::to_bytes(&payload).expect("encode state mutation payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedStateMutationRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
        }
    }

    fn signed_rollout_request(
        service_name: &str,
        rollout_handle: &str,
        healthy: bool,
        promote_to_percent: Option<u8>,
        governance_seed: &[u8],
        key_pair: &KeyPair,
    ) -> SignedRolloutAdvanceRequest {
        let payload = RolloutAdvancePayload {
            service_name: service_name.to_string(),
            rollout_handle: rollout_handle.to_string(),
            healthy,
            promote_to_percent,
            governance_tx_hash: Hash::new(governance_seed),
        };
        let encoded = norito::to_bytes(&payload).expect("encode rollout payload");
        let signature = Signature::new(key_pair.private_key(), &encoded);
        SignedRolloutAdvanceRequest {
            payload,
            provenance: ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            },
        }
    }

    #[tokio::test]
    async fn deploy_upgrade_rollback_workflow_updates_registry_and_audit_log() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();

        let deployed = registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");
        assert_eq!(deployed.action, SoracloudAction::Deploy);
        assert_eq!(deployed.current_version, "1.0.0");
        assert_eq!(deployed.audit_event_count, 1);

        let upgraded = registry
            .apply_upgrade(signed_bundle_request(fixture_bundle("1.1.0"), &key_pair))
            .await
            .expect("upgrade");
        assert_eq!(upgraded.action, SoracloudAction::Upgrade);
        assert_eq!(upgraded.previous_version.as_deref(), Some("1.0.0"));
        assert_eq!(upgraded.current_version, "1.1.0");
        assert_eq!(upgraded.audit_event_count, 2);

        let rolled_back = registry
            .apply_rollback(signed_rollback_request("web_portal", None, &key_pair))
            .await
            .expect("rollback");
        assert_eq!(rolled_back.action, SoracloudAction::Rollback);
        assert_eq!(rolled_back.current_version, "1.0.0");
        assert_eq!(rolled_back.audit_event_count, 3);

        let snapshot = registry.snapshot(Some("web_portal"), 10).await;
        assert_eq!(snapshot.service_count, 1);
        assert_eq!(snapshot.audit_event_count, 3);
        assert_eq!(snapshot.services[0].current_version, "1.0.0");
        assert_eq!(snapshot.recent_audit_events.len(), 3);
    }

    #[tokio::test]
    async fn deploy_rejects_invalid_bundle_signature() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        let other = KeyPair::random();
        let mut request = signed_bundle_request(fixture_bundle("1.0.0"), &key_pair);
        request.provenance.signature = Signature::new(other.private_key(), b"tampered-payload");

        let err = registry
            .apply_deploy(request)
            .await
            .expect_err("invalid signature must fail");
        assert_eq!(err.kind, SoracloudErrorKind::Unauthorized);
    }

    #[tokio::test]
    async fn state_mutation_tracks_binding_usage_for_current_revision() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let first = signed_state_mutation_request(
            StateMutationRequest {
                service_name: "web_portal".to_string(),
                binding_name: "session_store".to_string(),
                key: "/state/session/user-1".to_string(),
                operation: StateMutationOperation::Upsert,
                value_size_bytes: Some(128),
                encryption: SoraStateEncryptionV1::ClientCiphertext,
                governance_tx_hash: Hash::new(b"governance-tx-1"),
            },
            &key_pair,
        );
        let first_result = registry
            .apply_state_mutation(first)
            .await
            .expect("first upsert");
        assert_eq!(first_result.binding_total_bytes, 128);
        assert_eq!(first_result.binding_key_count, 1);
        assert_eq!(first_result.audit_event_count, 2);

        let second = signed_state_mutation_request(
            StateMutationRequest {
                service_name: "web_portal".to_string(),
                binding_name: "session_store".to_string(),
                key: "/state/session/user-1".to_string(),
                operation: StateMutationOperation::Upsert,
                value_size_bytes: Some(64),
                encryption: SoraStateEncryptionV1::ClientCiphertext,
                governance_tx_hash: Hash::new(b"governance-tx-2"),
            },
            &key_pair,
        );
        let second_result = registry
            .apply_state_mutation(second)
            .await
            .expect("overwrite upsert");
        assert_eq!(second_result.binding_total_bytes, 64);
        assert_eq!(second_result.binding_key_count, 1);
        assert_eq!(second_result.audit_event_count, 3);
    }

    #[tokio::test]
    async fn state_mutation_enforces_append_only_and_prefix_rules() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let first = signed_state_mutation_request(
            StateMutationRequest {
                service_name: "web_portal".to_string(),
                binding_name: "patient_records".to_string(),
                key: "/state/health/patient-1".to_string(),
                operation: StateMutationOperation::Upsert,
                value_size_bytes: Some(256),
                encryption: SoraStateEncryptionV1::FheCiphertext,
                governance_tx_hash: Hash::new(b"governance-tx-3"),
            },
            &key_pair,
        );
        registry
            .apply_state_mutation(first)
            .await
            .expect("append-only first write");

        let overwrite = signed_state_mutation_request(
            StateMutationRequest {
                service_name: "web_portal".to_string(),
                binding_name: "patient_records".to_string(),
                key: "/state/health/patient-1".to_string(),
                operation: StateMutationOperation::Upsert,
                value_size_bytes: Some(512),
                encryption: SoraStateEncryptionV1::FheCiphertext,
                governance_tx_hash: Hash::new(b"governance-tx-4"),
            },
            &key_pair,
        );
        let overwrite_err = registry
            .apply_state_mutation(overwrite)
            .await
            .expect_err("append-only overwrite must fail");
        assert_eq!(overwrite_err.kind, SoracloudErrorKind::Conflict);

        let wrong_prefix = signed_state_mutation_request(
            StateMutationRequest {
                service_name: "web_portal".to_string(),
                binding_name: "session_store".to_string(),
                key: "/state/other/key".to_string(),
                operation: StateMutationOperation::Upsert,
                value_size_bytes: Some(32),
                encryption: SoraStateEncryptionV1::ClientCiphertext,
                governance_tx_hash: Hash::new(b"governance-tx-5"),
            },
            &key_pair,
        );
        let prefix_err = registry
            .apply_state_mutation(wrong_prefix)
            .await
            .expect_err("wrong prefix must fail");
        assert_eq!(prefix_err.kind, SoracloudErrorKind::Conflict);
    }

    #[tokio::test]
    async fn rollout_canary_advances_and_closes_on_full_promotion() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");

        let upgraded = registry
            .apply_upgrade(signed_bundle_request(fixture_bundle("1.1.0"), &key_pair))
            .await
            .expect("upgrade");
        let handle = upgraded
            .rollout_handle
            .clone()
            .expect("upgrade should provide rollout handle");
        assert_eq!(upgraded.rollout_stage, Some(RolloutStage::Canary));
        assert_eq!(upgraded.rollout_percent, Some(20));

        let canary = registry
            .apply_rollout(signed_rollout_request(
                "web_portal",
                &handle,
                true,
                Some(50),
                b"rollout-canary-1",
                &key_pair,
            ))
            .await
            .expect("canary advance");
        assert_eq!(canary.action, SoracloudAction::Rollout);
        assert_eq!(canary.stage, RolloutStage::Canary);
        assert_eq!(canary.traffic_percent, 50);

        let promoted = registry
            .apply_rollout(signed_rollout_request(
                "web_portal",
                &handle,
                true,
                None,
                b"rollout-promote-2",
                &key_pair,
            ))
            .await
            .expect("promotion");
        assert_eq!(promoted.action, SoracloudAction::Rollout);
        assert_eq!(promoted.stage, RolloutStage::Promoted);
        assert_eq!(promoted.traffic_percent, 100);

        let snapshot = registry.snapshot(Some("web_portal"), 10).await;
        let service = snapshot.services.first().expect("service exists");
        assert!(
            service.active_rollout.is_none(),
            "promoted rollout should close"
        );
        assert_eq!(
            service.last_rollout.as_ref().map(|rollout| rollout.stage),
            Some(RolloutStage::Promoted)
        );
    }

    #[tokio::test]
    async fn rollout_auto_rolls_back_after_health_failures() {
        let registry = Registry::default();
        let key_pair = KeyPair::random();
        registry
            .apply_deploy(signed_bundle_request(fixture_bundle("1.0.0"), &key_pair))
            .await
            .expect("deploy");
        let upgraded = registry
            .apply_upgrade(signed_bundle_request(fixture_bundle("1.1.0"), &key_pair))
            .await
            .expect("upgrade");
        let handle = upgraded
            .rollout_handle
            .clone()
            .expect("upgrade should provide rollout handle");

        for index in 0..2 {
            let response = registry
                .apply_rollout(signed_rollout_request(
                    "web_portal",
                    &handle,
                    false,
                    None,
                    format!("rollout-fail-{index}").as_bytes(),
                    &key_pair,
                ))
                .await
                .expect("pre-threshold health failure");
            assert_eq!(response.action, SoracloudAction::Rollout);
            assert_eq!(response.stage, RolloutStage::Canary);
        }

        let rollback = registry
            .apply_rollout(signed_rollout_request(
                "web_portal",
                &handle,
                false,
                None,
                b"rollout-fail-terminal",
                &key_pair,
            ))
            .await
            .expect("terminal health failure should rollback");
        assert_eq!(rollback.action, SoracloudAction::Rollback);
        assert_eq!(rollback.stage, RolloutStage::RolledBack);
        assert_eq!(rollback.current_version, "1.0.0");
        assert_eq!(rollback.traffic_percent, 0);

        let snapshot = registry.snapshot(Some("web_portal"), 10).await;
        let service = snapshot.services.first().expect("service exists");
        assert_eq!(service.current_version, "1.0.0");
        assert!(service.active_rollout.is_none());
        assert_eq!(
            service.last_rollout.as_ref().map(|rollout| rollout.stage),
            Some(RolloutStage::RolledBack)
        );
    }
}
