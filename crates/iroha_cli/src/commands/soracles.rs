//! Soracles evidence helpers (audit bundle generation).

use std::{
    collections::{BTreeMap, HashSet},
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use clap::{Args, Subcommand, ValueEnum};
use eyre::{Result, WrapErr, eyre};
use iroha::data_model::{
    events::data::oracle::FeedEventRecord,
    isi::{InstructionBox, oracle as oracle_isi},
    nexus::UniversalAccountId,
    oracle::{
        FeedConfig, FeedConfigVersion, FeedEventOutcome, FeedId, FeedSlot, KeyedHash, Observation,
        OracleChangeClass, OracleChangeId, OracleChangeStage, OracleDisputeId,
        OracleDisputeOutcome, OracleProviderKey, OracleRejectionCode, DefiOracleAttestation,
        DefiOracleAttestationKey, TwitterBindingAttestation,
    },
    prelude::{Hash, QueryBuilderExt},
    query::oracle::prelude as oracle_query,
};
use iroha_primitives::numeric::Numeric;
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json::{self, JsonDeserializeOwned, Value},
};

use crate::cli_output::print_with_optional_text;
use crate::{Run, RunContext};

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Build and submit oracle transactions.
    #[command(subcommand)]
    Tx(TxCommand),
    /// Run oracle queries.
    #[command(subcommand)]
    Query(QueryCommand),
    /// Build an audit bundle containing oracle feed events and evidence files.
    Bundle(Bundle),
    /// Show the oracle rejection/error catalog for SDK parity.
    Catalog(Catalog),
    /// Garbage-collect evidence bundles and prune unreferenced artifacts.
    #[command(name = "evidence-gc")]
    Gc(GcArgs),
}

