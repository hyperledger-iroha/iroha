//! Durable native deployment of one physical dataspace and its paid namespace.
//!
//! A phase is submitted at most once from this journal. An uncertain submission is
//! reconciled by its retained signed transaction; it is never replaced or retried.
//! Applied observations are not a finality proof or a deployment-complete claim.

use crate::{Run, RunContext, quote_and_sign_transaction_with_admission};
use eyre::{Result, WrapErr, eyre};
use iroha::{blocking::Client as BlockingClient, client::Client, sns::SnsNamespacePath};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    alias_setup::{
        AccountAliasRoleV1, AccountProvisionV1, AliasDataspaceBootstrapGrantV1, AliasIntentV1,
        AliasPlanDispositionV1, AliasSetupPlanRequestV1, AliasTransactionPlanV1,
    },
    asset::AssetDefinitionId,
    isi::{InstructionBox, SetParameter},
    nexus::{
        LaneConfig, LaneLifecycleStatusV1, NexusCatalogTransitionV1, NexusRuntimeCatalogV1,
        RuntimeDataSpaceAdditionV1, RuntimeLaneManifestV1,
    },
    parameter::{Parameter, Parameters},
    transaction::{
        Executable, FeePaymentIntent, SignedTransaction, TransactionAdmissionIntent,
        signed::{FeeChargeKind, TransactionEntrypoint},
    },
};
use iroha_model_base::metadata::Metadata;
use iroha_primitives::numeric::Quantity;
use iroha_torii_shared::{
    FeeQuoteResponse, PipelineTransactionDetailsResponse, PipelineTransactionStatusResponse,
};
use iroha_version::codec::DecodeVersioned as _;
use norito::json::{self, JsonDeserialize, JsonSerialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[path = "taira_dataspace_deploy_finality.rs"]
mod finality;
#[path = "taira_dataspace_deploy_manifest.rs"]
mod lane_manifest;
#[path = "taira_dataspace_deploy_profile.rs"]
mod profile;

pub(crate) use finality::authenticated_height::{
    AuthenticatedHeightObserverV1, HeightObservationV1, VerifiedCommittedHeightV1,
};
pub(crate) use finality::{PeerV1 as DeploymentPeerV1, TrustV1 as DeploymentTrustV1};

pub(crate) fn validate_deployment_trust(
    trust: &DeploymentTrustV1,
    network: NetworkId,
) -> Result<()> {
    trust.validate(network)
}

const MAX_BYTES: usize = 8 * 1024 * 1024;
const PHASES: [&str; 3] = ["catalog", "bootstrap", "aliases"];
const DEFAULT_OPERATION_TIMEOUT_MS: u64 = 180_000;

/// Plan, advance, or inspect a single durable dataspace deployment.
#[derive(Debug, clap::Subcommand)]
pub(crate) enum Command {
    /// Export retained-network expectations from independently selected public inputs.
    ExportProfile(profile::ExportProfile),
    /// Generate native deployment intent from signed genesis and current namespace policies.
    Init(InitArgs),
    /// Read-only live readiness check of a complete deployment manifest; retain no operation.
    Preflight(PreflightArgs),
    /// Resume one fixed deployment request through the existing init, plan and saved apply journal.
    Ensure(EnsureArgs),
    /// Validate live capabilities and the exact intent, then retain an immutable plan.
    Plan(PlanArgs),
    /// Advance the saved plan within one budget; uncertain submissions are only observed again.
    Apply(SavedArgs),
    /// Read the exact saved transactions and current observations without submitting.
    Status(SavedArgs),
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub(crate) enum LaneProfile {
    RestrictedFullReplica,
    PublicFullReplica,
}

#[derive(Debug, clap::Args)]
pub(crate) struct InitArgs {
    #[arg(long)]
    dataspace: String,
    #[arg(long)]
    lane_id: u32,
    #[arg(long, value_enum)]
    lane_profile: LaneProfile,
    #[arg(long)]
    account_alias: String,
    #[arg(long)]
    trust: PathBuf,
    #[arg(long)]
    payment_asset: AssetDefinitionId,
    #[arg(long)]
    alias_create_maximum: Quantity,
    #[arg(long)]
    transaction_fee_maximum: Quantity,
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..))]
    lease_years: u8,
    #[arg(long, default_value_t = 3600, value_parser = clap::value_parser!(u64).range(1..=86400))]
    quote_lifetime_secs: u64,
    #[arg(long)]
    operation_id: Option<String>,
    /// Fresh owner-private bundle; plan consumes its deployment.json file.
    #[arg(long)]
    output_dir: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct PlanArgs {
    #[arg(long)]
    manifest: PathBuf,
    /// Existing owner-private directory containing operation-ID subdirectories.
    #[arg(long)]
    journal_dir: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct PreflightArgs {
    #[arg(long)]
    manifest: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct EnsureArgs {
    /// Owner-private V1 request; operation identity and all first-run inputs are fixed here.
    #[arg(long)]
    request: PathBuf,
    /// Budget for the saved apply or status pass; this does not change operation identity.
    #[arg(long, default_value_t = 600_000,
          value_parser = clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SavedArgs {
    #[arg(long)]
    journal_dir: PathBuf,
    #[arg(long)]
    operation_id: String,
    /// Total budget for preflight, retained phases and fresh four-validator verification.
    #[arg(long, default_value_t = DEFAULT_OPERATION_TIMEOUT_MS,
          value_parser = clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,
}

fn operation_deadline(timeout_ms: u64) -> Result<Instant> {
    require(timeout_ms > 0, "--timeout-ms must be greater than zero")?;
    Instant::now()
        .checked_add(Duration::from_millis(timeout_ms))
        .ok_or_else(|| eyre!("dataspace deployment deadline overflow"))
}

fn require_operation_budget(deadline: Instant, stage: &str) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("dataspace deployment deadline elapsed during {stage}"),
        )
        .into());
    }
    Ok(())
}

fn operation_poll_delay(deadline: Instant, now: Instant) -> Duration {
    Duration::from_millis(500).min(deadline.saturating_duration_since(now))
}

fn observe_phase_until(
    apply: bool,
    deadline: Instant,
    phase: &str,
    mut observe_retained: impl FnMut() -> Result<PhaseObservationV1>,
) -> Result<PhaseObservationV1> {
    let stage = format!("phase {phase} observation");
    loop {
        require_operation_budget(deadline, &stage)?;
        let observation = observe_retained()?;
        require_operation_budget(deadline, &stage)?;
        if !apply || observation.state != "pending" {
            return Ok(observation);
        }
        std::thread::sleep(operation_poll_delay(deadline, Instant::now()));
    }
}

fn complete_until(
    apply: bool,
    deadline: Instant,
    report: &mut ReportV1,
    mut verify: impl FnMut(&mut ReportV1) -> Result<()>,
) -> Result<()> {
    loop {
        require_operation_budget(deadline, "four-validator verification")?;
        verify(report)?;
        require_operation_budget(deadline, "four-validator verification")?;
        if !apply
            || !matches!(
                report.state.as_str(),
                "verification_sync_pending" | "verification_peer_pending"
            )
        {
            return Ok(());
        }
        // Only explicit proof-sync or validated peer progress is retryable.
        // Malformed proofs, changed identities and other verification errors stop.
        std::thread::sleep(operation_poll_delay(deadline, Instant::now()));
    }
}

