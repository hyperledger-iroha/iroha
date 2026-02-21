//! Soracloud deployment helpers (`init/deploy/status/upgrade/rollback`).
//!
//! These commands provide a deterministic local control-plane simulation for
//! Soracloud manifests. They validate `SoraDeploymentBundleV1` admission rules
//! and maintain a machine-readable registry/audit log file that can be used by
//! scripts and CI checks. `init` also supports Vue3 scaffolding templates for
//! static sites and dynamic webapps.

use std::{
    collections::BTreeMap,
    fs,
    num::{NonZeroU16, NonZeroU64},
    path::{Path, PathBuf},
    time::Duration,
};

use eyre::{Result, WrapErr, eyre};
use iroha::data_model::{
    Encode,
    name::Name,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        SORA_DEPLOYMENT_BUNDLE_VERSION_V1, SORA_STATE_BINDING_VERSION_V1, SoraContainerManifestV1,
        SoraContainerRuntimeV1, SoraDeploymentBundleV1, SoraNetworkPolicyV1, SoraRouteTargetV1,
        SoraRouteVisibilityV1, SoraServiceManifestV1, SoraStateBindingV1, SoraStateEncryptionV1,
        SoraStateMutabilityV1, SoraStateScopeV1, SoraTlsModeV1,
    },
};
use iroha_crypto::{Hash, KeyPair, Signature};
use norito::json::{self, JsonDeserialize, JsonSerialize};
use reqwest::{
    blocking::Client as BlockingHttpClient,
    header::{self, HeaderValue},
};

use crate::{Run, RunContext};

const DEFAULT_CONTAINER_MANIFEST: &str = "fixtures/soracloud/sora_container_manifest_v1.json";
const DEFAULT_SERVICE_MANIFEST: &str = "fixtures/soracloud/sora_service_manifest_v1.json";
const DEFAULT_REGISTRY_PATH: &str = ".soracloud/registry.json";
const REGISTRY_SCHEMA_VERSION: u16 = 1;

/// Soracloud local control-plane commands.
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Scaffold baseline container/service manifests and initialize registry state.
    Init(InitArgs),
    /// Validate manifests and register a new service deployment.
    Deploy(DeployArgs),
    /// Show current registry state (all services or one service).
    Status(StatusArgs),
    /// Validate manifests and upgrade an existing deployed service.
    Upgrade(UpgradeArgs),
    /// Roll back a deployed service to a previous (or specified) version.
    Rollback(RollbackArgs),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Init(args) => context.print_data(&args.run()?),
            Command::Deploy(args) => {
                let output = args.run(MutationMode::Deploy, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::Status(args) => context.print_data(&args.run()?),
            Command::Upgrade(args) => {
                let output = args.run(MutationMode::Upgrade, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::Rollback(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
        }
    }
}

/// Arguments for `app soracloud init`.
#[derive(clap::Args, Debug)]
pub struct InitArgs {
    /// Directory where manifests and registry state will be created.
    #[arg(long, value_name = "DIR", default_value = ".soracloud")]
    output_dir: PathBuf,
    /// Logical service name used in the scaffolded service manifest.
    #[arg(long, value_name = "NAME", default_value = "web_portal")]
    service_name: String,
    /// Version string used in the scaffolded service manifest.
    #[arg(long, value_name = "VERSION", default_value = "0.1.0")]
    service_version: String,
    /// Scaffolding template to generate in addition to control-plane manifests.
    #[arg(long, value_enum, default_value_t = InitTemplate::Baseline)]
    template: InitTemplate,
    /// Overwrite existing files in the output directory.
    #[arg(long)]
    overwrite: bool,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Default)]
enum InitTemplate {
    /// Generate only Soracloud control-plane manifests.
    #[default]
    Baseline,
    /// Generate a Vue3/Vite static SPA starter with SoraFS publish workflow.
    Site,
    /// Generate a Vue3 SPA + API starter with session/auth and chain-ID hooks.
    Webapp,
}

impl InitTemplate {
    fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Site => "site",
            Self::Webapp => "webapp",
        }
    }
}

impl InitArgs {
    fn run(self) -> Result<InitOutput> {
        fs::create_dir_all(&self.output_dir).wrap_err_with(|| {
            format!(
                "failed to create output directory {}",
                self.output_dir.display()
            )
        })?;

        let service_name: Name = self
            .service_name
            .parse()
            .wrap_err("invalid --service-name for soracloud scaffold")?;
        if self.service_version.trim().is_empty() {
            return Err(eyre!("--service-version must not be empty"));
        }

        let mut container =
            load_json::<SoraContainerManifestV1>(&workspace_fixture(DEFAULT_CONTAINER_MANIFEST))?;
        let mut service =
            load_json::<SoraServiceManifestV1>(&workspace_fixture(DEFAULT_SERVICE_MANIFEST))?;

        apply_init_template_defaults(self.template, &service_name, &mut service, &mut container)?;

        service.service_name = service_name;
        service.service_version = self.service_version;
        let container_hash = Hash::new(Encode::encode(&container));
        service.container.manifest_hash = container_hash;
        service.container.expected_schema_version = container.schema_version;

        let bundle = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container: container.clone(),
            service: service.clone(),
        };
        bundle.validate_for_admission()?;

        let container_path = self.output_dir.join("container_manifest.json");
        let service_path = self.output_dir.join("service_manifest.json");
        let registry_path = self.output_dir.join("registry.json");
        ensure_can_write(&container_path, self.overwrite)?;
        ensure_can_write(&service_path, self.overwrite)?;
        ensure_can_write(&registry_path, self.overwrite)?;

        write_json(&container_path, &container)?;
        write_json(&service_path, &service)?;
        write_json(&registry_path, &RegistryState::default())?;
        let template_artifacts = scaffold_init_template(
            self.template,
            &self.output_dir,
            service.service_name.as_ref(),
            self.overwrite,
        )?;

        Ok(InitOutput {
            template: self.template.as_str().to_owned(),
            container_manifest_path: container_path.to_string_lossy().into_owned(),
            service_manifest_path: service_path.to_string_lossy().into_owned(),
            registry_path: registry_path.to_string_lossy().into_owned(),
            container_manifest_hash: bundle.container_manifest_hash(),
            service_manifest_hash: bundle.service_manifest_hash(),
            template_artifacts,
        })
    }
}