#[derive(Args, Debug)]
pub struct Bundle {
    /// Path to a JSON file containing `FeedEventRecord` values (array or single record).
    #[arg(long, value_name = "PATH")]
    events: PathBuf,
    /// Directory where the bundle (manifest + hashed artefacts) will be written.
    #[arg(long, value_name = "DIR")]
    output: PathBuf,
    /// Directory of observation JSON files to include (hashed and copied into the bundle).
    #[arg(long, value_name = "DIR")]
    observations: Option<PathBuf>,
    /// Directory of report JSON files to include.
    #[arg(long, value_name = "DIR")]
    reports: Option<PathBuf>,
    /// Directory of connector response JSON files to include.
    #[arg(long, value_name = "DIR")]
    responses: Option<PathBuf>,
    /// Directory of dispute evidence JSON files to include.
    #[arg(long, value_name = "DIR")]
    disputes: Option<PathBuf>,
    /// Optional telemetry snapshot (JSON) to include in the bundle.
    #[arg(long, value_name = "PATH")]
    telemetry: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct Catalog {
    /// Output format (`json` for machine consumption, `markdown` for docs/runbooks).
    ///
    /// Ignored when `--output-format json` is used.
    #[arg(long, value_enum, default_value_t = CatalogFormat::Json)]
    format: CatalogFormat,
}

/// Prune expired soracles evidence bundles and unreferenced artifacts.
#[derive(Args, Debug)]
pub struct GcArgs {
    /// Root directory containing soracles evidence bundles (each with `bundle.json`).
    #[arg(long, value_name = "DIR", default_value = "artifacts/soracles")]
    root: PathBuf,
    /// Retention period in days; bundles older than this are removed.
    #[arg(long, value_name = "DAYS", default_value_t = 180)]
    retention_days: u64,
    /// Retention period for bundles containing dispute evidence (defaults to a longer window).
    #[arg(long, value_name = "DAYS", default_value_t = 365)]
    dispute_retention_days: u64,
    /// Emit a GC summary report to this path (defaults to `<root>/gc_report.json`).
    #[arg(long, value_name = "PATH")]
    report: Option<PathBuf>,
    /// Remove artifact files that are not referenced by `bundle.json`.
    #[arg(long)]
    prune_unreferenced: bool,
    /// Perform a dry run and only report what would be removed.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CatalogFormat {
    Json,
    Markdown,
}

#[derive(Subcommand, Debug)]
pub enum TxCommand {
    /// Register an oracle feed from a Norito JSON feed config file.
    Register(RegisterTx),
    /// Submit a provider-signed observation from a Norito JSON file.
    Submit(SubmitTx),
    /// Aggregate a feed window.
    Aggregate(AggregateTx),
    /// Open an oracle dispute anchored to retained feed history.
    #[command(name = "open-dispute")]
    OpenDispute(OpenDisputeTx),
    /// Resolve an oracle dispute.
    #[command(name = "resolve-dispute")]
    ResolveDispute(ResolveDisputeTx),
    /// Propose an oracle feed change.
    #[command(name = "propose-change")]
    ProposeChange(ProposeChangeTx),
    /// Vote in the active stage of an oracle feed change.
    #[command(name = "vote-change-stage")]
    VoteChangeStage(VoteChangeStageTx),
    /// Roll back the active stage of an oracle feed change.
    #[command(name = "rollback-change")]
    RollbackChange(RollbackChangeTx),
    /// Submit a native DeFi oracle attestation.
    #[command(name = "attest-defi")]
    AttestDefi(AttestDefiTx),
    /// Record a Twitter binding attestation.
    #[command(name = "record-twitter-binding")]
    RecordTwitterBinding(RecordTwitterBindingTx),
    /// Revoke a Twitter binding.
    #[command(name = "revoke-twitter-binding")]
    RevokeTwitterBinding(RevokeTwitterBindingTx),
}

#[derive(Subcommand, Debug)]
pub enum QueryCommand {
    /// List all registered oracle feeds.
    Feeds,
    /// Fetch one oracle feed.
    Feed(FeedQuery),
    /// List retained feed history.
    History(FeedQuery),
    /// List provider stats for a feed or fetch one provider stats record.
    #[command(name = "provider-stats")]
    ProviderStats(ProviderStatsQuery),
    /// List oracle disputes, optionally filtered by feed id.
    Disputes(DisputesQuery),
    /// Fetch one oracle dispute.
    Dispute(DisputeQuery),
    /// List oracle changes.
    Changes,
    /// Fetch one oracle change.
    Change(ChangeQuery),
    /// List Twitter bindings by UAID from a Norito JSON UAID file.
    #[command(name = "twitter-bindings")]
    TwitterBindings(TwitterBindingsQuery),
    /// Fetch latest DeFi oracle attestation by domain and subject id.
    #[command(name = "defi-attestation")]
    DefiAttestation(DefiAttestationQuery),
}

#[derive(Args, Debug)]
pub struct RegisterTx {
    /// Norito JSON file containing `FeedConfig`.
    #[arg(long, value_name = "PATH")]
    feed_json: PathBuf,
}

#[derive(Args, Debug)]
pub struct SubmitTx {
    /// Norito JSON file containing `Observation`.
    #[arg(long, value_name = "PATH")]
    observation_json: PathBuf,
}

#[derive(Args, Debug)]
pub struct AggregateTx {
    /// Feed identifier.
    #[arg(long)]
    feed_id: FeedId,
    /// Slot to aggregate.
    #[arg(long)]
    slot: FeedSlot,
    /// Request hash for the window.
    #[arg(long)]
    request_hash: Hash,
    /// Evidence hashes to anchor on the resulting feed event.
    #[arg(long = "evidence-hash")]
    evidence_hashes: Vec<Hash>,
}

#[derive(Args, Debug)]
pub struct OpenDisputeTx {
    /// Feed identifier.
    #[arg(long)]
    feed_id: FeedId,
    /// Disputed slot.
    #[arg(long)]
    slot: FeedSlot,
    /// Request hash for the disputed window.
    #[arg(long)]
    request_hash: Hash,
    /// Provider being challenged.
    #[arg(long)]
    target: String,
    /// Optional bond amount; defaults to oracle economics config.
    #[arg(long)]
    bond: Option<Numeric>,
    /// Evidence hashes backing the dispute.
    #[arg(long = "evidence-hash")]
    evidence_hashes: Vec<Hash>,
    /// Human-readable reason for the dispute.
    #[arg(long, default_value = "")]
    reason: String,
}

#[derive(Args, Debug)]
pub struct ResolveDisputeTx {
    /// Dispute identifier.
    #[arg(long)]
    dispute_id: u64,
    /// Resolution outcome.
    #[arg(long, value_enum)]
    outcome: DisputeOutcomeArg,
    /// Optional notes retained by clients/auditors.
    #[arg(long, default_value = "")]
    notes: String,
}

#[derive(Args, Debug)]
pub struct ProposeChangeTx {
    /// Change id hash.
    #[arg(long)]
    change_id: Hash,
    /// Norito JSON file containing proposed `FeedConfig`.
    #[arg(long, value_name = "PATH")]
    feed_json: PathBuf,
    /// Governance class for the proposal.
    #[arg(long, value_enum)]
    class: ChangeClassArg,
    /// Hash of the off-chain change manifest.
    #[arg(long)]
    payload_hash: Hash,
    /// Evidence hashes attached to intake.
    #[arg(long = "evidence-hash")]
    evidence_hashes: Vec<Hash>,
}

#[derive(Args, Debug)]
pub struct VoteChangeStageTx {
    /// Change id hash.
    #[arg(long)]
    change_id: Hash,
    /// Stage being voted. Must be the active stage.
    #[arg(long, value_enum)]
    stage: ChangeStageArg,
    /// Approve the stage; pass `false` to reject.
    #[arg(long, default_value_t = true)]
    approve: bool,
    /// Evidence hashes attached to this stage vote.
    #[arg(long = "evidence-hash")]
    evidence_hashes: Vec<Hash>,
}

#[derive(Args, Debug)]
pub struct RollbackChangeTx {
    /// Change id hash.
    #[arg(long)]
    change_id: Hash,
    /// Optional stage to roll back. If omitted, rolls back the active stage.
    #[arg(long, value_enum)]
    stage: Option<ChangeStageArg>,
    /// Human-readable rollback reason.
    #[arg(long)]
    reason: String,
}

#[derive(Args, Debug)]
pub struct RecordTwitterBindingTx {
    /// Norito JSON file containing `TwitterBindingAttestation`.
    #[arg(long, value_name = "PATH")]
    attestation_json: PathBuf,
    /// Feed identifier for the binding feed.
    #[arg(long)]
    feed_id: FeedId,
}

#[derive(Args, Debug)]
pub struct RevokeTwitterBindingTx {
    /// Norito JSON file containing the keyed binding hash.
    #[arg(long, value_name = "PATH")]
    binding_hash_json: PathBuf,
    /// Human-readable revocation reason.
    #[arg(long, default_value = "")]
    reason: String,
}

#[derive(Args, Debug)]
pub struct AttestDefiTx {
    /// Norito JSON file containing `DefiOracleAttestation`.
    #[arg(long, value_name = "PATH")]
    attestation_json: PathBuf,
}

#[derive(Args, Debug)]
pub struct FeedQuery {
    /// Feed identifier.
    #[arg(long)]
    feed_id: FeedId,
}

#[derive(Args, Debug)]
pub struct ProviderStatsQuery {
    /// Feed identifier.
    #[arg(long)]
    feed_id: FeedId,
    /// Optional provider account id for a singular lookup.
    #[arg(long)]
    provider: Option<String>,
}

#[derive(Args, Debug)]
pub struct DisputesQuery {
    /// Optional feed identifier filter.
    #[arg(long)]
    feed_id: Option<FeedId>,
}

#[derive(Args, Debug)]
pub struct DisputeQuery {
    /// Dispute identifier.
    #[arg(long)]
    dispute_id: u64,
}

#[derive(Args, Debug)]
pub struct ChangeQuery {
    /// Change id hash.
    #[arg(long)]
    change_id: Hash,
}

#[derive(Args, Debug)]
pub struct TwitterBindingsQuery {
    /// Norito JSON file containing `UniversalAccountId`.
    #[arg(long, value_name = "PATH")]
    uaid_json: PathBuf,
}

#[derive(Args, Debug)]
pub struct DefiAttestationQuery {
    /// DeFi oracle domain (`1=perps_market`, `2=options_series`,
    /// `3=options_shout`, `4=cover_policy`).
    #[arg(long)]
    domain: u32,
    /// Domain subject id (`market_id`, `series_id`, `position_id`, or `policy_id`).
    #[arg(long = "subject-id")]
    subject_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum DisputeOutcomeArg {
    Upheld,
    Reduced,
    Frivolous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ChangeClassArg {
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ChangeStageArg {
    Intake,
    RulesCommittee,
    CopReview,
    TechnicalAudit,
    PolicyJury,
    Enactment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum EvidenceKind {
    Observation,
    Report,
    Response,
    Dispute,
    Telemetry,
}

impl EvidenceKind {
    fn as_label(self) -> &'static str {
        match self {
            Self::Observation => "observation",
            Self::Report => "report",
            Self::Response => "response",
            Self::Dispute => "dispute",
            Self::Telemetry => "telemetry",
        }
    }
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct EvidenceEntry {
    pub kind: String,
    pub hash: Hash,
    pub sources: Vec<String>,
    pub bundled_path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct FeedEventDigest {
    pub feed_id: FeedId,
    pub feed_config_version: FeedConfigVersion,
    pub slot: FeedSlot,
    pub outcome: FeedEventOutcome,
    pub evidence_hashes: Vec<Hash>,
    pub missing_evidence_hashes: Vec<Hash>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct BundleSummary {
    #[cfg_attr(feature = "json", norito(default))]
    pub generated_at_unix: u64,
    pub artifact_root: String,
    pub feed_events: Vec<FeedEventDigest>,
    pub evidence: Vec<EvidenceEntry>,
    pub coverage: BundleCoverage,
}

#[derive(Debug, Clone, JsonSerialize)]
struct BundleOutput {
    manifest_path: String,
    summary: BundleSummary,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct BundleCoverage {
    pub total_feed_events: usize,
    pub events_with_evidence: usize,
    pub evidence_entries: usize,
    pub evidence_by_kind: BTreeMap<String, usize>,
    pub missing_hashes_total: usize,
    pub missing_hashes_by_feed: BTreeMap<String, usize>,
}

/// Garbage-collection summary for evidence bundles.
#[derive(Debug, Default, JsonSerialize)]
pub struct GcReport {
    pub generated_at_unix: u64,
    pub retention_days: u64,
    pub dispute_retention_days: u64,
    pub dry_run: bool,
    pub retained_bundles: usize,
    pub removed_bundles: Vec<PrunedBundle>,
    pub pruned_files: Vec<PrunedFile>,
    pub skipped_bundles: Vec<SkippedBundle>,
    pub bytes_freed: u64,
}

#[derive(Debug, JsonSerialize)]
struct GcOutput {
    report_path: String,
    report: GcReport,
}

#[derive(Debug, JsonSerialize)]
pub struct PrunedBundle {
    pub path: String,
    pub reason: String,
    pub bytes_freed: u64,
}

#[derive(Debug, JsonSerialize)]
pub struct PrunedFile {
    pub path: String,
    pub bundle: String,
    pub bytes_freed: u64,
}

#[derive(Debug, JsonSerialize)]
pub struct SkippedBundle {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct RejectionCatalog {
    pub version: u8,
    pub observation_errors: Vec<CatalogEntry>,
    pub aggregation_errors: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct CatalogEntry {
    pub code: String,
    pub meaning: String,
}

impl CatalogEntry {
    fn new(code: impl Into<String>, meaning: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            meaning: meaning.into(),
        }
    }
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Tx(cmd) => cmd.run(context),
            Command::Query(cmd) => cmd.run(context),
            Command::Bundle(cmd) => cmd.run(context),
            Command::Catalog(cmd) => cmd.run(context),
            Command::Gc(cmd) => cmd.run(context),
        }
    }
}

impl From<DisputeOutcomeArg> for OracleDisputeOutcome {
    fn from(value: DisputeOutcomeArg) -> Self {
        match value {
            DisputeOutcomeArg::Upheld => Self::Upheld,
            DisputeOutcomeArg::Reduced => Self::Reduced,
            DisputeOutcomeArg::Frivolous => Self::Frivolous,
        }
    }
}

impl From<ChangeClassArg> for OracleChangeClass {
    fn from(value: ChangeClassArg) -> Self {
        match value {
            ChangeClassArg::Low => Self::Low,
            ChangeClassArg::Medium => Self::Medium,
            ChangeClassArg::High => Self::High,
        }
    }
}

impl From<ChangeStageArg> for OracleChangeStage {
    fn from(value: ChangeStageArg) -> Self {
        match value {
            ChangeStageArg::Intake => Self::Intake,
            ChangeStageArg::RulesCommittee => Self::RulesCommittee,
            ChangeStageArg::CopReview => Self::CopReview,
            ChangeStageArg::TechnicalAudit => Self::TechnicalAudit,
            ChangeStageArg::PolicyJury => Self::PolicyJury,
            ChangeStageArg::Enactment => Self::Enactment,
        }
    }
}

impl Run for TxCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let instruction = match self {
            Self::Register(args) => {
                let feed: FeedConfig = load_json_file(&args.feed_json)?;
                InstructionBox::from(oracle_isi::RegisterOracleFeed { feed })
            }
            Self::Submit(args) => {
                let observation: Observation = load_json_file(&args.observation_json)?;
                InstructionBox::from(oracle_isi::SubmitOracleObservation { observation })
            }
            Self::Aggregate(args) => InstructionBox::from(oracle_isi::AggregateOracleFeed {
                feed_id: args.feed_id,
                slot: args.slot,
                request_hash: args.request_hash,
                evidence_hashes: args.evidence_hashes,
            }),
            Self::OpenDispute(args) => {
                let target = crate::resolve_account_id(context, &args.target)?;
                InstructionBox::from(oracle_isi::OpenOracleDispute {
                    feed_id: args.feed_id,
                    slot: args.slot,
                    request_hash: args.request_hash,
                    target,
                    bond: args.bond,
                    evidence_hashes: args.evidence_hashes,
                    reason: args.reason,
                })
            }
            Self::ResolveDispute(args) => InstructionBox::from(oracle_isi::ResolveOracleDispute {
                dispute_id: OracleDisputeId(args.dispute_id),
                outcome: args.outcome.into(),
                notes: args.notes,
            }),
            Self::ProposeChange(args) => {
                let feed: FeedConfig = load_json_file(&args.feed_json)?;
                InstructionBox::from(oracle_isi::ProposeOracleChange {
                    change_id: OracleChangeId(args.change_id),
                    feed,
                    class: args.class.into(),
                    payload_hash: args.payload_hash,
                    evidence_hashes: args.evidence_hashes,
                })
            }
            Self::VoteChangeStage(args) => {
                InstructionBox::from(oracle_isi::VoteOracleChangeStage {
                    change_id: OracleChangeId(args.change_id),
                    stage: args.stage.into(),
                    approve: args.approve,
                    evidence_hashes: args.evidence_hashes,
                })
            }
            Self::RollbackChange(args) => InstructionBox::from(oracle_isi::RollbackOracleChange {
                change_id: OracleChangeId(args.change_id),
                stage: args.stage.map(Into::into),
                reason: args.reason,
            }),
            Self::AttestDefi(args) => {
                let attestation: DefiOracleAttestation =
                    load_json_file(&args.attestation_json)?;
                InstructionBox::from(oracle_isi::SubmitDefiOracleAttestation { attestation })
            }
            Self::RecordTwitterBinding(args) => {
                let attestation: TwitterBindingAttestation =
                    load_json_file(&args.attestation_json)?;
                InstructionBox::from(oracle_isi::RecordTwitterBinding {
                    attestation,
                    feed_id: args.feed_id,
                })
            }
            Self::RevokeTwitterBinding(args) => {
                let binding_hash: KeyedHash = load_json_file(&args.binding_hash_json)?;
                InstructionBox::from(oracle_isi::RevokeTwitterBinding {
                    binding_hash,
                    reason: args.reason,
                })
            }
        };

        context.finish(vec![instruction])
    }
}

impl Run for QueryCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        match self {
            Self::Feeds => {
                let feeds = client.query(oracle_query::FindOracleFeeds).execute_all()?;
                context.print_data(&feeds)
            }
            Self::Feed(args) => {
                let feed =
                    client.query_single(oracle_query::FindOracleFeedById::new(args.feed_id))?;
                context.print_data(&feed)
            }
            Self::History(args) => {
                let history = client
                    .query(oracle_query::FindOracleHistoryByFeedId::new(args.feed_id))
                    .execute_all()?;
                context.print_data(&history)
            }
            Self::ProviderStats(args) => {
                if let Some(provider) = args.provider {
                    let provider = crate::resolve_account_id(context, &provider)?;
                    let stats =
                        client.query_single(oracle_query::FindOracleProviderStatsByKey::new(
                            OracleProviderKey::new(args.feed_id, provider),
                        ))?;
                    context.print_data(&stats)
                } else {
                    let stats = client
                        .query(oracle_query::FindOracleProviderStatsByFeedId::new(
                            args.feed_id,
                        ))
                        .execute_all()?;
                    context.print_data(&stats)
                }
            }
            Self::Disputes(args) => {
                if let Some(feed_id) = args.feed_id {
                    let disputes = client
                        .query(oracle_query::FindOracleDisputesByFeedId::new(feed_id))
                        .execute_all()?;
                    context.print_data(&disputes)
                } else {
                    let disputes = client
                        .query(oracle_query::FindOracleDisputes)
                        .execute_all()?;
                    context.print_data(&disputes)
                }
            }
            Self::Dispute(args) => {
                let dispute = client.query_single(oracle_query::FindOracleDisputeById::new(
                    OracleDisputeId(args.dispute_id),
                ))?;
                context.print_data(&dispute)
            }
            Self::Changes => {
                let changes = client
                    .query(oracle_query::FindOracleChanges)
                    .execute_all()?;
                context.print_data(&changes)
            }
            Self::Change(args) => {
                let change = client.query_single(oracle_query::FindOracleChangeById::new(
                    OracleChangeId(args.change_id),
                ))?;
                context.print_data(&change)
            }
            Self::TwitterBindings(args) => {
                let uaid: UniversalAccountId = load_json_file(&args.uaid_json)?;
                let bindings = client
                    .query(oracle_query::FindTwitterBindingsByUaid::new(uaid))
                    .execute_all()?;
                context.print_data(&bindings)
            }
            Self::DefiAttestation(args) => {
                let attestation =
                    client.query_single(oracle_query::FindLatestDefiOracleAttestation::new(
                        DefiOracleAttestationKey::new(args.domain, args.subject_id),
                    ))?;
                context.print_data(&attestation)
            }
        }
    }
}

impl Run for Bundle {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bundle_root = normalize_dir(&self.output)?;
        let summary = build_bundle(&self, &bundle_root)?;
        let manifest_path = bundle_root.join("bundle.json");
        let rendered = json::to_json_pretty(&summary)
            .map_err(|err| eyre!("failed to render bundle manifest: {err}"))?;
        fs::create_dir_all(&bundle_root)
            .wrap_err_with(|| format!("failed to create {}", bundle_root.display()))?;
        fs::write(&manifest_path, rendered)
            .wrap_err_with(|| format!("failed to write {}", manifest_path.display()))?;

        let text = format!(
            "soracles bundle wrote manifest={} (artifacts={} entries, feed_events={}, missing_hashes={})",
            manifest_path.display(),
            summary.evidence.len(),
            summary.feed_events.len(),
            summary.coverage.missing_hashes_total
        );
        let output = BundleOutput {
            manifest_path: manifest_path.display().to_string(),
            summary,
        };
        print_with_optional_text(context, Some(text), &output)
    }
}

impl Run for Catalog {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let catalog = build_rejection_catalog();
        match context.output_format() {
            crate::CliOutputFormat::Json => context.print_data(&catalog),
            crate::CliOutputFormat::Text => match self.format {
                CatalogFormat::Json => context.print_data(&catalog),
                CatalogFormat::Markdown => context.println(render_catalog_markdown(&catalog)),
            },
        }
    }
}

impl Run for GcArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let now = SystemTime::now();
        let report_path = self
            .report
            .clone()
            .unwrap_or_else(|| self.root.join("gc_report.json"));
        let report = garbage_collect(
            &self.root,
            self.retention_days,
            self.dispute_retention_days,
            self.prune_unreferenced,
            self.dry_run,
            now,
        )?;
        let rendered = json::to_json_pretty(&report)
            .map_err(|err| eyre!("failed to render GC report: {err}"))?;
        if let Some(parent) = report_path.parent() {
            fs::create_dir_all(parent)
                .wrap_err_with(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&report_path, rendered)
            .wrap_err_with(|| format!("failed to write {}", report_path.display()))?;

        let verb = if self.dry_run { "scanned" } else { "pruned" };
        let text = format!(
            "soracles evidence-gc {verb}: removed_bundles={} pruned_files={} bytes_freed={} retained={} report={}",
            report.removed_bundles.len(),
            report.pruned_files.len(),
            report.bytes_freed,
            report.retained_bundles,
            report_path.display()
        );
        let output = GcOutput {
            report_path: report_path.display().to_string(),
            report,
        };
        print_with_optional_text(context, Some(text), &output)
    }
}