/// One first-release intent, expressed entirely in maintained native model types.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ManifestV1 {
    pub(crate) schema_version: u8,
    #[norito(required)]
    pub(crate) operation_id: Option<String>,
    pub(crate) network_id: NetworkId,
    pub(crate) owner: AccountId,
    pub(crate) dataspace: RuntimeDataSpaceAdditionV1,
    pub(crate) lane: LaneConfig,
    pub(crate) lane_manifest: RuntimeLaneManifestV1,
    pub(crate) alias_request: AliasSetupPlanRequestV1,
    pub(crate) spending: SpendingV1,
    pub(crate) finality: finality::TrustV1,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct SpendingV1 {
    pub(crate) asset_definition_id: AssetDefinitionId,
    pub(crate) alias_create_maximum: Quantity,
    pub(crate) transaction_fee_maximum: Quantity,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct EnsureRequestV1 {
    schema: String,
    operation_id: String,
    network_id: NetworkId,
    owner: AccountId,
    dataspace: String,
    lane_id: u32,
    /// `restricted_full_replica` or `public_full_replica`.
    lane_profile: String,
    account_alias: String,
    trust: PathBuf,
    trust_sha256: String,
    payment_asset: AssetDefinitionId,
    alias_create_maximum: Quantity,
    transaction_fee_maximum: Quantity,
    lease_years: u8,
    manifest_dir: PathBuf,
    journal_dir: PathBuf,
}

/// Durable first-run selection and the exact manifest produced from native policies.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct EnsureBindingV1 {
    schema: String,
    request_sha256: String,
    manifest_sha256: String,
    manifest: ManifestV1,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PlanV1 {
    schema_version: u8,
    operation_id: String,
    intent_sha256: String,
    manifest: ManifestV1,
    baseline: LaneLifecycleStatusV1,
    #[norito(required)]
    baseline_overlay: Option<NexusRuntimeCatalogV1>,
    catalog_transition: NexusCatalogTransitionV1,
    bootstrap_grant: AliasDataspaceBootstrapGrantV1,
    initial_alias_plan: AliasTransactionPlanV1,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PreparedV1 {
    schema_version: u8,
    operation_id: String,
    intent_sha256: String,
    phase: String,
    signed_transaction_wire_hex: String,
    transaction_hash: String,
    instructions: Vec<InstructionBox>,
    fee_quote: FeeQuoteResponse,
    #[norito(required)]
    alias_plan: Option<AliasTransactionPlanV1>,
}

/// Inputs for the separate anchored finality, inclusion, and four-peer verifier.
/// This is a request for verification, not evidence that those checks ran.
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct VerificationRequestV1 {
    pub(crate) schema_version: u8,
    pub(crate) operation_id: String,
    pub(crate) intent_sha256: String,
    pub(crate) network_id: NetworkId,
    pub(crate) owner: AccountId,
    pub(crate) dataspace: RuntimeDataSpaceAdditionV1,
    pub(crate) lane: LaneConfig,
    pub(crate) lane_manifest: RuntimeLaneManifestV1,
    pub(crate) bootstrap_grant: AliasDataspaceBootstrapGrantV1,
    pub(crate) alias_request: AliasSetupPlanRequestV1,
    pub(crate) baseline: LaneLifecycleStatusV1,
    #[norito(required)]
    pub(crate) baseline_overlay: Option<NexusRuntimeCatalogV1>,
    pub(crate) transactions: Vec<PhaseObservationV1>,
    pub(crate) independently_anchored_finality_required: bool,
    pub(crate) authenticated_execution_commitment_required: bool,
    pub(crate) all_four_state_observations_required: bool,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct PhaseObservationV1 {
    pub(crate) phase: String,
    pub(crate) state: String,
    #[norito(required)]
    pub(crate) transaction_hash: Option<String>,
    pub(crate) instructions: Vec<InstructionBox>,
    pub(crate) signed_transaction_wire_sha256: String,
    #[norito(required)]
    pub(crate) alias_plan: Option<AliasTransactionPlanV1>,
    #[norito(required)]
    pub(crate) global_status: Option<PipelineTransactionStatusResponse>,
    #[norito(required)]
    pub(crate) peer_status: Option<PipelineTransactionStatusResponse>,
    #[norito(required)]
    pub(crate) committed: Option<PipelineTransactionDetailsResponse>,
}

#[derive(Debug, Clone, JsonSerialize)]
struct ReportV1 {
    schema_version: u8,
    operation_id: String,
    state: String,
    deployment_complete: bool,
    #[norito(required)]
    verification_error: Option<String>,
    #[norito(required)]
    completion_receipt: Option<String>,
    verification: VerificationRequestV1,
}

/// A closed success record; failures return an error and never claim readiness.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PreflightReportV1 {
    schema: String,
    operation_id: String,
    manifest_sha256: String,
    intent_sha256: String,
    state: String,
    deployment_complete: bool,
    manifest_and_profile_verified: bool,
    validator_finality_routes_verified: bool,
    capabilities_and_owner_permissions_verified: bool,
    catalog_and_namespace_available: bool,
    funding_caps_covered: bool,
    native_alias_quotes_verified: bool,
}

fn preflight_report(plan: &PlanV1, manifest_sha256: String) -> PreflightReportV1 {
    PreflightReportV1 {
        schema: "iroha.taira.dataspace-deploy.preflight.v1".into(),
        operation_id: plan.operation_id.clone(),
        manifest_sha256,
        intent_sha256: plan.intent_sha256.clone(),
        state: "ready_for_plan".into(),
        deployment_complete: false,
        manifest_and_profile_verified: true,
        validator_finality_routes_verified: true,
        capabilities_and_owner_permissions_verified: true,
        catalog_and_namespace_available: true,
        funding_caps_covered: true,
        native_alias_quotes_verified: true,
    }
}

fn require(condition: bool, message: &str) -> Result<()> {
    if !condition {
        eyre::bail!("{message}");
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn operation_id(value: &str) -> Result<()> {
    require(
        !value.is_empty()
            && value.len() <= 96
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "operation ID must contain 1..96 ASCII letters, digits, '-' or '_'",
    )
}

impl ManifestV1 {
    fn validate(&self) -> Result<AliasDataspaceBootstrapGrantV1> {
        require(
            self.schema_version == 1,
            "unsupported dataspace manifest version",
        )?;
        self.finality.validate(self.network_id)?;
        if let Some(id) = &self.operation_id {
            operation_id(id)?;
        }
        self.dataspace.validate_structure()?;
        self.lane_manifest.validate_structure()?;
        iroha_core::governance::manifest::LaneManifestRegistry::validate_runtime_manifest(
            &self.lane_manifest,
            &self.lane,
            &self.dataspace.descriptor,
            &iroha_config::parameters::actual::GovernanceCatalog::default(),
        )
        .map_err(|error| eyre!("invalid native lane manifest: {error}"))?;
        require(
            self.lane_manifest.manifest
                == lane_manifest::generate(&self.lane.alias, &self.finality)?,
            "native lane manifest differs from the selected genesis committee and peer endpoints",
        )?;
        let grant = AliasDataspaceBootstrapGrantV1::try_new(
            &self.dataspace.descriptor.alias,
            self.owner.clone(),
        )?;
        require(
            self.dataspace.descriptor.id == grant.dataspace.dataspace_id
                && self.dataspace.manifest_hash == grant.name_hash
                && self.lane.dataspace_id == grant.dataspace.dataspace_id
                && self.lane.id == self.lane_manifest.lane_id
                && self.lane.alias == grant.dataspace.canonical_name.to_string(),
            "dataspace, selector hash, lane and manifest must bind the same native identity",
        )?;
        require(
            !self.spending.alias_create_maximum.is_zero()
                && !self.spending.transaction_fee_maximum.is_zero(),
            "spending caps must be positive",
        )?;
        require(
            self.alias_request.schema_version == AliasSetupPlanRequestV1::VERSION
                && self.alias_request.intents.len() == 2,
            "deployment requires exactly one dataspace and one existing-owner account alias",
        )?;
        let mut dataspace = false;
        let mut account = false;
        for ensure in &self.alias_request.intents {
            require(
                ensure.acquisition.term_years > 0
                    && ensure.quote_guard.expected_payment_asset
                        == self.spending.asset_definition_id
                    && ensure.quote_guard.max_amount <= self.spending.alias_create_maximum,
                "alias intent exceeds its exact asset or acquisition cap",
            )?;
            match &ensure.intent {
                AliasIntentV1::Dataspace(intent) => {
                    require(
                        !dataspace
                            && intent.dataspace == grant.dataspace
                            && intent.owner == self.owner,
                        "dataspace alias intent differs from the deployment owner or identity",
                    )?;
                    dataspace = true;
                }
                AliasIntentV1::AccountAlias(intent) => {
                    require(
                        !account
                            && intent.target_account == self.owner
                            && intent.provision == AccountProvisionV1::Existing
                            && intent.role == AccountAliasRoleV1::Additional
                            && intent.alias.dataspace_id == grant.dataspace.dataspace_id
                            && intent.alias.canonical_name.dataspace
                                == grant.dataspace.canonical_name
                            && intent.alias.canonical_name.domain.is_none(),
                        "account alias must bind the existing owner as an additional alias in this dataspace",
                    )?;
                    account = true;
                }
                AliasIntentV1::Domain(_) => {
                    eyre::bail!("domain creation is outside this deployment contract")
                }
            }
        }
        require(
            dataspace && account,
            "deployment alias intent set is incomplete",
        )?;
        Ok(grant)
    }

    fn intent_digest(&self) -> Result<String> {
        let mut canonical = self.clone();
        canonical.operation_id = None;
        canonical
            .alias_request
            .intents
            .sort_by(|a, b| a.intent.cmp(&b.intent));
        Ok(digest(&json::to_vec(&canonical)?))
    }

    fn resolved_id(&self) -> Result<String> {
        Ok(self.operation_id.clone().unwrap_or(self.intent_digest()?))
    }
}

impl PlanV1 {
    fn verify(&self) -> Result<()> {
        require(
            self.schema_version == 1
                && self.intent_sha256 == self.manifest.intent_digest()?
                && self.operation_id == self.manifest.resolved_id()?
                && self.bootstrap_grant == self.manifest.validate()?
                && self.catalog_transition == transition(&self.manifest, &self.baseline)?
                && self
                    .baseline_overlay
                    .as_ref()
                    .map(NexusRuntimeCatalogV1::canonical_hash)
                    .transpose()?
                    == self.baseline.runtime_catalog_hash,
            "immutable plan is not self-consistent",
        )?;
        validate_paid_plan(&self.manifest, &self.initial_alias_plan)?;
        Ok(())
    }
}

fn transition(
    manifest: &ManifestV1,
    baseline: &LaneLifecycleStatusV1,
) -> Result<NexusCatalogTransitionV1> {
    baseline.validate()?;
    require(
        !baseline.lanes.iter().any(|lane| {
            lane.id == manifest.lane.id
                || lane.alias == manifest.lane.alias
                || lane.dataspace_id == manifest.lane.dataspace_id
        }),
        "new dataspace or lane is already present; this first-release plan cannot overwrite it",
    )?;
    let value = NexusCatalogTransitionV1 {
        version: NexusCatalogTransitionV1::VERSION,
        expected_catalog_hash: baseline.catalog_hash,
        expected_incarnation_root: baseline.incarnation_root,
        expected_runtime_catalog_hash: baseline.runtime_catalog_hash,
        dataspace_additions: vec![manifest.dataspace.clone()],
        lane_additions: vec![manifest.lane.clone()],
        manifest_additions: vec![manifest.lane_manifest.clone()],
    };
    value.validate_structure()?;
    Ok(value)
}

fn overlay(
    parameters: &Parameters,
    status: &LaneLifecycleStatusV1,
) -> Result<Option<NexusRuntimeCatalogV1>> {
    let value = parameters
        .custom
        .get(&NexusRuntimeCatalogV1::parameter_id())
        .map(NexusRuntimeCatalogV1::from_custom_parameter)
        .transpose()?
        .flatten();
    require(
        value
            .as_ref()
            .map(NexusRuntimeCatalogV1::canonical_hash)
            .transpose()?
            == status.runtime_catalog_hash,
        "parameter overlay and lifecycle status are not the same committed snapshot",
    )?;
    Ok(value)
}

fn validate_alias_plan(
    manifest: &ManifestV1,
    plan: &AliasTransactionPlanV1,
    client: &Client,
) -> Result<Vec<InstructionBox>> {
    client.verify_alias_setup_plan_for_request(&manifest.alias_request, plan)?;
    validate_paid_plan(manifest, plan)
}

fn validate_paid_plan(
    manifest: &ManifestV1,
    plan: &AliasTransactionPlanV1,
) -> Result<Vec<InstructionBox>> {
    require(
        plan.body.authority == manifest.owner && plan.body.network_id == manifest.network_id,
        "alias plan changed owner or network",
    )?;
    let instructions = iroha::client::decode_and_verify_alias_setup_plan_for_request(
        &manifest.alias_request,
        plan,
    )?;
    require(
        plan.body.resources.len() == 2 && instructions.len() == 2,
        "deployment requires exactly two native EnsureAlias instructions",
    )?;
    for resource in &plan.body.resources {
        require(
            resource.disposition == AliasPlanDispositionV1::Create,
            "first-release namespace must contain exactly two paid Create resources",
        )?;
        let quote = resource
            .quote
            .as_ref()
            .ok_or_else(|| eyre!("Create resource has no native quote"))?;
        require(
            !quote.exact_amount.is_zero()
                && quote.guard.expected_payment_asset == manifest.spending.asset_definition_id
                && quote.exact_amount <= manifest.spending.alias_create_maximum
                && quote.guard.max_amount <= manifest.spending.alias_create_maximum,
            "native alias quote exceeds the reviewed acquisition asset or cap",
        )?;
    }
    Ok(instructions)
}

fn preflight<C: RunContext>(
    context: &C,
    manifest: &ManifestV1,
    require_write_permissions: bool,
    native_client: Client,
) -> Result<BlockingClient> {
    require(
        !context.input_instructions()
            && !context.output_instructions()
            && context.transaction_metadata().is_none(),
        "dataspace deployment cannot combine instruction piping or unbound transaction metadata",
    )?;
    require(
        context.config().network_id == manifest.network_id
            && context.config().account == manifest.owner,
        "configured signer or NetworkId differs from the manifest",
    )?;
    let client = BlockingClient::from_client(native_client)?;
    client.refresh_capabilities()?;
    require(
        client
            .client()
            .get_account_read(&manifest.owner)?
            .account_id
            == manifest.owner,
        "native account read differs from the deployment owner",
    )?;
    if require_write_permissions {
        let permissions = crate::account::list_effective_permissions(
            client.client(),
            &manifest.owner,
            None,
            0,
            None,
        )?;
        for name in ["CanSetParameters", "CanReadAllLedgerData"] {
            require(
                permissions
                    .iter()
                    .any(|p| p.name() == name && p.payload().get() == "null"),
                &format!("deployment owner lacks exact {name} permission"),
            )?;
        }
    }
    Ok(client)
}

fn check_fee(manifest: &ManifestV1, quote: &FeeQuoteResponse) -> Result<()> {
    let FeePaymentIntent::Authority(intent) = &quote.intent else {
        eyre::bail!("deployment must pay from its authority");
    };
    require(
        intent.gas_limit.is_none() && !quote.components.is_empty(),
        "invalid instruction transaction fee quote",
    )?;
    let mut total = Quantity::zero();
    for component in &quote.components {
        require(
            component.kind == FeeChargeKind::Nexus
                && component.asset_definition_id == manifest.spending.asset_definition_id,
            "transaction quote changed fee kind or asset",
        )?;
        total = total.checked_add(&component.max_amount)?;
    }
    require(
        total <= manifest.spending.transaction_fee_maximum,
        "native transaction fee exceeds its explicit cap",
    )
}

fn check_funding(client: &Client, manifest: &ManifestV1, remaining_phases: usize) -> Result<()> {
    use iroha_data_model::{
        asset::{AssetBalanceScope, AssetId},
        prelude::FindAssetById,
    };
    let id = AssetId::with_scope(
        manifest.spending.asset_definition_id.clone(),
        manifest.owner.clone(),
        AssetBalanceScope::Global,
    );
    let balance = client.query_single(FindAssetById::new(id.clone()))?;
    require(
        balance.id == id,
        "funding read returned another asset/account/scope",
    )?;
    let mut reserve = manifest
        .spending
        .alias_create_maximum
        .checked_add(&manifest.spending.alias_create_maximum)?;
    for _ in 0..remaining_phases {
        reserve = reserve.checked_add(&manifest.spending.transaction_fee_maximum)?;
    }
    require(
        balance.value() >= &reserve,
        "global owner balance does not cover the remaining explicit deployment caps",
    )
}

fn physical_matches(plan: &PlanV1, client: &Client) -> Result<()> {
    let status = client.get_lane_lifecycle_status()?;
    status.validate()?;
    let parameters = client
        .get_parameters()
        .wrap_err("deployment catalog verification: read committed parameters")?;
    let current = overlay(&parameters, &status)?
        .ok_or_else(|| eyre!("committed runtime catalog overlay is absent"))?;
    let mut lanes = plan.baseline.lanes.clone();
    lanes.push(plan.manifest.lane.clone());
    lanes.sort_by_key(|lane| lane.id);
    require(
        status.lanes == lanes
            && status.lane_count
                == plan.baseline.lane_count.max(
                    plan.manifest
                        .lane
                        .id
                        .as_u32()
                        .checked_add(1)
                        .ok_or_else(|| eyre!("lane namespace overflow"))?,
                ),
        "committed catalog differs from the exact additive plan",
    )?;
    require(
        plan.baseline
            .incarnations
            .iter()
            .all(|entry| status.incarnations.contains(entry)),
        "an existing lane incarnation changed",
    )?;
    let mut dataspaces = plan
        .baseline_overlay
        .as_ref()
        .map(|v| v.dataspaces.clone())
        .unwrap_or_default();
    let mut manifests = plan
        .baseline_overlay
        .as_ref()
        .map(|v| v.manifests.clone())
        .unwrap_or_default();
    dataspaces.push(plan.manifest.dataspace.clone());
    manifests.push(plan.manifest.lane_manifest.clone());
    dataspaces.sort_by_key(|value| value.descriptor.id);
    manifests.sort_by_key(|value| value.lane_id);
    require(
        current.dataspaces == dataspaces && current.manifests == manifests,
        "committed overlay contains missing, changed or unexpected additions",
    )?;
    if let Some(before) = &plan.baseline_overlay {
        require(
            current.baseline_dataspaces_hash == before.baseline_dataspaces_hash
                && current.baseline_manifests_hash == before.baseline_manifests_hash,
            "runtime overlay changed its startup baseline",
        )?;
    }
    Ok(())
}

fn bootstrap_present(plan: &PlanV1, client: &Client) -> Result<bool> {
    let parameters = client.get_parameters()?;
    let Some(value) = parameters.custom.get(&plan.bootstrap_grant.parameter_id()?) else {
        return Ok(false);
    };
    require(
        AliasDataspaceBootstrapGrantV1::from_custom_parameter(value)?.as_ref()
            == Some(&plan.bootstrap_grant),
        "existing immutable bootstrap grant differs from the planned native grant",
    )?;
    Ok(true)
}

fn phase_instructions(
    plan: &PlanV1,
    phase: &str,
    client: &Client,
) -> Result<(Vec<InstructionBox>, Option<AliasTransactionPlanV1>)> {
    match phase {
        "catalog" => {
            require(
                client.get_lane_lifecycle_status()? == plan.baseline,
                "catalog CAS changed since planning; retained plan cannot be rebased",
            )?;
            Ok((
                vec![
                    SetParameter::new(Parameter::Custom(
                        plan.catalog_transition.clone().into_custom_parameter()?,
                    ))
                    .into(),
                ],
                None,
            ))
        }
        "bootstrap" => {
            physical_matches(plan, client)?;
            require(
                !bootstrap_present(plan, client)?,
                "bootstrap grant already exists outside this phase's transaction",
            )?;
            let name = plan.bootstrap_grant.dataspace.canonical_name.as_ref();
            require(
                client
                    .sns()
                    .get_name_optional(SnsNamespacePath::Dataspace, name)?
                    .is_none(),
                "bootstrap must precede the first dataspace SNS record",
            )?;
            Ok((
                vec![
                    SetParameter::new(Parameter::Custom(
                        plan.bootstrap_grant.clone().into_custom_parameter()?,
                    ))
                    .into(),
                ],
                None,
            ))
        }
        "aliases" => {
            physical_matches(plan, client)?;
            require(
                bootstrap_present(plan, client)?,
                "paid namespace requires the exact committed bootstrap grant",
            )?;
            let alias_plan = client.plan_alias_setup(&plan.manifest.alias_request)?;
            let instructions = validate_alias_plan(&plan.manifest, &alias_plan, client)?;
            Ok((instructions, Some(alias_plan)))
        }
        _ => eyre::bail!("unknown deployment phase"),
    }
}

impl PreparedV1 {
    fn verify(&self, plan: &PlanV1, phase: &str) -> Result<SignedTransaction> {
        require(
            self.schema_version == 1
                && self.operation_id == plan.operation_id
                && self.intent_sha256 == plan.intent_sha256
                && self.phase == phase,
            "prepared phase belongs to another exact intent",
        )?;
        let wire = hex::decode(&self.signed_transaction_wire_hex)?;
        require(
            !wire.is_empty()
                && wire.len() <= MAX_BYTES
                && hex::encode(&wire) == self.signed_transaction_wire_hex,
            "invalid retained transaction wire",
        )?;
        let transaction = SignedTransaction::decode_all_versioned(&wire)?;
        transaction.verify_signature()?;
        require(
            transaction.attachments().is_none() && transaction.multisig_signatures().is_none(),
            "deployment requires one configured signer without unrelated attachments",
        )?;
        require(
            (phase == "aliases") == self.alias_plan.is_some(),
            "phase has a missing or unexpected alias plan",
        )?;
        if let Some(alias) = &self.alias_plan {
            require(
                transaction.creation_time().as_millis() <= u128::from(alias.body.valid_until_ms),
                "retained transaction was created after the native alias plan deadline",
            )?;
        }

        require(
            transaction.encode_wire_v1()? == wire
                && hex::encode(transaction.hash().as_ref()) == self.transaction_hash
                && transaction.authority() == &plan.manifest.owner
                && transaction.network_id() == Some(&plan.manifest.network_id)
                && transaction.instructions() == &Executable::from(self.instructions.clone())
                && transaction.fee_payment_intent() == &self.fee_quote.intent,
            "retained signed transaction differs from its exact native phase",
        )?;
        self.fee_quote
            .validate_for_signed_payload(transaction.payload())
            .map_err(|error| eyre!(error))?;
        require(transaction.admission_intent() == iroha_data_model::transaction::signed::TransactionAdmissionIntent::QueuePlanSynced,
            "deployment transaction is not QueuePlan-synchronized")?;
        require(
            transaction.metadata().is_empty(),
            "retained deployment carries unbound metadata",
        )?;
        check_fee(&plan.manifest, &self.fee_quote)?;
        let expected: Vec<InstructionBox> = match phase {
            "catalog" => vec![
                SetParameter::new(Parameter::Custom(
                    plan.catalog_transition.clone().into_custom_parameter()?,
                ))
                .into(),
            ],
            "bootstrap" => vec![
                SetParameter::new(Parameter::Custom(
                    plan.bootstrap_grant.clone().into_custom_parameter()?,
                ))
                .into(),
            ],
            "aliases" => validate_paid_plan(
                &plan.manifest,
                self.alias_plan
                    .as_ref()
                    .ok_or_else(|| eyre!("paid phase omitted its native plan"))?,
            )?,
            _ => eyre::bail!("unknown retained phase"),
        };
        require(
            !expected.is_empty() && expected == self.instructions,
            "retained instructions differ from the reviewed native intent",
        )?;
        Ok(transaction)
    }
}

fn observe(
    client: &Client,
    prepared: &PreparedV1,
    transaction: &SignedTransaction,
) -> Result<PhaseObservationV1> {
    let hash = transaction.hash();
    let global = client
        .get_transaction_status_response_global(hash)
        .wrap_err_with(|| {
            format!(
                "deployment phase {}: read global transaction status",
                prepared.phase
            )
        })?;
    let peer = client
        .get_transaction_status_response_local(hash)
        .wrap_err_with(|| {
            format!(
                "deployment phase {}: read local transaction status",
                prepared.phase
            )
        })?;
    let mut result = PhaseObservationV1 {
        phase: prepared.phase.clone(),
        state: "pending".into(),
        transaction_hash: Some(prepared.transaction_hash.clone()),
        instructions: prepared.instructions.clone(),
        signed_transaction_wire_sha256: digest(&transaction.encode_wire_v1()?),
        alias_plan: prepared.alias_plan.clone(),
        global_status: global,
        peer_status: peer,
        committed: None,
    };
    let applied_height = matching_applied_height(
        &prepared.transaction_hash,
        &result.global_status,
        &result.peer_status,
    )?;
    let mut failed = false;
    for value in [&result.global_status, &result.peer_status] {
        if let Some(value) = value {
            if matches!(value.status.kind.as_str(), "Rejected" | "Expired") {
                failed = true;
            }
        }
    }
    if failed {
        result.state = "failed".into();
        if [&result.global_status, &result.peer_status]
            .into_iter()
            .flatten()
            .any(|value| value.status.kind == "Rejected")
        {
            result.committed = retain_rejected_details(
                transaction,
                client.get_transaction_details(transaction.hash_as_entrypoint()),
            )
            .wrap_err_with(|| {
                format!(
                    "deployment phase {} transaction {}: read exact rejection details",
                    prepared.phase, prepared.transaction_hash
                )
            })?;
        }
        return Ok(result);
    }
    if applied_height.is_some() {
        let details = client
            .get_successful_transaction_details(transaction.hash_as_entrypoint())
            .wrap_err_with(|| {
                format!(
                    "deployment phase {}: read exact committed transaction details",
                    prepared.phase
                )
            })?;
        let TransactionEntrypoint::External(actual) = details.transaction.entrypoint() else {
            eyre::bail!("committed phase is not an external transaction");
        };
        require(
            actual.encode_wire_v1()? == transaction.encode_wire_v1()?,
            "committed phase differs from the retained exact signed transaction",
        )?;
        result.state = "applied_verification_pending".into();
        result.committed = Some(details);
    }
    Ok(result)
}

// A precommit rejection may have no committed details. Only native typed
// absence permits that result; malformed, unauthorized and unbound reads fail.
fn retain_rejected_details(
    transaction: &SignedTransaction,
    response: std::result::Result<PipelineTransactionDetailsResponse, iroha::query::QueryError>,
) -> Result<Option<PipelineTransactionDetailsResponse>> {
    let details = match response {
        Ok(details) => details,
        Err(iroha::query::QueryError::Validation(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::NotFound,
            ),
        )) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    // The SDK binds entrypoint/result hashes. The deployment additionally binds
    // the exact retained wire and requires the rejected native result.
    let TransactionEntrypoint::External(actual) = details.transaction.entrypoint() else {
        eyre::bail!("rejection details are not an external transaction");
    };
    require(
        actual.encode_wire_v1()? == transaction.encode_wire_v1()?,
        "rejection details differ from the retained exact signed transaction",
    )?;
    require(
        details.transaction.result().is_err(),
        "Rejected status resolves to a successful committed transaction",
    )?;
    Ok(Some(details))
}

fn matching_applied_height(
    hash: &str,
    global: &Option<PipelineTransactionStatusResponse>,
    peer: &Option<PipelineTransactionStatusResponse>,
) -> Result<Option<u64>> {
    let mut heights = Vec::new();
    let mut pending = false;
    for (value, scope) in [(global, "global"), (peer, "local")] {
        let Some(value) = value else {
            pending = true;
            continue;
        };
        require(
            value.hash == hash && value.scope == scope,
            "status changed requested hash or scope",
        )?;
        require(
            matches!(
                value.status.kind.as_str(),
                "Queued" | "Approved" | "Committed" | "Applied" | "Rejected" | "Expired"
            ),
            "unknown native pipeline status kind",
        )?;
        require(
            matches!(value.resolved_from.as_str(), "cache" | "queue" | "state"),
            "unknown native pipeline status source",
        )?;
        if value.status.kind == "Applied" {
            let height = value
                .status
                .block_height
                .filter(|height| *height > 0)
                .ok_or_else(|| eyre!("Applied observation has no nonzero height"))?;
            if value.resolved_from == "state" {
                heights.push(height);
            } else {
                pending = true;
            }
        } else {
            pending = true;
        }
    }
    // A lagging or absent response must never hide a malformed response from
    // the other scope and turn a fixed binding error into a retryable wait.
    if pending {
        return Ok(None);
    }
    require(
        heights[0] == heights[1],
        "global and peer Applied heights differ",
    )?;
    Ok(Some(heights[0]))
}

fn phase_report(plan: &PlanV1, observations: Vec<PhaseObservationV1>) -> ReportV1 {
    let state = if observations.len() == PHASES.len()
        && observations
            .iter()
            .all(|v| v.state == "applied_verification_pending")
    {
        "applied_verification_pending"
    } else if observations.iter().any(|v| v.state == "failed") {
        "failed"
    } else {
        "pending"
    };
    ReportV1 {
        schema_version: 1,
        operation_id: plan.operation_id.clone(),
        state: state.into(),
        deployment_complete: false,
        verification_error: None,
        completion_receipt: None,
        verification: VerificationRequestV1 {
            schema_version: 1,
            operation_id: plan.operation_id.clone(),
            intent_sha256: plan.intent_sha256.clone(),
            network_id: plan.manifest.network_id,
            owner: plan.manifest.owner.clone(),
            dataspace: plan.manifest.dataspace.clone(),
            lane: plan.manifest.lane.clone(),
            lane_manifest: plan.manifest.lane_manifest.clone(),
            bootstrap_grant: plan.bootstrap_grant.clone(),
            alias_request: plan.manifest.alias_request.clone(),
            baseline: plan.baseline.clone(),
            baseline_overlay: plan.baseline_overlay.clone(),
            transactions: observations,
            independently_anchored_finality_required: true,
            authenticated_execution_commitment_required: true,
            all_four_state_observations_required: true,
        },
    }
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        require(
            context.config().chain.to_string() == "fc56984b-2be7-431d-840e-21514d1883f0"
                && context.config().account_chain_discriminant == 369,
            "Taira dataspace deployment requires the canonical chain and account profile369",
        )?;
        let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
        match self {
            Self::ExportProfile(_) => eyre::bail!(
                "`taira dataspace-deploy export-profile` must run before client configuration is loaded"
            ),
            Self::Init(args) => initialize(context, args),
            Self::Preflight(args) => preflight_manifest(context, args),
            Self::Ensure(args) => ensure(context, args),
            Self::Plan(args) => plan(context, args),
            Self::Apply(args) => saved(context, args, true),
            Self::Status(args) => saved(context, args, false),
        }
    }
}

fn initialize<C: RunContext>(context: &mut C, args: InitArgs) -> Result<()> {
    let manifest = derive_manifest(context, &args)?;
    let journal = Journal::open_unpublished(&args.output_dir)?;
    journal.require_unplanned()?;
    journal.install_json("deployment.json", &manifest)?;
    context.print_data(&manifest)
}

fn derive_manifest<C: RunContext>(context: &C, args: &InitArgs) -> Result<ManifestV1> {
    use iroha_data_model::sns::{ACCOUNT_ALIAS_SUFFIX_ID, DATASPACE_ALIAS_SUFFIX_ID};
    let trust: finality::TrustV1 = json::from_slice(&read_public_input(&args.trust)?)?;
    trust.validate(context.config().network_id)?;
    let client = context.client_from_config()?;
    let policies = [
        client.sns().get_policy(DATASPACE_ALIAS_SUFFIX_ID)?,
        client.sns().get_policy(ACCOUNT_ALIAS_SUFFIX_ID)?,
    ];
    let now = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    let deadline = now
        .checked_add(
            args.quote_lifetime_secs
                .checked_mul(1000)
                .ok_or_else(|| eyre!("quote lifetime overflow"))?,
        )
        .ok_or_else(|| eyre!("quote deadline overflow"))?;
    let manifest = init_manifest(
        args,
        context.config().network_id,
        context.config().account.clone(),
        trust,
        &policies,
        deadline,
    )?;
    let configured = preflight(context, &manifest, true, context.client_from_config()?)?;
    let plan = configured
        .client()
        .plan_alias_setup(&manifest.alias_request)?;
    validate_alias_plan(&manifest, &plan, configured.client())?;
    Ok(manifest)
}

fn init_manifest(
    args: &InitArgs,
    network_id: NetworkId,
    owner: AccountId,
    trust: finality::TrustV1,
    policies: &[iroha_data_model::sns::SuffixPolicyV1; 2],
    deadline: u64,
) -> Result<ManifestV1> {
    use iroha_data_model::{
        alias_setup::{
            AccountAliasName, AccountAliasRoleV1, AliasAccountIntentV1, AliasDataSpaceIntentV1,
            AliasLeaseAcquisitionV1, AliasQuoteGuardV1, ResolvedAccountAliasV1,
        },
        isi::alias_setup::EnsureAlias,
        nexus::{DataSpaceMetadata, LaneStorageProfile, LaneVisibility},
        sns::{ACCOUNT_ALIAS_SUFFIX_ID, DATASPACE_ALIAS_SUFFIX_ID, SuffixPolicyV1, SuffixStatus},
    };
    use iroha_model_base::topology::LaneId;
    require(
        deadline > 0 && deadline < u64::MAX,
        "generated plan guard requires a finite deadline",
    )?;
    let grant = AliasDataspaceBootstrapGrantV1::try_new(&args.dataspace, owner.clone())?;
    let guard = |policy: &SuffixPolicyV1, expected_suffix| -> Result<AliasQuoteGuardV1> {
        require(
            policy.suffix_id == expected_suffix
                && policy.status == SuffixStatus::Active
                && policy.min_term_years <= args.lease_years
                && args.lease_years <= policy.max_term_years,
            "native namespace policy is inactive, mismatched, or excludes the requested lease term",
        )?;
        let asset = AssetDefinitionId::parse_address_literal(&policy.payment_asset_id)?;
        require(
            asset == args.payment_asset,
            "native namespace policy uses another payment asset",
        )?;
        Ok(AliasQuoteGuardV1 {
            expected_policy_version: policy.policy_version,
            expected_payment_asset: asset,
            max_amount: args.alias_create_maximum.clone(),
            valid_until_ms: deadline,
        })
    };
    let name = grant.dataspace.canonical_name.to_string();
    let inline_manifest = lane_manifest::generate(&name, &trust)?;
    let alias = AccountAliasName::try_new(&args.account_alias, None::<&str>, &name)?;
    let intents = vec![
        EnsureAlias::new(
            AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
                dataspace: grant.dataspace.clone(),
                owner: owner.clone(),
            }),
            AliasLeaseAcquisitionV1::new(args.lease_years, None),
            guard(&policies[0], DATASPACE_ALIAS_SUFFIX_ID)?,
        ),
        EnsureAlias::new(
            AliasIntentV1::AccountAlias(AliasAccountIntentV1 {
                alias: ResolvedAccountAliasV1::new(alias, grant.dataspace.dataspace_id),
                target_account: owner.clone(),
                provision: AccountProvisionV1::Existing,
                role: AccountAliasRoleV1::Additional,
            }),
            AliasLeaseAcquisitionV1::new(args.lease_years, None),
            guard(&policies[1], ACCOUNT_ALIAS_SUFFIX_ID)?,
        ),
    ];
    let lane_id = LaneId::new(args.lane_id);
    let manifest = ManifestV1 {
        schema_version: 1,
        operation_id: args.operation_id.clone(),
        network_id,
        owner,
        dataspace: RuntimeDataSpaceAdditionV1 {
            descriptor: DataSpaceMetadata {
                id: grant.dataspace.dataspace_id,
                alias: name.clone(),
                description: None,
                fault_tolerance: 1,
            },
            manifest_hash: grant.name_hash,
        },
        lane: LaneConfig {
            id: lane_id,
            dataspace_id: grant.dataspace.dataspace_id,
            alias: name,
            visibility: match args.lane_profile {
                LaneProfile::RestrictedFullReplica => LaneVisibility::Restricted,
                LaneProfile::PublicFullReplica => LaneVisibility::Public,
            },
            storage: LaneStorageProfile::FullReplica,
            ..LaneConfig::default()
        },
        lane_manifest: RuntimeLaneManifestV1 {
            lane_id,
            manifest: inline_manifest,
        },
        alias_request: AliasSetupPlanRequestV1::new(intents),
        spending: SpendingV1 {
            asset_definition_id: args.payment_asset.clone(),
            alias_create_maximum: args.alias_create_maximum.clone(),
            transaction_fee_maximum: args.transaction_fee_maximum.clone(),
        },
        finality: trust,
    };
    manifest.validate()?;
    Ok(manifest)
}

fn plan<C: RunContext>(context: &mut C, args: PlanArgs) -> Result<()> {
    let planned = plan_manifest(context, args)?;
    context.print_data(&planned)
}

fn plan_manifest<C: RunContext>(context: &C, args: PlanArgs) -> Result<PlanV1> {
    let bytes = read_public_input(&args.manifest)?;
    let manifest: ManifestV1 = json::from_slice(&bytes)?;
    let grant = manifest.validate()?;
    let id = manifest.resolved_id()?;
    let client = preflight(context, &manifest, true, context.client_from_config()?)?;
    let path = args.journal_dir.join(&id);
    // Acquire the operation lock before inspecting an existing directory. A
    // crash may leave only its lock (or an unpublished staging file); no phase
    // can have been signed or dispatched before the immutable plan exists.
    let journal = Journal::open_unpublished(&path)?;
    if let Some(existing) = journal.optional_json::<PlanV1>("plan.json")? {
        existing.verify()?;
        require(
            existing.manifest == manifest && existing.intent_sha256 == manifest.intent_digest()?,
            "operation ID is already bound to another manifest",
        )?;
        return Ok(existing);
    }
    journal.require_unplanned()?;
    let result = fresh_plan_after_preflight(context, manifest, grant, id, client.client())?;
    journal.install_json("plan.json", &result)?;
    Ok(result)
}

/// Runs the same fresh-plan admission as `plan`, ending before its first journal write.
fn fresh_plan_after_preflight<C: RunContext>(
    context: &C,
    manifest: ManifestV1,
    grant: AliasDataspaceBootstrapGrantV1,
    id: String,
    client: &Client,
) -> Result<PlanV1> {
    finality::preflight(context, &manifest)?;
    let baseline = client.get_lane_lifecycle_status()?;
    let baseline_overlay = overlay(&client.get_parameters()?, &baseline)?;
    let catalog_transition = transition(&manifest, &baseline)?;
    check_funding(client, &manifest, PHASES.len())?;
    let initial_alias_plan = client.plan_alias_setup(&manifest.alias_request)?;
    validate_alias_plan(&manifest, &initial_alias_plan, client)?;
    let result = PlanV1 {
        schema_version: 1,
        operation_id: id,
        intent_sha256: manifest.intent_digest()?,
        manifest,
        baseline,
        baseline_overlay,
        catalog_transition,
        bootstrap_grant: grant,
        initial_alias_plan,
    };
    result.verify()?;
    Ok(result)
}

fn preflight_manifest<C: RunContext>(context: &mut C, args: PreflightArgs) -> Result<()> {
    let bytes = read_public_input(&args.manifest)?;
    let manifest: ManifestV1 = json::from_slice(&bytes)?;
    let grant = manifest.validate()?;
    let id = manifest.resolved_id()?;
    let client = preflight(context, &manifest, true, context.client_from_config()?)?;
    let candidate = fresh_plan_after_preflight(context, manifest, grant, id, client.client())?;
    context.print_data(&preflight_report(&candidate, digest(&bytes)))
}

fn normal_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path.components().all(|part| {
            matches!(
                part,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
}

impl EnsureRequestV1 {
    fn lane_profile(&self) -> Result<LaneProfile> {
        match self.lane_profile.as_str() {
            "restricted_full_replica" => Ok(LaneProfile::RestrictedFullReplica),
            "public_full_replica" => Ok(LaneProfile::PublicFullReplica),
            _ => eyre::bail!("ensure request uses an unknown native lane profile"),
        }
    }

    fn validate<C: RunContext>(&self, context: &C) -> Result<finality::TrustV1> {
        require(
            self.schema == "iroha.taira.dataspace-deploy.ensure-request.v1",
            "unknown dataspace ensure request schema",
        )?;
        operation_id(&self.operation_id)?;
        require(
            self.network_id == context.config().network_id
                && self.owner == context.config().account,
            "ensure request differs from the configured network or signer",
        )?;
        require(
            self.lane_id > 0 && self.lease_years > 0 && self.lane_profile().is_ok(),
            "ensure request has an invalid lane or lease selection",
        )?;
        for path in [&self.trust, &self.manifest_dir, &self.journal_dir] {
            require(
                normal_absolute(path),
                "ensure request paths must be absolute and normal",
            )?;
        }
        require(
            self.manifest_dir != self.journal_dir
                && self.manifest_dir != self.journal_dir.join(&self.operation_id),
            "ensure request reuses a journal path for its manifest",
        )?;
        let bytes = read_public_input(&self.trust)?;
        require(
            digest(&bytes) == self.trust_sha256,
            "ensure request trust profile digest differs",
        )?;
        let trust: finality::TrustV1 = json::from_slice(&bytes)?;
        trust.validate(self.network_id)?;
        #[cfg(unix)]
        private_metadata(&fs::symlink_metadata(&self.journal_dir)?, true)?;
        Ok(trust)
    }

    fn init_args(&self) -> Result<InitArgs> {
        Ok(InitArgs {
            dataspace: self.dataspace.clone(),
            lane_id: self.lane_id,
            lane_profile: self.lane_profile()?,
            account_alias: self.account_alias.clone(),
            trust: self.trust.clone(),
            payment_asset: self.payment_asset.clone(),
            alias_create_maximum: self.alias_create_maximum.clone(),
            transaction_fee_maximum: self.transaction_fee_maximum.clone(),
            lease_years: self.lease_years,
            quote_lifetime_secs: 3600,
            operation_id: Some(self.operation_id.clone()),
            output_dir: self.manifest_dir.clone(),
        })
    }

    fn matches_manifest(&self, manifest: &ManifestV1, trust: &finality::TrustV1) -> Result<()> {
        use iroha_data_model::{alias_setup::AccountAliasName, nexus::LaneVisibility};
        use iroha_model_base::topology::LaneId;
        manifest.validate()?;
        let grant = AliasDataspaceBootstrapGrantV1::try_new(&self.dataspace, self.owner.clone())?;
        let dataspace = grant.dataspace.canonical_name.to_string();
        let account_alias =
            AccountAliasName::try_new(&self.account_alias, None::<&str>, &dataspace)?;
        let visibility = match self.lane_profile()? {
            LaneProfile::RestrictedFullReplica => LaneVisibility::Restricted,
            LaneProfile::PublicFullReplica => LaneVisibility::Public,
        };
        let expected_dataspace = RuntimeDataSpaceAdditionV1 {
            descriptor: iroha_data_model::nexus::DataSpaceMetadata {
                id: grant.dataspace.dataspace_id,
                alias: dataspace.clone(),
                description: None,
                fault_tolerance: 1,
            },
            manifest_hash: grant.name_hash,
        };
        let expected_lane = LaneConfig {
            id: LaneId::new(self.lane_id),
            dataspace_id: grant.dataspace.dataspace_id,
            alias: dataspace,
            visibility,
            storage: iroha_data_model::nexus::LaneStorageProfile::FullReplica,
            ..LaneConfig::default()
        };
        require(
            manifest.operation_id.as_deref() == Some(self.operation_id.as_str())
                && manifest.network_id == self.network_id
                && manifest.owner == self.owner
                && manifest.dataspace == expected_dataspace
                && manifest.lane == expected_lane
                && manifest.finality == *trust
                && manifest.spending.asset_definition_id == self.payment_asset
                && manifest.spending.alias_create_maximum == self.alias_create_maximum
                && manifest.spending.transaction_fee_maximum == self.transaction_fee_maximum
                && manifest.alias_request.intents.iter().all(|intent| {
                    intent.acquisition.term_years == self.lease_years
                        && intent.acquisition.pricing_class_hint.is_none()
                        && intent.quote_guard.max_amount == self.alias_create_maximum
                })
                && manifest.alias_request.intents.iter().any(|intent| {
                    matches!(&intent.intent, AliasIntentV1::AccountAlias(alias)
                        if alias.alias.canonical_name == account_alias)
                }),
            "retained dataspace manifest differs from the fixed ensure request",
        )
    }
}

impl EnsureBindingV1 {
    fn new(request: &EnsureRequestV1, manifest: ManifestV1) -> Result<Self> {
        Ok(Self {
            schema: "iroha.taira.dataspace-deploy.ensure-binding.v1".into(),
            request_sha256: digest(&json::to_vec(request)?),
            manifest_sha256: digest(&json::to_vec(&manifest)?),
            manifest,
        })
    }

    fn verify(&self, request: &EnsureRequestV1, trust: &finality::TrustV1) -> Result<()> {
        require(
            self.schema == "iroha.taira.dataspace-deploy.ensure-binding.v1"
                && self.request_sha256 == digest(&json::to_vec(request)?)
                && self.manifest_sha256 == digest(&json::to_vec(&self.manifest)?),
            "retained ensure binding differs from the exact typed request or manifest",
        )?;
        request.matches_manifest(&self.manifest, trust)
    }
}

/// A binding is published before deployment.json. Replaying that intermediate
/// state republishes its exact manifest instead of deriving a new quote guard.
fn recover_bound_manifest(
    journal: &Journal,
    request: &EnsureRequestV1,
    trust: &finality::TrustV1,
) -> Result<Option<ManifestV1>> {
    let Some(binding) = journal.optional_json::<EnsureBindingV1>("ensure-binding.json")? else {
        require(
            journal
                .optional_json::<ManifestV1>("deployment.json")?
                .is_none(),
            "deployment manifest exists without its native ensure binding",
        )?;
        return Ok(None);
    };
    binding.verify(request, trust)?;
    if let Some(published) = journal.optional_json::<ManifestV1>("deployment.json")? {
        require(
            published == binding.manifest,
            "published deployment manifest differs from its retained ensure binding",
        )?;
    } else {
        journal.install_json("deployment.json", &binding.manifest)?;
    }
    Ok(Some(binding.manifest))
}

fn ensure_bound_manifest<C: RunContext>(
    context: &C,
    request: &EnsureRequestV1,
    trust: &finality::TrustV1,
    allow_create: bool,
) -> Result<ManifestV1> {
    let journal = if allow_create {
        Journal::open_unpublished(&request.manifest_dir)?
    } else {
        Journal::open(&request.manifest_dir, false)?
    };
    if let Some(manifest) = recover_bound_manifest(&journal, request, trust)? {
        return Ok(manifest);
    }
    require(
        allow_create,
        "planned deployment is missing its retained native ensure binding",
    )?;
    journal.require_unplanned()?;
    let manifest = derive_manifest(context, &request.init_args()?)?;
    request.matches_manifest(&manifest, trust)?;
    let binding = EnsureBindingV1::new(request, manifest.clone())?;
    journal.install_json("ensure-binding.json", &binding)?;
    journal.install_json("deployment.json", &manifest)?;
    Ok(manifest)
}

fn ensure<C: RunContext>(context: &mut C, args: EnsureArgs) -> Result<()> {
    let bytes = read_ensure_request(&args.request)?;
    let request: EnsureRequestV1 = json::from_slice(&bytes)?;
    let trust = request.validate(context)?;
    let operation_dir = request.journal_dir.join(&request.operation_id);
    let manifest_path = request.manifest_dir.join("deployment.json");
    let plan_exists = operation_dir.join("plan.json").try_exists()?;
    let manifest = ensure_bound_manifest(context, &request, &trust, !plan_exists)?;
    if plan_exists {
        let journal = Journal::open(&operation_dir, false)?;
        let retained: PlanV1 = journal.read_json("plan.json")?;
        retained.verify()?;
        require(
            retained.operation_id == request.operation_id,
            "retained plan belongs to another operation",
        )?;
        require(
            retained.manifest == manifest,
            "retained plan differs from the native ensure manifest",
        )?;
    } else {
        let retained = plan_manifest(
            context,
            PlanArgs {
                manifest: manifest_path,
                journal_dir: request.journal_dir.clone(),
            },
        )?;
        require(
            retained.operation_id == request.operation_id,
            "planned operation identity differs from the ensure request",
        )?;
        require(
            retained.manifest == manifest,
            "planned manifest differs from the native ensure binding",
        )?;
    }
    drop(bytes);
    if plan_exists {
        let status = run_saved(
            context,
            SavedArgs {
                journal_dir: request.journal_dir.clone(),
                operation_id: request.operation_id.clone(),
                timeout_ms: args.timeout_ms,
            },
            false,
        )?;
        if status.deployment_complete {
            return print_saved_report(&status, true, |report| context.print_data(report));
        }
    }
    let report = run_saved(
        context,
        SavedArgs {
            journal_dir: request.journal_dir,
            operation_id: request.operation_id,
            timeout_ms: args.timeout_ms,
        },
        true,
    )?;
    print_saved_report(&report, true, |report| context.print_data(report))
}

fn saved<C: RunContext>(context: &mut C, args: SavedArgs, apply: bool) -> Result<()> {
    let report = run_saved(context, args, apply)?;
    print_saved_report(&report, apply, |report| context.print_data(report))
}

fn print_saved_report(
    report: &ReportV1,
    apply: bool,
    print: impl FnOnce(&ReportV1) -> Result<()>,
) -> Result<()> {
    print(report)?;
    if apply
        && !(report.state == "completed"
            && report.deployment_complete
            && report
                .completion_receipt
                .as_deref()
                .is_some_and(|name| !name.is_empty())
            && report.verification_error.is_none())
    {
        eyre::bail!(
            "dataspace deployment {} did not complete ({}): {}",
            report.operation_id,
            report.state,
            incomplete_report_detail(report)
        );
    }
    Ok(())
}

// Render retained observations and authenticated-query rejection details only.
// Neither is an independently anchored finality or deployment completion claim.
fn incomplete_report_detail(report: &ReportV1) -> String {
    let mut details: Vec<String> = report.verification_error.iter().cloned().collect();
    for phase in &report.verification.transactions {
        if phase.state != "failed" {
            continue;
        }
        for (status, scope) in [
            (&phase.global_status, "global"),
            (&phase.peer_status, "local"),
        ] {
            let Some(status) = status else { continue };
            if phase.transaction_hash.as_deref() != Some(status.hash.as_str())
                || status.scope != scope
                || !matches!(status.status.kind.as_str(), "Rejected" | "Expired")
            {
                continue;
            }
            let height = status
                .status
                .block_height
                .map(|height| format!(", block {height}"))
                .unwrap_or_default();
            let reason = if status.status.kind == "Rejected" {
                match phase
                    .committed
                    .as_ref()
                    .and_then(|details| details.transaction.result().as_ref().err())
                {
                    Some(reason) => {
                        format!("; rejection reason: {}", rejection_error_chain(reason))
                    }
                    None => "; committed rejection details unavailable".into(),
                }
            } else {
                String::new()
            };
            details.push(format!(
                "phase {}: observed {} for transaction {} (scope {}, source {}{}){}",
                phase.phase,
                status.status.kind,
                status.hash,
                scope,
                status.resolved_from,
                height,
                reason
            ));
        }
    }
    if details.is_empty() {
        "inspect the retained operation with status".into()
    } else {
        details.join("; ")
    }
}

// Display/source messages expose the native cause without Debug-formatting
// instruction or signed transaction payloads. Bound terminal diagnostic size.
fn rejection_error_chain(error: &(dyn std::error::Error + 'static)) -> String {
    let mut next = Some(error);
    let mut parts = Vec::new();
    for _ in 0..16 {
        let Some(error) = next else { break };
        let message = error.to_string();
        if !message.is_empty() {
            parts.push(message);
        }
        next = error.source();
    }
    let message = parts.join(": ");
    let mut chars = message.chars();
    let mut bounded: String = chars.by_ref().take(4096).collect();
    if chars.next().is_some() || next.is_some() {
        bounded.push_str(" [truncated]");
    }
    bounded
}

fn run_saved<C: RunContext>(context: &C, args: SavedArgs, apply: bool) -> Result<ReportV1> {
    let deadline = operation_deadline(args.timeout_ms)?;
    require_operation_budget(deadline, "open retained operation")?;
    operation_id(&args.operation_id)?;
    let journal = Journal::open(&args.journal_dir.join(&args.operation_id), false)?;
    let plan: PlanV1 = journal
        .read_json("plan.json")
        .wrap_err("saved deployment: read plan.json")?;
    plan.verify()
        .wrap_err("saved deployment: verify retained plan")?;
    require_operation_budget(deadline, "verify retained plan")?;
    require(
        plan.operation_id == args.operation_id,
        "operation directory contains another plan",
    )?;
    let client = preflight(
        context,
        &plan.manifest,
        apply,
        context
            .client_from_config()?
            .with_request_deadline(deadline),
    )
    .wrap_err("saved deployment: signer and capability preflight")?;
    require_operation_budget(deadline, "signer and capability preflight")?;
    let mut observations = Vec::new();
    for (phase_index, phase) in PHASES.into_iter().enumerate() {
        require_operation_budget(deadline, &format!("phase {phase} preparation"))?;
        eprintln!("[dataspace-deploy] phase {phase}: preparation");
        let prepared_name = format!("{phase}.prepared.json");
        let claim_name = format!("{phase}.submitted.json");
        let mut prepared: Option<PreparedV1> =
            journal.optional_json(&prepared_name).wrap_err_with(|| {
                format!("deployment phase {phase}: read retained preparation {prepared_name}")
            })?;
        if prepared.is_none() && apply {
            check_funding(client.client(), &plan.manifest, PHASES.len() - phase_index)
                .wrap_err_with(|| {
                    format!("deployment phase {phase}: verify funding for remaining caps")
                })?;
            let (instructions, alias_plan) = phase_instructions(&plan, phase, client.client())
                .wrap_err_with(|| {
                    format!("deployment phase {phase}: prepare native instructions")
                })?;
            require(
                !instructions.is_empty(),
                "empty deployment transactions are forbidden",
            )?;
            let (transaction, quote) = quote_and_sign_transaction_with_admission(
                &client,
                Executable::from(instructions.clone()),
                FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
                TransactionAdmissionIntent::QueuePlanSynced,
            )
            .wrap_err_with(|| {
                format!("deployment phase {phase}: quote and sign exact transaction")
            })?;
            check_fee(&plan.manifest, &quote)?;
            let value = PreparedV1 {
                schema_version: 1,
                operation_id: plan.operation_id.clone(),
                intent_sha256: plan.intent_sha256.clone(),
                phase: phase.into(),
                signed_transaction_wire_hex: hex::encode(transaction.encode_wire_v1()?),
                transaction_hash: hex::encode(transaction.hash().as_ref()),
                instructions,
                fee_quote: quote,
                alias_plan,
            };
            value
                .verify(&plan, phase)
                .wrap_err_with(|| format!("deployment phase {phase}: verify new preparation"))?;
            require_operation_budget(deadline, &format!("phase {phase} retain preparation"))?;
            journal.install_json(&prepared_name, &value)?;
            prepared = Some(value);
        }
        let Some(prepared) = prepared else {
            break;
        };
        let transaction = prepared.verify(&plan, phase).wrap_err_with(|| {
            format!("deployment phase {phase}: verify retained preparation {prepared_name}")
        })?;
        require_operation_budget(deadline, &format!("phase {phase} verify preparation"))?;
        let claim: Option<String> = journal.optional_json(&claim_name)?;
        if let Some(claim) = &claim {
            require(
                claim == &digest(&json::to_vec(&prepared)?),
                "dispatch claim does not bind the retained transaction",
            )?;
        }
        if apply && claim.is_none() {
            check_funding(client.client(), &plan.manifest, PHASES.len() - phase_index)
                .wrap_err_with(|| {
                    format!("deployment phase {phase}: verify funding for remaining caps")
                })?;
            // Revalidate current native conditions without preparing another transaction.
            let (instructions, fresh_alias_plan) =
                phase_instructions(&plan, phase, client.client()).wrap_err_with(|| {
                    format!("deployment phase {phase}: revalidate instructions before dispatch")
                })?;
            require(
                instructions == prepared.instructions,
                "phase changed before first dispatch",
            )?;
            if let Some(fresh) = fresh_alias_plan {
                // Anchors may move; the exact retained plan, guards and instructions may not.
                validate_alias_plan(&plan.manifest, &fresh, client.client())?;
                client
                    .client()
                    .verify_alias_setup_plan(prepared.alias_plan.as_ref().unwrap())?;
            }
            let transaction_expiry = transaction
                .creation_time()
                .checked_add(
                    transaction
                        .time_to_live()
                        .ok_or_else(|| eyre!("retained transaction has no finite lifetime"))?,
                )
                .ok_or_else(|| eyre!("transaction lifetime overflow"))?;
            require(
                SystemTime::now().duration_since(UNIX_EPOCH)? < transaction_expiry,
                "retained transaction expired before dispatch; it will not be replaced",
            )?;
            require_operation_budget(deadline, &format!("phase {phase} dispatch claim"))?;
            require(
                record_dispatch_claim(&journal, &claim_name, &prepared)?,
                "phase was already dispatched",
            )?;
            // The claim is durable before this sole mutation call. Transport failure is pending.
            let outcome = client.submit_transaction(&transaction);
            if let Ok(hash) = &outcome {
                require(
                    hash == &transaction.hash(),
                    "submit response changed the retained transaction hash",
                )?;
            }
            let receipt = SubmissionResultV1 {
                transaction_hash: prepared.transaction_hash.clone(),
                accepted: outcome.is_ok(),
                error: outcome.err().map(|error| format!("{error:#}")),
            };
            eprintln!(
                "[dataspace-deploy] phase {phase}: dispatch {}",
                if receipt.accepted {
                    "accepted"
                } else {
                    "uncertain; observing retained transaction"
                }
            );
            journal.install_json(&format!("{phase}.submission-result.json"), &receipt)?;
        }
        eprintln!(
            "[dataspace-deploy] phase {phase}: {}",
            if apply {
                "waiting for exact Applied"
            } else {
                "reading exact retained state"
            }
        );
        // This loop only reads the exact retained transaction. It never re-enters
        // preparation, signing, the durable dispatch claim, or submission.
        let observation = observe_phase_until(apply, deadline, phase, || {
            observe(client.client(), &prepared, &transaction)
        })?;
        let advance = observation.state == "applied_verification_pending";
        eprintln!(
            "[dataspace-deploy] phase {phase}: {}",
            if advance {
                "exact Applied"
            } else {
                observation.state.as_str()
            }
        );
        observations.push(observation);
        if !advance {
            break;
        }
    }
    let mut report = phase_report(&plan, observations);
    if report.state == "applied_verification_pending" {
        eprintln!("[dataspace-deploy] starting fresh four-validator finality verification");
        let verification: Result<()> = (|| {
            let mut completion = finality::Completion::new(&plan, &journal, deadline)?;
            complete_until(apply, deadline, &mut report, |report| {
                completion.complete(context, report)
            })
        })();
        if let Err(error) = verification {
            report.state = "applied_verification_pending".into();
            report.deployment_complete = false;
            report.completion_receipt = None;
            report.verification_error = Some(format!("{error:#}"));
        }
    }
    if report.deployment_complete {
        require_operation_budget(deadline, "return completed deployment")?;
    }
    eprintln!("[dataspace-deploy] result: {}", report.state);
    Ok(report)
}

#[derive(JsonSerialize)]
struct SubmissionResultV1 {
    transaction_hash: String,
    accepted: bool,
    #[norito(required)]
    error: Option<String>,
}

fn record_dispatch_claim(journal: &Journal, name: &str, prepared: &PreparedV1) -> Result<bool> {
    let expected = digest(&json::to_vec(prepared)?);
    if let Some(current) = journal.optional_json::<String>(name)? {
        require(
            current == expected,
            "dispatch claim changed its exact signed phase",
        )?;
        return Ok(false);
    }
    journal.install_json(name, &expected)?;
    Ok(true)
}

/// Descriptor-relative immutable storage. A held flock serializes each operation.
struct Journal {
    path: PathBuf,
    directory: File,
    _lock: File,
    lock_snapshot: fs::Metadata,
}

#[cfg(unix)]
fn private_metadata(metadata: &fs::Metadata, directory: bool) -> Result<()> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    require(
        metadata.uid() == rustix::process::geteuid().as_raw()
            && metadata.permissions().mode() & 0o077 == 0
            && if directory {
                metadata.is_dir()
            } else {
                metadata.is_file() && metadata.nlink() == 1
            },
        "journal must be a current-owner private directory with direct single-link files",
    )
}

#[cfg(unix)]
fn same_file_snapshot(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.uid() == after.uid()
        && before.gid() == after.gid()
        && before.mode() == after.mode()
        && before.nlink() == after.nlink()
        && before.len() == after.len()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
        && before.ctime() == after.ctime()
        && before.ctime_nsec() == after.ctime_nsec()
}

#[cfg(unix)]
fn require_unplanned_entries(path: &Path, directory: &File, allow_lock: bool) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;

    let held = directory.metadata()?;
    let named = fs::symlink_metadata(path)?;
    private_metadata(&named, true)?;
    require(
        held.dev() == named.dev() && held.ino() == named.ino(),
        "operation journal directory was replaced",
    )?;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let name = entry.file_name();
        if allow_lock && name.to_str() == Some("lock") {
            continue;
        }
        require(
            name.to_str()
                .is_some_and(|name| name.starts_with(".staging-")),
            "operation journal contains evidence without a durable plan",
        )?;
        private_metadata(&fs::symlink_metadata(entry.path())?, false)?;
    }
    let named = fs::symlink_metadata(path)?;
    private_metadata(&named, true)?;
    require(
        held.dev() == named.dev() && held.ino() == named.ino(),
        "operation journal directory was replaced",
    )
}

impl Journal {
    #[cfg(unix)]
    fn open(path: &Path, create: bool) -> Result<Self> {
        Self::open_inner(path, create, false)
    }

    #[cfg(unix)]
    fn open_unpublished(path: &Path) -> Result<Self> {
        Self::open_inner(path, true, true)
    }

    #[cfg(unix)]
    fn open_inner(path: &Path, create: bool, recover_unplanned: bool) -> Result<Self> {
        use rustix::fs::{Mode, OFlags};
        let name = path
            .file_name()
            .ok_or_else(|| eyre!("operation directory has no name"))?;
        let parent_path = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .canonicalize()?;
        let parent = File::from(rustix::fs::open(
            &parent_path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )?);
        private_metadata(&parent.metadata()?, true)?;
        let mut created = false;
        if create {
            match rustix::fs::mkdirat(&parent, name, Mode::from_raw_mode(0o700)) {
                Ok(()) => created = true,
                Err(rustix::io::Errno::EXIST) if recover_unplanned => {}
                Err(error) => return Err(error.into()),
            }
            parent.sync_all()?;
        }
        let directory = File::from(rustix::fs::openat(
            &parent,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )?);
        private_metadata(&directory.metadata()?, true)?;
        let canonical_path = parent_path.join(name);
        let lock_flags = OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
        let lock = if recover_unplanned && !created {
            match rustix::fs::openat(&directory, "lock", lock_flags, Mode::empty()) {
                Ok(fd) => File::from(fd),
                Err(rustix::io::Errno::NOENT) => {
                    // A lost lock from a planned operation must never be
                    // silently replaced: another process may still hold its
                    // unlinked inode across an uncertain submission.
                    require_unplanned_entries(&canonical_path, &directory, false)?;
                    File::from(rustix::fs::openat(
                        &directory,
                        "lock",
                        lock_flags | OFlags::CREATE | OFlags::EXCL,
                        Mode::from_raw_mode(0o600),
                    )?)
                }
                Err(error) => return Err(error.into()),
            }
        } else {
            File::from(rustix::fs::openat(
                &directory,
                "lock",
                lock_flags
                    | if create {
                        OFlags::CREATE | OFlags::EXCL
                    } else {
                        OFlags::empty()
                    },
                Mode::from_raw_mode(0o600),
            )?)
        };
        private_metadata(&lock.metadata()?, false)?;
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive)?;
        directory.sync_all()?;
        let value = Self {
            path: canonical_path,
            directory,
            lock_snapshot: lock.metadata()?,
            _lock: lock,
        };
        value.revalidate()?;
        Ok(value)
    }

    #[cfg(not(unix))]
    fn open(_: &Path, _: bool) -> Result<Self> {
        eyre::bail!("durable deployments require Unix descriptor custody")
    }

    #[cfg(not(unix))]
    fn open_unpublished(_: &Path) -> Result<Self> {
        eyre::bail!("durable deployments require Unix descriptor custody")
    }

    #[cfg(unix)]
    fn require_unplanned(&self) -> Result<()> {
        self.revalidate()?;
        require_unplanned_entries(&self.path, &self.directory, true)?;
        self.revalidate()
    }

    #[cfg(not(unix))]
    fn require_unplanned(&self) -> Result<()> {
        eyre::bail!("durable deployments require Unix descriptor custody")
    }

    #[cfg(unix)]
    fn revalidate(&self) -> Result<()> {
        use std::os::unix::fs::MetadataExt as _;
        let actual = fs::symlink_metadata(&self.path)?;
        let pinned = self.directory.metadata()?;
        private_metadata(&actual, true)?;
        require(
            actual.dev() == pinned.dev() && actual.ino() == pinned.ino(),
            "operation journal directory was replaced",
        )?;
        self.revalidate_file("lock", &self._lock, &self.lock_snapshot, &[])
    }

    #[cfg(unix)]
    fn revalidate_file_metadata(
        &self,
        name: &str,
        file: &File,
        before: &fs::Metadata,
    ) -> Result<()> {
        use rustix::fs::{Mode, OFlags};
        let after = file.metadata()?;
        private_metadata(&after, false)?;
        let named = File::from(rustix::fs::openat(
            &self.directory,
            name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )?);
        let named = named.metadata()?;
        private_metadata(&named, false)?;
        require(
            same_file_snapshot(before, &after) && same_file_snapshot(&after, &named),
            "journal file or held lock changed during custody",
        )
    }

    #[cfg(unix)]
    fn revalidate_file(
        &self,
        name: &str,
        file: &File,
        before: &fs::Metadata,
        expected: &[u8],
    ) -> Result<()> {
        use std::os::unix::fs::FileExt as _;
        self.revalidate_file_metadata(name, file, before)?;
        require(
            before.len() == u64::try_from(expected.len())?,
            "journal content length differs from its retained snapshot",
        )?;
        // Metadata timestamps can collide. Recheck the bytes consumed by this
        // operation without disturbing offsets shared by cloned descriptors.
        let mut buffer = [0_u8; 64 * 1024];
        let mut offset = 0_u64;
        for chunk in expected.chunks(buffer.len()) {
            let observed = &mut buffer[..chunk.len()];
            file.read_exact_at(observed, offset)
                .map_err(|_| eyre!("journal content revalidation read failed"))?;
            require(observed == chunk, "journal content changed during custody")?;
            offset += u64::try_from(chunk.len())?;
        }
        let mut extra = [0_u8; 1];
        require(
            file.read_at(&mut extra, offset)
                .map_err(|_| eyre!("journal content revalidation read failed"))?
                == 0,
            "journal content grew during custody",
        )?;
        self.revalidate_file_metadata(name, file, before)
    }

    #[cfg(not(unix))]
    fn revalidate(&self) -> Result<()> {
        eyre::bail!("Unix required")
    }

    fn optional_json<T: JsonDeserialize + JsonSerialize>(&self, name: &str) -> Result<Option<T>> {
        self.read_optional(name)?
            .map(|bytes| {
                let value: T = json::from_slice(&bytes)
                    .wrap_err_with(|| format!("failed to decode retained journal file `{name}`"))?;
                require(
                    json::to_vec(&value)? == bytes,
                    "retained JSON is not canonical or is incomplete",
                )?;
                Ok(value)
            })
            .transpose()
    }
    fn read_json<T: JsonDeserialize + JsonSerialize>(&self, name: &str) -> Result<T> {
        self.optional_json(name)?
            .ok_or_else(|| eyre!("operation preparation is incomplete"))
    }
    fn install_json<T: JsonSerialize>(&self, name: &str, value: &T) -> Result<()> {
        self.install(name, &json::to_vec(value)?)
    }

    fn read_optional(&self, name: &str) -> Result<Option<Vec<u8>>> {
        self.read_optional_bounded(name, MAX_BYTES)
    }

    fn install(&self, name: &str, bytes: &[u8]) -> Result<()> {
        self.install_bounded(name, bytes, MAX_BYTES)
    }

    #[cfg(unix)]
    fn read_optional_bounded(&self, name: &str, maximum: usize) -> Result<Option<Vec<u8>>> {
        use rustix::fs::{Mode, OFlags};
        self.revalidate()?;
        let fd = match rustix::fs::openat(
            &self.directory,
            name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut file = File::from(fd);
        let before = file.metadata()?;
        private_metadata(&before, false)?;
        require(
            before.len() <= u64::try_from(maximum)?,
            "journal file exceeds bound",
        )?;
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut file)
            .take(
                u64::try_from(maximum)?
                    .checked_add(1)
                    .ok_or_else(|| eyre!("journal byte bound overflow"))?,
            )
            .read_to_end(&mut bytes)?;
        require(bytes.len() <= maximum, "journal file exceeds bound")?;
        self.revalidate_file(name, &file, &before, &bytes)?;
        self.revalidate()?;
        Ok(Some(bytes))
    }

    #[cfg(unix)]
    fn install_bounded(&self, name: &str, bytes: &[u8], maximum: usize) -> Result<()> {
        use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
        require(bytes.len() <= maximum, "journal output exceeds bound")?;
        require(
            !name.is_empty() && !name.contains('/') && name != "." && name != "..",
            "journal evidence name must be one direct filename",
        )?;
        self.revalidate()?;
        let temporary = format!(".staging-{}", hex::encode(rand::random::<[u8; 16]>()));
        let mut file = File::from(rustix::fs::openat(
            &self.directory,
            temporary.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::from_raw_mode(0o600),
        )?);
        let result: Result<()> = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            private_metadata(&file.metadata()?, false)?;
            self.revalidate()?;
            rustix::fs::renameat_with(
                &self.directory,
                temporary.as_str(),
                &self.directory,
                name,
                RenameFlags::NOREPLACE,
            )?;
            self.directory.sync_all()?;
            self.revalidate()?;
            require(
                self.read_optional_bounded(name, maximum)?.as_deref() == Some(bytes),
                "published journal evidence changed",
            )
        })();
        // A crash may leave this unreferenced private staging file. Readers ignore it;
        // a subsequent attempt uses a distinct name and cannot replace final evidence.
        match rustix::fs::unlinkat(&self.directory, temporary.as_str(), AtFlags::empty()) {
            Ok(()) => self.directory.sync_all()?,
            Err(rustix::io::Errno::NOENT) => {}
            Err(error) if result.is_ok() => return Err(error.into()),
            Err(_) => {}
        }
        result
    }

    #[cfg(not(unix))]
    fn read_optional_bounded(&self, _: &str, _: usize) -> Result<Option<Vec<u8>>> {
        eyre::bail!("Unix required")
    }
    #[cfg(not(unix))]
    fn install_bounded(&self, _: &str, _: &[u8], _: usize) -> Result<()> {
        eyre::bail!("Unix required")
    }
}