/// Arguments for `app soracloud deploy`.
#[derive(clap::Args, Debug)]
pub struct DeployArgs {
    /// Path to a `SoraContainerManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_CONTAINER_MANIFEST)]
    container: PathBuf,
    /// Path to a `SoraServiceManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_SERVICE_MANIFEST)]
    service: PathBuf,
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Optional Torii base URL to execute deploy against live control-plane APIs.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for Torii mutation requests.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl DeployArgs {
    fn run(self, mode: MutationMode, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let container: SoraContainerManifestV1 = load_json(&self.container)?;
        let service: SoraServiceManifestV1 = load_json(&self.service)?;
        let bundle = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        };
        bundle.validate_for_admission()?;

        if let Some(torii_url) = self.torii_url.as_deref() {
            let endpoint_path = match mode {
                MutationMode::Deploy => "v1/soracloud/deploy",
                MutationMode::Upgrade => "v1/soracloud/upgrade",
            };
            let request = signed_bundle_request(bundle, key_pair)?;
            let (_, payload) = post_torii_soracloud_mutation(
                torii_url,
                endpoint_path,
                &request,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let mut registry = load_registry(&self.registry)?;
        let output = apply_mutation(&mut registry, mode, &bundle)?;
        write_json(&self.registry, &registry)?;
        json::to_value(&output).wrap_err("failed to encode soracloud deploy output")
    }
}

/// Arguments for `app soracloud upgrade`.
#[derive(clap::Args, Debug)]
pub struct UpgradeArgs {
    /// Path to a `SoraContainerManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_CONTAINER_MANIFEST)]
    container: PathBuf,
    /// Path to a `SoraServiceManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_SERVICE_MANIFEST)]
    service: PathBuf,
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Optional Torii base URL to execute upgrade against live control-plane APIs.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for Torii mutation requests.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl UpgradeArgs {
    fn run(self, mode: MutationMode, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let container: SoraContainerManifestV1 = load_json(&self.container)?;
        let service: SoraServiceManifestV1 = load_json(&self.service)?;
        let bundle = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        };
        bundle.validate_for_admission()?;

        if let Some(torii_url) = self.torii_url.as_deref() {
            let endpoint_path = match mode {
                MutationMode::Deploy => "v1/soracloud/deploy",
                MutationMode::Upgrade => "v1/soracloud/upgrade",
            };
            let request = signed_bundle_request(bundle, key_pair)?;
            let (_, payload) = post_torii_soracloud_mutation(
                torii_url,
                endpoint_path,
                &request,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let mut registry = load_registry(&self.registry)?;
        let output = apply_mutation(&mut registry, mode, &bundle)?;
        write_json(&self.registry, &registry)?;
        json::to_value(&output).wrap_err("failed to encode soracloud upgrade output")
    }
}

/// Arguments for `app soracloud status`.
#[derive(clap::Args, Debug)]
pub struct StatusArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Optional service name filter.
    #[arg(long, value_name = "NAME")]
    service_name: Option<String>,
    /// Optional Torii base URL (for example `http://127.0.0.1:8080/`) to query
    /// `/v1/soracloud/status` from a live control plane.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when querying Torii.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for Torii status requests.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl StatusArgs {
    fn run(self) -> Result<StatusOutput> {
        let service_filter = self.service_name;
        if let Some(torii_url) = self.torii_url.as_deref() {
            let (endpoint, payload) = fetch_torii_soracloud_status(
                torii_url,
                service_filter.as_deref(),
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(StatusOutput::from_network(endpoint, payload));
        }

        let registry = load_registry(&self.registry)?;
        let mut services = Vec::new();
        for (service_name, entry) in &registry.services {
            if service_filter
                .as_ref()
                .is_some_and(|needle| needle != service_name)
            {
                continue;
            }
            services.push(ServiceStatusOutput::from_entry(service_name, entry));
        }
        Ok(StatusOutput::from_local(
            registry.schema_version,
            u32::try_from(services.len()).unwrap_or(u32::MAX),
            u32::try_from(registry.audit_log.len()).unwrap_or(u32::MAX),
            services,
        ))
    }
}

/// Arguments for `app soracloud rollback`.
#[derive(clap::Args, Debug)]
pub struct RollbackArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Service name to roll back.
    #[arg(long, value_name = "NAME")]
    service_name: String,
    /// Optional target version. When omitted, rolls back to the previous version.
    #[arg(long, value_name = "VERSION")]
    target_version: Option<String>,
    /// Optional Torii base URL to execute rollback against live control-plane APIs.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for Torii mutation requests.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl RollbackArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if let Some(torii_url) = self.torii_url.as_deref() {
            let request = signed_rollback_request(
                &self.service_name,
                self.target_version.as_deref(),
                key_pair,
            )?;
            let (_, payload) = post_torii_soracloud_mutation(
                torii_url,
                "v1/soracloud/rollback",
                &request,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let mut registry = load_registry(&self.registry)?;
        let output = apply_rollback(
            &mut registry,
            &self.service_name,
            self.target_version.as_deref(),
        )?;
        write_json(&self.registry, &registry)?;
        json::to_value(&output).wrap_err("failed to encode soracloud rollback output")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MutationMode {
    Deploy,
    Upgrade,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "action", content = "value")]
enum SoracloudAction {
    Deploy,
    Upgrade,
    Rollback,
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
    service_name: String,
    current_version: String,
    #[norito(default)]
    revisions: Vec<RegistryServiceRevision>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct RegistryServiceRevision {
    sequence: u64,
    action: SoracloudAction,
    service_version: String,
    service_manifest_hash: Hash,
    container_manifest_hash: Hash,
    replicas: u16,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    route_host: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    route_path_prefix: Option<String>,
    state_binding_count: u32,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct RegistryAuditEvent {
    sequence: u64,
    action: SoracloudAction,
    service_name: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    from_version: Option<String>,
    to_version: String,
    service_manifest_hash: Hash,
    container_manifest_hash: Hash,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct InitOutput {
    template: String,
    container_manifest_path: String,
    service_manifest_path: String,
    registry_path: String,
    container_manifest_hash: Hash,
    service_manifest_hash: Hash,
    #[norito(default)]
    template_artifacts: Vec<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct MutationOutput {
    action: SoracloudAction,
    service_name: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    previous_version: Option<String>,
    current_version: String,
    sequence: u64,
    service_manifest_hash: Hash,
    container_manifest_hash: Hash,
    revision_count: u32,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct StatusOutput {
    source: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    torii_endpoint: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    schema_version: Option<u16>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    service_count: Option<u32>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    audit_event_count: Option<u32>,
    #[norito(default)]
    services: Vec<ServiceStatusOutput>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    network_status: Option<norito::json::Value>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct ServiceStatusOutput {
    service_name: String,
    current_version: String,
    revision_count: u32,
    #[norito(default)]
    latest_revision: Option<RegistryServiceRevision>,
}

impl ServiceStatusOutput {
    fn from_entry(service_name: &str, entry: &RegistryServiceEntry) -> Self {
        Self {
            service_name: service_name.to_string(),
            current_version: entry.current_version.clone(),
            revision_count: u32::try_from(entry.revisions.len()).unwrap_or(u32::MAX),
            latest_revision: entry.revisions.last().cloned(),
        }
    }
}

impl StatusOutput {
    fn from_local(
        schema_version: u16,
        service_count: u32,
        audit_event_count: u32,
        services: Vec<ServiceStatusOutput>,
    ) -> Self {
        Self {
            source: "local_registry".to_owned(),
            torii_endpoint: None,
            schema_version: Some(schema_version),
            service_count: Some(service_count),
            audit_event_count: Some(audit_event_count),
            services,
            network_status: None,
        }
    }

    fn from_network(endpoint: String, network_status: norito::json::Value) -> Self {
        Self {
            source: "torii_control_plane".to_owned(),
            torii_endpoint: Some(endpoint),
            schema_version: None,
            service_count: None,
            audit_event_count: None,
            services: Vec::new(),
            network_status: Some(network_status),
        }
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedBundleRequest {
    bundle: SoraDeploymentBundleV1,
    provenance: ManifestProvenance,
}

#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
struct RollbackPayload {
    service_name: String,
    #[norito(default)]
    target_version: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedRollbackRequest {
    payload: RollbackPayload,
    provenance: ManifestProvenance,
}

fn apply_mutation(
    registry: &mut RegistryState,
    mode: MutationMode,
    bundle: &SoraDeploymentBundleV1,
) -> Result<MutationOutput> {
    ensure_registry_schema(registry)?;

    let service_name = bundle.service.service_name.to_string();
    let next_sequence = registry.next_sequence;
    let entry = registry.services.get_mut(&service_name);
    let (action, previous_version) = match (mode, entry.is_some()) {
        (MutationMode::Deploy, false) => (SoracloudAction::Deploy, None),
        (MutationMode::Deploy, true) => {
            return Err(eyre!(
                "service `{service_name}` already deployed; use `app soracloud upgrade`"
            ));
        }
        (MutationMode::Upgrade, false) => {
            return Err(eyre!(
                "service `{service_name}` not found; deploy it before upgrading"
            ));
        }
        (MutationMode::Upgrade, true) => (
            SoracloudAction::Upgrade,
            entry.map(|e| e.current_version.clone()),
        ),
    };

    let container_manifest_hash = bundle.container_manifest_hash();
    let service_manifest_hash = bundle.service_manifest_hash();
    let service_version = bundle.service.service_version.clone();

    if let Some(existing) = registry.services.get(&service_name)
        && existing.current_version == service_version
    {
        return Err(eyre!(
            "service `{service_name}` already at version `{service_version}`"
        ));
    }

    let revision = RegistryServiceRevision {
        sequence: next_sequence,
        action,
        service_version: service_version.clone(),
        service_manifest_hash,
        container_manifest_hash,
        replicas: bundle.service.replicas.get(),
        route_host: bundle
            .service
            .route
            .as_ref()
            .map(|route| route.host.clone()),
        route_path_prefix: bundle
            .service
            .route
            .as_ref()
            .map(|route| route.path_prefix.clone()),
        state_binding_count: u32::try_from(bundle.service.state_bindings.len()).unwrap_or(u32::MAX),
    };

    let entry = registry
        .services
        .entry(service_name.clone())
        .or_insert_with(|| RegistryServiceEntry {
            service_name: service_name.clone(),
            current_version: service_version.clone(),
            revisions: Vec::new(),
        });
    entry.current_version = service_version.clone();
    entry.revisions.push(revision);

    registry.audit_log.push(RegistryAuditEvent {
        sequence: next_sequence,
        action,
        service_name: service_name.clone(),
        from_version: previous_version.clone(),
        to_version: service_version.clone(),
        service_manifest_hash,
        container_manifest_hash,
    });
    registry.next_sequence = registry.next_sequence.saturating_add(1);

    Ok(MutationOutput {
        action,
        service_name,
        previous_version,
        current_version: service_version,
        sequence: next_sequence,
        service_manifest_hash,
        container_manifest_hash,
        revision_count: u32::try_from(entry.revisions.len()).unwrap_or(u32::MAX),
    })
}

fn apply_rollback(
    registry: &mut RegistryState,
    service_name: &str,
    target_version: Option<&str>,
) -> Result<MutationOutput> {
    ensure_registry_schema(registry)?;
    let entry = registry
        .services
        .get_mut(service_name)
        .ok_or_else(|| eyre!("service `{service_name}` not found in registry"))?;
    let previous_version = Some(entry.current_version.clone());

    let target = if let Some(target_version) = target_version {
        entry
            .revisions
            .iter()
            .rev()
            .find(|revision| revision.service_version == target_version)
            .cloned()
            .ok_or_else(|| {
                eyre!(
                    "service `{service_name}` has no deployed revision for version `{target_version}`"
                )
            })?
    } else {
        entry
            .revisions
            .iter()
            .rev()
            .find(|revision| revision.service_version != entry.current_version)
            .cloned()
            .ok_or_else(|| {
                eyre!("service `{service_name}` has no previous revision to roll back to")
            })?
    };

    let sequence = registry.next_sequence;
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
    };
    entry.current_version = target.service_version.clone();
    entry.revisions.push(rollback_revision);

    registry.audit_log.push(RegistryAuditEvent {
        sequence,
        action: SoracloudAction::Rollback,
        service_name: service_name.to_string(),
        from_version: previous_version.clone(),
        to_version: target.service_version.clone(),
        service_manifest_hash: target.service_manifest_hash,
        container_manifest_hash: target.container_manifest_hash,
    });
    registry.next_sequence = registry.next_sequence.saturating_add(1);

    Ok(MutationOutput {
        action: SoracloudAction::Rollback,
        service_name: service_name.to_string(),
        previous_version,
        current_version: target.service_version,
        sequence,
        service_manifest_hash: target.service_manifest_hash,
        container_manifest_hash: target.container_manifest_hash,
        revision_count: u32::try_from(entry.revisions.len()).unwrap_or(u32::MAX),
    })
}

fn ensure_registry_schema(registry: &RegistryState) -> Result<()> {
    if registry.schema_version != REGISTRY_SCHEMA_VERSION {
        return Err(eyre!(
            "unsupported soracloud registry schema {} (expected {})",
            registry.schema_version,
            REGISTRY_SCHEMA_VERSION
        ));
    }
    Ok(())
}

fn load_registry(path: &Path) -> Result<RegistryState> {
    if !path.exists() {
        return Ok(RegistryState::default());
    }
    let state: RegistryState = load_json(path)?;
    ensure_registry_schema(&state)?;
    Ok(state)
}

fn signed_bundle_request(
    bundle: SoraDeploymentBundleV1,
    key_pair: &KeyPair,
) -> Result<SignedBundleRequest> {
    let payload =
        norito::to_bytes(&bundle).wrap_err("failed to encode deployment bundle for signing")?;
    let signature = Signature::new(key_pair.private_key(), &payload);
    Ok(SignedBundleRequest {
        bundle,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_rollback_request(
    service_name: &str,
    target_version: Option<&str>,
    key_pair: &KeyPair,
) -> Result<SignedRollbackRequest> {
    if service_name.trim().is_empty() {
        return Err(eyre!("--service-name must not be empty"));
    }
    let payload = RollbackPayload {
        service_name: service_name.to_string(),
        target_version: target_version.map(ToOwned::to_owned),
    };
    let encoded =
        norito::to_bytes(&payload).wrap_err("failed to encode rollback payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedRollbackRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn post_torii_soracloud_mutation<T>(
    torii_url: &str,
    endpoint_path: &str,
    request_payload: &T,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)>
where
    T: JsonSerialize + ?Sized,
{
    let endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join(endpoint_path)
        .wrap_err_with(|| format!("failed to derive /{endpoint_path} URL from --torii-url"))?;
    let body = json::to_vec(request_payload)
        .wrap_err("failed to encode soracloud mutation request payload")?;

    let timeout = Duration::from_secs(timeout_secs.max(1));
    let client = BlockingHttpClient::builder()
        .timeout(timeout)
        .build()
        .wrap_err("failed to build HTTP client for soracloud mutation")?;

    let mut request = client
        .post(endpoint.clone())
        .header(header::ACCEPT, HeaderValue::from_static("application/json"))
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )
        .body(body);
    if let Some(token) = api_token {
        request = request.header("x-api-token", token);
    }

    let response = request
        .send()
        .wrap_err_with(|| format!("failed to call `{}`", endpoint.as_str()))?;
    let status = response.status();
    let body = response
        .bytes()
        .wrap_err("failed to read Torii mutation response body")?;
    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body);
        return Err(eyre!(
            "Torii /{endpoint_path} returned {}: {}",
            status,
            body_text
        ));
    }
    let payload: norito::json::Value =
        json::from_slice(&body).wrap_err("failed to decode Torii mutation JSON payload")?;
    Ok((endpoint.to_string(), payload))
}

fn fetch_torii_soracloud_status(
    torii_url: &str,
    service_name: Option<&str>,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/status")
        .wrap_err("failed to derive /v1/soracloud/status URL from --torii-url")?;

    if let Some(service_name) = service_name
        && !service_name.is_empty()
    {
        endpoint
            .query_pairs_mut()
            .append_pair("service_name", service_name);
    }

    let timeout = Duration::from_secs(timeout_secs.max(1));
    let client = BlockingHttpClient::builder()
        .timeout(timeout)
        .build()
        .wrap_err("failed to build HTTP client for soracloud status")?;

    let mut request = client.get(endpoint.clone());
    request = request.header(header::ACCEPT, HeaderValue::from_static("application/json"));
    if let Some(token) = api_token {
        request = request.header("x-api-token", token);
    }

    let response = request
        .send()
        .wrap_err_with(|| format!("failed to fetch `{}`", endpoint.as_str()))?;
    let status = response.status();
    let body = response
        .bytes()
        .wrap_err("failed to read Torii status response body")?;
    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body);
        return Err(eyre!(
            "Torii /v1/soracloud/status returned {}: {}",
            status,
            body_text
        ));
    }

    let payload: norito::json::Value =
        json::from_slice(&body).wrap_err("failed to decode Torii soracloud status JSON payload")?;
    Ok((endpoint.to_string(), payload))
}

fn load_json<T>(path: &Path) -> Result<T>
where
    T: JsonDeserialize,
{
    let bytes =
        fs::read(path).wrap_err_with(|| format!("failed to read JSON file {}", path.display()))?;
    json::from_slice(&bytes).wrap_err_with(|| format!("failed to decode {}", path.display()))
}

fn write_json<T>(path: &Path, value: &T) -> Result<()>
where
    T: JsonSerialize + ?Sized,
{
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create directory {}", parent.display()))?;
    }
    let bytes = json::to_vec_pretty(value).wrap_err("failed to encode JSON")?;
    fs::write(path, bytes).wrap_err_with(|| format!("failed to write {}", path.display()))
}

fn ensure_can_write(path: &Path, overwrite: bool) -> Result<()> {
    if !overwrite && path.exists() {
        return Err(eyre!(
            "file {} already exists (use --overwrite to replace it)",
            path.display()
        ));
    }
    Ok(())
}

fn workspace_fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}

fn apply_init_template_defaults(
    template: InitTemplate,
    service_name: &Name,
    service: &mut SoraServiceManifestV1,
    container: &mut SoraContainerManifestV1,
) -> Result<()> {
    let dns_label = normalized_service_label(service_name.as_ref());
    let host = format!("{dns_label}.sora");
    match template {
        InitTemplate::Baseline => Ok(()),
        InitTemplate::Site => {
            container.runtime = SoraContainerRuntimeV1::NativeProcess;
            container.bundle_path = "/bundles/site-static.car".to_owned();
            container.entrypoint = "/usr/bin/sorafs-static-gateway".to_owned();
            container.args = vec!["--root=/app/dist".to_owned(), "--port=8080".to_owned()];
            container
                .env
                .insert("SORACLOUD_TEMPLATE".to_owned(), "site".to_owned());
            container.capabilities.network =
                SoraNetworkPolicyV1::Allowlist(vec!["torii.sora.internal".to_owned()]);
            container.capabilities.allow_wallet_signing = false;
            container.capabilities.allow_state_writes = false;
            container.capabilities.allow_model_training = false;
            container.lifecycle.healthcheck_path = Some("/healthz".to_owned());

            service.route = Some(SoraRouteTargetV1 {
                host,
                path_prefix: "/".to_owned(),
                service_port: NonZeroU16::new(8080).expect("nonzero literal"),
                visibility: SoraRouteVisibilityV1::Public,
                tls_mode: SoraTlsModeV1::Required,
            });
            service.replicas = NonZeroU16::new(2).expect("nonzero literal");
            service.state_bindings.clear();
            Ok(())
        }
        InitTemplate::Webapp => {
            container.runtime = SoraContainerRuntimeV1::NativeProcess;
            container.bundle_path = "/bundles/webapp-api.car".to_owned();
            container.entrypoint = "/app/api/server.mjs".to_owned();
            container.args = vec!["--port=8787".to_owned()];
            container
                .env
                .insert("SORACLOUD_TEMPLATE".to_owned(), "webapp".to_owned());
            container.env.insert(
                "CHAIN_IDENTITY_ENDPOINT".to_owned(),
                "http://127.0.0.1:8080".to_owned(),
            );
            container.capabilities.network = SoraNetworkPolicyV1::Allowlist(vec![
                "torii.sora.internal".to_owned(),
                "wallet.sora.internal".to_owned(),
            ]);
            container.capabilities.allow_wallet_signing = true;
            container.capabilities.allow_state_writes = true;
            container.capabilities.allow_model_training = false;
            container.lifecycle.healthcheck_path = Some("/api/healthz".to_owned());

            service.route = Some(SoraRouteTargetV1 {
                host,
                path_prefix: "/api".to_owned(),
                service_port: NonZeroU16::new(8787).expect("nonzero literal"),
                visibility: SoraRouteVisibilityV1::Public,
                tls_mode: SoraTlsModeV1::Required,
            });
            service.state_bindings = vec![SoraStateBindingV1 {
                schema_version: SORA_STATE_BINDING_VERSION_V1,
                binding_name: "session_store"
                    .parse()
                    .expect("literal binding name is valid"),
                scope: SoraStateScopeV1::ServiceState,
                mutability: SoraStateMutabilityV1::ReadWrite,
                encryption: SoraStateEncryptionV1::ClientCiphertext,
                key_prefix: "/state/session".to_owned(),
                max_item_bytes: NonZeroU64::new(4_096).expect("nonzero literal"),
                max_total_bytes: NonZeroU64::new(262_144).expect("nonzero literal"),
            }];
            Ok(())
        }
    }
}

fn scaffold_init_template(
    template: InitTemplate,
    output_dir: &Path,
    service_name: &str,
    overwrite: bool,
) -> Result<Vec<String>> {
    match template {
        InitTemplate::Baseline => Ok(Vec::new()),
        InitTemplate::Site => scaffold_site_template(output_dir, service_name, overwrite),
        InitTemplate::Webapp => scaffold_webapp_template(output_dir, service_name, overwrite),
    }
}

fn scaffold_site_template(
    output_dir: &Path,
    service_name: &str,
    overwrite: bool,
) -> Result<Vec<String>> {
    let project_dir = output_dir.join("site");
    let package_name = normalized_service_label(service_name);
    let dns_host = format!("{package_name}.sora");
    let files = vec![
        (
            project_dir.join("package.json"),
            site_package_json(&package_name),
        ),
        (
            project_dir.join("tsconfig.json"),
            site_tsconfig_json().to_owned(),
        ),
        (
            project_dir.join("vite.config.ts"),
            site_vite_config().to_owned(),
        ),
        (project_dir.join("index.html"), site_index_html().to_owned()),
        (project_dir.join("src/main.ts"), site_main_ts().to_owned()),
        (project_dir.join("src/App.vue"), site_app_vue(service_name)),
        (
            project_dir.join(".gitignore"),
            "node_modules/\ndist/\n".to_owned(),
        ),
        (
            project_dir.join("README.md"),
            site_readme(service_name, &dns_host),
        ),
    ];
    write_template_files(files, overwrite)
}

fn scaffold_webapp_template(
    output_dir: &Path,
    service_name: &str,
    overwrite: bool,
) -> Result<Vec<String>> {
    let project_dir = output_dir.join("webapp");
    let package_name = normalized_service_label(service_name);
    let files = vec![
        (
            project_dir.join("package.json"),
            webapp_root_package_json(&package_name),
        ),
        (
            project_dir.join("frontend/package.json"),
            webapp_frontend_package_json(&package_name),
        ),
        (
            project_dir.join("frontend/tsconfig.json"),
            site_tsconfig_json().to_owned(),
        ),
        (
            project_dir.join("frontend/vite.config.ts"),
            webapp_frontend_vite_config().to_owned(),
        ),
        (
            project_dir.join("frontend/index.html"),
            site_index_html().to_owned(),
        ),
        (
            project_dir.join("frontend/src/main.ts"),
            site_main_ts().to_owned(),
        ),
        (
            project_dir.join("frontend/src/App.vue"),
            webapp_frontend_app_vue(service_name),
        ),
        (
            project_dir.join("api/server.mjs"),
            webapp_api_server_mjs().to_owned(),
        ),
        (project_dir.join("README.md"), webapp_readme(service_name)),
        (
            project_dir.join(".gitignore"),
            "node_modules/\nfrontend/node_modules/\nfrontend/dist/\n".to_owned(),
        ),
    ];
    write_template_files(files, overwrite)
}

fn write_template_files(files: Vec<(PathBuf, String)>, overwrite: bool) -> Result<Vec<String>> {
    let mut written = Vec::with_capacity(files.len());
    for (path, body) in files {
        write_template_file(&path, &body, overwrite)?;
        written.push(path.to_string_lossy().into_owned());
    }
    Ok(written)
}

fn write_template_file(path: &Path, body: &str, overwrite: bool) -> Result<()> {
    ensure_can_write(path, overwrite)?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!("failed to create template directory {}", parent.display())
        })?;
    }
    fs::write(path, body)
        .wrap_err_with(|| format!("failed to write template file {}", path.display()))
}

fn normalized_service_label(service_name: &str) -> String {
    let mut out = String::new();
    for ch in service_name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch == '-' || ch == '_') && !out.ends_with('-') {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "sora-app".to_owned()
    } else {
        out
    }
}

fn site_package_json(package_name: &str) -> String {
    format!(
        r#"{{
  "name": "{package_name}-site",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {{
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview"
  }},
  "dependencies": {{
    "vue": "^3.5.0"
  }},
  "devDependencies": {{
    "@vitejs/plugin-vue": "^5.2.0",
    "typescript": "^5.6.0",
    "vite": "^5.4.0"
  }}
}}
"#
    )
}

fn webapp_root_package_json(package_name: &str) -> String {
    format!(
        r#"{{
  "name": "{package_name}-webapp",
  "private": true,
  "version": "0.1.0",
  "scripts": {{
    "dev:frontend": "npm --prefix frontend run dev",
    "dev:api": "node api/server.mjs",
    "build": "npm --prefix frontend run build",
    "start": "node api/server.mjs"
  }}
}}
"#
    )
}

fn webapp_frontend_package_json(package_name: &str) -> String {
    format!(
        r#"{{
  "name": "{package_name}-frontend",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {{
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview"
  }},
  "dependencies": {{
    "vue": "^3.5.0"
  }},
  "devDependencies": {{
    "@vitejs/plugin-vue": "^5.2.0",
    "typescript": "^5.6.0",
    "vite": "^5.4.0"
  }}
}}
"#
    )
}

