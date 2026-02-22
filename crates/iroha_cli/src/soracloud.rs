//! Soracloud deployment helpers (`init/deploy/status/upgrade/rollback/rollout`).
//!
//! These commands provide a deterministic local control-plane simulation for
//! Soracloud manifests. They validate `SoraDeploymentBundleV1` admission rules
//! and maintain a machine-readable registry/audit log file that can be used by
//! scripts and CI checks. `init` also supports Vue3 scaffolding templates for
//! static sites, dynamic webapps, and regulated health workloads. Live Torii
//! mode also exposes model-training and weight-lifecycle control-plane helpers.

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
        AgentApartmentManifestV1, AgentUpgradePolicyV1, SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        SORA_STATE_BINDING_VERSION_V1, SoraContainerManifestV1, SoraContainerRuntimeV1,
        SoraDeploymentBundleV1, SoraNetworkPolicyV1, SoraRouteTargetV1, SoraRouteVisibilityV1,
        SoraServiceManifestV1, SoraStateBindingV1, SoraStateEncryptionV1, SoraStateMutabilityV1,
        SoraStateScopeV1, SoraTlsModeV1, encode_agent_artifact_allow_provenance_payload,
        encode_agent_autonomy_run_provenance_payload, encode_agent_deploy_provenance_payload,
        encode_agent_lease_renew_provenance_payload, encode_agent_message_ack_provenance_payload,
        encode_agent_message_send_provenance_payload,
        encode_agent_policy_revoke_provenance_payload, encode_agent_restart_provenance_payload,
        encode_agent_wallet_approve_provenance_payload,
        encode_agent_wallet_spend_provenance_payload, encode_bundle_provenance_payload,
        encode_model_artifact_register_provenance_payload,
        encode_model_weight_promote_provenance_payload,
        encode_model_weight_register_provenance_payload,
        encode_model_weight_rollback_provenance_payload, encode_rollback_provenance_payload,
        encode_rollout_provenance_payload, encode_training_job_checkpoint_provenance_payload,
        encode_training_job_retry_provenance_payload, encode_training_job_start_provenance_payload,
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
const DEFAULT_AGENT_APARTMENT_MANIFEST: &str =
    "fixtures/soracloud/agent_apartment_manifest_v1.json";
const DEFAULT_REGISTRY_PATH: &str = ".soracloud/registry.json";
const REGISTRY_SCHEMA_VERSION: u16 = 1;
const AGENT_WALLET_DAY_TICKS: u64 = 10_000;
const AGENT_MAILBOX_MAX_PAYLOAD_BYTES: usize = 8 * 1024;
const AGENT_AUTONOMY_DEFAULT_BUDGET_UNITS: u64 = 10_000;
const AGENT_AUTONOMY_MAX_LABEL_BYTES: usize = 256;
const AGENT_AUTONOMY_MAX_HASH_BYTES: usize = 256;

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
    /// Advance or fail a rollout step using health-gated canary controls.
    Rollout(RolloutArgs),
    /// Register a persistent AI apartment manifest into local scheduler state.
    AgentDeploy(AgentDeployArgs),
    /// Renew an apartment lease in local scheduler state.
    AgentLeaseRenew(AgentLeaseRenewArgs),
    /// Request deterministic apartment restart in local scheduler state.
    AgentRestart(AgentRestartArgs),
    /// Show local apartment scheduler status.
    AgentStatus(AgentStatusArgs),
    /// Submit an apartment wallet spend request under policy guardrails.
    AgentWalletSpend(AgentWalletSpendArgs),
    /// Approve a pending apartment wallet spend request.
    AgentWalletApprove(AgentWalletApproveArgs),
    /// Revoke an apartment policy capability.
    AgentPolicyRevoke(AgentPolicyRevokeArgs),
    /// Send a deterministic mailbox message between apartments.
    AgentMessageSend(AgentMessageSendArgs),
    /// Acknowledge (consume) a mailbox message from an apartment queue.
    AgentMessageAck(AgentMessageAckArgs),
    /// Inspect mailbox queue state for an apartment.
    AgentMailboxStatus(AgentMailboxStatusArgs),
    /// Add an artifact hash (and optional provenance hash) to autonomy allowlist.
    AgentArtifactAllow(AgentArtifactAllowArgs),
    /// Approve an autonomous run under allowlist/provenance/budget guardrails.
    AgentAutonomyRun(AgentAutonomyRunArgs),
    /// Show autonomous-run policy state for an apartment.
    AgentAutonomyStatus(AgentAutonomyStatusArgs),
    /// Start a distributed training job in live Torii control-plane mode.
    TrainingJobStart(TrainingJobStartArgs),
    /// Record a training checkpoint in live Torii control-plane mode.
    TrainingJobCheckpoint(TrainingJobCheckpointArgs),
    /// Submit a training retry request in live Torii control-plane mode.
    TrainingJobRetry(TrainingJobRetryArgs),
    /// Query training job status in live Torii control-plane mode.
    TrainingJobStatus(TrainingJobStatusArgs),
    /// Register model-artifact metadata in live Torii control-plane mode.
    ModelArtifactRegister(ModelArtifactRegisterArgs),
    /// Query model-artifact status in live Torii control-plane mode.
    ModelArtifactStatus(ModelArtifactStatusArgs),
    /// Register a model weight version in live Torii control-plane mode.
    ModelWeightRegister(ModelWeightRegisterArgs),
    /// Promote a model weight version in live Torii control-plane mode.
    ModelWeightPromote(ModelWeightPromoteArgs),
    /// Roll back a model weight version in live Torii control-plane mode.
    ModelWeightRollback(ModelWeightRollbackArgs),
    /// Query model weight status in live Torii control-plane mode.
    ModelWeightStatus(ModelWeightStatusArgs),
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
            Command::Rollout(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::AgentDeploy(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::AgentLeaseRenew(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::AgentRestart(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::AgentStatus(args) => context.print_data(&args.run()?),
            Command::AgentWalletSpend(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::AgentWalletApprove(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::AgentPolicyRevoke(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::AgentMessageSend(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::AgentMessageAck(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::AgentMailboxStatus(args) => context.print_data(&args.run()?),
            Command::AgentArtifactAllow(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::AgentAutonomyRun(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::AgentAutonomyStatus(args) => context.print_data(&args.run()?),
            Command::TrainingJobStart(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::TrainingJobCheckpoint(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::TrainingJobRetry(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::TrainingJobStatus(args) => context.print_data(&args.run()?),
            Command::ModelArtifactRegister(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::ModelArtifactStatus(args) => context.print_data(&args.run()?),
            Command::ModelWeightRegister(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::ModelWeightPromote(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::ModelWeightRollback(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Command::ModelWeightStatus(args) => context.print_data(&args.run()?),
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
    /// Generate a private health-app starter with consent + retention workflows.
    HealthApp,
}

impl InitTemplate {
    fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Site => "site",
            Self::Webapp => "webapp",
            Self::HealthApp => "health-app",
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

/// Arguments for `app soracloud rollout`.
#[derive(clap::Args, Debug)]
pub struct RolloutArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Service name with an active rollout.
    #[arg(long, value_name = "NAME")]
    service_name: String,
    /// Rollout handle emitted by `upgrade` output (`rollout_handle`).
    #[arg(long, value_name = "HANDLE")]
    rollout_handle: String,
    /// Health signal for this rollout step.
    #[arg(long, value_enum, default_value_t = RolloutHealth::Healthy)]
    health: RolloutHealth,
    /// Optional target traffic percentage for healthy promotions.
    #[arg(long, value_name = "PERCENT")]
    promote_to_percent: Option<u8>,
    /// Governance transaction hash linked to this rollout action.
    #[arg(long, value_name = "HASH")]
    governance_tx_hash: Hash,
    /// Optional Torii base URL to execute rollout against live control-plane APIs.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for Torii mutation requests.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl RolloutArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if let Some(torii_url) = self.torii_url.as_deref() {
            let request = signed_rollout_request(
                &self.service_name,
                &self.rollout_handle,
                self.health.is_healthy(),
                self.promote_to_percent,
                self.governance_tx_hash,
                key_pair,
            )?;
            let (_, payload) = post_torii_soracloud_mutation(
                torii_url,
                "v1/soracloud/rollout",
                &request,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let mut registry = load_registry(&self.registry)?;
        let output = apply_rollout(
            &mut registry,
            &self.service_name,
            &self.rollout_handle,
            self.health.is_healthy(),
            self.promote_to_percent,
            self.governance_tx_hash,
        )?;
        write_json(&self.registry, &registry)?;
        json::to_value(&output).wrap_err("failed to encode soracloud rollout output")
    }
}

/// Arguments for `app soracloud agent-deploy`.
#[derive(clap::Args, Debug)]
pub struct AgentDeployArgs {
    /// Path to an `AgentApartmentManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_AGENT_APARTMENT_MANIFEST)]
    manifest: PathBuf,
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Lease length, measured in deterministic control-plane sequence ticks.
    #[arg(long, value_name = "TICKS", default_value_t = 120)]
    lease_ticks: u64,
    /// Initial autonomy execution budget units.
    #[arg(long, value_name = "UNITS", default_value_t = AGENT_AUTONOMY_DEFAULT_BUDGET_UNITS)]
    autonomy_budget_units: u64,
    /// Optional Torii base URL; when provided, calls live `agent/deploy` instead of local registry simulation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutations.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl AgentDeployArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if self.lease_ticks == 0 {
            return Err(eyre!("--lease-ticks must be greater than zero"));
        }
        if self.autonomy_budget_units == 0 {
            return Err(eyre!("--autonomy-budget-units must be greater than zero"));
        }
        let manifest: AgentApartmentManifestV1 = load_json(&self.manifest)?;
        manifest.validate()?;

        if let Some(torii_url) = self.torii_url.as_deref() {
            let request = signed_agent_deploy_request(
                manifest,
                self.lease_ticks,
                self.autonomy_budget_units,
                key_pair,
            )?;
            let (_, payload) = post_torii_soracloud_mutation(
                torii_url,
                "v1/soracloud/agent/deploy",
                &request,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let mut registry = load_registry(&self.registry)?;
        let output = apply_agent_deploy_with_budget(
            &mut registry,
            &manifest,
            self.lease_ticks,
            self.autonomy_budget_units,
        )?;
        write_json(&self.registry, &registry)?;
        json::to_value(&output).wrap_err("failed to encode soracloud agent deploy output")
    }
}

/// Arguments for `app soracloud agent-lease-renew`.
#[derive(clap::Args, Debug)]
pub struct AgentLeaseRenewArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Apartment name to renew.
    #[arg(long, value_name = "NAME")]
    apartment_name: String,
    /// Lease extension ticks.
    #[arg(long, value_name = "TICKS", default_value_t = 120)]
    lease_ticks: u64,
    /// Optional Torii base URL; when provided, calls live `agent/lease/renew` instead of local registry simulation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutations.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl AgentLeaseRenewArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if self.lease_ticks == 0 {
            return Err(eyre!("--lease-ticks must be greater than zero"));
        }

        if let Some(torii_url) = self.torii_url.as_deref() {
            let request =
                signed_agent_lease_renew_request(&self.apartment_name, self.lease_ticks, key_pair)?;
            let (_, payload) = post_torii_soracloud_mutation(
                torii_url,
                "v1/soracloud/agent/lease/renew",
                &request,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let mut registry = load_registry(&self.registry)?;
        let output =
            apply_agent_lease_renew(&mut registry, &self.apartment_name, self.lease_ticks)?;
        write_json(&self.registry, &registry)?;
        json::to_value(&output).wrap_err("failed to encode soracloud agent lease renew output")
    }
}

/// Arguments for `app soracloud agent-restart`.
#[derive(clap::Args, Debug)]
pub struct AgentRestartArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Apartment name to restart.
    #[arg(long, value_name = "NAME")]
    apartment_name: String,
    /// Human-readable reason captured in scheduler events.
    #[arg(long, value_name = "TEXT")]
    reason: String,
    /// Optional Torii base URL; when provided, calls live `agent/restart` instead of local registry simulation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutations.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl AgentRestartArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if let Some(torii_url) = self.torii_url.as_deref() {
            let request =
                signed_agent_restart_request(&self.apartment_name, &self.reason, key_pair)?;
            let (_, payload) = post_torii_soracloud_mutation(
                torii_url,
                "v1/soracloud/agent/restart",
                &request,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let mut registry = load_registry(&self.registry)?;
        let output = apply_agent_restart(&mut registry, &self.apartment_name, &self.reason)?;
        write_json(&self.registry, &registry)?;
        json::to_value(&output).wrap_err("failed to encode soracloud agent restart output")
    }
}

/// Arguments for `app soracloud agent-status`.
#[derive(clap::Args, Debug)]
pub struct AgentStatusArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Optional apartment name filter.
    #[arg(long, value_name = "NAME")]
    apartment_name: Option<String>,
    /// Optional Torii base URL; when provided, queries live `agent/status` instead of local registry simulation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when querying live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane status query.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl AgentStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        if let Some(torii_url) = self.torii_url.as_deref() {
            let (_, payload) = fetch_torii_soracloud_agent_status(
                torii_url,
                self.apartment_name.as_deref(),
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let registry = load_registry(&self.registry)?;
        let mut apartments = Vec::new();
        for (apartment_name, entry) in &registry.apartments {
            if self
                .apartment_name
                .as_ref()
                .is_some_and(|needle| needle != apartment_name)
            {
                continue;
            }
            apartments.push(AgentApartmentStatusEntry::from_state(
                apartment_name,
                entry,
                registry.next_sequence,
            ));
        }
        let output = AgentStatusOutput {
            schema_version: registry.schema_version,
            apartment_count: u32::try_from(apartments.len()).unwrap_or(u32::MAX),
            event_count: u32::try_from(registry.apartment_events.len()).unwrap_or(u32::MAX),
            apartments,
        };
        json::to_value(&output).wrap_err("failed to encode soracloud agent status output")
    }
}

/// Arguments for `app soracloud agent-wallet-spend`.
#[derive(clap::Args, Debug)]
pub struct AgentWalletSpendArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Apartment name issuing the spend request.
    #[arg(long, value_name = "NAME")]
    apartment_name: String,
    /// Asset definition identifier (`definition#domain`).
    #[arg(long, value_name = "ASSET")]
    asset_definition: String,
    /// Spend amount in nanos.
    #[arg(long, value_name = "NANOS")]
    amount_nanos: u64,
    /// Optional Torii base URL; when provided, calls live `agent/wallet/spend` instead of local registry simulation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutations.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl AgentWalletSpendArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if self.amount_nanos == 0 {
            return Err(eyre!("--amount-nanos must be greater than zero"));
        }

        if let Some(torii_url) = self.torii_url.as_deref() {
            let request = signed_agent_wallet_spend_request(
                &self.apartment_name,
                &self.asset_definition,
                self.amount_nanos,
                key_pair,
            )?;
            let (_, payload) = post_torii_soracloud_mutation(
                torii_url,
                "v1/soracloud/agent/wallet/spend",
                &request,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let mut registry = load_registry(&self.registry)?;
        let output = apply_agent_wallet_spend(
            &mut registry,
            &self.apartment_name,
            &self.asset_definition,
            self.amount_nanos,
        )?;
        write_json(&self.registry, &registry)?;
        json::to_value(&output).wrap_err("failed to encode soracloud agent wallet spend output")
    }
}

/// Arguments for `app soracloud agent-wallet-approve`.
#[derive(clap::Args, Debug)]
pub struct AgentWalletApproveArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Apartment name owning the request.
    #[arg(long, value_name = "NAME")]
    apartment_name: String,
    /// Wallet request identifier emitted by `agent-wallet-spend`.
    #[arg(long, value_name = "REQUEST")]
    request_id: String,
    /// Optional Torii base URL; when provided, calls live `agent/wallet/approve` instead of local registry simulation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutations.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl AgentWalletApproveArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if let Some(torii_url) = self.torii_url.as_deref() {
            let request = signed_agent_wallet_approve_request(
                &self.apartment_name,
                &self.request_id,
                key_pair,
            )?;
            let (_, payload) = post_torii_soracloud_mutation(
                torii_url,
                "v1/soracloud/agent/wallet/approve",
                &request,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let mut registry = load_registry(&self.registry)?;
        let output =
            apply_agent_wallet_approve(&mut registry, &self.apartment_name, &self.request_id)?;
        write_json(&self.registry, &registry)?;
        json::to_value(&output).wrap_err("failed to encode soracloud agent wallet approve output")
    }
}

/// Arguments for `app soracloud agent-policy-revoke`.
#[derive(clap::Args, Debug)]
pub struct AgentPolicyRevokeArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Apartment name whose policy should be updated.
    #[arg(long, value_name = "NAME")]
    apartment_name: String,
    /// Capability identifier to revoke (for example `wallet.sign`).
    #[arg(long, value_name = "CAPABILITY")]
    capability: String,
    /// Optional reason included in audit events.
    #[arg(long, value_name = "TEXT")]
    reason: Option<String>,
    /// Optional Torii base URL; when provided, calls live `agent/policy/revoke` instead of local registry simulation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutations.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl AgentPolicyRevokeArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if let Some(torii_url) = self.torii_url.as_deref() {
            let request = signed_agent_policy_revoke_request(
                &self.apartment_name,
                &self.capability,
                self.reason.as_deref(),
                key_pair,
            )?;
            let (_, payload) = post_torii_soracloud_mutation(
                torii_url,
                "v1/soracloud/agent/policy/revoke",
                &request,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let mut registry = load_registry(&self.registry)?;
        let output = apply_agent_policy_revoke(
            &mut registry,
            &self.apartment_name,
            &self.capability,
            self.reason.as_deref(),
        )?;
        write_json(&self.registry, &registry)?;
        json::to_value(&output).wrap_err("failed to encode soracloud agent policy revoke output")
    }
}

/// Arguments for `app soracloud agent-message-send`.
#[derive(clap::Args, Debug)]
pub struct AgentMessageSendArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Sender apartment name.
    #[arg(long, value_name = "NAME")]
    from_apartment: String,
    /// Recipient apartment name.
    #[arg(long, value_name = "NAME")]
    to_apartment: String,
    /// Logical mailbox channel.
    #[arg(long, value_name = "CHANNEL", default_value = "default")]
    channel: String,
    /// Message payload (UTF-8 text).
    #[arg(long, value_name = "TEXT")]
    payload: String,
    /// Optional Torii base URL; when provided, calls live `agent/message/send` instead of local registry simulation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutations.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl AgentMessageSendArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if let Some(torii_url) = self.torii_url.as_deref() {
            let request = signed_agent_message_send_request(
                &self.from_apartment,
                &self.to_apartment,
                &self.channel,
                &self.payload,
                key_pair,
            )?;
            let (_, payload) = post_torii_soracloud_mutation(
                torii_url,
                "v1/soracloud/agent/message/send",
                &request,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let mut registry = load_registry(&self.registry)?;
        let output = apply_agent_message_send(
            &mut registry,
            &self.from_apartment,
            &self.to_apartment,
            &self.channel,
            &self.payload,
        )?;
        write_json(&self.registry, &registry)?;
        json::to_value(&output).wrap_err("failed to encode soracloud agent message send output")
    }
}

/// Arguments for `app soracloud agent-message-ack`.
#[derive(clap::Args, Debug)]
pub struct AgentMessageAckArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Apartment name consuming the message.
    #[arg(long, value_name = "NAME")]
    apartment_name: String,
    /// Message identifier emitted by `agent-message-send`.
    #[arg(long, value_name = "MESSAGE")]
    message_id: String,
    /// Optional Torii base URL; when provided, calls live `agent/message/ack` instead of local registry simulation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutations.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl AgentMessageAckArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if let Some(torii_url) = self.torii_url.as_deref() {
            let request =
                signed_agent_message_ack_request(&self.apartment_name, &self.message_id, key_pair)?;
            let (_, payload) = post_torii_soracloud_mutation(
                torii_url,
                "v1/soracloud/agent/message/ack",
                &request,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let mut registry = load_registry(&self.registry)?;
        let output =
            apply_agent_message_ack(&mut registry, &self.apartment_name, &self.message_id)?;
        write_json(&self.registry, &registry)?;
        json::to_value(&output).wrap_err("failed to encode soracloud agent message ack output")
    }
}

/// Arguments for `app soracloud agent-mailbox-status`.
#[derive(clap::Args, Debug)]
pub struct AgentMailboxStatusArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Apartment name to inspect.
    #[arg(long, value_name = "NAME")]
    apartment_name: String,
    /// Optional Torii base URL; when provided, queries live `agent/mailbox/status` instead of local registry simulation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when querying live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane status query.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl AgentMailboxStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        if let Some(torii_url) = self.torii_url.as_deref() {
            let (_, payload) = fetch_torii_soracloud_agent_mailbox_status(
                torii_url,
                &self.apartment_name,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let registry = load_registry(&self.registry)?;
        let apartment_name = self.apartment_name.trim();
        if apartment_name.is_empty() {
            return Err(eyre!("--apartment-name must not be empty"));
        }
        let runtime = registry
            .apartments
            .get(apartment_name)
            .ok_or_else(|| eyre!("apartment `{apartment_name}` not found in registry"))?;
        let messages = runtime
            .mailbox_queue
            .iter()
            .map(AgentMailboxMessageEntry::from_message)
            .collect::<Vec<_>>();
        let output = AgentMailboxStatusOutput {
            schema_version: registry.schema_version,
            apartment_name: apartment_name.to_owned(),
            status: runtime_status_for_sequence(runtime, registry.next_sequence),
            pending_message_count: u32::try_from(messages.len()).unwrap_or(u32::MAX),
            event_count: u32::try_from(registry.apartment_events.len()).unwrap_or(u32::MAX),
            messages,
        };
        json::to_value(&output).wrap_err("failed to encode soracloud agent mailbox status output")
    }
}

/// Arguments for `app soracloud agent-artifact-allow`.
#[derive(clap::Args, Debug)]
pub struct AgentArtifactAllowArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Apartment name whose allowlist should be updated.
    #[arg(long, value_name = "NAME")]
    apartment_name: String,
    /// Artifact hash identifier.
    #[arg(long, value_name = "HASH")]
    artifact_hash: String,
    /// Optional provenance hash required for this artifact.
    #[arg(long, value_name = "HASH")]
    provenance_hash: Option<String>,
    /// Optional Torii base URL; when provided, calls live `agent/autonomy/allow` instead of local registry simulation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutations.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl AgentArtifactAllowArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if let Some(torii_url) = self.torii_url.as_deref() {
            let request = signed_agent_artifact_allow_request(
                &self.apartment_name,
                &self.artifact_hash,
                self.provenance_hash.as_deref(),
                key_pair,
            )?;
            let (_, payload) = post_torii_soracloud_mutation(
                torii_url,
                "v1/soracloud/agent/autonomy/allow",
                &request,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let mut registry = load_registry(&self.registry)?;
        let output = apply_agent_artifact_allow(
            &mut registry,
            &self.apartment_name,
            &self.artifact_hash,
            self.provenance_hash.as_deref(),
        )?;
        write_json(&self.registry, &registry)?;
        json::to_value(&output).wrap_err("failed to encode soracloud agent autonomy allow output")
    }
}

/// Arguments for `app soracloud agent-autonomy-run`.
#[derive(clap::Args, Debug)]
pub struct AgentAutonomyRunArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Apartment name requesting autonomous execution.
    #[arg(long, value_name = "NAME")]
    apartment_name: String,
    /// Artifact hash identifier.
    #[arg(long, value_name = "HASH")]
    artifact_hash: String,
    /// Optional provenance hash for this run request.
    #[arg(long, value_name = "HASH")]
    provenance_hash: Option<String>,
    /// Budget units requested for this run.
    #[arg(long, value_name = "UNITS")]
    budget_units: u64,
    /// Human-readable run label.
    #[arg(long, value_name = "LABEL")]
    run_label: String,
    /// Optional Torii base URL; when provided, calls live `agent/autonomy/run` instead of local registry simulation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutations.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl AgentAutonomyRunArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if self.budget_units == 0 {
            return Err(eyre!("--budget-units must be greater than zero"));
        }

        if let Some(torii_url) = self.torii_url.as_deref() {
            let request = signed_agent_autonomy_run_request(
                &self.apartment_name,
                &self.artifact_hash,
                self.provenance_hash.as_deref(),
                self.budget_units,
                &self.run_label,
                key_pair,
            )?;
            let (_, payload) = post_torii_soracloud_mutation(
                torii_url,
                "v1/soracloud/agent/autonomy/run",
                &request,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let mut registry = load_registry(&self.registry)?;
        let output = apply_agent_autonomy_run(
            &mut registry,
            &self.apartment_name,
            &self.artifact_hash,
            self.provenance_hash.as_deref(),
            self.budget_units,
            &self.run_label,
        )?;
        write_json(&self.registry, &registry)?;
        json::to_value(&output).wrap_err("failed to encode soracloud agent autonomy run output")
    }
}

/// Arguments for `app soracloud agent-autonomy-status`.
#[derive(clap::Args, Debug)]
pub struct AgentAutonomyStatusArgs {
    /// Registry state JSON path.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_REGISTRY_PATH)]
    registry: PathBuf,
    /// Apartment name to inspect.
    #[arg(long, value_name = "NAME")]
    apartment_name: String,
    /// Optional Torii base URL; when provided, queries live `agent/autonomy/status` instead of local registry simulation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when querying Torii.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane query.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl AgentAutonomyStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        if let Some(torii_url) = self.torii_url.as_deref() {
            let (_, payload) = fetch_torii_soracloud_agent_autonomy_status(
                torii_url,
                &self.apartment_name,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            return Ok(payload);
        }

        let registry = load_registry(&self.registry)?;
        let apartment_name = self.apartment_name.trim();
        if apartment_name.is_empty() {
            return Err(eyre!("--apartment-name must not be empty"));
        }
        let runtime = registry
            .apartments
            .get(apartment_name)
            .ok_or_else(|| eyre!("apartment `{apartment_name}` not found in registry"))?;
        let allowlist = runtime
            .artifact_allowlist
            .values()
            .map(AgentAutonomyAllowlistEntry::from_rule)
            .collect::<Vec<_>>();
        let recent_runs = runtime
            .autonomy_run_history
            .iter()
            .rev()
            .take(20)
            .cloned()
            .collect::<Vec<_>>();
        let output = AgentAutonomyStatusOutput {
            schema_version: registry.schema_version,
            apartment_name: apartment_name.to_owned(),
            status: runtime_status_for_sequence(runtime, registry.next_sequence),
            budget_ceiling_units: runtime.autonomy_budget_ceiling_units,
            budget_remaining_units: runtime.autonomy_budget_remaining_units,
            allowlist_count: u32::try_from(runtime.artifact_allowlist.len()).unwrap_or(u32::MAX),
            run_count: u32::try_from(runtime.autonomy_run_history.len()).unwrap_or(u32::MAX),
            event_count: u32::try_from(registry.apartment_events.len()).unwrap_or(u32::MAX),
            allowlist,
            recent_runs,
        };
        json::to_value(&output).wrap_err("failed to encode soracloud agent autonomy status output")
    }
}

/// Arguments for `app soracloud training-job-start`.
#[derive(clap::Args, Debug)]
pub struct TrainingJobStartArgs {
    /// Service name that owns the training job.
    #[arg(long, value_name = "NAME")]
    service_name: String,
    /// Model name for the training job.
    #[arg(long, value_name = "NAME")]
    model_name: String,
    /// Deterministic training job identifier.
    #[arg(long, value_name = "ID")]
    job_id: String,
    /// Worker-group size for the distributed training run.
    #[arg(long, value_name = "COUNT", default_value_t = 1)]
    worker_group_size: u16,
    /// Target number of steps to complete the training job.
    #[arg(long, value_name = "STEPS")]
    target_steps: u32,
    /// Step cadence for checkpoint creation.
    #[arg(long, value_name = "STEPS")]
    checkpoint_interval_steps: u32,
    /// Maximum allowed retries for the training job.
    #[arg(long, value_name = "COUNT", default_value_t = 3)]
    max_retries: u8,
    /// Compute units charged per step.
    #[arg(long, value_name = "UNITS")]
    step_compute_units: u64,
    /// Total compute budget units for the training job.
    #[arg(long, value_name = "UNITS")]
    compute_budget_units: u64,
    /// Total storage budget bytes for checkpoints.
    #[arg(long, value_name = "BYTES")]
    storage_budget_bytes: u64,
    /// Torii base URL for live control-plane mutation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token`.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutation.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl TrainingJobStartArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if self.worker_group_size == 0 {
            return Err(eyre!("--worker-group-size must be greater than zero"));
        }
        if self.target_steps == 0 {
            return Err(eyre!("--target-steps must be greater than zero"));
        }
        if self.checkpoint_interval_steps == 0 {
            return Err(eyre!(
                "--checkpoint-interval-steps must be greater than zero"
            ));
        }
        if self.step_compute_units == 0 {
            return Err(eyre!("--step-compute-units must be greater than zero"));
        }
        if self.compute_budget_units == 0 {
            return Err(eyre!("--compute-budget-units must be greater than zero"));
        }
        if self.storage_budget_bytes == 0 {
            return Err(eyre!("--storage-budget-bytes must be greater than zero"));
        }
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_training_job_start_request(
            &self.service_name,
            &self.model_name,
            &self.job_id,
            self.worker_group_size,
            self.target_steps,
            self.checkpoint_interval_steps,
            self.max_retries,
            self.step_compute_units,
            self.compute_budget_units,
            self.storage_budget_bytes,
            key_pair,
        )?;
        let (_, payload) = post_torii_soracloud_mutation(
            torii_url,
            "v1/soracloud/training/job/start",
            &request,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        Ok(payload)
    }
}

/// Arguments for `app soracloud training-job-checkpoint`.
#[derive(clap::Args, Debug)]
pub struct TrainingJobCheckpointArgs {
    /// Service name that owns the training job.
    #[arg(long, value_name = "NAME")]
    service_name: String,
    /// Training job identifier.
    #[arg(long, value_name = "ID")]
    job_id: String,
    /// Completed step represented by this checkpoint.
    #[arg(long, value_name = "STEP")]
    completed_step: u32,
    /// Checkpoint payload size in bytes.
    #[arg(long, value_name = "BYTES")]
    checkpoint_size_bytes: u64,
    /// Hash of metrics/telemetry emitted for this checkpoint.
    #[arg(long, value_name = "HASH")]
    metrics_hash: Hash,
    /// Torii base URL for live control-plane mutation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token`.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutation.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl TrainingJobCheckpointArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if self.completed_step == 0 {
            return Err(eyre!("--completed-step must be greater than zero"));
        }
        if self.checkpoint_size_bytes == 0 {
            return Err(eyre!("--checkpoint-size-bytes must be greater than zero"));
        }
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_training_job_checkpoint_request(
            &self.service_name,
            &self.job_id,
            self.completed_step,
            self.checkpoint_size_bytes,
            self.metrics_hash,
            key_pair,
        )?;
        let (_, payload) = post_torii_soracloud_mutation(
            torii_url,
            "v1/soracloud/training/job/checkpoint",
            &request,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        Ok(payload)
    }
}

/// Arguments for `app soracloud training-job-retry`.
#[derive(clap::Args, Debug)]
pub struct TrainingJobRetryArgs {
    /// Service name that owns the training job.
    #[arg(long, value_name = "NAME")]
    service_name: String,
    /// Training job identifier.
    #[arg(long, value_name = "ID")]
    job_id: String,
    /// Human-readable retry reason recorded in audit logs.
    #[arg(long, value_name = "TEXT")]
    reason: String,
    /// Torii base URL for live control-plane mutation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token`.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutation.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl TrainingJobRetryArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_training_job_retry_request(
            &self.service_name,
            &self.job_id,
            &self.reason,
            key_pair,
        )?;
        let (_, payload) = post_torii_soracloud_mutation(
            torii_url,
            "v1/soracloud/training/job/retry",
            &request,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        Ok(payload)
    }
}

/// Arguments for `app soracloud training-job-status`.
#[derive(clap::Args, Debug)]
pub struct TrainingJobStatusArgs {
    /// Service name that owns the training job.
    #[arg(long, value_name = "NAME")]
    service_name: String,
    /// Training job identifier.
    #[arg(long, value_name = "ID")]
    job_id: String,
    /// Torii base URL for live control-plane query.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token`.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane query.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl TrainingJobStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, payload) = fetch_torii_soracloud_training_job_status(
            torii_url,
            &self.service_name,
            &self.job_id,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        Ok(payload)
    }
}

/// Arguments for `app soracloud model-artifact-register`.
#[derive(clap::Args, Debug)]
pub struct ModelArtifactRegisterArgs {
    /// Service name that owns the model.
    #[arg(long, value_name = "NAME")]
    service_name: String,
    /// Model name.
    #[arg(long, value_name = "NAME")]
    model_name: String,
    /// Training job identifier backing this artifact registration.
    #[arg(long, value_name = "ID")]
    training_job_id: String,
    /// Weight artifact hash.
    #[arg(long, value_name = "HASH")]
    weight_artifact_hash: Hash,
    /// Dataset reference identifier.
    #[arg(long, value_name = "REF")]
    dataset_ref: String,
    /// Hash of training config used for the run.
    #[arg(long, value_name = "HASH")]
    training_config_hash: Hash,
    /// Reproducibility metadata hash.
    #[arg(long, value_name = "HASH")]
    reproducibility_hash: Hash,
    /// Provenance attestation hash.
    #[arg(long, value_name = "HASH")]
    provenance_attestation_hash: Hash,
    /// Torii base URL for live control-plane mutation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token`.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutation.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl ModelArtifactRegisterArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_model_artifact_register_request(
            &self.service_name,
            &self.model_name,
            &self.training_job_id,
            self.weight_artifact_hash,
            &self.dataset_ref,
            self.training_config_hash,
            self.reproducibility_hash,
            self.provenance_attestation_hash,
            key_pair,
        )?;
        let (_, payload) = post_torii_soracloud_mutation(
            torii_url,
            "v1/soracloud/model/artifact/register",
            &request,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        Ok(payload)
    }
}