fn read_public_input(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    require(
        metadata.is_file() && metadata.len() <= MAX_BYTES as u64,
        "manifest must be a bounded direct regular file",
    )?;
    #[cfg(unix)]
    let mut file = {
        use rustix::fs::{Mode, OFlags};
        File::from(rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )?)
    };
    #[cfg(not(unix))]
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    require(bytes.len() <= MAX_BYTES, "manifest exceeds bound")?;
    Ok(bytes)
}

#[cfg(unix)]
fn read_ensure_request(path: &Path) -> Result<Vec<u8>> {
    use rustix::fs::{Mode, OFlags};
    require(
        normal_absolute(path),
        "ensure request path must be absolute and normal",
    )?;
    let mut file = File::from(rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )?);
    let before = file.metadata()?;
    private_metadata(&before, false)?;
    require(
        before.len() > 0 && before.len() <= MAX_BYTES as u64,
        "ensure request must be nonempty and bounded",
    )?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    require(
        bytes.len() as u64 == before.len()
            && same_file_snapshot(&before, &file.metadata()?)
            && same_file_snapshot(&before, &fs::symlink_metadata(path)?),
        "ensure request changed during custody",
    )?;
    Ok(bytes)
}

#[cfg(not(unix))]
fn read_ensure_request(_: &Path) -> Result<Vec<u8>> {
    eyre::bail!("dataspace ensure requires Unix descriptor custody")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        alias_setup::{
            AccountAliasRoleV1, AliasAccountIntentV1, AliasAssetTotalV1, AliasDataSpaceIntentV1,
            AliasFramedInstructionV1, AliasLeaseAcquisitionV1, AliasLeaseQuoteV1,
            AliasPlanAnchorV1, AliasPlanResourceV1, AliasQuoteGuardV1, AliasTransactionPlanBodyV1,
            ResolvedAccountAliasV1,
        },
        isi::alias_setup::EnsureAlias,
        nexus::{DataSpaceMetadata, FeeDebitSource, LaneCatalog},
        transaction::{
            TransactionBuilder,
            signed::{FeeChargeLimit, TransactionAdmissionIntent},
        },
    };
    use iroha_model_base::topology::{DataSpaceId, LaneId};
    use iroha_primitives::{json::Json, numeric::Numeric};
    use iroha_torii_shared::{
        FeeQuoteComponent, FeeQuoteDecision, FeeQuoteObservation, PipelineTransactionStatus,
    };
    use std::{collections::BTreeMap, num::NonZeroU32};

    fn amount(value: u32, scale: u32) -> Quantity {
        Quantity::from_canonical_numeric(Numeric::new(value, scale)).unwrap()
    }

    fn ensure_request(manifest: &ManifestV1) -> EnsureRequestV1 {
        EnsureRequestV1 {
            schema: "iroha.taira.dataspace-deploy.ensure-request.v1".into(),
            operation_id: "fixed-op".into(),
            network_id: manifest.network_id,
            owner: manifest.owner.clone(),
            dataspace: "devex".into(),
            lane_id: 6,
            lane_profile: "public_full_replica".into(),
            account_alias: "admin".into(),
            trust: PathBuf::from("/private/trust.json"),
            trust_sha256: "ab".repeat(32),
            payment_asset: manifest.spending.asset_definition_id.clone(),
            alias_create_maximum: manifest.spending.alias_create_maximum.clone(),
            transaction_fee_maximum: manifest.spending.transaction_fee_maximum.clone(),
            lease_years: 1,
            manifest_dir: PathBuf::from("/private/manifest"),
            journal_dir: PathBuf::from("/private/journals"),
        }
    }

    #[test]
    fn ensure_request_binds_exact_retained_operation_and_selection() {
        let mut manifest = manifest();
        manifest.operation_id = Some("fixed-op".into());
        let request = ensure_request(&manifest);
        request
            .matches_manifest(&manifest, &manifest.finality)
            .unwrap();
        let args = request.init_args().unwrap();
        assert_eq!(args.operation_id.as_deref(), Some("fixed-op"));
        assert_eq!(args.output_dir, request.manifest_dir);
        let mut changed = request.clone();
        changed.operation_id = "different-op".into();
        assert!(
            changed
                .matches_manifest(&manifest, &manifest.finality)
                .is_err()
        );
        changed = request.clone();
        changed.transaction_fee_maximum = amount(2, 0);
        assert!(
            changed
                .matches_manifest(&manifest, &manifest.finality)
                .is_err()
        );
        changed = request.clone();
        changed.account_alias = "another".into();
        assert!(
            changed
                .matches_manifest(&manifest, &manifest.finality)
                .is_err()
        );
        let mut changed_manifest = manifest.clone();
        let account_alias = changed_manifest
            .alias_request
            .intents
            .iter_mut()
            .find_map(|intent| match &mut intent.intent {
                AliasIntentV1::AccountAlias(alias) => Some(alias),
                _ => None,
            })
            .unwrap();
        account_alias.role = AccountAliasRoleV1::Primary;
        assert!(changed_manifest.validate().is_err());
        assert!(
            request
                .matches_manifest(&changed_manifest, &changed_manifest.finality)
                .is_err()
        );
        let wire = json::to_vec(&request).unwrap();
        let mut value: json::Value = json::from_slice(&wire).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("retry_with_new_quote".into(), norito::json!(true));
        assert!(json::from_slice::<EnsureRequestV1>(&json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn ensure_binding_recovers_exact_manifest_and_rejects_foreign_request_or_unbound_manifest() {
        use std::os::unix::fs::PermissionsExt as _;

        let mut manifest = manifest();
        manifest.operation_id = Some("fixed-op".into());
        let request = ensure_request(&manifest);
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let directory = root.path().join("bound");
        let journal = Journal::open_unpublished(&directory).unwrap();
        journal.require_unplanned().unwrap();
        let binding = EnsureBindingV1::new(&request, manifest.clone()).unwrap();
        binding.verify(&request, &manifest.finality).unwrap();
        journal
            .install_json("ensure-binding.json", &binding)
            .unwrap();
        drop(journal);

        // A crash before deployment.json must replay the exact retained
        // manifest, including its original quote guard and deadline.
        let resumed = Journal::open_unpublished(&directory).unwrap();
        assert_eq!(
            recover_bound_manifest(&resumed, &request, &manifest.finality).unwrap(),
            Some(manifest.clone())
        );
        assert_eq!(
            resumed.read_json::<ManifestV1>("deployment.json").unwrap(),
            manifest
        );
        let mut changed = request.clone();
        changed.journal_dir = PathBuf::from("/private/other-journals");
        assert!(
            changed
                .matches_manifest(&manifest, &manifest.finality)
                .is_ok()
        );
        assert!(recover_bound_manifest(&resumed, &changed, &manifest.finality).is_err());
        drop(resumed);

        let unbound = Journal::open_unpublished(&root.path().join("unbound")).unwrap();
        unbound.install_json("deployment.json", &manifest).unwrap();
        assert!(recover_bound_manifest(&unbound, &request, &manifest.finality).is_err());

        let mismatched = Journal::open_unpublished(&root.path().join("mismatched")).unwrap();
        mismatched
            .install_json("ensure-binding.json", &binding)
            .unwrap();
        let mut foreign = manifest.clone();
        foreign.operation_id = Some("foreign-op".into());
        mismatched
            .install_json("deployment.json", &foreign)
            .unwrap();
        assert!(recover_bound_manifest(&mismatched, &request, &manifest.finality).is_err());
    }

    #[test]
    fn ensure_cli_accepts_one_fixed_request_and_budget() {
        use clap::Parser as _;
        #[derive(clap::Parser)]
        struct Wrapper {
            #[command(subcommand)]
            command: Command,
        }
        let parsed =
            Wrapper::try_parse_from(["test", "ensure", "--request", "/private/ensure.json"])
                .unwrap();
        let Command::Ensure(args) = parsed.command else {
            panic!("ensure subcommand expected")
        };
        assert_eq!(args.request.as_path(), Path::new("/private/ensure.json"));
        assert_eq!(args.timeout_ms, 600_000);
        assert!(
            Wrapper::try_parse_from([
                "test",
                "ensure",
                "--request",
                "/private/ensure.json",
                "--timeout-ms",
                "0"
            ])
            .is_err()
        );
    }

    #[test]
    fn preflight_command_and_closed_readiness_report() {
        use clap::Parser as _;
        #[derive(clap::Parser)]
        struct Wrapper {
            #[command(subcommand)]
            command: Command,
        }
        let parsed = Wrapper::try_parse_from([
            "test",
            "preflight",
            "--manifest",
            "/private/deployment.json",
        ])
        .unwrap();
        let Command::Preflight(args) = parsed.command else {
            panic!("preflight subcommand expected")
        };
        assert_eq!(
            args.manifest.as_path(),
            Path::new("/private/deployment.json")
        );
        assert!(Wrapper::try_parse_from(["test", "preflight"]).is_err());
        let plan = fixture_plan();
        let report = preflight_report(&plan, "ab".repeat(32));
        assert_eq!(report.schema, "iroha.taira.dataspace-deploy.preflight.v1");
        assert_eq!(report.operation_id, plan.operation_id);
        assert_eq!(report.intent_sha256, plan.intent_sha256);
        assert_eq!(report.manifest_sha256, "ab".repeat(32));
        assert_eq!(report.state, "ready_for_plan");
        assert!(!report.deployment_complete);
        let wire = json::to_vec(&report).unwrap();
        assert_eq!(
            json::from_slice::<PreflightReportV1>(&wire).unwrap(),
            report
        );
        let mut unknown: json::Value = json::from_slice(&wire).unwrap();
        unknown
            .as_object_mut()
            .unwrap()
            .insert("deployment_applied".into(), norito::json!(true));
        assert!(json::from_slice::<PreflightReportV1>(&json::to_vec(&unknown).unwrap()).is_err());
    }

    fn pending_observation() -> PhaseObservationV1 {
        PhaseObservationV1 {
            phase: "aliases".into(),
            state: "pending".into(),
            transaction_hash: Some("ab".repeat(32)),
            instructions: Vec::new(),
            signed_transaction_wire_sha256: "cd".repeat(32),
            alias_plan: None,
            global_status: None,
            peer_status: None,
            committed: None,
        }
    }

    #[test]
    fn saved_commands_require_positive_budget_and_default_to_three_minutes() {
        use clap::Parser as _;
        #[derive(clap::Parser)]
        struct Wrapper {
            #[command(subcommand)]
            command: Command,
        }
        for command in ["apply", "status"] {
            let arguments = [
                "test",
                command,
                "--journal-dir",
                "/unused",
                "--operation-id",
                "test",
            ];
            let parsed = Wrapper::try_parse_from(arguments).unwrap();
            let (Command::Apply(saved) | Command::Status(saved)) = parsed.command else {
                panic!("saved command expected")
            };
            assert_eq!(saved.timeout_ms, 180_000);
            let mut explicit = arguments.to_vec();
            explicit.extend(["--timeout-ms", "1"]);
            assert!(Wrapper::try_parse_from(&explicit).is_ok());
            *explicit.last_mut().unwrap() = "0";
            assert!(Wrapper::try_parse_from(&explicit).is_err());
        }
        assert!(operation_deadline(0).is_err());
    }

    #[test]
    fn saved_apply_emits_report_before_rejecting_incomplete_success() {
        let mut completed = phase_report(&fixture_plan(), Vec::new());
        completed.state = "completed".into();
        completed.deployment_complete = true;
        completed.completion_receipt = Some("completion-test.json".into());
        let mut variants = vec![completed.clone()];
        for defect in 0..5 {
            let mut report = completed.clone();
            match defect {
                0 => report.state = "applied_verification_pending".into(),
                1 => report.deployment_complete = false,
                2 => report.completion_receipt = None,
                3 => report.completion_receipt = Some(String::new()),
                _ => report.verification_error = Some("invalid validator proof".into()),
            }
            variants.push(report);
        }
        for (index, report) in variants.iter().enumerate() {
            for apply in [false, true] {
                let mut output = Vec::new();
                let result = print_saved_report(report, apply, |value| {
                    output.push(json::to_value(value)?);
                    Ok(())
                });
                assert_eq!(output, vec![json::to_value(report).unwrap()]);
                assert_eq!(result.is_ok(), !apply || index == 0);
                if apply && index == 5 {
                    assert!(
                        result
                            .unwrap_err()
                            .to_string()
                            .contains("invalid validator proof")
                    );
                }
            }
        }
        // Both native terminal kinds are rendered for either bound scope. The
        // machine report stays intact and the status command remains read-only.
        let hash = "ab".repeat(32);
        for scope in ["global", "local"] {
            for kind in ["Rejected", "Expired"] {
                let mut phase = pending_observation();
                phase.phase = "catalog".into();
                phase.state = "failed".into();
                let status = PipelineTransactionStatusResponse {
                    hash: hash.clone(),
                    scope: scope.into(),
                    resolved_from: "cache".into(),
                    status: PipelineTransactionStatus {
                        kind: kind.into(),
                        block_height: (kind == "Rejected").then_some(10),
                    },
                };
                if scope == "global" {
                    phase.global_status = Some(status);
                } else {
                    phase.peer_status = Some(status);
                }
                let report = phase_report(&fixture_plan(), vec![phase.clone()]);
                let before = json::to_value(&report).unwrap();
                let mut output = Vec::new();
                let error = print_saved_report(&report, true, |value| {
                    output.push(json::to_value(value)?);
                    Ok(())
                })
                .unwrap_err();
                let height = if kind == "Rejected" { ", block 10" } else { "" };
                let missing = if kind == "Rejected" {
                    "; committed rejection details unavailable"
                } else {
                    ""
                };
                let detail = format!(
                    "phase catalog: observed {kind} for transaction {hash} (scope {scope}, source cache{height}){missing}"
                );
                assert_eq!(
                    error.to_string(),
                    format!(
                        "dataspace deployment {} did not complete (failed): {detail}",
                        report.operation_id
                    )
                );
                assert_eq!(output, vec![before.clone()]);
                assert_eq!(json::to_value(&report).unwrap(), before);
                assert!(print_saved_report(&report, false, |_| Ok(())).is_ok());
                let mut with_verification_error = report.clone();
                with_verification_error.verification_error = Some("invalid validator proof".into());
                assert_eq!(
                    incomplete_report_detail(&with_verification_error),
                    format!("invalid validator proof; {detail}")
                );

                // Unbound, absent, or nonterminal observations cannot be
                // attributed as the failed transaction's native outcome.
                for defect in 0..5 {
                    let mut altered = phase.clone();
                    match defect {
                        0 => altered.transaction_hash = Some("ef".repeat(32)),
                        1 => altered.transaction_hash = None,
                        2 => {
                            let value = altered
                                .global_status
                                .as_mut()
                                .or(altered.peer_status.as_mut())
                                .unwrap();
                            value.scope = "other".into();
                        }
                        3 => {
                            let value = altered
                                .global_status
                                .as_mut()
                                .or(altered.peer_status.as_mut())
                                .unwrap();
                            value.status.kind = "Queued".into();
                        }
                        _ => altered.state = "pending".into(),
                    }
                    let report = phase_report(&fixture_plan(), vec![altered]);
                    assert_eq!(
                        incomplete_report_detail(&report),
                        "inspect the retained operation with status"
                    );
                }
            }
        }
        use iroha_data_model::{
            ValidationFail,
            block::execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1},
            isi::error::InstructionExecutionError,
            query::CommittedTransaction,
            transaction::{
                DataTriggerSequence, TransactionResult,
                error::{InstructionExecutionFail, TransactionRejectionReason},
            },
        };
        let plan = fixture_plan();
        let prepared = prepared(&plan);
        let transaction = prepared.verify(&plan, "catalog").unwrap();
        let marker = "lane 6 manifest authority account is not registered";
        let reason = TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
            InstructionExecutionError::Conversion(marker.into()),
        ));
        let make_details = |transaction: SignedTransaction, result: TransactionResult| {
            let output = ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                input_index: 0,
                result,
                completions: Vec::new(),
            });
            PipelineTransactionDetailsResponse {
                hash: transaction.hash_as_entrypoint().to_string(),
                transaction: CommittedTransaction {
                    block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                        b"rejection report test",
                    )),
                    entrypoint_hash: transaction.hash_as_entrypoint(),
                    entrypoint_proof: iroha_crypto::MerkleProof::from_audit_path(0, Vec::new()),
                    entrypoint: TransactionEntrypoint::External(transaction),
                    output_hash: iroha_crypto::HashOf::new(&output),
                    output_proof: iroha_crypto::MerkleProof::from_audit_path(0, Vec::new()),
                    output,
                },
            }
        };
        let details = make_details(transaction.clone(), TransactionResult::new(Err(reason)));
        let retained = retain_rejected_details(&transaction, Ok(details.clone()))
            .unwrap()
            .unwrap();
        assert_eq!(
            json::to_value(&retained).unwrap(),
            json::to_value(&details).unwrap()
        );
        let mut phase = pending_observation();
        phase.phase = "catalog".into();
        phase.state = "failed".into();
        phase.transaction_hash = Some(prepared.transaction_hash.clone());
        phase.signed_transaction_wire_sha256 = digest(&transaction.encode_wire_v1().unwrap());
        phase.global_status = Some(PipelineTransactionStatusResponse {
            hash: prepared.transaction_hash.clone(),
            scope: "global".into(),
            resolved_from: "state".into(),
            status: PipelineTransactionStatus {
                kind: "Rejected".into(),
                block_height: Some(10),
            },
        });
        phase.committed = Some(retained);
        let report = phase_report(&plan, vec![phase]);
        let error = print_saved_report(&report, true, |_| Ok(()))
            .unwrap_err()
            .to_string();
        assert!(error.contains(marker));
        assert!(
            error.contains("Validation failed: Instruction execution failed: Conversion Error:")
        );
        assert!(error.contains(&prepared.transaction_hash));
        assert!(!error.contains(&prepared.signed_transaction_wire_hex));
        assert!(!error.contains("completion receipt"));
        // The native instruction error owns an instruction, but its Display/source
        // rendering must expose the reason without dumping that instruction.
        let private_payload = "instruction-payload-must-not-be-printed";
        let safe = TransactionRejectionReason::InstructionExecution(InstructionExecutionFail {
            instruction: iroha_data_model::isi::Log::new(
                iroha_data_model::Level::INFO,
                private_payload.into(),
            )
            .into(),
            reason: marker.into(),
        });
        let summary = rejection_error_chain(&safe);
        assert!(summary.contains(marker));
        assert!(!summary.contains(private_payload));
        let long =
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted("é".repeat(5000)));
        let bounded = rejection_error_chain(&long);
        assert!(bounded.ends_with(" [truncated]"));
        assert_eq!(bounded.chars().count(), 4096 + " [truncated]".len());
        let absent = iroha::query::QueryError::Validation(ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound,
        ));
        assert!(
            retain_rejected_details(&transaction, Err(absent))
                .unwrap()
                .is_none()
        );
        for error in [
            iroha::query::QueryError::Validation(ValidationFail::NotPermitted(
                "not authorized".into(),
            )),
            iroha::query::QueryError::Other(eyre!("malformed or mismatched exact details")),
        ] {
            assert!(retain_rejected_details(&transaction, Err(error)).is_err());
        }
        let success = make_details(
            transaction.clone(),
            TransactionResult::new(Ok(DataTriggerSequence::default())),
        );
        assert!(
            retain_rejected_details(&transaction, Ok(success))
                .unwrap_err()
                .to_string()
                .contains("successful")
        );
        let other = TransactionBuilder::new(
            plan.manifest.network_id,
            plan.manifest.owner.clone(),
            prepared.fee_quote.intent.clone(),
        )
        .with_instructions([iroha_data_model::isi::Log::new(
            iroha_data_model::Level::INFO,
            "different transaction".into(),
        )])
        .try_sign(key().private_key())
        .unwrap();
        let mut wrong = details;
        wrong.hash = other.hash_as_entrypoint().to_string();
        wrong.transaction.entrypoint_hash = other.hash_as_entrypoint();
        wrong.transaction.entrypoint = TransactionEntrypoint::External(other);
        assert!(
            retain_rejected_details(&transaction, Ok(wrong))
                .unwrap_err()
                .to_string()
                .contains("exact signed transaction")
        );
    }

    #[test]
    fn saved_report_preserves_output_failure() {
        let report = phase_report(&fixture_plan(), Vec::new());
        for apply in [false, true] {
            let error = print_saved_report(&report, apply, |_| {
                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "output closed").into())
            })
            .unwrap_err();
            assert_eq!(
                error.downcast_ref::<std::io::Error>().unwrap().kind(),
                std::io::ErrorKind::BrokenPipe
            );
        }
    }

    #[test]
    fn saved_zero_budget_stops_before_journal_or_client_access() {
        struct NoIoContext;
        impl RunContext for NoIoContext {
            fn config(&self) -> &iroha::config::Config {
                panic!("expired operation accessed configuration")
            }
            fn transaction_metadata(&self) -> Option<&Metadata> {
                panic!("expired operation accessed metadata")
            }
            fn input_instructions(&self) -> bool {
                panic!("expired operation accessed instructions")
            }
            fn output_instructions(&self) -> bool {
                panic!("expired operation accessed instructions")
            }
            fn i18n(&self) -> &iroha_i18n::Localizer {
                panic!("expired operation accessed localization")
            }
            fn print_data<T: JsonSerialize + ?Sized>(&mut self, _: &T) -> Result<()> {
                panic!("expired operation printed success")
            }
            fn println(&mut self, _: impl std::fmt::Display) -> Result<()> {
                panic!("expired operation printed success")
            }
        }
        for apply in [false, true] {
            let error = run_saved(
                &NoIoContext,
                SavedArgs {
                    journal_dir: PathBuf::from("/journal-must-not-be-opened"),
                    operation_id: "deadline-test".into(),
                    timeout_ms: 0,
                },
                apply,
            )
            .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("--timeout-ms must be greater than zero")
            );
        }
    }

    #[test]
    fn expired_operation_never_observes_or_starts_completion() {
        let deadline = Instant::now();
        let error = observe_phase_until(true, deadline, "aliases", || {
            panic!("expired operation read")
        })
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::TimedOut
        );
        assert!(error.to_string().contains("phase aliases observation"));
        let mut report = phase_report(&fixture_plan(), Vec::new());
        assert!(
            complete_until(true, deadline, &mut report, |_| panic!(
                "expired completion"
            ))
            .is_err()
        );
        assert!(!report.deployment_complete);
    }

    #[test]
    fn apply_observes_pending_until_applied_without_reentering_dispatch() {
        let retained = pending_observation();
        let mut observations = 0;
        // Submission is deliberately outside the observer callback's API. A
        // pending read must only repeat this exact retained hash and wire binding.
        let actual =
            observe_phase_until(true, operation_deadline(5_000).unwrap(), "aliases", || {
                observations += 1;
                let mut next = retained.clone();
                if observations == 2 {
                    next.state = "applied_verification_pending".into();
                }
                Ok(next)
            })
            .unwrap();
        assert_eq!(observations, 2);
        assert_eq!(actual.state, "applied_verification_pending");
        assert_eq!(actual.transaction_hash, retained.transaction_hash);
        assert_eq!(
            actual.signed_transaction_wire_sha256,
            retained.signed_transaction_wire_sha256
        );
    }

    #[test]
    fn status_observes_once_and_terminal_apply_does_not_retry() {
        for (apply, state) in [(false, "pending"), (true, "failed")] {
            let mut reads = 0;
            let observed =
                observe_phase_until(apply, operation_deadline(5_000).unwrap(), "aliases", || {
                    reads += 1;
                    assert_eq!(reads, 1);
                    Ok(PhaseObservationV1 {
                        state: state.into(),
                        ..pending_observation()
                    })
                })
                .unwrap();
            assert_eq!(observed.state, state);
            assert_eq!(reads, 1);
        }
    }

    #[test]
    fn phase_deadline_rejects_late_applied_and_clips_pending_sleep() {
        let now = Instant::now();
        assert_eq!(
            operation_poll_delay(now + Duration::from_secs(1), now),
            Duration::from_millis(500)
        );
        assert_eq!(
            operation_poll_delay(now + Duration::from_millis(10), now),
            Duration::from_millis(10)
        );
        assert_eq!(operation_poll_delay(now, now), Duration::ZERO);
        for applied in [false, true] {
            let deadline = operation_deadline(10).unwrap();
            let mut reads = 0;
            let error = observe_phase_until(true, deadline, "aliases", || {
                reads += 1;
                if applied {
                    std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
                }
                Ok(PhaseObservationV1 {
                    state: if applied {
                        "applied_verification_pending"
                    } else {
                        "pending"
                    }
                    .into(),
                    ..pending_observation()
                })
            })
            .unwrap_err();
            assert!(reads <= 1);
            assert_eq!(
                error.downcast_ref::<std::io::Error>().unwrap().kind(),
                std::io::ErrorKind::TimedOut
            );
        }
    }

    #[test]
    fn completion_retries_only_explicit_sync_progress_and_status_is_one_attempt() {
        for progress in ["verification_sync_pending", "verification_peer_pending"] {
            for apply in [false, true] {
                let mut report = phase_report(&fixture_plan(), Vec::new());
                let mut attempts = 0;
                complete_until(
                    apply,
                    operation_deadline(5_000).unwrap(),
                    &mut report,
                    |report| {
                        attempts += 1;
                        report.state = if attempts == 1 { progress } else { "completed" }.into();
                        report.deployment_complete = attempts == 2;
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(attempts, if apply { 2 } else { 1 });
                assert_eq!(report.deployment_complete, apply);
            }
        }
        let mut report = phase_report(&fixture_plan(), Vec::new());
        let mut attempts = 0;
        let error = complete_until(
            true,
            operation_deadline(5_000).unwrap(),
            &mut report,
            |_| {
                attempts += 1;
                eyre::bail!("changed validator authority")
            },
        )
        .unwrap_err();
        assert_eq!(attempts, 1);
        assert!(error.to_string().contains("changed validator authority"));
    }

    #[test]
    fn completion_deadline_rejects_a_late_success() {
        let mut report = phase_report(&fixture_plan(), Vec::new());
        let deadline = operation_deadline(10).unwrap();
        let error = complete_until(true, deadline, &mut report, |report| {
            std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
            report.state = "completed".into();
            report.deployment_complete = true;
            Ok(())
        })
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::TimedOut
        );
    }
    fn key() -> KeyPair {
        KeyPair::try_from_seed(vec![37; 32], Algorithm::Ed25519).unwrap()
    }
    fn manifest() -> ManifestV1 {
        let trust = lane_manifest::test_trust();
        let network_id = NetworkId::from_genesis_hash(
            iroha_genesis::decode_signed_genesis(
                &hex::decode(&trust.genesis_signed_wire_hex).unwrap(),
            )
            .unwrap()
            .hash(),
        );
        let native_manifest = lane_manifest::generate("devex", &trust).unwrap();
        let owner = AccountId::new(key().public_key().clone());
        let grant = AliasDataspaceBootstrapGrantV1::try_new("devex", owner.clone()).unwrap();
        let asset: AssetDefinitionId = "6TEAJqbb8oEPmLncoNiMRbLEK6tw".parse().unwrap();
        let guard = AliasQuoteGuardV1 {
            expected_policy_version: 1,
            expected_payment_asset: asset.clone(),
            max_amount: amount(5, 1),
            valid_until_ms: u64::MAX,
        };
        let ds = EnsureAlias::new(
            AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
                dataspace: grant.dataspace.clone(),
                owner: owner.clone(),
            }),
            AliasLeaseAcquisitionV1::new(1, None),
            guard.clone(),
        );
        let account = EnsureAlias::new(
            AliasIntentV1::AccountAlias(AliasAccountIntentV1 {
                alias: ResolvedAccountAliasV1::new(
                    "admin@devex".parse().unwrap(),
                    grant.dataspace.dataspace_id,
                ),
                target_account: owner.clone(),
                provision: AccountProvisionV1::Existing,
                role: AccountAliasRoleV1::Additional,
            }),
            AliasLeaseAcquisitionV1::new(1, None),
            guard,
        );
        ManifestV1 {
            finality: trust,
            schema_version: 1,
            operation_id: None,
            network_id,
            owner,
            dataspace: RuntimeDataSpaceAdditionV1 {
                descriptor: DataSpaceMetadata {
                    id: grant.dataspace.dataspace_id,
                    alias: "devex".into(),
                    description: None,
                    fault_tolerance: 1,
                },
                manifest_hash: grant.name_hash,
            },
            lane: LaneConfig {
                id: LaneId::new(6),
                alias: "devex".into(),
                dataspace_id: grant.dataspace.dataspace_id,
                ..LaneConfig::default()
            },
            lane_manifest: RuntimeLaneManifestV1 {
                lane_id: LaneId::new(6),
                manifest: native_manifest,
            },
            alias_request: AliasSetupPlanRequestV1::new(vec![ds, account]),
            spending: SpendingV1 {
                asset_definition_id: asset,
                alias_create_maximum: amount(5, 1),
                transaction_fee_maximum: amount(1, 0),
            },
        }
    }
    fn alias_plan(manifest: &ManifestV1) -> AliasTransactionPlanV1 {
        let mut frames = Vec::new();
        let resources = manifest
            .alias_request
            .intents
            .iter()
            .enumerate()
            .map(|(index, ensure)| {
                let instruction: InstructionBox = ensure.clone().into();
                let (wire_id, framed_payload) =
                    iroha_data_model::isi::framed_instruction_payload(&instruction).unwrap();
                frames.push(AliasFramedInstructionV1 {
                    wire_id: wire_id.into(),
                    framed_payload,
                });
                AliasPlanResourceV1 {
                    intent: ensure.intent.clone(),
                    disposition: AliasPlanDispositionV1::Create,
                    quote: Some(AliasLeaseQuoteV1 {
                        target: ensure.intent.target(),
                        pricing_class: 0,
                        exact_amount: amount(5, 1),
                        guard: ensure.quote_guard.clone(),
                        expires_at_ms: 100,
                        grace_expires_at_ms: 200,
                        redemption_expires_at_ms: 300,
                    }),
                    instruction_index: Some(index as u32),
                }
            })
            .collect();
        AliasTransactionPlanV1::new(AliasTransactionPlanBodyV1 {
            version: 1,
            authority: manifest.owner.clone(),
            network_id: manifest.network_id,
            anchor: AliasPlanAnchorV1 {
                block_height: 1,
                block_hash: Hash::new(b"alias anchor"),
            },
            resources,
            instructions: frames,
            totals_by_asset: vec![AliasAssetTotalV1 {
                payment_asset: manifest.spending.asset_definition_id.clone(),
                amount: amount(1, 0),
            }],
            warnings: Vec::new(),
            blockers: Vec::new(),
            valid_until_ms: 9_000_000_000_000,
        })
    }
    fn fixture_plan() -> PlanV1 {
        let manifest = manifest();
        let lanes = vec![
            LaneConfig::default(),
            LaneConfig {
                id: LaneId::new(7),
                alias: "is".into(),
                dataspace_id: DataSpaceId::new(77),
                ..LaneConfig::default()
            },
        ];
        let incarnations: BTreeMap<_, _> = lanes
            .iter()
            .map(|lane| (lane.id, Hash::new(lane.alias.as_bytes())))
            .collect();
        let catalog = LaneCatalog::new(NonZeroU32::new(8).unwrap(), lanes).unwrap();
        let baseline = LaneLifecycleStatusV1::new(&catalog, &incarnations, None).unwrap();
        PlanV1 {
            schema_version: 1,
            operation_id: manifest.resolved_id().unwrap(),
            intent_sha256: manifest.intent_digest().unwrap(),
            bootstrap_grant: manifest.validate().unwrap(),
            catalog_transition: transition(&manifest, &baseline).unwrap(),
            initial_alias_plan: alias_plan(&manifest),
            manifest,
            baseline,
            baseline_overlay: None,
        }
    }
    fn prepared(plan: &PlanV1) -> PreparedV1 {
        let instructions: Vec<InstructionBox> = vec![
            SetParameter::new(Parameter::Custom(
                plan.catalog_transition
                    .clone()
                    .into_custom_parameter()
                    .unwrap(),
            ))
            .into(),
        ];
        let intent = FeePaymentIntent::authority(
            vec![FeeChargeLimit::new(
                FeeChargeKind::Nexus,
                plan.manifest.spending.asset_definition_id.clone(),
                amount(1, 1),
            )],
            None,
        );
        let transaction = TransactionBuilder::new(
            plan.manifest.network_id,
            plan.manifest.owner.clone(),
            intent.clone(),
        )
        .with_instructions(instructions.clone())
        .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
        .try_sign(key().private_key())
        .unwrap();
        let quote = FeeQuoteResponse {
            intent,
            observation: FeeQuoteObservation {
                ledger_time_ms: 1,
                next_block_height: 2,
                route_dataspace_id: DataSpaceId::UNIVERSAL,
            },
            components: vec![FeeQuoteComponent {
                kind: FeeChargeKind::Nexus,
                asset_definition_id: plan.manifest.spending.asset_definition_id.clone(),
                max_amount: amount(1, 1),
            }],
            capacities: Vec::new(),
            decision: FeeQuoteDecision::Accepted {
                debit_source: FeeDebitSource::Account(plan.manifest.owner.clone()),
                program_revision: None,
            },
        };
        PreparedV1 {
            schema_version: 1,
            operation_id: plan.operation_id.clone(),
            intent_sha256: plan.intent_sha256.clone(),
            phase: "catalog".into(),
            signed_transaction_wire_hex: hex::encode(transaction.encode_wire_v1().unwrap()),
            transaction_hash: hex::encode(transaction.hash().as_ref()),
            instructions,
            fee_quote: quote,
            alias_plan: None,
        }
    }

    #[test]
    fn manifest_binds_native_identity_and_spending_limits() {
        let value = manifest();
        value.validate().unwrap();
        let mut wrong = value.clone();
        wrong.dataspace.manifest_hash[0] ^= 1;
        assert!(wrong.validate().is_err());
        let mut wrong = value.clone();
        wrong.lane_manifest.lane_id = LaneId::new(5);
        assert!(wrong.validate().is_err());
        for (field, changed) in [
            ("lane", norito::json!("another-lane")),
            ("quorum", norito::json!(2)),
            ("validators", norito::json!([])),
            ("unknown", norito::json!(true)),
        ] {
            let mut wrong = value.clone();
            let mut native: json::Value =
                json::from_str(wrong.lane_manifest.manifest.get()).unwrap();
            native
                .as_object_mut()
                .unwrap()
                .insert(field.into(), changed);
            wrong.lane_manifest.manifest = Json::new(native);
            assert!(wrong.validate().is_err(), "accepted invalid native {field}");
        }
        let mut wrong = value.clone();
        let mut native: json::Value = json::from_str(wrong.lane_manifest.manifest.get()).unwrap();
        native
            .as_object_mut()
            .unwrap()
            .get_mut("validators")
            .unwrap()
            .as_array_mut()
            .unwrap()[0]
            .as_object_mut()
            .unwrap()
            .insert(
                "torii_url".into(),
                norito::json!("https://unselected.example/"),
            );
        wrong.lane_manifest.manifest = Json::new(native);
        assert!(wrong.validate().is_err());
        let mut wrong = value.clone();
        wrong.lane.alias = "another-lane".into();
        assert!(wrong.validate().is_err());
        let mut wrong = value.clone();
        wrong.dataspace.descriptor.fault_tolerance = 2;
        assert!(wrong.validate().is_err());
        let mut wrong = value.clone();
        wrong.alias_request.intents[0].quote_guard.max_amount = amount(6, 1);
        assert!(wrong.validate().is_err());
        let mut wrong = value.clone();
        wrong.alias_request.intents.pop();
        assert!(wrong.validate().is_err());
        let mut json = json::to_value(&value).unwrap();
        json.as_object_mut()
            .unwrap()
            .insert("unknown".into(), norito::json!(1));
        assert!(json::from_value::<ManifestV1>(json).is_err());
    }
    #[test]
    fn operation_id_is_stable_for_equivalent_intent() {
        let value = manifest();
        let mut reordered = value.clone();
        reordered.alias_request.intents.reverse();
        assert_eq!(
            value.resolved_id().unwrap(),
            reordered.resolved_id().unwrap()
        );
        let mut changed = value.clone();
        changed.spending.transaction_fee_maximum = amount(2, 0);
        assert_ne!(value.resolved_id().unwrap(), changed.resolved_id().unwrap());
        assert!(operation_id("../escape").is_err());
        assert!(operation_id("").is_err());
        let mut explicit = value;
        explicit.operation_id = Some("release-one".into());
        assert_eq!(explicit.resolved_id().unwrap(), "release-one");
    }
    #[test]
    fn catalog_transition_preserves_sparse_baseline_and_cas() {
        let value = fixture_plan();
        assert_eq!(value.baseline.lane_count, 8);
        assert!(
            !value
                .baseline
                .lanes
                .iter()
                .any(|lane| lane.id == LaneId::new(5))
        );
        assert_eq!(
            value.catalog_transition.expected_catalog_hash,
            value.baseline.catalog_hash
        );
        assert_eq!(
            value.catalog_transition.expected_incarnation_root,
            value.baseline.incarnation_root
        );
        assert_eq!(
            value.catalog_transition.lane_additions,
            vec![value.manifest.lane.clone()]
        );
        let mut collision = value.manifest.clone();
        collision.lane.id = LaneId::new(7);
        assert!(transition(&collision, &value.baseline).is_err());
        let mut forged = value.baseline.clone();
        forged.catalog_hash = Hash::new(b"foreign catalog");
        assert!(transition(&value.manifest, &forged).is_err());
    }
    #[test]
    fn retained_phase_rejects_changed_wire_owner_fee_or_instructions() {
        let plan = fixture_plan();
        let value = prepared(&plan);
        value.verify(&plan, "catalog").unwrap();
        let mut wrong = value.clone();
        wrong.signed_transaction_wire_hex.push_str("00");
        assert!(wrong.verify(&plan, "catalog").is_err());
        let mut wrong = value.clone();
        wrong.instructions.clear();
        assert!(wrong.verify(&plan, "catalog").is_err());
        let mut wrong = value.clone();
        wrong.fee_quote.components[0].max_amount = amount(1, 2);
        assert!(wrong.verify(&plan, "catalog").is_err());
        let mut wrong = plan.clone();
        wrong.manifest.owner = AccountId::new(
            KeyPair::try_from_seed(vec![38; 32], Algorithm::Ed25519)
                .unwrap()
                .public_key()
                .clone(),
        );
        assert!(value.verify(&wrong, "catalog").is_err());
        assert!(value.verify(&plan, "bootstrap").is_err());
    }
    #[test]
    fn namespace_plan_requires_two_bounded_paid_creates() {
        let manifest = manifest();
        let plan = alias_plan(&manifest);
        assert_eq!(validate_paid_plan(&manifest, &plan).unwrap().len(), 2);
        let mut changed = plan.clone();
        changed.body.resources[0].disposition = AliasPlanDispositionV1::NoOp;
        changed = AliasTransactionPlanV1::new(changed.body);
        assert!(validate_paid_plan(&manifest, &changed).is_err());
        let mut changed = manifest.clone();
        changed.spending.alias_create_maximum = amount(4, 1);
        assert!(validate_paid_plan(&changed, &plan).is_err());
        let mut changed = plan.clone();
        changed.body.instructions[0].framed_payload.push(0);
        changed = AliasTransactionPlanV1::new(changed.body);
        assert!(validate_paid_plan(&manifest, &changed).is_err());
    }
    #[test]
    fn journal_creates_and_reopens_relative_output_directory() {
        use std::os::unix::fs::PermissionsExt as _;

        const CHILD_ENV: &str = "IROHA_CLI_TEST_RELATIVE_JOURNAL_CHILD";
        if std::env::var_os(CHILD_ENV).is_some() {
            let path = Path::new("fresh-output");
            let journal = Journal::open(path, true).expect("create relative output journal");
            assert_eq!(journal.path, std::env::current_dir().unwrap().join(path));
            journal
                .install_json("intent.json", &"retained intent")
                .unwrap();
            drop(journal);

            let reopened = Journal::open(path, false).expect("reopen relative output journal");
            assert_eq!(
                reopened.read_json::<String>("intent.json").unwrap(),
                "retained intent"
            );
            reopened.revalidate().unwrap();
            return;
        }

        // Run in an isolated private working directory without changing the
        // process-global cwd used by concurrently executing tests.
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "taira_dataspace_deploy::tests::journal_creates_and_reopens_relative_output_directory",
                "--nocapture",
            ])
            .env(CHILD_ENV, "1")
            .current_dir(root.path())
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "relative journal child failed:\n{}\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
        assert!(root.path().join("fresh-output/intent.json").is_file());
    }

    #[test]
    fn unplanned_operation_recovers_only_before_any_phase_evidence() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let before_lock = root.path().join("before-lock");
        fs::create_dir(&before_lock).unwrap();
        fs::set_permissions(&before_lock, fs::Permissions::from_mode(0o700)).unwrap();
        let recovered = Journal::open_unpublished(&before_lock).unwrap();
        recovered.require_unplanned().unwrap();
        drop(recovered);

        let before_plan = root.path().join("before-plan");
        let journal = Journal::open_unpublished(&before_plan).unwrap();
        journal.require_unplanned().unwrap();
        drop(journal);
        let resumed = Journal::open_unpublished(&before_plan).unwrap();
        resumed.require_unplanned().unwrap();
        resumed
            .install_json("plan.json", &"immutable plan")
            .unwrap();
        drop(resumed);
        assert_eq!(
            Journal::open_unpublished(&before_plan)
                .unwrap()
                .read_json::<String>("plan.json")
                .unwrap(),
            "immutable plan"
        );
        fs::remove_file(before_plan.join("lock")).unwrap();
        assert!(Journal::open_unpublished(&before_plan).is_err());
        assert!(!before_plan.join("lock").exists());

        let suspicious = root.path().join("evidence-without-plan");
        let journal = Journal::open_unpublished(&suspicious).unwrap();
        journal
            .install_json("catalog.prepared.json", &"signed wire")
            .unwrap();
        assert!(journal.require_unplanned().is_err());
    }

    #[test]
    fn journal_dispatch_claim_is_durable_and_exclusive() {
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(
            root.path(),
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .unwrap();
        let path = root.path().join("operation");
        let value = prepared(&fixture_plan());
        let journal = Journal::open(&path, true).unwrap();
        journal
            .install_json("catalog.prepared.json", &value)
            .unwrap();
        assert!(record_dispatch_claim(&journal, "catalog.submitted.json", &value).unwrap());
        assert!(!record_dispatch_claim(&journal, "catalog.submitted.json", &value).unwrap());
        assert!(Journal::open(&path, false).is_err());
        drop(journal);
        let journal = Journal::open(&path, false).unwrap();
        assert!(!record_dispatch_claim(&journal, "catalog.submitted.json", &value).unwrap());
        let mut other = value;
        other.transaction_hash.push('0');
        assert!(record_dispatch_claim(&journal, "catalog.submitted.json", &other).is_err());
        fs::rename(path.join("lock"), path.join("retained-lock")).unwrap();
        File::create(path.join("lock"))
            .unwrap()
            .set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))
            .unwrap();
        assert!(journal.revalidate().is_err());
        assert!(
            journal
                .install_json("after-lock-change.json", &"refused")
                .is_err()
        );
    }
    #[test]
    fn journal_content_revalidation_preserves_offset_and_rejects_metadata_collisions() {
        use std::{
            io::{Seek as _, SeekFrom},
            os::unix::fs::PermissionsExt as _,
        };
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = root.path().join("operation");
        let journal = Journal::open(&path, true).unwrap();
        let expected = vec![0xA5; 64 * 1024 + 3];
        journal.install("custody.nrt", &expected).unwrap();
        let mut held = File::open(path.join("custody.nrt")).unwrap();
        held.seek(SeekFrom::Start(7)).unwrap();
        let before = held.metadata().unwrap();
        journal
            .revalidate_file("custody.nrt", &held, &before, &expected)
            .unwrap();
        assert_eq!(held.stream_position().unwrap(), 7);

        let mut changed = expected.clone();
        *changed.last_mut().unwrap() ^= 1;
        fs::write(path.join("custody.nrt"), &changed).unwrap();
        // Model a filesystem clock collision deterministically: the retained
        // content is unchanged while every compared metadata field agrees.
        let indistinguishable = held.metadata().unwrap();
        assert!(same_file_snapshot(
            &indistinguishable,
            &fs::metadata(path.join("custody.nrt")).unwrap()
        ));
        let error = journal
            .revalidate_file("custody.nrt", &held, &indistinguishable, &expected)
            .unwrap_err();
        assert_eq!(error.to_string(), "journal content changed during custody");
        assert_eq!(held.stream_position().unwrap(), 7);
        journal
            .revalidate_file("custody.nrt", &held, &indistinguishable, &changed)
            .unwrap();
        assert_eq!(held.stream_position().unwrap(), 7);

        for length in [expected.len() - 1, expected.len() + 1] {
            fs::write(path.join("custody.nrt"), vec![0xA5; length]).unwrap();
            assert!(
                journal
                    .revalidate_file("custody.nrt", &held, &held.metadata().unwrap(), &expected)
                    .is_err()
            );
            assert_eq!(held.stream_position().unwrap(), 7);
        }
    }

    #[test]
    fn journal_rejects_links_replacement_and_incomplete_records() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(
            root.path(),
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .unwrap();
        let path = root.path().join("operation");
        let journal = Journal::open(&path, true).unwrap();
        fs::write(
            path.join(".staging-interrupted"),
            b"partial bytes before publication",
        )
        .unwrap();
        assert!(journal.read_optional("after-crash.json").unwrap().is_none());
        journal
            .install_json("after-crash.json", &"complete")
            .unwrap();
        assert_eq!(
            journal.read_json::<String>("after-crash.json").unwrap(),
            "complete"
        );
        assert!(
            journal
                .install_json("after-crash.json", &"replacement")
                .is_err()
        );
        assert!(journal.install_bounded("carrier.nrt", b"wire", 3).is_err());
        journal.install_bounded("carrier.nrt", b"wire", 4).unwrap();
        assert!(journal.read_optional_bounded("carrier.nrt", 3).is_err());
        assert_eq!(
            journal
                .read_optional_bounded("carrier.nrt", 4)
                .unwrap()
                .as_deref(),
            Some(b"wire".as_slice())
        );
        journal.install("custody.nrt", b"before").unwrap();
        let retained = File::open(path.join("custody.nrt")).unwrap();
        let before = retained.metadata().unwrap();
        fs::write(path.join("custody.nrt"), b"edited").unwrap();
        assert!(
            journal
                .revalidate_file("custody.nrt", &retained, &before, b"before")
                .is_err()
        );
        let before = retained.metadata().unwrap();
        fs::rename(path.join("custody.nrt"), path.join("retained-custody.nrt")).unwrap();
        fs::copy(path.join("retained-custody.nrt"), path.join("custody.nrt")).unwrap();
        assert!(
            journal
                .revalidate_file("custody.nrt", &retained, &before, b"edited")
                .is_err()
        );
        journal.install("broken.json", b"{ incomplete").unwrap();
        let malformed = journal
            .optional_json::<ManifestV1>("broken.json")
            .unwrap_err();
        assert!(
            malformed
                .to_string()
                .contains("retained journal file `broken.json`")
        );
        assert!(
            malformed.downcast_ref::<json::Error>().is_some(),
            "filename context must retain the native JSON decoder cause: {malformed:#}"
        );
        symlink(path.join("broken.json"), path.join("linked.json")).unwrap();
        assert!(journal.read_optional("linked.json").is_err());
        fs::hard_link(path.join("broken.json"), path.join("hard.json")).unwrap();
        assert!(journal.read_optional("hard.json").is_err());
        fs::rename(&path, root.path().join("moved")).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(journal.revalidate().is_err());
    }
    #[test]
    fn status_requires_exact_global_and_peer_state_applied() {
        let hash = "ab".repeat(32);
        let status = |scope: &str| {
            Some(PipelineTransactionStatusResponse {
                hash: hash.clone(),
                scope: scope.into(),
                resolved_from: "state".into(),
                status: PipelineTransactionStatus {
                    kind: "Applied".into(),
                    block_height: Some(3),
                },
            })
        };
        let global = status("global");
        let peer = status("local");
        assert_eq!(
            matching_applied_height(&hash, &global, &peer).unwrap(),
            Some(3)
        );
        let mut cache = peer.clone();
        cache.as_mut().unwrap().resolved_from = "cache".into();
        assert_eq!(
            matching_applied_height(&hash, &global, &cache).unwrap(),
            None
        );
        assert!(matching_applied_height(&hash, &global, &global).is_err());
        let mut other = peer.clone();
        other.as_mut().unwrap().status.block_height = Some(4);
        assert!(matching_applied_height(&hash, &global, &other).is_err());
        let mut zero = peer;
        zero.as_mut().unwrap().status.block_height = Some(0);
        assert!(matching_applied_height(&hash, &global, &zero).is_err());
        let mut queued = global.clone();
        queued.as_mut().unwrap().status.kind = "Queued".into();
        queued.as_mut().unwrap().status.block_height = None;
        queued.as_mut().unwrap().resolved_from = "queue".into();
        for global_pending in [None, queued] {
            for field in [
                "kind",
                "source",
                "scope",
                "hash",
                "height",
                "missing-height",
            ] {
                let mut malformed = status("local");
                let value = malformed.as_mut().unwrap();
                match field {
                    "kind" => value.status.kind = "Unknown".into(),
                    "source" => value.resolved_from = "untrusted".into(),
                    "scope" => value.scope = "global".into(),
                    "hash" => value.hash = "cd".repeat(32),
                    "height" => value.status.block_height = Some(0),
                    "missing-height" => value.status.block_height = None,
                    _ => unreachable!(),
                }
                assert!(
                    matching_applied_height(&hash, &global_pending, &malformed).is_err(),
                    "{field}"
                );
            }
            assert_eq!(
                matching_applied_height(&hash, &global_pending, &status("local")).unwrap(),
                None
            );
        }
        let report = phase_report(&fixture_plan(), Vec::new());
        assert!(!report.deployment_complete);
        assert!(
            report
                .verification
                .authenticated_execution_commitment_required
        );
    }
    fn init_args() -> InitArgs {
        InitArgs {
            dataspace: "devex".into(),
            lane_id: 6,
            lane_profile: LaneProfile::RestrictedFullReplica,
            account_alias: "admin".into(),
            trust: PathBuf::from("trust.json"),
            payment_asset: manifest().spending.asset_definition_id,
            alias_create_maximum: amount(5, 1),
            transaction_fee_maximum: amount(1, 0),
            lease_years: 1,
            quote_lifetime_secs: 3600,
            operation_id: None,
            output_dir: PathBuf::from("fresh-output"),
        }
    }
    fn init_policies(args: &InitArgs) -> [iroha_data_model::sns::SuffixPolicyV1; 2] {
        use iroha_data_model::sns::{
            ACCOUNT_ALIAS_SUFFIX_ID, DATASPACE_ALIAS_SUFFIX_ID, fixtures::default_policy,
        };
        let mut first = default_policy();
        first.suffix_id = DATASPACE_ALIAS_SUFFIX_ID;
        first.policy_version = 7;
        first.payment_asset_id = args.payment_asset.to_string();
        let mut second = first.clone();
        second.suffix_id = ACCOUNT_ALIAS_SUFFIX_ID;
        second.policy_version = 9;
        [first, second]
    }
    #[test]
    fn init_builds_native_restricted_intent_from_policy_and_profile() {
        use iroha_data_model::nexus::{LaneStorageProfile, LaneVisibility};
        let args = init_args();
        let policies = init_policies(&args);
        let reference = manifest();
        let value = init_manifest(
            &args,
            reference.network_id,
            reference.owner.clone(),
            reference.finality.clone(),
            &policies,
            9_000_000_000_000,
        )
        .unwrap();
        assert_eq!(value.lane.visibility, LaneVisibility::Restricted);
        assert_eq!(value.lane.storage, LaneStorageProfile::FullReplica);
        assert_eq!(value.dataspace.descriptor.fault_tolerance, 1);
        assert_eq!(value.dataspace, reference.dataspace);
        assert_eq!(
            value.alias_request.intents[0]
                .quote_guard
                .expected_policy_version,
            7
        );
        assert_eq!(
            value.alias_request.intents[1]
                .quote_guard
                .expected_policy_version,
            9
        );
        assert_eq!(
            value.alias_request.intents[1].quote_guard.max_amount,
            amount(5, 1)
        );
        assert!(value.lane.metadata.is_empty());
        let mut public = args;
        public.lane_profile = LaneProfile::PublicFullReplica;
        let value = init_manifest(
            &public,
            reference.network_id,
            reference.owner,
            reference.finality.clone(),
            &policies,
            9_000_000_000_000,
        )
        .unwrap();
        assert_eq!(value.lane.visibility, LaneVisibility::Public);
    }
    #[test]
    fn init_rejects_policy_drift_and_parses_explicit_caps() {
        use clap::Parser as _;
        #[derive(clap::Parser)]
        struct Wrapper {
            #[command(subcommand)]
            command: Command,
        }
        let args = init_args();
        let reference = manifest();
        let mut policies = init_policies(&args);
        policies[0].status = iroha_data_model::sns::SuffixStatus::Paused;
        assert!(
            init_manifest(
                &args,
                reference.network_id,
                reference.owner.clone(),
                reference.finality.clone(),
                &policies,
                9_000_000_000_000
            )
            .is_err()
        );
        policies = init_policies(&args);
        policies[1].min_term_years = 2;
        assert!(
            init_manifest(
                &args,
                reference.network_id,
                reference.owner.clone(),
                reference.finality.clone(),
                &policies,
                9_000_000_000_000
            )
            .is_err()
        );
        policies = init_policies(&args);
        policies[1].payment_asset_id = "invalid-asset".into();
        assert!(
            init_manifest(
                &args,
                reference.network_id,
                reference.owner,
                reference.finality.clone(),
                &policies,
                9_000_000_000_000
            )
            .is_err()
        );
        let argv = [
            "test",
            "init",
            "--dataspace",
            "devex",
            "--lane-id",
            "6",
            "--lane-profile",
            "restricted-full-replica",
            "--account-alias",
            "admin",
            "--trust",
            "trust.json",
            "--payment-asset",
            "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
            "--alias-create-maximum",
            "0.5",
            "--transaction-fee-maximum",
            "1",
            "--output-dir",
            "new",
        ];
        let parsed = Wrapper::try_parse_from(argv).unwrap();
        let Command::Init(init) = parsed.command else {
            panic!("init command expected");
        };
        assert_eq!(init.alias_create_maximum, amount(5, 1));
        assert_eq!(init.transaction_fee_maximum, amount(1, 0));
        assert!(Wrapper::try_parse_from(&argv[..argv.len() - 2]).is_err());
        let mut retired = argv.to_vec();
        retired.extend(["--lane-manifest", "handwritten.json"]);
        assert!(Wrapper::try_parse_from(retired).is_err());
    }
}