fn site_tsconfig_json() -> &'static str {
    r#"{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "module": "ESNext",
    "moduleResolution": "Node",
    "strict": true,
    "jsx": "preserve",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "esModuleInterop": true,
    "lib": [
      "ES2020",
      "DOM",
      "DOM.Iterable"
    ],
    "types": [
      "vite/client"
    ]
  },
  "include": [
    "src/**/*.ts",
    "src/**/*.vue"
  ]
}
"#
}

fn site_vite_config() -> &'static str {
    r#"import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  server: {
    host: "0.0.0.0",
    port: 5173
  }
});
"#
}

fn webapp_frontend_vite_config() -> &'static str {
    r#"import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  server: {
    host: "0.0.0.0",
    port: 5173,
    proxy: {
      "/api": "http://127.0.0.1:8787"
    }
  }
});
"#
}

fn site_index_html() -> &'static str {
    r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>SoraCloud App</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
"#
}

fn site_main_ts() -> &'static str {
    r##"import { createApp } from "vue";
import App from "./App.vue";

createApp(App).mount("#app");
"##
}

fn site_app_vue(service_name: &str) -> String {
    format!(
        r#"<template>
  <main class="shell">
    <h1>{service_name}</h1>
    <p>This Vue3 static site is ready for SoraFS packaging and SoraDNS binding.</p>
  </main>
</template>

<style scoped>
.shell {{
  font-family: "Avenir Next", "Segoe UI", sans-serif;
  max-width: 720px;
  margin: 4rem auto;
  padding: 0 1.25rem;
  color: #16324f;
}}

h1 {{
  font-size: 2.25rem;
  margin: 0 0 1rem;
}}
</style>
"#
    )
}