fn build_bundle(options: &Bundle, bundle_root: &Path) -> Result<BundleSummary> {
    let artifacts_root = bundle_root.join("artifacts");
    fs::create_dir_all(&artifacts_root).wrap_err_with(|| {
        format!(
            "failed to create artifact directory {}",
            artifacts_root.display()
        )
    })?;

    let feed_events = load_feed_events(&options.events)?;
    let mut evidence = BTreeMap::<Hash, EvidenceEntry>::new();

    if let Some(path) = &options.observations {
        ingest_path(
            path,
            EvidenceKind::Observation,
            &artifacts_root,
            &mut evidence,
        )?;
    }
    if let Some(path) = &options.reports {
        ingest_path(path, EvidenceKind::Report, &artifacts_root, &mut evidence)?;
    }
    if let Some(path) = &options.responses {
        ingest_path(path, EvidenceKind::Response, &artifacts_root, &mut evidence)?;
    }
    if let Some(path) = &options.disputes {
        ingest_path(path, EvidenceKind::Dispute, &artifacts_root, &mut evidence)?;
    }
    if let Some(path) = &options.telemetry {
        ingest_path(
            path,
            EvidenceKind::Telemetry,
            &artifacts_root,
            &mut evidence,
        )?;
    }

    for entry in evidence.values_mut() {
        entry.sources.sort();
        entry.sources.dedup();
    }

    let feed_events = feed_events
        .into_iter()
        .map(|record| digest_event(record, &evidence))
        .collect::<Vec<_>>();
    let coverage = compute_coverage(&feed_events, &evidence);
    let evidence_entries: Vec<EvidenceEntry> = evidence.into_values().collect();

    write_artifact_index(&artifacts_root, &evidence_entries)?;

    let summary = BundleSummary {
        generated_at_unix: now_unix(SystemTime::now())?,
        artifact_root: "artifacts".to_string(),
        feed_events,
        evidence: evidence_entries,
        coverage,
    };

    Ok(summary)
}

