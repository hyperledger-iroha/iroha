//! App-facing Soracloud control-plane shim.
//!
//! This module provides a deterministic in-memory control-plane surface for
//! `deploy`/`upgrade`/`rollback` workflows while SCR host integration is in
//! progress. Requests must carry signed payloads so admission can verify
//! manifest provenance before mutating registry state.

use std::collections::BTreeMap;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use iroha_crypto::Hash;
use iroha_data_model::{
    name::Name, smart_contract::manifest::ManifestProvenance, soracloud::SoraDeploymentBundleV1,
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
    pub signed_by: String,
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
                signed_by: signer.clone(),
            };

            entry.current_version = target.service_version.clone();
            entry.revisions.push(revision);
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
        })
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
            signed_by: signer.clone(),
        };

        let revision_count = {
            let entry = state
                .services
                .entry(service_name.clone())
                .or_insert_with(|| RegistryServiceEntry {
                    current_version: service_version.clone(),
                    revisions: Vec::new(),
                });
            entry.current_version = service_version.clone();
            entry.revisions.push(revision);
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

fn ensure_registry_schema(state: &RegistryState) -> Result<(), SoracloudError> {
    if state.schema_version != REGISTRY_SCHEMA_VERSION {
        return Err(SoracloudError::internal(format!(
            "unsupported registry schema {} (expected {REGISTRY_SCHEMA_VERSION})",
            state.schema_version
        )));
    }
    Ok(())
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
}