fn webapp_frontend_app_vue(service_name: &str) -> String {
    format!(
        r#"<template>
  <main class="shell">
    <h1>{service_name} Control Panel</h1>
    <form @submit.prevent="login">
      <label>
        Account
        <input v-model="account" placeholder="ih58..." />
      </label>
      <label>
        Signature
        <input v-model="signature" placeholder="hex signature" />
      </label>
      <button type="submit">Start Session</button>
    </form>
    <p v-if="session">{{{{ session }}}}</p>
    <p v-if="error" class="error">{{{{ error }}}}</p>
  </main>
</template>

<script setup lang="ts">
import {{ ref }} from "vue";

const account = ref("");
const signature = ref("");
const session = ref("");
const error = ref("");

async function login() {{
  error.value = "";
  const response = await fetch("/api/session", {{
    method: "POST",
    headers: {{ "content-type": "application/json" }},
    body: JSON.stringify({{ account: account.value, signature: signature.value }})
  }});
  if (!response.ok) {{
    error.value = await response.text();
    return;
  }}
  const payload = await response.json();
  session.value = `active for ${{payload.account}}`;
}}
</script>

<style scoped>
.shell {{
  font-family: "Avenir Next", "Segoe UI", sans-serif;
  max-width: 720px;
  margin: 4rem auto;
  padding: 0 1.25rem;
}}

form {{
  display: grid;
  gap: 0.75rem;
  margin: 1.5rem 0;
}}

input {{
  width: 100%;
  padding: 0.5rem;
}}

.error {{
  color: #b42318;
}}
</style>
"#
    )
}