fn compute_coverage(
    feed_events: &[FeedEventDigest],
    evidence: &BTreeMap<Hash, EvidenceEntry>,
) -> BundleCoverage {
    let mut evidence_by_kind = BTreeMap::new();
    for entry in evidence.values() {
        *evidence_by_kind.entry(entry.kind.clone()).or_insert(0) += 1usize;
    }

    let mut events_with_evidence = 0usize;
    let mut missing_hashes_total = 0usize;
    let mut missing_hashes_by_feed = BTreeMap::new();
    for event in feed_events {
        if !event.evidence_hashes.is_empty() {
            events_with_evidence += 1;
        }
        let missing = event.missing_evidence_hashes.len();
        if missing > 0 {
            missing_hashes_total += missing;
            let key = event.feed_id.to_string();
            *missing_hashes_by_feed.entry(key).or_insert(0) += missing;
        }
    }

    BundleCoverage {
        total_feed_events: feed_events.len(),
        events_with_evidence,
        evidence_entries: evidence.len(),
        evidence_by_kind,
        missing_hashes_total,
        missing_hashes_by_feed,
    }
}

fn write_artifact_index(root: &Path, evidence: &[EvidenceEntry]) -> Result<()> {
    let index_path = root.join("index.json");
    let rendered = json::to_json_pretty(&evidence.to_vec())
        .map_err(|err| eyre!("failed to render artifact index: {err}"))?;
    fs::write(&index_path, rendered)
        .wrap_err_with(|| format!("failed to write {}", index_path.display()))?;
    Ok(())
}

fn build_rejection_catalog() -> RejectionCatalog {
    let observation_errors = vec![
        CatalogEntry::new(
            "ResourceUnavailable",
            "Upstream resource unavailable or transiently failing.",
        ),
        CatalogEntry::new(
            "AuthFailed",
            "Connector authentication or credentials failed.",
        ),
        CatalogEntry::new("Timeout", "Connector timed out before returning data."),
        CatalogEntry::new(
            "Missing",
            "Connector returned no payload or could not parse upstream data.",
        ),
        CatalogEntry::new(
            "Other(<u16>)",
            "Connector-specific error code recorded in `ObservationErrorCode::Other`.",
        ),
    ];

    let aggregation_errors = OracleRejectionCode::all()
        .iter()
        .map(|code| CatalogEntry::new(code.as_code(), code.description()))
        .collect();

    RejectionCatalog {
        version: 1,
        observation_errors,
        aggregation_errors,
    }
}

fn render_catalog_markdown(catalog: &RejectionCatalog) -> String {
    fn render_section(title: &str, entries: &[CatalogEntry], out: &mut String) {
        out.push_str("### ");
        out.push_str(title);
        out.push_str("\n\n| code | meaning |\n| --- | --- |\n");
        for entry in entries {
            out.push_str("| ");
            out.push_str(&entry.code);
            out.push_str(" | ");
            out.push_str(&entry.meaning);
            out.push_str(" |\n");
        }
        out.push('\n');
    }

    let mut rendered = String::new();
    let _ = writeln!(&mut rendered, "Catalog version {}", catalog.version);
    rendered.push('\n');
    render_section(
        "Observation errors",
        &catalog.observation_errors,
        &mut rendered,
    );
    render_section(
        "Aggregation errors",
        &catalog.aggregation_errors,
        &mut rendered,
    );
    rendered
}

fn digest_event(
    record: FeedEventRecord,
    evidence: &BTreeMap<Hash, EvidenceEntry>,
) -> FeedEventDigest {
    let missing = record
        .evidence_hashes
        .iter()
        .filter(|hash| !evidence.contains_key(hash))
        .copied()
        .collect::<Vec<_>>();

    FeedEventDigest {
        feed_id: record.event.feed_id,
        feed_config_version: record.event.feed_config_version,
        slot: record.event.slot,
        outcome: record.event.outcome,
        evidence_hashes: record.evidence_hashes,
        missing_evidence_hashes: missing,
    }
}

fn load_feed_events(path: &Path) -> Result<Vec<FeedEventRecord>> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read feed events from {}", path.display()))?;
    if let Ok(list) = json::from_slice::<Vec<FeedEventRecord>>(&bytes) {
        Ok(list)
    } else {
        let value: Value =
            json::from_slice(&bytes).wrap_err("failed to parse feed event JSON payload")?;
        if let Value::Array(values) = value {
            let mut events = Vec::new();
            for entry in values {
                let record = json::from_value::<FeedEventRecord>(entry)
                    .map_err(|err| eyre!("failed to parse feed event from array entry: {err}"))?;
                events.push(record);
            }
            Ok(events)
        } else {
            let record: FeedEventRecord =
                json::from_value(value).wrap_err("failed to parse feed event record")?;
            Ok(vec![record])
        }
    }
}

fn load_json_file<T>(path: &Path) -> Result<T>
where
    T: JsonDeserializeOwned,
{
    let bytes = fs::read(path).wrap_err_with(|| format!("failed to read {}", path.display()))?;
    json::from_slice(&bytes).wrap_err_with(|| format!("failed to parse {}", path.display()))
}

fn ingest_path(
    path: &Path,
    kind: EvidenceKind,
    artifact_root: &Path,
    evidence: &mut BTreeMap<Hash, EvidenceEntry>,
) -> Result<()> {
    let metadata = path
        .metadata()
        .wrap_err_with(|| format!("failed to stat {}", path.display()))?;
    if metadata.is_dir() {
        for entry in
            fs::read_dir(path).wrap_err_with(|| format!("failed to read {}", path.display()))?
        {
            let entry = entry?;
            ingest_path(&entry.path(), kind, artifact_root, evidence)?;
        }
        return Ok(());
    }

    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read evidence file {}", path.display()))?;
    let hash = Hash::new(&bytes);
    let bundled_name = bundled_name(&hash, path);
    let bundled_path = artifact_root.join(&bundled_name);

    if !bundled_path.exists() {
        fs::write(&bundled_path, &bytes).wrap_err_with(|| {
            format!(
                "failed to copy evidence {} -> {}",
                path.display(),
                bundled_path.display()
            )
        })?;
    }

    let source = path.display().to_string();
    let entry = evidence.entry(hash).or_insert_with(|| EvidenceEntry {
        kind: kind.as_label().to_string(),
        hash,
        sources: Vec::new(),
        bundled_path: format!("artifacts/{bundled_name}"),
        size_bytes: bytes.len() as u64,
    });

    if !entry.sources.contains(&source) {
        entry.sources.push(source);
    }

    Ok(())
}

fn bundled_name(hash: &Hash, source: &Path) -> String {
    let digest = hex::encode_upper(hash.as_ref());
    match source.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{digest}.{ext}"),
        _ => digest,
    }
}

fn normalize_dir(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .wrap_err("failed to get current directory")?
            .join(path)
    };
    Ok(absolute)
}

struct GcContext {
    retention: Option<Duration>,
    dispute_retention: Option<Duration>,
    retention_days: u64,
    dispute_retention_days: u64,
    prune_unreferenced: bool,
    dry_run: bool,
    now: SystemTime,
}

fn garbage_collect(
    root: &Path,
    retention_days: u64,
    dispute_retention_days: u64,
    prune_unreferenced: bool,
    dry_run: bool,
    now: SystemTime,
) -> Result<GcReport> {
    let mut report = GcReport {
        generated_at_unix: now_unix(now)?,
        retention_days,
        dispute_retention_days,
        dry_run,
        ..GcReport::default()
    };

    if !root.exists() {
        return Ok(report);
    }

    let ctx = GcContext {
        retention: retention_window(retention_days),
        dispute_retention: retention_window(dispute_retention_days),
        retention_days,
        dispute_retention_days,
        prune_unreferenced,
        dry_run,
        now,
    };

    for entry in
        fs::read_dir(root).wrap_err_with(|| format!("failed to read {}", root.display()))?
    {
        let entry = entry?;
        let bundle_root = entry.path();
        if !bundle_root.is_dir() {
            continue;
        }
        process_bundle_root(&bundle_root, &ctx, &mut report)?;
    }

    Ok(report)
}