/// Arguments for `app soracloud model-artifact-status`.
#[derive(clap::Args, Debug)]
pub struct ModelArtifactStatusArgs {
    /// Service name that owns the model artifact.
    #[arg(long, value_name = "NAME")]
    service_name: String,
    /// Training job identifier associated with the artifact.
    #[arg(long, value_name = "ID")]
    training_job_id: String,
    /// Torii base URL for live control-plane query.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token`.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane query.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl ModelArtifactStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, payload) = fetch_torii_soracloud_model_artifact_status(
            torii_url,
            &self.service_name,
            &self.training_job_id,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        Ok(payload)
    }
}

/// Arguments for `app soracloud model-weight-register`.
#[derive(clap::Args, Debug)]
pub struct ModelWeightRegisterArgs {
    /// Service name that owns the model.
    #[arg(long, value_name = "NAME")]
    service_name: String,
    /// Model name.
    #[arg(long, value_name = "NAME")]
    model_name: String,
    /// New weight version identifier.
    #[arg(long, value_name = "VERSION")]
    weight_version: String,
    /// Training job identifier backing this weight version.
    #[arg(long, value_name = "ID")]
    training_job_id: String,
    /// Optional lineage parent version.
    #[arg(long, value_name = "VERSION")]
    parent_version: Option<String>,
    /// Weight artifact hash.
    #[arg(long, value_name = "HASH")]
    weight_artifact_hash: Hash,
    /// Dataset reference identifier.
    #[arg(long, value_name = "REF")]
    dataset_ref: String,
    /// Hash of training config used for the run.
    #[arg(long, value_name = "HASH")]
    training_config_hash: Hash,
    /// Reproducibility metadata hash.
    #[arg(long, value_name = "HASH")]
    reproducibility_hash: Hash,
    /// Provenance attestation hash.
    #[arg(long, value_name = "HASH")]
    provenance_attestation_hash: Hash,
    /// Torii base URL for live control-plane mutation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token`.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutation.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl ModelWeightRegisterArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_model_weight_register_request(
            &self.service_name,
            &self.model_name,
            &self.weight_version,
            &self.training_job_id,
            self.parent_version.as_deref(),
            self.weight_artifact_hash,
            &self.dataset_ref,
            self.training_config_hash,
            self.reproducibility_hash,
            self.provenance_attestation_hash,
            key_pair,
        )?;
        let (_, payload) = post_torii_soracloud_mutation(
            torii_url,
            "v1/soracloud/model/weight/register",
            &request,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        Ok(payload)
    }
}

/// Arguments for `app soracloud model-weight-promote`.
#[derive(clap::Args, Debug)]
pub struct ModelWeightPromoteArgs {
    /// Service name that owns the model.
    #[arg(long, value_name = "NAME")]
    service_name: String,
    /// Model name.
    #[arg(long, value_name = "NAME")]
    model_name: String,
    /// Weight version to promote.
    #[arg(long, value_name = "VERSION")]
    weight_version: String,
    /// Gate approval flag.
    #[arg(long)]
    gate_approved: bool,
    /// Hash of gate report/evidence for this promotion decision.
    #[arg(long, value_name = "HASH")]
    gate_report_hash: Hash,
    /// Torii base URL for live control-plane mutation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token`.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutation.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl ModelWeightPromoteArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_model_weight_promote_request(
            &self.service_name,
            &self.model_name,
            &self.weight_version,
            self.gate_approved,
            self.gate_report_hash,
            key_pair,
        )?;
        let (_, payload) = post_torii_soracloud_mutation(
            torii_url,
            "v1/soracloud/model/weight/promote",
            &request,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        Ok(payload)
    }
}

/// Arguments for `app soracloud model-weight-rollback`.
#[derive(clap::Args, Debug)]
pub struct ModelWeightRollbackArgs {
    /// Service name that owns the model.
    #[arg(long, value_name = "NAME")]
    service_name: String,
    /// Model name.
    #[arg(long, value_name = "NAME")]
    model_name: String,
    /// Target version to roll back to.
    #[arg(long, value_name = "VERSION")]
    target_version: String,
    /// Human-readable rollback reason.
    #[arg(long, value_name = "TEXT")]
    reason: String,
    /// Torii base URL for live control-plane mutation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token`.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane mutation.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl ModelWeightRollbackArgs {
    fn run(self, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_model_weight_rollback_request(
            &self.service_name,
            &self.model_name,
            &self.target_version,
            &self.reason,
            key_pair,
        )?;
        let (_, payload) = post_torii_soracloud_mutation(
            torii_url,
            "v1/soracloud/model/weight/rollback",
            &request,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        Ok(payload)
    }
}

/// Arguments for `app soracloud model-weight-status`.
#[derive(clap::Args, Debug)]
pub struct ModelWeightStatusArgs {
    /// Service name that owns the model.
    #[arg(long, value_name = "NAME")]
    service_name: String,
    /// Model name.
    #[arg(long, value_name = "NAME")]
    model_name: String,
    /// Torii base URL for live control-plane query.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token`.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for live control-plane query.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}

impl ModelWeightStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, payload) = fetch_torii_soracloud_model_weight_status(
            torii_url,
            &self.service_name,
            &self.model_name,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        Ok(payload)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MutationMode {
    Deploy,
    Upgrade,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Default)]
enum RolloutHealth {
    #[default]
    Healthy,
    Unhealthy,
}

impl RolloutHealth {
    fn is_healthy(self) -> bool {
        matches!(self, Self::Healthy)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "action", content = "value")]
enum SoracloudAction {
    Deploy,
    Upgrade,
    Rollback,
    Rollout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "stage", content = "value")]
enum RolloutStage {
    Canary,
    Promoted,
    RolledBack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "status", content = "value")]