fn webapp_api_server_mjs() -> &'static str {
    r#"import http from "node:http";
import crypto from "node:crypto";

const portArg = process.argv.find((value) => value.startsWith("--port="));
const port = Number(portArg?.slice("--port=".length) ?? process.env.PORT ?? "8787");
const sessionKey = process.env.SESSION_HMAC_KEY ?? "replace-me-with-a-random-key";

function parseCookie(headerValue = "") {
  const cookies = Object.create(null);
  for (const entry of headerValue.split(";")) {
    const [rawKey, rawValue] = entry.trim().split("=");
    if (!rawKey || !rawValue) {
      continue;
    }
    cookies[rawKey] = decodeURIComponent(rawValue);
  }
  return cookies;
}

function signSession(account) {
  const mac = crypto.createHmac("sha256", sessionKey).update(account).digest("hex");
  return `${account}.${mac}`;
}

function verifySession(token) {
  const [account, mac] = token.split(".");
  if (!account || !mac) {
    return null;
  }
  const expected = crypto.createHmac("sha256", sessionKey).update(account).digest("hex");
  const a = Buffer.from(mac, "hex");
  const b = Buffer.from(expected, "hex");
  if (a.length !== b.length || !crypto.timingSafeEqual(a, b)) {
    return null;
  }
  return account;
}