fn process_bundle_root(bundle_root: &Path, ctx: &GcContext, report: &mut GcReport) -> Result<()> {
    let manifest_path = bundle_root.join("bundle.json");
    if !manifest_path.exists() {
        return Ok(());
    }

    let manifest_meta = match manifest_path.metadata() {
        Ok(meta) => meta,
        Err(err) => {
            report.skipped_bundles.push(SkippedBundle {
                path: bundle_root.display().to_string(),
                reason: format!("failed to stat bundle.json: {err}"),
            });
            return Ok(());
        }
    };

    let manifest_bytes = fs::read(&manifest_path)
        .wrap_err_with(|| format!("failed to read {}", manifest_path.display()))?;
    let summary: BundleSummary = match json::from_slice(&manifest_bytes) {
        Ok(summary) => summary,
        Err(err) => {
            report.skipped_bundles.push(SkippedBundle {
                path: bundle_root.display().to_string(),
                reason: format!("failed to parse manifest: {err}"),
            });
            return Ok(());
        }
    };

    let age = compute_bundle_age(&summary, &manifest_meta, ctx.now);
    let has_dispute = summary
        .evidence
        .iter()
        .any(|entry| entry.kind == EvidenceKind::Dispute.as_label());
    let window = if has_dispute {
        ctx.dispute_retention.as_ref()
    } else {
        ctx.retention.as_ref()
    };

    let expired = match (window, age) {
        (None, _) => true,
        (Some(limit), Some(duration)) => duration >= *limit,
        (Some(_), None) => false,
    };

    let bundle_label = bundle_root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let artifacts_root = bundle_root.join(&summary.artifact_root);

    if expired {
        let reason_days = if has_dispute {
            ctx.dispute_retention_days
        } else {
            ctx.retention_days
        };
        let bytes_freed = dir_size(bundle_root)?;
        report.bytes_freed = report.bytes_freed.saturating_add(bytes_freed);
        report.removed_bundles.push(PrunedBundle {
            path: bundle_root.display().to_string(),
            reason: format!("older than {reason_days} days"),
            bytes_freed,
        });
        if !ctx.dry_run {
            fs::remove_dir_all(bundle_root).wrap_err_with(|| {
                format!(
                    "failed to remove expired bundle directory {}",
                    bundle_root.display()
                )
            })?;
        }
        return Ok(());
    }

    report.retained_bundles = report.retained_bundles.saturating_add(1);

    if ctx.prune_unreferenced && artifacts_root.exists() {
        let referenced = referenced_artifacts(bundle_root, &summary);
        prune_unreferenced_files(
            &artifacts_root,
            &referenced,
            &bundle_label,
            ctx.dry_run,
            report,
        )?;
    }

    Ok(())
}

fn compute_bundle_age(
    summary: &BundleSummary,
    manifest_meta: &fs::Metadata,
    now: SystemTime,
) -> Option<Duration> {
    let generated = if summary.generated_at_unix > 0 {
        UNIX_EPOCH.checked_add(Duration::from_secs(summary.generated_at_unix))?
    } else {
        manifest_meta.modified().ok()?
    };
    now.duration_since(generated).ok()
}

fn retention_window(days: u64) -> Option<Duration> {
    if days == 0 {
        None
    } else {
        Some(Duration::from_secs(days.saturating_mul(86_400)))
    }
}

fn referenced_artifacts(bundle_root: &Path, summary: &BundleSummary) -> HashSet<PathBuf> {
    summary
        .evidence
        .iter()
        .map(|entry| bundle_root.join(&entry.bundled_path))
        .collect()
}

fn prune_unreferenced_files(
    artifact_root: &Path,
    referenced: &HashSet<PathBuf>,
    bundle_label: &str,
    dry_run: bool,
    report: &mut GcReport,
) -> Result<()> {
    let mut stack = vec![artifact_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in
            fs::read_dir(&dir).wrap_err_with(|| format!("failed to read {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !path.is_file() || referenced.contains(&path) {
                continue;
            }

            let bytes_freed = path.metadata().map(|m| m.len()).unwrap_or(0);
            report.pruned_files.push(PrunedFile {
                path: path.display().to_string(),
                bundle: bundle_label.to_string(),
                bytes_freed,
            });
            report.bytes_freed = report.bytes_freed.saturating_add(bytes_freed);
            if !dry_run {
                fs::remove_file(&path).wrap_err_with(|| {
                    format!("failed to remove unreferenced artifact {}", path.display())
                })?;
            }
        }
    }

    Ok(())
}