enum AgentRuntimeStatus {
    Running,
    LeaseExpired,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct RolloutRuntimeState {
    rollout_handle: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    baseline_version: Option<String>,
    candidate_version: String,
    canary_percent: u8,
    traffic_percent: u8,
    stage: RolloutStage,
    health_failures: u32,
    max_health_failures: u32,
    health_window_secs: u32,
    created_sequence: u64,
    updated_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct RegistryState {
    schema_version: u16,
    next_sequence: u64,
    #[norito(default)]
    services: BTreeMap<String, RegistryServiceEntry>,
    #[norito(default)]
    audit_log: Vec<RegistryAuditEvent>,
    #[norito(default)]
    apartments: BTreeMap<String, AgentApartmentRuntimeState>,
    #[norito(default)]
    apartment_events: Vec<AgentApartmentEvent>,
}

impl Default for RegistryState {
    fn default() -> Self {
        Self {
            schema_version: REGISTRY_SCHEMA_VERSION,
            next_sequence: 1,
            services: BTreeMap::new(),
            audit_log: Vec::new(),
            apartments: BTreeMap::new(),
            apartment_events: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct RegistryServiceEntry {
    service_name: String,
    current_version: String,
    #[norito(default)]
    revisions: Vec<RegistryServiceRevision>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    active_rollout: Option<RolloutRuntimeState>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    last_rollout: Option<RolloutRuntimeState>,
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
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    governance_tx_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    rollout_handle: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "action", content = "value")]
enum AgentApartmentAction {
    Deploy,
    LeaseRenew,
    Restart,
    WalletSpendRequested,
    WalletSpendApproved,
    PolicyRevoked,
    MessageEnqueued,
    MessageAcknowledged,
    ArtifactAllowed,
    AutonomyRunApproved,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentWalletSpendRequest {
    request_id: String,
    asset_definition: String,
    amount_nanos: u64,
    created_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentWalletDailySpendEntry {
    asset_definition: String,
    day_bucket: u64,
    spent_nanos: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentMailboxMessage {
    message_id: String,
    from_apartment: String,
    channel: String,
    payload: String,
    payload_hash: Hash,
    enqueued_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentArtifactAllowRule {
    artifact_hash: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    provenance_hash: Option<String>,
    added_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentAutonomyRunRecord {
    run_id: String,
    artifact_hash: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    provenance_hash: Option<String>,
    budget_units: u64,
    run_label: String,
    approved_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentApartmentRuntimeState {
    manifest: AgentApartmentManifestV1,
    manifest_hash: Hash,
    status: AgentRuntimeStatus,
    deployed_sequence: u64,
    lease_started_sequence: u64,
    lease_expires_sequence: u64,
    last_renewed_sequence: u64,
    restart_count: u32,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    last_restart_sequence: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    last_restart_reason: Option<String>,
    #[norito(default)]
    revoked_policy_capabilities: Vec<String>,
    #[norito(default)]
    pending_wallet_requests: BTreeMap<String, AgentWalletSpendRequest>,
    #[norito(default)]
    wallet_daily_spend: BTreeMap<String, AgentWalletDailySpendEntry>,
    #[norito(default)]
    mailbox_queue: Vec<AgentMailboxMessage>,
    #[norito(default)]
    autonomy_budget_ceiling_units: u64,
    #[norito(default)]
    autonomy_budget_remaining_units: u64,
    #[norito(default)]
    artifact_allowlist: BTreeMap<String, AgentArtifactAllowRule>,
    #[norito(default)]
    autonomy_run_history: Vec<AgentAutonomyRunRecord>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentApartmentEvent {
    sequence: u64,
    action: AgentApartmentAction,
    apartment_name: String,
    status: AgentRuntimeStatus,
    lease_expires_sequence: u64,
    manifest_hash: Hash,
    restart_count: u32,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    asset_definition: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    amount_nanos: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    capability: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    from_apartment: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    to_apartment: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    channel: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    payload_hash: Option<Hash>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    artifact_hash: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    provenance_hash: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    run_label: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    budget_units: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    budget_remaining_units: Option<u64>,
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
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    rollout_handle: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    rollout_stage: Option<RolloutStage>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    rollout_percent: Option<u8>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct RolloutOutput {
    action: SoracloudAction,
    service_name: String,
    rollout_handle: String,
    stage: RolloutStage,
    current_version: String,
    traffic_percent: u8,
    health_failures: u32,
    max_health_failures: u32,
    sequence: u64,
    governance_tx_hash: Hash,
    audit_event_count: u32,
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
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    active_rollout: Option<RolloutRuntimeState>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    last_rollout: Option<RolloutRuntimeState>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentMutationOutput {
    action: AgentApartmentAction,
    apartment_name: String,
    sequence: u64,
    manifest_hash: Hash,
    status: AgentRuntimeStatus,
    lease_expires_sequence: u64,
    lease_remaining_ticks: u64,
    restart_count: u32,
    revoked_policy_capability_count: u32,
    pending_wallet_request_count: u32,
    event_count: u32,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    asset_definition: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    amount_nanos: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    day_bucket: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    day_spent_nanos: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    capability: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentWalletMutationOutput {
    action: AgentApartmentAction,
    apartment_name: String,
    sequence: u64,
    manifest_hash: Hash,
    status: AgentRuntimeStatus,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    asset_definition: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    amount_nanos: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    day_bucket: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    day_spent_nanos: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    capability: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    pending_request_count: u32,
    revoked_policy_capability_count: u32,
    event_count: u32,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentMailboxMutationOutput {
    action: AgentApartmentAction,
    apartment_name: String,
    sequence: u64,
    message_id: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    from_apartment: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    to_apartment: Option<String>,
    channel: String,
    payload_hash: Hash,
    status: AgentRuntimeStatus,
    pending_message_count: u32,
    event_count: u32,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentMailboxStatusOutput {
    schema_version: u16,
    apartment_name: String,
    status: AgentRuntimeStatus,
    pending_message_count: u32,
    event_count: u32,
    messages: Vec<AgentMailboxMessageEntry>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentMailboxMessageEntry {
    message_id: String,
    from_apartment: String,
    channel: String,
    payload: String,
    payload_hash: Hash,
    enqueued_sequence: u64,
}

impl AgentMailboxMessageEntry {
    fn from_message(message: &AgentMailboxMessage) -> Self {
        Self {
            message_id: message.message_id.clone(),
            from_apartment: message.from_apartment.clone(),
            channel: message.channel.clone(),
            payload: message.payload.clone(),
            payload_hash: message.payload_hash,
            enqueued_sequence: message.enqueued_sequence,
        }
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentAutonomyMutationOutput {
    action: AgentApartmentAction,
    apartment_name: String,
    sequence: u64,
    artifact_hash: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    provenance_hash: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    run_id: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    run_label: Option<String>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    budget_units: Option<u64>,
    status: AgentRuntimeStatus,
    budget_remaining_units: u64,
    allowlist_count: u32,
    run_count: u32,
    event_count: u32,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentAutonomyStatusOutput {
    schema_version: u16,
    apartment_name: String,
    status: AgentRuntimeStatus,
    budget_ceiling_units: u64,
    budget_remaining_units: u64,
    allowlist_count: u32,
    run_count: u32,
    event_count: u32,
    allowlist: Vec<AgentAutonomyAllowlistEntry>,
    recent_runs: Vec<AgentAutonomyRunRecord>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentAutonomyAllowlistEntry {
    artifact_hash: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    provenance_hash: Option<String>,
    added_sequence: u64,
}

impl AgentAutonomyAllowlistEntry {
    fn from_rule(rule: &AgentArtifactAllowRule) -> Self {
        Self {
            artifact_hash: rule.artifact_hash.clone(),
            provenance_hash: rule.provenance_hash.clone(),
            added_sequence: rule.added_sequence,
        }
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentStatusOutput {
    schema_version: u16,
    apartment_count: u32,
    event_count: u32,
    apartments: Vec<AgentApartmentStatusEntry>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct AgentApartmentStatusEntry {
    apartment_name: String,
    manifest_hash: Hash,
    status: AgentRuntimeStatus,
    lease_started_sequence: u64,
    lease_expires_sequence: u64,
    lease_remaining_ticks: u64,
    restart_count: u32,
    state_quota_bytes: u64,
    tool_capability_count: u32,
    policy_capability_count: u32,
    revoked_policy_capability_count: u32,
    pending_wallet_request_count: u32,
    pending_mailbox_message_count: u32,
    autonomy_budget_ceiling_units: u64,
    autonomy_budget_remaining_units: u64,
    artifact_allowlist_count: u32,
    autonomy_run_count: u32,
    spend_limit_count: u32,
    upgrade_policy: AgentUpgradePolicyV1,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    last_restart_sequence: Option<u64>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    last_restart_reason: Option<String>,
}

impl AgentApartmentStatusEntry {
    fn from_state(
        apartment_name: &str,
        state: &AgentApartmentRuntimeState,
        current_sequence: u64,
    ) -> Self {
        Self {
            apartment_name: apartment_name.to_owned(),
            manifest_hash: state.manifest_hash,
            status: runtime_status_for_sequence(state, current_sequence),
            lease_started_sequence: state.lease_started_sequence,
            lease_expires_sequence: state.lease_expires_sequence,
            lease_remaining_ticks: lease_remaining_ticks(state, current_sequence),
            restart_count: state.restart_count,
            state_quota_bytes: state.manifest.state_quota_bytes.get(),
            tool_capability_count: u32::try_from(state.manifest.tool_capabilities.len())
                .unwrap_or(u32::MAX),
            policy_capability_count: u32::try_from(state.manifest.policy_capabilities.len())
                .unwrap_or(u32::MAX),
            revoked_policy_capability_count: u32::try_from(state.revoked_policy_capabilities.len())
                .unwrap_or(u32::MAX),
            pending_wallet_request_count: u32::try_from(state.pending_wallet_requests.len())
                .unwrap_or(u32::MAX),
            pending_mailbox_message_count: u32::try_from(state.mailbox_queue.len())
                .unwrap_or(u32::MAX),
            autonomy_budget_ceiling_units: state.autonomy_budget_ceiling_units,
            autonomy_budget_remaining_units: state.autonomy_budget_remaining_units,
            artifact_allowlist_count: u32::try_from(state.artifact_allowlist.len())
                .unwrap_or(u32::MAX),
            autonomy_run_count: u32::try_from(state.autonomy_run_history.len()).unwrap_or(u32::MAX),
            spend_limit_count: u32::try_from(state.manifest.spend_limits.len()).unwrap_or(u32::MAX),
            upgrade_policy: state.manifest.upgrade_policy,
            last_restart_sequence: state.last_restart_sequence,
            last_restart_reason: state.last_restart_reason.clone(),
        }
    }
}

impl ServiceStatusOutput {
    fn from_entry(service_name: &str, entry: &RegistryServiceEntry) -> Self {
        Self {
            service_name: service_name.to_string(),
            current_version: entry.current_version.clone(),
            revision_count: u32::try_from(entry.revisions.len()).unwrap_or(u32::MAX),
            latest_revision: entry.revisions.last().cloned(),
            active_rollout: entry.active_rollout.clone(),
            last_rollout: entry.last_rollout.clone(),
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

#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
struct RolloutAdvancePayload {
    service_name: String,
    rollout_handle: String,
    healthy: bool,
    #[norito(default)]
    promote_to_percent: Option<u8>,
    governance_tx_hash: Hash,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedRolloutAdvanceRequest {
    payload: RolloutAdvancePayload,
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
struct AgentDeployPayload {
    manifest: AgentApartmentManifestV1,
    lease_ticks: u64,
    #[norito(default)]
    autonomy_budget_units: Option<u64>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedAgentDeployRequest {
    payload: AgentDeployPayload,
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
struct AgentLeaseRenewPayload {
    apartment_name: String,
    lease_ticks: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedAgentLeaseRenewRequest {
    payload: AgentLeaseRenewPayload,
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
struct AgentRestartPayload {
    apartment_name: String,
    reason: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedAgentRestartRequest {
    payload: AgentRestartPayload,
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
struct AgentPolicyRevokePayload {
    apartment_name: String,
    capability: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedAgentPolicyRevokeRequest {
    payload: AgentPolicyRevokePayload,
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
struct AgentWalletSpendPayload {
    apartment_name: String,
    asset_definition: String,
    amount_nanos: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedAgentWalletSpendRequest {
    payload: AgentWalletSpendPayload,
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
struct AgentWalletApprovePayload {
    apartment_name: String,
    request_id: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedAgentWalletApproveRequest {
    payload: AgentWalletApprovePayload,
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
struct AgentMessageSendPayload {
    from_apartment: String,
    to_apartment: String,
    channel: String,
    payload: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedAgentMessageSendRequest {
    payload: AgentMessageSendPayload,
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
struct AgentMessageAckPayload {
    apartment_name: String,
    message_id: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedAgentMessageAckRequest {
    payload: AgentMessageAckPayload,
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
struct AgentArtifactAllowPayload {
    apartment_name: String,
    artifact_hash: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    provenance_hash: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedAgentArtifactAllowRequest {
    payload: AgentArtifactAllowPayload,
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
struct AgentAutonomyRunPayload {
    apartment_name: String,
    artifact_hash: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    provenance_hash: Option<String>,
    budget_units: u64,
    run_label: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedAgentAutonomyRunRequest {
    payload: AgentAutonomyRunPayload,
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
struct TrainingJobStartPayload {
    service_name: String,
    model_name: String,
    job_id: String,
    worker_group_size: u16,
    target_steps: u32,
    checkpoint_interval_steps: u32,
    max_retries: u8,
    step_compute_units: u64,
    compute_budget_units: u64,
    storage_budget_bytes: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedTrainingJobStartRequest {
    payload: TrainingJobStartPayload,
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
struct TrainingJobCheckpointPayload {
    service_name: String,
    job_id: String,
    completed_step: u32,
    checkpoint_size_bytes: u64,
    metrics_hash: Hash,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedTrainingJobCheckpointRequest {
    payload: TrainingJobCheckpointPayload,
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
struct TrainingJobRetryPayload {
    service_name: String,
    job_id: String,
    reason: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedTrainingJobRetryRequest {
    payload: TrainingJobRetryPayload,
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
struct ModelArtifactRegisterPayload {
    service_name: String,
    model_name: String,
    training_job_id: String,
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedModelArtifactRegisterRequest {
    payload: ModelArtifactRegisterPayload,
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
struct ModelWeightRegisterPayload {
    service_name: String,
    model_name: String,
    weight_version: String,
    training_job_id: String,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    parent_version: Option<String>,
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedModelWeightRegisterRequest {
    payload: ModelWeightRegisterPayload,
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
struct ModelWeightPromotePayload {
    service_name: String,
    model_name: String,
    weight_version: String,
    gate_approved: bool,
    gate_report_hash: Hash,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedModelWeightPromoteRequest {
    payload: ModelWeightPromotePayload,
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
struct ModelWeightRollbackPayload {
    service_name: String,
    model_name: String,
    target_version: String,
    reason: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct SignedModelWeightRollbackRequest {
    payload: ModelWeightRollbackPayload,
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

    let mut response_rollout_handle = None;
    let mut response_rollout_stage = None;
    let mut response_rollout_percent = None;
    let entry = registry
        .services
        .entry(service_name.clone())
        .or_insert_with(|| RegistryServiceEntry {
            service_name: service_name.clone(),
            current_version: service_version.clone(),
            revisions: Vec::new(),
            active_rollout: None,
            last_rollout: None,
        });
    entry.current_version = service_version.clone();
    entry.revisions.push(revision);
    if action == SoracloudAction::Upgrade {
        let canary_percent = bundle.service.rollout.canary_percent.min(100);
        let traffic_percent = if canary_percent == 0 {
            100
        } else {
            canary_percent
        };
        let rollout_state = RolloutRuntimeState {
            rollout_handle: rollout_handle(&service_name, next_sequence),
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
            max_health_failures: bundle.service.rollout.automatic_rollback_failures.get(),
            health_window_secs: bundle.service.rollout.health_window_secs.get(),
            created_sequence: next_sequence,
            updated_sequence: next_sequence,
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

    registry.audit_log.push(RegistryAuditEvent {
        sequence: next_sequence,
        action,
        service_name: service_name.clone(),
        from_version: previous_version.clone(),
        to_version: service_version.clone(),
        service_manifest_hash,
        container_manifest_hash,
        governance_tx_hash: None,
        rollout_handle: response_rollout_handle.clone(),
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
        rollout_handle: response_rollout_handle,
        rollout_stage: response_rollout_stage,
        rollout_percent: response_rollout_percent,
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
    entry.active_rollout = None;
    entry.last_rollout = None;

    registry.audit_log.push(RegistryAuditEvent {
        sequence,
        action: SoracloudAction::Rollback,
        service_name: service_name.to_string(),
        from_version: previous_version.clone(),
        to_version: target.service_version.clone(),
        service_manifest_hash: target.service_manifest_hash,
        container_manifest_hash: target.container_manifest_hash,
        governance_tx_hash: None,
        rollout_handle: None,
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
        rollout_handle: None,
        rollout_stage: None,
        rollout_percent: None,
    })
}

fn apply_rollout(
    registry: &mut RegistryState,
    service_name: &str,
    rollout_handle: &str,
    healthy: bool,
    promote_to_percent: Option<u8>,
    governance_tx_hash: Hash,
) -> Result<RolloutOutput> {
    ensure_registry_schema(registry)?;
    if service_name.trim().is_empty() {
        return Err(eyre!("--service-name must not be empty"));
    }
    if rollout_handle.trim().is_empty() {
        return Err(eyre!("--rollout-handle must not be empty"));
    }
    if promote_to_percent.is_some_and(|value| value > 100) {
        return Err(eyre!("--promote-to-percent must be within 0..=100"));
    }

    let sequence = registry.next_sequence;
    let (mut response, audit_event) = {
        let entry = registry
            .services
            .get_mut(service_name)
            .ok_or_else(|| eyre!("service `{service_name}` not found in registry"))?;

        let mut rollout = entry
            .active_rollout
            .clone()
            .ok_or_else(|| eyre!("service `{service_name}` has no active rollout to advance"))?;
        if rollout.rollout_handle != rollout_handle {
            return Err(eyre!(
                "service `{service_name}` active rollout handle mismatch (expected `{}`)",
                rollout.rollout_handle
            ));
        }

        if healthy {
            let promote_to = promote_to_percent.unwrap_or(100);
            if promote_to < rollout.traffic_percent {
                return Err(eyre!(
                    "rollout traffic cannot decrease from {} to {promote_to}",
                    rollout.traffic_percent
                ));
            }
            if promote_to < rollout.canary_percent {
                return Err(eyre!(
                    "rollout traffic cannot be below canary_percent {}",
                    rollout.canary_percent
                ));
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
            let current_revision = entry
                .revisions
                .last()
                .cloned()
                .ok_or_else(|| eyre!("service `{service_name}` has no active revision"))?;
            let audit_event = RegistryAuditEvent {
                sequence,
                action: SoracloudAction::Rollout,
                service_name: service_name.to_string(),
                from_version: Some(current_version.clone()),
                to_version: current_version.clone(),
                service_manifest_hash: current_revision.service_manifest_hash,
                container_manifest_hash: current_revision.container_manifest_hash,
                governance_tx_hash: Some(governance_tx_hash),
                rollout_handle: Some(rollout_handle.to_string()),
            };
            let response = RolloutOutput {
                action: SoracloudAction::Rollout,
                service_name: service_name.to_string(),
                rollout_handle: rollout_handle.to_string(),
                stage: rollout.stage,
                current_version,
                traffic_percent: rollout.traffic_percent,
                health_failures: rollout.health_failures,
                max_health_failures: rollout.max_health_failures,
                sequence,
                governance_tx_hash,
                audit_event_count: 0,
            };
            (response, audit_event)
        } else {
            rollout.health_failures = rollout.health_failures.saturating_add(1);
            rollout.updated_sequence = sequence;

            if rollout.health_failures >= rollout.max_health_failures {
                let baseline_version = rollout.baseline_version.clone().ok_or_else(|| {
                    eyre!("service `{service_name}` has no baseline version for auto rollback")
                })?;
                let previous_version = entry.current_version.clone();
                let target = entry
                    .revisions
                    .iter()
                    .rev()
                    .find(|revision| revision.service_version == baseline_version)
                    .cloned()
                    .ok_or_else(|| {
                        eyre!(
                            "service `{service_name}` missing baseline revision `{baseline_version}`"
                        )
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
                };
                entry.current_version = target.service_version.clone();
                entry.revisions.push(rollback_revision);

                rollout.stage = RolloutStage::RolledBack;
                rollout.traffic_percent = 0;
                entry.active_rollout = None;
                entry.last_rollout = Some(rollout.clone());

                let audit_event = RegistryAuditEvent {
                    sequence,
                    action: SoracloudAction::Rollback,
                    service_name: service_name.to_string(),
                    from_version: Some(previous_version),
                    to_version: target.service_version.clone(),
                    service_manifest_hash: target.service_manifest_hash,
                    container_manifest_hash: target.container_manifest_hash,
                    governance_tx_hash: Some(governance_tx_hash),
                    rollout_handle: Some(rollout_handle.to_string()),
                };
                let response = RolloutOutput {
                    action: SoracloudAction::Rollback,
                    service_name: service_name.to_string(),
                    rollout_handle: rollout_handle.to_string(),
                    stage: rollout.stage,
                    current_version: target.service_version,
                    traffic_percent: rollout.traffic_percent,
                    health_failures: rollout.health_failures,
                    max_health_failures: rollout.max_health_failures,
                    sequence,
                    governance_tx_hash,
                    audit_event_count: 0,
                };
                (response, audit_event)
            } else {
                entry.active_rollout = Some(rollout.clone());
                entry.last_rollout = Some(rollout.clone());

                let current_version = entry.current_version.clone();
                let current_revision = entry
                    .revisions
                    .last()
                    .cloned()
                    .ok_or_else(|| eyre!("service `{service_name}` has no active revision"))?;
                let audit_event = RegistryAuditEvent {
                    sequence,
                    action: SoracloudAction::Rollout,
                    service_name: service_name.to_string(),
                    from_version: Some(current_version.clone()),
                    to_version: current_version.clone(),
                    service_manifest_hash: current_revision.service_manifest_hash,
                    container_manifest_hash: current_revision.container_manifest_hash,
                    governance_tx_hash: Some(governance_tx_hash),
                    rollout_handle: Some(rollout_handle.to_string()),
                };
                let response = RolloutOutput {
                    action: SoracloudAction::Rollout,
                    service_name: service_name.to_string(),
                    rollout_handle: rollout_handle.to_string(),
                    stage: rollout.stage,
                    current_version,
                    traffic_percent: rollout.traffic_percent,
                    health_failures: rollout.health_failures,
                    max_health_failures: rollout.max_health_failures,
                    sequence,
                    governance_tx_hash,
                    audit_event_count: 0,
                };
                (response, audit_event)
            }
        }
    };

    registry.audit_log.push(audit_event);
    registry.next_sequence = registry.next_sequence.saturating_add(1);
    response.audit_event_count = u32::try_from(registry.audit_log.len()).unwrap_or(u32::MAX);
    Ok(response)
}

#[cfg(test)]
fn apply_agent_deploy(
    registry: &mut RegistryState,
    manifest: &AgentApartmentManifestV1,
    lease_ticks: u64,
) -> Result<AgentMutationOutput> {
    apply_agent_deploy_with_budget(
        registry,
        manifest,
        lease_ticks,
        AGENT_AUTONOMY_DEFAULT_BUDGET_UNITS,
    )
}

fn apply_agent_deploy_with_budget(
    registry: &mut RegistryState,
    manifest: &AgentApartmentManifestV1,
    lease_ticks: u64,
    autonomy_budget_units: u64,
) -> Result<AgentMutationOutput> {
    ensure_registry_schema(registry)?;
    manifest.validate()?;
    if lease_ticks == 0 {
        return Err(eyre!("--lease-ticks must be greater than zero"));
    }
    if autonomy_budget_units == 0 {
        return Err(eyre!("autonomy budget units must be greater than zero"));
    }

    let apartment_name = manifest.apartment_name.to_string();
    validate_apartment_name(&apartment_name)?;
    if registry.apartments.contains_key(&apartment_name) {
        return Err(eyre!(
            "apartment `{apartment_name}` already registered; use lease-renew/restart for lifecycle actions"
        ));
    }

    let sequence = registry.next_sequence;
    let manifest_hash = Hash::new(Encode::encode(manifest));
    let runtime_state = AgentApartmentRuntimeState {
        manifest: manifest.clone(),
        manifest_hash,
        status: AgentRuntimeStatus::Running,
        deployed_sequence: sequence,
        lease_started_sequence: sequence,
        lease_expires_sequence: sequence.saturating_add(lease_ticks),
        last_renewed_sequence: sequence,
        restart_count: 0,
        last_restart_sequence: None,
        last_restart_reason: None,
        revoked_policy_capabilities: Vec::new(),
        pending_wallet_requests: BTreeMap::new(),
        wallet_daily_spend: BTreeMap::new(),
        mailbox_queue: Vec::new(),
        autonomy_budget_ceiling_units: autonomy_budget_units,
        autonomy_budget_remaining_units: autonomy_budget_units,
        artifact_allowlist: BTreeMap::new(),
        autonomy_run_history: Vec::new(),
    };
    let output = AgentMutationOutput {
        action: AgentApartmentAction::Deploy,
        apartment_name: apartment_name.clone(),
        sequence,
        manifest_hash,
        status: runtime_status_for_sequence(&runtime_state, sequence.saturating_add(1)),
        lease_expires_sequence: runtime_state.lease_expires_sequence,
        lease_remaining_ticks: lease_remaining_ticks(&runtime_state, sequence.saturating_add(1)),
        restart_count: runtime_state.restart_count,
        revoked_policy_capability_count: 0,
        pending_wallet_request_count: 0,
        event_count: 0,
        reason: None,
        request_id: None,
        asset_definition: None,
        amount_nanos: None,
        day_bucket: None,
        day_spent_nanos: None,
        capability: None,
    };
    registry
        .apartments
        .insert(apartment_name.clone(), runtime_state);
    registry.apartment_events.push(AgentApartmentEvent {
        sequence,
        action: AgentApartmentAction::Deploy,
        apartment_name,
        status: AgentRuntimeStatus::Running,
        lease_expires_sequence: output.lease_expires_sequence,
        manifest_hash,
        restart_count: output.restart_count,
        reason: None,
        request_id: None,
        asset_definition: None,
        amount_nanos: None,
        capability: None,
        from_apartment: None,
        to_apartment: None,
        channel: None,
        payload_hash: None,
        artifact_hash: None,
        provenance_hash: None,
        run_label: None,
        budget_units: None,
        budget_remaining_units: None,
    });
    registry.next_sequence = registry.next_sequence.saturating_add(1);

    Ok(AgentMutationOutput {
        event_count: u32::try_from(registry.apartment_events.len()).unwrap_or(u32::MAX),
        ..output
    })
}

fn apply_agent_lease_renew(
    registry: &mut RegistryState,
    apartment_name: &str,
    lease_ticks: u64,
) -> Result<AgentMutationOutput> {
    ensure_registry_schema(registry)?;
    validate_apartment_name(apartment_name)?;
    if lease_ticks == 0 {
        return Err(eyre!("--lease-ticks must be greater than zero"));
    }

    let sequence = registry.next_sequence;
    let output = {
        let entry = registry
            .apartments
            .get_mut(apartment_name)
            .ok_or_else(|| eyre!("apartment `{apartment_name}` not found in registry"))?;
        let base = entry.lease_expires_sequence.max(sequence);
        entry.lease_expires_sequence = base.saturating_add(lease_ticks);
        entry.last_renewed_sequence = sequence;
        entry.status = AgentRuntimeStatus::Running;
        AgentMutationOutput {
            action: AgentApartmentAction::LeaseRenew,
            apartment_name: apartment_name.to_owned(),
            sequence,
            manifest_hash: entry.manifest_hash,
            status: runtime_status_for_sequence(entry, sequence.saturating_add(1)),
            lease_expires_sequence: entry.lease_expires_sequence,
            lease_remaining_ticks: lease_remaining_ticks(entry, sequence.saturating_add(1)),
            restart_count: entry.restart_count,
            revoked_policy_capability_count: revoked_policy_capability_count(entry),
            pending_wallet_request_count: pending_wallet_request_count(entry),
            event_count: 0,
            reason: None,
            request_id: None,
            asset_definition: None,
            amount_nanos: None,
            day_bucket: None,
            day_spent_nanos: None,
            capability: None,
        }
    };
    registry.apartment_events.push(AgentApartmentEvent {
        sequence,
        action: AgentApartmentAction::LeaseRenew,
        apartment_name: apartment_name.to_owned(),
        status: output.status,
        lease_expires_sequence: output.lease_expires_sequence,
        manifest_hash: output.manifest_hash,
        restart_count: output.restart_count,
        reason: None,
        request_id: None,
        asset_definition: None,
        amount_nanos: None,
        capability: None,
        from_apartment: None,
        to_apartment: None,
        channel: None,
        payload_hash: None,
        artifact_hash: None,
        provenance_hash: None,
        run_label: None,
        budget_units: None,
        budget_remaining_units: None,
    });
    registry.next_sequence = registry.next_sequence.saturating_add(1);

    Ok(AgentMutationOutput {
        event_count: u32::try_from(registry.apartment_events.len()).unwrap_or(u32::MAX),
        ..output
    })
}

fn apply_agent_restart(
    registry: &mut RegistryState,
    apartment_name: &str,
    reason: &str,
) -> Result<AgentMutationOutput> {
    ensure_registry_schema(registry)?;
    validate_apartment_name(apartment_name)?;
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(eyre!("--reason must not be empty"));
    }

    let sequence = registry.next_sequence;
    let output = {
        let entry = registry
            .apartments
            .get_mut(apartment_name)
            .ok_or_else(|| eyre!("apartment `{apartment_name}` not found in registry"))?;
        if runtime_status_for_sequence(entry, sequence) == AgentRuntimeStatus::LeaseExpired {
            return Err(eyre!(
                "apartment `{apartment_name}` lease expired at sequence {}; renew before restart",
                entry.lease_expires_sequence
            ));
        }

        entry.status = AgentRuntimeStatus::Running;
        entry.restart_count = entry.restart_count.saturating_add(1);
        entry.last_restart_sequence = Some(sequence);
        entry.last_restart_reason = Some(reason.to_owned());
        AgentMutationOutput {
            action: AgentApartmentAction::Restart,
            apartment_name: apartment_name.to_owned(),
            sequence,
            manifest_hash: entry.manifest_hash,
            status: runtime_status_for_sequence(entry, sequence.saturating_add(1)),
            lease_expires_sequence: entry.lease_expires_sequence,
            lease_remaining_ticks: lease_remaining_ticks(entry, sequence.saturating_add(1)),
            restart_count: entry.restart_count,
            revoked_policy_capability_count: revoked_policy_capability_count(entry),
            pending_wallet_request_count: pending_wallet_request_count(entry),
            event_count: 0,
            reason: Some(reason.to_owned()),
            request_id: None,
            asset_definition: None,
            amount_nanos: None,
            day_bucket: None,
            day_spent_nanos: None,
            capability: None,
        }
    };
    registry.apartment_events.push(AgentApartmentEvent {
        sequence,
        action: AgentApartmentAction::Restart,
        apartment_name: apartment_name.to_owned(),
        status: output.status,
        lease_expires_sequence: output.lease_expires_sequence,
        manifest_hash: output.manifest_hash,
        restart_count: output.restart_count,
        reason: Some(reason.to_owned()),
        request_id: None,
        asset_definition: None,
        amount_nanos: None,
        capability: None,
        from_apartment: None,
        to_apartment: None,
        channel: None,
        payload_hash: None,
        artifact_hash: None,
        provenance_hash: None,
        run_label: None,
        budget_units: None,
        budget_remaining_units: None,
    });
    registry.next_sequence = registry.next_sequence.saturating_add(1);

    Ok(AgentMutationOutput {
        event_count: u32::try_from(registry.apartment_events.len()).unwrap_or(u32::MAX),
        ..output
    })
}

fn apply_agent_wallet_spend(
    registry: &mut RegistryState,
    apartment_name: &str,
    asset_definition: &str,
    amount_nanos: u64,
) -> Result<AgentWalletMutationOutput> {
    ensure_registry_schema(registry)?;
    validate_apartment_name(apartment_name)?;
    let asset_definition = asset_definition.trim();
    if asset_definition.is_empty() {
        return Err(eyre!("--asset-definition must not be empty"));
    }
    if amount_nanos == 0 {
        return Err(eyre!("--amount-nanos must be greater than zero"));
    }

    let sequence = registry.next_sequence;
    let output = {
        let entry = registry
            .apartments
            .get_mut(apartment_name)
            .ok_or_else(|| eyre!("apartment `{apartment_name}` not found in registry"))?;
        if runtime_status_for_sequence(entry, sequence) == AgentRuntimeStatus::LeaseExpired {
            return Err(eyre!(
                "apartment `{apartment_name}` lease expired at sequence {}; renew before wallet actions",
                entry.lease_expires_sequence
            ));
        }
        if !policy_capability_active(entry, "wallet.sign") {
            return Err(eyre!(
                "apartment `{apartment_name}` does not have active `wallet.sign` capability"
            ));
        }
        let spend_limit = entry
            .manifest
            .spend_limits
            .iter()
            .find(|limit| limit.asset_definition == asset_definition)
            .ok_or_else(|| {
                eyre!(
                    "apartment `{apartment_name}` has no spend limit configured for asset `{asset_definition}`"
                )
            })?;
        if amount_nanos > spend_limit.max_per_tx_nanos.get() {
            return Err(eyre!(
                "requested amount {} exceeds max_per_tx_nanos {} for asset `{asset_definition}`",
                amount_nanos,
                spend_limit.max_per_tx_nanos.get()
            ));
        }

        let day_bucket = wallet_day_bucket(sequence);
        let current_day_spent = wallet_day_spent(entry, asset_definition, day_bucket);
        let projected_day_spent = current_day_spent
            .checked_add(amount_nanos)
            .ok_or_else(|| eyre!("wallet daily spend overflow for apartment `{apartment_name}`"))?;
        if projected_day_spent > spend_limit.max_per_day_nanos.get() {
            return Err(eyre!(
                "projected daily spend {} exceeds max_per_day_nanos {} for asset `{asset_definition}`",
                projected_day_spent,
                spend_limit.max_per_day_nanos.get()
            ));
        }

        let request_id = format!("{apartment_name}:wallet:{sequence}");
        let action = if policy_capability_active(entry, "wallet.auto_approve") {
            wallet_record_spend(entry, asset_definition, day_bucket, projected_day_spent);
            AgentApartmentAction::WalletSpendApproved
        } else {
            entry.pending_wallet_requests.insert(
                request_id.clone(),
                AgentWalletSpendRequest {
                    request_id: request_id.clone(),
                    asset_definition: asset_definition.to_owned(),
                    amount_nanos,
                    created_sequence: sequence,
                },
            );
            AgentApartmentAction::WalletSpendRequested
        };

        let day_spent_nanos = wallet_day_spent(entry, asset_definition, day_bucket);
        AgentWalletMutationOutput {
            action,
            apartment_name: apartment_name.to_owned(),
            sequence,
            manifest_hash: entry.manifest_hash,
            status: runtime_status_for_sequence(entry, sequence.saturating_add(1)),
            request_id: Some(request_id),
            asset_definition: Some(asset_definition.to_owned()),
            amount_nanos: Some(amount_nanos),
            day_bucket: Some(day_bucket),
            day_spent_nanos: Some(day_spent_nanos),
            capability: None,
            reason: None,
            pending_request_count: pending_wallet_request_count(entry),
            revoked_policy_capability_count: revoked_policy_capability_count(entry),
            event_count: 0,
        }
    };
    registry.apartment_events.push(AgentApartmentEvent {
        sequence,
        action: output.action,
        apartment_name: apartment_name.to_owned(),
        status: output.status,
        lease_expires_sequence: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.lease_expires_sequence)
            .unwrap_or(sequence),
        manifest_hash: output.manifest_hash,
        restart_count: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.restart_count)
            .unwrap_or(0),
        reason: None,
        request_id: output.request_id.clone(),
        asset_definition: output.asset_definition.clone(),
        amount_nanos: output.amount_nanos,
        capability: None,
        from_apartment: None,
        to_apartment: None,
        channel: None,
        payload_hash: None,
        artifact_hash: None,
        provenance_hash: None,
        run_label: None,
        budget_units: None,
        budget_remaining_units: None,
    });
    registry.next_sequence = registry.next_sequence.saturating_add(1);

    Ok(AgentWalletMutationOutput {
        event_count: u32::try_from(registry.apartment_events.len()).unwrap_or(u32::MAX),
        ..output
    })
}

fn apply_agent_wallet_approve(
    registry: &mut RegistryState,
    apartment_name: &str,
    request_id: &str,
) -> Result<AgentWalletMutationOutput> {
    ensure_registry_schema(registry)?;
    validate_apartment_name(apartment_name)?;
    let request_id = request_id.trim();
    if request_id.is_empty() {
        return Err(eyre!("--request-id must not be empty"));
    }

    let sequence = registry.next_sequence;
    let output = {
        let entry = registry
            .apartments
            .get_mut(apartment_name)
            .ok_or_else(|| eyre!("apartment `{apartment_name}` not found in registry"))?;
        if runtime_status_for_sequence(entry, sequence) == AgentRuntimeStatus::LeaseExpired {
            return Err(eyre!(
                "apartment `{apartment_name}` lease expired at sequence {}; renew before wallet actions",
                entry.lease_expires_sequence
            ));
        }
        if !policy_capability_active(entry, "wallet.sign") {
            return Err(eyre!(
                "apartment `{apartment_name}` does not have active `wallet.sign` capability"
            ));
        }

        let request = entry
            .pending_wallet_requests
            .remove(request_id)
            .ok_or_else(|| {
                eyre!("wallet request `{request_id}` not found for apartment `{apartment_name}`")
            })?;

        let spend_limit = entry
            .manifest
            .spend_limits
            .iter()
            .find(|limit| limit.asset_definition == request.asset_definition)
            .ok_or_else(|| {
                eyre!(
                    "apartment `{apartment_name}` has no spend limit configured for asset `{}`",
                    request.asset_definition
                )
            })?;
        let day_bucket = wallet_day_bucket(sequence);
        let current_day_spent = wallet_day_spent(entry, &request.asset_definition, day_bucket);
        let projected_day_spent = current_day_spent
            .checked_add(request.amount_nanos)
            .ok_or_else(|| eyre!("wallet daily spend overflow for apartment `{apartment_name}`"))?;
        if projected_day_spent > spend_limit.max_per_day_nanos.get() {
            return Err(eyre!(
                "projected daily spend {} exceeds max_per_day_nanos {} for asset `{}`",
                projected_day_spent,
                spend_limit.max_per_day_nanos.get(),
                request.asset_definition
            ));
        }
        wallet_record_spend(
            entry,
            &request.asset_definition,
            day_bucket,
            projected_day_spent,
        );
        AgentWalletMutationOutput {
            action: AgentApartmentAction::WalletSpendApproved,
            apartment_name: apartment_name.to_owned(),
            sequence,
            manifest_hash: entry.manifest_hash,
            status: runtime_status_for_sequence(entry, sequence.saturating_add(1)),
            request_id: Some(request.request_id),
            asset_definition: Some(request.asset_definition),
            amount_nanos: Some(request.amount_nanos),
            day_bucket: Some(day_bucket),
            day_spent_nanos: Some(projected_day_spent),
            capability: None,
            reason: None,
            pending_request_count: pending_wallet_request_count(entry),
            revoked_policy_capability_count: revoked_policy_capability_count(entry),
            event_count: 0,
        }
    };
    registry.apartment_events.push(AgentApartmentEvent {
        sequence,
        action: AgentApartmentAction::WalletSpendApproved,
        apartment_name: apartment_name.to_owned(),
        status: output.status,
        lease_expires_sequence: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.lease_expires_sequence)
            .unwrap_or(sequence),
        manifest_hash: output.manifest_hash,
        restart_count: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.restart_count)
            .unwrap_or(0),
        reason: None,
        request_id: output.request_id.clone(),
        asset_definition: output.asset_definition.clone(),
        amount_nanos: output.amount_nanos,
        capability: None,
        from_apartment: None,
        to_apartment: None,
        channel: None,
        payload_hash: None,
        artifact_hash: None,
        provenance_hash: None,
        run_label: None,
        budget_units: None,
        budget_remaining_units: None,
    });
    registry.next_sequence = registry.next_sequence.saturating_add(1);

    Ok(AgentWalletMutationOutput {
        event_count: u32::try_from(registry.apartment_events.len()).unwrap_or(u32::MAX),
        ..output
    })
}

fn apply_agent_policy_revoke(
    registry: &mut RegistryState,
    apartment_name: &str,
    capability: &str,
    reason: Option<&str>,
) -> Result<AgentWalletMutationOutput> {
    ensure_registry_schema(registry)?;
    validate_apartment_name(apartment_name)?;
    let capability = capability.trim();
    if capability.is_empty() {
        return Err(eyre!("--capability must not be empty"));
    }

    let sequence = registry.next_sequence;
    let output = {
        let entry = registry
            .apartments
            .get_mut(apartment_name)
            .ok_or_else(|| eyre!("apartment `{apartment_name}` not found in registry"))?;
        let capability_present = entry
            .manifest
            .policy_capabilities
            .iter()
            .any(|candidate| candidate.as_ref() == capability);
        if !capability_present {
            return Err(eyre!(
                "apartment `{apartment_name}` does not declare policy capability `{capability}`"
            ));
        }
        if entry
            .revoked_policy_capabilities
            .iter()
            .any(|candidate| candidate == capability)
        {
            return Err(eyre!(
                "policy capability `{capability}` already revoked for apartment `{apartment_name}`"
            ));
        }
        entry
            .revoked_policy_capabilities
            .push(capability.to_owned());
        entry.revoked_policy_capabilities.sort();
        AgentWalletMutationOutput {
            action: AgentApartmentAction::PolicyRevoked,
            apartment_name: apartment_name.to_owned(),
            sequence,
            manifest_hash: entry.manifest_hash,
            status: runtime_status_for_sequence(entry, sequence.saturating_add(1)),
            request_id: None,
            asset_definition: None,
            amount_nanos: None,
            day_bucket: None,
            day_spent_nanos: None,
            capability: Some(capability.to_owned()),
            reason: reason.map(str::to_owned),
            pending_request_count: pending_wallet_request_count(entry),
            revoked_policy_capability_count: revoked_policy_capability_count(entry),
            event_count: 0,
        }
    };
    registry.apartment_events.push(AgentApartmentEvent {
        sequence,
        action: AgentApartmentAction::PolicyRevoked,
        apartment_name: apartment_name.to_owned(),
        status: output.status,
        lease_expires_sequence: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.lease_expires_sequence)
            .unwrap_or(sequence),
        manifest_hash: output.manifest_hash,
        restart_count: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.restart_count)
            .unwrap_or(0),
        reason: output.reason.clone(),
        request_id: None,
        asset_definition: None,
        amount_nanos: None,
        capability: output.capability.clone(),
        from_apartment: None,
        to_apartment: None,
        channel: None,
        payload_hash: None,
        artifact_hash: None,
        provenance_hash: None,
        run_label: None,
        budget_units: None,
        budget_remaining_units: None,
    });
    registry.next_sequence = registry.next_sequence.saturating_add(1);

    Ok(AgentWalletMutationOutput {
        event_count: u32::try_from(registry.apartment_events.len()).unwrap_or(u32::MAX),
        ..output
    })
}

fn apply_agent_message_send(
    registry: &mut RegistryState,
    from_apartment: &str,
    to_apartment: &str,
    channel: &str,
    payload: &str,
) -> Result<AgentMailboxMutationOutput> {
    ensure_registry_schema(registry)?;
    validate_apartment_name(from_apartment)?;
    validate_apartment_name(to_apartment)?;
    let channel = channel.trim();
    if channel.is_empty() {
        return Err(eyre!("--channel must not be empty"));
    }
    let payload = payload.trim();
    if payload.is_empty() {
        return Err(eyre!("--payload must not be empty"));
    }
    if payload.len() > AGENT_MAILBOX_MAX_PAYLOAD_BYTES {
        return Err(eyre!(
            "--payload exceeds max mailbox payload bytes ({AGENT_MAILBOX_MAX_PAYLOAD_BYTES})"
        ));
    }

    let sequence = registry.next_sequence;
    let sender = registry
        .apartments
        .get(from_apartment)
        .ok_or_else(|| eyre!("apartment `{from_apartment}` not found in registry"))?;
    if runtime_status_for_sequence(sender, sequence) == AgentRuntimeStatus::LeaseExpired {
        return Err(eyre!(
            "sender apartment `{from_apartment}` lease expired at sequence {}; renew before messaging",
            sender.lease_expires_sequence
        ));
    }
    if !policy_capability_active(sender, "agent.mailbox.send") {
        return Err(eyre!(
            "apartment `{from_apartment}` does not have active `agent.mailbox.send` capability"
        ));
    }

    let recipient_snapshot = registry
        .apartments
        .get(to_apartment)
        .ok_or_else(|| eyre!("apartment `{to_apartment}` not found in registry"))?;
    if runtime_status_for_sequence(recipient_snapshot, sequence) == AgentRuntimeStatus::LeaseExpired
    {
        return Err(eyre!(
            "recipient apartment `{to_apartment}` lease expired at sequence {}; renew before messaging",
            recipient_snapshot.lease_expires_sequence
        ));
    }
    if !policy_capability_active(recipient_snapshot, "agent.mailbox.receive") {
        return Err(eyre!(
            "apartment `{to_apartment}` does not have active `agent.mailbox.receive` capability"
        ));
    }

    let message_id = format!("{to_apartment}:mail:{sequence}");
    let payload_hash = Hash::new(payload.as_bytes());
    let output = {
        let recipient = registry
            .apartments
            .get_mut(to_apartment)
            .ok_or_else(|| eyre!("apartment `{to_apartment}` not found in registry"))?;
        recipient.mailbox_queue.push(AgentMailboxMessage {
            message_id: message_id.clone(),
            from_apartment: from_apartment.to_owned(),
            channel: channel.to_owned(),
            payload: payload.to_owned(),
            payload_hash,
            enqueued_sequence: sequence,
        });
        AgentMailboxMutationOutput {
            action: AgentApartmentAction::MessageEnqueued,
            apartment_name: to_apartment.to_owned(),
            sequence,
            message_id: message_id.clone(),
            from_apartment: Some(from_apartment.to_owned()),
            to_apartment: Some(to_apartment.to_owned()),
            channel: channel.to_owned(),
            payload_hash,
            status: runtime_status_for_sequence(recipient, sequence.saturating_add(1)),
            pending_message_count: pending_mailbox_message_count(recipient),
            event_count: 0,
        }
    };
    registry.apartment_events.push(AgentApartmentEvent {
        sequence,
        action: AgentApartmentAction::MessageEnqueued,
        apartment_name: to_apartment.to_owned(),
        status: output.status,
        lease_expires_sequence: registry
            .apartments
            .get(to_apartment)
            .map(|entry| entry.lease_expires_sequence)
            .unwrap_or(sequence),
        manifest_hash: registry
            .apartments
            .get(to_apartment)
            .map(|entry| entry.manifest_hash)
            .unwrap_or(payload_hash),
        restart_count: registry
            .apartments
            .get(to_apartment)
            .map(|entry| entry.restart_count)
            .unwrap_or(0),
        reason: None,
        request_id: Some(message_id),
        asset_definition: None,
        amount_nanos: None,
        capability: None,
        from_apartment: Some(from_apartment.to_owned()),
        to_apartment: Some(to_apartment.to_owned()),
        channel: Some(channel.to_owned()),
        payload_hash: Some(payload_hash),
        artifact_hash: None,
        provenance_hash: None,
        run_label: None,
        budget_units: None,
        budget_remaining_units: None,
    });
    registry.next_sequence = registry.next_sequence.saturating_add(1);

    Ok(AgentMailboxMutationOutput {
        event_count: u32::try_from(registry.apartment_events.len()).unwrap_or(u32::MAX),
        ..output
    })
}

fn apply_agent_message_ack(
    registry: &mut RegistryState,
    apartment_name: &str,
    message_id: &str,
) -> Result<AgentMailboxMutationOutput> {
    ensure_registry_schema(registry)?;
    validate_apartment_name(apartment_name)?;
    let message_id = message_id.trim();
    if message_id.is_empty() {
        return Err(eyre!("--message-id must not be empty"));
    }

    let sequence = registry.next_sequence;
    let output = {
        let recipient = registry
            .apartments
            .get_mut(apartment_name)
            .ok_or_else(|| eyre!("apartment `{apartment_name}` not found in registry"))?;
        if runtime_status_for_sequence(recipient, sequence) == AgentRuntimeStatus::LeaseExpired {
            return Err(eyre!(
                "apartment `{apartment_name}` lease expired at sequence {}; renew before mailbox actions",
                recipient.lease_expires_sequence
            ));
        }
        if !policy_capability_active(recipient, "agent.mailbox.receive") {
            return Err(eyre!(
                "apartment `{apartment_name}` does not have active `agent.mailbox.receive` capability"
            ));
        }
        let index = recipient
            .mailbox_queue
            .iter()
            .position(|message| message.message_id == message_id)
            .ok_or_else(|| {
                eyre!("mailbox message `{message_id}` not found for apartment `{apartment_name}`")
            })?;
        let message = recipient.mailbox_queue.remove(index);
        AgentMailboxMutationOutput {
            action: AgentApartmentAction::MessageAcknowledged,
            apartment_name: apartment_name.to_owned(),
            sequence,
            message_id: message.message_id,
            from_apartment: Some(message.from_apartment),
            to_apartment: Some(apartment_name.to_owned()),
            channel: message.channel,
            payload_hash: message.payload_hash,
            status: runtime_status_for_sequence(recipient, sequence.saturating_add(1)),
            pending_message_count: pending_mailbox_message_count(recipient),
            event_count: 0,
        }
    };

    registry.apartment_events.push(AgentApartmentEvent {
        sequence,
        action: AgentApartmentAction::MessageAcknowledged,
        apartment_name: apartment_name.to_owned(),
        status: output.status,
        lease_expires_sequence: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.lease_expires_sequence)
            .unwrap_or(sequence),
        manifest_hash: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.manifest_hash)
            .unwrap_or(output.payload_hash),
        restart_count: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.restart_count)
            .unwrap_or(0),
        reason: None,
        request_id: Some(output.message_id.clone()),
        asset_definition: None,
        amount_nanos: None,
        capability: None,
        from_apartment: output.from_apartment.clone(),
        to_apartment: Some(apartment_name.to_owned()),
        channel: Some(output.channel.clone()),
        payload_hash: Some(output.payload_hash),
        artifact_hash: None,
        provenance_hash: None,
        run_label: None,
        budget_units: None,
        budget_remaining_units: None,
    });
    registry.next_sequence = registry.next_sequence.saturating_add(1);

    Ok(AgentMailboxMutationOutput {
        event_count: u32::try_from(registry.apartment_events.len()).unwrap_or(u32::MAX),
        ..output
    })
}

fn apply_agent_artifact_allow(
    registry: &mut RegistryState,
    apartment_name: &str,
    artifact_hash: &str,
    provenance_hash: Option<&str>,
) -> Result<AgentAutonomyMutationOutput> {
    ensure_registry_schema(registry)?;
    validate_apartment_name(apartment_name)?;
    let artifact_hash = artifact_hash.trim();
    validate_hash_like_value("--artifact-hash", artifact_hash)?;
    let provenance_hash = normalize_optional_hash_like_value("--provenance-hash", provenance_hash)?;

    let sequence = registry.next_sequence;
    let output = {
        let entry = registry
            .apartments
            .get_mut(apartment_name)
            .ok_or_else(|| eyre!("apartment `{apartment_name}` not found in registry"))?;
        if runtime_status_for_sequence(entry, sequence) == AgentRuntimeStatus::LeaseExpired {
            return Err(eyre!(
                "apartment `{apartment_name}` lease expired at sequence {}; renew before autonomy actions",
                entry.lease_expires_sequence
            ));
        }
        if !(policy_capability_active(entry, "governance.audit")
            || policy_capability_active(entry, "agent.autonomy.allow"))
        {
            return Err(eyre!(
                "apartment `{apartment_name}` does not have active `governance.audit` or `agent.autonomy.allow` capability"
            ));
        }
        if entry
            .artifact_allowlist
            .get(artifact_hash)
            .is_some_and(|rule| rule.provenance_hash == provenance_hash)
        {
            return Err(eyre!(
                "artifact `{artifact_hash}` already allowlisted for apartment `{apartment_name}` with the same provenance rule"
            ));
        }
        entry.artifact_allowlist.insert(
            artifact_hash.to_owned(),
            AgentArtifactAllowRule {
                artifact_hash: artifact_hash.to_owned(),
                provenance_hash: provenance_hash.clone(),
                added_sequence: sequence,
            },
        );

        AgentAutonomyMutationOutput {
            action: AgentApartmentAction::ArtifactAllowed,
            apartment_name: apartment_name.to_owned(),
            sequence,
            artifact_hash: artifact_hash.to_owned(),
            provenance_hash: provenance_hash.clone(),
            run_id: None,
            run_label: None,
            budget_units: None,
            status: runtime_status_for_sequence(entry, sequence.saturating_add(1)),
            budget_remaining_units: entry.autonomy_budget_remaining_units,
            allowlist_count: u32::try_from(entry.artifact_allowlist.len()).unwrap_or(u32::MAX),
            run_count: u32::try_from(entry.autonomy_run_history.len()).unwrap_or(u32::MAX),
            event_count: 0,
        }
    };
    registry.apartment_events.push(AgentApartmentEvent {
        sequence,
        action: AgentApartmentAction::ArtifactAllowed,
        apartment_name: apartment_name.to_owned(),
        status: output.status,
        lease_expires_sequence: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.lease_expires_sequence)
            .unwrap_or(sequence),
        manifest_hash: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.manifest_hash)
            .unwrap_or(Hash::new(artifact_hash.as_bytes())),
        restart_count: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.restart_count)
            .unwrap_or(0),
        reason: None,
        request_id: None,
        asset_definition: None,
        amount_nanos: None,
        capability: None,
        from_apartment: None,
        to_apartment: None,
        channel: None,
        payload_hash: None,
        artifact_hash: Some(output.artifact_hash.clone()),
        provenance_hash: output.provenance_hash.clone(),
        run_label: None,
        budget_units: None,
        budget_remaining_units: Some(output.budget_remaining_units),
    });
    registry.next_sequence = registry.next_sequence.saturating_add(1);

    Ok(AgentAutonomyMutationOutput {
        event_count: u32::try_from(registry.apartment_events.len()).unwrap_or(u32::MAX),
        ..output
    })
}

fn apply_agent_autonomy_run(
    registry: &mut RegistryState,
    apartment_name: &str,
    artifact_hash: &str,
    provenance_hash: Option<&str>,
    budget_units: u64,
    run_label: &str,
) -> Result<AgentAutonomyMutationOutput> {
    ensure_registry_schema(registry)?;
    validate_apartment_name(apartment_name)?;
    if budget_units == 0 {
        return Err(eyre!("--budget-units must be greater than zero"));
    }
    let artifact_hash = artifact_hash.trim();
    validate_hash_like_value("--artifact-hash", artifact_hash)?;
    let provenance_hash = normalize_optional_hash_like_value("--provenance-hash", provenance_hash)?;
    let run_label = run_label.trim();
    if run_label.is_empty() {
        return Err(eyre!("--run-label must not be empty"));
    }
    if run_label.len() > AGENT_AUTONOMY_MAX_LABEL_BYTES {
        return Err(eyre!(
            "--run-label exceeds max bytes ({AGENT_AUTONOMY_MAX_LABEL_BYTES})"
        ));
    }
    if run_label.chars().any(|ch| ch.is_control()) {
        return Err(eyre!("--run-label must not contain control characters"));
    }

    let sequence = registry.next_sequence;
    let output = {
        let entry = registry
            .apartments
            .get_mut(apartment_name)
            .ok_or_else(|| eyre!("apartment `{apartment_name}` not found in registry"))?;
        if runtime_status_for_sequence(entry, sequence) == AgentRuntimeStatus::LeaseExpired {
            return Err(eyre!(
                "apartment `{apartment_name}` lease expired at sequence {}; renew before autonomy actions",
                entry.lease_expires_sequence
            ));
        }
        if !policy_capability_active(entry, "agent.autonomy.run") {
            return Err(eyre!(
                "apartment `{apartment_name}` does not have active `agent.autonomy.run` capability"
            ));
        }
        let allow_rule = entry.artifact_allowlist.get(artifact_hash).ok_or_else(|| {
            eyre!("artifact `{artifact_hash}` is not allowlisted for apartment `{apartment_name}`")
        })?;
        if let Some(expected_provenance) = allow_rule.provenance_hash.as_deref() {
            let provided_provenance = provenance_hash.as_deref().ok_or_else(|| {
                eyre!(
                    "artifact `{artifact_hash}` requires --provenance-hash `{expected_provenance}`"
                )
            })?;
            if provided_provenance != expected_provenance {
                return Err(eyre!(
                    "artifact `{artifact_hash}` provenance mismatch: expected `{expected_provenance}`, got `{provided_provenance}`"
                ));
            }
        }
        if budget_units > entry.autonomy_budget_remaining_units {
            return Err(eyre!(
                "requested budget {} exceeds remaining autonomy budget {} for apartment `{apartment_name}`",
                budget_units,
                entry.autonomy_budget_remaining_units
            ));
        }

        entry.autonomy_budget_remaining_units = entry
            .autonomy_budget_remaining_units
            .saturating_sub(budget_units);
        let run_id = format!("{apartment_name}:autonomy:{sequence}");
        entry.autonomy_run_history.push(AgentAutonomyRunRecord {
            run_id: run_id.clone(),
            artifact_hash: artifact_hash.to_owned(),
            provenance_hash: provenance_hash.clone(),
            budget_units,
            run_label: run_label.to_owned(),
            approved_sequence: sequence,
        });

        AgentAutonomyMutationOutput {
            action: AgentApartmentAction::AutonomyRunApproved,
            apartment_name: apartment_name.to_owned(),
            sequence,
            artifact_hash: artifact_hash.to_owned(),
            provenance_hash: provenance_hash.clone(),
            run_id: Some(run_id),
            run_label: Some(run_label.to_owned()),
            budget_units: Some(budget_units),
            status: runtime_status_for_sequence(entry, sequence.saturating_add(1)),
            budget_remaining_units: entry.autonomy_budget_remaining_units,
            allowlist_count: u32::try_from(entry.artifact_allowlist.len()).unwrap_or(u32::MAX),
            run_count: u32::try_from(entry.autonomy_run_history.len()).unwrap_or(u32::MAX),
            event_count: 0,
        }
    };
    registry.apartment_events.push(AgentApartmentEvent {
        sequence,
        action: AgentApartmentAction::AutonomyRunApproved,
        apartment_name: apartment_name.to_owned(),
        status: output.status,
        lease_expires_sequence: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.lease_expires_sequence)
            .unwrap_or(sequence),
        manifest_hash: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.manifest_hash)
            .unwrap_or(Hash::new(artifact_hash.as_bytes())),
        restart_count: registry
            .apartments
            .get(apartment_name)
            .map(|entry| entry.restart_count)
            .unwrap_or(0),
        reason: None,
        request_id: output.run_id.clone(),
        asset_definition: None,
        amount_nanos: None,
        capability: None,
        from_apartment: None,
        to_apartment: None,
        channel: None,
        payload_hash: None,
        artifact_hash: Some(output.artifact_hash.clone()),
        provenance_hash: output.provenance_hash.clone(),
        run_label: output.run_label.clone(),
        budget_units: output.budget_units,
        budget_remaining_units: Some(output.budget_remaining_units),
    });
    registry.next_sequence = registry.next_sequence.saturating_add(1);

    Ok(AgentAutonomyMutationOutput {
        event_count: u32::try_from(registry.apartment_events.len()).unwrap_or(u32::MAX),
        ..output
    })
}

fn policy_capability_active(state: &AgentApartmentRuntimeState, capability: &str) -> bool {
    let declared = state
        .manifest
        .policy_capabilities
        .iter()
        .any(|candidate| candidate.as_ref() == capability);
    let revoked = state
        .revoked_policy_capabilities
        .iter()
        .any(|candidate| candidate == capability);
    declared && !revoked
}

fn wallet_day_bucket(sequence: u64) -> u64 {
    sequence / AGENT_WALLET_DAY_TICKS
}

fn wallet_day_key(asset_definition: &str, day_bucket: u64) -> String {
    format!("{asset_definition}:{day_bucket}")
}

fn wallet_day_spent(
    state: &AgentApartmentRuntimeState,
    asset_definition: &str,
    day_bucket: u64,
) -> u64 {
    state
        .wallet_daily_spend
        .get(&wallet_day_key(asset_definition, day_bucket))
        .map(|entry| entry.spent_nanos)
        .unwrap_or(0)
}

fn wallet_record_spend(
    state: &mut AgentApartmentRuntimeState,
    asset_definition: &str,
    day_bucket: u64,
    spent_nanos: u64,
) {
    let key = wallet_day_key(asset_definition, day_bucket);
    state.wallet_daily_spend.insert(
        key,
        AgentWalletDailySpendEntry {
            asset_definition: asset_definition.to_owned(),
            day_bucket,
            spent_nanos,
        },
    );
}

fn revoked_policy_capability_count(state: &AgentApartmentRuntimeState) -> u32 {
    u32::try_from(state.revoked_policy_capabilities.len()).unwrap_or(u32::MAX)
}

fn pending_wallet_request_count(state: &AgentApartmentRuntimeState) -> u32 {
    u32::try_from(state.pending_wallet_requests.len()).unwrap_or(u32::MAX)
}

fn pending_mailbox_message_count(state: &AgentApartmentRuntimeState) -> u32 {
    u32::try_from(state.mailbox_queue.len()).unwrap_or(u32::MAX)
}

fn validate_hash_like_value(flag_name: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(eyre!("{flag_name} must not be empty"));
    }
    if value.len() > AGENT_AUTONOMY_MAX_HASH_BYTES {
        return Err(eyre!(
            "{flag_name} exceeds max bytes ({AGENT_AUTONOMY_MAX_HASH_BYTES})"
        ));
    }
    if value.chars().any(|ch| ch.is_ascii_whitespace()) {
        return Err(eyre!("{flag_name} must not contain whitespace"));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ':' | '-' | '_' | '.' | '#'))
    {
        return Err(eyre!(
            "{flag_name} must use only ASCII letters, digits, or [: - _ . #]"
        ));
    }
    Ok(())
}

fn normalize_optional_hash_like_value(
    flag_name: &str,
    value: Option<&str>,
) -> Result<Option<String>> {
    match value {
        Some(raw) => {
            let normalized = raw.trim();
            validate_hash_like_value(flag_name, normalized)?;
            Ok(Some(normalized.to_owned()))
        }
        None => Ok(None),
    }
}

fn validate_apartment_name(apartment_name: &str) -> Result<()> {
    if apartment_name.trim().is_empty() {
        return Err(eyre!("--apartment-name must not be empty"));
    }
    Ok(())
}

fn runtime_status_for_sequence(
    state: &AgentApartmentRuntimeState,
    current_sequence: u64,
) -> AgentRuntimeStatus {
    if current_sequence >= state.lease_expires_sequence {
        AgentRuntimeStatus::LeaseExpired
    } else {
        state.status
    }
}

fn lease_remaining_ticks(state: &AgentApartmentRuntimeState, current_sequence: u64) -> u64 {
    state
        .lease_expires_sequence
        .saturating_sub(current_sequence)
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

fn rollout_handle(service_name: &str, sequence: u64) -> String {
    format!("{service_name}:rollout:{sequence}")
}

fn signed_bundle_request(
    bundle: SoraDeploymentBundleV1,
    key_pair: &KeyPair,
) -> Result<SignedBundleRequest> {
    let payload = encode_bundle_provenance_payload(&bundle)
        .wrap_err("failed to encode deployment bundle payload for signing")?;
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
    let encoded = encode_rollback_signature_payload(&payload)
        .wrap_err("failed to encode rollback payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedRollbackRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn encode_rollback_signature_payload(payload: &RollbackPayload) -> Result<Vec<u8>> {
    encode_rollback_provenance_payload(
        payload.service_name.as_str(),
        payload.target_version.as_deref(),
    )
    .wrap_err("failed to encode rollback signature payload tuple")
}

fn signed_rollout_request(
    service_name: &str,
    rollout_handle: &str,
    healthy: bool,
    promote_to_percent: Option<u8>,
    governance_tx_hash: Hash,
    key_pair: &KeyPair,
) -> Result<SignedRolloutAdvanceRequest> {
    if service_name.trim().is_empty() {
        return Err(eyre!("--service-name must not be empty"));
    }
    if rollout_handle.trim().is_empty() {
        return Err(eyre!("--rollout-handle must not be empty"));
    }
    if promote_to_percent.is_some_and(|value| value > 100) {
        return Err(eyre!("--promote-to-percent must be within 0..=100"));
    }
    let payload = RolloutAdvancePayload {
        service_name: service_name.to_string(),
        rollout_handle: rollout_handle.to_string(),
        healthy,
        promote_to_percent,
        governance_tx_hash,
    };
    let encoded = encode_rollout_signature_payload(&payload)
        .wrap_err("failed to encode rollout payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedRolloutAdvanceRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_agent_deploy_request(
    manifest: AgentApartmentManifestV1,
    lease_ticks: u64,
    autonomy_budget_units: u64,
    key_pair: &KeyPair,
) -> Result<SignedAgentDeployRequest> {
    let payload = AgentDeployPayload {
        manifest,
        lease_ticks,
        autonomy_budget_units: Some(autonomy_budget_units),
    };
    let encoded = encode_agent_deploy_signature_payload(&payload)
        .wrap_err("failed to encode agent deploy payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedAgentDeployRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_agent_lease_renew_request(
    apartment_name: &str,
    lease_ticks: u64,
    key_pair: &KeyPair,
) -> Result<SignedAgentLeaseRenewRequest> {
    if apartment_name.trim().is_empty() {
        return Err(eyre!("--apartment-name must not be empty"));
    }
    if lease_ticks == 0 {
        return Err(eyre!("--lease-ticks must be greater than zero"));
    }
    let payload = AgentLeaseRenewPayload {
        apartment_name: apartment_name.to_owned(),
        lease_ticks,
    };
    let encoded = encode_agent_lease_renew_signature_payload(&payload)
        .wrap_err("failed to encode agent lease renew payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedAgentLeaseRenewRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_agent_restart_request(
    apartment_name: &str,
    reason: &str,
    key_pair: &KeyPair,
) -> Result<SignedAgentRestartRequest> {
    if apartment_name.trim().is_empty() {
        return Err(eyre!("--apartment-name must not be empty"));
    }
    if reason.trim().is_empty() {
        return Err(eyre!("--reason must not be empty"));
    }
    let payload = AgentRestartPayload {
        apartment_name: apartment_name.to_owned(),
        reason: reason.to_owned(),
    };
    let encoded = encode_agent_restart_signature_payload(&payload)
        .wrap_err("failed to encode agent restart payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedAgentRestartRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_agent_policy_revoke_request(
    apartment_name: &str,
    capability: &str,
    reason: Option<&str>,
    key_pair: &KeyPair,
) -> Result<SignedAgentPolicyRevokeRequest> {
    if apartment_name.trim().is_empty() {
        return Err(eyre!("--apartment-name must not be empty"));
    }
    if capability.trim().is_empty() {
        return Err(eyre!("--capability must not be empty"));
    }
    let payload = AgentPolicyRevokePayload {
        apartment_name: apartment_name.to_owned(),
        capability: capability.to_owned(),
        reason: reason.map(ToOwned::to_owned),
    };
    let encoded = encode_agent_policy_revoke_signature_payload(&payload)
        .wrap_err("failed to encode agent policy revoke payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedAgentPolicyRevokeRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_agent_wallet_spend_request(
    apartment_name: &str,
    asset_definition: &str,
    amount_nanos: u64,
    key_pair: &KeyPair,
) -> Result<SignedAgentWalletSpendRequest> {
    if apartment_name.trim().is_empty() {
        return Err(eyre!("--apartment-name must not be empty"));
    }
    if asset_definition.trim().is_empty() {
        return Err(eyre!("--asset-definition must not be empty"));
    }
    if amount_nanos == 0 {
        return Err(eyre!("--amount-nanos must be greater than zero"));
    }
    let payload = AgentWalletSpendPayload {
        apartment_name: apartment_name.to_owned(),
        asset_definition: asset_definition.to_owned(),
        amount_nanos,
    };
    let encoded = encode_agent_wallet_spend_signature_payload(&payload)
        .wrap_err("failed to encode agent wallet spend payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedAgentWalletSpendRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_agent_wallet_approve_request(
    apartment_name: &str,
    request_id: &str,
    key_pair: &KeyPair,
) -> Result<SignedAgentWalletApproveRequest> {
    if apartment_name.trim().is_empty() {
        return Err(eyre!("--apartment-name must not be empty"));
    }
    if request_id.trim().is_empty() {
        return Err(eyre!("--request-id must not be empty"));
    }
    let payload = AgentWalletApprovePayload {
        apartment_name: apartment_name.to_owned(),
        request_id: request_id.to_owned(),
    };
    let encoded = encode_agent_wallet_approve_signature_payload(&payload)
        .wrap_err("failed to encode agent wallet approve payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedAgentWalletApproveRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_agent_message_send_request(
    from_apartment: &str,
    to_apartment: &str,
    channel: &str,
    payload: &str,
    key_pair: &KeyPair,
) -> Result<SignedAgentMessageSendRequest> {
    if from_apartment.trim().is_empty() {
        return Err(eyre!("--from-apartment must not be empty"));
    }
    if to_apartment.trim().is_empty() {
        return Err(eyre!("--to-apartment must not be empty"));
    }
    if channel.trim().is_empty() {
        return Err(eyre!("--channel must not be empty"));
    }
    if payload.trim().is_empty() {
        return Err(eyre!("--payload must not be empty"));
    }
    let payload = AgentMessageSendPayload {
        from_apartment: from_apartment.to_owned(),
        to_apartment: to_apartment.to_owned(),
        channel: channel.to_owned(),
        payload: payload.to_owned(),
    };
    let encoded = encode_agent_message_send_signature_payload(&payload)
        .wrap_err("failed to encode agent message send payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedAgentMessageSendRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_agent_message_ack_request(
    apartment_name: &str,
    message_id: &str,
    key_pair: &KeyPair,
) -> Result<SignedAgentMessageAckRequest> {
    if apartment_name.trim().is_empty() {
        return Err(eyre!("--apartment-name must not be empty"));
    }
    if message_id.trim().is_empty() {
        return Err(eyre!("--message-id must not be empty"));
    }
    let payload = AgentMessageAckPayload {
        apartment_name: apartment_name.to_owned(),
        message_id: message_id.to_owned(),
    };
    let encoded = encode_agent_message_ack_signature_payload(&payload)
        .wrap_err("failed to encode agent message ack payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedAgentMessageAckRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_agent_artifact_allow_request(
    apartment_name: &str,
    artifact_hash: &str,
    provenance_hash: Option<&str>,
    key_pair: &KeyPair,
) -> Result<SignedAgentArtifactAllowRequest> {
    if apartment_name.trim().is_empty() {
        return Err(eyre!("--apartment-name must not be empty"));
    }
    validate_hash_like_value("--artifact-hash", artifact_hash.trim())?;
    if let Some(provenance_hash) = provenance_hash {
        validate_hash_like_value("--provenance-hash", provenance_hash.trim())?;
    }
    let payload = AgentArtifactAllowPayload {
        apartment_name: apartment_name.to_owned(),
        artifact_hash: artifact_hash.to_owned(),
        provenance_hash: provenance_hash.map(ToOwned::to_owned),
    };
    let encoded = encode_agent_artifact_allow_signature_payload(&payload)
        .wrap_err("failed to encode agent autonomy allow payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedAgentArtifactAllowRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_agent_autonomy_run_request(
    apartment_name: &str,
    artifact_hash: &str,
    provenance_hash: Option<&str>,
    budget_units: u64,
    run_label: &str,
    key_pair: &KeyPair,
) -> Result<SignedAgentAutonomyRunRequest> {
    if apartment_name.trim().is_empty() {
        return Err(eyre!("--apartment-name must not be empty"));
    }
    validate_hash_like_value("--artifact-hash", artifact_hash.trim())?;
    if let Some(provenance_hash) = provenance_hash {
        validate_hash_like_value("--provenance-hash", provenance_hash.trim())?;
    }
    if budget_units == 0 {
        return Err(eyre!("--budget-units must be greater than zero"));
    }
    let run_label = run_label.trim();
    if run_label.is_empty() {
        return Err(eyre!("--run-label must not be empty"));
    }
    let payload = AgentAutonomyRunPayload {
        apartment_name: apartment_name.to_owned(),
        artifact_hash: artifact_hash.to_owned(),
        provenance_hash: provenance_hash.map(ToOwned::to_owned),
        budget_units,
        run_label: run_label.to_owned(),
    };
    let encoded = encode_agent_autonomy_run_signature_payload(&payload)
        .wrap_err("failed to encode agent autonomy run payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedAgentAutonomyRunRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn signed_training_job_start_request(
    service_name: &str,
    model_name: &str,
    job_id: &str,
    worker_group_size: u16,
    target_steps: u32,
    checkpoint_interval_steps: u32,
    max_retries: u8,
    step_compute_units: u64,
    compute_budget_units: u64,
    storage_budget_bytes: u64,
    key_pair: &KeyPair,
) -> Result<SignedTrainingJobStartRequest> {
    if service_name.trim().is_empty() {
        return Err(eyre!("--service-name must not be empty"));
    }
    if model_name.trim().is_empty() {
        return Err(eyre!("--model-name must not be empty"));
    }
    if job_id.trim().is_empty() {
        return Err(eyre!("--job-id must not be empty"));
    }
    if worker_group_size == 0 {
        return Err(eyre!("--worker-group-size must be greater than zero"));
    }
    if target_steps == 0 {
        return Err(eyre!("--target-steps must be greater than zero"));
    }
    if checkpoint_interval_steps == 0 {
        return Err(eyre!(
            "--checkpoint-interval-steps must be greater than zero"
        ));
    }
    if step_compute_units == 0 {
        return Err(eyre!("--step-compute-units must be greater than zero"));
    }
    if compute_budget_units == 0 {
        return Err(eyre!("--compute-budget-units must be greater than zero"));
    }
    if storage_budget_bytes == 0 {
        return Err(eyre!("--storage-budget-bytes must be greater than zero"));
    }
    let payload = TrainingJobStartPayload {
        service_name: service_name.to_owned(),
        model_name: model_name.to_owned(),
        job_id: job_id.to_owned(),
        worker_group_size,
        target_steps,
        checkpoint_interval_steps,
        max_retries,
        step_compute_units,
        compute_budget_units,
        storage_budget_bytes,
    };
    let encoded = encode_training_job_start_signature_payload(&payload)
        .wrap_err("failed to encode training job start payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedTrainingJobStartRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_training_job_checkpoint_request(
    service_name: &str,
    job_id: &str,
    completed_step: u32,
    checkpoint_size_bytes: u64,
    metrics_hash: Hash,
    key_pair: &KeyPair,
) -> Result<SignedTrainingJobCheckpointRequest> {
    if service_name.trim().is_empty() {
        return Err(eyre!("--service-name must not be empty"));
    }
    if job_id.trim().is_empty() {
        return Err(eyre!("--job-id must not be empty"));
    }
    if completed_step == 0 {
        return Err(eyre!("--completed-step must be greater than zero"));
    }
    if checkpoint_size_bytes == 0 {
        return Err(eyre!("--checkpoint-size-bytes must be greater than zero"));
    }
    let payload = TrainingJobCheckpointPayload {
        service_name: service_name.to_owned(),
        job_id: job_id.to_owned(),
        completed_step,
        checkpoint_size_bytes,
        metrics_hash,
    };
    let encoded = encode_training_job_checkpoint_signature_payload(&payload)
        .wrap_err("failed to encode training job checkpoint payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedTrainingJobCheckpointRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_training_job_retry_request(
    service_name: &str,
    job_id: &str,
    reason: &str,
    key_pair: &KeyPair,
) -> Result<SignedTrainingJobRetryRequest> {
    if service_name.trim().is_empty() {
        return Err(eyre!("--service-name must not be empty"));
    }
    if job_id.trim().is_empty() {
        return Err(eyre!("--job-id must not be empty"));
    }
    if reason.trim().is_empty() {
        return Err(eyre!("--reason must not be empty"));
    }
    let payload = TrainingJobRetryPayload {
        service_name: service_name.to_owned(),
        job_id: job_id.to_owned(),
        reason: reason.to_owned(),
    };
    let encoded = encode_training_job_retry_signature_payload(&payload)
        .wrap_err("failed to encode training job retry payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedTrainingJobRetryRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn signed_model_artifact_register_request(
    service_name: &str,
    model_name: &str,
    training_job_id: &str,
    weight_artifact_hash: Hash,
    dataset_ref: &str,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    key_pair: &KeyPair,
) -> Result<SignedModelArtifactRegisterRequest> {
    if service_name.trim().is_empty() {
        return Err(eyre!("--service-name must not be empty"));
    }
    if model_name.trim().is_empty() {
        return Err(eyre!("--model-name must not be empty"));
    }
    if training_job_id.trim().is_empty() {
        return Err(eyre!("--training-job-id must not be empty"));
    }
    if dataset_ref.trim().is_empty() {
        return Err(eyre!("--dataset-ref must not be empty"));
    }
    let payload = ModelArtifactRegisterPayload {
        service_name: service_name.to_owned(),
        model_name: model_name.to_owned(),
        training_job_id: training_job_id.to_owned(),
        weight_artifact_hash,
        dataset_ref: dataset_ref.to_owned(),
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    };
    let encoded = encode_model_artifact_register_signature_payload(&payload)
        .wrap_err("failed to encode model artifact register payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedModelArtifactRegisterRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn signed_model_weight_register_request(
    service_name: &str,
    model_name: &str,
    weight_version: &str,
    training_job_id: &str,
    parent_version: Option<&str>,
    weight_artifact_hash: Hash,
    dataset_ref: &str,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    key_pair: &KeyPair,
) -> Result<SignedModelWeightRegisterRequest> {
    if service_name.trim().is_empty() {
        return Err(eyre!("--service-name must not be empty"));
    }
    if model_name.trim().is_empty() {
        return Err(eyre!("--model-name must not be empty"));
    }
    if weight_version.trim().is_empty() {
        return Err(eyre!("--weight-version must not be empty"));
    }
    if training_job_id.trim().is_empty() {
        return Err(eyre!("--training-job-id must not be empty"));
    }
    if parent_version.is_some_and(|value| value.trim().is_empty()) {
        return Err(eyre!("--parent-version must not be empty when provided"));
    }
    if dataset_ref.trim().is_empty() {
        return Err(eyre!("--dataset-ref must not be empty"));
    }
    let payload = ModelWeightRegisterPayload {
        service_name: service_name.to_owned(),
        model_name: model_name.to_owned(),
        weight_version: weight_version.to_owned(),
        training_job_id: training_job_id.to_owned(),
        parent_version: parent_version.map(ToOwned::to_owned),
        weight_artifact_hash,
        dataset_ref: dataset_ref.to_owned(),
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    };
    let encoded = encode_model_weight_register_signature_payload(&payload)
        .wrap_err("failed to encode model weight register payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedModelWeightRegisterRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_model_weight_promote_request(
    service_name: &str,
    model_name: &str,
    weight_version: &str,
    gate_approved: bool,
    gate_report_hash: Hash,
    key_pair: &KeyPair,
) -> Result<SignedModelWeightPromoteRequest> {
    if service_name.trim().is_empty() {
        return Err(eyre!("--service-name must not be empty"));
    }
    if model_name.trim().is_empty() {
        return Err(eyre!("--model-name must not be empty"));
    }
    if weight_version.trim().is_empty() {
        return Err(eyre!("--weight-version must not be empty"));
    }
    let payload = ModelWeightPromotePayload {
        service_name: service_name.to_owned(),
        model_name: model_name.to_owned(),
        weight_version: weight_version.to_owned(),
        gate_approved,
        gate_report_hash,
    };
    let encoded = encode_model_weight_promote_signature_payload(&payload)
        .wrap_err("failed to encode model weight promote payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedModelWeightPromoteRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn signed_model_weight_rollback_request(
    service_name: &str,
    model_name: &str,
    target_version: &str,
    reason: &str,
    key_pair: &KeyPair,
) -> Result<SignedModelWeightRollbackRequest> {
    if service_name.trim().is_empty() {
        return Err(eyre!("--service-name must not be empty"));
    }
    if model_name.trim().is_empty() {
        return Err(eyre!("--model-name must not be empty"));
    }
    if target_version.trim().is_empty() {
        return Err(eyre!("--target-version must not be empty"));
    }
    if reason.trim().is_empty() {
        return Err(eyre!("--reason must not be empty"));
    }
    let payload = ModelWeightRollbackPayload {
        service_name: service_name.to_owned(),
        model_name: model_name.to_owned(),
        target_version: target_version.to_owned(),
        reason: reason.to_owned(),
    };
    let encoded = encode_model_weight_rollback_signature_payload(&payload)
        .wrap_err("failed to encode model weight rollback payload for signing")?;
    let signature = Signature::new(key_pair.private_key(), &encoded);
    Ok(SignedModelWeightRollbackRequest {
        payload,
        provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        },
    })
}

fn encode_rollout_signature_payload(payload: &RolloutAdvancePayload) -> Result<Vec<u8>> {
    encode_rollout_provenance_payload(
        payload.service_name.as_str(),
        payload.rollout_handle.as_str(),
        payload.healthy,
        payload.promote_to_percent,
        payload.governance_tx_hash.clone(),
    )
    .wrap_err("failed to encode rollout signature payload tuple")
}

fn encode_agent_deploy_signature_payload(payload: &AgentDeployPayload) -> Result<Vec<u8>> {
    encode_agent_deploy_provenance_payload(
        payload.manifest.clone(),
        payload.lease_ticks,
        payload.autonomy_budget_units,
    )
    .wrap_err("failed to encode agent deploy signature payload tuple")
}

fn encode_agent_lease_renew_signature_payload(payload: &AgentLeaseRenewPayload) -> Result<Vec<u8>> {
    encode_agent_lease_renew_provenance_payload(
        payload.apartment_name.as_str(),
        payload.lease_ticks,
    )
    .wrap_err("failed to encode agent lease renew signature payload tuple")
}

fn encode_agent_restart_signature_payload(payload: &AgentRestartPayload) -> Result<Vec<u8>> {
    encode_agent_restart_provenance_payload(
        payload.apartment_name.as_str(),
        payload.reason.as_str(),
    )
    .wrap_err("failed to encode agent restart signature payload tuple")
}

fn encode_agent_policy_revoke_signature_payload(
    payload: &AgentPolicyRevokePayload,
) -> Result<Vec<u8>> {
    encode_agent_policy_revoke_provenance_payload(
        payload.apartment_name.as_str(),
        payload.capability.as_str(),
        payload.reason.as_deref(),
    )
    .wrap_err("failed to encode agent policy revoke signature payload tuple")
}

fn encode_agent_wallet_spend_signature_payload(
    payload: &AgentWalletSpendPayload,
) -> Result<Vec<u8>> {
    encode_agent_wallet_spend_provenance_payload(
        payload.apartment_name.as_str(),
        payload.asset_definition.as_str(),
        payload.amount_nanos,
    )
    .wrap_err("failed to encode agent wallet spend signature payload tuple")
}

fn encode_agent_wallet_approve_signature_payload(
    payload: &AgentWalletApprovePayload,
) -> Result<Vec<u8>> {
    encode_agent_wallet_approve_provenance_payload(
        payload.apartment_name.as_str(),
        payload.request_id.as_str(),
    )
    .wrap_err("failed to encode agent wallet approve signature payload tuple")
}

fn encode_agent_message_send_signature_payload(
    payload: &AgentMessageSendPayload,
) -> Result<Vec<u8>> {
    encode_agent_message_send_provenance_payload(
        payload.from_apartment.as_str(),
        payload.to_apartment.as_str(),
        payload.channel.as_str(),
        payload.payload.as_str(),
    )
    .wrap_err("failed to encode agent message send signature payload tuple")
}

fn encode_agent_message_ack_signature_payload(payload: &AgentMessageAckPayload) -> Result<Vec<u8>> {
    encode_agent_message_ack_provenance_payload(
        payload.apartment_name.as_str(),
        payload.message_id.as_str(),
    )
    .wrap_err("failed to encode agent message ack signature payload tuple")
}

fn encode_agent_artifact_allow_signature_payload(
    payload: &AgentArtifactAllowPayload,
) -> Result<Vec<u8>> {
    encode_agent_artifact_allow_provenance_payload(
        payload.apartment_name.as_str(),
        payload.artifact_hash.as_str(),
        payload.provenance_hash.as_deref(),
    )
    .wrap_err("failed to encode agent artifact allow signature payload tuple")
}

fn encode_agent_autonomy_run_signature_payload(
    payload: &AgentAutonomyRunPayload,
) -> Result<Vec<u8>> {
    encode_agent_autonomy_run_provenance_payload(
        payload.apartment_name.as_str(),
        payload.artifact_hash.as_str(),
        payload.provenance_hash.as_deref(),
        payload.budget_units,
        payload.run_label.as_str(),
    )
    .wrap_err("failed to encode agent autonomy run signature payload tuple")
}

fn encode_training_job_start_signature_payload(
    payload: &TrainingJobStartPayload,
) -> Result<Vec<u8>> {
    encode_training_job_start_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.job_id.as_str(),
        payload.worker_group_size,
        payload.target_steps,
        payload.checkpoint_interval_steps,
        payload.max_retries,
        payload.step_compute_units,
        payload.compute_budget_units,
        payload.storage_budget_bytes,
    )
    .wrap_err("failed to encode training job start signature payload tuple")
}

fn encode_training_job_checkpoint_signature_payload(
    payload: &TrainingJobCheckpointPayload,
) -> Result<Vec<u8>> {
    encode_training_job_checkpoint_provenance_payload(
        payload.service_name.as_str(),
        payload.job_id.as_str(),
        payload.completed_step,
        payload.checkpoint_size_bytes,
        payload.metrics_hash,
    )
    .wrap_err("failed to encode training job checkpoint signature payload tuple")
}

fn encode_training_job_retry_signature_payload(
    payload: &TrainingJobRetryPayload,
) -> Result<Vec<u8>> {
    encode_training_job_retry_provenance_payload(
        payload.service_name.as_str(),
        payload.job_id.as_str(),
        payload.reason.as_str(),
    )
    .wrap_err("failed to encode training job retry signature payload tuple")
}

fn encode_model_artifact_register_signature_payload(
    payload: &ModelArtifactRegisterPayload,
) -> Result<Vec<u8>> {
    encode_model_artifact_register_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.training_job_id.as_str(),
        payload.weight_artifact_hash,
        payload.dataset_ref.as_str(),
        payload.training_config_hash,
        payload.reproducibility_hash,
        payload.provenance_attestation_hash,
    )
    .wrap_err("failed to encode model artifact register signature payload tuple")
}

fn encode_model_weight_register_signature_payload(
    payload: &ModelWeightRegisterPayload,
) -> Result<Vec<u8>> {
    encode_model_weight_register_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.weight_version.as_str(),
        payload.training_job_id.as_str(),
        payload.parent_version.as_deref(),
        payload.weight_artifact_hash,
        payload.dataset_ref.as_str(),
        payload.training_config_hash,
        payload.reproducibility_hash,
        payload.provenance_attestation_hash,
    )
    .wrap_err("failed to encode model weight register signature payload tuple")
}

fn encode_model_weight_promote_signature_payload(
    payload: &ModelWeightPromotePayload,
) -> Result<Vec<u8>> {
    encode_model_weight_promote_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.weight_version.as_str(),
        payload.gate_approved,
        payload.gate_report_hash,
    )
    .wrap_err("failed to encode model weight promote signature payload tuple")
}

fn encode_model_weight_rollback_signature_payload(
    payload: &ModelWeightRollbackPayload,
) -> Result<Vec<u8>> {
    encode_model_weight_rollback_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.target_version.as_str(),
        payload.reason.as_str(),
    )
    .wrap_err("failed to encode model weight rollback signature payload tuple")
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

fn fetch_torii_soracloud_agent_status(
    torii_url: &str,
    apartment_name: Option<&str>,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/agent/status")
        .wrap_err("failed to derive /v1/soracloud/agent/status URL from --torii-url")?;
    if let Some(apartment_name) = apartment_name
        && !apartment_name.trim().is_empty()
    {
        endpoint
            .query_pairs_mut()
            .append_pair("apartment_name", apartment_name.trim());
    }

    let timeout = Duration::from_secs(timeout_secs.max(1));
    let client = BlockingHttpClient::builder()
        .timeout(timeout)
        .build()
        .wrap_err("failed to build HTTP client for soracloud agent status")?;

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
        .wrap_err("failed to read Torii agent status response body")?;
    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body);
        return Err(eyre!(
            "Torii /v1/soracloud/agent/status returned {}: {}",
            status,
            body_text
        ));
    }

    let payload: norito::json::Value = json::from_slice(&body)
        .wrap_err("failed to decode Torii soracloud agent status JSON payload")?;
    Ok((endpoint.to_string(), payload))
}

fn fetch_torii_soracloud_agent_mailbox_status(
    torii_url: &str,
    apartment_name: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let apartment_name = apartment_name.trim();
    if apartment_name.is_empty() {
        return Err(eyre!("--apartment-name must not be empty"));
    }

    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/agent/mailbox/status")
        .wrap_err("failed to derive /v1/soracloud/agent/mailbox/status URL from --torii-url")?;
    endpoint
        .query_pairs_mut()
        .append_pair("apartment_name", apartment_name);

    let timeout = Duration::from_secs(timeout_secs.max(1));
    let client = BlockingHttpClient::builder()
        .timeout(timeout)
        .build()
        .wrap_err("failed to build HTTP client for soracloud agent mailbox status")?;

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
        .wrap_err("failed to read Torii agent mailbox status response body")?;
    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body);
        return Err(eyre!(
            "Torii /v1/soracloud/agent/mailbox/status returned {}: {}",
            status,
            body_text
        ));
    }

    let payload: norito::json::Value = json::from_slice(&body)
        .wrap_err("failed to decode Torii soracloud agent mailbox status JSON payload")?;
    Ok((endpoint.to_string(), payload))
}

fn fetch_torii_soracloud_agent_autonomy_status(
    torii_url: &str,
    apartment_name: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let apartment_name = apartment_name.trim();
    if apartment_name.is_empty() {
        return Err(eyre!("--apartment-name must not be empty"));
    }

    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/agent/autonomy/status")
        .wrap_err("failed to derive /v1/soracloud/agent/autonomy/status URL from --torii-url")?;
    endpoint
        .query_pairs_mut()
        .append_pair("apartment_name", apartment_name);

    let timeout = Duration::from_secs(timeout_secs.max(1));
    let client = BlockingHttpClient::builder()
        .timeout(timeout)
        .build()
        .wrap_err("failed to build HTTP client for soracloud agent autonomy status")?;

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
        .wrap_err("failed to read Torii agent autonomy status response body")?;
    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body);
        return Err(eyre!(
            "Torii /v1/soracloud/agent/autonomy/status returned {}: {}",
            status,
            body_text
        ));
    }

    let payload: norito::json::Value = json::from_slice(&body)
        .wrap_err("failed to decode Torii soracloud agent autonomy status JSON payload")?;
    Ok((endpoint.to_string(), payload))
}

fn fetch_torii_soracloud_training_job_status(
    torii_url: &str,
    service_name: &str,
    job_id: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let service_name = service_name.trim();
    let job_id = job_id.trim();
    if service_name.is_empty() {
        return Err(eyre!("--service-name must not be empty"));
    }
    if job_id.is_empty() {
        return Err(eyre!("--job-id must not be empty"));
    }

    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/training/job/status")
        .wrap_err("failed to derive /v1/soracloud/training/job/status URL from --torii-url")?;
    endpoint
        .query_pairs_mut()
        .append_pair("service_name", service_name)
        .append_pair("job_id", job_id);

    let timeout = Duration::from_secs(timeout_secs.max(1));
    let client = BlockingHttpClient::builder()
        .timeout(timeout)
        .build()
        .wrap_err("failed to build HTTP client for soracloud training job status")?;

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
        .wrap_err("failed to read Torii training job status response body")?;
    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body);
        return Err(eyre!(
            "Torii /v1/soracloud/training/job/status returned {}: {}",
            status,
            body_text
        ));
    }

    let payload: norito::json::Value = json::from_slice(&body)
        .wrap_err("failed to decode Torii training job status JSON payload")?;
    Ok((endpoint.to_string(), payload))
}

fn fetch_torii_soracloud_model_artifact_status(
    torii_url: &str,
    service_name: &str,
    training_job_id: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let service_name = service_name.trim();
    let training_job_id = training_job_id.trim();
    if service_name.is_empty() {
        return Err(eyre!("--service-name must not be empty"));
    }
    if training_job_id.is_empty() {
        return Err(eyre!("--training-job-id must not be empty"));
    }

    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/model/artifact/status")
        .wrap_err("failed to derive /v1/soracloud/model/artifact/status URL from --torii-url")?;
    endpoint
        .query_pairs_mut()
        .append_pair("service_name", service_name)
        .append_pair("training_job_id", training_job_id);

    let timeout = Duration::from_secs(timeout_secs.max(1));
    let client = BlockingHttpClient::builder()
        .timeout(timeout)
        .build()
        .wrap_err("failed to build HTTP client for soracloud model artifact status")?;

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
        .wrap_err("failed to read Torii model artifact status response body")?;
    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body);
        return Err(eyre!(
            "Torii /v1/soracloud/model/artifact/status returned {}: {}",
            status,
            body_text
        ));
    }

    let payload: norito::json::Value = json::from_slice(&body)
        .wrap_err("failed to decode Torii model artifact status JSON payload")?;
    Ok((endpoint.to_string(), payload))
}

fn fetch_torii_soracloud_model_weight_status(
    torii_url: &str,
    service_name: &str,
    model_name: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let service_name = service_name.trim();
    let model_name = model_name.trim();
    if service_name.is_empty() {
        return Err(eyre!("--service-name must not be empty"));
    }
    if model_name.is_empty() {
        return Err(eyre!("--model-name must not be empty"));
    }

    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/model/weight/status")
        .wrap_err("failed to derive /v1/soracloud/model/weight/status URL from --torii-url")?;
    endpoint
        .query_pairs_mut()
        .append_pair("service_name", service_name)
        .append_pair("model_name", model_name);

    let timeout = Duration::from_secs(timeout_secs.max(1));
    let client = BlockingHttpClient::builder()
        .timeout(timeout)
        .build()
        .wrap_err("failed to build HTTP client for soracloud model weight status")?;

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
        .wrap_err("failed to read Torii model weight status response body")?;
    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body);
        return Err(eyre!(
            "Torii /v1/soracloud/model/weight/status returned {}: {}",
            status,
            body_text
        ));
    }

    let payload: norito::json::Value = json::from_slice(&body)
        .wrap_err("failed to decode Torii model weight status JSON payload")?;
    Ok((endpoint.to_string(), payload))
}

fn require_torii_url<'a>(torii_url: Option<&'a str>) -> Result<&'a str> {
    torii_url.ok_or_else(|| {
        eyre!(
            "--torii-url is required for this command (local Soracloud simulation does not implement this operation)"
        )
    })
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
        InitTemplate::HealthApp => {
            container.runtime = SoraContainerRuntimeV1::NativeProcess;
            container.bundle_path = "/bundles/health-app-api.car".to_owned();
            container.entrypoint = "/app/api/server.mjs".to_owned();
            container.args = vec!["--port=8788".to_owned()];
            container
                .env
                .insert("SORACLOUD_TEMPLATE".to_owned(), "health-app".to_owned());
            container
                .env
                .insert("HEALTH_JURISDICTION".to_owned(), "us".to_owned());
            container.env.insert(
                "CONSENT_POLICY_NAMESPACE".to_owned(),
                "health.consent.v1".to_owned(),
            );
            container.capabilities.network =
                SoraNetworkPolicyV1::Allowlist(vec!["torii.sora.internal".to_owned()]);
            container.capabilities.allow_wallet_signing = false;
            container.capabilities.allow_state_writes = true;
            container.capabilities.allow_model_training = false;
            container.lifecycle.healthcheck_path = Some("/health/api/healthz".to_owned());

            service.route = Some(SoraRouteTargetV1 {
                host,
                path_prefix: "/health/api".to_owned(),
                service_port: NonZeroU16::new(8788).expect("nonzero literal"),
                visibility: SoraRouteVisibilityV1::Public,
                tls_mode: SoraTlsModeV1::Required,
            });
            service.replicas = NonZeroU16::new(3).expect("nonzero literal");
            service.state_bindings = vec![
                SoraStateBindingV1 {
                    schema_version: SORA_STATE_BINDING_VERSION_V1,
                    binding_name: "patient_records"
                        .parse()
                        .expect("literal binding name is valid"),
                    scope: SoraStateScopeV1::ConfidentialState,
                    mutability: SoraStateMutabilityV1::AppendOnly,
                    encryption: SoraStateEncryptionV1::FheCiphertext,
                    key_prefix: "/state/health/patient".to_owned(),
                    max_item_bytes: NonZeroU64::new(65_536).expect("nonzero literal"),
                    max_total_bytes: NonZeroU64::new(33_554_432).expect("nonzero literal"),
                },
                SoraStateBindingV1 {
                    schema_version: SORA_STATE_BINDING_VERSION_V1,
                    binding_name: "consent_events"
                        .parse()
                        .expect("literal binding name is valid"),
                    scope: SoraStateScopeV1::ServiceState,
                    mutability: SoraStateMutabilityV1::AppendOnly,
                    encryption: SoraStateEncryptionV1::ClientCiphertext,
                    key_prefix: "/state/health/consent".to_owned(),
                    max_item_bytes: NonZeroU64::new(8_192).expect("nonzero literal"),
                    max_total_bytes: NonZeroU64::new(8_388_608).expect("nonzero literal"),
                },
                SoraStateBindingV1 {
                    schema_version: SORA_STATE_BINDING_VERSION_V1,
                    binding_name: "retention_jobs"
                        .parse()
                        .expect("literal binding name is valid"),
                    scope: SoraStateScopeV1::ServiceState,
                    mutability: SoraStateMutabilityV1::ReadWrite,
                    encryption: SoraStateEncryptionV1::ClientCiphertext,
                    key_prefix: "/state/health/retention".to_owned(),
                    max_item_bytes: NonZeroU64::new(4_096).expect("nonzero literal"),
                    max_total_bytes: NonZeroU64::new(2_097_152).expect("nonzero literal"),
                },
            ];
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
        InitTemplate::HealthApp => {
            scaffold_health_app_template(output_dir, service_name, overwrite)
        }
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

fn scaffold_health_app_template(
    output_dir: &Path,
    service_name: &str,
    overwrite: bool,
) -> Result<Vec<String>> {
    let project_dir = output_dir.join("health-app");
    let package_name = normalized_service_label(service_name);
    let files = vec![
        (
            project_dir.join("package.json"),
            health_app_root_package_json(&package_name),
        ),
        (
            project_dir.join("frontend/package.json"),
            health_app_frontend_package_json(&package_name),
        ),
        (
            project_dir.join("frontend/tsconfig.json"),
            site_tsconfig_json().to_owned(),
        ),
        (
            project_dir.join("frontend/vite.config.ts"),
            health_app_frontend_vite_config().to_owned(),
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
            health_app_frontend_app_vue(service_name),
        ),
        (
            project_dir.join("api/server.mjs"),
            health_app_api_server_mjs().to_owned(),
        ),
        (
            project_dir.join("policy/consent_policy_template.json"),
            health_app_consent_policy_template(),
        ),
        (
            project_dir.join("policy/retention_policy_template.json"),
            health_app_retention_policy_template(),
        ),
        (
            project_dir.join("policy/deletion_workflow_template.json"),
            health_app_deletion_workflow_template(),
        ),
        (
            project_dir.join(".gitignore"),
            "node_modules/\nfrontend/node_modules/\nfrontend/dist/\n".to_owned(),
        ),
        (
            project_dir.join("README.md"),
            health_app_readme(service_name),
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

fn health_app_root_package_json(package_name: &str) -> String {
    format!(
        r#"{{
  "name": "{package_name}-health-app",
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

fn health_app_frontend_package_json(package_name: &str) -> String {
    format!(
        r#"{{
  "name": "{package_name}-health-frontend",
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

fn health_app_frontend_vite_config() -> &'static str {
    r#"import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  server: {
    host: "0.0.0.0",
    port: 5173,
    proxy: {
      "/health/api": "http://127.0.0.1:8788"
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

fn health_app_frontend_app_vue(service_name: &str) -> String {
    format!(
        r#"<template>
  <main class="shell">
    <h1>{service_name} Health Control Panel</h1>
    <section>
      <h2>Consent</h2>
      <form @submit.prevent="grantConsent">
        <label>
          Subject ID
          <input v-model="subjectId" placeholder="patient-001" />
        </label>
        <label>
          Scope
          <input v-model="scope" placeholder="care-team.read" />
        </label>
        <button type="submit">Grant Consent</button>
      </form>
      <button @click="revokeConsent">Revoke Consent</button>
    </section>

    <section>
      <h2>Retention / Deletion</h2>
      <button @click="runRetention">Run Retention Sweep</button>
      <button @click="requestDeletion">Request Subject Deletion</button>
    </section>

    <p v-if="message">{{{{ message }}}}</p>
    <p v-if="error" class="error">{{{{ error }}}}</p>
  </main>
</template>

<script setup lang="ts">
import {{ ref }} from "vue";

const subjectId = ref("patient-001");
const scope = ref("care-team.read");
const message = ref("");
const error = ref("");

async function post(path: string, body: Record<string, string>) {{
  error.value = "";
  const response = await fetch(path, {{
    method: "POST",
    headers: {{ "content-type": "application/json" }},
    body: JSON.stringify(body)
  }});
  if (!response.ok) {{
    error.value = await response.text();
    return null;
  }}
  return response.json();
}}

async function grantConsent() {{
  const payload = await post("/health/api/consent/grant", {{
    subject_id: subjectId.value,
    scope: scope.value
  }});
  if (payload) {{
    message.value = `consent granted for ${{payload.subject_id}}`;
  }}
}}

async function revokeConsent() {{
  const payload = await post("/health/api/consent/revoke", {{
    subject_id: subjectId.value,
    scope: scope.value
  }});
  if (payload) {{
    message.value = `consent revoked for ${{payload.subject_id}}`;
  }}
}}

async function runRetention() {{
  const payload = await post("/health/api/records/retention/sweep", {{
    jurisdiction: "us",
    policy_version: "retention-v1"
  }});
  if (payload) {{
    message.value = `retention sweep planned=${{payload.planned_actions}}`;
  }}
}}

async function requestDeletion() {{
  const payload = await post("/health/api/records/delete", {{
    subject_id: subjectId.value,
    reason: "subject request"
  }});
  if (payload) {{
    message.value = `deletion ticket ${{payload.ticket_id}}`;
  }}
}}
</script>

<style scoped>
.shell {{
  font-family: "Avenir Next", "Segoe UI", sans-serif;
  max-width: 780px;
  margin: 3rem auto;
  padding: 0 1.25rem;
}}

section {{
  margin: 1.5rem 0;
  padding: 1rem;
  border: 1px solid #dde4ec;
  border-radius: 0.5rem;
}}

form {{
  display: grid;
  gap: 0.75rem;
  margin-bottom: 0.75rem;
}}

input {{
  width: 100%;
  padding: 0.5rem;
}}

button {{
  margin-right: 0.75rem;
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

fn health_app_api_server_mjs() -> &'static str {
    r#"import http from "node:http";

const portArg = process.argv.find((value) => value.startsWith("--port="));
const port = Number(portArg?.slice("--port=".length) ?? process.env.PORT ?? "8788");

const consentState = new Map();
const retentionRuns = [];

async function readJson(req) {
  let body = "";
  for await (const chunk of req) {
    body += chunk.toString("utf8");
  }
  return JSON.parse(body);
}

function writeJson(res, status, body) {
  res.writeHead(status, { "content-type": "application/json" });
  res.end(JSON.stringify(body));
}

function requireNonEmpty(value, field) {
  if (!value || typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${field} must not be empty`);
  }
  return value.trim();
}

const server = http.createServer(async (req, res) => {
  if (req.url === "/health/api/healthz") {
    writeJson(res, 200, { ok: true });
    return;
  }

  if (req.method === "POST" && req.url === "/health/api/consent/grant") {
    try {
      const body = await readJson(req);
      const subjectId = requireNonEmpty(body.subject_id, "subject_id");
      const scope = requireNonEmpty(body.scope, "scope");
      const key = `${subjectId}:${scope}`;
      consentState.set(key, {
        status: "granted",
        updated_at_unix_ms: Date.now()
      });
      writeJson(res, 200, { subject_id: subjectId, scope, status: "granted" });
    } catch (error) {
      res.writeHead(400, { "content-type": "text/plain" });
      res.end(`invalid request: ${error.message}`);
    }
    return;
  }

  if (req.method === "POST" && req.url === "/health/api/consent/revoke") {
    try {
      const body = await readJson(req);
      const subjectId = requireNonEmpty(body.subject_id, "subject_id");
      const scope = requireNonEmpty(body.scope, "scope");
      const key = `${subjectId}:${scope}`;
      consentState.set(key, {
        status: "revoked",
        updated_at_unix_ms: Date.now()
      });
      writeJson(res, 200, { subject_id: subjectId, scope, status: "revoked" });
    } catch (error) {
      res.writeHead(400, { "content-type": "text/plain" });
      res.end(`invalid request: ${error.message}`);
    }
    return;
  }

  if (req.method === "POST" && req.url === "/health/api/records/retention/sweep") {
    try {
      const body = await readJson(req);
      const jurisdiction = requireNonEmpty(body.jurisdiction, "jurisdiction");
      const policyVersion = requireNonEmpty(body.policy_version, "policy_version");
      const run = {
        run_id: `retention-${Date.now()}`,
        jurisdiction,
        policy_version: policyVersion,
        planned_actions: 0
      };
      retentionRuns.push(run);
      writeJson(res, 200, run);
    } catch (error) {
      res.writeHead(400, { "content-type": "text/plain" });
      res.end(`invalid request: ${error.message}`);
    }
    return;
  }

  if (req.method === "POST" && req.url === "/health/api/records/delete") {
    try {
      const body = await readJson(req);
      const subjectId = requireNonEmpty(body.subject_id, "subject_id");
      const reason = requireNonEmpty(body.reason, "reason");
      const ticketId = `delete-${Date.now()}`;
      writeJson(res, 202, {
        ticket_id: ticketId,
        subject_id: subjectId,
        reason,
        status: "accepted"
      });
    } catch (error) {
      res.writeHead(400, { "content-type": "text/plain" });
      res.end(`invalid request: ${error.message}`);
    }
    return;
  }

  if (req.method === "GET" && req.url === "/health/api/consent/state") {
    writeJson(res, 200, { entries: Array.from(consentState.entries()) });
    return;
  }

  if (req.method === "GET" && req.url === "/health/api/retention/runs") {
    writeJson(res, 200, { runs: retentionRuns });
    return;
  }

  res.writeHead(404, { "content-type": "text/plain" });
  res.end("not found");
});

server.listen(port, "0.0.0.0", () => {
  // eslint-disable-next-line no-console
  console.log(`health api listening on :${port}`);
});
"#
}

fn health_app_consent_policy_template() -> String {
    r#"{
  "schema_version": 1,
  "policy_name": "health.consent.v1",
  "jurisdiction": "us",
  "required_capabilities": [
    "health.consent.grant",
    "health.consent.revoke"
  ],
  "allowed_scopes": [
    "care-team.read",
    "care-team.write",
    "billing.read"
  ],
  "audit_tag": "health.consent.audit"
}
"#
    .to_owned()
}

fn health_app_retention_policy_template() -> String {
    r#"{
  "schema_version": 1,
  "policy_name": "health.retention.v1",
  "jurisdiction": "us",
  "default_retention_days": 2555,
  "deletion_grace_days": 30,
  "bindings": [
    "patient_records",
    "consent_events"
  ],
  "audit_tag": "health.retention.audit"
}
"#
    .to_owned()
}

fn health_app_deletion_workflow_template() -> String {
    r#"{
  "schema_version": 1,
  "workflow_name": "health.subject.deletion.v1",
  "steps": [
    "validate-subject-request",
    "freeze-read-access",
    "enqueue-redaction-job",
    "emit-deletion-attestation"
  ],
  "requires_break_glass_approval": false,
  "audit_tag": "health.deletion.audit"
}
"#
    .to_owned()
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

fn health_app_readme(service_name: &str) -> String {
    format!(
        r#"# {service_name} Health-App Template

This template provides a private workload starter for regulated health data:

- `frontend/` Vue3 control panel for consent + retention/deletion actions.
- `api/server.mjs` health API starter with consent and retention/deletion endpoints.
- `policy/*.json` governance templates for consent, retention, and deletion workflows.
- Soracloud manifests in the parent init directory (`container_manifest.json`, `service_manifest.json`).

## Local dev

```bash
npm install
npm --prefix frontend install
npm run dev:api
npm run dev:frontend
```

## Policy templates

- `policy/consent_policy_template.json`
- `policy/retention_policy_template.json`
- `policy/deletion_workflow_template.json`

Populate these templates with jurisdiction-specific values and submit through
your governance flow before production rollout.

## Deploy API service on Soracloud

```bash
iroha app soracloud deploy \
  --container ../container_manifest.json \
  --service ../service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

## Publish frontend bundle

```bash
npm run build
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/health_frontend_manifest.to \
  --car-out ../sorafs/health_frontend_payload.car \
  --json-out ../sorafs/health_frontend_pack_report.json
```
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

    fn fixture_agent_apartment() -> AgentApartmentManifestV1 {
        load_json(&workspace_fixture(DEFAULT_AGENT_APARTMENT_MANIFEST))
            .expect("agent apartment fixture")
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
        assert!(deployed.rollout_handle.is_none());
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
        assert_eq!(upgraded.rollout_stage, Some(RolloutStage::Canary));
        assert_eq!(upgraded.rollout_percent, Some(20));
        assert!(
            upgraded
                .rollout_handle
                .as_ref()
                .is_some_and(|handle| !handle.is_empty())
        );

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
    fn rollout_canary_advances_and_promotes_locally() {
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
        let mut service_v2 = fixture_service();
        service_v2.service_version = "1.1.0".to_string();
        service_v2.container.manifest_hash = container_hash;
        let bundle_v2 = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service: service_v2,
        };

        let mut registry = RegistryState::default();
        apply_mutation(&mut registry, MutationMode::Deploy, &bundle_v1).expect("deploy");
        let upgraded = apply_mutation(&mut registry, MutationMode::Upgrade, &bundle_v2)
            .expect("upgrade should succeed");
        let handle = upgraded
            .rollout_handle
            .as_deref()
            .expect("upgrade should emit rollout handle");

        let canary = apply_rollout(
            &mut registry,
            "web_portal",
            handle,
            true,
            Some(60),
            Hash::new(b"rollout-canary"),
        )
        .expect("canary advance");
        assert_eq!(canary.action, SoracloudAction::Rollout);
        assert_eq!(canary.stage, RolloutStage::Canary);
        assert_eq!(canary.traffic_percent, 60);

        let promoted = apply_rollout(
            &mut registry,
            "web_portal",
            handle,
            true,
            Some(100),
            Hash::new(b"rollout-promoted"),
        )
        .expect("promotion");
        assert_eq!(promoted.action, SoracloudAction::Rollout);
        assert_eq!(promoted.stage, RolloutStage::Promoted);
        assert_eq!(promoted.current_version, "1.1.0");
        assert_eq!(promoted.traffic_percent, 100);

        let service = registry.services.get("web_portal").expect("service exists");
        assert!(service.active_rollout.is_none());
        assert_eq!(
            service.last_rollout.as_ref().map(|rollout| rollout.stage),
            Some(RolloutStage::Promoted)
        );
    }

    #[test]
    fn rollout_auto_rolls_back_after_failure_threshold_locally() {
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
        let mut service_v2 = fixture_service();
        service_v2.service_version = "1.1.0".to_string();
        service_v2.container.manifest_hash = container_hash;
        let bundle_v2 = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service: service_v2,
        };

        let mut registry = RegistryState::default();
        apply_mutation(&mut registry, MutationMode::Deploy, &bundle_v1).expect("deploy");
        let upgraded = apply_mutation(&mut registry, MutationMode::Upgrade, &bundle_v2)
            .expect("upgrade should succeed");
        let handle = upgraded
            .rollout_handle
            .as_deref()
            .expect("upgrade should emit rollout handle");

        for index in 0_u8..2 {
            let response = apply_rollout(
                &mut registry,
                "web_portal",
                handle,
                false,
                None,
                Hash::new(format!("rollout-fail-{index}").as_bytes()),
            )
            .expect("intermediate unhealthy report");
            assert_eq!(response.action, SoracloudAction::Rollout);
            assert_eq!(response.stage, RolloutStage::Canary);
        }

        let rollback = apply_rollout(
            &mut registry,
            "web_portal",
            handle,
            false,
            None,
            Hash::new(b"rollout-fail-terminal"),
        )
        .expect("terminal unhealthy report should rollback");
        assert_eq!(rollback.action, SoracloudAction::Rollback);
        assert_eq!(rollback.stage, RolloutStage::RolledBack);
        assert_eq!(rollback.current_version, "1.0.0");
        assert_eq!(rollback.traffic_percent, 0);
        assert_eq!(registry.audit_log.len(), 5);

        let service = registry.services.get("web_portal").expect("service exists");
        assert_eq!(service.current_version, "1.0.0");
        assert!(service.active_rollout.is_none());
        assert_eq!(
            service.last_rollout.as_ref().map(|rollout| rollout.stage),
            Some(RolloutStage::RolledBack)
        );
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
    fn fetch_torii_agent_autonomy_status_rejects_invalid_url() {
        let err = fetch_torii_soracloud_agent_autonomy_status("not-a-url", "ops_agent", None, 5)
            .expect_err("invalid URL must fail");
        assert!(err.to_string().contains("invalid --torii-url"));
    }

    #[test]
    fn fetch_torii_agent_status_rejects_invalid_url() {
        let err = fetch_torii_soracloud_agent_status("not-a-url", Some("ops_agent"), None, 5)
            .expect_err("invalid URL must fail");
        assert!(err.to_string().contains("invalid --torii-url"));
    }

    #[test]
    fn fetch_torii_agent_mailbox_status_rejects_invalid_url() {
        let err = fetch_torii_soracloud_agent_mailbox_status("not-a-url", "ops_agent", None, 5)
            .expect_err("invalid URL must fail");
        assert!(err.to_string().contains("invalid --torii-url"));
    }

    #[test]
    fn fetch_torii_training_job_status_rejects_invalid_url() {
        let err =
            fetch_torii_soracloud_training_job_status("not-a-url", "web_portal", "job-1", None, 5)
                .expect_err("invalid URL must fail");
        assert!(err.to_string().contains("invalid --torii-url"));
    }

    #[test]
    fn fetch_torii_model_artifact_status_rejects_invalid_url() {
        let err = fetch_torii_soracloud_model_artifact_status(
            "not-a-url",
            "web_portal",
            "job-1",
            None,
            5,
        )
        .expect_err("invalid URL must fail");
        assert!(err.to_string().contains("invalid --torii-url"));
    }

    #[test]
    fn fetch_torii_model_weight_status_rejects_invalid_url() {
        let err = fetch_torii_soracloud_model_weight_status(
            "not-a-url",
            "web_portal",
            "model-v1",
            None,
            5,
        )
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
        let payload = encode_bundle_provenance_payload(&request.bundle).expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_rollback_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_rollback_request("web_portal", None, &key_pair)
            .expect("signed rollback request");
        let payload = encode_rollback_signature_payload(&request.payload).expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_rollout_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_rollout_request(
            "web_portal",
            "web_portal:rollout:2",
            true,
            Some(100),
            Hash::new(b"governance"),
            &key_pair,
        )
        .expect("signed rollout request");
        let payload = encode_rollout_signature_payload(&request.payload).expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_agent_deploy_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_agent_deploy_request(fixture_agent_apartment(), 120, 500, &key_pair)
            .expect("signed agent deploy request");
        let payload =
            encode_agent_deploy_signature_payload(&request.payload).expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_agent_lease_renew_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_agent_lease_renew_request("ops_agent", 120, &key_pair)
            .expect("signed agent lease renew request");
        let payload =
            encode_agent_lease_renew_signature_payload(&request.payload).expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_agent_restart_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_agent_restart_request("ops_agent", "manual-restart", &key_pair)
            .expect("signed agent restart request");
        let payload =
            encode_agent_restart_signature_payload(&request.payload).expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_agent_policy_revoke_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_agent_policy_revoke_request(
            "ops_agent",
            "agent.autonomy.run",
            Some("manual-review"),
            &key_pair,
        )
        .expect("signed agent policy revoke request");
        let payload =
            encode_agent_policy_revoke_signature_payload(&request.payload).expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_agent_wallet_spend_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request =
            signed_agent_wallet_spend_request("ops_agent", "xor#sora", 1_000_000, &key_pair)
                .expect("signed agent wallet spend request");
        let payload =
            encode_agent_wallet_spend_signature_payload(&request.payload).expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_agent_wallet_approve_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request =
            signed_agent_wallet_approve_request("ops_agent", "ops_agent:wallet:7", &key_pair)
                .expect("signed agent wallet approve request");
        let payload = encode_agent_wallet_approve_signature_payload(&request.payload)
            .expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_agent_message_send_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_agent_message_send_request(
            "ops_agent",
            "worker_agent",
            "ops.sync",
            "rotate-key-42",
            &key_pair,
        )
        .expect("signed agent message send request");
        let payload =
            encode_agent_message_send_signature_payload(&request.payload).expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_agent_message_ack_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request =
            signed_agent_message_ack_request("worker_agent", "worker_agent:mail:3", &key_pair)
                .expect("signed agent message ack request");
        let payload =
            encode_agent_message_ack_signature_payload(&request.payload).expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_agent_artifact_allow_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_agent_artifact_allow_request(
            "ops_agent",
            "hash:ABCD0123#01",
            Some("hash:PROV0001#01"),
            &key_pair,
        )
        .expect("signed agent artifact allow request");
        let payload = encode_agent_artifact_allow_signature_payload(&request.payload)
            .expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_agent_autonomy_run_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_agent_autonomy_run_request(
            "ops_agent",
            "hash:ABCD0123#01",
            Some("hash:PROV0001#01"),
            120,
            "nightly-train-step-1",
            &key_pair,
        )
        .expect("signed agent autonomy run request");
        let payload =
            encode_agent_autonomy_run_signature_payload(&request.payload).expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_training_job_start_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_training_job_start_request(
            "web_portal",
            "model-1",
            "job-1",
            4,
            100,
            20,
            3,
            500,
            50_000,
            4_000,
            &key_pair,
        )
        .expect("signed training start request");
        let payload =
            encode_training_job_start_signature_payload(&request.payload).expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_training_job_checkpoint_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_training_job_checkpoint_request(
            "web_portal",
            "job-1",
            20,
            1_024,
            Hash::new(b"metrics"),
            &key_pair,
        )
        .expect("signed training checkpoint request");
        let payload = encode_training_job_checkpoint_signature_payload(&request.payload)
            .expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_training_job_retry_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_training_job_retry_request(
            "web_portal",
            "job-1",
            "worker unavailable",
            &key_pair,
        )
        .expect("signed training retry request");
        let payload =
            encode_training_job_retry_signature_payload(&request.payload).expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_model_artifact_register_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_model_artifact_register_request(
            "web_portal",
            "model-1",
            "job-1",
            Hash::new(b"weight-artifact"),
            "dataset://synthetic/v1",
            Hash::new(b"train-config"),
            Hash::new(b"repro"),
            Hash::new(b"attestation"),
            &key_pair,
        )
        .expect("signed model artifact request");
        let payload = encode_model_artifact_register_signature_payload(&request.payload)
            .expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_model_weight_register_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_model_weight_register_request(
            "web_portal",
            "model-1",
            "1.0.0",
            "job-1",
            Some("0.9.0"),
            Hash::new(b"weight-artifact"),
            "dataset://synthetic/v1",
            Hash::new(b"train-config"),
            Hash::new(b"repro"),
            Hash::new(b"attestation"),
            &key_pair,
        )
        .expect("signed model weight register request");
        let payload = encode_model_weight_register_signature_payload(&request.payload)
            .expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_model_weight_promote_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_model_weight_promote_request(
            "web_portal",
            "model-1",
            "1.0.0",
            true,
            Hash::new(b"gate-report"),
            &key_pair,
        )
        .expect("signed model weight promote request");
        let payload = encode_model_weight_promote_signature_payload(&request.payload)
            .expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn signed_model_weight_rollback_request_uses_verifiable_signature() {
        let key_pair = KeyPair::random();
        let request = signed_model_weight_rollback_request(
            "web_portal",
            "model-1",
            "0.9.0",
            "gate regression",
            &key_pair,
        )
        .expect("signed model weight rollback request");
        let payload = encode_model_weight_rollback_signature_payload(&request.payload)
            .expect("encode payload");
        request
            .provenance
            .signature
            .verify(&request.provenance.signer, &payload)
            .expect("signature should verify");
    }

    #[test]
    fn bundle_signature_payload_layout_is_canonical_layout() {
        let container = fixture_container();
        let mut service = fixture_service();
        service.container.manifest_hash = Hash::new(Encode::encode(&container));
        let bundle = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        };
        let encoded = encode_bundle_provenance_payload(&bundle).expect("encode signature payload");
        let expected = norito::to_bytes(&bundle).expect("encode canonical layout");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn rollback_signature_payload_layout_is_canonical_tuple() {
        let payload = RollbackPayload {
            service_name: "web_portal".to_owned(),
            target_version: Some("1.0.0".to_owned()),
        };
        let encoded =
            encode_rollback_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.target_version.as_deref(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn rollout_signature_payload_layout_is_canonical_tuple() {
        let governance_tx_hash = Hash::new(b"governance");
        let payload = RolloutAdvancePayload {
            service_name: "web_portal".to_owned(),
            rollout_handle: "web_portal:rollout:2".to_owned(),
            healthy: true,
            promote_to_percent: Some(100),
            governance_tx_hash: governance_tx_hash.clone(),
        };
        let encoded = encode_rollout_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.rollout_handle.as_str(),
            payload.healthy,
            payload.promote_to_percent,
            governance_tx_hash,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_deploy_signature_payload_layout_is_canonical_tuple() {
        let manifest = fixture_agent_apartment();
        let payload = AgentDeployPayload {
            manifest: manifest.clone(),
            lease_ticks: 120,
            autonomy_budget_units: Some(500),
        };
        let encoded =
            encode_agent_deploy_signature_payload(&payload).expect("encode signature payload");
        let expected =
            norito::to_bytes(&(manifest, 120u64, Some(500u64))).expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_lease_renew_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentLeaseRenewPayload {
            apartment_name: "ops_agent".to_owned(),
            lease_ticks: 120,
        };
        let encoded =
            encode_agent_lease_renew_signature_payload(&payload).expect("encode signature payload");
        let expected = norito::to_bytes(&(payload.apartment_name.as_str(), payload.lease_ticks))
            .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_restart_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentRestartPayload {
            apartment_name: "ops_agent".to_owned(),
            reason: "manual-restart".to_owned(),
        };
        let encoded =
            encode_agent_restart_signature_payload(&payload).expect("encode signature payload");
        let expected =
            norito::to_bytes(&(payload.apartment_name.as_str(), payload.reason.as_str()))
                .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_policy_revoke_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentPolicyRevokePayload {
            apartment_name: "ops_agent".to_owned(),
            capability: "agent.autonomy.run".to_owned(),
            reason: Some("manual-review".to_owned()),
        };
        let encoded = encode_agent_policy_revoke_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.apartment_name.as_str(),
            payload.capability.as_str(),
            payload.reason.as_deref(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_wallet_spend_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentWalletSpendPayload {
            apartment_name: "ops_agent".to_owned(),
            asset_definition: "xor#sora".to_owned(),
            amount_nanos: 1_000_000,
        };
        let encoded = encode_agent_wallet_spend_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.apartment_name.as_str(),
            payload.asset_definition.as_str(),
            payload.amount_nanos,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_wallet_approve_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentWalletApprovePayload {
            apartment_name: "ops_agent".to_owned(),
            request_id: "ops_agent:wallet:7".to_owned(),
        };
        let encoded = encode_agent_wallet_approve_signature_payload(&payload)
            .expect("encode signature payload");
        let expected =
            norito::to_bytes(&(payload.apartment_name.as_str(), payload.request_id.as_str()))
                .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_message_send_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentMessageSendPayload {
            from_apartment: "ops_agent".to_owned(),
            to_apartment: "worker_agent".to_owned(),
            channel: "ops.sync".to_owned(),
            payload: "rotate-key-42".to_owned(),
        };
        let encoded = encode_agent_message_send_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.from_apartment.as_str(),
            payload.to_apartment.as_str(),
            payload.channel.as_str(),
            payload.payload.as_str(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_message_ack_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentMessageAckPayload {
            apartment_name: "worker_agent".to_owned(),
            message_id: "worker_agent:mail:3".to_owned(),
        };
        let encoded =
            encode_agent_message_ack_signature_payload(&payload).expect("encode signature payload");
        let expected =
            norito::to_bytes(&(payload.apartment_name.as_str(), payload.message_id.as_str()))
                .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_artifact_allow_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentArtifactAllowPayload {
            apartment_name: "ops_agent".to_owned(),
            artifact_hash: "hash:ABCD0123#01".to_owned(),
            provenance_hash: Some("hash:PROV0001#01".to_owned()),
        };
        let encoded = encode_agent_artifact_allow_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.apartment_name.as_str(),
            payload.artifact_hash.as_str(),
            payload.provenance_hash.as_deref(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_autonomy_run_signature_payload_layout_is_canonical_tuple() {
        let payload = AgentAutonomyRunPayload {
            apartment_name: "ops_agent".to_owned(),
            artifact_hash: "hash:ABCD0123#01".to_owned(),
            provenance_hash: Some("hash:PROV0001#01".to_owned()),
            budget_units: 120,
            run_label: "nightly-train-step-1".to_owned(),
        };
        let encoded = encode_agent_autonomy_run_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.apartment_name.as_str(),
            payload.artifact_hash.as_str(),
            payload.provenance_hash.as_deref(),
            payload.budget_units,
            payload.run_label.as_str(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn training_job_start_signature_payload_layout_is_canonical_tuple() {
        let payload = TrainingJobStartPayload {
            service_name: "web_portal".to_owned(),
            model_name: "model-1".to_owned(),
            job_id: "job-1".to_owned(),
            worker_group_size: 4,
            target_steps: 100,
            checkpoint_interval_steps: 20,
            max_retries: 3,
            step_compute_units: 500,
            compute_budget_units: 50_000,
            storage_budget_bytes: 4_096,
        };
        let encoded = encode_training_job_start_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.model_name.as_str(),
            payload.job_id.as_str(),
            payload.worker_group_size,
            payload.target_steps,
            payload.checkpoint_interval_steps,
            payload.max_retries,
            payload.step_compute_units,
            payload.compute_budget_units,
            payload.storage_budget_bytes,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn training_job_checkpoint_signature_payload_layout_is_canonical_tuple() {
        let metrics_hash = Hash::new(b"metrics");
        let payload = TrainingJobCheckpointPayload {
            service_name: "web_portal".to_owned(),
            job_id: "job-1".to_owned(),
            completed_step: 20,
            checkpoint_size_bytes: 1_024,
            metrics_hash: metrics_hash.clone(),
        };
        let encoded = encode_training_job_checkpoint_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.job_id.as_str(),
            payload.completed_step,
            payload.checkpoint_size_bytes,
            metrics_hash,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn training_job_retry_signature_payload_layout_is_canonical_tuple() {
        let payload = TrainingJobRetryPayload {
            service_name: "web_portal".to_owned(),
            job_id: "job-1".to_owned(),
            reason: "worker unavailable".to_owned(),
        };
        let encoded = encode_training_job_retry_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.job_id.as_str(),
            payload.reason.as_str(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn model_artifact_register_signature_payload_layout_is_canonical_tuple() {
        let weight_artifact_hash = Hash::new(b"weight-artifact");
        let training_config_hash = Hash::new(b"train-config");
        let reproducibility_hash = Hash::new(b"repro");
        let provenance_attestation_hash = Hash::new(b"attestation");
        let payload = ModelArtifactRegisterPayload {
            service_name: "web_portal".to_owned(),
            model_name: "model-1".to_owned(),
            training_job_id: "job-1".to_owned(),
            weight_artifact_hash: weight_artifact_hash.clone(),
            dataset_ref: "dataset://synthetic/v1".to_owned(),
            training_config_hash: training_config_hash.clone(),
            reproducibility_hash: reproducibility_hash.clone(),
            provenance_attestation_hash: provenance_attestation_hash.clone(),
        };
        let encoded = encode_model_artifact_register_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.model_name.as_str(),
            payload.training_job_id.as_str(),
            weight_artifact_hash,
            payload.dataset_ref.as_str(),
            training_config_hash,
            reproducibility_hash,
            provenance_attestation_hash,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn model_weight_register_signature_payload_layout_is_canonical_tuple() {
        let weight_artifact_hash = Hash::new(b"weight-artifact");
        let training_config_hash = Hash::new(b"train-config");
        let reproducibility_hash = Hash::new(b"repro");
        let provenance_attestation_hash = Hash::new(b"attestation");
        let payload = ModelWeightRegisterPayload {
            service_name: "web_portal".to_owned(),
            model_name: "model-1".to_owned(),
            weight_version: "1.0.0".to_owned(),
            training_job_id: "job-1".to_owned(),
            parent_version: Some("0.9.0".to_owned()),
            weight_artifact_hash: weight_artifact_hash.clone(),
            dataset_ref: "dataset://synthetic/v1".to_owned(),
            training_config_hash: training_config_hash.clone(),
            reproducibility_hash: reproducibility_hash.clone(),
            provenance_attestation_hash: provenance_attestation_hash.clone(),
        };
        let encoded = encode_model_weight_register_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.model_name.as_str(),
            payload.weight_version.as_str(),
            payload.training_job_id.as_str(),
            payload.parent_version.as_deref(),
            weight_artifact_hash,
            payload.dataset_ref.as_str(),
            training_config_hash,
            reproducibility_hash,
            provenance_attestation_hash,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn model_weight_promote_signature_payload_layout_is_canonical_tuple() {
        let gate_report_hash = Hash::new(b"gate-report");
        let payload = ModelWeightPromotePayload {
            service_name: "web_portal".to_owned(),
            model_name: "model-1".to_owned(),
            weight_version: "1.0.0".to_owned(),
            gate_approved: true,
            gate_report_hash: gate_report_hash.clone(),
        };
        let encoded = encode_model_weight_promote_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.model_name.as_str(),
            payload.weight_version.as_str(),
            payload.gate_approved,
            gate_report_hash,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn model_weight_rollback_signature_payload_layout_is_canonical_tuple() {
        let payload = ModelWeightRollbackPayload {
            service_name: "web_portal".to_owned(),
            model_name: "model-1".to_owned(),
            target_version: "0.9.0".to_owned(),
            reason: "gate regression".to_owned(),
        };
        let encoded = encode_model_weight_rollback_signature_payload(&payload)
            .expect("encode signature payload");
        let expected = norito::to_bytes(&(
            payload.service_name.as_str(),
            payload.model_name.as_str(),
            payload.target_version.as_str(),
            payload.reason.as_str(),
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);
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

    #[test]
    fn init_health_app_template_scaffolds_private_policy_workflows() {
        let dir = temp_dir("health_app_template");
        let output = InitArgs {
            output_dir: dir.clone(),
            service_name: "clinic_console".to_owned(),
            service_version: "1.0.0".to_owned(),
            template: InitTemplate::HealthApp,
            overwrite: false,
        }
        .run()
        .expect("health-app init should succeed");

        assert_eq!(output.template, "health-app");
        assert!(dir.join("health-app/frontend/package.json").exists());
        assert!(dir.join("health-app/api/server.mjs").exists());
        assert!(
            dir.join("health-app/policy/consent_policy_template.json")
                .exists()
        );
        assert!(
            dir.join("health-app/policy/retention_policy_template.json")
                .exists()
        );
        assert!(
            dir.join("health-app/policy/deletion_workflow_template.json")
                .exists()
        );

        let readme =
            fs::read_to_string(dir.join("health-app/README.md")).expect("read health app readme");
        assert!(readme.contains("consent_policy_template.json"));
        assert!(readme.contains("retention_policy_template.json"));
        assert!(readme.contains("deletion_workflow_template.json"));

        let service: SoraServiceManifestV1 =
            load_json(&dir.join("service_manifest.json")).expect("service manifest");
        assert_eq!(
            service
                .route
                .as_ref()
                .map(|route| route.path_prefix.as_str()),
            Some("/health/api")
        );
        assert!(
            service.state_bindings.iter().any(|binding| {
                binding.binding_name.as_ref() == "patient_records"
                    && binding.encryption == SoraStateEncryptionV1::FheCiphertext
                    && binding.key_prefix == "/state/health/patient"
            }),
            "patient_records private binding missing from health-app template"
        );
        assert!(
            service.state_bindings.iter().any(|binding| {
                binding.binding_name.as_ref() == "consent_events"
                    && binding.key_prefix == "/state/health/consent"
            }),
            "consent_events binding missing from health-app template"
        );
    }

    #[test]
    fn agent_apartment_scheduler_deploy_renew_restart_updates_registry() {
        let manifest = fixture_agent_apartment();
        let apartment_name = manifest.apartment_name.to_string();
        let mut registry = RegistryState::default();

        let deployed =
            apply_agent_deploy(&mut registry, &manifest, 10).expect("agent deploy should succeed");
        assert_eq!(deployed.action, AgentApartmentAction::Deploy);
        assert_eq!(deployed.apartment_name, apartment_name);
        assert_eq!(deployed.status, AgentRuntimeStatus::Running);

        let renewed = apply_agent_lease_renew(&mut registry, "ops_agent", 5)
            .expect("lease renewal should succeed");
        assert_eq!(renewed.action, AgentApartmentAction::LeaseRenew);
        assert_eq!(renewed.status, AgentRuntimeStatus::Running);

        let restarted = apply_agent_restart(&mut registry, "ops_agent", "policy refresh")
            .expect("restart should succeed");
        assert_eq!(restarted.action, AgentApartmentAction::Restart);
        assert_eq!(restarted.restart_count, 1);
        assert_eq!(restarted.reason.as_deref(), Some("policy refresh"));
        assert_eq!(registry.apartment_events.len(), 3);

        let runtime = registry
            .apartments
            .get("ops_agent")
            .expect("runtime should exist");
        assert_eq!(runtime.restart_count, 1);
        assert_eq!(
            runtime.last_restart_reason.as_deref(),
            Some("policy refresh")
        );
    }

    #[test]
    fn agent_apartment_deploy_rejects_duplicate_name() {
        let manifest = fixture_agent_apartment();
        let mut registry = RegistryState::default();
        apply_agent_deploy(&mut registry, &manifest, 10).expect("initial deploy");
        let err = apply_agent_deploy(&mut registry, &manifest, 10)
            .expect_err("duplicate deploy must fail");
        assert!(err.to_string().contains("already registered"));
    }

    #[test]
    fn agent_apartment_restart_rejects_expired_lease() {
        let manifest = fixture_agent_apartment();
        let mut registry = RegistryState::default();
        apply_agent_deploy(&mut registry, &manifest, 1).expect("initial deploy");
        let err = apply_agent_restart(&mut registry, "ops_agent", "manual recover")
            .expect_err("restart after lease expiry must fail");
        assert!(err.to_string().contains("lease expired"));
    }

    #[test]
    fn agent_apartment_status_entry_reports_lease_expiry() {
        let manifest = fixture_agent_apartment();
        let mut registry = RegistryState::default();
        apply_agent_deploy(&mut registry, &manifest, 1).expect("initial deploy");
        let runtime = registry
            .apartments
            .get("ops_agent")
            .expect("runtime should exist");
        let status =
            AgentApartmentStatusEntry::from_state("ops_agent", runtime, registry.next_sequence);
        assert_eq!(status.status, AgentRuntimeStatus::LeaseExpired);
        assert_eq!(status.lease_remaining_ticks, 0);
    }

    #[test]
    fn agent_wallet_spend_requests_then_approves_under_policy() {
        let manifest = fixture_agent_apartment();
        let mut registry = RegistryState::default();
        apply_agent_deploy(&mut registry, &manifest, 120).expect("initial deploy");

        let request = apply_agent_wallet_spend(&mut registry, "ops_agent", "xor#sora", 1_000_000)
            .expect("wallet spend request should succeed");
        assert_eq!(request.action, AgentApartmentAction::WalletSpendRequested);
        assert_eq!(request.pending_request_count, 1);
        let request_id = request.request_id.clone().expect("request id");

        let approved = apply_agent_wallet_approve(&mut registry, "ops_agent", &request_id)
            .expect("approval should succeed");
        assert_eq!(approved.action, AgentApartmentAction::WalletSpendApproved);
        assert_eq!(approved.pending_request_count, 0);
        assert_eq!(approved.day_spent_nanos, Some(1_000_000));
    }

    #[test]
    fn agent_wallet_auto_approve_applies_daily_spend() {
        let mut manifest = fixture_agent_apartment();
        manifest.policy_capabilities.push(
            "wallet.auto_approve"
                .parse()
                .expect("valid capability name"),
        );
        manifest.validate().expect("manifest should remain valid");

        let mut registry = RegistryState::default();
        apply_agent_deploy(&mut registry, &manifest, 120).expect("initial deploy");

        let approved = apply_agent_wallet_spend(&mut registry, "ops_agent", "usd#bank", 2_000_000)
            .expect("auto approval should succeed");
        assert_eq!(approved.action, AgentApartmentAction::WalletSpendApproved);
        assert_eq!(approved.pending_request_count, 0);
        assert_eq!(approved.day_spent_nanos, Some(2_000_000));
    }

    #[test]
    fn agent_wallet_spend_rejects_when_wallet_sign_revoked() {
        let manifest = fixture_agent_apartment();
        let mut registry = RegistryState::default();
        apply_agent_deploy(&mut registry, &manifest, 120).expect("initial deploy");
        apply_agent_policy_revoke(&mut registry, "ops_agent", "wallet.sign", Some("rotated"))
            .expect("revoke should succeed");

        let err = apply_agent_wallet_spend(&mut registry, "ops_agent", "xor#sora", 1_000_000)
            .expect_err("wallet spend should be rejected");
        assert!(err.to_string().contains("wallet.sign"));
    }

    #[test]
    fn agent_wallet_spend_rejects_amount_above_per_tx_limit() {
        let manifest = fixture_agent_apartment();
        let mut registry = RegistryState::default();
        apply_agent_deploy(&mut registry, &manifest, 120).expect("initial deploy");

        let err = apply_agent_wallet_spend(&mut registry, "ops_agent", "xor#sora", 6_000_000)
            .expect_err("request should exceed per-tx limit");
        assert!(err.to_string().contains("max_per_tx_nanos"));
    }

    #[test]
    fn agent_mailbox_send_and_ack_flow_updates_queue_and_events() {
        let mut sender = fixture_agent_apartment();
        sender
            .policy_capabilities
            .push("agent.mailbox.send".parse().expect("valid capability"));
        sender.validate().expect("sender manifest should be valid");

        let mut recipient = fixture_agent_apartment();
        recipient.apartment_name = "worker_agent".parse().expect("valid apartment name");
        recipient
            .policy_capabilities
            .push("agent.mailbox.receive".parse().expect("valid capability"));
        recipient
            .validate()
            .expect("recipient manifest should be valid");

        let mut registry = RegistryState::default();
        apply_agent_deploy(&mut registry, &sender, 120).expect("deploy sender");
        apply_agent_deploy(&mut registry, &recipient, 120).expect("deploy recipient");

        let queued = apply_agent_message_send(
            &mut registry,
            "ops_agent",
            "worker_agent",
            "ops.sync",
            "rotate-key-42",
        )
        .expect("message send should succeed");
        assert_eq!(queued.action, AgentApartmentAction::MessageEnqueued);
        assert_eq!(queued.pending_message_count, 1);
        assert_eq!(queued.to_apartment.as_deref(), Some("worker_agent"));
        let message_id = queued.message_id.clone();

        let recipient_runtime = registry
            .apartments
            .get("worker_agent")
            .expect("recipient runtime exists");
        assert_eq!(recipient_runtime.mailbox_queue.len(), 1);
        assert_eq!(recipient_runtime.mailbox_queue[0].message_id, message_id);

        let acked = apply_agent_message_ack(&mut registry, "worker_agent", &message_id)
            .expect("message ack should succeed");
        assert_eq!(acked.action, AgentApartmentAction::MessageAcknowledged);
        assert_eq!(acked.pending_message_count, 0);
        assert_eq!(acked.from_apartment.as_deref(), Some("ops_agent"));

        let recipient_runtime = registry
            .apartments
            .get("worker_agent")
            .expect("recipient runtime exists");
        assert!(recipient_runtime.mailbox_queue.is_empty());
        assert_eq!(registry.apartment_events.len(), 4);
    }

    #[test]
    fn agent_mailbox_send_rejects_without_sender_capability() {
        let sender = fixture_agent_apartment();
        let mut recipient = fixture_agent_apartment();
        recipient.apartment_name = "worker_agent".parse().expect("valid apartment name");
        recipient
            .policy_capabilities
            .push("agent.mailbox.receive".parse().expect("valid capability"));
        recipient
            .validate()
            .expect("recipient manifest should be valid");

        let mut registry = RegistryState::default();
        apply_agent_deploy(&mut registry, &sender, 120).expect("deploy sender");
        apply_agent_deploy(&mut registry, &recipient, 120).expect("deploy recipient");

        let err = apply_agent_message_send(
            &mut registry,
            "ops_agent",
            "worker_agent",
            "ops.sync",
            "rotate-key-42",
        )
        .expect_err("send without sender capability should fail");
        assert!(err.to_string().contains("agent.mailbox.send"));
    }

    #[test]
    fn agent_autonomy_allow_and_run_flow_updates_budget_and_history() {
        let mut manifest = fixture_agent_apartment();
        manifest
            .policy_capabilities
            .push("agent.autonomy.run".parse().expect("valid capability"));
        manifest.validate().expect("manifest should remain valid");

        let mut registry = RegistryState::default();
        apply_agent_deploy_with_budget(&mut registry, &manifest, 120, 1_000)
            .expect("initial deploy");

        let allow = apply_agent_artifact_allow(
            &mut registry,
            "ops_agent",
            "hash:ABCD0123#01",
            Some("hash:PROV0001#01"),
        )
        .expect("artifact allow should succeed");
        assert_eq!(allow.action, AgentApartmentAction::ArtifactAllowed);
        assert_eq!(allow.allowlist_count, 1);
        assert_eq!(allow.budget_remaining_units, 1_000);

        let run = apply_agent_autonomy_run(
            &mut registry,
            "ops_agent",
            "hash:ABCD0123#01",
            Some("hash:PROV0001#01"),
            250,
            "nightly-train-step-1",
        )
        .expect("autonomy run should succeed");
        assert_eq!(run.action, AgentApartmentAction::AutonomyRunApproved);
        assert_eq!(run.budget_units, Some(250));
        assert_eq!(run.budget_remaining_units, 750);
        assert!(run.run_id.is_some());

        let runtime = registry
            .apartments
            .get("ops_agent")
            .expect("runtime should exist");
        assert_eq!(runtime.autonomy_budget_remaining_units, 750);
        assert_eq!(runtime.artifact_allowlist.len(), 1);
        assert_eq!(runtime.autonomy_run_history.len(), 1);
        assert_eq!(
            runtime.autonomy_run_history[0].run_label,
            "nightly-train-step-1"
        );
    }

    #[test]
    fn agent_autonomy_run_rejects_without_allowlist_and_on_budget_overflow() {
        let mut manifest = fixture_agent_apartment();
        manifest
            .policy_capabilities
            .push("agent.autonomy.run".parse().expect("valid capability"));
        manifest.validate().expect("manifest should remain valid");

        let mut registry = RegistryState::default();
        apply_agent_deploy_with_budget(&mut registry, &manifest, 120, 100).expect("initial deploy");

        let missing_allowlist = apply_agent_autonomy_run(
            &mut registry,
            "ops_agent",
            "hash:ABCD0123#01",
            None,
            10,
            "no-allowlist",
        )
        .expect_err("run must fail without allowlist");
        assert!(missing_allowlist.to_string().contains("not allowlisted"));

        apply_agent_artifact_allow(&mut registry, "ops_agent", "hash:ABCD0123#01", None)
            .expect("allowlist insert should succeed");

        let budget_err = apply_agent_autonomy_run(
            &mut registry,
            "ops_agent",
            "hash:ABCD0123#01",
            None,
            200,
            "budget-overflow",
        )
        .expect_err("run should fail when budget exceeds remaining");
        assert!(
            budget_err
                .to_string()
                .contains("exceeds remaining autonomy budget")
        );
    }
}