async function readJson(req) {
  let body = "";
  for await (const chunk of req) {
    body += chunk.toString("utf8");
  }
  return JSON.parse(body);
}

async function verifyChainIdentity(account, signature) {
  if (!account || !signature) {
    return false;
  }
  // TODO: replace with deterministic Torii verification flow for signatures.
  return true;
}

const server = http.createServer(async (req, res) => {
  if (req.url === "/api/healthz") {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ ok: true }));
    return;
  }

  if (req.method === "POST" && req.url === "/api/session") {
    try {
      const body = await readJson(req);
      if (!(await verifyChainIdentity(body.account, body.signature))) {
        res.writeHead(401, { "content-type": "text/plain" });
        res.end("chain identity verification failed");
        return;
      }
      const token = signSession(body.account);
      res.writeHead(200, {
        "content-type": "application/json",
        "set-cookie": `session=${encodeURIComponent(token)}; HttpOnly; Path=/; SameSite=Lax`
      });
      res.end(JSON.stringify({ account: body.account }));
    } catch (error) {
      res.writeHead(400, { "content-type": "text/plain" });
      res.end(`invalid request: ${error.message}`);
    }
    return;
  }

  if (req.method === "GET" && req.url === "/api/me") {
    const cookies = parseCookie(req.headers.cookie);
    const account = cookies.session ? verifySession(cookies.session) : null;
    if (!account) {
      res.writeHead(401, { "content-type": "text/plain" });
      res.end("no active session");
      return;
    }
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ account }));
    return;
  }

  if (req.method === "POST" && req.url === "/api/logout") {
    res.writeHead(204, {
      "set-cookie": "session=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax"
    });
    res.end();
    return;
  }

  res.writeHead(404, { "content-type": "text/plain" });
  res.end("not found");
});