fn dir_size(root: &Path) -> Result<u64> {
    let mut total: u64 = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in
            fs::read_dir(&dir).wrap_err_with(|| format!("failed to read {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(meta) = path.metadata() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

fn now_unix(now: SystemTime) -> Result<u64> {
    now.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|err| eyre!("system clock before unix epoch: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use iroha::data_model::account::AccountId;
    use iroha::data_model::oracle::{
        FeedEvent, FeedSuccess, ObservationBody, ObservationValue, ReportEntry,
        TwitterBindingStatus,
    };
    use iroha_crypto::HashOf;
    use iroha_i18n::{Bundle as I18nBundle, Language, Localizer};
    use std::fmt::Display;
    use tempfile::TempDir;

    struct TestContext {
        output_format: crate::CliOutputFormat,
        printed: Vec<String>,
        config: iroha::config::Config,
        i18n: Localizer,
        output_instructions: bool,
    }

    impl TestContext {
        fn new(output_format: crate::CliOutputFormat) -> Self {
            Self {
                output_format,
                printed: Vec::new(),
                config: crate::fallback_config(),
                i18n: Localizer::new(I18nBundle::Cli, Language::English),
                output_instructions: false,
            }
        }

        fn with_output_instructions(mut self) -> Self {
            self.output_instructions = true;
            self
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &iroha::config::Config {
            &self.config
        }

        fn transaction_metadata(&self) -> Option<&iroha::data_model::metadata::Metadata> {
            None
        }

        fn input_instructions(&self) -> bool {
            false
        }

        fn output_instructions(&self) -> bool {
            self.output_instructions
        }

        fn i18n(&self) -> &Localizer {
            &self.i18n
        }

        fn output_format(&self) -> crate::CliOutputFormat {
            self.output_format
        }

        fn print_data<T>(&mut self, data: &T) -> eyre::Result<()>
        where
            T: norito::json::JsonSerialize + ?Sized,
        {
            let rendered = norito::json::to_json_pretty(data)?;
            self.printed.push(rendered);
            Ok(())
        }

        fn println(&mut self, data: impl Display) -> eyre::Result<()> {
            self.printed.push(data.to_string());
            Ok(())
        }
    }

    fn test_oracle_account_id() -> AccountId {
        let signatory = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("oracle signatory");
        AccountId::new(signatory)
    }

    fn write_json_file<T>(dir: &TempDir, name: &str, value: &T) -> PathBuf
    where
        T: norito::json::JsonSerialize + ?Sized,
    {
        let path = dir.path().join(name);
        let rendered = json::to_json_pretty(value).expect("render json fixture");
        std::fs::write(&path, rendered).expect("write json fixture");
        path
    }

    fn write_invalid_json_file(dir: &TempDir, name: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, b"{not valid norito json").expect("write invalid json fixture");
        path
    }

    fn assert_failed_to_parse(error: eyre::Report) {
        let rendered = format!("{error:?}");
        assert!(
            rendered.contains("failed to parse"),
            "expected malformed JSON parse error, got: {rendered}"
        );
    }

    fn sample_twitter_attestation() -> TwitterBindingAttestation {
        TwitterBindingAttestation {
            binding_hash: KeyedHash::new("pepper-social-v1", b"test-pepper", b"twitter-user-123"),
            uaid: UniversalAccountId::from_hash(Hash::new(b"cli-uaid")),
            status: TwitterBindingStatus::Following,
            tweet_id: Some("tweet-123".to_string()),
            challenge_hash: Some(Hash::new(b"twitter-challenge")),
            expires_at_ms: 1_700_600_000_000,
            observed_at_ms: 1_700_000_000_000,
            request_hash: Hash::new(b"twitter-request"),
            slot: 42,
            feed_config_version: FeedConfigVersion(1),
        }
    }

    #[derive(Parser, Debug)]
    struct SoraclesHarness {
        #[command(subcommand)]
        command: Command,
    }

    #[test]
    fn parses_tx_and_query_subcommands() {
        let request_hash = Hash::new(b"cli-request").to_string();
        let parsed = SoraclesHarness::try_parse_from([
            "soracles",
            "tx",
            "aggregate",
            "--feed-id",
            "xor_usd",
            "--slot",
            "42",
            "--request-hash",
            request_hash.as_str(),
        ])
        .expect("parse aggregate tx");
        match parsed.command {
            Command::Tx(TxCommand::Aggregate(args)) => {
                assert_eq!(args.slot, 42);
                assert_eq!(args.feed_id.as_str(), "xor_usd");
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let provider = test_oracle_account_id().to_string();
        let parsed = SoraclesHarness::try_parse_from([
            "soracles",
            "query",
            "provider-stats",
            "--feed-id",
            "xor_usd",
            "--provider",
            provider.as_str(),
        ])
        .expect("parse provider stats query");
        match parsed.command {
            Command::Query(QueryCommand::ProviderStats(args)) => {
                assert_eq!(args.feed_id.as_str(), "xor_usd");
                assert_eq!(args.provider.as_deref(), Some(provider.as_str()));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_all_query_subcommands_without_live_node() {
        let provider = test_oracle_account_id().to_string();
        let change_id = Hash::new(b"query-change").to_string();
        let uaid_path = "/tmp/uaid.json";

        let parse_query = |argv: Vec<&str>| {
            let parsed = SoraclesHarness::try_parse_from(argv).expect("parse query command");
            match parsed.command {
                Command::Query(cmd) => cmd,
                other => panic!("unexpected top-level command: {other:?}"),
            }
        };

        assert!(matches!(
            parse_query(vec!["soracles", "query", "feeds"]),
            QueryCommand::Feeds
        ));
        match parse_query(vec!["soracles", "query", "feed", "--feed-id", "xor_usd"]) {
            QueryCommand::Feed(args) => assert_eq!(args.feed_id.as_str(), "xor_usd"),
            other => panic!("unexpected command: {other:?}"),
        }
        match parse_query(vec!["soracles", "query", "history", "--feed-id", "xor_usd"]) {
            QueryCommand::History(args) => assert_eq!(args.feed_id.as_str(), "xor_usd"),
            other => panic!("unexpected command: {other:?}"),
        }
        match parse_query(vec![
            "soracles",
            "query",
            "provider-stats",
            "--feed-id",
            "xor_usd",
        ]) {
            QueryCommand::ProviderStats(args) => {
                assert_eq!(args.feed_id.as_str(), "xor_usd");
                assert!(args.provider.is_none());
            }
            other => panic!("unexpected command: {other:?}"),
        }
        match parse_query(vec![
            "soracles",
            "query",
            "provider-stats",
            "--feed-id",
            "xor_usd",
            "--provider",
            provider.as_str(),
        ]) {
            QueryCommand::ProviderStats(args) => {
                assert_eq!(args.feed_id.as_str(), "xor_usd");
                assert_eq!(args.provider.as_deref(), Some(provider.as_str()));
            }
            other => panic!("unexpected command: {other:?}"),
        }
        match parse_query(vec!["soracles", "query", "disputes"]) {
            QueryCommand::Disputes(args) => assert!(args.feed_id.is_none()),
            other => panic!("unexpected command: {other:?}"),
        }
        match parse_query(vec![
            "soracles",
            "query",
            "disputes",
            "--feed-id",
            "xor_usd",
        ]) {
            QueryCommand::Disputes(args) => {
                assert_eq!(args.feed_id.expect("feed filter").as_str(), "xor_usd");
            }
            other => panic!("unexpected command: {other:?}"),
        }
        match parse_query(vec!["soracles", "query", "dispute", "--dispute-id", "7"]) {
            QueryCommand::Dispute(args) => assert_eq!(args.dispute_id, 7),
            other => panic!("unexpected command: {other:?}"),
        }
        assert!(matches!(
            parse_query(vec!["soracles", "query", "changes"]),
            QueryCommand::Changes
        ));
        match parse_query(vec![
            "soracles",
            "query",
            "change",
            "--change-id",
            change_id.as_str(),
        ]) {
            QueryCommand::Change(args) => assert_eq!(args.change_id.to_string(), change_id),
            other => panic!("unexpected command: {other:?}"),
        }
        match parse_query(vec![
            "soracles",
            "query",
            "twitter-bindings",
            "--uaid-json",
            uaid_path,
        ]) {
            QueryCommand::TwitterBindings(args) => {
                assert_eq!(args.uaid_json, PathBuf::from(uaid_path));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn tx_aggregate_generates_output_instruction_without_live_node() {
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json).with_output_instructions();
        TxCommand::Aggregate(AggregateTx {
            feed_id: "xor_usd".parse().expect("feed id"),
            slot: 42,
            request_hash: Hash::new(b"cli-output-request"),
            evidence_hashes: vec![Hash::new(b"cli-evidence")],
        })
        .run(&mut ctx)
        .expect("generate aggregate instruction");
    }

    #[test]
    fn tx_commands_generate_output_instructions_without_live_node() {
        let tmp = TempDir::new().expect("tmpdir");
        let kit = iroha::data_model::oracle::kits::price_xor_usd();
        let feed_path = write_json_file(&tmp, "feed.json", &kit.feed_config);
        let observation_path = write_json_file(&tmp, "observation.json", &kit.observations[0]);
        let attestation = sample_twitter_attestation();
        let attestation_path = write_json_file(&tmp, "attestation.json", &attestation);
        let binding_hash_path =
            write_json_file(&tmp, "binding_hash.json", &attestation.binding_hash);
        let feed_id: FeedId = "xor_usd".parse().expect("feed id");
        let target = test_oracle_account_id().to_string();
        let change_id = Hash::new(b"cli-change");

        let commands = vec![
            TxCommand::Register(RegisterTx {
                feed_json: feed_path.clone(),
            }),
            TxCommand::Submit(SubmitTx {
                observation_json: observation_path,
            }),
            TxCommand::Aggregate(AggregateTx {
                feed_id: feed_id.clone(),
                slot: 42,
                request_hash: Hash::new(b"aggregate-request"),
                evidence_hashes: vec![Hash::new(b"aggregate-evidence")],
            }),
            TxCommand::OpenDispute(OpenDisputeTx {
                feed_id: feed_id.clone(),
                slot: 42,
                request_hash: Hash::new(b"dispute-request"),
                target,
                bond: Some(Numeric::from(1_u32)),
                evidence_hashes: vec![Hash::new(b"dispute-evidence")],
                reason: "bad observation".to_string(),
            }),
            TxCommand::ResolveDispute(ResolveDisputeTx {
                dispute_id: 7,
                outcome: DisputeOutcomeArg::Upheld,
                notes: "resolved".to_string(),
            }),
            TxCommand::ProposeChange(ProposeChangeTx {
                change_id,
                feed_json: feed_path,
                class: ChangeClassArg::Low,
                payload_hash: Hash::new(b"change-payload"),
                evidence_hashes: vec![Hash::new(b"change-evidence")],
            }),
            TxCommand::VoteChangeStage(VoteChangeStageTx {
                change_id,
                stage: ChangeStageArg::Intake,
                approve: true,
                evidence_hashes: vec![Hash::new(b"vote-evidence")],
            }),
            TxCommand::RollbackChange(RollbackChangeTx {
                change_id,
                stage: Some(ChangeStageArg::Intake),
                reason: "rollback".to_string(),
            }),
            TxCommand::RecordTwitterBinding(RecordTwitterBindingTx {
                attestation_json: attestation_path,
                feed_id,
            }),
            TxCommand::RevokeTwitterBinding(RevokeTwitterBindingTx {
                binding_hash_json: binding_hash_path,
                reason: "stale".to_string(),
            }),
        ];

        for command in commands {
            let mut ctx = TestContext::new(crate::CliOutputFormat::Json).with_output_instructions();
            command
                .run(&mut ctx)
                .expect("generate output instruction without live node");
        }
    }

    #[test]
    fn tx_file_backed_commands_reject_malformed_json_without_live_node() {
        let tmp = TempDir::new().expect("tmpdir");
        let invalid_path = write_invalid_json_file(&tmp, "invalid.json");
        let feed_id: FeedId = "xor_usd".parse().expect("feed id");
        let change_id = Hash::new(b"cli-invalid-json-change");

        let commands = vec![
            TxCommand::Register(RegisterTx {
                feed_json: invalid_path.clone(),
            }),
            TxCommand::Submit(SubmitTx {
                observation_json: invalid_path.clone(),
            }),
            TxCommand::ProposeChange(ProposeChangeTx {
                change_id,
                feed_json: invalid_path.clone(),
                class: ChangeClassArg::Low,
                payload_hash: Hash::new(b"invalid-json-payload"),
                evidence_hashes: Vec::new(),
            }),
            TxCommand::RecordTwitterBinding(RecordTwitterBindingTx {
                attestation_json: invalid_path.clone(),
                feed_id,
            }),
            TxCommand::RevokeTwitterBinding(RevokeTwitterBindingTx {
                binding_hash_json: invalid_path.clone(),
                reason: "malformed".to_string(),
            }),
        ];

        for command in commands {
            let mut ctx = TestContext::new(crate::CliOutputFormat::Json).with_output_instructions();
            let err = command
                .run(&mut ctx)
                .expect_err("malformed JSON must reject before building an instruction");
            assert_failed_to_parse(err);
            assert!(
                ctx.printed.is_empty(),
                "failed command should not emit output instructions"
            );
        }
    }

    #[test]
    fn tx_file_backed_commands_reject_missing_files_without_live_node() {
        let tmp = TempDir::new().expect("tmpdir");
        let missing_path = tmp.path().join("missing.json");
        let feed_id: FeedId = "xor_usd".parse().expect("feed id");
        let change_id = Hash::new(b"cli-missing-file-change");

        let commands = vec![
            TxCommand::Register(RegisterTx {
                feed_json: missing_path.clone(),
            }),
            TxCommand::Submit(SubmitTx {
                observation_json: missing_path.clone(),
            }),
            TxCommand::ProposeChange(ProposeChangeTx {
                change_id,
                feed_json: missing_path.clone(),
                class: ChangeClassArg::Low,
                payload_hash: Hash::new(b"missing-file-payload"),
                evidence_hashes: Vec::new(),
            }),
            TxCommand::RecordTwitterBinding(RecordTwitterBindingTx {
                attestation_json: missing_path.clone(),
                feed_id,
            }),
            TxCommand::RevokeTwitterBinding(RevokeTwitterBindingTx {
                binding_hash_json: missing_path,
                reason: "missing file".to_string(),
            }),
        ];

        for command in commands {
            let mut ctx = TestContext::new(crate::CliOutputFormat::Json).with_output_instructions();
            let err = command
                .run(&mut ctx)
                .expect_err("missing JSON file must reject before building an instruction");
            let rendered = format!("{err:?}");
            assert!(
                rendered.contains("failed to read"),
                "expected missing-file read error, got: {rendered}"
            );
            assert!(
                ctx.printed.is_empty(),
                "failed command should not emit output instructions"
            );
        }
    }

    #[test]
    fn query_file_backed_command_rejects_malformed_uaid_json_without_live_node() {
        let tmp = TempDir::new().expect("tmpdir");
        let invalid_path = write_invalid_json_file(&tmp, "invalid-uaid.json");
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);
        let err = QueryCommand::TwitterBindings(TwitterBindingsQuery {
            uaid_json: invalid_path,
        })
        .run(&mut ctx)
        .expect_err("malformed UAID JSON must reject before a query result is printed");
        assert_failed_to_parse(err);
        assert!(ctx.printed.is_empty());
    }

    #[test]
    fn query_file_backed_command_rejects_missing_uaid_json_without_live_node() {
        let tmp = TempDir::new().expect("tmpdir");
        let missing_path = tmp.path().join("missing-uaid.json");
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);
        let err = QueryCommand::TwitterBindings(TwitterBindingsQuery {
            uaid_json: missing_path,
        })
        .run(&mut ctx)
        .expect_err("missing UAID JSON must reject before a query result is printed");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("failed to read"),
            "expected missing-file read error, got: {rendered}"
        );
        assert!(ctx.printed.is_empty());
    }

    #[test]
    fn rejects_unknown_change_stage_and_class_values_at_parse_time() {
        let change_id = Hash::new(b"bad-stage").to_string();
        let stage_err = SoraclesHarness::try_parse_from([
            "soracles",
            "tx",
            "vote-change-stage",
            "--change-id",
            change_id.as_str(),
            "--stage",
            "root",
        ])
        .expect_err("unknown stage must fail clap parsing")
        .to_string();
        assert!(
            stage_err.contains("invalid value"),
            "unexpected stage parse error: {stage_err}"
        );

        let tmp = TempDir::new().expect("tmpdir");
        let feed_path = tmp.path().join("feed.json");
        let payload_hash = Hash::new(b"bad-class").to_string();
        let class_err = SoraclesHarness::try_parse_from([
            "soracles",
            "tx",
            "propose-change",
            "--change-id",
            change_id.as_str(),
            "--feed-json",
            feed_path.to_str().expect("utf8 path"),
            "--class",
            "catastrophic",
            "--payload-hash",
            payload_hash.as_str(),
        ])
        .expect_err("unknown change class must fail clap parsing")
        .to_string();
        assert!(
            class_err.contains("invalid value"),
            "unexpected class parse error: {class_err}"
        );
    }

    #[test]
    fn rejects_unknown_dispute_outcome_and_invalid_approve_values_at_parse_time() {
        let change_id = Hash::new(b"bad-approve").to_string();
        let outcome_err = SoraclesHarness::try_parse_from([
            "soracles",
            "tx",
            "resolve-dispute",
            "--dispute-id",
            "7",
            "--outcome",
            "void",
        ])
        .expect_err("unknown dispute outcome must fail clap parsing")
        .to_string();
        assert!(
            outcome_err.contains("invalid value"),
            "unexpected outcome parse error: {outcome_err}"
        );

        let approve_err = SoraclesHarness::try_parse_from([
            "soracles",
            "tx",
            "vote-change-stage",
            "--change-id",
            change_id.as_str(),
            "--stage",
            "intake",
            "--approve",
            "maybe",
        ])
        .expect_err("invalid approve flag must fail clap parsing")
        .to_string();
        assert!(
            approve_err.contains("invalid value") || approve_err.contains("unexpected argument"),
            "unexpected approve parse error: {approve_err}"
        );
    }

    #[test]
    fn rejects_missing_required_tx_arguments_at_parse_time() {
        let change_id = Hash::new(b"missing-required").to_string();
        let payload_hash = Hash::new(b"missing-required-payload").to_string();
        let request_hash = Hash::new(b"missing-required-request").to_string();
        let cases = vec![
            vec![
                "soracles",
                "tx",
                "aggregate",
                "--feed-id",
                "xor_usd",
                "--slot",
                "42",
            ],
            vec![
                "soracles",
                "tx",
                "open-dispute",
                "--feed-id",
                "xor_usd",
                "--slot",
                "42",
                "--request-hash",
                request_hash.as_str(),
            ],
            vec![
                "soracles",
                "tx",
                "propose-change",
                "--change-id",
                change_id.as_str(),
                "--class",
                "low",
                "--payload-hash",
                payload_hash.as_str(),
            ],
            vec![
                "soracles",
                "tx",
                "rollback-change",
                "--change-id",
                change_id.as_str(),
            ],
            vec!["soracles", "tx", "record-twitter-binding", "--feed-id", "xor_usd"],
        ];

        for argv in cases {
            let err = SoraclesHarness::try_parse_from(argv)
                .expect_err("missing required argument must fail clap parsing")
                .to_string();
            assert!(
                err.contains("required"),
                "unexpected missing-argument parse error: {err}"
            );
        }
    }

    #[test]
    fn rejects_missing_required_query_arguments_at_parse_time() {
        let cases = vec![
            vec!["soracles", "query", "feed"],
            vec!["soracles", "query", "history"],
            vec!["soracles", "query", "provider-stats"],
            vec!["soracles", "query", "dispute"],
            vec!["soracles", "query", "change"],
            vec!["soracles", "query", "twitter-bindings"],
        ];

        for argv in cases {
            let err = SoraclesHarness::try_parse_from(argv)
                .expect_err("missing required query argument must fail clap parsing")
                .to_string();
            assert!(
                err.contains("required"),
                "unexpected missing-query parse error: {err}"
            );
        }
    }

    #[test]
    fn rejects_invalid_hash_and_numeric_values_at_parse_time() {
        let valid_hash = Hash::new(b"valid-cli-hash").to_string();
        let cases = vec![
            vec![
                "soracles",
                "tx",
                "aggregate",
                "--feed-id",
                "xor_usd",
                "--slot",
                "not-a-slot",
                "--request-hash",
                valid_hash.as_str(),
            ],
            vec![
                "soracles",
                "tx",
                "aggregate",
                "--feed-id",
                "xor_usd",
                "--slot",
                "42",
                "--request-hash",
                "not-a-hash",
            ],
            vec![
                "soracles",
                "tx",
                "resolve-dispute",
                "--dispute-id",
                "not-an-id",
                "--outcome",
                "upheld",
            ],
            vec![
                "soracles",
                "query",
                "change",
                "--change-id",
                "not-a-hash",
            ],
            vec![
                "soracles",
                "query",
                "dispute",
                "--dispute-id",
                "not-an-id",
            ],
        ];

        for argv in cases {
            let err = SoraclesHarness::try_parse_from(argv)
                .expect_err("invalid scalar argument must fail clap parsing")
                .to_string();
            assert!(
                err.contains("invalid value"),
                "unexpected invalid-scalar parse error: {err}"
            );
        }
    }

    #[test]
    fn builds_bundle_and_marks_missing_hashes() {
        let tmp = TempDir::new().expect("tmpdir");
        let events_path = tmp.path().join("events.json");
        let artifacts_root = tmp.path().join("evidence");
        let missing_hash = Hash::new(b"missing");
        let observed_payload = br#"{"dummy":"payload"}"#;

        let feed_id: FeedId = "xor_usd".parse().expect("feed id");
        let observed = Hash::new(observed_payload);
        let observation_hash = HashOf::<ObservationBody>::from_untyped_unchecked(Hash::new(b"req"));

        let event = FeedEventRecord {
            event: FeedEvent {
                feed_id: feed_id.clone(),
                feed_config_version: FeedConfigVersion(1),
                slot: 42,
                request_hash: Hash::new(b"request"),
                outcome: FeedEventOutcome::Success(FeedSuccess {
                    value: ObservationValue::new(1_000, 2),
                    entries: vec![ReportEntry {
                        oracle_id: test_oracle_account_id(),
                        observation_hash,
                        value: ObservationValue::new(1_000, 2),
                        outlier: false,
                    }],
                }),
            },
            evidence_hashes: vec![observed, missing_hash],
        };

        let rendered = json::to_json_pretty(&event).expect("serialize feed event");
        fs::write(&events_path, rendered).expect("write events");

        let observation_dir = tmp.path().join("observations");
        fs::create_dir(&observation_dir).expect("mkdir observations");
        let observation_path = observation_dir.join("obs.json");
        fs::write(&observation_path, observed_payload).expect("write observation");

        let options = Bundle {
            events: events_path.clone(),
            output: artifacts_root.clone(),
            observations: Some(observation_dir),
            reports: None,
            responses: None,
            disputes: None,
            telemetry: None,
        };

        let bundle_root = normalize_dir(&options.output).expect("normalize output");
        let summary = build_bundle(&options, &bundle_root).expect("build bundle");
        assert_eq!(summary.feed_events.len(), 1);
        assert_eq!(summary.evidence.len(), 1);
        assert_eq!(
            summary.feed_events[0].missing_evidence_hashes,
            vec![missing_hash]
        );
        assert_eq!(summary.coverage.total_feed_events, 1);
        assert_eq!(summary.coverage.events_with_evidence, 1);
        assert_eq!(summary.coverage.evidence_entries, 1);
        assert_eq!(summary.coverage.missing_hashes_total, 1);
        assert_eq!(
            summary
                .coverage
                .missing_hashes_by_feed
                .get(feed_id.as_str()),
            Some(&1)
        );
        assert_eq!(
            summary
                .coverage
                .evidence_by_kind
                .get("observation")
                .copied(),
            Some(1)
        );
        let bundled = artifacts_root.join("bundle.json");
        assert!(!bundled.exists(), "summary writing handled by run");
        let copied = artifacts_root.join("artifacts");
        assert!(copied.exists(), "artifact directory created");
        assert!(
            fs::read_dir(&copied).expect("read dir").next().is_some(),
            "artifact file written"
        );
        let index_path = copied.join("index.json");
        let index_bytes = fs::read(&index_path).expect("index exists");
        let index_value: Value = json::from_slice(&index_bytes).expect("index parses");
        match index_value {
            Value::Array(entries) => assert_eq!(entries.len(), 1, "index entry count"),
            other => panic!("unexpected index format: {other:?}"),
        }
    }

    #[test]
    fn catalog_markdown_lists_codes() {
        let catalog = build_rejection_catalog();
        let rendered = render_catalog_markdown(&catalog);
        assert!(
            rendered.contains("Observation errors"),
            "missing observation header"
        );
        assert!(
            rendered.contains("ResourceUnavailable"),
            "expected ResourceUnavailable entry"
        );
        assert!(
            rendered.contains("Aggregation errors"),
            "missing aggregation header"
        );
        assert!(
            rendered.contains("oracle_model_feed_version_mismatch"),
            "expected model code entry"
        );
        assert!(
            rendered.contains("oracle_agg_no_inliers"),
            "expected aggregation code entry"
        );
        assert!(
            rendered.contains("Catalog version"),
            "expected catalog version header"
        );
    }

    #[test]
    fn catalog_run_ignores_markdown_when_output_is_json() {
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);
        Catalog {
            format: CatalogFormat::Markdown,
        }
        .run(&mut ctx)
        .expect("run catalog");
        assert_eq!(ctx.printed.len(), 1);
        let value: Value = json::from_str(&ctx.printed[0]).expect("json output");
        let obj = value.as_object().expect("object");
        assert!(obj.contains_key("observation_errors"));
    }

    #[test]
    fn bundle_run_emits_json_output() {
        let tmp = TempDir::new().expect("tmpdir");
        let events_path = tmp.path().join("events.json");
        let artifacts_root = tmp.path().join("bundle");
        let observed_payload = br#"{"dummy":"payload"}"#;
        let feed_id: FeedId = "xor_usd".parse().expect("feed id");
        let observed = Hash::new(observed_payload);
        let observation_hash = HashOf::<ObservationBody>::from_untyped_unchecked(Hash::new(b"req"));
        let event = FeedEventRecord {
            event: FeedEvent {
                feed_id,
                feed_config_version: FeedConfigVersion(1),
                slot: 42,
                request_hash: Hash::new(b"request"),
                outcome: FeedEventOutcome::Success(FeedSuccess {
                    value: ObservationValue::new(1_000, 2),
                    entries: vec![ReportEntry {
                        oracle_id: test_oracle_account_id(),
                        observation_hash,
                        value: ObservationValue::new(1_000, 2),
                        outlier: false,
                    }],
                }),
            },
            evidence_hashes: vec![observed],
        };
        let rendered = json::to_json_pretty(&event).expect("serialize feed event");
        fs::write(&events_path, rendered).expect("write events");
        let observation_dir = tmp.path().join("observations");
        fs::create_dir(&observation_dir).expect("mkdir observations");
        let observation_path = observation_dir.join("obs.json");
        fs::write(&observation_path, observed_payload).expect("write observation");

        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);
        Bundle {
            events: events_path,
            output: artifacts_root,
            observations: Some(observation_dir),
            reports: None,
            responses: None,
            disputes: None,
            telemetry: None,
        }
        .run(&mut ctx)
        .expect("bundle run");
        assert_eq!(ctx.printed.len(), 1);
        let value: Value = json::from_str(&ctx.printed[0]).expect("json output");
        let obj = value.as_object().expect("object");
        assert!(obj.contains_key("manifest_path"));
        assert!(obj.contains_key("summary"));
    }

    #[test]
    fn gc_run_emits_json_output() {
        let tmp = TempDir::new().expect("tmpdir");
        let root = tmp.path().join("soracles");
        fs::create_dir(&root).expect("mkdir root");
        let report_path = root.join("gc_report.json");
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);
        GcArgs {
            root,
            retention_days: 1,
            dispute_retention_days: 1,
            report: Some(report_path.clone()),
            prune_unreferenced: false,
            dry_run: true,
        }
        .run(&mut ctx)
        .expect("gc run");
        assert_eq!(ctx.printed.len(), 1);
        let value: Value = json::from_str(&ctx.printed[0]).expect("json output");
        let obj = value.as_object().expect("object");
        assert_eq!(
            obj.get("report_path").and_then(Value::as_str),
            Some(report_path.to_string_lossy().as_ref())
        );
        assert!(obj.get("report").is_some());
    }

    #[test]
    fn gc_removes_expired_bundles_and_keeps_fresh() {
        let tmp = TempDir::new().expect("tmpdir");
        let now = SystemTime::now();
        let old_time = now
            .checked_sub(Duration::from_secs(172_800))
            .expect("time travel");
        let old_secs = now_unix(old_time).expect("unix time");
        let fresh_secs = now_unix(now).expect("unix time");

        let expired = write_test_bundle(
            tmp.path(),
            "expired",
            old_secs,
            "observation",
            &[("artifacts/old.json", b"old")],
        );
        let fresh = write_test_bundle(
            tmp.path(),
            "fresh",
            fresh_secs,
            "observation",
            &[("artifacts/fresh.json", b"fresh")],
        );

        let report = garbage_collect(tmp.path(), 1, 365, false, false, now).expect("gc succeeds");
        assert_eq!(report.removed_bundles.len(), 1);
        assert!(!expired.exists(), "expired bundle should be removed");
        assert!(fresh.exists(), "fresh bundle should remain");
    }

    #[test]
    fn gc_prunes_unreferenced_files_in_dry_run() {
        let tmp = TempDir::new().expect("tmpdir");
        let now = SystemTime::now();
        let bundle = write_test_bundle(
            tmp.path(),
            "keep",
            now_unix(now).expect("unix time"),
            "observation",
            &[("artifacts/ref.bin", b"ref")],
        );
        let orphan = bundle.join("artifacts/orphan.bin");
        fs::create_dir_all(orphan.parent().expect("parent")).expect("mkdir orphan parent");
        fs::write(&orphan, b"orphaned").expect("write orphan");

        let report = garbage_collect(tmp.path(), 365, 365, true, true, now)
            .expect("gc succeeds with dry-run");
        assert_eq!(
            report.pruned_files.len(),
            1,
            "unreferenced file should be reported"
        );
        assert!(orphan.exists(), "dry-run must not delete files");
        assert_eq!(report.retained_bundles, 1);
    }

    #[test]
    fn gc_respects_longer_dispute_retention() {
        let tmp = TempDir::new().expect("tmpdir");
        let now = SystemTime::now();
        let old_time = now
            .checked_sub(Duration::from_secs(200 * 86_400))
            .expect("time travel");
        let old_secs = now_unix(old_time).expect("unix time");

        let dispute_bundle = write_test_bundle(
            tmp.path(),
            "dispute",
            old_secs,
            "dispute",
            &[("artifacts/dispute.json", b"dispute")],
        );
        assert!(
            dispute_bundle.exists(),
            "bundle should be laid out for gc test"
        );

        let report = garbage_collect(tmp.path(), 180, 365, false, false, now)
            .expect("gc succeeds with dispute retention");
        assert_eq!(
            report.removed_bundles.len(),
            0,
            "dispute bundle should remain under longer window"
        );
        assert!(dispute_bundle.exists(), "dispute evidence not pruned early");
    }

    fn write_test_bundle(
        root: &Path,
        name: &str,
        generated_at_unix: u64,
        kind: &str,
        artifacts: &[(&str, &[u8])],
    ) -> PathBuf {
        let bundle_root = root.join(name);
        let artifact_dir = bundle_root.join("artifacts");
        fs::create_dir_all(&artifact_dir).expect("create artifact dir");

        let mut evidence_entries = Vec::new();
        for &(relative_path, contents) in artifacts {
            let full_path = bundle_root.join(relative_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).expect("create parent dirs");
            }
            fs::write(&full_path, contents).expect("write artifact");
            let hash = Hash::new(contents);
            evidence_entries.push(EvidenceEntry {
                kind: kind.to_string(),
                hash,
                sources: vec![],
                bundled_path: relative_path.to_string(),
                size_bytes: contents.len() as u64,
            });
        }

        let mut evidence_by_kind = BTreeMap::new();
        evidence_by_kind.insert(kind.to_string(), evidence_entries.len());
        let summary = BundleSummary {
            generated_at_unix,
            artifact_root: "artifacts".to_string(),
            feed_events: Vec::new(),
            evidence: evidence_entries,
            coverage: BundleCoverage {
                total_feed_events: 0,
                events_with_evidence: 0,
                evidence_entries: artifacts.len(),
                evidence_by_kind,
                missing_hashes_total: 0,
                missing_hashes_by_feed: BTreeMap::new(),
            },
        };
        let manifest = json::to_json_pretty(&summary).expect("render bundle");
        fs::create_dir_all(&bundle_root).expect("create bundle root");
        fs::write(bundle_root.join("bundle.json"), manifest).expect("write bundle manifest");
        bundle_root
    }
}