server.listen(port, "0.0.0.0", () => {
  // eslint-disable-next-line no-console
  console.log(`api listening on :${port}`);
});
"#
}

fn site_readme(service_name: &str, dns_host: &str) -> String {
    format!(
        r#"# {service_name} Static Site Template

This template is generated by:

```bash
iroha app soracloud init --template site --service-name {service_name}
```

## Local dev

```bash
npm install
npm run dev
```

## Build and package for SoraFS

```bash
npm run build
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

## Register and bind on SoraDNS

`pin register` needs `chunk_digest_sha3_256` and governance alias proof material.

```bash
export CHUNK_DIGEST_HEX=<chunk_digest_sha3_256_hex>
export CURRENT_EPOCH=<network_epoch>
iroha app sorafs pin register \
  --manifest ../sorafs/site_manifest.to \
  --chunk-digest "$CHUNK_DIGEST_HEX" \
  --submitted-epoch "$CURRENT_EPOCH" \
  --alias-namespace soradns \
  --alias-name {dns_host} \
  --alias-proof ../sorafs/alias_proof.bin
```
"#
    )
}

fn webapp_readme(service_name: &str) -> String {
    format!(
        r#"# {service_name} Webapp Template

This template provides:

- `frontend/` Vue3 SPA (Vite).
- `api/server.mjs` deterministic HTTP API with signed-cookie sessions.
- Soracloud manifests at the parent init directory (`container_manifest.json`, `service_manifest.json`).

## Local dev

```bash
npm install
npm --prefix frontend install
npm run dev:api
npm run dev:frontend
```

## Deploy API service on Soracloud

```bash
iroha app soracloud deploy \
  --container ../container_manifest.json \
  --service ../service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

## Publish frontend to SoraFS

```bash
npm run build
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

Update `api/server.mjs` `verifyChainIdentity` with your chain signature-verification policy before production use.
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("iroha_soracloud_cli_{name}_{nanos}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn fixture_container() -> SoraContainerManifestV1 {
        load_json(&workspace_fixture(DEFAULT_CONTAINER_MANIFEST)).expect("container fixture")
    }

    fn fixture_service() -> SoraServiceManifestV1 {
        load_json(&workspace_fixture(DEFAULT_SERVICE_MANIFEST)).expect("service fixture")
    }

    #[test]
    fn deploy_upgrade_rollback_workflow_updates_registry() {
        let dir = temp_dir("workflow");
        let registry_path = dir.join("registry.json");

        let container = fixture_container();
        let container_hash = Hash::new(Encode::encode(&container));
        let mut service_v1 = fixture_service();
        service_v1.service_version = "1.0.0".to_string();
        service_v1.container.manifest_hash = container_hash;
        let bundle_v1 = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container: container.clone(),
            service: service_v1,
        };
        bundle_v1.validate_for_admission().expect("bundle v1 valid");

        let mut registry = RegistryState::default();
        let deployed = apply_mutation(&mut registry, MutationMode::Deploy, &bundle_v1)
            .expect("deploy should succeed");
        assert_eq!(deployed.current_version, "1.0.0");
        write_json(&registry_path, &registry).expect("write registry");

        let mut service_v2 = fixture_service();
        service_v2.service_version = "1.1.0".to_string();
        service_v2.container.manifest_hash = container_hash;
        let bundle_v2 = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service: service_v2,
        };
        let mut loaded = load_registry(&registry_path).expect("load registry");
        let upgraded = apply_mutation(&mut loaded, MutationMode::Upgrade, &bundle_v2)
            .expect("upgrade should succeed");
        assert_eq!(upgraded.previous_version.as_deref(), Some("1.0.0"));
        assert_eq!(upgraded.current_version, "1.1.0");

        let rolled_back =
            apply_rollback(&mut loaded, "web_portal", None).expect("rollback should succeed");
        assert_eq!(rolled_back.current_version, "1.0.0");
        assert_eq!(loaded.audit_log.len(), 3);
    }

    #[test]
    fn deploy_rejects_existing_service() {
        let container = fixture_container();
        let container_hash = Hash::new(Encode::encode(&container));
        let mut service = fixture_service();
        service.service_version = "1.0.0".to_string();
        service.container.manifest_hash = container_hash;
        let bundle = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        };

        let mut registry = RegistryState::default();
        apply_mutation(&mut registry, MutationMode::Deploy, &bundle).expect("first deploy");
        let err = apply_mutation(&mut registry, MutationMode::Deploy, &bundle)
            .expect_err("second deploy must fail");
        assert!(err.to_string().contains("already deployed"));
    }

    #[test]
    fn rollback_rejects_unknown_target_version() {
        let container = fixture_container();
        let container_hash = Hash::new(Encode::encode(&container));
        let mut service = fixture_service();
        service.service_version = "1.0.0".to_string();
        service.container.manifest_hash = container_hash;
        let bundle = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        };
        let mut registry = RegistryState::default();
        apply_mutation(&mut registry, MutationMode::Deploy, &bundle).expect("deploy");

        let err = apply_rollback(&mut registry, "web_portal", Some("9.9.9"))
            .expect_err("unknown rollback target should fail");
        assert!(err.to_string().contains("no deployed revision"));
    }

    #[test]
    fn status_output_can_represent_torii_control_plane_snapshot() {
        let payload = norito::json!({
            "schema_version": 1,
            "service_health": {
                "mode": "local_only",
                "status": "not_configured"
            }
        });
        let output = StatusOutput::from_network(
            "http://127.0.0.1:8080/v1/soracloud/status".to_owned(),
            payload.clone(),
        );
        assert_eq!(output.source, "torii_control_plane");
        assert!(output.torii_endpoint.is_some());
        let payload = output.network_status.expect("network payload");
        assert_eq!(
            payload
                .get("schema_version")
                .and_then(norito::json::Value::as_u64),
            Some(1)
        );
    }

    #[test]
    fn fetch_torii_status_rejects_invalid_url() {
        let err = fetch_torii_soracloud_status("not-a-url", None, None, 5)
            .expect_err("invalid URL must fail");
        assert!(err.to_string().contains("invalid --torii-url"));
    }

    #[test]
    fn signed_bundle_request_uses_verifiable_signature() {
        let container = fixture_container();
        let mut service = fixture_service();
        service.container.manifest_hash = Hash::new(Encode::encode(&container));
        let bundle = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        };
        let key_pair = KeyPair::random();
        let request = signed_bundle_request(bundle, &key_pair).expect("signed request");
        let payload = norito::to_bytes(&request.bundle).expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn post_torii_mutation_rejects_invalid_url() {
        let payload = norito::json!({ "noop": true });
        let err =
            post_torii_soracloud_mutation("not-a-url", "v1/soracloud/deploy", &payload, None, 5)
                .expect_err("invalid URL must fail");
        assert!(err.to_string().contains("invalid --torii-url"));
    }

    #[test]
    fn init_site_template_scaffolds_vue_and_sorafs_workflow() {
        let dir = temp_dir("site_template");
        let output = InitArgs {
            output_dir: dir.clone(),
            service_name: "docs_portal".to_owned(),
            service_version: "1.0.0".to_owned(),
            template: InitTemplate::Site,
            overwrite: false,
        }
        .run()
        .expect("site init should succeed");

        assert_eq!(output.template, "site");
        assert!(dir.join("site/package.json").exists());
        assert!(dir.join("site/src/App.vue").exists());

        let readme = fs::read_to_string(dir.join("site/README.md")).expect("read site readme");
        assert!(readme.contains("iroha app sorafs toolkit pack"));
        assert!(readme.contains("alias-namespace soradns"));

        let container: SoraContainerManifestV1 =
            load_json(&dir.join("container_manifest.json")).expect("container manifest");
        assert_eq!(container.runtime, SoraContainerRuntimeV1::NativeProcess);
        let service: SoraServiceManifestV1 =
            load_json(&dir.join("service_manifest.json")).expect("service manifest");
        assert_eq!(
            service
                .route
                .as_ref()
                .map(|route| route.path_prefix.as_str()),
            Some("/")
        );
    }

    #[test]
    fn init_webapp_template_scaffolds_frontend_and_api() {
        let dir = temp_dir("webapp_template");
        let output = InitArgs {
            output_dir: dir.clone(),
            service_name: "agent_console".to_owned(),
            service_version: "1.0.0".to_owned(),
            template: InitTemplate::Webapp,
            overwrite: false,
        }
        .run()
        .expect("webapp init should succeed");

        assert_eq!(output.template, "webapp");
        assert!(dir.join("webapp/frontend/package.json").exists());
        assert!(dir.join("webapp/api/server.mjs").exists());

        let api = fs::read_to_string(dir.join("webapp/api/server.mjs")).expect("read api file");
        assert!(api.contains("verifyChainIdentity"));

        let service: SoraServiceManifestV1 =
            load_json(&dir.join("service_manifest.json")).expect("service manifest");
        assert_eq!(
            service
                .route
                .as_ref()
                .map(|route| route.path_prefix.as_str()),
            Some("/api")
        );
        assert!(
            service
                .state_bindings
                .iter()
                .any(|binding| binding.key_prefix == "/state/session")
        );
    }
}
