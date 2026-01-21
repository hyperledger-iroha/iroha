//! Developer automation entrypoint for repository maintenance tasks.
#![allow(unexpected_cfgs)]

use std::{
    cmp::Ordering,
    collections::BTreeMap,
    env,
    error::Error,
    fmt::Write as FmtWrite,
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    process,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    body::{self, Body},
    extract::ConnectInfo,
    http::Request,
};
use eyre::eyre;
use iroha_config::{
    base::read::ConfigReader,
    parameters::{
        actual::{self, IsoReferenceData, SorafsRolloutPhase},
        user,
    },
};
use iroha_core::{
    EventsSender,
    iso_bridge::reference_data::{
        DatasetSnapshot, ReferenceDataError, ReferenceDataSnapshots, SnapshotState,
        ValidationOutcome,
    },
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
};
use iroha_crypto::{
    Algorithm, KeyPair, PrivateKey, PublicKey, Signature,
    soranet::certificate::CertificateValidationPhase,
};
use iroha_data_model::{
    account::address::compliance_vectors::compliance_vectors_json, nexus::AssetPermissionManifest,
};
use iroha_torii::{
    MaybeTelemetry, OnlinePeersProvider,
    test_utils::{TestDataDirGuard, mk_minimal_root_cfg},
};
use norito::{
    derive::JsonSerialize,
    json::{self, Value},
    streaming::{
        codec::{BundleContextId, ContextFrequency},
        load_bundle_tables_from_toml,
    },
    to_bytes,
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use time::{
    Date, OffsetDateTime,
    format_description::{self, well_known::Rfc3339},
};
use tokio::runtime::Builder as TokioRuntimeBuilder;
use tower::ServiceExt;

mod da;
mod vote_tally;
use fastpq::{BenchInput, BenchManifestOptions};
use sorafs_manifest::deal::XorAmount;
use soranet_testnet::{
    DrillBundleOptions, VerificationAttachment, VerificationFeedOptions, evaluate_testnet_metrics,
    generate_drill_bundle, generate_testnet_kit, generate_verification_feed,
};
use vote_tally::{
    BundleSummary, bundle_file_names, iroha_hash, read_summary, summary_to_json, write_bundle,
};
mod address_gate;
mod address_manifest;
mod codec;
mod compute;
mod compute_harness;
mod docs_preview;
mod fastpq;
mod gar;
mod gar_bus;
mod i3_bench_suite;
mod i3_slo_harness;
mod kagami_profiles;
mod ministry;
mod ministry_agenda;
mod ministry_jury;
mod ministry_panel;
mod mochi;
mod nexus;
mod nexus_lane_maintenance;
mod norito_rpc;
mod offline_bundle;
mod offline_pos_provision;
mod offline_pos_verify;
mod offline_poseidon;
mod offline_provision;
mod offline_tooling;
mod offline_topup;
mod poseidon_bench;
mod sm;
mod sns;
mod soradns;
mod sorafs;
mod soranet;
mod soranet_billing;
mod soranet_bug_bounty;
mod soranet_chaos;
mod soranet_gar_controller;
mod soranet_gateway;
mod soranet_gateway_billing;
mod soranet_gateway_chaos;
mod soranet_gateway_hardening;
mod soranet_gateway_m1;
mod soranet_gateway_m2;
mod soranet_gateway_m3;
mod soranet_gateway_ops;
mod soranet_gateway_pq;
mod soranet_pop;
mod soranet_privacy;
mod soranet_rollout;
mod soranet_testnet;
#[cfg(test)]
mod soranet_vpn;
mod stage1_bench;
mod streaming_bench;
mod taikai;
mod taikai_anchor;
use crate::{
    norito_rpc::{
        FixtureOptions as NoritoRpcFixtureOptions, generate_fixtures as run_norito_rpc_fixtures,
        run_verify,
    },
    offline_bundle::OfflineBundleOptions,
    offline_pos_provision::OfflinePosProvisionOptions,
    offline_pos_verify::OfflinePosVerifyOptions,
    offline_provision::OfflineProvisionOptions,
    offline_topup::{OfflineTopupOptions, RegisterMode, RegisterOptions},
};

fn main() {
    if let Err(err) = entrypoint() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

enum CommandKind {
    OpenApi {
        outputs: Vec<PathBuf>,
        allow_stub: bool,
        manifest: PathBuf,
        signing_key: Option<PathBuf>,
    },
    OpenApiVerify {
        spec: PathBuf,
        manifest: PathBuf,
        allow_unsigned: bool,
        allowed_signers: PathBuf,
    },
    AddressVectors {
        target: JsonTarget,
        verify: bool,
    },
    AddressLocalGate {
        input: PathBuf,
        window_days: u64,
        json_out: Option<JsonTarget>,
        check_collisions: bool,
    },
    SoranetFixtures {
        output: PathBuf,
        verify: bool,
    },
    SoranetChaosKit {
        options: soranet_chaos::ChaosKitOptions,
    },
    SoranetChaosReport {
        options: soranet_chaos::ChaosReportOptions,
    },
    DocsPreview(docs_preview::Command),
    ComputeSloReport {
        options: compute::ComputeSloReportOptions,
    },
    ComputeFixtures {
        options: compute::ComputeFixtureOptions,
    },
    ZkVoteTallyBundle {
        output: PathBuf,
        verify: bool,
        print_hashes: bool,
        summary_target: Option<JsonTarget>,
        attestation_target: Option<JsonTarget>,
    },
    MochiBundle {
        output: PathBuf,
        profile: String,
        archive: bool,
        kagami: Option<PathBuf>,
        matrix: Option<PathBuf>,
        smoke: bool,
        stage: Option<PathBuf>,
    },
    KagamiProfiles {
        options: kagami_profiles::KagamiProfileOptions,
    },
    SorafsAdmissionFixtures {
        output: PathBuf,
    },
    SorafsFetchFixture {
        options: Box<sorafs::FetchFixtureOptions>,
    },
    SorafsGatewayFixtures {
        output: PathBuf,
        verify: bool,
    },
    SorafsPinFixtures {
        output: PathBuf,
    },
    SorafsAdoptionCheck {
        options: sorafs::AdoptionCheckOptions,
        report: Option<PathBuf>,
    },
    SnsPortalStub {
        options: sns::PortalStubOptions,
    },
    SorafsScoreboardDiff {
        options: sorafs::ScoreboardDiffOptions,
        report: Option<PathBuf>,
    },
    SorafsTaikaiCacheBundle {
        options: Box<sorafs::TaikaiCacheBundleOptions>,
    },
    SorafsBurnInCheck {
        options: sorafs::BurnInCheckOptions,
        output: Option<PathBuf>,
    },
    SorafsReserveMatrix {
        options: sorafs::ReserveMatrixOptions,
        target: JsonTarget,
    },
    SorafsGatewayAttest {
        options: Box<sorafs::GatewayAttestOptions>,
    },
    SorafsGatewayProbe {
        options: Box<sorafs::GatewayProbeOptions>,
    },
    SorafsGatewayCli {
        command: Box<sorafs::GatewayCliCommand>,
    },
    SoranetPopBundle {
        options: Box<soranet_pop::PopBundleOptions>,
    },
    SoranetPopCtl {
        options: Box<soranet_pop::PopBundleOptions>,
    },
    SoranetPopTemplate {
        input: PathBuf,
        output: PathBuf,
        resolver_config: Option<PathBuf>,
        edns_out: Option<PathBuf>,
        edns_resolver: Option<String>,
        ds_out: Option<PathBuf>,
        edns_tool: Option<String>,
    },
    SoranetPopValidate {
        input: PathBuf,
        roa: Option<PathBuf>,
        output: JsonTarget,
    },
    SoranetPopPolicyReport {
        options: Box<soranet_pop::PopPolicyHarnessOptions>,
    },
    SoranetPopPlan {
        input: PathBuf,
        roa: Option<PathBuf>,
        frr_out: PathBuf,
        report_out: JsonTarget,
    },
    SoranetGatewayM0 {
        options: soranet_gateway::GatewayM0Options,
    },
    SoranetGatewayM1 {
        options: soranet_gateway_m1::GatewayM1Options,
    },
    SoranetGatewayM2 {
        options: soranet_gateway_m2::GatewayM2Options,
    },
    SoranetGatewayM3 {
        options: soranet_gateway_m3::GatewayM3Options,
    },
    SoranetGatewayOpsM0 {
        options: soranet_gateway_ops::GatewayOpsOptions,
    },
    SoranetGatewayPq {
        options: soranet_gateway_pq::GatewayPqOptions,
    },
    SoranetGarController {
        options: soranet_gar_controller::GarControllerOptions,
    },
    SoranetBugBounty {
        options: soranet_bug_bounty::BugBountyOptions,
    },
    SoranetGatewayOpsM1 {
        options: soranet_gateway_ops::GatewayOpsMultiOptions,
    },
    SoranetGatewayChaos {
        options: soranet_gateway_chaos::ChaosOptions,
    },
    SoranetGatewayHardening {
        options: soranet_gateway_hardening::GatewayHardeningOptions,
    },
    SoranetGarExport {
        options: gar::ExportOptions,
        output: JsonTarget,
    },
    SoranetGarBus {
        options: gar_bus::GarBusOptions,
        output: JsonTarget,
    },
    SoranetGatewayBilling {
        options: soranet_gateway_billing::BillingOptions,
    },
    SoranetGatewayBillingM0 {
        options: soranet_billing::BillingM0Options,
    },
    TaikaiAnchorBundle {
        options: taikai_anchor::AnchorBundleOptions,
    },
    TaikaiRptVerify {
        options: taikai::RptVerifyOptions,
    },
    SoranetPrivacyReport {
        options: soranet_privacy::PrivacyReportOptions,
        json_out: Option<JsonTarget>,
        max_bucket_records: usize,
        suppression_ratio_budget: Option<f64>,
    },
    SoranetConstantRateProfile {
        selection: soranet::ConstantRateSelection,
        format: soranet::ConstantRateOutputFormat,
        include_tick_table: bool,
        tick_values: Vec<f64>,
        json_out: Option<JsonTarget>,
    },
    NexusLaneMaintenance {
        options: nexus_lane_maintenance::LaneMaintenanceOptions,
    },
    NexusLaneAudit {
        options: nexus::LaneAuditOptions,
    },
    NexusFixtures {
        output: PathBuf,
        verify: bool,
    },
    SoranetTestnetKit {
        output: PathBuf,
    },
    SoranetTestnetMetrics {
        input: PathBuf,
        output: JsonTarget,
    },
    SoranetTestnetFeed {
        options: Box<VerificationFeedOptions>,
    },
    SoranetTestnetDrillBundle {
        options: Box<DrillBundleOptions>,
    },
    FastpqBenchManifest {
        options: Box<BenchManifestOptions>,
    },
    FastpqStageProfile {
        options: Box<fastpq::StageProfileOptions>,
    },
    FastpqCudaSuite {
        options: Box<fastpq::CudaSuiteOptions>,
    },
    SoradnsDirectoryRelease {
        options: Box<soradns::DirectoryReleaseOptions>,
    },
    SoradnsHosts {
        names: Vec<String>,
        json: Option<JsonTarget>,
        verify: Vec<PathBuf>,
    },
    SoradnsBindingTemplate {
        options: soradns::BindingTemplateOptions,
        json: Option<JsonTarget>,
        headers_out: Option<PathBuf>,
    },
    SoradnsGarTemplate {
        options: soradns::GarTemplateOptions,
        output: Option<JsonTarget>,
    },
    SoradnsAcmePlan {
        options: soradns::AcmePlanOptions,
        output: JsonTarget,
    },
    SoradnsCachePlan {
        options: soradns::CacheInvalidationPlanOptions,
        output: JsonTarget,
    },
    SoradnsRoutePlan {
        options: soradns::RoutePlanOptions,
        output: JsonTarget,
    },
    SoradnsVerifyBinding {
        binding: PathBuf,
        alias: Option<String>,
        content_cid: Option<String>,
        hostname: Option<String>,
        proof_status: Option<String>,
        manifest: Option<PathBuf>,
    },
    SoradnsVerifyGar {
        options: SoradnsVerifyGarOptions,
    },
    DaThreatModelReport {
        output: JsonTarget,
        seed: u64,
        config_path: Option<PathBuf>,
    },
    DaReplicationAudit {
        options: da::ReplicationAuditOptions,
    },
    DaCommitmentReconcile {
        options: da::CommitmentReconcileOptions,
    },
    DaPrivilegeAudit {
        options: da::PrivilegeAuditOptions,
    },
    DaProofBench {
        options: da::ProofBenchOptions,
    },
    ConfigDebug {
        config: PathBuf,
    },
    SoranetRolloutPlan {
        options: soranet_rollout::PlanOptions,
    },
    SoranetRolloutCapture {
        options: soranet_rollout::CaptureOptions,
    },
    SmWycheproofSync(sm::WycheproofSyncOptions),
    SmOperatorSnippet {
        options: sm::SmOperatorSnippetOptions,
    },
    IsoBridgeLint {
        options: IsoLintOptions,
    },
    CodecRansTables {
        options: codec::RansTablesOptions,
    },
    VerifyRansTables {
        paths: Vec<PathBuf>,
    },
    StreamingBundleCheck {
        options: StreamingBundleCheckOptions,
    },
    StreamingEntropyBench {
        options: streaming_bench::EntropyBenchOptions,
        output: JsonTarget,
    },
    StreamingDecode {
        options: streaming_bench::DecodeOptions,
        output: Option<JsonTarget>,
    },
    Stage1Bench {
        options: stage1_bench::Stage1BenchOptions,
    },
    I3BenchSuite {
        options: i3_bench_suite::I3BenchOptions,
    },
    I3SloHarness {
        options: i3_slo_harness::SloHarnessOptions,
    },
    PoseidonBench {
        options: poseidon_bench::PoseidonBenchOptions,
    },
    StreamingContextRemap {
        options: StreamingContextRemapOptions,
    },
    SpaceDirectoryEncode {
        input: PathBuf,
        output: PathBuf,
    },
    SnsScorecard(sns::ScorecardOptions),
    SnsAnnex(sns::AnnexOptions),
    SnsCatalogVerify(sns::CatalogVerifyOptions),
    MinistryTransparency(ministry::Command),
    MinistryAgenda(ministry_agenda::Command),
    MinistryPanel(ministry_panel::Command),
    MinistryJury(ministry_jury::Command),
    AddressManifestVerify {
        options: address_manifest::VerifyOptions,
    },
    NoritoRpcVerify {
        json_out: Option<JsonTarget>,
    },
    NoritoRpcFixtures {
        options: NoritoRpcFixtureOptions,
    },
    OfflinePosProvision(OfflinePosProvisionOptions),
    OfflinePosVerify(OfflinePosVerifyOptions),
    OfflineProvision(OfflineProvisionOptions),
    OfflineTopup(OfflineTopupOptions),
    OfflineBundle(OfflineBundleOptions),
    OfflinePoseidonFixtures(offline_poseidon::OfflinePoseidonFixtureOptions),
    AccelerationState {
        format: AccelerationOutputFormat,
    },
    Help,
}

#[derive(Clone, Debug)]
pub(crate) enum JsonTarget {
    Stdout,
    File(PathBuf),
}

#[derive(Clone, Copy)]
enum AccelerationOutputFormat {
    Json,
    Table,
}

impl AccelerationOutputFormat {
    fn parse(value: &str) -> Result<Self, Box<dyn Error>> {
        match value {
            "json" => Ok(Self::Json),
            "table" => Ok(Self::Table),
            other => Err(format!("unknown format `{other}` (expected json or table)").into()),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct AccelerationConfigReport {
    enable_simd: bool,
    enable_metal: bool,
    enable_cuda: bool,
    max_gpus: Option<usize>,
    merkle_min_leaves_gpu: Option<usize>,
    merkle_min_leaves_metal: Option<usize>,
    merkle_min_leaves_cuda: Option<usize>,
    prefer_cpu_sha2_max_leaves_aarch64: Option<usize>,
    prefer_cpu_sha2_max_leaves_x86: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
struct AccelerationBackendReport {
    supported: bool,
    configured: bool,
    available: bool,
    parity_ok: bool,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct AccelerationStateReport {
    config: AccelerationConfigReport,
    simd: AccelerationBackendReport,
    metal: AccelerationBackendReport,
    cuda: AccelerationBackendReport,
}

impl AccelerationStateReport {
    fn capture() -> Self {
        Self::from_parts(
            ivm::acceleration_config(),
            ivm::acceleration_runtime_status(),
            ivm::acceleration_runtime_errors(),
        )
    }

    fn from_parts(
        config: ivm::AccelerationConfig,
        runtime: ivm::AccelerationRuntimeStatus,
        errors: ivm::AccelerationErrorStatus,
    ) -> Self {
        Self {
            config: AccelerationConfigReport::from_config(config),
            simd: AccelerationBackendReport::from_status(runtime.simd, errors.simd),
            metal: AccelerationBackendReport::from_status(runtime.metal, errors.metal),
            cuda: AccelerationBackendReport::from_status(runtime.cuda, errors.cuda),
        }
    }
}

impl AccelerationConfigReport {
    fn from_config(config: ivm::AccelerationConfig) -> Self {
        Self {
            enable_simd: config.enable_simd,
            enable_metal: config.enable_metal,
            enable_cuda: config.enable_cuda,
            max_gpus: config.max_gpus,
            merkle_min_leaves_gpu: config.merkle_min_leaves_gpu,
            merkle_min_leaves_metal: config.merkle_min_leaves_metal,
            merkle_min_leaves_cuda: config.merkle_min_leaves_cuda,
            prefer_cpu_sha2_max_leaves_aarch64: config.prefer_cpu_sha2_max_leaves_aarch64,
            prefer_cpu_sha2_max_leaves_x86: config.prefer_cpu_sha2_max_leaves_x86,
        }
    }
}

impl AccelerationBackendReport {
    fn from_status(status: ivm::BackendRuntimeStatus, last_error: Option<String>) -> Self {
        Self {
            supported: status.supported,
            configured: status.configured,
            available: status.available,
            parity_ok: status.parity_ok,
            last_error,
        }
    }
}

fn run_acceleration_state(format: AccelerationOutputFormat) -> Result<(), Box<dyn Error>> {
    let state = AccelerationStateReport::capture();
    let rendered = render_acceleration_state(&state, format)?;
    println!("{rendered}");
    Ok(())
}

fn render_acceleration_state(
    state: &AccelerationStateReport,
    format: AccelerationOutputFormat,
) -> Result<String, Box<dyn Error>> {
    match format {
        AccelerationOutputFormat::Json => Ok(serde_json::to_string_pretty(state)?),
        AccelerationOutputFormat::Table => Ok(format_acceleration_table(state)),
    }
}

fn format_acceleration_table(state: &AccelerationStateReport) -> String {
    let mut output = String::new();
    let cfg = &state.config;
    let _ = writeln!(output, "Acceleration Configuration");
    let _ = writeln!(output, "--------------------------");
    let _ = writeln!(output, "enable_simd: {}", format_bool(cfg.enable_simd));
    let _ = writeln!(output, "enable_metal: {}", format_bool(cfg.enable_metal));
    let _ = writeln!(output, "enable_cuda: {}", format_bool(cfg.enable_cuda));
    let _ = writeln!(output, "max_gpus: {}", format_optional(cfg.max_gpus));
    let _ = writeln!(
        output,
        "merkle_min_leaves_gpu: {}",
        format_optional(cfg.merkle_min_leaves_gpu)
    );
    let _ = writeln!(
        output,
        "merkle_min_leaves_metal: {}",
        format_optional(cfg.merkle_min_leaves_metal)
    );
    let _ = writeln!(
        output,
        "merkle_min_leaves_cuda: {}",
        format_optional(cfg.merkle_min_leaves_cuda)
    );
    let _ = writeln!(
        output,
        "prefer_cpu_sha2_max_leaves_aarch64: {}",
        format_optional(cfg.prefer_cpu_sha2_max_leaves_aarch64)
    );
    let _ = writeln!(
        output,
        "prefer_cpu_sha2_max_leaves_x86: {}",
        format_optional(cfg.prefer_cpu_sha2_max_leaves_x86)
    );
    let _ = writeln!(output, "Backend Status");
    let _ = writeln!(output, "--------------");
    let _ = writeln!(
        output,
        "{:<6} {:<10} {:<11} {:<10} {:<9} Last error",
        "Backend", "Supported", "Configured", "Available", "ParityOK"
    );
    for (name, backend) in [
        ("SIMD", &state.simd),
        ("Metal", &state.metal),
        ("CUDA", &state.cuda),
    ] {
        let last_error = backend.last_error.as_deref().unwrap_or("-");
        let _ = writeln!(
            output,
            "{:<6} {:<10} {:<11} {:<10} {:<9} {}",
            name,
            format_bool(backend.supported),
            format_bool(backend.configured),
            format_bool(backend.available),
            format_bool(backend.parity_ok),
            last_error
        );
    }
    output
}

fn format_optional(value: Option<usize>) -> String {
    match value {
        Some(v) => v.to_string(),
        None => "auto".to_string(),
    }
}

fn format_bool(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[derive(Default)]
struct IsoLintOptions {
    isin_path: Option<PathBuf>,
    bic_lei_path: Option<PathBuf>,
    mic_path: Option<PathBuf>,
    fixtures_path: Option<PathBuf>,
}

impl IsoLintOptions {
    fn is_empty(&self) -> bool {
        self.isin_path.is_none()
            && self.bic_lei_path.is_none()
            && self.mic_path.is_none()
            && self.fixtures_path.is_none()
    }

    fn ensure_defaults(&mut self) {
        if self.is_empty() {
            let root = workspace_root();
            self.isin_path = Some(root.join("fixtures/iso_bridge/isin_crosswalk.sample.json"));
            self.bic_lei_path = Some(root.join("fixtures/iso_bridge/bic_lei.sample.json"));
            self.mic_path = Some(root.join("fixtures/iso_bridge/mic_directory.sample.json"));
            self.fixtures_path = Some(root.join("fixtures/iso_bridge/fixtures.json"));
        }
    }
}

struct IsoFixtures {
    instruments: Vec<String>,
    bics: Vec<String>,
    leis: Vec<String>,
    mics: Vec<String>,
}

struct StreamingBundleCheckOptions {
    config_path: PathBuf,
    tables_override: Option<PathBuf>,
    output: JsonTarget,
}

struct StreamingContextRemapOptions {
    input: PathBuf,
    top: usize,
    output: JsonTarget,
}

struct SoradnsVerifyGarOptions {
    gar_path: PathBuf,
    expectations: soradns::GarVerifyOptions,
    json_out: Option<JsonTarget>,
}

impl IsoFixtures {
    fn load(path: &Path) -> Result<Self, Box<dyn Error>> {
        let raw = fs::read_to_string(path)?;
        let value: Value = norito::json::from_str(&raw)?;
        Ok(Self {
            instruments: string_array(&value, "instruments")?,
            bics: string_array(&value, "bics")?,
            leis: string_array(&value, "leis")?,
            mics: string_array(&value, "mics")?,
        })
    }
}

fn parse_u64_flag(raw: &str, flag: &str) -> Result<u64, Box<dyn Error>> {
    let trimmed = raw.trim();
    let parsed = trimmed
        .parse::<u64>()
        .map_err(|_| format!("{flag} requires an unsigned integer"))?;
    Ok(parsed)
}

fn unix_timestamp_now() -> Result<u64, Box<dyn Error>> {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => Ok(duration.as_secs()),
        Err(err) => Err(Box::new(err)),
    }
}

fn string_array(root: &Value, key: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let Some(value) = root.as_object().and_then(|map| map.get(key)) else {
        return Ok(Vec::new());
    };
    let array = value
        .as_array()
        .ok_or_else(|| format!("`{key}` must be an array of strings"))?;
    let mut out = Vec::with_capacity(array.len());
    for entry in array {
        let text = entry
            .as_str()
            .ok_or_else(|| format!("`{key}` entries must be strings"))?
            .trim();
        if !text.is_empty() {
            out.push(text.to_string());
        }
    }
    Ok(out)
}

fn ensure_dataset_loaded<T>(
    snapshot: &DatasetSnapshot<T>,
    label: &str,
) -> Result<(), Box<dyn Error>> {
    match snapshot.state() {
        SnapshotState::Loaded => Ok(()),
        SnapshotState::Missing => Err(format!("{label} dataset was not provided").into()),
        SnapshotState::Failed => {
            let diagnostics = snapshot.diagnostics().unwrap_or("unknown error");
            let source = snapshot
                .configured_path()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "unknown path".to_string());
            Err(format!("{label} dataset at {source} failed to load: {diagnostics}").into())
        }
    }
}

fn ensure_reference(
    label: &str,
    outcome: Result<ValidationOutcome, ReferenceDataError>,
) -> Result<(), Box<dyn Error>> {
    match outcome {
        Ok(ValidationOutcome::Enforced) => Ok(()),
        Ok(ValidationOutcome::Skipped) => {
            Err(format!("{label} validation skipped because dataset was unavailable").into())
        }
        Err(err) => Err(format!("{label} validation failed: {err}").into()),
    }
}

fn lint_iso_bridge(mut options: IsoLintOptions) -> Result<(), Box<dyn Error>> {
    options.ensure_defaults();

    let config = IsoReferenceData {
        isin_crosswalk_path: options.isin_path.clone(),
        bic_lei_path: options.bic_lei_path.clone(),
        mic_directory_path: options.mic_path.clone(),
        ..IsoReferenceData::default()
    };

    let snapshots = ReferenceDataSnapshots::from_config(&config);

    if options.isin_path.is_some() {
        ensure_dataset_loaded(snapshots.isin_cusip(), "ISIN↔CUSIP crosswalk")?;
    }
    if options.bic_lei_path.is_some() {
        ensure_dataset_loaded(snapshots.bic_lei(), "BIC↔LEI crosswalk")?;
    }
    if options.mic_path.is_some() {
        ensure_dataset_loaded(snapshots.mic_directory(), "MIC directory")?;
    }

    if let Some(fixtures_path) = options.fixtures_path.as_ref() {
        let fixtures = IsoFixtures::load(fixtures_path)?;

        if !fixtures.instruments.is_empty() {
            if options.isin_path.is_none() {
                return Err(
                    "instrument fixtures specified but no ISIN crosswalk configured".into(),
                );
            }
            for isin in &fixtures.instruments {
                ensure_reference(&format!("Instrument {isin}"), snapshots.validate_isin(isin))?;
            }
        }

        if !fixtures.bics.is_empty() {
            if options.bic_lei_path.is_none() {
                return Err("BIC fixtures specified but no BIC↔LEI dataset configured".into());
            }
            for bic in &fixtures.bics {
                ensure_reference(&format!("BIC {bic}"), snapshots.validate_bic(bic))?;
            }
        }

        if !fixtures.leis.is_empty() {
            if options.bic_lei_path.is_none() {
                return Err("LEI fixtures specified but no BIC↔LEI dataset configured".into());
            }
            for lei in &fixtures.leis {
                ensure_reference(&format!("LEI {lei}"), snapshots.validate_lei(lei))?;
            }
        }

        if !fixtures.mics.is_empty() {
            if options.mic_path.is_none() {
                return Err("MIC fixtures specified but no MIC directory configured".into());
            }
            for mic in &fixtures.mics {
                ensure_reference(&format!("MIC {mic}"), snapshots.validate_mic(mic))?;
            }
        }
    }

    println!("ISO bridge reference data lint passed");
    Ok(())
}

fn streaming_bundle_check(options: StreamingBundleCheckOptions) -> Result<(), Box<dyn Error>> {
    let summary = build_streaming_bundle_summary(&options.config_path, options.tables_override)?;
    write_json_output(&summary, options.output)?;
    Ok(())
}

fn build_streaming_bundle_summary(
    config_path: &Path,
    tables_override: Option<PathBuf>,
) -> Result<Value, Box<dyn Error>> {
    let actual = load_actual_config(config_path)?;
    let codec = &actual.streaming.codec;
    let tables_path = tables_override.unwrap_or_else(|| codec.rans_tables_path.clone());
    let tables_path = if tables_path.is_relative() {
        workspace_root().join(&tables_path)
    } else {
        tables_path
    };
    let tables = load_bundle_tables_from_toml(&tables_path).map_err(|err| {
        format!(
            "failed to load bundled rANS tables `{}`: {err}",
            tables_path.display()
        )
    })?;
    let checksum_hex = hex::encode(tables.checksum());
    let mut tables_json = json::Map::new();
    tables_json.insert(
        "path".into(),
        json::Value::from(tables_path.display().to_string()),
    );
    tables_json.insert(
        "precision_bits".into(),
        json::Value::from(tables.precision_bits()),
    );
    tables_json.insert("checksum".into(), json::Value::from(checksum_hex));

    let mut summary = json::Map::new();
    summary.insert(
        "config".into(),
        json::Value::from(config_path.display().to_string()),
    );
    summary.insert(
        "feature_bits".into(),
        json::Value::from(actual.streaming.feature_bits),
    );
    summary.insert(
        "bundle_required".into(),
        json::Value::from(codec.entropy_mode.is_bundled()),
    );
    summary.insert(
        "entropy_mode".into(),
        json::Value::from(codec.entropy_mode.to_string()),
    );
    summary.insert("bundle_width".into(), json::Value::from(codec.bundle_width));
    summary.insert(
        "bundle_accel".into(),
        json::Value::from(bundle_accel_label(codec.bundle_accel)),
    );
    let gpu_available = norito::streaming::BUNDLED_RANS_GPU_BUILD_AVAILABLE;
    let accel_allowed = match codec.bundle_accel {
        actual::BundleAcceleration::Gpu => gpu_available,
        _ => true,
    };
    summary.insert(
        "gpu_build_available".into(),
        json::Value::from(gpu_available),
    );
    summary.insert(
        "bundle_accel_allowed".into(),
        json::Value::from(accel_allowed),
    );
    summary.insert("tables".into(), json::Value::Object(tables_json));

    Ok(json::Value::Object(summary))
}

fn bundle_accel_label(accel: actual::BundleAcceleration) -> &'static str {
    match accel {
        actual::BundleAcceleration::None => "none",
        actual::BundleAcceleration::CpuSimd => "cpu_simd",
        actual::BundleAcceleration::Gpu => "gpu",
    }
}

#[derive(Serialize, JsonSerialize)]
struct ContextRemapRow {
    original: u16,
    remapped: Option<u16>,
    bundles: u64,
    total_bits: u64,
    dominant_symbol: Option<SymbolCount>,
}

#[derive(Serialize, JsonSerialize)]
struct SymbolCount {
    symbol: u8,
    count: u64,
}

#[derive(Serialize, JsonSerialize)]
struct ContextRemapReport {
    input: String,
    total_contexts: usize,
    escape_context: u16,
    kept: Vec<ContextRemapRow>,
    dropped: Vec<ContextRemapRow>,
}

const CONTEXT_SYMBOL_CAP: usize = 1 << 4;

fn streaming_context_remap(options: StreamingContextRemapOptions) -> Result<(), Box<dyn Error>> {
    let raw = fs::read_to_string(&options.input)?;
    let telemetry: Value = norito::json::from_str(&raw)?;
    let Some(raw_contexts) = telemetry
        .get("context_frequencies")
        .and_then(Value::as_array)
    else {
        return Err("context_frequencies missing from telemetry".into());
    };
    let mut contexts: Vec<ContextFrequency> = raw_contexts
        .iter()
        .filter_map(parse_context_frequency)
        .collect();
    if contexts.is_empty() {
        return Err("context_frequencies empty or unparsable".into());
    }
    contexts.sort_by(|a, b| {
        b.bundles
            .cmp(&a.bundles)
            .then_with(|| b.total_bits.cmp(&a.total_bits))
    });
    let keep = options.top.min(contexts.len());
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for (idx, ctx) in contexts.iter().enumerate() {
        let remapped = (idx < keep).then_some(idx as u16);
        let row = ContextRemapRow::from_frequency(ctx, remapped);
        if idx < keep {
            kept.push(row);
        } else {
            dropped.push(row);
        }
    }
    let report = ContextRemapReport {
        input: options.input.display().to_string(),
        total_contexts: contexts.len(),
        escape_context: u16::MAX,
        kept,
        dropped,
    };
    let value = json::to_value(&report)?;
    write_json_output(&value, options.output)
}

impl ContextRemapRow {
    fn from_frequency(freq: &ContextFrequency, remapped: Option<u16>) -> Self {
        let dominant_symbol = dominant_symbol(&freq.symbol_counts);
        Self {
            original: freq.context.0,
            remapped,
            bundles: freq.bundles,
            total_bits: freq.total_bits,
            dominant_symbol,
        }
    }
}

fn dominant_symbol(counts: &[u64]) -> Option<SymbolCount> {
    counts
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, count)| *count > 0)
        .max_by(
            |(a_idx, a_count), (b_idx, b_count)| match b_count.cmp(a_count) {
                Ordering::Equal => a_idx.cmp(b_idx),
                other => other,
            },
        )
        .map(|(symbol, count)| SymbolCount {
            symbol: symbol as u8,
            count,
        })
}

fn parse_context_frequency(entry: &Value) -> Option<ContextFrequency> {
    let obj = entry.as_object()?;
    let context = obj.get("context")?.as_u64()? as u16;
    let bundles = obj.get("bundles").and_then(Value::as_u64).unwrap_or(0);
    let total_bits = obj.get("total_bits").and_then(Value::as_u64).unwrap_or(0);
    let mut symbol_counts = [0u64; CONTEXT_SYMBOL_CAP];
    if let Some(array) = obj.get("symbol_counts").and_then(Value::as_array) {
        for (idx, value) in array.iter().enumerate().take(CONTEXT_SYMBOL_CAP) {
            symbol_counts[idx] = value.as_u64().unwrap_or(0);
        }
    }
    Some(ContextFrequency {
        context: BundleContextId::new(context),
        bundles,
        total_bits,
        symbol_counts,
    })
}

fn entrypoint() -> Result<(), Box<dyn Error>> {
    let command = parse_command(env::args())?;
    match command {
        CommandKind::OpenApi {
            outputs,
            allow_stub,
            manifest,
            signing_key,
        } => generate_openapi(outputs, allow_stub, manifest, signing_key)?,
        CommandKind::OpenApiVerify {
            spec,
            manifest,
            allow_unsigned,
            allowed_signers,
        } => verify_openapi_manifest(&spec, &manifest, allow_unsigned, Some(&allowed_signers))?,
        CommandKind::AddressManifestVerify { options } => {
            address_manifest::verify_manifest_bundle(&options)?;
        }
        CommandKind::AddressVectors { target, verify } => match (target, verify) {
            (JsonTarget::Stdout, true) => {
                return Err(
                    "--verify is not supported when writing address vectors to stdout".into(),
                );
            }
            (JsonTarget::Stdout, false) => {
                write_address_vectors(JsonTarget::Stdout)?;
            }
            (JsonTarget::File(path), false) => {
                write_address_vectors(JsonTarget::File(path))?;
            }
            (JsonTarget::File(path), true) => {
                verify_address_vectors(&path)?;
            }
        },
        CommandKind::AddressLocalGate {
            input,
            window_days,
            json_out,
            check_collisions,
        } => address_gate::run_local_gate(address_gate::LocalGateOptions {
            input,
            window_days,
            json_out,
            check_collisions,
        })?,
        CommandKind::ZkVoteTallyBundle {
            output,
            verify,
            print_hashes,
            summary_target,
            attestation_target,
        } => generate_vote_tally_bundle(
            output,
            verify,
            print_hashes,
            summary_target,
            attestation_target,
        )?,
        CommandKind::SoranetFixtures { output, verify } => {
            if verify {
                soranet::verify_fixtures(&output)?;
            } else {
                soranet::generate_capability_fixtures(&output)?;
            }
        }
        CommandKind::SoranetChaosKit { options } => {
            let outcome = soranet_chaos::write_chaos_kit(options)?;
            println!(
                "soranet-chaos-kit wrote plan={} markdown={} ({} scripts, log={})",
                outcome.plan_json.display(),
                outcome.plan_markdown.display(),
                outcome.scripts.len(),
                outcome.log_path.display()
            );
        }
        CommandKind::SoranetChaosReport { options } => {
            let summary = soranet_chaos::summarize_log(options)?;
            println!(
                "soranet-chaos-report processed {} (scenarios={})",
                summary.log_path.display(),
                summary.scenarios.len()
            );
        }
        CommandKind::DocsPreview(command) => {
            docs_preview::run(command)?;
        }
        CommandKind::ComputeSloReport { options } => {
            compute::run_slo_report(options)?;
        }
        CommandKind::ComputeFixtures { options } => {
            compute::write_fixtures(options)?;
        }
        CommandKind::DaThreatModelReport {
            output,
            seed,
            config_path,
        } => {
            da::generate_threat_model_report(da::ThreatModelReportOptions {
                output,
                seed,
                config_path,
            })?;
        }
        CommandKind::DaReplicationAudit { options } => {
            da::run_replication_audit(options)?;
        }
        CommandKind::DaCommitmentReconcile { options } => {
            da::run_commitment_reconciliation(options)?;
        }
        CommandKind::DaPrivilegeAudit { options } => {
            da::run_privilege_audit(options)?;
        }
        CommandKind::DaProofBench { options } => {
            let report = da::run_proof_bench(options)?;
            println!(
                "da-proof-bench: iterations={} proofs/run={} avg_total_ms={:.2} recommended_budget_ms={}",
                report.iterations,
                report.stats.proof_count,
                report.stats.average_total_ms,
                report.stats.recommended_budget_ms
            );
        }
        CommandKind::ConfigDebug { config } => {
            run_config_debug(&config)?;
        }
        CommandKind::MochiBundle {
            output,
            profile,
            archive,
            kagami,
            matrix,
            smoke,
            stage,
        } => {
            let result = mochi::bundle_mochi(&output, &profile, archive, kagami.as_deref())?;
            let smoke_passed = if smoke {
                mochi::run_bundle_smoke(&result)?;
                true
            } else {
                false
            };
            if let Some(matrix_path) = matrix {
                mochi::update_bundle_matrix(&result, &matrix_path, smoke_passed)?;
            }
            if let Some(stage_root) = stage {
                mochi::stage_bundle(&result, &stage_root)?;
            }
        }
        CommandKind::KagamiProfiles { options } => {
            kagami_profiles::generate(options)?;
        }
        CommandKind::SorafsAdmissionFixtures { output } => {
            sorafs::write_admission_fixtures(&output)?;
        }
        CommandKind::SorafsGatewayFixtures { output, verify } => {
            if verify {
                sorafs::verify_gateway_fixtures(&output)?;
            } else {
                sorafs::write_gateway_fixtures(&output)?;
            }
        }
        CommandKind::SorafsFetchFixture { options } => {
            sorafs::fetch_fixture(*options)?;
        }
        CommandKind::SorafsPinFixtures { output } => {
            sorafs::write_pin_registry_fixture(output)?;
        }
        CommandKind::SorafsAdoptionCheck { options, report } => {
            let adoption = sorafs::run_adoption_check(options)?;
            let mut message = format!(
                "sorafs adoption check: validated {} scoreboard(s) (min eligible per file = {})",
                adoption.total_evaluated, adoption.min_providers_required
            );
            if adoption.single_source_override_used {
                message.push_str(" (single-source override accepted)");
            }
            println!("{message}");
            if let Some(path) = report {
                let rendered = serde_json::to_string_pretty(&adoption)?;
                fs::write(&path, rendered)?;
                println!("sorafs adoption check report written to {}", path.display());
            }
        }
        CommandKind::SnsPortalStub { options } => {
            sns::write_portal_stub(options)?;
        }
        CommandKind::SorafsScoreboardDiff { options, report } => {
            let diff = sorafs::run_scoreboard_diff(options)?;
            sorafs::print_scoreboard_diff(&diff);
            if let Some(path) = report {
                let rendered = serde_json::to_string_pretty(&diff)?;
                fs::write(&path, rendered)?;
                println!(
                    "sorafs scoreboard diff report written to {}",
                    path.display()
                );
            }
        }
        CommandKind::SorafsTaikaiCacheBundle { options } => {
            sorafs::run_taikai_cache_bundle(*options)?;
        }
        CommandKind::SorafsBurnInCheck { options, output } => {
            let summary = sorafs::run_burn_in_check(options)?;
            let rendered = serde_json::to_string_pretty(&summary)?;
            if let Some(path) = output {
                fs::write(path, rendered)?;
            } else {
                println!("{rendered}");
            }
        }
        CommandKind::SorafsReserveMatrix { options, target } => {
            let value = sorafs::reserve_matrix_report(options)?;
            write_json_output(&value, target)?;
        }
        CommandKind::SorafsGatewayAttest { options } => {
            sorafs::generate_gateway_attestation(*options)?;
        }
        CommandKind::SorafsGatewayProbe { options } => {
            sorafs::run_gateway_probe(*options)?;
        }
        CommandKind::SorafsGatewayCli { command } => {
            sorafs::run_gateway_cli(*command)?;
        }
        CommandKind::SoranetPopBundle { options } => {
            let outcome = soranet_pop::build_pop_bundle(*options)?;
            let (pop_name, image_tag) = outcome.summary();
            println!(
                "soranet-pop-bundle wrote {} (pop={}, image_tag={})",
                outcome.manifest_path.display(),
                pop_name,
                image_tag
            );
        }
        CommandKind::SoranetPopCtl { options } => {
            let outcome = soranet_pop::build_pop_bundle(*options)?;
            let (pop_name, image_tag) = outcome.summary();
            println!(
                "soranet-popctl wrote {} (pop={}, image_tag={})",
                outcome.manifest_path.display(),
                pop_name,
                image_tag
            );
        }
        CommandKind::SoranetPopTemplate {
            input,
            output,
            resolver_config,
            edns_out,
            edns_resolver,
            ds_out,
            edns_tool,
        } => {
            soranet_pop::render_frr_config(&input, &output)?;
            if let Some(config_path) = resolver_config {
                soranet_pop::render_resolver_config(&input, &config_path)?;
            }
            if let Some(ds_path) = ds_out {
                soranet_pop::run_ds_validation(&ds_path)?;
            }
            if let Some(edns_path) = edns_out {
                let resolver = edns_resolver.as_deref().unwrap_or("127.0.0.1");
                soranet_pop::run_edns_matrix(resolver, &edns_path, edns_tool.as_deref())?;
            }
        }
        CommandKind::SoranetPopValidate { input, roa, output } => {
            let report = soranet_pop::validate_pop_descriptor(&input, roa.as_deref())?;
            let value = report.to_value();
            write_json_output(&value, output)?;
        }
        CommandKind::SoranetPopPolicyReport { options } => {
            let outcome = soranet_pop::build_pop_policy_harness(*options)?;
            println!(
                "soranet-pop-policy-report wrote {}",
                outcome.report_path.display()
            );
        }
        CommandKind::SoranetPopPlan {
            input,
            roa,
            frr_out,
            report_out,
        } => {
            soranet_pop::render_frr_config(&input, &frr_out)?;
            let report = soranet_pop::validate_pop_descriptor(&input, roa.as_deref())?;
            let value = report.to_value();
            write_json_output(&value, report_out)?;
            println!("soranet-pop-plan wrote FRR config to {}", frr_out.display());
        }
        CommandKind::SoranetGatewayM0 { options } => {
            let outcome = soranet_gateway::write_gateway_m0_pack(options)?;
            println!(
                "soranet-gateway-m0 wrote {}",
                outcome.summary_path.display()
            );
        }
        CommandKind::SoranetGatewayM1 { options } => {
            let outcome = soranet_gateway_m1::run_gateway_m1(options)?;
            println!(
                "soranet-gateway-m1 wrote {}",
                outcome.summary_path.display()
            );
        }
        CommandKind::SoranetGatewayM2 { options } => {
            let outcome = soranet_gateway_m2::run_gateway_m2(options)?;
            println!(
                "soranet-gateway-m2 wrote {}",
                outcome.summary_path.display()
            );
        }
        CommandKind::SoranetGatewayM3 { options } => {
            let outcome = soranet_gateway_m3::run_gateway_m3(options)?;
            println!(
                "soranet-gateway-m3 wrote {} (markdown {})",
                outcome.summary_json.display(),
                outcome.summary_markdown.display()
            );
        }
        CommandKind::SoranetGatewayOpsM0 { options } => {
            let outcome = soranet_gateway_ops::write_gateway_ops_pack(options)?;
            println!(
                "soranet-gateway-ops-m0 wrote {}",
                outcome.summary_path.display()
            );
        }
        CommandKind::SoranetGatewayPq { options } => {
            let outcome = soranet_gateway_pq::run_gateway_pq_readiness(options)?;
            println!(
                "soranet-gateway-pq wrote json={} markdown={}",
                outcome.summary_json.display(),
                outcome.summary_markdown.display()
            );
        }
        CommandKind::SoranetBugBounty { options } => {
            let outcome = soranet_bug_bounty::generate_bug_bounty_pack(options)?;
            println!(
                "soranet-bug-bounty wrote summary={} triage={}",
                outcome.summary_path.display(),
                outcome.triage_checklist_path.display()
            );
        }
        CommandKind::SoranetGatewayOpsM1 { options } => {
            let outcome = soranet_gateway_ops::write_gateway_ops_federated_pack(options)?;
            println!(
                "soranet-gateway-ops-m1 wrote {}",
                outcome.summary_path.display()
            );
        }
        CommandKind::SoranetGatewayChaos { options } => {
            let outcome = soranet_gateway_chaos::run(options)?;
            println!(
                "soranet-gateway-chaos wrote plan={} report={} markdown={}",
                outcome.plan_path.display(),
                outcome.report_path.display(),
                outcome.markdown_path.display()
            );
        }
        CommandKind::SoranetGatewayHardening { options } => {
            let outcome = soranet_gateway_hardening::run_gateway_hardening(options)?;
            println!(
                "soranet-gateway-hardening wrote json={} markdown={}",
                outcome.summary_json.display(),
                outcome.summary_markdown.display()
            );
        }
        CommandKind::SoranetGarController { options } => {
            let summary = soranet_gar_controller::run_gar_controller(options)?;
            println!(
                "soranet-gar-controller wrote events {} receipts {} metrics {} reconciliation {}",
                summary.events_path,
                summary.receipts_dir,
                summary.metrics_path,
                summary.reconciliation_path
            );
            println!(
                "audit log {} report: {}",
                summary.audit_log_path,
                summary
                    .markdown_report
                    .as_deref()
                    .unwrap_or("no report emitted")
            );
        }
        CommandKind::SoranetGarExport { options, output } => {
            let summary = gar::export_receipts(options)?;
            write_json_output(&summary, output)?;
        }
        CommandKind::SoranetGarBus { options, output } => {
            let summary = gar_bus::publish_policy(options)?;
            write_json_output(&summary, output)?;
        }
        CommandKind::SoranetGatewayBilling { options } => {
            let outcome = soranet_gateway_billing::run_billing(options)?;
            println!(
                "soranet-gateway-billing wrote invoice={} csv={} parquet={} ledger={} reconciliation={}",
                outcome.invoice_path.display(),
                outcome.csv_path.display(),
                outcome.parquet_path.display(),
                outcome.ledger_path.display(),
                outcome.reconciliation_report_path.display()
            );
        }
        CommandKind::SoranetGatewayBillingM0 { options } => {
            let outcome = soranet_billing::write_billing_m0_pack(options)?;
            println!(
                "soranet-gateway-billing-m0 wrote {}",
                outcome.summary_path.display()
            );
        }
        CommandKind::TaikaiAnchorBundle { options } => {
            taikai_anchor::run_anchor_bundle(options)?;
        }
        CommandKind::TaikaiRptVerify { options } => {
            taikai::run_rpt_verify(options)?;
        }
        CommandKind::SoranetPrivacyReport {
            options,
            json_out,
            max_bucket_records,
            suppression_ratio_budget,
        } => {
            let report = soranet_privacy::run_privacy_report(&options)?;
            soranet_privacy::print_summary(&report, max_bucket_records);
            if let Some(target) = json_out {
                soranet_privacy::write_report_json(&report, target, max_bucket_records)?;
            }
            if let Some(threshold) = suppression_ratio_budget {
                match report.suppression_ratio() {
                    Some(ratio) => {
                        if ratio > threshold {
                            return Err(format!(
                                "suppression ratio {:.2}% exceeded threshold {:.2}%",
                                ratio * 100.0,
                                threshold * 100.0
                            )
                            .into());
                        }
                    }
                    None => {
                        return Err(
                            "no completed buckets available; rerun after relays emit privacy metrics"
                                .into(),
                        );
                    }
                }
            }
        }
        CommandKind::SoranetConstantRateProfile {
            selection,
            format,
            include_tick_table,
            tick_values,
            json_out,
        } => {
            let options = soranet::ConstantRateProfileOptions {
                selection,
                include_tick_table,
                tick_values_ms: tick_values,
            };
            let report = soranet::build_constant_rate_report(&options)?;
            match format {
                soranet::ConstantRateOutputFormat::Table => {
                    let table = soranet::format_profile_table(&report.profiles);
                    print!("{table}");
                    if let Some(entries) = &report.tick_bandwidth {
                        println!();
                        let ticks = soranet::format_tick_table(entries);
                        print!("{ticks}");
                    }
                    if !report.profiles.is_empty() {
                        println!();
                        for summary in &report.profiles {
                            println!("{}: {}", summary.name, summary.description);
                        }
                    }
                }
                soranet::ConstantRateOutputFormat::Json => {
                    let rendered = serde_json::to_string_pretty(&report)?;
                    println!("{rendered}");
                }
                soranet::ConstantRateOutputFormat::Markdown => {
                    let table = soranet::format_profile_markdown_table(&report.profiles);
                    print!("{table}");
                    if let Some(entries) = &report.tick_bandwidth {
                        println!();
                        let ticks = soranet::format_tick_markdown_table(entries);
                        print!("{ticks}");
                    }
                    if !report.profiles.is_empty() {
                        println!();
                        for summary in &report.profiles {
                            println!("* `{}` – {}", summary.name, summary.description);
                        }
                    }
                }
            }
            if let Some(target) = json_out {
                let mut serialized = serde_json::to_string_pretty(&report)?;
                serialized.push('\n');
                match target {
                    JsonTarget::Stdout => {
                        print!("{serialized}");
                    }
                    JsonTarget::File(path) => {
                        if let Some(parent) = path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::write(path, serialized)?;
                    }
                }
            }
        }
        CommandKind::NexusLaneMaintenance { options } => {
            let report = nexus_lane_maintenance::run(options)?;
            if let Some(action) = report.compacted.last() {
                println!(
                    "archived {} retired segment(s); last move: {} -> {}",
                    report.compacted.len(),
                    action.from,
                    action.to
                );
            } else {
                println!(
                    "surveyed {} lane(s); retired segments: {}",
                    report.active.len(),
                    report.retired.len()
                );
            }
        }
        CommandKind::NexusLaneAudit { options } => {
            nexus::run_lane_audit(&options)?;
        }
        CommandKind::NexusFixtures { output, verify } => {
            if verify {
                nexus::verify_lane_commitment_fixtures(&output)?;
            } else {
                nexus::write_lane_commitment_fixtures(&output)?;
            }
        }
        CommandKind::SoranetTestnetKit { output } => {
            generate_testnet_kit(output)?;
        }
        CommandKind::SoranetTestnetMetrics { input, output } => {
            evaluate_testnet_metrics(input, output)?;
        }
        CommandKind::SoranetTestnetFeed { options } => {
            generate_verification_feed(*options)?;
        }
        CommandKind::SoranetTestnetDrillBundle { options } => {
            generate_drill_bundle(*options)?;
        }
        CommandKind::FastpqBenchManifest { options } => {
            fastpq::write_bench_manifest(*options)?;
        }
        CommandKind::FastpqStageProfile { options } => {
            fastpq::run_stage_profile(&options)?;
        }
        CommandKind::FastpqCudaSuite { options } => {
            let result = fastpq::run_cuda_suite(&options)?;
            eprintln!("fastpq-cuda-suite: raw {}", result.raw_output.display());
            if let Some(wrapped) = result.wrapped_output {
                eprintln!("fastpq-cuda-suite: wrapped {}", wrapped.display());
            }
            eprintln!(
                "fastpq-cuda-suite: plan {}{}",
                result.summary.display(),
                if result.dry_run { " (dry-run)" } else { "" }
            );
        }
        CommandKind::SoradnsDirectoryRelease { options } => {
            soradns::release_directory(*options)?;
        }
        CommandKind::SoradnsHosts {
            names,
            json,
            verify,
        } => {
            let summaries = soradns::derive_host_summaries(&names)?;
            if let Some(target) = json {
                let mut rendered = serde_json::to_string_pretty(&summaries)?;
                rendered.push('\n');
                match target {
                    JsonTarget::Stdout => {
                        print!("{rendered}");
                    }
                    JsonTarget::File(path) => {
                        fs::write(path, rendered)?;
                    }
                }
            } else {
                for summary in &summaries {
                    println!("{}", summary.name);
                    println!("  normalized : {}", summary.normalized_name);
                    println!("  canonical  : {}", summary.canonical_host);
                    println!("  pretty     : {}", summary.pretty_host);
                    println!("  hash label : {}", summary.canonical_label);
                    println!("  wildcard   : {}", summary.canonical_wildcard);
                    println!();
                }
            }
            if !verify.is_empty() {
                soradns::verify_host_patterns(&summaries, &verify)?;
            }
        }
        CommandKind::SoradnsBindingTemplate {
            options,
            json,
            headers_out,
        } => {
            let render = soradns::build_binding_template(&options)?;
            let mut rendered = serde_json::to_string_pretty(&render.payload)?;
            rendered.push('\n');
            match json {
                Some(JsonTarget::Stdout) | None => {
                    print!("{rendered}");
                }
                Some(JsonTarget::File(path)) => {
                    fs::write(path, rendered)?;
                }
            }
            if let Some(path) = headers_out {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&path, render.headers_template.as_bytes())?;
                println!("headers written to {}", path.display());
            } else {
                println!("{}", render.headers_template);
            }
        }
        CommandKind::SoradnsGarTemplate { options, output } => {
            let payload = soradns::build_gar_template(&options)?;
            let mut rendered = serde_json::to_string_pretty(&payload)?;
            rendered.push('\n');
            match output {
                Some(JsonTarget::Stdout) | None => {
                    print!("{rendered}");
                }
                Some(JsonTarget::File(path)) => {
                    fs::write(path, rendered)?;
                }
            }
        }
        CommandKind::SoradnsAcmePlan { options, output } => {
            let plan = soradns::build_acme_plan(&options)?;
            let mut rendered = serde_json::to_string_pretty(&plan)?;
            rendered.push('\n');
            match output {
                JsonTarget::Stdout => {
                    print!("{rendered}");
                }
                JsonTarget::File(path) => {
                    fs::write(path, rendered)?;
                }
            }
        }
        CommandKind::SoradnsCachePlan { options, output } => {
            let plan = soradns::build_cache_invalidation_plan(&options)?;
            let mut rendered = serde_json::to_string_pretty(&plan)?;
            rendered.push('\n');
            match output {
                JsonTarget::Stdout => {
                    print!("{rendered}");
                }
                JsonTarget::File(path) => {
                    fs::write(path, rendered)?;
                }
            }
        }
        CommandKind::SoradnsRoutePlan { options, output } => {
            let plan = soradns::build_route_plan(&options)?;
            let mut rendered = serde_json::to_string_pretty(&plan)?;
            rendered.push('\n');
            match output {
                JsonTarget::Stdout => {
                    print!("{rendered}");
                }
                JsonTarget::File(path) => {
                    fs::write(path, rendered)?;
                }
            }
        }
        CommandKind::SoradnsVerifyBinding {
            binding,
            alias,
            content_cid,
            hostname,
            proof_status,
            manifest,
        } => {
            let summary = soradns::verify_gateway_binding(
                &binding,
                &soradns::GatewayBindingExpectations {
                    alias,
                    content_cid,
                    hostname,
                    proof_status,
                    manifest_path: manifest,
                },
            )?;
            let alias_display = summary.alias.as_deref().unwrap_or("<no-alias>");
            let status_display = summary.proof_status.as_deref().unwrap_or("<unknown>");
            println!(
                "[soradns] verified gateway binding {alias_display} -> {} (canonical_label={}, canonical_host={}, status={status_display})",
                summary.content_cid, summary.canonical_label, summary.canonical_host
            );
        }
        CommandKind::SoradnsVerifyGar { options } => {
            let summary = soradns::verify_gar_payload(&options.gar_path, &options.expectations)?;
            if let Some(target) = &options.json_out {
                let mut rendered = serde_json::to_string_pretty(&summary.to_json_value())?;
                rendered.push('\n');
                match target {
                    JsonTarget::Stdout => {
                        print!("{rendered}");
                    }
                    JsonTarget::File(path) => {
                        if let Some(parent) = path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::write(path, rendered)?;
                    }
                }
            } else {
                println!(
                    "[soradns] verified GAR {} -> {} (canonical_label={}, hosts={}, telemetry_labels={}, valid_from={}, valid_until={})",
                    summary.normalized_name,
                    summary.manifest_cid,
                    summary.canonical_label,
                    summary.host_patterns.len(),
                    summary.telemetry_labels.len(),
                    summary.valid_from_epoch,
                    summary
                        .valid_until_epoch
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                );
            }
        }
        CommandKind::SoranetRolloutPlan { options } => {
            soranet_rollout::generate_plan(options)?;
        }
        CommandKind::SoranetRolloutCapture { options } => {
            soranet_rollout::capture_rollout(options)?;
        }
        CommandKind::SmWycheproofSync(options) => sm::sync_wycheproof(options)?,
        CommandKind::SmOperatorSnippet { options } => sm::generate_sm_operator_snippet(options)?,
        CommandKind::IsoBridgeLint { options } => lint_iso_bridge(options)?,
        CommandKind::NoritoRpcVerify { json_out } => {
            run_verify(json_out).map_err(|err| -> Box<dyn Error> { err.into() })?;
        }
        CommandKind::NoritoRpcFixtures { options } => {
            run_norito_rpc_fixtures(options)?;
        }
        CommandKind::CodecRansTables { options } => {
            codec::execute_rans_tables(options)?;
        }
        CommandKind::VerifyRansTables { paths } => {
            codec::verify_tables(&paths)?;
        }
        CommandKind::StreamingBundleCheck { options } => {
            streaming_bundle_check(options)?;
        }
        CommandKind::StreamingEntropyBench { options, output } => {
            let summary = streaming_bench::run_entropy_bench(options)?;
            write_json_output(&summary, output)?;
        }
        CommandKind::StreamingDecode { options, output } => {
            let summary = streaming_bench::run_streaming_decode(options)?;
            if let Some(target) = output {
                write_json_output(&summary, target)?;
            }
        }
        CommandKind::Stage1Bench { options } => {
            stage1_bench::run_stage1_bench(options)?;
        }
        CommandKind::I3BenchSuite { options } => {
            let report = i3_bench_suite::run_i3_bench_suite(options.clone())?;
            println!(
                "i3 bench suite wrote {} scenarios to {}",
                report.scenarios.len(),
                options.json_out.display()
            );
        }
        CommandKind::I3SloHarness { options } => {
            i3_slo_harness::run_i3_slo_harness(options)?;
        }
        CommandKind::PoseidonBench { options } => {
            poseidon_bench::run_poseidon_bench(options)?;
        }
        CommandKind::StreamingContextRemap { options } => {
            streaming_context_remap(options)?;
        }
        CommandKind::SpaceDirectoryEncode { input, output } => {
            encode_space_directory_manifest(&input, &output)?;
        }
        CommandKind::AccelerationState { format } => run_acceleration_state(format)?,
        CommandKind::SnsScorecard(options) => {
            sns::generate_scorecard(options)?;
        }
        CommandKind::SnsAnnex(options) => {
            sns::generate_annex(options)?;
        }
        CommandKind::SnsCatalogVerify(options) => {
            sns::verify_catalog(options)?;
        }
        CommandKind::MinistryTransparency(command) => {
            ministry::run(command)?;
        }
        CommandKind::MinistryAgenda(command) => {
            ministry_agenda::run(command)?;
        }
        CommandKind::MinistryPanel(command) => {
            ministry_panel::run(command)?;
        }
        CommandKind::MinistryJury(command) => {
            ministry_jury::run(command)?;
        }
        CommandKind::OfflinePosProvision(options) => {
            offline_pos_provision::run(options)?;
        }
        CommandKind::OfflinePosVerify(options) => {
            offline_pos_verify::run(options)?;
        }
        CommandKind::OfflineProvision(options) => {
            offline_provision::run(options)?;
        }
        CommandKind::OfflineTopup(options) => {
            offline_topup::run(options)?;
        }
        CommandKind::OfflineBundle(options) => {
            offline_bundle::run(options)?;
        }
        CommandKind::OfflinePoseidonFixtures(options) => {
            offline_poseidon::generate(options)?;
        }
        CommandKind::Help => {
            print_usage();
        }
    }
    Ok(())
}

fn parse_command<I>(mut args: I) -> Result<CommandKind, Box<dyn Error>>
where
    I: Iterator<Item = String>,
{
    // Skip binary name
    let _ = args.next();
    let Some(cmd) = args.next() else {
        print_usage();
        return Err("missing command".into());
    };

    match cmd.as_str() {
        "-h" | "--help" => Ok(CommandKind::Help),
        "offline-pos-provision" => {
            let mut spec_path: Option<PathBuf> = None;
            let mut output_root: Option<PathBuf> = None;
            let mut operator_key: Option<String> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--spec" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --spec".into());
                        };
                        spec_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        output_root = Some(normalize_path(Path::new(&path))?);
                    }
                    "--operator-key" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --operator-key".into());
                        };
                        operator_key = Some(value);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for offline-pos-provision: {flag}").into()
                        );
                    }
                }
            }
            let spec_path = spec_path
                .ok_or_else(|| "offline-pos-provision requires --spec <path>".to_string())?;
            let output_root = output_root
                .map(Ok)
                .unwrap_or_else(|| normalize_path(Path::new("artifacts/offline_pos_provision")))?;
            Ok(CommandKind::OfflinePosProvision(
                OfflinePosProvisionOptions {
                    spec_path,
                    output_root,
                    operator_key_override: operator_key,
                },
            ))
        }
        "offline-provision" => {
            let mut spec_path: Option<PathBuf> = None;
            let mut output_root: Option<PathBuf> = None;
            let mut inspector_key: Option<String> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--spec" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --spec".into());
                        };
                        spec_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        output_root = Some(normalize_path(Path::new(&path))?);
                    }
                    "--inspector-key" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --inspector-key".into());
                        };
                        inspector_key = Some(value);
                    }
                    flag => {
                        return Err(format!("unknown flag for offline-provision: {flag}").into());
                    }
                }
            }
            let spec_path =
                spec_path.ok_or_else(|| "offline-provision requires --spec <path>".to_string())?;
            let output_root = output_root
                .map(Ok)
                .unwrap_or_else(|| normalize_path(Path::new("artifacts/offline_provision")))?;
            Ok(CommandKind::OfflineProvision(OfflineProvisionOptions {
                spec_path,
                output_root,
                inspector_key_override: inspector_key,
            }))
        }
        "offline-pos-verify" => {
            let mut bundle_path: Option<PathBuf> = None;
            let mut manifest_path: Option<PathBuf> = None;
            let mut allowance_summaries: Vec<PathBuf> = Vec::new();
            let mut policy_path: Option<PathBuf> = None;
            let mut audit_log_path: Option<PathBuf> = None;
            let mut api_snapshot: Option<PathBuf> = None;
            let mut now_ms_override: Option<u64> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--bundle" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --bundle".into());
                        };
                        bundle_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--manifest" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --manifest".into());
                        };
                        manifest_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--allowance-summary" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --allowance-summary".into());
                        };
                        allowance_summaries.push(normalize_path(Path::new(&path))?);
                    }
                    "--policy" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --policy".into());
                        };
                        policy_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--audit-log" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --audit-log".into());
                        };
                        audit_log_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--api-snapshot" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --api-snapshot".into());
                        };
                        api_snapshot = Some(normalize_path(Path::new(&path))?);
                    }
                    "--now-ms" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --now-ms".into());
                        };
                        let parsed = value.parse::<u64>().map_err(|err| {
                            format!("invalid value for --now-ms `{value}`: {err}")
                        })?;
                        now_ms_override = Some(parsed);
                    }
                    flag => {
                        return Err(format!("unknown flag for offline-pos-verify: {flag}").into());
                    }
                }
            }
            if bundle_path.is_none() && manifest_path.is_none() && allowance_summaries.is_empty() {
                return Err(
                    "offline-pos-verify requires --bundle, --manifest, or --allowance-summary"
                        .into(),
                );
            }
            if bundle_path.is_none() && api_snapshot.is_some() {
                return Err("--api-snapshot requires --bundle".into());
            }
            Ok(CommandKind::OfflinePosVerify(OfflinePosVerifyOptions {
                bundle_json: bundle_path,
                manifest_json: manifest_path,
                allowance_summaries,
                policy_path,
                audit_log_path,
                api_snapshot_path: api_snapshot,
                now_ms_override,
            }))
        }
        "offline-topup" => {
            let mut spec_path: Option<PathBuf> = None;
            let mut output_root: Option<PathBuf> = None;
            let mut operator_key: Option<String> = None;
            let mut register_config: Option<PathBuf> = None;
            let mut register_mode = RegisterMode::Blocking;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--spec" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --spec".into());
                        };
                        spec_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        output_root = Some(normalize_path(Path::new(&path))?);
                    }
                    "--operator-key" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --operator-key".into());
                        };
                        operator_key = Some(value);
                    }
                    "--register-config" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --register-config".into());
                        };
                        register_config = Some(normalize_path(Path::new(&path))?);
                    }
                    "--register-mode" => {
                        let Some(mode) = pending.next() else {
                            return Err("expected mode after --register-mode".into());
                        };
                        register_mode =
                            match mode.as_str() {
                                "blocking" => RegisterMode::Blocking,
                                "immediate" => RegisterMode::Immediate,
                                other => return Err(format!(
                                    "register mode must be `blocking` or `immediate` (got {other})"
                                )
                                .into()),
                            };
                    }
                    flag => {
                        return Err(format!("unknown flag for offline-topup: {flag}").into());
                    }
                }
            }
            let spec_path =
                spec_path.ok_or_else(|| "offline-topup requires --spec <path>".to_string())?;
            let output_root = output_root
                .map(Ok)
                .unwrap_or_else(|| normalize_path(Path::new("artifacts/offline_topup")))?;
            let register = register_config.map(|config_path| RegisterOptions {
                config_path,
                mode: register_mode,
            });
            Ok(CommandKind::OfflineTopup(OfflineTopupOptions {
                spec_path,
                output_root,
                operator_key_override: operator_key,
                register,
            }))
        }
        "offline-bundle" => {
            let mut spec_path: Option<PathBuf> = None;
            let mut output_root: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--spec" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --spec".into());
                        };
                        spec_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        output_root = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for offline-bundle: {flag}").into());
                    }
                }
            }
            let spec_path =
                spec_path.ok_or_else(|| "offline-bundle requires --spec <path>".to_string())?;
            let output_root = output_root
                .map(Ok)
                .unwrap_or_else(|| normalize_path(Path::new("artifacts/offline_bundle")))?;
            Ok(CommandKind::OfflineBundle(OfflineBundleOptions {
                spec_path,
                output_root,
            }))
        }
        "offline-poseidon-fixtures" => {
            let mut constants_path: Option<PathBuf> = None;
            let mut vectors_path: Option<PathBuf> = None;
            let mut domain_tag: Option<String> = None;
            let mut mirror_sdk = true;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--constants" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --constants".into());
                        };
                        constants_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--vectors" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --vectors".into());
                        };
                        vectors_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--tag" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --tag".into());
                        };
                        domain_tag = Some(value);
                    }
                    "--no-sdk-mirror" => {
                        mirror_sdk = false;
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for offline-poseidon-fixtures: {flag}").into(),
                        );
                    }
                }
            }
            Ok(CommandKind::OfflinePoseidonFixtures(
                offline_poseidon::OfflinePoseidonFixtureOptions::new(
                    constants_path,
                    vectors_path,
                    domain_tag,
                    mirror_sdk,
                ),
            ))
        }
        "openapi" => {
            let mut outputs = Vec::new();
            let mut allow_stub = false;
            let mut manifest_path: Option<PathBuf> = None;
            let mut signing_key: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "-o" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        outputs.push(normalize_path(Path::new(&path))?);
                    }
                    "--allow-stub" => allow_stub = true,
                    "--manifest" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --manifest".into());
                        };
                        manifest_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--sign" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --sign".into());
                        };
                        signing_key = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => return Err(format!("unknown flag for openapi: {flag}").into()),
                }
            }

            if outputs.is_empty() {
                outputs.push(default_openapi_path());
            }

            outputs.sort();
            outputs.dedup();

            Ok(CommandKind::OpenApi {
                outputs,
                allow_stub,
                manifest: manifest_path.unwrap_or_else(default_openapi_manifest_path),
                signing_key,
            })
        }
        "da-threat-model-report" => {
            let mut output: Option<JsonTarget> = None;
            let mut seed = da::DEFAULT_REPORT_SEED;
            let mut config_path: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out".into());
                        };
                        if path == "-" {
                            output = Some(JsonTarget::Stdout);
                        } else {
                            output = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    "--seed" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --seed".into());
                        };
                        seed = da::parse_seed(&value)?;
                    }
                    "--config" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --config".into());
                        };
                        config_path = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for da-threat-model-report: {flag}").into()
                        );
                    }
                }
            }
            let output = output.unwrap_or_else(|| JsonTarget::File(default_da_report_path()));
            Ok(CommandKind::DaThreatModelReport {
                output,
                seed,
                config_path,
            })
        }
        "da-replication-audit" => {
            let mut config: Option<PathBuf> = None;
            let mut manifests = Vec::new();
            let mut orders = Vec::new();
            let mut json_out: Option<JsonTarget> = None;
            let mut plan_out: Option<JsonTarget> = None;
            let mut allow_mismatch = false;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--config" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --config".into());
                        };
                        config = Some(normalize_path(Path::new(&path))?);
                    }
                    "--manifest" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --manifest".into());
                        };
                        manifests.push(normalize_path(Path::new(&path))?);
                    }
                    "--replication-order" | "--order" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --replication-order".into());
                        };
                        orders.push(normalize_path(Path::new(&path))?);
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    "--plan-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --plan-out".into());
                        };
                        if path == "-" {
                            plan_out = Some(JsonTarget::Stdout);
                        } else {
                            plan_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    "--allow-mismatch" => allow_mismatch = true,
                    flag => {
                        return Err(format!("unknown flag for da-replication-audit: {flag}").into());
                    }
                }
            }

            let config =
                config.ok_or("da-replication-audit requires --config <torii_config.toml>")?;
            if manifests.is_empty() {
                return Err("da-replication-audit requires at least one --manifest <path>".into());
            }
            let json_output =
                json_out.unwrap_or_else(|| JsonTarget::File(default_da_replication_audit_path()));
            Ok(CommandKind::DaReplicationAudit {
                options: da::ReplicationAuditOptions {
                    config,
                    manifests,
                    replication_orders: orders,
                    json_output: Some(json_output),
                    plan_output: plan_out,
                    allow_mismatch,
                },
            })
        }
        "da-commitment-reconcile" => {
            let mut receipts = Vec::new();
            let mut blocks = Vec::new();
            let mut json_out: Option<JsonTarget> = None;
            let mut allow_unexpected = false;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--receipt" | "--receipts" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --receipt/--receipts".into());
                        };
                        receipts.push(normalize_path(Path::new(&path))?);
                    }
                    "--block" | "--commitment" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --block".into());
                        };
                        blocks.push(normalize_path(Path::new(&path))?);
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    "--allow-unexpected" => {
                        allow_unexpected = true;
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for da-commitment-reconcile: {flag}").into()
                        );
                    }
                }
            }
            if receipts.is_empty() {
                return Err(
                    "da-commitment-reconcile requires at least one --receipt <path>".into(),
                );
            }
            if blocks.is_empty() {
                return Err("da-commitment-reconcile requires at least one --block <path>".into());
            }
            let json_output = json_out
                .unwrap_or_else(|| JsonTarget::File(default_da_commitment_reconcile_path()));
            Ok(CommandKind::DaCommitmentReconcile {
                options: da::CommitmentReconcileOptions {
                    receipts,
                    blocks,
                    json_output: Some(json_output),
                    allow_unexpected_commitments: allow_unexpected,
                },
            })
        }
        "da-privilege-audit" => {
            let mut config: Option<PathBuf> = None;
            let mut extra_paths = Vec::new();
            let mut json_out: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--config" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --config".into());
                        };
                        config = Some(normalize_path(Path::new(&path))?);
                    }
                    "--extra-path" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --extra-path".into());
                        };
                        extra_paths.push(normalize_path(Path::new(&path))?);
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    flag => {
                        return Err(format!("unknown flag for da-privilege-audit: {flag}").into());
                    }
                }
            }
            let config =
                config.ok_or("da-privilege-audit requires --config <torii_config.toml>")?;
            let json_output =
                json_out.unwrap_or_else(|| JsonTarget::File(default_da_privilege_audit_path()));
            Ok(CommandKind::DaPrivilegeAudit {
                options: da::PrivilegeAuditOptions {
                    config,
                    extra_paths,
                    json_output: Some(json_output),
                },
            })
        }
        "da-proof-bench" => {
            let mut manifest: Option<PathBuf> = None;
            let mut payload: Option<PathBuf> = None;
            let mut payload_bytes: Option<usize> = None;
            let mut sample_count: Option<usize> = None;
            let mut sample_seed: Option<u64> = None;
            let mut budget_ms: Option<u64> = None;
            let mut json_out: Option<JsonTarget> = None;
            let mut markdown_out: Option<PathBuf> = None;
            let mut iterations: usize = 5;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--manifest" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --manifest".into());
                        };
                        manifest = Some(normalize_path(Path::new(&path))?);
                    }
                    "--payload" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --payload".into());
                        };
                        payload = Some(normalize_path(Path::new(&path))?);
                    }
                    "--payload-bytes" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --payload-bytes".into());
                        };
                        payload_bytes = Some(
                            value
                                .parse::<usize>()
                                .map_err(|err| format!("invalid --payload-bytes: {err}"))?,
                        );
                    }
                    "--sample-count" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --sample-count".into());
                        };
                        sample_count = Some(
                            value
                                .parse::<usize>()
                                .map_err(|err| format!("invalid --sample-count: {err}"))?,
                        );
                    }
                    "--sample-seed" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --sample-seed".into());
                        };
                        sample_seed = Some(da::parse_seed(&value)?);
                    }
                    "--budget-ms" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --budget-ms".into());
                        };
                        budget_ms = Some(
                            value
                                .parse::<u64>()
                                .map_err(|err| format!("invalid --budget-ms: {err}"))?,
                        );
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    "--markdown-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --markdown-out".into());
                        };
                        if path == "-" {
                            markdown_out = Some(PathBuf::from("-"));
                        } else {
                            markdown_out = Some(normalize_path(Path::new(&path))?);
                        }
                    }
                    "--iterations" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --iterations".into());
                        };
                        iterations = value
                            .parse::<usize>()
                            .map_err(|err| format!("invalid --iterations: {err}"))?;
                    }
                    flag => {
                        return Err(format!("unknown flag for da-proof-bench: {flag}").into());
                    }
                }
            }

            let manifest = match manifest {
                Some(path) => path,
                None => normalize_path(&default_da_proof_manifest())?,
            };
            let payload = match payload {
                Some(path) => path,
                None => {
                    if payload_bytes.is_some() {
                        normalize_path(&default_da_proof_generated_payload())?
                    } else {
                        normalize_path(&default_da_proof_payload())?
                    }
                }
            };
            let json_output =
                json_out.unwrap_or_else(|| JsonTarget::File(default_da_proof_bench_json_path()));
            let markdown_output = markdown_out.unwrap_or_else(default_da_proof_bench_markdown_path);
            Ok(CommandKind::DaProofBench {
                options: da::ProofBenchOptions {
                    manifest,
                    payload,
                    payload_bytes,
                    sample_count,
                    sample_seed,
                    budget_ms,
                    json_output: Some(json_output),
                    markdown_output: Some(markdown_output),
                    iterations,
                },
            })
        }
        "config-debug" => {
            let mut config: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--config" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --config".into());
                        };
                        config = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for config-debug: {flag}").into());
                    }
                }
            }
            let config = config.ok_or("config-debug requires --config <path>")?;
            Ok(CommandKind::ConfigDebug { config })
        }
        "openapi-verify" => {
            let mut manifest_path: Option<PathBuf> = None;
            let mut spec_path: Option<PathBuf> = None;
            let mut allow_unsigned = false;
            let mut allowed_signers: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--manifest" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --manifest".into());
                        };
                        manifest_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--spec" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --spec".into());
                        };
                        spec_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--allow-unsigned" => allow_unsigned = true,
                    "--allowed-signers" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --allowed-signers".into());
                        };
                        allowed_signers = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for openapi-verify: {flag}").into());
                    }
                }
            }

            Ok(CommandKind::OpenApiVerify {
                spec: spec_path.unwrap_or_else(default_openapi_path),
                manifest: manifest_path.unwrap_or_else(default_openapi_manifest_path),
                allow_unsigned,
                allowed_signers: allowed_signers
                    .unwrap_or_else(default_openapi_allowed_signers_path),
            })
        }
        "address-manifest" => {
            let Some(subcommand) = args.next() else {
                return Err("address-manifest requires a subcommand (e.g. `verify`)".into());
            };
            match subcommand.as_str() {
                "verify" => {
                    let mut bundle: Option<PathBuf> = None;
                    let mut previous: Option<PathBuf> = None;
                    let mut pending = args.peekable();
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--bundle" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --bundle".into());
                                };
                                bundle = Some(normalize_path(Path::new(&path))?);
                            }
                            "--previous" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --previous".into());
                                };
                                previous = Some(normalize_path(Path::new(&path))?);
                            }
                            flag => {
                                return Err(format!(
                                    "unknown flag for address-manifest verify: {flag}"
                                )
                                .into());
                            }
                        }
                    }
                    let bundle = bundle
                        .ok_or("address-manifest verify requires --bundle <manifest-directory>")?;
                    Ok(CommandKind::AddressManifestVerify {
                        options: address_manifest::VerifyOptions {
                            bundle,
                            previous_bundle: previous,
                        },
                    })
                }
                other => Err(format!(
                    "unknown subcommand `{other}` for address-manifest; only `verify` is supported"
                )
                .into()),
            }
        }
        "docs-preview" => {
            let mut pending = args.peekable();
            let Some(subcommand) = pending.next() else {
                return Err("docs-preview requires a subcommand (summary)".into());
            };
            match subcommand.as_str() {
                "summary" => {
                    let mut log_path = docs_preview::default_log_path();
                    let mut filter_wave: Option<String> = None;
                    let mut output = docs_preview::SummaryOutput::Human;
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--log" => {
                                let Some(path) = pending.next() else {
                                    return Err("--log requires a path".into());
                                };
                                log_path = PathBuf::from(path);
                            }
                            "--wave" => {
                                let Some(label) = pending.next() else {
                                    return Err("--wave requires a label".into());
                                };
                                filter_wave = Some(label);
                            }
                            "--json" => {
                                let Some(target) = pending.next() else {
                                    return Err("--json requires a path or '-'".into());
                                };
                                if target == "-" {
                                    output = docs_preview::SummaryOutput::JsonStdout;
                                } else {
                                    output = docs_preview::SummaryOutput::JsonFile(PathBuf::from(
                                        target,
                                    ));
                                }
                            }
                            "--json-stdout" => {
                                output = docs_preview::SummaryOutput::JsonStdout;
                            }
                            "--text" => {
                                output = docs_preview::SummaryOutput::Human;
                            }
                            flag if flag.starts_with('-') => {
                                return Err(format!(
                                    "unknown option `{flag}` for docs-preview summary"
                                )
                                .into());
                            }
                            other => {
                                return Err(format!(
                                    "unexpected argument `{other}` for docs-preview summary"
                                )
                                .into());
                            }
                        }
                    }
                    Ok(CommandKind::DocsPreview(docs_preview::Command::Summary(
                        docs_preview::SummaryOptions {
                            log_path,
                            filter_wave,
                            output,
                        },
                    )))
                }
                other => Err(format!(
                    "unknown docs-preview subcommand `{other}` (expected summary)"
                )
                .into()),
            }
        }
        "compute" => {
            let Some(subcmd) = args.next() else {
                return Err("compute requires a subcommand (slo-report|fixtures)".into());
            };
            match subcmd.as_str() {
                "slo-report" => {
                    let mut manifest =
                        PathBuf::from("fixtures/compute/manifest_compute_payments.json");
                    let mut json_out = PathBuf::from("artifacts/compute/compute_slo_report.json");
                    let mut markdown_out =
                        Some(PathBuf::from("artifacts/compute/compute_slo_report.md"));
                    let mut samples = std::num::NonZeroUsize::new(24).expect("non-zero");
                    let mut pending = args.peekable();
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--manifest" => {
                                let Some(path) = pending.next() else {
                                    return Err("--manifest requires a path".into());
                                };
                                manifest = normalize_path(Path::new(&path))?;
                            }
                            "--json-out" => {
                                let Some(path) = pending.next() else {
                                    return Err("--json-out requires a path".into());
                                };
                                json_out = PathBuf::from(path);
                            }
                            "--markdown-out" => {
                                let Some(path) = pending.next() else {
                                    return Err("--markdown-out requires a path or '-'".into());
                                };
                                if path == "-" {
                                    markdown_out = None;
                                } else {
                                    markdown_out = Some(PathBuf::from(path));
                                }
                            }
                            "--samples" => {
                                let Some(val) = pending.next() else {
                                    return Err("--samples requires a positive integer".into());
                                };
                                let parsed: usize = val.parse().map_err(|_| {
                                    format!("invalid value `{val}` for --samples (usize expected)")
                                })?;
                                samples = std::num::NonZeroUsize::new(parsed)
                                    .ok_or("--samples must be > 0")?;
                            }
                            flag => {
                                return Err(format!(
                                    "unknown option `{flag}` for compute slo-report"
                                )
                                .into());
                            }
                        }
                    }
                    Ok(CommandKind::ComputeSloReport {
                        options: compute::ComputeSloReportOptions {
                            manifest,
                            json_out,
                            markdown_out,
                            samples,
                        },
                    })
                }
                "fixtures" => {
                    let mut output_dir = PathBuf::from("fixtures/compute/sdk_parity");
                    let mut pending = args.peekable();
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "-o" | "--out" | "--output" => {
                                let Some(path) = pending.next() else {
                                    return Err("--output requires a directory".into());
                                };
                                output_dir = PathBuf::from(path);
                            }
                            flag => {
                                return Err(format!(
                                    "unknown option `{flag}` for compute fixtures"
                                )
                                .into());
                            }
                        }
                    }
                    Ok(CommandKind::ComputeFixtures {
                        options: compute::ComputeFixtureOptions { output_dir },
                    })
                }
                other => Err(format!(
                    "unknown compute subcommand `{other}` (expected slo-report|fixtures)"
                )
                .into()),
            }
        }
        "address-vectors" => {
            let mut target: Option<JsonTarget> = None;
            let mut verify = false;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        target = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                    }
                    "--stdout" => target = Some(JsonTarget::Stdout),
                    "--verify" => verify = true,
                    flag => {
                        return Err(format!("unknown flag for address-vectors: {flag}").into());
                    }
                }
            }
            if verify && matches!(target, Some(JsonTarget::Stdout)) {
                return Err("--verify cannot be combined with --stdout for address-vectors".into());
            }
            let target = target.unwrap_or(JsonTarget::File(default_address_vectors_path()?));
            Ok(CommandKind::AddressVectors { target, verify })
        }
        "address-local8-gate" => {
            let mut input: Option<PathBuf> = None;
            let mut window_days: u64 = 30;
            let mut json_out: Option<JsonTarget> = None;
            let mut check_collisions = true;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--input" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --input".into());
                        };
                        input = Some(normalize_path(Path::new(&path))?);
                    }
                    "--window-days" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --window-days".into());
                        };
                        window_days = value.parse::<u64>().map_err(|err| {
                            format!("failed to parse --window-days `{value}`: {err}")
                        })?;
                        if window_days == 0 {
                            return Err("--window-days must be greater than zero".into());
                        }
                    }
                    "--json-out" => {
                        let Some(target) = pending.next() else {
                            return Err("expected path or '-' after --json-out".into());
                        };
                        if target == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&target))?));
                        }
                    }
                    "--skip-collisions" => {
                        check_collisions = false;
                    }
                    flag => {
                        return Err(format!("unknown flag for address-local8-gate: {flag}").into());
                    }
                }
            }
            let input = input.ok_or("address-local8-gate requires --input <prom-range.json>")?;
            Ok(CommandKind::AddressLocalGate {
                input,
                window_days,
                json_out,
                check_collisions,
            })
        }
        "zk-vote-tally-bundle" => {
            let mut output = None;
            let mut verify = false;
            let mut print_hashes = false;
            let mut summary_target = None;
            let mut attestation_target = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    "--verify" => verify = true,
                    "--print-hashes" => print_hashes = true,
                    "--summary-json" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --summary-json".into());
                        };
                        if value == "-" {
                            summary_target = Some(JsonTarget::Stdout);
                        } else {
                            summary_target =
                                Some(JsonTarget::File(normalize_path(Path::new(&value))?));
                        }
                    }
                    "--attestation" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --attestation".into());
                        };
                        if value == "-" {
                            attestation_target = Some(JsonTarget::Stdout);
                        } else {
                            attestation_target =
                                Some(JsonTarget::File(normalize_path(Path::new(&value))?));
                        }
                    }
                    flag => {
                        return Err(format!("unknown flag for zk-vote-tally-bundle: {flag}").into());
                    }
                }
            }
            let output = output.unwrap_or_else(default_vote_tally_path);
            Ok(CommandKind::ZkVoteTallyBundle {
                output,
                verify,
                print_hashes,
                summary_target,
                attestation_target,
            })
        }
        "mochi-bundle" => {
            let mut output: Option<PathBuf> = None;
            let mut profile = String::from("release");
            let mut archive = true;
            let mut kagami: Option<PathBuf> = None;
            let mut matrix: Option<PathBuf> = None;
            let mut smoke = false;
            let mut stage: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    "--profile" => {
                        let Some(value) = pending.next() else {
                            return Err("expected profile name after --profile".into());
                        };
                        profile = value;
                    }
                    "--no-archive" => archive = false,
                    "--kagami" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --kagami".into());
                        };
                        kagami = Some(normalize_path(Path::new(&path))?);
                    }
                    "--matrix" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --matrix".into());
                        };
                        matrix = Some(normalize_path(Path::new(&path))?);
                    }
                    "--smoke" => {
                        smoke = true;
                    }
                    "--stage" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --stage".into());
                        };
                        stage = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for mochi-bundle: {flag}").into());
                    }
                }
            }

            let output = output.unwrap_or_else(default_mochi_bundle_path);
            Ok(CommandKind::MochiBundle {
                output,
                profile,
                archive,
                kagami,
                matrix,
                smoke,
                stage,
            })
        }
        "kagami-profiles" => {
            let mut output: Option<PathBuf> = None;
            let mut profiles: Vec<String> = Vec::new();
            let mut kagami: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--profile" => {
                        let Some(value) = pending.next() else {
                            return Err("expected profile after --profile".into());
                        };
                        profiles.push(value);
                    }
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    "--kagami" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --kagami".into());
                        };
                        kagami = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for kagami-profiles: {flag}").into());
                    }
                }
            }
            let output = output
                .map(Ok)
                .unwrap_or_else(|| normalize_path(Path::new("defaults/kagami")))?;
            Ok(CommandKind::KagamiProfiles {
                options: kagami_profiles::KagamiProfileOptions {
                    output,
                    profiles,
                    kagami_override: kagami,
                },
            })
        }
        "soranet-fixtures" => {
            let mut output: Option<PathBuf> = None;
            let mut verify = false;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    "--verify" => verify = true,
                    flag => {
                        return Err(format!("unknown flag for soranet-fixtures: {flag}").into());
                    }
                }
            }
            let output = output.unwrap_or_else(|| soranet::default_fixture_dir(&workspace_root()));
            Ok(CommandKind::SoranetFixtures { output, verify })
        }
        "soranet-chaos-kit" => {
            let mut output_dir: Option<PathBuf> = None;
            let mut pop_label = "soranet-pop-m0".to_string();
            let mut gateway_host = "soranet-gw-m0".to_string();
            let mut resolver_host = "soradns-resolver-m0".to_string();
            let mut quarter_label: Option<String> = None;
            let mut now_override: Option<SystemTime> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--out" | "--output-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--pop" => {
                        let Some(value) = pending.next() else {
                            return Err("expected label after --pop".into());
                        };
                        pop_label = value;
                    }
                    "--gateway" => {
                        let Some(value) = pending.next() else {
                            return Err("expected host after --gateway".into());
                        };
                        gateway_host = value;
                    }
                    "--resolver" => {
                        let Some(value) = pending.next() else {
                            return Err("expected host after --resolver".into());
                        };
                        resolver_host = value;
                    }
                    "--quarter" => {
                        let Some(value) = pending.next() else {
                            return Err("expected label after --quarter".into());
                        };
                        quarter_label = Some(value);
                    }
                    "--now" => {
                        let Some(value) = pending.next() else {
                            return Err("expected unix seconds after --now".into());
                        };
                        let secs: u64 = value.parse()?;
                        now_override = Some(UNIX_EPOCH + Duration::from_secs(secs));
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-chaos-kit: {flag}").into());
                    }
                }
            }
            let now_for_default = now_override.unwrap_or_else(SystemTime::now);
            let output_dir =
                output_dir.unwrap_or_else(|| default_soranet_chaos_dir(now_for_default));
            let options = soranet_chaos::ChaosKitOptions {
                output_dir,
                pop_label,
                gateway_host,
                resolver_host,
                quarter_label,
                now: Some(now_for_default),
            };
            Ok(CommandKind::SoranetChaosKit { options })
        }
        "soranet-chaos-report" => {
            let mut log_path: Option<PathBuf> = None;
            let mut json_out: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--log" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --log".into());
                        };
                        log_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out".into());
                        };
                        if path == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-chaos-report: {flag}").into());
                    }
                }
            }
            let log_path = log_path.ok_or("soranet-chaos-report requires --log <path>")?;
            let output = json_out.unwrap_or(JsonTarget::Stdout);
            Ok(CommandKind::SoranetChaosReport {
                options: soranet_chaos::ChaosReportOptions { log_path, output },
            })
        }
        "sorafs-gateway-fixtures" => {
            let mut output: Option<PathBuf> = None;
            let mut verify = false;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    "--verify" | "--check" => {
                        verify = true;
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for sorafs-gateway-fixtures: {flag}").into()
                        );
                    }
                }
            }
            let default = sorafs::default_gateway_fixture_dir();
            Ok(CommandKind::SorafsGatewayFixtures {
                output: output.unwrap_or(default),
                verify,
            })
        }
        "sorafs-admission-fixtures" => {
            let mut output: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for sorafs-admission-fixtures: {flag}").into(),
                        );
                    }
                }
            }
            let default = normalize_path(Path::new("fixtures/sorafs_manifest/provider_admission"))?;
            Ok(CommandKind::SorafsAdmissionFixtures {
                output: output.unwrap_or(default),
            })
        }
        "sorafs-adoption-check" => {
            let mut pending = args.peekable();
            let mut scoreboard_paths: Vec<PathBuf> = Vec::new();
            let mut summary_paths: Vec<PathBuf> = Vec::new();
            let mut min_providers: usize = 2;
            let mut require_positive_weight = true;
            let mut allow_single_source = false;
            let mut report: Option<PathBuf> = None;
            let mut require_telemetry_source = false;
            let mut require_telemetry_region = false;
            let mut allow_implicit_metadata = false;
            let mut require_direct_only = false;
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--scoreboard" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --scoreboard".into());
                        };
                        scoreboard_paths.push(normalize_path(Path::new(&path))?);
                    }
                    "--min-providers" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --min-providers".into());
                        };
                        let parsed: usize = value.parse().map_err(|err| {
                            format!("invalid --min-providers value `{value}`: {err}")
                        })?;
                        if parsed == 0 {
                            return Err("--min-providers must be at least 1".into());
                        }
                        min_providers = parsed;
                    }
                    "--summary" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --summary".into());
                        };
                        summary_paths.push(normalize_path(Path::new(&path))?);
                    }
                    "--allow-zero-weight" => {
                        require_positive_weight = false;
                    }
                    "--allow-single-source" => {
                        allow_single_source = true;
                    }
                    "--require-telemetry" | "--require-telemetry-source" => {
                        require_telemetry_source = true;
                    }
                    "--require-telemetry-region" => {
                        require_telemetry_region = true;
                    }
                    "--allow-implicit-metadata" => {
                        allow_implicit_metadata = true;
                    }
                    "--require-direct-only" => {
                        require_direct_only = true;
                    }
                    "--report" | "--report-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --report".into());
                        };
                        report = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for sorafs-adoption-check: {flag}").into()
                        );
                    }
                }
            }
            Ok(CommandKind::SorafsAdoptionCheck {
                options: sorafs::AdoptionCheckOptions {
                    scoreboard_paths,
                    summary_paths,
                    min_eligible_providers: min_providers,
                    require_positive_weight,
                    allow_single_source_fallback: allow_single_source,
                    require_telemetry_source,
                    require_telemetry_region,
                    allow_implicit_metadata,
                    require_direct_only,
                },
                report,
            })
        }
        "sns-portal-stub" => {
            let mut pending = args.peekable();
            let mut cycle: Option<String> = None;
            let mut suffixes: Vec<String> = Vec::new();
            let mut output: Option<PathBuf> = None;
            let mut overwrite = false;
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--cycle" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --cycle".into());
                        };
                        cycle = Some(value);
                    }
                    "--suffix" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --suffix".into());
                        };
                        suffixes.push(value);
                    }
                    "--output" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        output = Some(normalize_path(Path::new(&value))?);
                    }
                    "--force" | "--overwrite" => {
                        overwrite = true;
                    }
                    flag => {
                        return Err(format!("unknown flag for sns-portal-stub: {flag}").into());
                    }
                }
            }
            let cycle =
                cycle.ok_or_else(|| "--cycle is required for sns-portal-stub".to_string())?;
            let output = output.unwrap_or_else(|| sns::default_portal_stub_path(&cycle));
            Ok(CommandKind::SnsPortalStub {
                options: sns::PortalStubOptions {
                    cycle,
                    suffixes,
                    output,
                    overwrite,
                },
            })
        }
        "sorafs-scoreboard-diff" => {
            let mut pending = args.peekable();
            let mut previous: Option<PathBuf> = None;
            let mut current: Option<PathBuf> = None;
            let mut threshold_percent = 10.0f64;
            let mut report: Option<PathBuf> = None;
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--previous" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --previous".into());
                        };
                        previous = Some(normalize_path(Path::new(&value))?);
                    }
                    "--current" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --current".into());
                        };
                        current = Some(normalize_path(Path::new(&value))?);
                    }
                    "--threshold-percent" => {
                        let Some(raw) = pending.next() else {
                            return Err("expected value after --threshold-percent".into());
                        };
                        threshold_percent = raw.parse().map_err(|err| {
                            format!("invalid --threshold-percent value `{raw}`: {err}")
                        })?;
                    }
                    "--report" | "--report-out" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --report".into());
                        };
                        report = Some(normalize_path(Path::new(&value))?);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for sorafs-scoreboard-diff: {flag}").into()
                        );
                    }
                }
            }
            let previous =
                previous.ok_or("sorafs-scoreboard-diff requires --previous <scoreboard.json>")?;
            let current =
                current.ok_or("sorafs-scoreboard-diff requires --current <scoreboard.json>")?;
            Ok(CommandKind::SorafsScoreboardDiff {
                options: sorafs::ScoreboardDiffOptions {
                    previous_scoreboard: previous,
                    current_scoreboard: current,
                    threshold_percent,
                },
                report,
            })
        }
        "sorafs-taikai-cache-bundle" => {
            let mut pending = args.peekable();
            let mut profile_paths: Vec<PathBuf> = Vec::new();
            let mut output_root: Option<PathBuf> = None;
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--profile" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --profile".into());
                        };
                        let path = sorafs::resolve_taikai_cache_profile_path(&value)?;
                        profile_paths.push(path);
                    }
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out/--output".into());
                        };
                        output_root = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for sorafs-taikai-cache-bundle: {flag}").into(),
                        );
                    }
                }
            }
            let options = sorafs::TaikaiCacheBundleOptions {
                profile_paths,
                output_root: output_root.unwrap_or_else(sorafs::default_taikai_cache_output_root),
            };
            Ok(CommandKind::SorafsTaikaiCacheBundle {
                options: Box::new(options),
            })
        }
        "taikai-anchor-bundle" => {
            let mut spool_dir: Option<PathBuf> = None;
            let mut copy_dir: Option<PathBuf> = None;
            let mut signing_key: Option<PathBuf> = None;
            let mut output: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--spool" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --spool".into());
                        };
                        spool_dir = Some(normalize_path(Path::new(&value))?);
                    }
                    "--copy-dir" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --copy-dir".into());
                        };
                        copy_dir = Some(normalize_path(Path::new(&value))?);
                    }
                    "--signing-key" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --signing-key".into());
                        };
                        signing_key = Some(normalize_path(Path::new(&value))?);
                    }
                    "-o" | "--out" | "--output" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --out/--output".into());
                        };
                        if value == "-" {
                            output = Some(JsonTarget::Stdout);
                        } else {
                            output = Some(JsonTarget::File(normalize_path(Path::new(&value))?));
                        }
                    }
                    flag => {
                        return Err(format!("unknown flag for taikai-anchor-bundle: {flag}").into());
                    }
                }
            }
            Ok(CommandKind::TaikaiAnchorBundle {
                options: taikai_anchor::AnchorBundleOptions {
                    spool_dir: spool_dir.unwrap_or_else(default_taikai_spool_dir),
                    copy_dir,
                    signing_key,
                    output: output.unwrap_or(JsonTarget::Stdout),
                },
            })
        }
        "taikai-rpt-verify" => {
            let mut envelope: Option<PathBuf> = None;
            let mut gar_path: Option<PathBuf> = None;
            let mut cek_receipt: Option<PathBuf> = None;
            let mut bundle_path: Option<PathBuf> = None;
            let mut json_out: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--envelope" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --envelope".into());
                        };
                        envelope = Some(normalize_path(Path::new(&path))?);
                    }
                    "--gar" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --gar".into());
                        };
                        gar_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--cek-receipt" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --cek-receipt".into());
                        };
                        cek_receipt = Some(normalize_path(Path::new(&path))?);
                    }
                    "--bundle" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --bundle".into());
                        };
                        bundle_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--json-out" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --json-out".into());
                        };
                        if value == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&value))?));
                        }
                    }
                    flag => {
                        return Err(format!("unknown flag for taikai-rpt-verify: {flag}").into());
                    }
                }
            }
            let envelope_path = envelope
                .ok_or_else(|| "taikai-rpt-verify requires --envelope <path>".to_string())?;
            Ok(CommandKind::TaikaiRptVerify {
                options: taikai::RptVerifyOptions {
                    envelope_path,
                    gar_path,
                    cek_receipt_path: cek_receipt,
                    bundle_path,
                    output: json_out,
                },
            })
        }
        "sorafs-burn-in-check" => {
            let mut pending = args.peekable();
            let mut log_paths: Vec<PathBuf> = Vec::new();
            let mut window_days: u64 = 30;
            let mut min_pq_ratio: f64 = 0.95;
            let mut max_no_provider_errors: u64 = 0;
            let mut max_brownout_ratio: f64 = 0.01;
            let mut min_fetches: u64 = 100;
            let mut output: Option<PathBuf> = None;
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--log" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --log".into());
                        };
                        log_paths.push(normalize_path(Path::new(&path))?);
                    }
                    "--window-days" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --window-days".into());
                        };
                        let parsed: u64 = value.parse().map_err(|err| {
                            format!("invalid --window-days value `{value}`: {err}")
                        })?;
                        if parsed == 0 {
                            return Err("--window-days must be at least 1".into());
                        }
                        window_days = parsed;
                    }
                    "--min-pq-ratio" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --min-pq-ratio".into());
                        };
                        let parsed: f64 = value.parse().map_err(|err| {
                            format!("invalid --min-pq-ratio value `{value}`: {err}")
                        })?;
                        if !(0.0..=1.0).contains(&parsed) {
                            return Err("--min-pq-ratio must be between 0.0 and 1.0".into());
                        }
                        min_pq_ratio = parsed;
                    }
                    "--max-no-provider-errors" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --max-no-provider-errors".into());
                        };
                        max_no_provider_errors = value.parse().map_err(|err| {
                            format!("invalid --max-no-provider-errors value `{value}`: {err}")
                        })?;
                    }
                    "--max-brownout-ratio" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --max-brownout-ratio".into());
                        };
                        let parsed: f64 = value.parse().map_err(|err| {
                            format!("invalid --max-brownout-ratio value `{value}`: {err}")
                        })?;
                        if !(0.0..=1.0).contains(&parsed) {
                            return Err("--max-brownout-ratio must be between 0.0 and 1.0".into());
                        }
                        max_brownout_ratio = parsed;
                    }
                    "--min-fetches" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --min-fetches".into());
                        };
                        let parsed: u64 = value.parse().map_err(|err| {
                            format!("invalid --min-fetches value `{value}`: {err}")
                        })?;
                        if parsed == 0 {
                            return Err("--min-fetches must be at least 1".into());
                        }
                        min_fetches = parsed;
                    }
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for sorafs-burn-in-check: {flag}").into());
                    }
                }
            }
            if log_paths.is_empty() {
                return Err("sorafs-burn-in-check requires at least one --log <path>".into());
            }
            Ok(CommandKind::SorafsBurnInCheck {
                options: sorafs::BurnInCheckOptions {
                    log_paths,
                    required_window_days: window_days,
                    min_pq_ratio,
                    max_no_provider_errors,
                    max_brownout_ratio,
                    min_fetches,
                },
                output,
            })
        }
        "sorafs-reserve-matrix" => {
            let mut capacities = Vec::new();
            let mut storage_classes = Vec::new();
            let mut tiers = Vec::new();
            let mut durations = Vec::new();
            let mut policy_json: Option<PathBuf> = None;
            let mut policy_norito: Option<PathBuf> = None;
            let mut reserve_balance: Option<XorAmount> = None;
            let mut label: Option<String> = None;
            let mut target: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--capacity" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --capacity".into());
                        };
                        let parsed = parse_u64_flag(&value, "--capacity")?;
                        capacities.push(parsed);
                    }
                    "--storage-class" => {
                        let Some(value) = pending.next() else {
                            return Err("expected label after --storage-class".into());
                        };
                        let class = sorafs::parse_storage_class_label(&value)?;
                        storage_classes.push(class);
                    }
                    "--tier" => {
                        let Some(value) = pending.next() else {
                            return Err("expected label after --tier".into());
                        };
                        let tier = sorafs::parse_reserve_tier_label(&value)?;
                        tiers.push(tier);
                    }
                    "--duration" => {
                        let Some(value) = pending.next() else {
                            return Err("expected label after --duration".into());
                        };
                        let duration = sorafs::parse_reserve_duration_label(&value)?;
                        durations.push(duration);
                    }
                    "--policy-json" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --policy-json".into());
                        };
                        policy_json = Some(normalize_path(Path::new(&path))?);
                    }
                    "--policy-norito" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --policy-norito".into());
                        };
                        policy_norito = Some(normalize_path(Path::new(&path))?);
                    }
                    "--reserve-balance" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --reserve-balance".into());
                        };
                        let amount = sorafs::parse_xor_amount_decimal(&value)?;
                        reserve_balance = Some(amount);
                    }
                    "--label" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --label".into());
                        };
                        label = Some(value);
                    }
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out".into());
                        };
                        if path == "-" {
                            target = Some(JsonTarget::Stdout);
                        } else {
                            target = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for sorafs-reserve-matrix: {flag}").into()
                        );
                    }
                }
            }
            let options = sorafs::ReserveMatrixOptions {
                capacities_gib: capacities,
                storage_classes,
                tiers,
                durations,
                reserve_balance: reserve_balance.unwrap_or_else(XorAmount::zero),
                policy_json,
                policy_norito,
                label,
            };
            let target = target.unwrap_or(JsonTarget::Stdout);
            Ok(CommandKind::SorafsReserveMatrix { options, target })
        }
        "sorafs-fetch-fixture" => {
            let mut signatures: Option<sorafs::FetchSource> = None;
            let mut manifest: Option<sorafs::FetchSource> = None;
            let mut output: Option<PathBuf> = None;
            let mut profile = String::from("sorafs.sf1@1.0.0");
            let mut allow_unsigned = false;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--signatures" => {
                        let Some(value) = pending.next() else {
                            return Err("expected argument after --signatures".into());
                        };
                        signatures = Some(sorafs::FetchSource::parse(&value)?);
                    }
                    "--manifest" => {
                        let Some(value) = pending.next() else {
                            return Err("expected argument after --manifest".into());
                        };
                        manifest = Some(sorafs::FetchSource::parse(&value)?);
                    }
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected argument after --out".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    "--profile" => {
                        let Some(value) = pending.next() else {
                            return Err("expected argument after --profile".into());
                        };
                        profile = value;
                    }
                    "--allow-unsigned" => allow_unsigned = true,
                    flag => {
                        return Err(format!("unknown flag for sorafs-fetch-fixture: {flag}").into());
                    }
                }
            }

            let signatures =
                signatures.ok_or("sorafs-fetch-fixture requires --signatures <path|url>")?;
            let output =
                output.unwrap_or_else(|| workspace_root().join("fixtures").join("sorafs_chunker"));

            Ok(CommandKind::SorafsFetchFixture {
                options: Box::new(sorafs::FetchFixtureOptions {
                    signatures_source: signatures,
                    manifest_source: manifest,
                    output_dir: output,
                    profile_handle: profile,
                    allow_unsigned,
                }),
            })
        }
        "sorafs-gateway-attest" => {
            let mut output: Option<PathBuf> = None;
            let mut signing_key: Option<PathBuf> = None;
            let mut signer_account: Option<String> = None;
            let mut gateway_target: Option<String> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    "--signing-key" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --signing-key".into());
                        };
                        signing_key = Some(normalize_path(Path::new(&path))?);
                    }
                    "--signer-account" => {
                        let Some(value) = pending.next() else {
                            return Err("expected account after --signer-account".into());
                        };
                        signer_account = Some(value);
                    }
                    "--gateway" => {
                        let Some(value) = pending.next() else {
                            return Err("expected URL after --gateway".into());
                        };
                        gateway_target = Some(value);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for sorafs-gateway-attest: {flag}").into()
                        );
                    }
                }
            }

            let signing_key_path =
                signing_key.ok_or("sorafs-gateway-attest requires --signing-key <path>")?;
            let signer_account = signer_account
                .ok_or("sorafs-gateway-attest requires --signer-account <account>")?;
            let output = output.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("sorafs_gateway_attest")
            });

            Ok(CommandKind::SorafsGatewayAttest {
                options: Box::new(sorafs::GatewayAttestOptions {
                    output_dir: output,
                    signing_key_path,
                    signer_account,
                    gateway_target,
                }),
            })
        }
        "sorafs-gateway-probe" => {
            let mut gateway: Option<String> = None;
            let mut headers_path: Option<PathBuf> = None;
            let mut method: Option<String> = None;
            let mut timeout_secs: Option<u64> = None;
            let mut extra_headers: Vec<(String, String)> = Vec::new();
            let mut gar_path: Option<PathBuf> = None;
            let mut gar_keys: Vec<(String, Vec<u8>)> = Vec::new();
            let mut cache_max_age: Option<u64> = None;
            let mut cache_swr: Option<u64> = None;
            let mut now_override: Option<u64> = None;
            let mut host_override: Option<String> = None;
            let mut require_tls_state = true;
            let mut report_target: Option<JsonTarget> = None;
            let mut summary_path: Option<PathBuf> = None;
            let mut drill_log_path: Option<PathBuf> = None;
            let mut drill_scenario: Option<String> = None;
            let mut drill_ic: Option<String> = None;
            let mut drill_scribe: Option<String> = None;
            let mut drill_notes: Option<String> = None;
            let mut drill_link: Option<String> = None;
            let mut pagerduty_payload: Option<PathBuf> = None;
            let mut pagerduty_routing_key: Option<String> = None;
            let mut pagerduty_severity: Option<String> = None;
            let mut pagerduty_source: Option<String> = None;
            let mut pagerduty_component: Option<String> = None;
            let mut pagerduty_group: Option<String> = None;
            let mut pagerduty_class: Option<String> = None;
            let mut pagerduty_dedup_key: Option<String> = None;
            let mut pagerduty_links: Vec<(String, String)> = Vec::new();
            let mut pagerduty_url: Option<Url> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--gateway" => {
                        let Some(value) = pending.next() else {
                            return Err("expected URL after --gateway".into());
                        };
                        gateway = Some(value);
                    }
                    "--headers-file" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --headers-file".into());
                        };
                        headers_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--method" => {
                        let Some(value) = pending.next() else {
                            return Err("expected method after --method".into());
                        };
                        method = Some(value);
                    }
                    "--timeout-secs" => {
                        let Some(value) = pending.next() else {
                            return Err("expected integer after --timeout-secs".into());
                        };
                        timeout_secs = Some(
                            value
                                .parse()
                                .map_err(|_| "invalid integer for --timeout-secs")?,
                        );
                    }
                    "--header" => {
                        let Some(value) = pending.next() else {
                            return Err("expected `Name: Value` after --header".into());
                        };
                        let (name, val) = value
                            .split_once(':')
                            .ok_or("expected --header value in `Name: Value` form")?;
                        extra_headers.push((name.trim().to_string(), val.trim().to_string()));
                    }
                    "--gar" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --gar".into());
                        };
                        gar_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--gar-key" => {
                        let Some(value) = pending.next() else {
                            return Err("expected `kid=hex` after --gar-key".into());
                        };
                        let (kid, hex_value) = value
                            .split_once('=')
                            .ok_or("expected --gar-key value in `kid=hex` form")?;
                        let bytes = hex::decode(hex_value.trim())
                            .map_err(|_| "failed to decode --gar-key hex value")?;
                        gar_keys.push((kid.trim().to_string(), bytes));
                    }
                    "--cache-max-age" => {
                        let Some(value) = pending.next() else {
                            return Err("expected integer after --cache-max-age".into());
                        };
                        cache_max_age = Some(
                            value
                                .parse()
                                .map_err(|_| "invalid integer for --cache-max-age")?,
                        );
                    }
                    "--cache-stale-while-revalidate" => {
                        let Some(value) = pending.next() else {
                            return Err(
                                "expected integer after --cache-stale-while-revalidate".into()
                            );
                        };
                        cache_swr =
                            Some(value.parse().map_err(
                                |_| "invalid integer for --cache-stale-while-revalidate",
                            )?);
                    }
                    "--now" => {
                        let Some(value) = pending.next() else {
                            return Err("expected unix timestamp after --now".into());
                        };
                        now_override = Some(
                            value
                                .parse()
                                .map_err(|_| "invalid integer for --now (unix seconds)")?,
                        );
                    }
                    "--host" => {
                        let Some(value) = pending.next() else {
                            return Err("expected host after --host".into());
                        };
                        host_override = Some(value);
                    }
                    "--allow-missing-tls-state" => {
                        require_tls_state = false;
                    }
                    "--report-json" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path or - after --report-json".into());
                        };
                        if value == "-" {
                            report_target = Some(JsonTarget::Stdout);
                        } else {
                            report_target =
                                Some(JsonTarget::File(normalize_path(Path::new(&value))?));
                        }
                    }
                    "--summary-json" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --summary-json".into());
                        };
                        summary_path = Some(normalize_path(Path::new(&value))?);
                    }
                    "--drill-log" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --drill-log".into());
                        };
                        drill_log_path = Some(normalize_path(Path::new(&value))?);
                    }
                    "--drill-scenario" => {
                        let Some(value) = pending.next() else {
                            return Err("expected name after --drill-scenario".into());
                        };
                        drill_scenario = Some(value);
                    }
                    "--drill-ic" => {
                        let Some(value) = pending.next() else {
                            return Err("expected name after --drill-ic".into());
                        };
                        drill_ic = Some(value);
                    }
                    "--drill-scribe" => {
                        let Some(value) = pending.next() else {
                            return Err("expected name after --drill-scribe".into());
                        };
                        drill_scribe = Some(value);
                    }
                    "--drill-notes" => {
                        let Some(value) = pending.next() else {
                            return Err("expected text after --drill-notes".into());
                        };
                        drill_notes = Some(value);
                    }
                    "--drill-link" => {
                        let Some(value) = pending.next() else {
                            return Err("expected URL after --drill-link".into());
                        };
                        drill_link = Some(value);
                    }
                    "--pagerduty-payload" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --pagerduty-payload".into());
                        };
                        pagerduty_payload = Some(normalize_path(Path::new(&value))?);
                    }
                    "--pagerduty-routing-key" => {
                        let Some(value) = pending.next() else {
                            return Err("expected key after --pagerduty-routing-key".into());
                        };
                        pagerduty_routing_key = Some(value);
                    }
                    "--pagerduty-severity" => {
                        let Some(value) = pending.next() else {
                            return Err("expected severity after --pagerduty-severity".into());
                        };
                        pagerduty_severity = Some(value);
                    }
                    "--pagerduty-source" => {
                        let Some(value) = pending.next() else {
                            return Err("expected source after --pagerduty-source".into());
                        };
                        pagerduty_source = Some(value);
                    }
                    "--pagerduty-component" => {
                        let Some(value) = pending.next() else {
                            return Err("expected component after --pagerduty-component".into());
                        };
                        pagerduty_component = Some(value);
                    }
                    "--pagerduty-group" => {
                        let Some(value) = pending.next() else {
                            return Err("expected group after --pagerduty-group".into());
                        };
                        pagerduty_group = Some(value);
                    }
                    "--pagerduty-class" => {
                        let Some(value) = pending.next() else {
                            return Err("expected class after --pagerduty-class".into());
                        };
                        pagerduty_class = Some(value);
                    }
                    "--pagerduty-dedup-key" => {
                        let Some(value) = pending.next() else {
                            return Err("expected key after --pagerduty-dedup-key".into());
                        };
                        pagerduty_dedup_key = Some(value);
                    }
                    "--pagerduty-link" => {
                        let Some(value) = pending.next() else {
                            return Err("expected text=url after --pagerduty-link".into());
                        };
                        let (text, href) = value
                            .split_once('=')
                            .ok_or("expected --pagerduty-link value in `text=url` form")?;
                        pagerduty_links.push((text.trim().to_string(), href.trim().to_string()));
                    }
                    "--pagerduty-url" => {
                        let Some(value) = pending.next() else {
                            return Err("expected URL after --pagerduty-url".into());
                        };
                        pagerduty_url = Some(Url::parse(&value)?);
                    }
                    flag => {
                        return Err(format!("unknown flag for sorafs-gateway-probe: {flag}").into());
                    }
                }
            }

            if gateway.is_none() && headers_path.is_none() {
                return Err("sorafs-gateway-probe requires --gateway or --headers-file".into());
            }
            if gateway.is_some() && headers_path.is_some() {
                return Err("provide either --gateway or --headers-file, not both".into());
            }
            let gar_path =
                gar_path.ok_or("sorafs-gateway-probe requires --gar <path-to-gar-jws>")?;
            if gar_keys.is_empty() {
                return Err("sorafs-gateway-probe requires at least one --gar-key kid=hex".into());
            }
            let drill = if drill_log_path.is_some()
                || drill_scenario.is_some()
                || drill_ic.is_some()
                || drill_scribe.is_some()
                || drill_notes.is_some()
                || drill_link.is_some()
            {
                let scenario = drill_scenario
                    .ok_or("--drill-scenario is required when enabling drill logging")?;
                let log_path = drill_log_path
                    .unwrap_or_else(|| workspace_root().join("ops").join("drill-log.md"));
                Some(sorafs::DrillLogConfig {
                    log_path,
                    scenario,
                    ic: drill_ic,
                    scribe: drill_scribe,
                    notes: drill_notes,
                    link: drill_link,
                })
            } else {
                None
            };
            let pagerduty = if pagerduty_payload.is_some()
                || pagerduty_routing_key.is_some()
                || pagerduty_severity.is_some()
                || pagerduty_source.is_some()
                || pagerduty_component.is_some()
                || pagerduty_group.is_some()
                || pagerduty_class.is_some()
                || pagerduty_dedup_key.is_some()
                || !pagerduty_links.is_empty()
                || pagerduty_url.is_some()
            {
                let routing_key = pagerduty_routing_key.ok_or(
                    "--pagerduty-routing-key is required when enabling PagerDuty payloads",
                )?;
                let payload_path = pagerduty_payload.unwrap_or_else(|| {
                    workspace_root()
                        .join("artifacts")
                        .join("sorafs_gateway_probe")
                        .join("pagerduty_event.json")
                });
                Some(sorafs::PagerDutyConfig {
                    payload_path,
                    routing_key,
                    severity: pagerduty_severity.unwrap_or_else(|| "error".to_string()),
                    source: pagerduty_source.unwrap_or_else(|| "sorafs-gateway-probe".to_string()),
                    component: pagerduty_component,
                    group: pagerduty_group,
                    class_name: pagerduty_class,
                    dedup_key: pagerduty_dedup_key,
                    links: pagerduty_links
                        .into_iter()
                        .map(|(text, href)| sorafs::PagerDutyLink { text, href })
                        .collect(),
                    endpoint_url: pagerduty_url,
                })
            } else {
                None
            };

            let options = sorafs::GatewayProbeOptions {
                request: gateway.map(|url| sorafs::GatewayProbeRequest {
                    url,
                    method: method.unwrap_or_else(|| "HEAD".to_string()),
                    timeout_secs,
                    extra_headers,
                }),
                headers_path,
                gar_path,
                gar_keys,
                cache_max_age,
                cache_swr,
                now_override,
                host_override,
                require_tls_state,
                report_target,
                summary_path,
                drill,
                pagerduty,
            };
            Ok(CommandKind::SorafsGatewayProbe {
                options: Box::new(options),
            })
        }
        "sorafs-gateway" => {
            let remaining: Vec<String> = args.collect();
            let command = sorafs::parse_gateway_cli(remaining.into_iter())?;
            Ok(CommandKind::SorafsGatewayCli {
                command: Box::new(command),
            })
        }
        "soranet-pop-bundle" => {
            let mut input_path: Option<PathBuf> = None;
            let mut roa_path: Option<PathBuf> = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut edns_resolver: Option<String> = None;
            let mut edns_tool: Option<String> = None;
            let mut skip_edns = false;
            let mut skip_ds = false;
            let mut image_tag = "latest".to_string();
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--input" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --input".into());
                        };
                        input_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--roa" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --roa".into());
                        };
                        roa_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--edns-resolver" => {
                        let Some(value) = pending.next() else {
                            return Err("expected resolver address after --edns-resolver".into());
                        };
                        edns_resolver = Some(value);
                    }
                    "--edns-tool" => {
                        let Some(value) = pending.next() else {
                            return Err("expected binary name after --edns-tool".into());
                        };
                        edns_tool = Some(value);
                    }
                    "--skip-edns" => {
                        skip_edns = true;
                    }
                    "--skip-ds" => {
                        skip_ds = true;
                    }
                    "--image-tag" => {
                        let Some(value) = pending.next() else {
                            return Err("expected tag after --image-tag".into());
                        };
                        image_tag = value;
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-pop-bundle: {flag}").into());
                    }
                }
            }
            let input = input_path
                .ok_or_else(|| "soranet-pop-bundle requires --input <path>".to_string())?;
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("soranet_pop")
                    .join("bundle")
            });
            let options = soranet_pop::PopBundleOptions {
                descriptor: input,
                roa_bundle: roa_path,
                output_dir,
                edns_resolver,
                edns_tool,
                skip_edns,
                skip_ds,
                image_tag,
            };
            Ok(CommandKind::SoranetPopBundle {
                options: Box::new(options),
            })
        }
        "soranet-popctl" => {
            let mut input_path: Option<PathBuf> = None;
            let mut roa_path: Option<PathBuf> = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut edns_resolver: Option<String> = None;
            let mut edns_tool: Option<String> = None;
            let mut skip_edns = false;
            let mut skip_ds = false;
            let mut image_tag = "latest".to_string();
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--input" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --input".into());
                        };
                        input_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--roa" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --roa".into());
                        };
                        roa_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--edns-resolver" => {
                        let Some(value) = pending.next() else {
                            return Err("expected resolver address after --edns-resolver".into());
                        };
                        edns_resolver = Some(value);
                    }
                    "--edns-tool" => {
                        let Some(value) = pending.next() else {
                            return Err("expected binary name after --edns-tool".into());
                        };
                        edns_tool = Some(value);
                    }
                    "--skip-edns" => skip_edns = true,
                    "--skip-ds" => skip_ds = true,
                    "--image-tag" => {
                        let Some(value) = pending.next() else {
                            return Err("expected tag after --image-tag".into());
                        };
                        image_tag = value;
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-popctl: {flag}").into());
                    }
                }
            }
            let input =
                input_path.ok_or_else(|| "soranet-popctl requires --input <path>".to_string())?;
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("soranet_pop")
                    .join("bundle")
            });
            let options = soranet_pop::PopBundleOptions {
                descriptor: input,
                roa_bundle: roa_path,
                output_dir,
                edns_resolver,
                edns_tool,
                skip_edns,
                skip_ds,
                image_tag,
            };
            Ok(CommandKind::SoranetPopCtl {
                options: Box::new(options),
            })
        }
        "soranet-pop-template" => {
            let mut input_path: Option<PathBuf> = None;
            let mut output_path: Option<PathBuf> = None;
            let mut resolver_config: Option<PathBuf> = None;
            let mut edns_out: Option<PathBuf> = None;
            let mut ds_out: Option<PathBuf> = None;
            let mut edns_resolver: Option<String> = None;
            let mut edns_tool: Option<String> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--input" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --input".into());
                        };
                        input_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        output_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--resolver-config" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --resolver-config".into());
                        };
                        resolver_config = Some(normalize_path(Path::new(&path))?);
                    }
                    "--edns-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --edns-out".into());
                        };
                        edns_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--edns-resolver" => {
                        let Some(value) = pending.next() else {
                            return Err("expected resolver address after --edns-resolver".into());
                        };
                        edns_resolver = Some(value);
                    }
                    "--edns-tool" => {
                        let Some(value) = pending.next() else {
                            return Err("expected binary name after --edns-tool".into());
                        };
                        edns_tool = Some(value);
                    }
                    "--ds-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --ds-out".into());
                        };
                        ds_out = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-pop-template: {flag}").into());
                    }
                }
            }
            let input = input_path
                .ok_or_else(|| "soranet-pop-template requires --input <path>".to_string())?;
            let output = output_path.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("soranet_pop")
                    .join("frr.conf")
            });
            Ok(CommandKind::SoranetPopTemplate {
                input,
                output,
                resolver_config,
                edns_out,
                edns_resolver,
                ds_out,
                edns_tool,
            })
        }
        "soranet-pop-validate" => {
            let mut input_path: Option<PathBuf> = None;
            let mut roa_path: Option<PathBuf> = None;
            let mut output: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--input" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --input".into());
                        };
                        input_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--roa" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --roa".into());
                        };
                        roa_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--out" | "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path or '-' after --json-out".into());
                        };
                        if path == "-" {
                            output = Some(JsonTarget::Stdout);
                        } else {
                            output = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-pop-validate: {flag}").into());
                    }
                }
            }
            let input = input_path
                .ok_or_else(|| "soranet-pop-validate requires --input <path>".to_string())?;
            let output = output.unwrap_or(JsonTarget::Stdout);
            Ok(CommandKind::SoranetPopValidate {
                input,
                roa: roa_path,
                output,
            })
        }
        "soranet-pop-policy-report" => {
            let mut input_path: Option<PathBuf> = None;
            let mut roa_path: Option<PathBuf> = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut grafana_out: Option<PathBuf> = None;
            let mut alert_rules_out: Option<PathBuf> = None;
            let mut report_out: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--input" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --input".into());
                        };
                        input_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--roa" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --roa".into());
                        };
                        roa_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--grafana-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --grafana-out".into());
                        };
                        grafana_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--alert-rules-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --alert-rules-out".into());
                        };
                        alert_rules_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--json-out" | "--report-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out/--report-out".into());
                        };
                        report_out = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for soranet-pop-policy-report: {flag}").into(),
                        );
                    }
                }
            }
            let input = input_path
                .ok_or_else(|| "soranet-pop-policy-report requires --input <path>".to_string())?;
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("soranet_pop")
                    .join("policy")
            });
            let options = soranet_pop::PopPolicyHarnessOptions {
                descriptor: input,
                roa_bundle: roa_path,
                output_dir,
                grafana_out,
                alert_rules_out,
                report_out,
            };
            Ok(CommandKind::SoranetPopPolicyReport {
                options: Box::new(options),
            })
        }
        "soranet-pop-plan" => {
            let mut input_path: Option<PathBuf> = None;
            let mut roa_path: Option<PathBuf> = None;
            let mut frr_out: Option<PathBuf> = None;
            let mut report_out: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--input" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --input".into());
                        };
                        input_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--roa" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --roa".into());
                        };
                        roa_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--frr-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --frr-out".into());
                        };
                        frr_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--report-out" | "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path or '-' after --json-out".into());
                        };
                        if path == "-" {
                            report_out = Some(JsonTarget::Stdout);
                        } else {
                            report_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-pop-plan: {flag}").into());
                    }
                }
            }
            let input =
                input_path.ok_or_else(|| "soranet-pop-plan requires --input <path>".to_string())?;
            let frr_out = frr_out.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("soranet_pop")
                    .join("frr.conf")
            });
            let report_out = report_out.unwrap_or(JsonTarget::Stdout);
            Ok(CommandKind::SoranetPopPlan {
                input,
                roa: roa_path,
                frr_out,
                report_out,
            })
        }
        "soranet-gateway-m0" => {
            let mut output_dir: Option<PathBuf> = None;
            let mut edge_name = "soranet-edge-m0".to_string();
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--output-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--edge-name" => {
                        let Some(value) = pending.next() else {
                            return Err("expected name after --edge-name".into());
                        };
                        edge_name = value;
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-gateway-m0: {flag}").into());
                    }
                }
            }
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("configs")
                    .join("soranet")
                    .join("gateway_m0")
            });
            Ok(CommandKind::SoranetGatewayM0 {
                options: soranet_gateway::GatewayM0Options {
                    output_dir,
                    edge_name,
                },
            })
        }
        "soranet-gateway-m1" => {
            let mut config_path: Option<PathBuf> = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--config" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --config".into());
                        };
                        config_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output-dir" | "--out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir/--out".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-gateway-m1: {flag}").into());
                    }
                }
            }
            let config_path = config_path.unwrap_or_else(|| {
                workspace_root()
                    .join("configs")
                    .join("soranet")
                    .join("gateway_m1")
                    .join("alpha_config.json")
            });
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("soranet")
                    .join("gateway_m1")
                    .join("alpha")
            });
            Ok(CommandKind::SoranetGatewayM1 {
                options: soranet_gateway_m1::GatewayM1Options {
                    config_path,
                    output_dir,
                },
            })
        }
        "soranet-gateway-m2" => {
            let mut config_path: Option<PathBuf> = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--config" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --config".into());
                        };
                        config_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output-dir" | "--out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir/--out".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-gateway-m2: {flag}").into());
                    }
                }
            }
            let config_path = config_path.unwrap_or_else(|| {
                workspace_root()
                    .join("configs")
                    .join("soranet")
                    .join("gateway_m2")
                    .join("beta_config.json")
            });
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("soranet")
                    .join("gateway_m2")
                    .join("beta")
            });
            Ok(CommandKind::SoranetGatewayM2 {
                options: soranet_gateway_m2::GatewayM2Options {
                    config_path,
                    output_dir,
                },
            })
        }
        "soranet-gateway-m3" => {
            let mut m2_summary: Option<PathBuf> = None;
            let mut autoscale_plan: Option<PathBuf> = None;
            let mut worker_pack: Option<PathBuf> = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut sla_target: Option<String> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--m2-summary" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --m2-summary".into());
                        };
                        m2_summary = Some(normalize_path(Path::new(&path))?);
                    }
                    "--autoscale-plan" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --autoscale-plan".into());
                        };
                        autoscale_plan = Some(normalize_path(Path::new(&path))?);
                    }
                    "--worker-pack" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --worker-pack".into());
                        };
                        worker_pack = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output-dir" | "--out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir/--out".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--sla-target" => {
                        let Some(value) = pending.next() else {
                            return Err("expected label after --sla-target".into());
                        };
                        sla_target = Some(value);
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-gateway-m3: {flag}").into());
                    }
                }
            }
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("soranet")
                    .join("gateway_m3")
            });
            let m2_summary = m2_summary
                .ok_or_else(|| "soranet-gateway-m3 requires --m2-summary <path>".to_string())?;
            let autoscale_plan = autoscale_plan
                .ok_or_else(|| "soranet-gateway-m3 requires --autoscale-plan <path>".to_string())?;
            let worker_pack = worker_pack
                .ok_or_else(|| "soranet-gateway-m3 requires --worker-pack <path>".to_string())?;
            Ok(CommandKind::SoranetGatewayM3 {
                options: soranet_gateway_m3::GatewayM3Options {
                    m2_summary,
                    output_dir,
                    autoscale_plan,
                    worker_pack,
                    sla_target,
                },
            })
        }
        "soranet-gateway-ops-m0" => {
            let mut output_dir: Option<PathBuf> = None;
            let mut pop = "soranet-pop-m0".to_string();
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--output-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--pop" => {
                        let Some(value) = pending.next() else {
                            return Err("expected name after --pop".into());
                        };
                        pop = value;
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for soranet-gateway-ops-m0: {flag}").into()
                        );
                    }
                }
            }
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("configs")
                    .join("soranet")
                    .join("gateway_m0")
                    .join("observability")
            });
            Ok(CommandKind::SoranetGatewayOpsM0 {
                options: soranet_gateway_ops::GatewayOpsOptions { output_dir, pop },
            })
        }
        "soranet-gateway-pq" => {
            let mut output_dir: Option<PathBuf> = None;
            let mut pop = "soranet-pop-pq".to_string();
            let mut srcv2_bundle: Option<PathBuf> = None;
            let mut tls_bundle_dir: Option<PathBuf> = None;
            let mut trustless_config: Option<PathBuf> = None;
            let mut canary_hosts: Vec<String> = Vec::new();
            let mut phase = CertificateValidationPhase::Phase3RequireDual;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--output-dir" | "--out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir/--out".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--pop" => {
                        let Some(value) = pending.next() else {
                            return Err("expected name after --pop".into());
                        };
                        pop = value;
                    }
                    "--srcv2" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --srcv2".into());
                        };
                        srcv2_bundle = Some(normalize_path(Path::new(&path))?);
                    }
                    "--tls-bundle" => {
                        let Some(path) = pending.next() else {
                            return Err("expected directory after --tls-bundle".into());
                        };
                        tls_bundle_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--trustless-config" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --trustless-config".into());
                        };
                        trustless_config = Some(normalize_path(Path::new(&path))?);
                    }
                    "--canary" => {
                        let Some(host) = pending.next() else {
                            return Err("expected host after --canary".into());
                        };
                        canary_hosts.push(host);
                    }
                    "--phase" => {
                        let Some(value) = pending.next() else {
                            return Err("expected phase after --phase".into());
                        };
                        phase = match value.as_str() {
                            "1" => CertificateValidationPhase::Phase1AllowSingle,
                            "2" => CertificateValidationPhase::Phase2PreferDual,
                            "3" => CertificateValidationPhase::Phase3RequireDual,
                            other => {
                                return Err(format!(
                                    "unknown validation phase `{other}` (expected 1|2|3)"
                                )
                                .into());
                            }
                        };
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-gateway-pq: {flag}").into());
                    }
                }
            }

            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("soranet")
                    .join("gateway_pq")
            });
            let srcv2_bundle = srcv2_bundle
                .ok_or_else(|| "soranet-gateway-pq requires --srcv2 <bundle>".to_string())?;
            let tls_bundle_dir = tls_bundle_dir
                .ok_or_else(|| "soranet-gateway-pq requires --tls-bundle <dir>".to_string())?;
            let trustless_config = trustless_config.ok_or_else(|| {
                "soranet-gateway-pq requires --trustless-config <path>".to_string()
            })?;

            Ok(CommandKind::SoranetGatewayPq {
                options: soranet_gateway_pq::GatewayPqOptions {
                    output_dir,
                    pop,
                    srcv2_bundle,
                    tls_bundle_dir,
                    trustless_config,
                    canary_hosts,
                    validation_phase: phase,
                },
            })
        }
        "soranet-bug-bounty" => {
            let mut config_path: Option<PathBuf> = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--config" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --config".into());
                        };
                        config_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-bug-bounty: {flag}").into());
                    }
                }
            }
            let config_path = config_path
                .ok_or_else(|| "soranet-bug-bounty requires --config <path>".to_string())?;
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("soranet")
                    .join("gateway")
                    .join("bug_bounty")
            });
            Ok(CommandKind::SoranetBugBounty {
                options: soranet_bug_bounty::BugBountyOptions {
                    config_path,
                    output_dir,
                },
            })
        }
        "soranet-gateway-ops-m1" => {
            let mut output_dir: Option<PathBuf> = None;
            let mut pops: Vec<String> = Vec::new();
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--output-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--pops" => {
                        let Some(list) = pending.next() else {
                            return Err("expected comma-separated list after --pops".into());
                        };
                        pops.extend(list.split(',').map(|s| s.to_string()));
                    }
                    "--pop" => {
                        let Some(pop) = pending.next() else {
                            return Err("expected name after --pop".into());
                        };
                        pops.push(pop);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for soranet-gateway-ops-m1: {flag}").into()
                        );
                    }
                }
            }
            if pops.is_empty() {
                pops = vec![
                    "soranet-sjc01".to_string(),
                    "soranet-iad01".to_string(),
                    "soranet-fra01".to_string(),
                ];
            }
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("configs")
                    .join("soranet")
                    .join("gateway_m1")
            });
            Ok(CommandKind::SoranetGatewayOpsM1 {
                options: soranet_gateway_ops::GatewayOpsMultiOptions { output_dir, pops },
            })
        }
        "soranet-gateway-chaos" => {
            let mut config: Option<PathBuf> = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut pop = "soranet-pop-m0".to_string();
            let mut scenarios: Option<Vec<String>> = None;
            let mut execute = false;
            let mut note: Option<String> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--config" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --config".into());
                        };
                        config = Some(normalize_path(Path::new(&path))?);
                    }
                    "--out" | "--output-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out/--output-dir".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--pop" => {
                        let Some(value) = pending.next() else {
                            return Err("expected name after --pop".into());
                        };
                        pop = value;
                    }
                    "--scenario" => {
                        let Some(value) = pending.next() else {
                            return Err("expected id after --scenario".into());
                        };
                        let ids = value
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>();
                        scenarios = Some(ids);
                    }
                    "--execute" => {
                        execute = true;
                    }
                    "--note" => {
                        let Some(value) = pending.next() else {
                            return Err("expected note after --note".into());
                        };
                        note = Some(value);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for soranet-gateway-chaos: {flag}").into()
                        );
                    }
                }
            }
            let config_path = config.unwrap_or_else(|| {
                workspace_root()
                    .join("configs")
                    .join("soranet")
                    .join("gateway_m0")
                    .join("observability")
                    .join("chaos_scenarios.json")
            });
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("soranet")
                    .join("gateway_chaos")
            });
            let selection = scenarios
                .map(|ids| {
                    if ids.iter().any(|id| id == "all") {
                        soranet_gateway_chaos::ScenarioSelection::All
                    } else {
                        soranet_gateway_chaos::ScenarioSelection::Only(ids)
                    }
                })
                .unwrap_or(soranet_gateway_chaos::ScenarioSelection::All);
            Ok(CommandKind::SoranetGatewayChaos {
                options: soranet_gateway_chaos::ChaosOptions {
                    config_path,
                    output_dir,
                    pop,
                    scenarios: selection,
                    execute,
                    note,
                    now: OffsetDateTime::now_utc(),
                },
            })
        }
        "soranet-gateway-hardening" => {
            let mut output_dir: Option<PathBuf> = None;
            let mut sbom: Option<PathBuf> = None;
            let mut vuln_report: Option<PathBuf> = None;
            let mut hsm_policy: Option<PathBuf> = None;
            let mut sandbox_profile: Option<PathBuf> = None;
            let mut data_retention_days: u32 = 30;
            let mut log_retention_days: u32 = 30;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--output-dir" | "--out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir/--out".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--sbom" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --sbom".into());
                        };
                        sbom = Some(normalize_path(Path::new(&path))?);
                    }
                    "--vuln-report" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --vuln-report".into());
                        };
                        vuln_report = Some(normalize_path(Path::new(&path))?);
                    }
                    "--hsm-policy" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --hsm-policy".into());
                        };
                        hsm_policy = Some(normalize_path(Path::new(&path))?);
                    }
                    "--sandbox-profile" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --sandbox-profile".into());
                        };
                        sandbox_profile = Some(normalize_path(Path::new(&path))?);
                    }
                    "--data-retention-days" => {
                        let Some(value) = pending.next() else {
                            return Err("expected integer after --data-retention-days".into());
                        };
                        data_retention_days = value.parse().map_err(|_| {
                            "data retention must be an unsigned integer".to_string()
                        })?;
                    }
                    "--log-retention-days" => {
                        let Some(value) = pending.next() else {
                            return Err("expected integer after --log-retention-days".into());
                        };
                        log_retention_days = value
                            .parse()
                            .map_err(|_| "log retention must be an unsigned integer")?;
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for soranet-gateway-hardening: {flag}").into(),
                        );
                    }
                }
            }
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("soranet")
                    .join("gateway_hardening")
            });
            Ok(CommandKind::SoranetGatewayHardening {
                options: soranet_gateway_hardening::GatewayHardeningOptions {
                    output_dir,
                    sbom,
                    vuln_report,
                    hsm_policy,
                    sandbox_profile,
                    data_retention_days,
                    log_retention_days,
                },
            })
        }
        "soranet-gar-controller" => {
            let mut config: Option<PathBuf> = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut markdown_out: Option<PathBuf> = None;
            let mut now_unix: Option<u64> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--config" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --config".into());
                        };
                        config = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--markdown-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --markdown-out".into());
                        };
                        markdown_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--now" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --now".into());
                        };
                        now_unix = Some(
                            value
                                .parse::<u64>()
                                .map_err(|_| "expected u64 after --now")?,
                        );
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for soranet-gar-controller: {flag}").into()
                        );
                    }
                }
            }
            let config = config.unwrap_or_else(|| {
                workspace_root()
                    .join("configs")
                    .join("soranet")
                    .join("gateway_m0")
                    .join("gar_controller.sample.json")
            });
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("soranet")
                    .join("gateway")
                    .join("gar_controller")
            });
            Ok(CommandKind::SoranetGarController {
                options: soranet_gar_controller::GarControllerOptions {
                    config,
                    output_dir,
                    markdown_out,
                    now_unix,
                },
            })
        }
        "soranet-gar-export" => {
            let mut receipts_dir: Option<PathBuf> = None;
            let mut ack_dir: Option<PathBuf> = None;
            let mut json_out: Option<JsonTarget> = None;
            let mut markdown_out: Option<PathBuf> = None;
            let mut pop_label: Option<String> = None;
            let mut now_unix: Option<u64> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--receipts-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --receipts-dir".into());
                        };
                        receipts_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--acks-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --acks-dir".into());
                        };
                        ack_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    "--markdown-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --markdown-out".into());
                        };
                        markdown_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--pop" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --pop".into());
                        };
                        pop_label = Some(value);
                    }
                    "--now" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --now".into());
                        };
                        now_unix = Some(
                            value
                                .parse::<u64>()
                                .map_err(|_| "expected u64 after --now")?,
                        );
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-gar-export: {flag}").into());
                    }
                }
            }
            let pop_clone = pop_label.clone();
            let receipts_dir = match receipts_dir {
                Some(dir) => dir,
                None => {
                    let pop = pop_label.clone().ok_or_else(|| {
                        "soranet-gar-export requires --receipts-dir or --pop".to_string()
                    })?;
                    default_gar_receipts_dir(&pop)
                }
            };
            let ack_dir = match ack_dir {
                Some(dir) => Some(dir),
                None => pop_clone.as_ref().map(|pop| default_gar_acks_dir(pop)),
            };
            let output = json_out.unwrap_or_else(|| {
                pop_clone
                    .as_ref()
                    .map(|pop| JsonTarget::File(default_gar_summary_path(pop)))
                    .unwrap_or(JsonTarget::Stdout)
            });
            Ok(CommandKind::SoranetGarExport {
                options: gar::ExportOptions {
                    receipts_dir,
                    ack_dir,
                    pop_label,
                    markdown_out,
                    now_unix,
                },
                output,
            })
        }
        "soranet-gar-bus" => {
            let mut policy_path: Option<PathBuf> = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut pop_label: Option<String> = None;
            let mut json_out: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--policy" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --policy".into());
                        };
                        policy_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--out-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out-dir".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--pop" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --pop".into());
                        };
                        pop_label = Some(value);
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-gar-bus: {flag}").into());
                    }
                }
            }
            let policy_path = policy_path
                .ok_or_else(|| "soranet-gar-bus requires --policy <path>".to_string())?;
            let output_dir =
                output_dir.unwrap_or_else(|| default_gar_bus_dir(pop_label.as_deref()));
            let output = json_out.unwrap_or(JsonTarget::Stdout);
            Ok(CommandKind::SoranetGarBus {
                options: gar_bus::GarBusOptions {
                    policy_path,
                    output_dir,
                    pop: pop_label,
                },
                output,
            })
        }
        "soranet-gateway-billing" => {
            let mut usage_path: Option<PathBuf> = None;
            let mut catalog_path: Option<PathBuf> = None;
            let mut guardrails_path: Option<PathBuf> = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut payer =
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                    .to_string();
            let mut treasury =
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                    .to_string();
            let mut asset = "xor#wonderland".to_string();
            let mut allow_hard_cap = false;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--usage" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --usage".into());
                        };
                        usage_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--catalog" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --catalog".into());
                        };
                        catalog_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--guardrails" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --guardrails".into());
                        };
                        guardrails_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output-dir" | "--out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--payer" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --payer".into());
                        };
                        payer = value;
                    }
                    "--treasury" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --treasury".into());
                        };
                        treasury = value;
                    }
                    "--asset" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --asset".into());
                        };
                        asset = value;
                    }
                    "--allow-hard-cap" => {
                        allow_hard_cap = true;
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for soranet-gateway-billing: {flag}").into()
                        );
                    }
                }
            }
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("soranet")
                    .join("gateway_billing")
            });
            let usage_path = usage_path.unwrap_or_else(|| {
                workspace_root()
                    .join("configs")
                    .join("soranet")
                    .join("gateway_m0")
                    .join("billing_usage_sample.json")
            });
            let catalog_path = catalog_path.unwrap_or_else(|| {
                workspace_root()
                    .join("configs")
                    .join("soranet")
                    .join("gateway_m0")
                    .join("meter_catalog.json")
            });
            let guardrails_path = guardrails_path.or_else(|| {
                let default = workspace_root()
                    .join("configs")
                    .join("soranet")
                    .join("gateway_m0")
                    .join("billing_guardrails.json");
                default.exists().then_some(default)
            });
            Ok(CommandKind::SoranetGatewayBilling {
                options: soranet_gateway_billing::BillingOptions {
                    usage_path,
                    catalog_path,
                    guardrails_path,
                    output_dir,
                    payer,
                    treasury,
                    asset_definition: asset,
                    allow_hard_cap,
                },
            })
        }
        "soranet-gateway-billing-m0" => {
            let mut output_dir: Option<PathBuf> = None;
            let mut billing_period = "2026-11".to_string();
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--output-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-dir".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--billing-period" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --billing-period".into());
                        };
                        billing_period = value;
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for soranet-gateway-billing-m0: {flag}").into(),
                        );
                    }
                }
            }
            let output_dir = output_dir.unwrap_or_else(|| {
                workspace_root()
                    .join("configs")
                    .join("soranet")
                    .join("gateway_m0")
                    .join("billing")
            });
            Ok(CommandKind::SoranetGatewayBillingM0 {
                options: soranet_billing::BillingM0Options {
                    output_dir,
                    billing_period,
                },
            })
        }
        "soranet-privacy-report" => {
            let mut pending = args.peekable();
            let mut inputs: Vec<PathBuf> = Vec::new();
            let mut config = iroha_telemetry::privacy::PrivacyBucketConfig::default();
            let mut drain_at_unix: Option<u64> = None;
            let mut max_bucket_records = soranet_privacy::DEFAULT_MAX_BUCKET_RECORDS;
            let mut json_out: Option<JsonTarget> = None;
            let mut suppression_ratio_budget: Option<f64> = None;
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--input" | "-i" | "--in" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --input".into());
                        };
                        let resolved = normalize_path(Path::new(&path))?;
                        inputs.push(resolved);
                    }
                    "--bucket-secs" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --bucket-secs".into());
                        };
                        config.bucket_secs = value.parse().map_err(|err| {
                            format!("invalid --bucket-secs value `{value}`: {err}")
                        })?;
                    }
                    "--min-contributors" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --min-contributors".into());
                        };
                        config.min_contributors = value.parse().map_err(|err| {
                            format!("invalid --min-contributors value `{value}`: {err}")
                        })?;
                    }
                    "--flush-delay-buckets" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --flush-delay-buckets".into());
                        };
                        config.flush_delay_buckets = value.parse().map_err(|err| {
                            format!("invalid --flush-delay-buckets value `{value}`: {err}")
                        })?;
                    }
                    "--force-flush-buckets" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --force-flush-buckets".into());
                        };
                        config.force_flush_buckets = value.parse().map_err(|err| {
                            format!("invalid --force-flush-buckets value `{value}`: {err}")
                        })?;
                    }
                    "--max-completed-buckets" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --max-completed-buckets".into());
                        };
                        config.max_completed_buckets = value.parse().map_err(|err| {
                            format!("invalid --max-completed-buckets value `{value}`: {err}")
                        })?;
                    }
                    "--expected-shares" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --expected-shares".into());
                        };
                        config.expected_shares = value.parse().map_err(|err| {
                            format!("invalid --expected-shares value `{value}`: {err}")
                        })?;
                    }
                    "--drain-at" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --drain-at".into());
                        };
                        drain_at_unix =
                            Some(value.parse().map_err(|err| {
                                format!("invalid --drain-at value `{value}`: {err}")
                            })?);
                    }
                    "--max-buckets" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --max-buckets".into());
                        };
                        let parsed: usize = value.parse().map_err(|err| {
                            format!("invalid --max-buckets value `{value}`: {err}")
                        })?;
                        if parsed == 0 {
                            return Err("--max-buckets must be at least 1".into());
                        }
                        max_bucket_records = parsed;
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    "--max-suppression-ratio" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --max-suppression-ratio".into());
                        };
                        let parsed: f64 = value.parse().map_err(|err| {
                            format!("invalid --max-suppression-ratio value `{value}`: {err}")
                        })?;
                        if !(0.0..=1.0).contains(&parsed) {
                            return Err("--max-suppression-ratio must be between 0 and 1".into());
                        }
                        suppression_ratio_budget = Some(parsed);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for soranet-privacy-report: {flag}").into()
                        );
                    }
                }
            }
            if inputs.is_empty() {
                return Err("soranet-privacy-report requires at least one --input <path>".into());
            }
            let options = soranet_privacy::PrivacyReportOptions {
                input_paths: inputs,
                bucket_config: config,
                drain_at_unix,
            };
            Ok(CommandKind::SoranetPrivacyReport {
                options,
                json_out,
                max_bucket_records,
                suppression_ratio_budget,
            })
        }
        "nexus-fixtures" => {
            let mut output: Option<PathBuf> = None;
            let mut verify = false;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    "--verify" => verify = true,
                    flag => {
                        return Err(format!("unknown flag for nexus-fixtures: {flag}").into());
                    }
                }
            }
            let output = output.unwrap_or_else(default_nexus_lane_commitment_dir);
            Ok(CommandKind::NexusFixtures { output, verify })
        }
        "nexus-lane-maintenance" => {
            let mut config: Option<PathBuf> = None;
            let mut json_out: Option<PathBuf> = None;
            let mut compact_retired = false;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "-c" | "--config" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --config".into());
                        };
                        config = Some(normalize_path(Path::new(&path))?);
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        json_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--compact-retired" => compact_retired = true,
                    flag => {
                        return Err(
                            format!("unknown flag for nexus-lane-maintenance: {flag}").into()
                        );
                    }
                }
            }
            let config = config
                .ok_or_else(|| "nexus-lane-maintenance requires --config <path>".to_string())?;
            let json_output = json_out.unwrap_or_else(|| {
                workspace_root()
                    .join("artifacts")
                    .join("nexus_lane_maintenance.json")
            });
            let options = nexus_lane_maintenance::LaneMaintenanceOptions {
                config_path: config,
                json_output,
                compact_retired,
            };
            Ok(CommandKind::NexusLaneMaintenance { options })
        }
        "nexus-lane-audit" => {
            let mut status_path: Option<PathBuf> = None;
            let mut json_out: Option<PathBuf> = None;
            let mut parquet_out: Option<PathBuf> = None;
            let mut markdown_out: Option<PathBuf> = None;
            let mut captured_at: Option<String> = None;
            let mut lane_compliance: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--status" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --status".into());
                        };
                        status_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        json_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--parquet-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --parquet-out".into());
                        };
                        parquet_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--markdown-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --markdown-out".into());
                        };
                        markdown_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--captured-at" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --captured-at".into());
                        };
                        captured_at = Some(value);
                    }
                    "--lane-compliance" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --lane-compliance".into());
                        };
                        lane_compliance = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for nexus-lane-audit: {flag}").into());
                    }
                }
            }
            let status_path = status_path
                .ok_or_else(|| "nexus-lane-audit requires --status <path>".to_string())?;
            let options = nexus::LaneAuditOptions {
                status_path,
                json_output: json_out
                    .unwrap_or_else(|| PathBuf::from("artifacts/nexus_lane_audit.json")),
                parquet_output: parquet_out
                    .unwrap_or_else(|| PathBuf::from("artifacts/nexus_lane_audit.parquet")),
                markdown_output: markdown_out
                    .unwrap_or_else(|| PathBuf::from("artifacts/nexus_lane_audit.md")),
                captured_at,
                lane_compliance,
            };
            Ok(CommandKind::NexusLaneAudit { options })
        }
        "soranet-testnet-kit" => {
            let mut output: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-testnet-kit: {flag}").into());
                    }
                }
            }
            let output = output.unwrap_or_else(default_testnet_kit_path);
            Ok(CommandKind::SoranetTestnetKit { output })
        }
        "soranet-testnet-metrics" => {
            let mut input_path: Option<PathBuf> = None;
            let mut output: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "-i" | "--input" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --input".into());
                        };
                        input_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        if path == "-" {
                            output = Some(JsonTarget::Stdout);
                        } else {
                            output = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for soranet-testnet-metrics: {flag}").into()
                        );
                    }
                }
            }
            let Some(input) = input_path else {
                return Err("soranet-testnet-metrics requires --input <path>".into());
            };
            let output = output.unwrap_or(JsonTarget::Stdout);
            Ok(CommandKind::SoranetTestnetMetrics { input, output })
        }
        "soranet-testnet-feed" => {
            let mut promotion: Option<String> = None;
            let mut window_start: Option<Date> = None;
            let mut window_end: Option<Date> = None;
            let mut relays: Vec<String> = Vec::new();
            let mut relays_file: Option<PathBuf> = None;
            let mut metrics_report: Option<PathBuf> = None;
            let mut drill_log: Option<PathBuf> = None;
            let mut stage_report: Option<PathBuf> = None;
            let mut attachments: Vec<VerificationAttachment> = Vec::new();
            let mut output: Option<JsonTarget> = None;

            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--promotion" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --promotion".into());
                        };
                        promotion = Some(value);
                    }
                    "--window-start" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --window-start".into());
                        };
                        window_start = Some(parse_gate_date(&value)?);
                    }
                    "--window-end" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --window-end".into());
                        };
                        window_end = Some(parse_gate_date(&value)?);
                    }
                    "--relay" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --relay".into());
                        };
                        relays.push(value);
                    }
                    "--relays-file" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --relays-file".into());
                        };
                        relays_file = Some(normalize_path(Path::new(&path))?);
                    }
                    "--metrics-report" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --metrics-report".into());
                        };
                        metrics_report = Some(normalize_path(Path::new(&path))?);
                    }
                    "--drill-log" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --drill-log".into());
                        };
                        drill_log = Some(normalize_path(Path::new(&path))?);
                    }
                    "--stage-report" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --stage-report".into());
                        };
                        stage_report = Some(normalize_path(Path::new(&path))?);
                    }
                    "--attachment" => {
                        let Some(spec) = pending.next() else {
                            return Err("expected spec after --attachment".into());
                        };
                        let (label, path) = parse_attachment_spec(&spec)?;
                        attachments.push(VerificationAttachment { label, path });
                    }
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        if path == "-" {
                            output = Some(JsonTarget::Stdout);
                        } else {
                            output = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-testnet-feed: {flag}").into());
                    }
                }
            }

            let promotion = promotion.ok_or("soranet-testnet-feed requires --promotion <value>")?;
            let window_start =
                window_start.ok_or("soranet-testnet-feed requires --window-start <date>")?;
            let window_end =
                window_end.ok_or("soranet-testnet-feed requires --window-end <date>")?;
            let metrics_report =
                metrics_report.ok_or("soranet-testnet-feed requires --metrics-report <path>")?;

            if let Some(path) = relays_file {
                let mut from_file = load_relays_from_file(&path)?;
                relays.append(&mut from_file);
            }
            if relays.is_empty() {
                return Err(
                    "soranet-testnet-feed requires at least one --relay or --relays-file".into(),
                );
            }

            let options = VerificationFeedOptions {
                promotion,
                window_start,
                window_end,
                relays,
                metrics_report,
                drill_log,
                stage_report,
                attachments,
                output: output.unwrap_or(JsonTarget::Stdout),
            };
            Ok(CommandKind::SoranetTestnetFeed {
                options: Box::new(options),
            })
        }
        "soranet-testnet-drill-bundle" => {
            let mut log_path: Option<PathBuf> = None;
            let mut signing_key: Option<PathBuf> = None;
            let mut promotion: Option<String> = None;
            let mut window_start: Option<Date> = None;
            let mut window_end: Option<Date> = None;
            let mut attachments: Vec<VerificationAttachment> = Vec::new();
            let mut output: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--log" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --log".into());
                        };
                        log_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--signing-key" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --signing-key".into());
                        };
                        signing_key = Some(normalize_path(Path::new(&path))?);
                    }
                    "--promotion" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --promotion".into());
                        };
                        promotion = Some(value);
                    }
                    "--window-start" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --window-start".into());
                        };
                        window_start = Some(parse_gate_date(&value)?);
                    }
                    "--window-end" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --window-end".into());
                        };
                        window_end = Some(parse_gate_date(&value)?);
                    }
                    "--attachment" => {
                        let Some(spec) = pending.next() else {
                            return Err("expected spec after --attachment".into());
                        };
                        let (label, path) = parse_attachment_spec(&spec)?;
                        attachments.push(VerificationAttachment { label, path });
                    }
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        if path == "-" {
                            output = Some(JsonTarget::Stdout);
                        } else {
                            output = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    flag => {
                        return Err(format!(
                            "unknown flag for soranet-testnet-drill-bundle: {flag}"
                        )
                        .into());
                    }
                }
            }
            let log_path = log_path.ok_or("soranet-testnet-drill-bundle requires --log <path>")?;
            let signing_key =
                signing_key.ok_or("soranet-testnet-drill-bundle requires --signing-key <path>")?;
            let options = DrillBundleOptions {
                log_path,
                signing_key,
                promotion,
                window_start,
                window_end,
                attachments,
                output: output.unwrap_or(JsonTarget::Stdout),
            };
            Ok(CommandKind::SoranetTestnetDrillBundle {
                options: Box::new(options),
            })
        }
        "fastpq-bench-manifest" => {
            let mut benches = Vec::new();
            let mut output: Option<PathBuf> = None;
            let mut require_rows: Option<u64> = None;
            let mut max_operation_ms: BTreeMap<String, f64> = BTreeMap::new();
            let mut min_operation_speedup: BTreeMap<String, f64> = BTreeMap::new();
            let mut signing_key: Option<PathBuf> = None;
            let mut matrix_manifest: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--bench" => {
                        let Some(spec) = pending.next() else {
                            return Err("expected value after --bench".into());
                        };
                        let (label, path) = parse_labelled_path(&spec, "--bench")?;
                        benches.push(BenchInput { label, path });
                    }
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    "--require-rows" => {
                        let Some(value) = pending.next() else {
                            return Err("expected integer after --require-rows".into());
                        };
                        let rows = value
                            .parse::<u64>()
                            .map_err(|err| format!("invalid --require-rows `{value}`: {err}"))?;
                        require_rows = Some(rows);
                    }
                    "--max-operation-ms" => {
                        let Some(spec) = pending.next() else {
                            return Err("expected spec after --max-operation-ms".into());
                        };
                        let (name, limit) = parse_operation_limit(&spec, "--max-operation-ms")?;
                        max_operation_ms.insert(name, limit);
                    }
                    "--min-operation-speedup" => {
                        let Some(spec) = pending.next() else {
                            return Err("expected spec after --min-operation-speedup".into());
                        };
                        let (name, limit) =
                            parse_operation_limit(&spec, "--min-operation-speedup")?;
                        min_operation_speedup.insert(name, limit);
                    }
                    "--signing-key" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --signing-key".into());
                        };
                        signing_key = Some(normalize_path(Path::new(&path))?);
                    }
                    "--matrix" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --matrix".into());
                        };
                        matrix_manifest = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for fastpq-bench-manifest: {flag}").into()
                        );
                    }
                }
            }
            if matrix_manifest.is_none() {
                let default_matrix = fastpq::default_matrix_manifest_path();
                if default_matrix.is_file() {
                    matrix_manifest = Some(default_matrix);
                }
            }
            let options = BenchManifestOptions {
                benches,
                output: output.unwrap_or_else(fastpq::default_manifest_path),
                signing_key,
                require_rows,
                max_operation_ms,
                min_operation_speedup,
                matrix_manifest,
                label_max_operation_ms: BTreeMap::new(),
                label_min_operation_speedup: BTreeMap::new(),
            };
            if options.benches.is_empty() {
                return Err(
                    "fastpq-bench-manifest requires at least one --bench label=path".into(),
                );
            }
            Ok(CommandKind::FastpqBenchManifest {
                options: Box::new(options),
            })
        }
        "fastpq-stage-profile" => {
            let mut options = fastpq::StageProfileOptions::default();
            let mut custom_stages = Vec::new();
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--rows" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --rows".into());
                        };
                        options.rows = value
                            .parse::<usize>()
                            .map_err(|err| format!("invalid --rows `{value}`: {err}"))?;
                    }
                    "--warmups" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --warmups".into());
                        };
                        options.warmups = value
                            .parse::<usize>()
                            .map_err(|err| format!("invalid --warmups `{value}`: {err}"))?;
                    }
                    "--iterations" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --iterations".into());
                        };
                        options.iterations = value
                            .parse::<usize>()
                            .map_err(|err| format!("invalid --iterations `{value}`: {err}"))?;
                    }
                    "--out-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out-dir".into());
                        };
                        options.output_dir = normalize_path(Path::new(&path))?;
                    }
                    "--trace" => {
                        options.capture_trace = true;
                    }
                    "--no-trace" => {
                        options.capture_trace = false;
                    }
                    "--trace-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --trace-dir".into());
                        };
                        options.capture_trace = true;
                        options.trace_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--trace-template" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --trace-template".into());
                        };
                        options.capture_trace = true;
                        options.trace_template = Some(value);
                    }
                    "--trace-seconds" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --trace-seconds".into());
                        };
                        let seconds = value
                            .parse::<u32>()
                            .map_err(|err| format!("invalid --trace-seconds `{value}`: {err}"))?;
                        options.capture_trace = true;
                        options.trace_seconds = Some(seconds);
                    }
                    "--debug" => {
                        options.release = false;
                    }
                    "--no-gpu-probe" => {
                        options.gpu_probe = false;
                    }
                    "--stage" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --stage".into());
                        };
                        let Some(stage) = fastpq::StageKind::from_str(&value) else {
                            return Err(format!("unknown --stage `{value}`").into());
                        };
                        custom_stages.push(stage);
                    }
                    flag => {
                        return Err(format!("unknown flag for fastpq-stage-profile: {flag}").into());
                    }
                }
            }
            if !custom_stages.is_empty() {
                options.stages = custom_stages;
            }
            Ok(CommandKind::FastpqStageProfile {
                options: Box::new(options),
            })
        }
        "fastpq-cuda-suite" => {
            let mut options = fastpq::CudaSuiteOptions::default();
            let mut output_overridden = false;
            let mut raw_overridden = false;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--rows" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --rows".into());
                        };
                        options.rows = value
                            .parse::<usize>()
                            .map_err(|err| format!("invalid --rows `{value}`: {err}"))?;
                    }
                    "--warmups" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --warmups".into());
                        };
                        options.warmups = value
                            .parse::<usize>()
                            .map_err(|err| format!("invalid --warmups `{value}`: {err}"))?;
                    }
                    "--iterations" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --iterations".into());
                        };
                        options.iterations = value
                            .parse::<usize>()
                            .map_err(|err| format!("invalid --iterations `{value}`: {err}"))?;
                    }
                    "--columns" | "--column-count" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --columns".into());
                        };
                        options.column_count = value
                            .parse::<usize>()
                            .map_err(|err| format!("invalid --columns `{value}`: {err}"))?;
                    }
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        options.output = normalize_path(Path::new(&path))?;
                        output_overridden = true;
                    }
                    "--raw-output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --raw-output".into());
                        };
                        options.raw_output = normalize_path(Path::new(&path))?;
                        raw_overridden = true;
                    }
                    "--no-wrap" => {
                        options.wrap_output = false;
                    }
                    "--wrapper" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --wrapper".into());
                        };
                        options.wrapper = normalize_path(Path::new(&path))?;
                    }
                    "--require-lde-mean-ms" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --require-lde-mean-ms".into());
                        };
                        options.require_lde_mean_ms = value.parse::<f64>().map_err(|err| {
                            format!("invalid --require-lde-mean-ms `{value}`: {err}")
                        })?;
                    }
                    "--require-poseidon-mean-ms" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --require-poseidon-mean-ms".into());
                        };
                        options.require_poseidon_mean_ms = value.parse::<f64>().map_err(|err| {
                            format!("invalid --require-poseidon-mean-ms `{value}`: {err}")
                        })?;
                    }
                    "--label" => {
                        let Some(spec) = pending.next() else {
                            return Err("expected value after --label".into());
                        };
                        let (key, value) = parse_label_value(&spec, "--label")?;
                        options.labels.insert(key, value);
                    }
                    "--row-usage" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --row-usage".into());
                        };
                        options.row_usage = Some(normalize_path(Path::new(&path))?);
                    }
                    "--device" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --device".into());
                        };
                        options.device = Some(value);
                    }
                    "--notes" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --notes".into());
                        };
                        options.notes = Some(value);
                    }
                    "--accel-instance" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --accel-instance".into());
                        };
                        options.accel_instance = Some(value);
                    }
                    "--accel-state-json" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --accel-state-json".into());
                        };
                        options.accel_state_json = Some(normalize_path(Path::new(&path))?);
                    }
                    "--accel-state-prom" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --accel-state-prom".into());
                        };
                        options.accel_state_prom = Some(normalize_path(Path::new(&path))?);
                    }
                    "--sign-output" => {
                        options.sign_output = true;
                    }
                    "--gpg-key" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --gpg-key".into());
                        };
                        options.gpg_key = Some(value);
                    }
                    "--dry-run" => {
                        options.dry_run = true;
                    }
                    "--require-gpu" => {
                        options.require_gpu = true;
                    }
                    flag => {
                        return Err(format!("unknown flag for fastpq-cuda-suite: {flag}").into());
                    }
                }
            }
            if output_overridden && !raw_overridden {
                options.raw_output = fastpq::default_cuda_raw_output(&options.output);
            }
            Ok(CommandKind::FastpqCudaSuite {
                options: Box::new(options),
            })
        }
        "soranet-constant-rate-profile" => {
            let mut profile_name: Option<String> = None;
            let mut format = soranet::ConstantRateOutputFormat::Table;
            let mut include_tick_table = false;
            let mut tick_values: Vec<f64> = Vec::new();
            let mut json_out: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--profile" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --profile".into());
                        };
                        if value.trim().is_empty() {
                            return Err("--profile requires a non-empty value".into());
                        }
                        profile_name = Some(value);
                    }
                    "--format" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --format".into());
                        };
                        format = match value.as_str() {
                            "table" => soranet::ConstantRateOutputFormat::Table,
                            "json" => soranet::ConstantRateOutputFormat::Json,
                            "markdown" => soranet::ConstantRateOutputFormat::Markdown,
                            other => {
                                return Err(format!(
                                    "unknown --format `{other}`; expected table|json|markdown"
                                )
                                .into());
                            }
                        };
                    }
                    "--tick-table" => {
                        include_tick_table = true;
                    }
                    "--tick-values" => {
                        let Some(value) = pending.next() else {
                            return Err(
                                "expected comma-separated values after --tick-values".into()
                            );
                        };
                        include_tick_table = true;
                        let mut parsed = Vec::new();
                        for raw in value.split(',') {
                            let trimmed = raw.trim();
                            if trimmed.is_empty() {
                                return Err("tick values may not be empty".into());
                            }
                            let parsed_value = trimmed
                                .parse::<f64>()
                                .map_err(|err| format!("invalid tick value `{trimmed}`: {err}"))?;
                            parsed.push(parsed_value);
                        }
                        tick_values = parsed;
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    flag => {
                        return Err(format!(
                            "unknown flag for soranet-constant-rate-profile: {flag}"
                        )
                        .into());
                    }
                }
            }
            let selection = match profile_name {
                Some(name) => soranet::ConstantRateSelection::Named(name),
                None => soranet::ConstantRateSelection::All,
            };
            Ok(CommandKind::SoranetConstantRateProfile {
                selection,
                format,
                include_tick_table,
                tick_values,
                json_out,
            })
        }
        "soradns-hosts" => {
            let mut names = Vec::new();
            let mut json_out: Option<JsonTarget> = None;
            let mut verify_paths: Vec<PathBuf> = Vec::new();
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--name" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --name".into());
                        };
                        let trimmed = value.trim();
                        if trimmed.is_empty() {
                            return Err("--name requires a non-empty value".into());
                        }
                        names.push(trimmed.to_string());
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    "--verify-host-patterns" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --verify-host-patterns".into());
                        };
                        verify_paths.push(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for soradns-hosts: {flag}").into());
                    }
                }
            }
            if names.is_empty() {
                return Err("soradns-hosts requires at least one --name <fqdn>".into());
            }
            Ok(CommandKind::SoradnsHosts {
                names,
                json: json_out,
                verify: verify_paths,
            })
        }
        "soradns-binding-template" => {
            let mut manifest: Option<PathBuf> = None;
            let mut alias: Option<String> = None;
            let mut hostname: Option<String> = None;
            let mut route_label: Option<String> = None;
            let mut proof_status: Option<String> = None;
            let mut csp_template: Option<String> = None;
            let mut permissions_template: Option<String> = None;
            let mut hsts_template: Option<String> = None;
            let mut include_csp = true;
            let mut include_permissions = true;
            let mut include_hsts = true;
            let mut json_out: Option<JsonTarget> = None;
            let mut headers_out: Option<PathBuf> = None;
            let mut generated_at: Option<OffsetDateTime> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--manifest" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --manifest".into());
                        };
                        manifest = Some(normalize_path(Path::new(&value))?);
                    }
                    "--alias" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --alias".into());
                        };
                        alias = Some(value);
                    }
                    "--hostname" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --hostname".into());
                        };
                        hostname = Some(value);
                    }
                    "--route-label" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --route-label".into());
                        };
                        route_label = Some(value);
                    }
                    "--proof-status" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --proof-status".into());
                        };
                        proof_status = Some(value);
                    }
                    "--csp-template" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --csp-template".into());
                        };
                        csp_template = Some(value);
                    }
                    "--permissions-template" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --permissions-template".into());
                        };
                        permissions_template = Some(value);
                    }
                    "--hsts-template" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --hsts-template".into());
                        };
                        hsts_template = Some(value);
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    "--headers-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --headers-out".into());
                        };
                        headers_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--generated-at" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --generated-at".into());
                        };
                        let parsed = OffsetDateTime::parse(&value, &Rfc3339)
                            .map_err(|err| format!("invalid RFC3339 timestamp `{value}`: {err}"))?;
                        generated_at = Some(parsed);
                    }
                    "--no-csp" => include_csp = false,
                    "--no-permissions-policy" => include_permissions = false,
                    "--no-hsts" => include_hsts = false,
                    flag => {
                        return Err(
                            format!("unknown flag for soradns-binding-template: {flag}").into()
                        );
                    }
                }
            }
            let manifest = manifest.ok_or("soradns-binding-template requires --manifest <path>")?;
            let alias = alias.ok_or("soradns-binding-template requires --alias <value>")?;
            let hostname =
                hostname.ok_or("soradns-binding-template requires --hostname <value>")?;
            let options = soradns::BindingTemplateOptions {
                manifest_path: manifest,
                alias,
                hostname,
                route_label,
                proof_status,
                csp_template,
                permissions_template,
                hsts_template,
                include_csp,
                include_permissions,
                include_hsts,
                generated_at: generated_at.unwrap_or_else(OffsetDateTime::now_utc),
            };
            Ok(CommandKind::SoradnsBindingTemplate {
                options,
                json: json_out,
                headers_out,
            })
        }
        "soradns-gar-template" => {
            let mut name: Option<String> = None;
            let mut manifest_cid: Option<String> = None;
            let mut manifest_digest: Option<String> = None;
            let mut manifest_path: Option<PathBuf> = None;
            let mut valid_from_epoch: Option<u64> = None;
            let mut valid_until_epoch: Option<u64> = None;
            let mut csp_template: Option<String> = None;
            let mut hsts_template: Option<String> = None;
            let mut permissions_template: Option<String> = None;
            let mut telemetry_labels: Vec<String> = Vec::new();
            let mut json_out: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--name" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --name".into());
                        };
                        let trimmed = value.trim();
                        if trimmed.is_empty() {
                            return Err("--name requires a non-empty value".into());
                        }
                        name = Some(trimmed.to_string());
                    }
                    "--manifest-cid" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --manifest-cid".into());
                        };
                        let trimmed = value.trim();
                        if trimmed.is_empty() {
                            return Err("--manifest-cid requires a non-empty value".into());
                        }
                        manifest_cid = Some(trimmed.to_string());
                    }
                    "--manifest-digest" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --manifest-digest".into());
                        };
                        if value.trim().is_empty() {
                            manifest_digest = None;
                        } else {
                            manifest_digest = Some(value);
                        }
                    }
                    "--manifest" => {
                        let Some(value) = pending.next() else {
                            return Err("expected path after --manifest".into());
                        };
                        manifest_path = Some(normalize_path(Path::new(&value))?);
                    }
                    "--valid-from" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --valid-from".into());
                        };
                        let parsed = parse_u64_flag(&value, "--valid-from")?;
                        valid_from_epoch = Some(parsed);
                    }
                    "--valid-until" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --valid-until".into());
                        };
                        let parsed = parse_u64_flag(&value, "--valid-until")?;
                        valid_until_epoch = Some(parsed);
                    }
                    "--csp-template" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --csp-template".into());
                        };
                        if value.trim().is_empty() {
                            csp_template = None;
                        } else {
                            csp_template = Some(value);
                        }
                    }
                    "--hsts-template" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --hsts-template".into());
                        };
                        if value.trim().is_empty() {
                            hsts_template = None;
                        } else {
                            hsts_template = Some(value);
                        }
                    }
                    "--permissions-template" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --permissions-template".into());
                        };
                        if value.trim().is_empty() {
                            permissions_template = None;
                        } else {
                            permissions_template = Some(value);
                        }
                    }
                    "--telemetry-label" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --telemetry-label".into());
                        };
                        let trimmed = value.trim();
                        if !trimmed.is_empty() {
                            telemetry_labels.push(trimmed.to_string());
                        }
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    flag => {
                        return Err(format!("unknown flag for soradns-gar-template: {flag}").into());
                    }
                }
            }
            let Some(name) = name else {
                return Err("soradns-gar-template requires --name <fqdn>".into());
            };
            if manifest_cid.is_none() && manifest_path.is_none() {
                return Err(
                    "soradns-gar-template requires --manifest <path> or --manifest-cid <cid>"
                        .into(),
                );
            }
            let mut resolved_manifest_cid = manifest_cid;
            let mut resolved_manifest_digest = manifest_digest;
            if let Some(path) = manifest_path {
                let manifest_meta =
                    soradns::manifest_cid_and_digest_from_path(&path).map_err(|err| {
                        format!(
                            "failed to derive manifest metadata from {}: {err}",
                            path.display()
                        )
                    })?;
                match resolved_manifest_cid {
                    Some(ref mut manual) => {
                        let trimmed = manual.trim();
                        if !trimmed.eq_ignore_ascii_case(manifest_meta.manifest_cid.as_str()) {
                            return Err(format!(
                                "--manifest-cid `{}` does not match derived `{}`",
                                trimmed, manifest_meta.manifest_cid
                            )
                            .into());
                        }
                        *manual = manifest_meta.manifest_cid.clone();
                    }
                    None => {
                        resolved_manifest_cid = Some(manifest_meta.manifest_cid.clone());
                    }
                }
                match resolved_manifest_digest {
                    Some(ref mut manual) => {
                        let normalized =
                            soradns::normalize_manifest_digest(manual).map_err(|err| {
                                format!("invalid --manifest-digest `{manual}`: {err}")
                            })?;
                        if normalized != manifest_meta.manifest_digest_hex {
                            return Err(format!(
                                "--manifest-digest `{}` does not match derived `{}`",
                                manual, manifest_meta.manifest_digest_hex
                            )
                            .into());
                        }
                        *manual = normalized;
                    }
                    None => {
                        resolved_manifest_digest = Some(manifest_meta.manifest_digest_hex.clone());
                    }
                }
            }
            let Some(manifest_cid) = resolved_manifest_cid else {
                return Err(
                    "soradns-gar-template could not determine a manifest CID (pass --manifest or --manifest-cid)".into(),
                );
            };
            let default_valid_from = unix_timestamp_now()?;
            let options = soradns::GarTemplateOptions {
                name,
                manifest_cid,
                manifest_digest: resolved_manifest_digest,
                csp_template,
                hsts_template,
                permissions_template,
                valid_from_epoch: valid_from_epoch.unwrap_or(default_valid_from),
                valid_until_epoch,
                telemetry_labels,
            };
            Ok(CommandKind::SoradnsGarTemplate {
                options,
                output: json_out,
            })
        }
        "soradns-acme-plan" => {
            let mut names: Vec<String> = Vec::new();
            let mut directory_url: Option<String> = None;
            let mut include_pretty = true;
            let mut include_canonical_wildcard = true;
            let mut generated_at: Option<OffsetDateTime> = None;
            let mut json_out: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--name" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --name".into());
                        };
                        names.push(value);
                    }
                    "--directory-url" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --directory-url".into());
                        };
                        directory_url = Some(value);
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        json_out = Some(if path == "-" {
                            JsonTarget::Stdout
                        } else {
                            JsonTarget::File(normalize_path(Path::new(&path))?)
                        });
                    }
                    "--generated-at" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --generated-at".into());
                        };
                        let parsed = OffsetDateTime::parse(&value, &Rfc3339)
                            .map_err(|err| format!("invalid RFC3339 timestamp `{value}`: {err}"))?;
                        generated_at = Some(parsed);
                    }
                    "--no-pretty" => include_pretty = false,
                    "--no-canonical-wildcard" => include_canonical_wildcard = false,
                    flag => {
                        return Err(format!("unknown flag for soradns-acme-plan: {flag}").into());
                    }
                }
            }
            if names.is_empty() {
                return Err("soradns-acme-plan requires at least one --name <fqdn>".into());
            }
            if !include_pretty && !include_canonical_wildcard {
                return Err("soradns-acme-plan requires at least one certificate target; enable canonical wildcard and/or pretty hosts".into());
            }
            let options = soradns::AcmePlanOptions {
                names,
                directory_url: directory_url
                    .unwrap_or_else(|| soradns::DEFAULT_ACME_DIRECTORY_URL.to_string()),
                include_canonical_wildcard,
                include_pretty_hosts: include_pretty,
                generated_at: generated_at.unwrap_or_else(OffsetDateTime::now_utc),
            };
            Ok(CommandKind::SoradnsAcmePlan {
                options,
                output: json_out.unwrap_or(JsonTarget::Stdout),
            })
        }
        "soradns-cache-plan" => {
            let mut names: Vec<String> = Vec::new();
            let mut include_pretty_hosts = true;
            let mut paths: Vec<String> = Vec::new();
            let mut http_method: Option<String> = None;
            let mut auth_header: Option<String> = None;
            let mut auth_env: Option<String> = None;
            let mut generated_at: Option<OffsetDateTime> = None;
            let mut json_out: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--name" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --name".into());
                        };
                        names.push(value);
                    }
                    "--path" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --path".into());
                        };
                        paths.push(value);
                    }
                    "--http-method" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --http-method".into());
                        };
                        http_method = Some(value);
                    }
                    "--auth-header" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --auth-header".into());
                        };
                        auth_header = Some(value);
                    }
                    "--auth-env" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --auth-env".into());
                        };
                        auth_env = Some(value);
                    }
                    "--generated-at" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --generated-at".into());
                        };
                        let parsed = OffsetDateTime::parse(&value, &Rfc3339)
                            .map_err(|err| format!("invalid RFC3339 timestamp `{value}`: {err}"))?;
                        generated_at = Some(parsed);
                    }
                    "--no-pretty" => include_pretty_hosts = false,
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        json_out = Some(if path == "-" {
                            JsonTarget::Stdout
                        } else {
                            JsonTarget::File(normalize_path(Path::new(&path))?)
                        });
                    }
                    flag => {
                        return Err(format!("unknown flag for soradns-cache-plan: {flag}").into());
                    }
                }
            }
            if names.is_empty() {
                return Err("soradns-cache-plan requires at least one --name <fqdn>".into());
            }
            let options = soradns::CacheInvalidationPlanOptions {
                names,
                include_pretty_hosts,
                paths,
                http_method: http_method.unwrap_or_else(|| "PURGE".to_string()),
                auth_header,
                auth_env,
                generated_at: generated_at.unwrap_or_else(OffsetDateTime::now_utc),
            };
            Ok(CommandKind::SoradnsCachePlan {
                options,
                output: json_out.unwrap_or(JsonTarget::Stdout),
            })
        }
        "soradns-route-plan" => {
            let mut names: Vec<String> = Vec::new();
            let mut include_pretty_hosts = true;
            let mut generated_at: Option<OffsetDateTime> = None;
            let mut json_out: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--name" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --name".into());
                        };
                        names.push(value);
                    }
                    "--no-pretty" => include_pretty_hosts = false,
                    "--generated-at" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --generated-at".into());
                        };
                        let parsed = OffsetDateTime::parse(&value, &Rfc3339)
                            .map_err(|err| format!("invalid RFC3339 timestamp `{value}`: {err}"))?;
                        generated_at = Some(parsed);
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        json_out = Some(if path == "-" {
                            JsonTarget::Stdout
                        } else {
                            JsonTarget::File(normalize_path(Path::new(&path))?)
                        });
                    }
                    flag => {
                        return Err(format!("unknown flag for soradns-route-plan: {flag}").into());
                    }
                }
            }
            if names.is_empty() {
                return Err("soradns-route-plan requires at least one --name <fqdn>".into());
            }
            let options = soradns::RoutePlanOptions {
                names,
                include_pretty_hosts,
                generated_at: generated_at.unwrap_or_else(OffsetDateTime::now_utc),
            };
            Ok(CommandKind::SoradnsRoutePlan {
                options,
                output: json_out.unwrap_or(JsonTarget::Stdout),
            })
        }
        "soradns-verify-gar" => {
            let mut gar_path: Option<PathBuf> = None;
            let mut name: Option<String> = None;
            let mut manifest_cid: Option<String> = None;
            let mut manifest_digest: Option<String> = None;
            let mut telemetry_labels: Vec<String> = Vec::new();
            let mut json_out: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--gar" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --gar".into());
                        };
                        gar_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--name" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --name".into());
                        };
                        let trimmed = value.trim();
                        if trimmed.is_empty() {
                            return Err("--name requires a non-empty value".into());
                        }
                        name = Some(trimmed.to_string());
                    }
                    "--manifest-cid" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --manifest-cid".into());
                        };
                        manifest_cid = Some(value);
                    }
                    "--manifest-digest" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --manifest-digest".into());
                        };
                        manifest_digest = Some(value);
                    }
                    "--telemetry-label" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --telemetry-label".into());
                        };
                        if !value.trim().is_empty() {
                            telemetry_labels.push(value);
                        }
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            json_out = Some(JsonTarget::Stdout);
                        } else {
                            json_out = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    flag => {
                        return Err(format!("unknown flag for soradns-verify-gar: {flag}").into());
                    }
                }
            }
            let gar_path =
                gar_path.ok_or("soradns-verify-gar requires --gar <path/to/gar.json>")?;
            let name = name.ok_or("soradns-verify-gar requires --name <fqdn>")?;
            let options = soradns::GarVerifyOptions {
                name,
                expected_manifest_cid: manifest_cid,
                expected_manifest_digest: manifest_digest,
                required_telemetry_labels: telemetry_labels,
            };
            Ok(CommandKind::SoradnsVerifyGar {
                options: SoradnsVerifyGarOptions {
                    gar_path,
                    expectations: options,
                    json_out,
                },
            })
        }
        "soradns-verify-binding" => {
            let mut binding: Option<PathBuf> = None;
            let mut alias: Option<String> = None;
            let mut content_cid: Option<String> = None;
            let mut hostname: Option<String> = None;
            let mut proof_status: Option<String> = None;
            let mut manifest_json: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--binding" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --binding".into());
                        };
                        binding = Some(normalize_path(Path::new(&path))?);
                    }
                    "--alias" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --alias".into());
                        };
                        alias = Some(value.trim().to_string());
                    }
                    "--content-cid" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --content-cid".into());
                        };
                        content_cid = Some(value.trim().to_string());
                    }
                    "--hostname" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --hostname".into());
                        };
                        hostname = Some(value.trim().to_string());
                    }
                    "--proof-status" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --proof-status".into());
                        };
                        proof_status = Some(value.trim().to_string());
                    }
                    "--manifest-json" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --manifest-json".into());
                        };
                        manifest_json = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for soradns-verify-binding: {flag}").into()
                        );
                    }
                }
            }
            let Some(binding_path) = binding else {
                return Err("soradns-verify-binding requires --binding <path>".into());
            };
            Ok(CommandKind::SoradnsVerifyBinding {
                binding: binding_path,
                alias,
                content_cid,
                hostname,
                proof_status,
                manifest: manifest_json,
            })
        }
        "soradns-directory-release" => {
            let mut rad_dir = None;
            let mut output_root = None;
            let mut release_key = None;
            let mut car_cid = None;
            let mut prev_id = None;
            let mut created_at = None;
            let mut note = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--rad-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --rad-dir".into());
                        };
                        rad_dir = Some(PathBuf::from(path));
                    }
                    "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        output_root = Some(normalize_path(Path::new(&path))?);
                    }
                    "--release-key" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --release-key".into());
                        };
                        release_key = Some(normalize_path(Path::new(&path))?);
                    }
                    "--car-cid" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --car-cid".into());
                        };
                        car_cid = Some(value);
                    }
                    "--prev-id" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --prev-id".into());
                        };
                        prev_id = Some(value.to_ascii_lowercase());
                    }
                    "--created-at" => {
                        let Some(value) = pending.next() else {
                            return Err("expected RFC3339 timestamp after --created-at".into());
                        };
                        let timestamp = OffsetDateTime::parse(&value, &Rfc3339)
                            .map_err(|err| format!("invalid --created-at `{value}`: {err}"))?;
                        created_at = Some(timestamp);
                    }
                    "--note" => {
                        let Some(value) = pending.next() else {
                            return Err("expected note after --note".into());
                        };
                        note = Some(value);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for soradns-directory-release: {flag}").into(),
                        );
                    }
                }
            }
            let rad_dir = rad_dir.ok_or("missing required --rad-dir")?;
            let release_key_path = release_key.ok_or("missing required --release-key")?;
            let car_cid = car_cid.ok_or("missing required --car-cid")?;
            let options = soradns::DirectoryReleaseOptions {
                rad_dir,
                output_root: output_root.unwrap_or_else(default_directory_release_output_root),
                release_key_path,
                car_cid,
                prev_id_hex: prev_id,
                created_at,
                note,
            };
            Ok(CommandKind::SoradnsDirectoryRelease {
                options: Box::new(options),
            })
        }
        "soranet-rollout-plan" => {
            let mut regions: Option<String> = None;
            let mut start: Option<OffsetDateTime> = None;
            let mut window_spec: Option<String> = None;
            let mut spacing_spec: Option<String> = None;
            let mut client_offset_spec: Option<String> = None;
            let mut output_json: Option<PathBuf> = None;
            let mut markdown_out: Option<PathBuf> = None;
            let mut label: Option<String> = None;
            let mut environment: Option<String> = None;
            let mut phase = SorafsRolloutPhase::Ramp;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--regions" => {
                        let Some(value) = pending.next() else {
                            return Err("expected comma-separated list after --regions".into());
                        };
                        regions = Some(value);
                    }
                    "--start" => {
                        let Some(value) = pending.next() else {
                            return Err("expected RFC3339 timestamp after --start".into());
                        };
                        let parsed = OffsetDateTime::parse(&value, &Rfc3339)
                            .map_err(|err| format!("invalid --start timestamp `{value}`: {err}"))?;
                        start = Some(parsed);
                    }
                    "--window" => {
                        let Some(value) = pending.next() else {
                            return Err("expected duration after --window".into());
                        };
                        window_spec = Some(value);
                    }
                    "--spacing" => {
                        let Some(value) = pending.next() else {
                            return Err("expected duration after --spacing".into());
                        };
                        spacing_spec = Some(value);
                    }
                    "--client-offset" => {
                        let Some(value) = pending.next() else {
                            return Err("expected duration after --client-offset".into());
                        };
                        client_offset_spec = Some(value);
                    }
                    "--phase" => {
                        let Some(value) = pending.next() else {
                            return Err("expected phase label after --phase".into());
                        };
                        phase = SorafsRolloutPhase::parse(&value).ok_or_else(|| {
                            format!("invalid rollout phase `{value}`; expected canary|ramp|default")
                        })?;
                    }
                    "--label" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --label".into());
                        };
                        label = Some(value);
                    }
                    "--environment" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --environment".into());
                        };
                        environment = Some(value);
                    }
                    "-o" | "--out" | "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out".into());
                        };
                        output_json = Some(normalize_path(Path::new(&path))?);
                    }
                    "--markdown" | "--markdown-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --markdown-out".into());
                        };
                        markdown_out = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for soranet-rollout-plan: {flag}").into());
                    }
                }
            }

            let regions = regions.ok_or("soranet-rollout-plan requires --regions <a,b,...>")?;
            let start = start.ok_or("soranet-rollout-plan requires --start <RFC3339 timestamp>")?;
            let regions_vec: Vec<String> = regions
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string())
                .collect();
            if regions_vec.is_empty() {
                return Err("at least one region must be provided after --regions".into());
            }
            let window =
                soranet_rollout::parse_duration_spec(window_spec.as_deref().unwrap_or("6h"))?;
            let spacing =
                soranet_rollout::parse_duration_spec(spacing_spec.as_deref().unwrap_or("24h"))?;
            let client_offset = match client_offset_spec {
                Some(spec) => Some(soranet_rollout::parse_duration_spec(&spec)?),
                None => None,
            };

            let options = soranet_rollout::PlanOptions {
                label: label.unwrap_or_else(|| "SNNet-16G rollout".to_string()),
                environment: environment.unwrap_or_else(|| "production".to_string()),
                phase,
                start,
                regions: regions_vec,
                window,
                spacing,
                client_offset,
                output_json: output_json.unwrap_or_else(default_rollout_plan_path),
                output_markdown: Some(markdown_out.unwrap_or_else(default_rollout_markdown_path)),
            };
            Ok(CommandKind::SoranetRolloutPlan { options })
        }
        "soranet-rollout-capture" => {
            let mut base_dir: Option<PathBuf> = None;
            let mut label: Option<String> = None;
            let mut environment = String::from("production");
            let mut phase = SorafsRolloutPhase::Ramp;
            let mut log_path: Option<PathBuf> = None;
            let mut artifacts: Vec<soranet_rollout::ArtifactInput> = Vec::new();
            let mut key_path: Option<PathBuf> = None;
            let mut note: Option<String> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out".into());
                        };
                        base_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--label" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --label".into());
                        };
                        label = Some(value);
                    }
                    "--environment" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --environment".into());
                        };
                        environment = value;
                    }
                    "--phase" => {
                        let Some(value) = pending.next() else {
                            return Err("expected phase label after --phase".into());
                        };
                        phase = SorafsRolloutPhase::parse(&value).ok_or_else(|| {
                            format!("invalid rollout phase `{value}`; expected canary|ramp|default")
                        })?;
                    }
                    "--log" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --log".into());
                        };
                        log_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--artifact" => {
                        let Some(spec) = pending.next() else {
                            return Err("expected spec after --artifact".into());
                        };
                        let (kind, path_str) = parse_rollout_artifact_spec(&spec)?;
                        let path = normalize_path(Path::new(&path_str))?;
                        artifacts.push(soranet_rollout::ArtifactInput { kind, path });
                    }
                    "--key" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --key".into());
                        };
                        key_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--note" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --note".into());
                        };
                        note = Some(value);
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for soranet-rollout-capture: {flag}").into()
                        );
                    }
                }
            }

            let log_path = log_path.ok_or("soranet-rollout-capture requires --log <path>")?;
            let key_path = key_path.ok_or("soranet-rollout-capture requires --key <path>")?;
            let options = soranet_rollout::CaptureOptions {
                base_output_dir: base_dir.unwrap_or_else(default_rollout_capture_dir),
                label,
                environment,
                phase,
                log_path,
                additional_artifacts: artifacts,
                key_path,
                note,
            };
            Ok(CommandKind::SoranetRolloutCapture { options })
        }
        "sorafs-pin-fixtures" => {
            let mut output: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "-o" | "--out" | "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for sorafs-pin-fixtures: {flag}").into());
                    }
                }
            }
            let output = output.unwrap_or_else(default_pin_fixture_path);
            Ok(CommandKind::SorafsPinFixtures { output })
        }
        "space-directory" => {
            let Some(subcommand) = args.next() else {
                print_usage();
                return Err("missing space-directory subcommand".into());
            };
            match subcommand.as_str() {
                "encode" => {
                    let mut input: Option<PathBuf> = None;
                    let mut output: Option<PathBuf> = None;
                    while let Some(arg) = args.next() {
                        match arg.as_str() {
                            "--json" => {
                                let value = args
                                    .next()
                                    .ok_or("--json requires a path to the manifest JSON")?;
                                input = Some(normalize_path(Path::new(&value))?);
                            }
                            "--out" | "--output" => {
                                let value = args
                                    .next()
                                    .ok_or("--out/--output requires a destination path")?;
                                output = Some(normalize_path(Path::new(&value))?);
                            }
                            "-h" | "--help" => {
                                print_usage();
                                return Err("".into());
                            }
                            other => {
                                return Err(format!(
                                    "unknown flag `{other}` for space-directory encode"
                                )
                                .into());
                            }
                        }
                    }
                    let input =
                        input.ok_or("space-directory encode requires --json <manifest.json>")?;
                    let output = output.unwrap_or_else(|| {
                        let mut candidate = input.clone();
                        candidate.set_extension("to");
                        candidate
                    });
                    Ok(CommandKind::SpaceDirectoryEncode { input, output })
                }
                other => Err(format!(
                    "unknown space-directory subcommand `{other}` (expected encode)"
                )
                .into()),
            }
        }
        "acceleration-state" => {
            let mut format = AccelerationOutputFormat::Table;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--format" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --format".into());
                        };
                        format = AccelerationOutputFormat::parse(&value)?;
                    }
                    flag => {
                        return Err(format!("unknown flag for acceleration-state: {flag}").into());
                    }
                }
            }
            Ok(CommandKind::AccelerationState { format })
        }
        "sns-scorecard" => {
            let mut input: Option<PathBuf> = None;
            let mut output_json: Option<PathBuf> = None;
            let mut output_markdown: Option<PathBuf> = None;
            let mut output_handoff_json: Option<PathBuf> = None;
            let mut output_handoff_markdown: Option<PathBuf> = None;
            let mut handoff_dir: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--input" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --input".into());
                        };
                        input = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output-json" | "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-json".into());
                        };
                        output_json = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output-markdown" | "--markdown-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output-markdown".into());
                        };
                        output_markdown = Some(normalize_path(Path::new(&path))?);
                    }
                    "--handoff-json" | "--handoff-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --handoff-json".into());
                        };
                        output_handoff_json = Some(normalize_path(Path::new(&path))?);
                    }
                    "--handoff-markdown" | "--handoff-md" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --handoff-markdown".into());
                        };
                        output_handoff_markdown = Some(normalize_path(Path::new(&path))?);
                    }
                    "--handoff-dir" | "--handoffs" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --handoff-dir".into());
                        };
                        handoff_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for sns-scorecard: {flag}").into());
                    }
                }
            }

            let input =
                input.ok_or("sns-scorecard requires --input <path to steward metrics JSON>")?;
            let output_json = output_json.ok_or(
                "sns-scorecard requires --output-json <path> to write the canonical scoreboard",
            )?;
            Ok(CommandKind::SnsScorecard(sns::ScorecardOptions {
                input,
                output_json,
                output_markdown,
                output_handoff_json,
                output_handoff_markdown,
                handoff_dir,
            }))
        }
        "sns-annex" => {
            let mut suffix: Option<String> = None;
            let mut cycle: Option<String> = None;
            let mut dashboard: Option<PathBuf> = None;
            let mut output: Option<PathBuf> = None;
            let mut dashboard_label: Option<String> = None;
            let mut dashboard_artifact: Option<PathBuf> = None;
            let mut regulatory_entry: Option<PathBuf> = None;
            let mut portal_entry: Option<PathBuf> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--suffix" => {
                        let Some(value) = pending.next() else {
                            return Err("expected suffix after --suffix (e.g., .sora)".into());
                        };
                        suffix = Some(value);
                    }
                    "--cycle" => {
                        let Some(value) = pending.next() else {
                            return Err("expected cycle after --cycle (YYYY-MM)".into());
                        };
                        cycle = Some(value);
                    }
                    "--dashboard" | "--dashboard-json" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --dashboard".into());
                        };
                        dashboard = Some(normalize_path(Path::new(&path))?);
                    }
                    "--output" | "--output-markdown" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    "--dashboard-label" | "--dashboard-path-label" => {
                        let Some(value) = pending.next() else {
                            return Err("expected string after --dashboard-label".into());
                        };
                        dashboard_label = Some(value);
                    }
                    "--dashboard-artifact"
                    | "--dashboard-artifact-path"
                    | "--dashboard-copy-to" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --dashboard-artifact".into());
                        };
                        dashboard_artifact = Some(normalize_path(Path::new(&path))?);
                    }
                    "--regulatory-entry" | "--regulatory-memo" => {
                        let Some(path) = pending.next() else {
                            return Err(
                                "expected path after --regulatory-entry (regulatory memo)".into()
                            );
                        };
                        regulatory_entry = Some(normalize_path(Path::new(&path))?);
                    }
                    "--portal-entry" | "--portal-memo" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --portal-entry (portal memo)".into());
                        };
                        portal_entry = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for sns-annex: {flag}").into());
                    }
                }
            }

            let suffix = suffix.ok_or("sns-annex requires --suffix <.suffix>")?;
            let cycle = cycle.ok_or("sns-annex requires --cycle <YYYY-MM>")?;
            let dashboard_path =
                dashboard.ok_or("sns-annex requires --dashboard <path to grafana export>")?;
            let output_markdown = output.unwrap_or_else(|| {
                let mut candidate = workspace_root();
                candidate.push("docs/source/sns/reports");
                candidate.push(&suffix);
                candidate.push(format!("{cycle}.md"));
                candidate
            });

            Ok(CommandKind::SnsAnnex(sns::AnnexOptions {
                suffix,
                cycle,
                dashboard_path,
                output_markdown,
                dashboard_label,
                dashboard_artifact,
                regulatory_entry,
                portal_entry,
            }))
        }
        "sns-catalog-verify" => {
            let mut inputs: Vec<PathBuf> = Vec::new();
            let mut allow_missing_checksum = false;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--input" | "--catalog" | "--json" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --input".into());
                        };
                        inputs.push(normalize_path(Path::new(&path))?);
                    }
                    "--allow-missing-checksum" => {
                        allow_missing_checksum = true;
                    }
                    flag => {
                        return Err(format!("unknown flag for sns-catalog-verify: {flag}").into());
                    }
                }
            }
            Ok(CommandKind::SnsCatalogVerify(sns::CatalogVerifyOptions {
                inputs,
                allow_missing_checksum,
            }))
        }
        "codec" => {
            let Some(subcmd) = args.next() else {
                return Err("missing codec subcommand".into());
            };
            match subcmd.as_str() {
                "rans-tables" => {
                    let mut options = codec::RansTablesOptions::default();
                    let mut pending = args.peekable();
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--seed" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --seed".into());
                                };
                                options.seed = parse_seed(&value)?;
                            }
                            "--bundle-width" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --bundle-width".into());
                                };
                                let width = value.parse::<u8>().map_err(|_| {
                                    format!("invalid bundle width `{value}` (expected integer)")
                                })?;
                                if !(codec::MIN_BUNDLE_WIDTH..=codec::MAX_BUNDLE_WIDTH)
                                    .contains(&width)
                                {
                                    return Err(format!(
                                        "bundle width must be within {}..={}",
                                        codec::MIN_BUNDLE_WIDTH,
                                        codec::MAX_BUNDLE_WIDTH
                                    )
                                    .into());
                                }
                                options.bundle_width = width;
                            }
                            "-o" | "--out" | "--output" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --output".into());
                                };
                                options.output_base = normalize_path(Path::new(&path))?;
                            }
                            "--format" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --format".into());
                                };
                                options.add_format(&value)?;
                            }
                            "--signing-key" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --signing-key".into());
                                };
                                options.signing_key = Some(normalize_path(Path::new(&path))?);
                            }
                            "--verify" => {
                                let mut path = None;
                                if let Some(next) = pending.peek()
                                    && !next.starts_with('-')
                                {
                                    let value = pending.next().expect("peeked value");
                                    path = Some(normalize_path(Path::new(&value))?);
                                }
                                options.verify_paths.push(path.unwrap_or_else(|| {
                                    options.default_output_path(codec::OutputFormat::Json)
                                }));
                            }
                            flag => {
                                return Err(
                                    format!("unknown flag for codec rans-tables: {flag}").into()
                                );
                            }
                        }
                    }
                    options.finalize_formats();
                    Ok(CommandKind::CodecRansTables { options })
                }
                other => Err(format!("unknown codec subcommand: {other}").into()),
            }
        }
        "verify-tables" => {
            let mut paths: Vec<PathBuf> = Vec::new();
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--tables" | "--table" | "-t" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --tables".into());
                        };
                        paths.push(normalize_path(Path::new(&path))?);
                    }
                    flag if flag.starts_with('-') => {
                        return Err(format!("unknown flag for verify-tables: {flag}").into());
                    }
                    value => {
                        paths.push(normalize_path(Path::new(value))?);
                    }
                }
            }
            if paths.is_empty() {
                paths.push(normalize_path(Path::new(
                    "codec/rans/tables/rans_seed0.toml",
                ))?);
            }
            Ok(CommandKind::VerifyRansTables { paths })
        }
        "streaming-bundle-check" => {
            let mut config: Option<PathBuf> = None;
            let mut tables: Option<PathBuf> = None;
            let mut output: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--config" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --config".into());
                        };
                        config = Some(normalize_path(Path::new(&path))?);
                    }
                    "--tables" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --tables".into());
                        };
                        tables = Some(normalize_path(Path::new(&path))?);
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            output = Some(JsonTarget::Stdout);
                        } else {
                            output = Some(JsonTarget::File(normalize_path(Path::new(&path))?));
                        }
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for streaming-bundle-check: {flag}").into()
                        );
                    }
                }
            }
            let config = config
                .ok_or_else(|| "streaming-bundle-check requires --config <path>".to_string())?;
            Ok(CommandKind::StreamingBundleCheck {
                options: StreamingBundleCheckOptions {
                    config_path: config,
                    tables_override: tables,
                    output: output.unwrap_or(JsonTarget::Stdout),
                },
            })
        }
        "streaming-entropy-bench" => {
            let mut options = streaming_bench::EntropyBenchOptions::default();
            let mut output = JsonTarget::File(normalize_path(Path::new(
                "artifacts/nsc/entropy_bench.json",
            ))?);
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--frames" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --frames".into());
                        };
                        options.frames_per_segment = value
                            .parse::<u16>()
                            .map_err(|_| "--frames must be a positive integer".to_string())?;
                    }
                    "--segments" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --segments".into());
                        };
                        options.segments = value
                            .parse::<u16>()
                            .map_err(|_| "--segments must be a positive integer".to_string())?;
                    }
                    "--width" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --width".into());
                        };
                        options.bundle_width = value
                            .parse::<u8>()
                            .map_err(|_| "--width must be between 1 and 255".to_string())?;
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            output = JsonTarget::Stdout;
                        } else {
                            output = JsonTarget::File(normalize_path(Path::new(&path))?);
                        }
                    }
                    "--y4m-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --y4m-out".into());
                        };
                        options.y4m_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--y4m-in" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --y4m-in".into());
                        };
                        options.y4m_in = Some(normalize_path(Path::new(&path))?);
                    }
                    "--chunk-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --chunk-out".into());
                        };
                        options.chunk_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--quantizer" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --quantizer".into());
                        };
                        let quantizer = value
                            .parse::<u8>()
                            .map_err(|_| "--quantizer must be between 1 and 255".to_string())?;
                        if quantizer == 0 {
                            return Err("--quantizer must be at least 1".into());
                        }
                        options.quantizers.push(quantizer);
                    }
                    "--target-bitrate-mbps" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --target-bitrate-mbps".into());
                        };
                        let target = value
                            .parse::<f64>()
                            .map_err(|_| "--target-bitrate-mbps must be positive".to_string())?;
                        if !(target.is_finite() && target > 0.0) {
                            return Err("--target-bitrate-mbps must be positive".into());
                        }
                        options.target_bitrates_mbps.push(target);
                    }
                    "--psnr-mode" => {
                        let Some(mode) = pending.next() else {
                            return Err("expected value after --psnr-mode".into());
                        };
                        options.psnr_mode = match mode.as_str() {
                            "y" => streaming_bench::PsnrMode::Luma,
                            "yuv" => streaming_bench::PsnrMode::Yuv,
                            _ => {
                                return Err(format!(
                                    "invalid psnr-mode '{mode}', expected one of: y, yuv"
                                )
                                .into());
                            }
                        };
                    }
                    "--tiny-clip-preset" => {
                        options.tiny_clip_preset = true;
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for streaming-entropy-bench: {flag}").into()
                        );
                    }
                }
            }
            Ok(CommandKind::StreamingEntropyBench { options, output })
        }
        "streaming-decode" => {
            let mut options = streaming_bench::DecodeOptions {
                bundle_path: PathBuf::new(),
                y4m_out: PathBuf::new(),
                reference_y4m: None,
                psnr_mode: streaming_bench::PsnrMode::Luma,
            };
            let mut output: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--input" | "--bundle" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --bundle".into());
                        };
                        options.bundle_path = normalize_path(Path::new(&path))?;
                    }
                    "--y4m-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --y4m-out".into());
                        };
                        options.y4m_out = normalize_path(Path::new(&path))?;
                    }
                    "--psnr-ref" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --psnr-ref".into());
                        };
                        options.reference_y4m = Some(normalize_path(Path::new(&path))?);
                    }
                    "--psnr-mode" => {
                        let Some(mode) = pending.next() else {
                            return Err("expected value after --psnr-mode".into());
                        };
                        options.psnr_mode = match mode.as_str() {
                            "y" => streaming_bench::PsnrMode::Luma,
                            "yuv" => streaming_bench::PsnrMode::Yuv,
                            _ => {
                                return Err(format!(
                                    "invalid psnr-mode '{mode}', expected one of: y, yuv"
                                )
                                .into());
                            }
                        };
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        output = if path == "-" {
                            Some(JsonTarget::Stdout)
                        } else {
                            Some(JsonTarget::File(normalize_path(Path::new(&path))?))
                        };
                    }
                    flag => return Err(format!("unknown flag for streaming-decode: {flag}").into()),
                }
            }
            if options.bundle_path.as_os_str().is_empty() {
                return Err("streaming-decode requires --bundle <path>".into());
            }
            if options.y4m_out.as_os_str().is_empty() {
                return Err("streaming-decode requires --y4m-out <path>".into());
            }
            Ok(CommandKind::StreamingDecode { options, output })
        }
        "stage1-bench" => {
            let mut sizes: Vec<usize> = vec![4 * 1024, 16 * 1024, 64 * 1024, 128 * 1024];
            let mut iterations: u32 = 5;
            let mut json_out = normalize_path(Path::new("benchmarks/norito_stage1/latest.json"))?;
            let mut markdown_out = Some(normalize_path(Path::new(
                "benchmarks/norito_stage1/latest.md",
            ))?);
            let mut allow_overwrite = false;

            fn parse_size_bytes(raw: &str) -> Result<usize, String> {
                let raw = raw.trim();
                if let Some(stripped) = raw.strip_suffix('k').or_else(|| raw.strip_suffix('K')) {
                    return stripped
                        .parse::<usize>()
                        .map(|n| n * 1024)
                        .map_err(|_| format!("invalid size `{raw}`"));
                }
                if let Some(stripped) = raw.strip_suffix('m').or_else(|| raw.strip_suffix('M')) {
                    return stripped
                        .parse::<usize>()
                        .map(|n| n * 1024 * 1024)
                        .map_err(|_| format!("invalid size `{raw}`"));
                }
                raw.parse::<usize>()
                    .map_err(|_| format!("invalid size `{raw}`"))
            }

            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--size" | "--bytes" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --size".into());
                        };
                        let size = parse_size_bytes(&value)?;
                        sizes.push(size);
                    }
                    "--iterations" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --iterations".into());
                        };
                        iterations = value
                            .parse::<u32>()
                            .map_err(|_| "--iterations must be a positive integer".to_string())?;
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        json_out = normalize_path(Path::new(&path))?;
                    }
                    "--markdown-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --markdown-out".into());
                        };
                        markdown_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--no-markdown" => {
                        markdown_out = None;
                    }
                    "--allow-overwrite" => {
                        allow_overwrite = true;
                    }
                    flag => {
                        return Err(format!("unknown flag for stage1-bench: {flag}").into());
                    }
                }
            }
            sizes.sort_unstable();
            sizes.dedup();
            Ok(CommandKind::Stage1Bench {
                options: stage1_bench::Stage1BenchOptions {
                    sizes,
                    iterations,
                    json_out,
                    markdown_out,
                    allow_overwrite,
                },
            })
        }
        "i3-bench-suite" => {
            let mut iterations: u32 = 64;
            let mut sample_size: u32 = 5;
            let mut json_out = normalize_path(Path::new("benchmarks/i3/latest.json"))?;
            let mut csv_out = Some(normalize_path(Path::new("benchmarks/i3/latest.csv"))?);
            let mut markdown_out = Some(normalize_path(Path::new("benchmarks/i3/latest.md"))?);
            let mut threshold = Some(normalize_path(Path::new("benchmarks/i3/thresholds.json"))?);
            let mut allow_overwrite = false;
            let mut flamegraph_hint = false;

            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--iterations" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --iterations".into());
                        };
                        iterations = value
                            .parse::<u32>()
                            .map_err(|_| "--iterations must be a positive integer".to_string())?;
                    }
                    "--sample-count" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --sample-count".into());
                        };
                        sample_size = value
                            .parse::<u32>()
                            .map_err(|_| "--sample-count must be a positive integer".to_string())?;
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        json_out = normalize_path(Path::new(&path))?;
                    }
                    "--csv-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --csv-out".into());
                        };
                        csv_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--no-csv" => {
                        csv_out = None;
                    }
                    "--markdown-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --markdown-out".into());
                        };
                        markdown_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--no-markdown" => {
                        markdown_out = None;
                    }
                    "--threshold" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --threshold".into());
                        };
                        threshold = Some(normalize_path(Path::new(&path))?);
                    }
                    "--no-threshold" => {
                        threshold = None;
                    }
                    "--allow-overwrite" => {
                        allow_overwrite = true;
                    }
                    "--flamegraph-hint" => {
                        flamegraph_hint = true;
                    }
                    flag => {
                        return Err(format!("unknown flag for i3-bench-suite: {flag}").into());
                    }
                }
            }
            Ok(CommandKind::I3BenchSuite {
                options: i3_bench_suite::I3BenchOptions {
                    iterations,
                    sample_size,
                    json_out,
                    csv_out,
                    markdown_out,
                    allow_overwrite,
                    threshold,
                    flamegraph_hint,
                },
            })
        }
        "i3-slo-harness" => {
            let mut iterations: u32 = 64;
            let mut sample_size: u32 = 5;
            let mut out_dir = normalize_path(Path::new("artifacts/i3_slo/latest"))?;
            let mut budgets = normalize_path(Path::new("benchmarks/i3/slo_budgets.json"))?;
            let mut threshold = normalize_path(Path::new("benchmarks/i3/slo_thresholds.json"))?;
            let mut allow_overwrite = false;
            let mut flamegraph_hint = false;

            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--iterations" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --iterations".into());
                        };
                        iterations = value
                            .parse::<u32>()
                            .map_err(|_| "--iterations must be a positive integer".to_string())?;
                    }
                    "--sample-count" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --sample-count".into());
                        };
                        sample_size = value
                            .parse::<u32>()
                            .map_err(|_| "--sample-count must be a positive integer".to_string())?;
                    }
                    "--out-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out-dir".into());
                        };
                        out_dir = normalize_path(Path::new(&path))?;
                    }
                    "--budget" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --budget".into());
                        };
                        budgets = normalize_path(Path::new(&path))?;
                    }
                    "--threshold" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --threshold".into());
                        };
                        threshold = normalize_path(Path::new(&path))?;
                    }
                    "--allow-overwrite" => {
                        allow_overwrite = true;
                    }
                    "--flamegraph-hint" => {
                        flamegraph_hint = true;
                    }
                    flag => {
                        return Err(format!("unknown flag for i3-slo-harness: {flag}").into());
                    }
                }
            }
            Ok(CommandKind::I3SloHarness {
                options: i3_slo_harness::SloHarnessOptions {
                    iterations,
                    sample_size,
                    out_dir,
                    budgets,
                    threshold,
                    allow_overwrite,
                    flamegraph_hint,
                },
            })
        }
        "poseidon-cuda-bench" => {
            let mut batch_size: usize = 1024;
            let mut iterations: u32 = 32;
            let mut json_out =
                normalize_path(Path::new("benchmarks/poseidon/poseidon_cuda_latest.json"))?;
            let mut markdown_out = Some(normalize_path(Path::new(
                "benchmarks/poseidon/poseidon_cuda_latest.md",
            ))?);
            let mut allow_overwrite = false;

            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--batch-size" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --batch-size".into());
                        };
                        batch_size = value
                            .parse::<usize>()
                            .map_err(|_| "--batch-size must be a positive integer".to_string())?;
                    }
                    "--iterations" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --iterations".into());
                        };
                        iterations = value
                            .parse::<u32>()
                            .map_err(|_| "--iterations must be a positive integer".to_string())?;
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        json_out = normalize_path(Path::new(&path))?;
                    }
                    "--markdown-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --markdown-out".into());
                        };
                        markdown_out = Some(normalize_path(Path::new(&path))?);
                    }
                    "--no-markdown" => {
                        markdown_out = None;
                    }
                    "--allow-overwrite" => {
                        allow_overwrite = true;
                    }
                    flag => {
                        return Err(format!("unknown flag for poseidon-cuda-bench: {flag}").into());
                    }
                }
            }
            Ok(CommandKind::PoseidonBench {
                options: poseidon_bench::PoseidonBenchOptions {
                    batch_size,
                    iterations,
                    json_out,
                    markdown_out,
                    allow_overwrite,
                },
            })
        }
        "streaming-context-remap" => {
            let mut input: Option<PathBuf> = None;
            let mut top = 32usize;
            let mut output = JsonTarget::Stdout;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--input" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --input".into());
                        };
                        input = Some(normalize_path(Path::new(&path))?);
                    }
                    "--top" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --top".into());
                        };
                        top = value
                            .parse::<usize>()
                            .map_err(|_| "--top must be a positive integer".to_string())?;
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        output = if path == "-" {
                            JsonTarget::Stdout
                        } else {
                            JsonTarget::File(normalize_path(Path::new(&path))?)
                        };
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for streaming-context-remap: {flag}").into()
                        );
                    }
                }
            }
            let input = input
                .ok_or_else(|| "streaming-context-remap requires --input <path>".to_string())?;
            Ok(CommandKind::StreamingContextRemap {
                options: StreamingContextRemapOptions { input, top, output },
            })
        }
        "norito-rpc-fixtures" => {
            let mut fixtures_json: Option<PathBuf> = None;
            let mut exporter_manifest: Option<PathBuf> = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut selection_manifest: Option<PathBuf> = None;
            let mut include_all = false;
            let mut check_encoded = true;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--fixtures" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --fixtures".into());
                        };
                        fixtures_json = Some(normalize_path(Path::new(&path))?);
                    }
                    "--exporter" | "--exporter-manifest" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --exporter".into());
                        };
                        exporter_manifest = Some(normalize_path(Path::new(&path))?);
                    }
                    "-o" | "--out" | "--output" | "--out-dir" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --out-dir".into());
                        };
                        output_dir = Some(normalize_path(Path::new(&path))?);
                    }
                    "--selection" | "--selection-manifest" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --selection".into());
                        };
                        selection_manifest = Some(normalize_path(Path::new(&path))?);
                    }
                    "--all" => {
                        include_all = true;
                    }
                    "--skip-encoded-check" => {
                        check_encoded = false;
                    }
                    flag => {
                        return Err(format!("unknown flag for norito-rpc-fixtures: {flag}").into());
                    }
                }
            }
            Ok(CommandKind::NoritoRpcFixtures {
                options: NoritoRpcFixtureOptions {
                    fixtures_json,
                    exporter_manifest,
                    output_dir,
                    selection_manifest,
                    include_all,
                    check_encoded,
                },
            })
        }
        "norito-rpc-verify" => {
            let mut json_out: Option<JsonTarget> = None;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        json_out = Some(if path == "-" {
                            JsonTarget::Stdout
                        } else {
                            JsonTarget::File(normalize_path(Path::new(&path))?)
                        });
                    }
                    flag => {
                        return Err(format!("unknown flag for norito-rpc-verify: {flag}").into());
                    }
                }
            }
            Ok(CommandKind::NoritoRpcVerify { json_out })
        }
        "sm-wycheproof-sync" => {
            let mut input: Option<sm::WycheproofInput> = None;
            let mut output: Option<PathBuf> = None;
            let mut generator_version: Option<String> = None;
            let mut pretty = true;
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--input" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --input".into());
                        };
                        if input.is_some() {
                            return Err(
                                "sm-wycheproof-sync accepts only one of --input/--input-url/--input-official"
                                    .into(),
                            );
                        }
                        input = Some(sm::WycheproofInput::File(normalize_path(Path::new(&path))?));
                    }
                    "--input-url" => {
                        let Some(url) = pending.next() else {
                            return Err("expected URL after --input-url".into());
                        };
                        if input.is_some() {
                            return Err(
                                "sm-wycheproof-sync accepts only one of --input/--input-url/--input-official"
                                    .into(),
                            );
                        }
                        input = Some(sm::WycheproofInput::Url(url));
                    }
                    "--input-official" => {
                        if input.is_some() {
                            return Err(
                                "sm-wycheproof-sync accepts only one of --input/--input-url/--input-official"
                                    .into(),
                            );
                        }
                        input = Some(sm::WycheproofInput::Official);
                    }
                    "--output" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --output".into());
                        };
                        output = Some(normalize_path(Path::new(&path))?);
                    }
                    "--generator-version" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --generator-version".into());
                        };
                        generator_version = Some(value);
                    }
                    "--minify" => pretty = false,
                    "--pretty" => pretty = true,
                    flag => {
                        return Err(format!("unknown flag for sm-wycheproof-sync: {flag}").into());
                    }
                }
            }
            let Some(input) = input else {
                return Err(
                    "sm-wycheproof-sync requires one of --input <path>, --input-url <url>, or --input-official"
                        .into(),
                );
            };
            let output = output.unwrap_or_else(default_sm_wycheproof_path);
            Ok(CommandKind::SmWycheproofSync(sm::WycheproofSyncOptions {
                input,
                output,
                generator_version,
                pretty,
            }))
        }
        "sm-operator-snippet" => {
            let mut options = sm::SmOperatorSnippetOptions::default();
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--distid" => {
                        let Some(value) = pending.next() else {
                            return Err("expected value after --distid".into());
                        };
                        options.distid = Some(value);
                    }
                    "--seed-hex" => {
                        let Some(value) = pending.next() else {
                            return Err("expected hex string after --seed-hex".into());
                        };
                        options.seed_hex = Some(value);
                    }
                    "--json-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --json-out".into());
                        };
                        if path == "-" {
                            options.json_out = Some(sm::OutputTarget::Stdout);
                        } else {
                            options.json_out =
                                Some(sm::OutputTarget::file(normalize_path(Path::new(&path))?));
                        }
                    }
                    "--snippet-out" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --snippet-out".into());
                        };
                        if path == "-" {
                            options.snippet_out = Some(sm::OutputTarget::Stdout);
                        } else {
                            options.snippet_out =
                                Some(sm::OutputTarget::file(normalize_path(Path::new(&path))?));
                        }
                    }
                    flag => {
                        return Err(format!("unknown flag for sm-operator-snippet: {flag}").into());
                    }
                }
            }
            Ok(CommandKind::SmOperatorSnippet { options })
        }
        "iso-bridge-lint" => {
            let mut options = IsoLintOptions::default();
            let mut pending = args.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--isin" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --isin".into());
                        };
                        options.isin_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--bic-lei" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --bic-lei".into());
                        };
                        options.bic_lei_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--mic" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --mic".into());
                        };
                        options.mic_path = Some(normalize_path(Path::new(&path))?);
                    }
                    "--fixtures" => {
                        let Some(path) = pending.next() else {
                            return Err("expected path after --fixtures".into());
                        };
                        options.fixtures_path = Some(normalize_path(Path::new(&path))?);
                    }
                    flag => {
                        return Err(format!("unknown flag for iso-bridge-lint: {flag}").into());
                    }
                }
            }
            Ok(CommandKind::IsoBridgeLint { options })
        }
        "ministry-transparency" => {
            let mut pending = args.peekable();
            let Some(subcommand) = pending.next() else {
                return Err(
                    "ministry-transparency requires a subcommand (ingest|build|sanitize|anchor|volunteer-validate)"
                        .into(),
                );
            };
            match subcommand.as_str() {
                "ingest" => {
                    let mut quarter = None;
                    let mut ledger = None;
                    let mut appeals = None;
                    let mut denylist = None;
                    let mut treasury = None;
                    let mut volunteer = None;
                    let mut red_team_reports = Vec::new();
                    let mut panel_proposal = None;
                    let mut panel_ai_manifest = None;
                    let mut panel_round = None;
                    let mut panel_summary_out = None;
                    let mut panel_language = None;
                    let mut panel_generated_at = None;
                    let mut output = None;
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--quarter" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --quarter".into());
                                };
                                quarter = Some(value);
                            }
                            "--ledger" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --ledger".into());
                                };
                                ledger = Some(normalize_path(Path::new(&path))?);
                            }
                            "--appeals" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --appeals".into());
                                };
                                appeals = Some(normalize_path(Path::new(&path))?);
                            }
                            "--denylist" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --denylist".into());
                                };
                                denylist = Some(normalize_path(Path::new(&path))?);
                            }
                            "--treasury" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --treasury".into());
                                };
                                treasury = Some(normalize_path(Path::new(&path))?);
                            }
                            "--volunteer" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --volunteer".into());
                                };
                                volunteer = Some(normalize_path(Path::new(&path))?);
                            }
                            "--red-team-report" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --red-team-report".into());
                                };
                                red_team_reports.push(normalize_path(Path::new(&path))?);
                            }
                            "--panel-proposal" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --panel-proposal".into());
                                };
                                panel_proposal = Some(normalize_path(Path::new(&path))?);
                            }
                            "--panel-ai-manifest" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --panel-ai-manifest".into());
                                };
                                panel_ai_manifest = Some(normalize_path(Path::new(&path))?);
                            }
                            "--panel-round" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --panel-round".into());
                                };
                                panel_round = Some(value);
                            }
                            "--panel-summary-out" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --panel-summary-out".into());
                                };
                                panel_summary_out = Some(normalize_path(Path::new(&path))?);
                            }
                            "--panel-language" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --panel-language".into());
                                };
                                panel_language = Some(value);
                            }
                            "--panel-generated-at" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --panel-generated-at".into());
                                };
                                let millis: u64 = value.parse().map_err(|err| {
                                    format!("invalid --panel-generated-at value `{value}`: {err}")
                                })?;
                                panel_generated_at = Some(millis);
                            }
                            "-o" | "--out" | "--output" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --output".into());
                                };
                                output = Some(normalize_path(Path::new(&path))?);
                            }
                            flag => {
                                return Err(format!(
                                    "unknown flag for ministry-transparency ingest: {flag}"
                                )
                                .into());
                            }
                        }
                    }
                    let quarter = quarter
                        .ok_or("ministry-transparency ingest requires --quarter <YYYY-Q>")?;
                    let ledger =
                        ledger.ok_or("ministry-transparency ingest requires --ledger <path>")?;
                    let appeals =
                        appeals.ok_or("ministry-transparency ingest requires --appeals <path>")?;
                    let denylist = denylist
                        .ok_or("ministry-transparency ingest requires --denylist <path>")?;
                    let treasury = treasury
                        .ok_or("ministry-transparency ingest requires --treasury <path>")?;
                    let output =
                        output.ok_or("ministry-transparency ingest requires --output <path>")?;
                    let panel_summary = match (
                        panel_proposal,
                        panel_ai_manifest,
                        panel_round,
                        panel_summary_out,
                    ) {
                        (None, None, None, None) => None,
                        (Some(proposal), Some(ai_manifest), Some(round), Some(summary_out)) => {
                            let volunteer_path = volunteer.clone().ok_or(
                                "ministry-transparency ingest --panel-* flags require --volunteer <path>",
                            )?;
                            Some(ministry::PanelSummaryRequest {
                                proposal_path: proposal,
                                volunteer_path,
                                ai_manifest_path: ai_manifest,
                                panel_round_id: round,
                                output_path: summary_out,
                                language_override: panel_language,
                                generated_at_unix_ms: panel_generated_at,
                            })
                        }
                        _ => {
                            return Err("ministry-transparency ingest requires all of --panel-proposal/--panel-ai-manifest/--panel-round/--panel-summary-out when any are supplied".into());
                        }
                    };
                    Ok(CommandKind::MinistryTransparency(
                        ministry::Command::Ingest(Box::new(ministry::IngestOptions {
                            quarter,
                            ledger_path: ledger,
                            appeals_path: appeals,
                            denylist_path: denylist,
                            treasury_path: treasury,
                            volunteer_path: volunteer,
                            red_team_reports,
                            panel_summary,
                            output_path: output,
                        })),
                    ))
                }
                "build" => {
                    let mut ingest = None;
                    let mut metrics_out = None;
                    let mut manifest_out = None;
                    let mut note = None;
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--ingest" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --ingest".into());
                                };
                                ingest = Some(normalize_path(Path::new(&path))?);
                            }
                            "--metrics-out" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --metrics-out".into());
                                };
                                metrics_out = Some(normalize_path(Path::new(&path))?);
                            }
                            "--manifest-out" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --manifest-out".into());
                                };
                                manifest_out = Some(normalize_path(Path::new(&path))?);
                            }
                            "--note" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --note".into());
                                };
                                note = Some(value);
                            }
                            flag => {
                                return Err(format!(
                                    "unknown flag for ministry-transparency build: {flag}"
                                )
                                .into());
                            }
                        }
                    }
                    let ingest =
                        ingest.ok_or("ministry-transparency build requires --ingest <path>")?;
                    let metrics_out = metrics_out
                        .ok_or("ministry-transparency build requires --metrics-out <path>")?;
                    let manifest_out = manifest_out
                        .ok_or("ministry-transparency build requires --manifest-out <path>")?;
                    Ok(CommandKind::MinistryTransparency(ministry::Command::Build(
                        Box::new(ministry::BuildOptions {
                            ingest_path: ingest,
                            metrics_output: metrics_out,
                            manifest_output: manifest_out,
                            note,
                        }),
                    )))
                }
                "sanitize" => {
                    let mut ingest = None;
                    let mut output = None;
                    let mut report = None;
                    let mut epsilon_counts = 0.75;
                    let mut epsilon_accuracy = 0.5;
                    let mut delta = 1e-6;
                    let mut suppress_threshold = 5;
                    let mut min_accuracy_samples = 100;
                    let mut seed = None;
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--ingest" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --ingest".into());
                                };
                                ingest = Some(normalize_path(Path::new(&path))?);
                            }
                            "--output" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --output".into());
                                };
                                output = Some(normalize_path(Path::new(&path))?);
                            }
                            "--report" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --report".into());
                                };
                                report = Some(normalize_path(Path::new(&path))?);
                            }
                            "--epsilon-counts" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --epsilon-counts".into());
                                };
                                epsilon_counts = value.parse()?;
                            }
                            "--epsilon-accuracy" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --epsilon-accuracy".into());
                                };
                                epsilon_accuracy = value.parse()?;
                            }
                            "--delta" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --delta".into());
                                };
                                delta = value.parse()?;
                            }
                            "--suppress-threshold" => {
                                let Some(value) = pending.next() else {
                                    return Err(
                                        "expected value after --suppress-threshold".into()
                                    );
                                };
                                suppress_threshold = value.parse()?;
                            }
                            "--min-accuracy-samples" => {
                                let Some(value) = pending.next() else {
                                    return Err(
                                        "expected value after --min-accuracy-samples".into()
                                    );
                                };
                                min_accuracy_samples = value.parse()?;
                            }
                            "--seed" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --seed".into());
                                };
                                seed = Some(value.parse()?);
                            }
                            flag => {
                                return Err(format!(
                                    "unknown flag for ministry-transparency sanitize: {flag}"
                                )
                                .into());
                            }
                        }
                    }
                    let ingest =
                        ingest.ok_or("ministry-transparency sanitize requires --ingest <path>")?;
                    let output =
                        output.ok_or("ministry-transparency sanitize requires --output <path>")?;
                    let report =
                        report.ok_or("ministry-transparency sanitize requires --report <path>")?;
                    Ok(CommandKind::MinistryTransparency(
                        ministry::Command::Sanitize(Box::new(ministry::SanitizeOptions {
                            ingest_path: ingest,
                            output_path: output,
                            report_path: report,
                            epsilon_counts,
                            epsilon_accuracy,
                            delta,
                            suppress_threshold,
                            min_accuracy_samples,
                            seed,
                        })),
                    ))
                }
                "anchor" => {
                    let mut action = None;
                    let mut governance_dir = None;
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--action" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --action".into());
                                };
                                action = Some(normalize_path(Path::new(&path))?);
                            }
                            "--governance-dir" => {
                                let Some(path) = pending.next() else {
                                    return Err(
                                        "expected path after --governance-dir".into()
                                    );
                                };
                                governance_dir = Some(normalize_path(Path::new(&path))?);
                            }
                            flag => {
                                return Err(format!(
                                    "unknown flag for ministry-transparency anchor: {flag}"
                                )
                                .into());
                            }
                        }
                    }
                    let action_path =
                        action.ok_or("ministry-transparency anchor requires --action <path>")?;
                    let governance_dir = governance_dir.ok_or(
                        "ministry-transparency anchor requires --governance-dir <path>",
                    )?;
                    Ok(CommandKind::MinistryTransparency(ministry::Command::Anchor(
                        Box::new(ministry::AnchorOptions {
                            action_path,
                            governance_dir,
                        }),
                    )))
                }
                "volunteer-validate" => {
                    let mut inputs: Vec<PathBuf> = Vec::new();
                    let mut json_output = None;
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--input" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --input".into());
                                };
                                inputs.push(normalize_path(Path::new(&path))?);
                            }
                            "--json-output" => {
                                let Some(path) = pending.next() else {
                                    return Err(
                                        "expected path after --json-output".into()
                                    );
                                };
                                json_output = Some(normalize_path(Path::new(&path))?);
                            }
                            flag => {
                                return Err(format!(
                                    "unknown flag for ministry-transparency volunteer-validate: {flag}"
                                )
                                .into());
                            }
                        }
                    }
                    Ok(CommandKind::MinistryTransparency(
                        ministry::Command::VolunteerValidate(Box::new(
                            ministry::VolunteerValidateOptions {
                                inputs,
                                json_output,
                            },
                        )),
                    ))
                }
                other => Err(format!(
                    "unknown ministry-transparency subcommand `{other}` (expected ingest|build|sanitize|anchor|volunteer-validate)"
                )
                .into()),
            }
        }
        "ministry-agenda" => {
            let mut pending = args.peekable();
            let Some(subcommand) = pending.next() else {
                return Err(
                    "ministry-agenda requires a subcommand (validate|sortition|impact)".into(),
                );
            };
            match subcommand.as_str() {
                "validate" => {
                    let mut proposal = None;
                    let mut registry = None;
                    let mut allow_conflicts = false;
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--proposal" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --proposal".into());
                                };
                                proposal = Some(normalize_path(Path::new(&path))?);
                            }
                            "--registry" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --registry".into());
                                };
                                registry = Some(normalize_path(Path::new(&path))?);
                            }
                            "--allow-registry-conflicts" => allow_conflicts = true,
                            flag => {
                                return Err(format!(
                                    "unknown flag for ministry-agenda validate: {flag}"
                                )
                                .into());
                            }
                        }
                    }
                    let proposal =
                        proposal.ok_or("ministry-agenda validate requires --proposal <path>")?;
                    Ok(CommandKind::MinistryAgenda(
                        ministry_agenda::Command::Validate(ministry_agenda::ValidateOptions {
                            proposal_path: proposal,
                            registry_path: registry,
                            allow_registry_conflicts: allow_conflicts,
                        }),
                    ))
                }
                "sortition" => {
                    let mut roster = None;
                    let mut slots = None;
                    let mut seed = None;
                    let mut output = None;
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--roster" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --roster".into());
                                };
                                roster = Some(normalize_path(Path::new(&path))?);
                            }
                            "--slots" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --slots".into());
                                };
                                let parsed: usize = value.parse().map_err(|err| {
                                    format!("invalid --slots value `{value}`: {err}")
                                })?;
                                slots = Some(parsed);
                            }
                            "--seed" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --seed".into());
                                };
                                seed = Some(value);
                            }
                            "-o" | "--out" | "--output" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --out/--output".into());
                                };
                                output = Some(normalize_path(Path::new(&path))?);
                            }
                            flag => {
                                return Err(format!(
                                    "unknown flag for ministry-agenda sortition: {flag}"
                                )
                                .into());
                            }
                        }
                    }
                    let roster_path =
                        roster.ok_or("ministry-agenda sortition requires --roster <path>")?;
                    let slots =
                        slots.ok_or("ministry-agenda sortition requires --slots <count>")?;
                    let seed_hex = seed.ok_or("ministry-agenda sortition requires --seed <hex>")?;
                    Ok(CommandKind::MinistryAgenda(
                        ministry_agenda::Command::Sortition(ministry_agenda::SortitionOptions {
                            roster_path,
                            slots,
                            seed_hex,
                            output_path: output,
                        }),
                    ))
                }
                "impact" => {
                    let mut options = ministry_agenda::ImpactOptions::default();
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--proposal" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --proposal".into());
                                };
                                options
                                    .proposal_paths
                                    .push(normalize_path(Path::new(&path))?);
                            }
                            "--proposal-dir" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --proposal-dir".into());
                                };
                                options
                                    .proposal_dirs
                                    .push(normalize_path(Path::new(&path))?);
                            }
                            "--registry" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --registry".into());
                                };
                                options.registry_path = Some(normalize_path(Path::new(&path))?);
                            }
                            "--policy-snapshot" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --policy-snapshot".into());
                                };
                                options.policy_snapshot_path =
                                    Some(normalize_path(Path::new(&path))?);
                            }
                            "-o" | "--out" | "--output" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --out/--output".into());
                                };
                                options.output_path = Some(normalize_path(Path::new(&path))?);
                            }
                            flag => {
                                return Err(format!(
                                    "unknown flag for ministry-agenda impact: {flag}"
                                )
                                .into());
                            }
                        }
                    }
                    if options.proposal_paths.is_empty() && options.proposal_dirs.is_empty() {
                        return Err(
                            "ministry-agenda impact requires at least one --proposal <path> or --proposal-dir <dir>"
                                .into(),
                        );
                    }
                    Ok(CommandKind::MinistryAgenda(
                        ministry_agenda::Command::impact(options),
                    ))
                }
                other => Err(format!(
                    "unknown ministry-agenda subcommand `{other}` (expected validate|sortition|impact)"
                )
                .into()),
            }
        }
        "ministry-panel" => {
            let mut pending = args.peekable();
            let Some(subcommand) = pending.next() else {
                return Err("ministry-panel requires a subcommand (synthesize|packet)".into());
            };
            match subcommand.as_str() {
                "synthesize" => {
                    let mut proposal = None;
                    let mut volunteer = None;
                    let mut ai_manifest = None;
                    let mut output = None;
                    let mut panel_round = None;
                    let mut language = None;
                    let mut generated_at = None;
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--proposal" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --proposal".into());
                                };
                                proposal = Some(normalize_path(Path::new(&path))?);
                            }
                            "--volunteer" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --volunteer".into());
                                };
                                volunteer = Some(normalize_path(Path::new(&path))?);
                            }
                            "--ai-manifest" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --ai-manifest".into());
                                };
                                ai_manifest = Some(normalize_path(Path::new(&path))?);
                            }
                            "--output" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --output".into());
                                };
                                let resolved = if path == "-" {
                                    PathBuf::from("-")
                                } else {
                                    normalize_path(Path::new(&path))?
                                };
                                output = Some(resolved);
                            }
                            "--panel-round" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --panel-round".into());
                                };
                                panel_round = Some(value);
                            }
                            "--language" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --language".into());
                                };
                                language = Some(value);
                            }
                            "--generated-at" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --generated-at".into());
                                };
                                let millis = value.parse::<u64>().map_err(|err| {
                                    format!("invalid --generated-at value `{value}`: {err}")
                                })?;
                                generated_at = Some(millis);
                            }
                            flag => {
                                return Err(format!(
                                    "unknown flag for ministry-panel synthesize: {flag}"
                                )
                                .into());
                            }
                        }
                    }
                    let proposal_path =
                        proposal.ok_or("ministry-panel synthesize requires --proposal <path>")?;
                    let volunteer_path =
                        volunteer.ok_or("ministry-panel synthesize requires --volunteer <path>")?;
                    let ai_manifest_path = ai_manifest
                        .ok_or("ministry-panel synthesize requires --ai-manifest <path>")?;
                    let output_path =
                        output.ok_or("ministry-panel synthesize requires --output <path>")?;
                    let panel_round_id = panel_round
                        .ok_or("ministry-panel synthesize requires --panel-round <id>")?;
                    Ok(CommandKind::MinistryPanel(
                        ministry_panel::Command::Synthesize(ministry_panel::SynthesizeOptions {
                            proposal_path,
                            volunteer_path,
                            ai_manifest_path,
                            output_path,
                            panel_round_id,
                            language_override: language,
                            generated_at_unix_ms: generated_at,
                        }),
                    ))
                }
                "packet" => {
                    let mut proposal = None;
                    let mut volunteer = None;
                    let mut ai_manifest = None;
                    let mut panel_round = None;
                    let mut language = None;
                    let mut generated_at = None;
                    let mut sortition = None;
                    let mut impact = None;
                    let mut summary_out = None;
                    let mut output = None;
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--proposal" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --proposal".into());
                                };
                                proposal = Some(normalize_path(Path::new(&path))?);
                            }
                            "--volunteer" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --volunteer".into());
                                };
                                volunteer = Some(normalize_path(Path::new(&path))?);
                            }
                            "--ai-manifest" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --ai-manifest".into());
                                };
                                ai_manifest = Some(normalize_path(Path::new(&path))?);
                            }
                            "--panel-round" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --panel-round".into());
                                };
                                panel_round = Some(value);
                            }
                            "--language" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --language".into());
                                };
                                language = Some(value);
                            }
                            "--generated-at" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --generated-at".into());
                                };
                                let millis = value.parse::<u64>().map_err(|err| {
                                    format!("invalid --generated-at value `{value}`: {err}")
                                })?;
                                generated_at = Some(millis);
                            }
                            "--sortition" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --sortition".into());
                                };
                                sortition = Some(normalize_path(Path::new(&path))?);
                            }
                            "--impact" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --impact".into());
                                };
                                impact = Some(normalize_path(Path::new(&path))?);
                            }
                            "--summary-out" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --summary-out".into());
                                };
                                let resolved = if path == "-" {
                                    PathBuf::from("-")
                                } else {
                                    normalize_path(Path::new(&path))?
                                };
                                summary_out = Some(resolved);
                            }
                            "--output" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --output".into());
                                };
                                let resolved = if path == "-" {
                                    PathBuf::from("-")
                                } else {
                                    normalize_path(Path::new(&path))?
                                };
                                output = Some(resolved);
                            }
                            flag => {
                                return Err(format!(
                                    "unknown flag for ministry-panel packet: {flag}"
                                )
                                .into());
                            }
                        }
                    }
                    let proposal_path =
                        proposal.ok_or("ministry-panel packet requires --proposal <path>")?;
                    let volunteer_path =
                        volunteer.ok_or("ministry-panel packet requires --volunteer <path>")?;
                    let ai_manifest_path =
                        ai_manifest.ok_or("ministry-panel packet requires --ai-manifest <path>")?;
                    let panel_round_id =
                        panel_round.ok_or("ministry-panel packet requires --panel-round <id>")?;
                    let sortition_path =
                        sortition.ok_or("ministry-panel packet requires --sortition <path>")?;
                    let impact_path =
                        impact.ok_or("ministry-panel packet requires --impact <path>")?;
                    let output_path =
                        output.ok_or("ministry-panel packet requires --output <path>")?;
                    let synth = ministry_panel::SynthesizeOptions {
                        proposal_path,
                        volunteer_path,
                        ai_manifest_path,
                        output_path: PathBuf::from("-"),
                        panel_round_id,
                        language_override: language,
                        generated_at_unix_ms: generated_at,
                    };
                    Ok(CommandKind::MinistryPanel(ministry_panel::Command::Packet(
                        ministry_panel::PacketOptions {
                            synth,
                            sortition_summary_path: sortition_path,
                            impact_report_path: impact_path,
                            output_path,
                            summary_out_path: summary_out,
                        },
                    )))
                }
                other => Err(format!(
                    "unknown ministry-panel subcommand `{other}` (expected synthesize|packet)"
                )
                .into()),
            }
        }
        "ministry-jury" => {
            let mut pending = args.peekable();
            let Some(subcommand) = pending.next() else {
                return Err("ministry-jury requires a subcommand (sortition|ballot)".into());
            };
            match subcommand.as_str() {
                "sortition" => {
                    let mut roster = None;
                    let mut proposal_id = None;
                    let mut round_id = None;
                    let mut beacon = None;
                    let mut committee = None;
                    let mut waitlist = None;
                    let mut drawn_at = None;
                    let mut waitlist_ttl_hours: u32 = 72;
                    let mut grace_period_secs: u32 = 900;
                    let mut failover_grace_secs: u32 = 600;
                    let mut output = None;
                    while let Some(arg) = pending.next() {
                        match arg.as_str() {
                            "--roster" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --roster".into());
                                };
                                roster = Some(normalize_path(Path::new(&path))?);
                            }
                            "--proposal" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --proposal".into());
                                };
                                proposal_id = Some(value);
                            }
                            "--round" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --round".into());
                                };
                                round_id = Some(value);
                            }
                            "--beacon" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --beacon".into());
                                };
                                beacon = Some(value);
                            }
                            "--committee-size" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --committee-size".into());
                                };
                                let parsed = value.parse::<usize>().map_err(|err| {
                                    format!("invalid --committee-size value `{value}`: {err}")
                                })?;
                                committee = Some(parsed);
                            }
                            "--waitlist-size" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --waitlist-size".into());
                                };
                                let parsed = value.parse::<usize>().map_err(|err| {
                                    format!("invalid --waitlist-size value `{value}`: {err}")
                                })?;
                                waitlist = Some(parsed);
                            }
                            "--drawn-at" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --drawn-at".into());
                                };
                                let timestamp = ministry_jury::parse_drawn_at(&value)?;
                                drawn_at = Some(timestamp);
                            }
                            "--waitlist-ttl-hours" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --waitlist-ttl-hours".into());
                                };
                                waitlist_ttl_hours = value.parse::<u32>().map_err(|err| {
                                    format!("invalid --waitlist-ttl-hours value `{value}`: {err}")
                                })?;
                            }
                            "--grace-period" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --grace-period".into());
                                };
                                grace_period_secs = value.parse::<u32>().map_err(|err| {
                                    format!("invalid --grace-period value `{value}`: {err}")
                                })?;
                            }
                            "--failover-grace" => {
                                let Some(value) = pending.next() else {
                                    return Err("expected value after --failover-grace".into());
                                };
                                failover_grace_secs = value.parse::<u32>().map_err(|err| {
                                    format!("invalid --failover-grace value `{value}`: {err}")
                                })?;
                            }
                            "--out" => {
                                let Some(path) = pending.next() else {
                                    return Err("expected path after --out".into());
                                };
                                output = Some(normalize_path(Path::new(&path))?);
                            }
                            flag => {
                                return Err(format!(
                                    "unknown flag for ministry-jury sortition: {flag}"
                                )
                                .into());
                            }
                        }
                    }
                    let roster = roster
                        .ok_or_else(|| eyre!("ministry-jury sortition requires --roster <path>"))
                        .map_err(|err| -> Box<dyn Error> { err.into() })?;
                    let proposal_id = proposal_id
                        .ok_or_else(|| eyre!("ministry-jury sortition requires --proposal <id>"))
                        .map_err(|err| -> Box<dyn Error> { err.into() })?;
                    let round_id = round_id
                        .ok_or_else(|| eyre!("ministry-jury sortition requires --round <id>"))
                        .map_err(|err| -> Box<dyn Error> { err.into() })?;
                    let beacon = beacon
                        .ok_or_else(|| eyre!("ministry-jury sortition requires --beacon <hex>"))
                        .map_err(|err| -> Box<dyn Error> { err.into() })?;
                    let committee = committee
                        .ok_or_else(|| {
                            eyre!("ministry-jury sortition requires --committee-size <count>")
                        })
                        .map_err(|err| -> Box<dyn Error> { err.into() })?;
                    let waitlist = waitlist
                        .ok_or_else(|| {
                            eyre!("ministry-jury sortition requires --waitlist-size <count>")
                        })
                        .map_err(|err| -> Box<dyn Error> { err.into() })?;
                    let drawn_at = drawn_at
                        .ok_or_else(|| {
                            eyre!("ministry-jury sortition requires --drawn-at <RFC3339>")
                        })
                        .map_err(|err| -> Box<dyn Error> { err.into() })?;
                    Ok(CommandKind::MinistryJury(
                        ministry_jury::Command::Sortition(ministry_jury::SortitionOptions {
                            roster_path: roster,
                            proposal_id,
                            round_id,
                            beacon_hex: beacon,
                            committee_size: committee,
                            waitlist_size: waitlist,
                            drawn_at,
                            waitlist_ttl_hours,
                            grace_period_secs,
                            failover_grace_secs,
                            output_path: output,
                        }),
                    ))
                }
                "ballot" => {
                    let Some(action) = pending.next() else {
                        return Err(
                            "ministry-jury ballot requires a subcommand (commit|verify)".into()
                        );
                    };
                    match action.as_str() {
                        "commit" => {
                            let mut proposal_id = None;
                            let mut round_id = None;
                            let mut juror_id = None;
                            let mut choice_literal: Option<String> = None;
                            let mut nonce_hex: Option<String> = None;
                            let mut committed_at: Option<OffsetDateTime> = None;
                            let mut revealed_at: Option<OffsetDateTime> = None;
                            let mut output = None;
                            let mut reveal_output = None;
                            while let Some(flag) = pending.next() {
                                match flag.as_str() {
                                    "--proposal" => {
                                        let Some(value) = pending.next() else {
                                            return Err(
                                                "expected value after --proposal".into()
                                            );
                                        };
                                        proposal_id = Some(value);
                                    }
                                    "--round" => {
                                        let Some(value) = pending.next() else {
                                            return Err("expected value after --round".into());
                                        };
                                        round_id = Some(value);
                                    }
                                    "--juror" => {
                                        let Some(value) = pending.next() else {
                                            return Err("expected value after --juror".into());
                                        };
                                        juror_id = Some(value);
                                    }
                                    "--choice" => {
                                        let Some(value) = pending.next() else {
                                            return Err("expected value after --choice".into());
                                        };
                                        choice_literal = Some(value);
                                    }
                                    "--nonce-hex" => {
                                        let Some(value) = pending.next() else {
                                            return Err(
                                                "expected value after --nonce-hex".into()
                                            );
                                        };
                                        nonce_hex = Some(value);
                                    }
                                    "--committed-at" => {
                                        let Some(value) = pending.next() else {
                                            return Err(
                                                "expected value after --committed-at".into()
                                            );
                                        };
                                        committed_at =
                                            Some(ministry_jury::parse_ballot_timestamp(
                                                "--committed-at",
                                                &value,
                                            )?);
                                    }
                                    "--revealed-at" => {
                                        let Some(value) = pending.next() else {
                                            return Err(
                                                "expected value after --revealed-at".into()
                                            );
                                        };
                                        revealed_at =
                                            Some(ministry_jury::parse_ballot_timestamp(
                                                "--revealed-at",
                                                &value,
                                            )?);
                                    }
                                    "--out" => {
                                        let Some(path) = pending.next() else {
                                            return Err("expected path after --out".into());
                                        };
                                        output = Some(normalize_path(Path::new(&path))?);
                                    }
                                    "--reveal-out" => {
                                        let Some(path) = pending.next() else {
                                            return Err(
                                                "expected path after --reveal-out".into()
                                            );
                                        };
                                        reveal_output =
                                            Some(normalize_path(Path::new(&path))?);
                                    }
                                    flag => {
                                        return Err(format!(
                                            "unknown flag for ministry-jury ballot commit: {flag}"
                                        )
                                        .into());
                                    }
                                }
                            }
                            let proposal_id = proposal_id
                                .ok_or_else(|| {
                                    eyre!("ministry-jury ballot commit requires --proposal <id>")
                                })
                                .map_err(|err| -> Box<dyn Error> { err.into() })?;
                            let round_id = round_id
                                .ok_or_else(|| {
                                    eyre!("ministry-jury ballot commit requires --round <id>")
                                })
                                .map_err(|err| -> Box<dyn Error> { err.into() })?;
                            let juror_id = juror_id
                                .ok_or_else(|| {
                                    eyre!("ministry-jury ballot commit requires --juror <id>")
                                })
                                .map_err(|err| -> Box<dyn Error> { err.into() })?;
                            let choice_literal = choice_literal
                                .ok_or_else(|| {
                                    eyre!(
                                        "ministry-jury ballot commit requires --choice <approve|reject|abstain>"
                                    )
                                })
                                .map_err(|err| -> Box<dyn Error> { err.into() })?;
                            let choice = ministry_jury::parse_vote_choice(&choice_literal)?;
                            let nonce =
                                ministry_jury::parse_nonce_hex_or_random(nonce_hex.as_deref())?;
                            let committed_at =
                                committed_at.unwrap_or_else(OffsetDateTime::now_utc);
                            Ok(CommandKind::MinistryJury(
                                ministry_jury::Command::BallotCommit(
                                    ministry_jury::BallotCommitOptions {
                                        proposal_id,
                                        round_id,
                                        juror_id,
                                        choice,
                                        nonce,
                                        committed_at,
                                        revealed_at,
                                        output_path: output,
                                        reveal_output_path: reveal_output,
                                    },
                                ),
                            ))
                        }
                        "verify" => {
                            let mut commit_path = None;
                            let mut reveal_path = None;
                            while let Some(flag) = pending.next() {
                                match flag.as_str() {
                                    "--commit" => {
                                        let Some(path) = pending.next() else {
                                            return Err(
                                                "expected path after --commit".into()
                                            );
                                        };
                                        commit_path =
                                            Some(normalize_path(Path::new(&path))?);
                                    }
                                    "--reveal" => {
                                        let Some(path) = pending.next() else {
                                            return Err(
                                                "expected path after --reveal".into()
                                            );
                                        };
                                        reveal_path =
                                            Some(normalize_path(Path::new(&path))?);
                                    }
                                    flag => {
                                        return Err(format!(
                                            "unknown flag for ministry-jury ballot verify: {flag}"
                                        )
                                        .into());
                                    }
                                }
                            }
                            let commit_path = commit_path
                                .ok_or_else(|| {
                                    eyre!("ministry-jury ballot verify requires --commit <path>")
                                })
                                .map_err(|err| -> Box<dyn Error> { err.into() })?;
                            let reveal_path = reveal_path
                                .ok_or_else(|| {
                                    eyre!("ministry-jury ballot verify requires --reveal <path>")
                                })
                                .map_err(|err| -> Box<dyn Error> { err.into() })?;
                            Ok(CommandKind::MinistryJury(
                                ministry_jury::Command::BallotVerify(
                                    ministry_jury::BallotVerifyOptions {
                                        commit_path,
                                        reveal_path,
                                    },
                                ),
                            ))
                        }
                        other => Err(format!(
                            "unknown ministry-jury ballot subcommand `{other}` (expected commit|verify)"
                        )
                        .into()),
                    }
                }
                other => Err(format!(
                    "unknown ministry-jury subcommand `{other}` (expected sortition|ballot)"
                )
                .into()),
            }
        }
        other => {
            print_usage();
            Err(format!("unknown command: {other}").into())
        }
    }
}

fn generate_openapi(
    outputs: Vec<PathBuf>,
    allow_stub: bool,
    manifest: PathBuf,
    signing_key: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let spec = match try_generate_router_openapi() {
        Ok(Some(value)) => value,
        Ok(None) => {
            if allow_stub {
                eprintln!(
                    "warning: Torii did not expose an OpenAPI document; emitting stub specification"
                );
                build_stub_spec()
            } else {
                return Err("Torii did not expose an OpenAPI document; rerun with --allow-stub to emit the placeholder spec".into());
            }
        }
        Err(err) => {
            if allow_stub {
                eprintln!(
                    "warning: failed to generate OpenAPI from Torii router ({err}); emitting stub specification"
                );
                build_stub_spec()
            } else {
                return Err(format!(
                    "failed to generate OpenAPI from Torii router: {err}. rerun with --allow-stub to emit the placeholder spec"
                )
                .into());
            }
        }
    };

    let formatted = json::to_string_pretty(&spec)?;

    for path in &outputs {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, formatted.as_bytes())?;
        println!("wrote {}", path.display());
    }

    if let Some(signing_key) = signing_key {
        let canonical = default_openapi_path();
        if !outputs.contains(&canonical) {
            return Err(
                "--sign requires generating docs/portal/static/openapi/torii.json; include the canonical output path"
                    .into(),
            );
        }
        write_openapi_manifest(&canonical, &manifest, &signing_key)?;
    }

    Ok(())
}

#[derive(Serialize, Deserialize)]
struct OpenApiManifest {
    version: u32,
    generated_unix_ms: u64,
    generator_commit: Option<String>,
    artifact: OpenApiArtifact,
}

#[derive(Serialize, Deserialize)]
struct OpenApiArtifact {
    path: String,
    bytes: u64,
    sha256_hex: String,
    blake3_hex: String,
    signature: Option<SignatureEnvelope>,
}

#[derive(Clone, Serialize, Deserialize)]
struct SignatureEnvelope {
    algorithm: String,
    public_key_hex: String,
    signature_hex: String,
}

#[derive(Serialize, Deserialize)]
struct SignerAllowlist {
    version: u32,
    allow: Vec<AllowedSigner>,
}

#[derive(Serialize, Deserialize)]
struct AllowedSigner {
    algorithm: String,
    public_key_hex: String,
}

fn write_openapi_manifest(
    spec_path: &Path,
    manifest_path: &Path,
    signing_key: &Path,
) -> Result<(), Box<dyn Error>> {
    let spec_bytes = fs::read(spec_path).map_err(|err| {
        format!(
            "failed to read {} for manifest signing: {err}",
            spec_path.display()
        )
    })?;

    let sha256_hex = hex::encode(Sha256::digest(&spec_bytes));
    let blake3_hex = blake3::hash(&spec_bytes).to_hex().to_string();
    let size_bytes = spec_bytes.len() as u64;

    let signature = sign_manifest_payload(&spec_bytes, signing_key)?;

    let manifest = OpenApiManifest {
        version: 1,
        generated_unix_ms: unix_ms_now(),
        generator_commit: current_git_commit(),
        artifact: OpenApiArtifact {
            path: infer_artifact_label(spec_path),
            bytes: size_bytes,
            sha256_hex,
            blake3_hex,
            signature: Some(signature),
        },
    };

    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    fs::write(manifest_path, manifest_json.as_bytes())?;
    println!("wrote {}", manifest_path.display());
    Ok(())
}

fn verify_openapi_manifest(
    spec_path: &Path,
    manifest_path: &Path,
    allow_unsigned: bool,
    allowed_signers_path: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    let spec_bytes = fs::read(spec_path).map_err(|err| {
        format!(
            "failed to read {} for manifest verification: {err}",
            spec_path.display()
        )
    })?;
    let manifest_text = fs::read_to_string(manifest_path).map_err(|err| {
        format!(
            "failed to read {} for manifest verification: {err}",
            manifest_path.display()
        )
    })?;
    let manifest: OpenApiManifest = serde_json::from_str(&manifest_text).map_err(|err| {
        format!(
            "failed to parse {} as JSON manifest: {err}",
            manifest_path.display()
        )
    })?;

    if manifest.version != 1 {
        return Err(format!(
            "unsupported OpenAPI manifest version {}; expected 1",
            manifest.version
        )
        .into());
    }

    let expected_sha256 = hex::encode(Sha256::digest(&spec_bytes));
    if !manifest
        .artifact
        .sha256_hex
        .eq_ignore_ascii_case(&expected_sha256)
    {
        return Err(format!(
            "manifest sha256_hex mismatch (expected {expected_sha256}, found {})",
            manifest.artifact.sha256_hex
        )
        .into());
    }

    let expected_blake3 = blake3::hash(&spec_bytes).to_hex().to_string();
    if !manifest
        .artifact
        .blake3_hex
        .eq_ignore_ascii_case(&expected_blake3)
    {
        return Err(format!(
            "manifest blake3_hex mismatch (expected {expected_blake3}, found {})",
            manifest.artifact.blake3_hex
        )
        .into());
    }

    let actual_size = spec_bytes.len() as u64;
    if manifest.artifact.bytes != actual_size {
        return Err(format!(
            "manifest size mismatch (expected {actual_size} bytes, found {})",
            manifest.artifact.bytes
        )
        .into());
    }

    match &manifest.artifact.signature {
        Some(signature) => {
            let allowed_signers = allowed_signers_path
                .map(load_signer_allowlist)
                .transpose()
                .map_err(|err| {
                    format!(
                        "failed to load allowed signers from {}: {err}",
                        allowed_signers_path
                            .expect("allowed_signers_path present when reporting error")
                            .display()
                    )
                })?;
            verify_manifest_signature(signature, &spec_bytes)?;
            if let Some(allowlist) = allowed_signers.as_ref() {
                ensure_signer_allowed(signature, allowlist)?;
            }
        }
        None if allow_unsigned => {
            eprintln!(
                "warning: manifest {} missing signature; continuing due to --allow-unsigned",
                manifest_path.display()
            );
        }
        None => {
            return Err(
                "manifest missing signature; re-run `cargo xtask openapi --sign <key>`".into(),
            );
        }
    }

    println!(
        "verified {} against {}",
        manifest_path.display(),
        spec_path.display()
    );
    Ok(())
}

fn encode_space_directory_manifest(input: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    if !input.exists() {
        return Err(format!("manifest JSON `{}` does not exist", input.display()).into());
    }
    if !input.is_file() {
        return Err(format!("manifest JSON `{}` is not a file", input.display()).into());
    }

    let contents = fs::read(input)
        .map_err(|err| format!("failed to read manifest JSON `{}`: {err}", input.display()))?;
    let manifest: AssetPermissionManifest = norito::json::from_slice(&contents).map_err(|err| {
        format!(
            "failed to parse `{}` as capability manifest JSON: {err}",
            input.display()
        )
    })?;
    let norito_bytes = to_bytes(&manifest).map_err(|err| {
        format!(
            "failed to encode Norito payload for `{}`: {err}",
            input.display()
        )
    })?;

    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create directory `{}`: {err}", parent.display()))?;
    }
    fs::write(output, &norito_bytes).map_err(|err| {
        format!(
            "failed to write Norito manifest `{}`: {err}",
            output.display()
        )
    })?;

    let blake3_hex = blake3::hash(&norito_bytes).to_hex().to_string();
    println!(
        "encoded manifest for UAID {} in dataspace {} -> {} ({} entries, {} bytes, blake3={})",
        manifest.uaid,
        manifest.dataspace,
        output.display(),
        manifest.entries.len(),
        norito_bytes.len(),
        blake3_hex
    );
    Ok(())
}

fn sign_manifest_payload(
    payload: &[u8],
    key_path: &Path,
) -> Result<SignatureEnvelope, Box<dyn Error>> {
    let key_hex = fs::read_to_string(key_path)
        .map_err(|err| format!("failed to read signing key {}: {err}", key_path.display()))?;
    let cleaned: String = key_hex
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let private_key = PrivateKey::from_hex(Algorithm::Ed25519, &cleaned).map_err(|err| {
        format!(
            "failed to parse signing key {} as Ed25519 hex: {err}",
            key_path.display()
        )
    })?;
    let key_pair: KeyPair = private_key.clone().into();
    let signature = Signature::new(key_pair.private_key(), payload);
    let (algorithm, public_bytes) = key_pair.public_key().to_bytes();
    if algorithm != Algorithm::Ed25519 {
        return Err("--sign currently supports Ed25519 keys only".into());
    }
    Ok(SignatureEnvelope {
        algorithm: "ed25519".to_string(),
        public_key_hex: hex::encode(public_bytes),
        signature_hex: hex::encode(signature.payload()),
    })
}

fn verify_manifest_signature(
    signature: &SignatureEnvelope,
    payload: &[u8],
) -> Result<(), Box<dyn Error>> {
    let algorithm = match normalize_algorithm_label(&signature.algorithm).as_str() {
        "ed25519" => Algorithm::Ed25519,
        other => return Err(format!("unsupported manifest signature algorithm `{other}`").into()),
    };
    let public_key = PublicKey::from_hex(algorithm, &signature.public_key_hex)
        .map_err(|err| format!("invalid manifest public key hex: {err}"))?;
    let sig_bytes = hex::decode(&signature.signature_hex)
        .map_err(|err| format!("invalid manifest signature hex: {err}"))?;
    let sig = Signature::from_bytes(&sig_bytes);
    sig.verify(&public_key, payload)
        .map_err(|err| format!("manifest signature verification failed: {err}").into())
}

fn load_signer_allowlist(path: &Path) -> Result<SignerAllowlist, Box<dyn Error>> {
    let contents =
        fs::read_to_string(path).map_err(|err| format!("failed to read allowlist: {err}"))?;
    let allowlist: SignerAllowlist =
        serde_json::from_str(&contents).map_err(|err| format!("invalid allowlist JSON: {err}"))?;
    if allowlist.version != 1 {
        return Err(format!(
            "unsupported allowlist version {}; expected 1",
            allowlist.version
        )
        .into());
    }
    Ok(allowlist)
}

fn ensure_signer_allowed(
    signature: &SignatureEnvelope,
    allowlist: &SignerAllowlist,
) -> Result<(), Box<dyn Error>> {
    let normalized_algorithm = normalize_algorithm_label(&signature.algorithm);
    let public_key_hex = signature.public_key_hex.to_ascii_lowercase();
    let allowed = allowlist.allow.iter().any(|signer| {
        normalize_algorithm_label(&signer.algorithm) == normalized_algorithm
            && signer
                .public_key_hex
                .eq_ignore_ascii_case(&signature.public_key_hex)
    });
    if allowed {
        return Ok(());
    }

    let allowed_keys: Vec<String> = allowlist
        .allow
        .iter()
        .map(|signer| {
            format!(
                "{}:{}",
                normalize_algorithm_label(&signer.algorithm),
                signer.public_key_hex.to_ascii_lowercase()
            )
        })
        .collect();
    Err(format!(
        "unrecognized manifest signer {}:{}; allowed signers: {}",
        normalized_algorithm,
        public_key_hex,
        allowed_keys.join(", ")
    )
    .into())
}

fn normalize_algorithm_label(label: &str) -> String {
    label.trim().to_ascii_lowercase()
}

fn infer_artifact_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn unix_ms_now() -> u64 {
    let nanos = OffsetDateTime::now_utc().unix_timestamp_nanos();
    if nanos <= 0 {
        0
    } else {
        (nanos / 1_000_000) as u64
    }
}

fn current_git_commit() -> Option<String> {
    let output = process::Command::new("git")
        .current_dir(workspace_root())
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.trim().to_string())
}

#[cfg(test)]
mod acceleration_state_tests {
    use serde_json::Value;

    use super::*;

    #[test]
    fn parse_supports_acceleration_state_command() {
        let args = ["xtask", "acceleration-state", "--format", "json"];
        let iter = args.into_iter().map(String::from);
        let command = parse_command(iter).expect("parse acceleration-state command");
        match command {
            CommandKind::AccelerationState {
                format: AccelerationOutputFormat::Json,
            } => {}
            _ => panic!("expected acceleration-state command"),
        }
    }

    #[test]
    fn render_outputs_include_last_error() {
        let config = ivm::AccelerationConfig {
            enable_simd: true,
            enable_metal: true,
            enable_cuda: false,
            max_gpus: Some(2),
            merkle_min_leaves_gpu: Some(4_096),
            ..Default::default()
        };
        let runtime = ivm::AccelerationRuntimeStatus {
            simd: ivm::BackendRuntimeStatus {
                supported: true,
                configured: true,
                available: true,
                parity_ok: true,
            },
            metal: ivm::BackendRuntimeStatus {
                supported: true,
                configured: true,
                available: false,
                parity_ok: true,
            },
            cuda: ivm::BackendRuntimeStatus {
                supported: false,
                configured: false,
                available: false,
                parity_ok: false,
            },
        };
        let errors = ivm::AccelerationErrorStatus {
            simd: None,
            metal: Some("policy disabled".to_string()),
            cuda: None,
        };
        let state = AccelerationStateReport::from_parts(config, runtime, errors);

        let table =
            render_acceleration_state(&state, AccelerationOutputFormat::Table).expect("render");
        assert!(
            table.contains("policy disabled"),
            "table output should include last error"
        );
        assert!(
            table.contains("max_gpus: 2"),
            "table output should include config values"
        );

        let json =
            render_acceleration_state(&state, AccelerationOutputFormat::Json).expect("render json");
        let parsed: Value = serde_json::from_str(&json).expect("json parse");
        assert_eq!(parsed["config"]["max_gpus"], serde_json::json!(2));
        assert_eq!(parsed["config"]["enable_simd"], serde_json::json!(true));
        assert_eq!(parsed["simd"]["supported"], serde_json::json!(true));
        assert_eq!(parsed["metal"]["supported"], serde_json::json!(true));
        assert_eq!(parsed["cuda"]["supported"], serde_json::json!(false));
        assert_eq!(
            parsed["metal"]["last_error"],
            serde_json::json!("policy disabled")
        );
        assert_eq!(parsed["simd"]["last_error"], serde_json::Value::Null);
    }
}

#[cfg(test)]
mod openapi_tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn manifest_signature_round_trip() {
        let tmp = tempdir().expect("tempdir");
        let spec_path = tmp.path().join("spec.json");
        fs::write(&spec_path, b"{\"ok\":true}").expect("write spec");
        let key_path = tmp.path().join("key.hex");
        let key_hex = hex::encode([0x11u8; 32]);
        fs::write(&key_path, key_hex).expect("write key");
        let payload = fs::read(&spec_path).expect("read spec");
        let signature = sign_manifest_payload(&payload, &key_path).expect("sign payload");
        verify_manifest_signature(&signature, &payload).expect("verify signature");
    }

    #[test]
    fn infer_label_uses_file_name() {
        let tmp = tempdir().expect("tempdir");
        let nested = tmp.path().join("nested").join("file.txt");
        assert_eq!(infer_artifact_label(&nested), "file.txt");
        assert_eq!(infer_artifact_label(Path::new("torii.json")), "torii.json");
    }

    #[test]
    fn manifest_signature_must_be_allowlisted() {
        let tmp = tempdir().expect("tempdir");
        let spec_path = tmp.path().join("spec.json");
        fs::write(&spec_path, b"{\"ok\":true}").expect("write spec");

        let signing_key_path = tmp.path().join("signing.key");
        let signing_key_hex = hex::encode([0x33u8; 32]);
        fs::write(&signing_key_path, signing_key_hex).expect("write key");

        let manifest_path = tmp.path().join("manifest.json");
        write_openapi_manifest(&spec_path, &manifest_path, &signing_key_path)
            .expect("sign manifest");
        let manifest_json = fs::read_to_string(&manifest_path).expect("manifest json");
        let manifest: OpenApiManifest =
            serde_json::from_str(&manifest_json).expect("parse manifest json");
        let signer = manifest
            .artifact
            .signature
            .as_ref()
            .expect("signature must exist after signing");

        let allowlist_path = tmp.path().join("allow.json");
        let allowlist_contents = format!(
            "{{\"version\":1,\"allow\":[{{\"algorithm\":\"ed25519\",\"public_key_hex\":\"{}\"}}]}}",
            signer.public_key_hex
        );
        fs::write(&allowlist_path, allowlist_contents).expect("write allowlist");

        verify_openapi_manifest(
            &spec_path,
            &manifest_path,
            false,
            Some(allowlist_path.as_path()),
        )
        .expect("allowed signer should verify");

        let rejecting_path = tmp.path().join("reject.json");
        let rejecting_contents = format!(
            "{{\"version\":1,\"allow\":[{{\"algorithm\":\"ed25519\",\"public_key_hex\":\"{}\"}}]}}",
            hex::encode([0x44u8; 32])
        );
        fs::write(&rejecting_path, rejecting_contents).expect("write rejecting allowlist");

        let err = verify_openapi_manifest(
            &spec_path,
            &manifest_path,
            false,
            Some(rejecting_path.as_path()),
        )
        .expect_err("verification should fail for unlisted signer");
        let message = err.to_string();
        assert!(
            message.contains("unrecognized manifest signer"),
            "expected unrecognized signer message, got {message}"
        );
    }
}

#[cfg(test)]
mod space_directory_tests {
    use tempfile::tempdir;

    use super::*;

    const SAMPLE_MANIFEST: &str = r#"{
  "version": 1,
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "issued_ms": 1762723200000,
  "activation_epoch": 4097,
  "expiry_epoch": 4600,
  "entries": [
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.transfer",
        "method": "transfer",
        "asset": "CBDC#centralbank",
        "role": "Initiator"
      },
      "effect": {
        "Allow": {
          "max_amount": "500000000",
          "window": "PerDay"
        }
      },
      "notes": "Wholesale transfer allowance (per UAID, per day)."
    },
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.kit",
        "method": "withdraw"
      },
      "effect": {
        "Deny": {
          "reason": "Withdrawals disabled for this UAID."
        }
      },
      "notes": "Deny wins over any preceding allowance."
    }
  ]
}"#;

    const INVALID_MANIFEST: &str = r#"{
  "version": 2,
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 12,
  "issued_ms": 1700000000000,
  "activation_epoch": 1024,
  "entries": []
}"#;

    #[test]
    fn encode_manifest_round_trips_through_norito() {
        let tmp = tempdir().expect("tempdir");
        let json_path = tmp.path().join("cbdc_wholesale.manifest.json");
        fs::write(&json_path, SAMPLE_MANIFEST).expect("write manifest json");
        let output_path = tmp.path().join("cbdc_wholesale.manifest.to");

        encode_space_directory_manifest(&json_path, &output_path)
            .expect("encode manifest into Norito");

        let norito_bytes = fs::read(&output_path).expect("read encoded manifest");
        assert!(
            norito_bytes.starts_with(&norito::core::MAGIC),
            "encoded manifest must include Norito magic header"
        );
    }

    #[test]
    fn reject_unsupported_manifest_version() {
        assert!(
            norito::json::from_str::<AssetPermissionManifest>(INVALID_MANIFEST).is_err(),
            "unsupported manifest versions must be rejected"
        );
    }
}

#[cfg(test)]
mod streaming_bundle_tests {
    use super::*;

    fn fixtures_dir() -> PathBuf {
        workspace_root()
            .join("crates")
            .join("iroha_config")
            .join("tests")
            .join("fixtures")
    }

    #[test]
    fn bundle_summary_reports_defaults() {
        let config = fixtures_dir().join("minimal_with_trusted_peers.toml");
        let summary =
            build_streaming_bundle_summary(&config, None).expect("summary for minimal config");
        assert_eq!(
            summary.get("entropy_mode").and_then(|v| v.as_str()),
            Some("rans_bundled")
        );
        assert_eq!(
            summary.get("bundle_required").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            summary.get("bundle_accel").and_then(|v| v.as_str()),
            Some("none")
        );
        assert_eq!(
            summary.get("bundle_width").and_then(|v| v.as_u64()),
            Some(2)
        );
        let expected_checksum = {
            let path = workspace_root().join("codec/rans/tables/rans_seed0.toml");
            let tables = load_bundle_tables_from_toml(path).expect("load default tables");
            hex::encode(tables.checksum())
        };
        let tables = summary
            .get("tables")
            .and_then(|v| v.as_object())
            .expect("tables object");
        assert_eq!(
            tables.get("precision_bits").and_then(|v| v.as_u64()),
            Some(12)
        );
        assert_eq!(
            tables.get("checksum").and_then(|v| v.as_str()),
            Some(expected_checksum.as_str())
        );
    }

    #[test]
    fn bundle_summary_honors_config_overrides() {
        let config = fixtures_dir().join("streaming_bundled.toml");
        let summary =
            build_streaming_bundle_summary(&config, None).expect("summary for bundled config");
        assert_eq!(
            summary.get("entropy_mode").and_then(|v| v.as_str()),
            Some("rans_bundled")
        );
        assert_eq!(
            summary.get("bundle_required").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            summary.get("bundle_accel").and_then(|v| v.as_str()),
            Some("cpu_simd")
        );
        assert_eq!(
            summary.get("bundle_width").and_then(|v| v.as_u64()),
            Some(3)
        );
    }

    #[test]
    fn context_remap_produces_top_mapping() {
        let tmp = TempDir::new().expect("tmp dir");
        let telemetry_path = tmp.path().join("telemetry.json");
        let mut symbol_counts = vec![0u64; CONTEXT_SYMBOL_CAP];
        symbol_counts[1] = 10;
        let mut secondary_counts = vec![0u64; CONTEXT_SYMBOL_CAP];
        secondary_counts[2] = 4;
        let telemetry = norito::json!({
            "context_frequencies": [
                {
                    "context": 1,
                    "bundles": 10,
                    "total_bits": 40,
                    "symbol_counts": symbol_counts,
                },
                {
                    "context": 2,
                    "bundles": 4,
                    "total_bits": 16,
                    "symbol_counts": secondary_counts,
                }
            ]
        });
        let json = norito::json::to_string_pretty(&telemetry).expect("serialize telemetry string");
        fs::write(&telemetry_path, json).expect("write telemetry");
        let output_path = tmp.path().join("remap.json");
        streaming_context_remap(StreamingContextRemapOptions {
            input: telemetry_path.clone(),
            top: 1,
            output: JsonTarget::File(output_path.clone()),
        })
        .expect("remap");
        let rendered = fs::read_to_string(output_path).expect("read remap output");
        let value: Value = norito::json::from_str(&rendered).expect("json");
        assert_eq!(
            value
                .get("kept")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|row| row.get("remapped"))
                .and_then(|v| v.as_u64()),
            Some(0)
        );
        assert_eq!(
            value
                .get("dropped")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|row| row.get("remapped"))
                .and_then(|v| v.as_u64()),
            None
        );
    }
}

fn try_generate_router_openapi() -> Result<Option<Value>, Box<dyn Error>> {
    let runtime = TokioRuntimeBuilder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(generate_router_openapi_async())
}

async fn generate_router_openapi_async() -> Result<Option<Value>, Box<dyn Error>> {
    const OPENAPI_ENDPOINT_CANDIDATES: &[&str] = &["/openapi.json", "/openapi"];

    let _data_dir = TestDataDirGuard::new();
    let mut cfg = mk_minimal_root_cfg();

    let mut tokens = vec!["Test-Token".to_owned()];
    if let Ok(single) = std::env::var("TORII_OPENAPI_TOKEN")
        && !single.is_empty()
    {
        tokens.push(single);
    }
    if let Some(env_tokens) = std::env::var_os("IROHA_TORII_OPENAPI_TOKENS") {
        tokens.extend(
            env_tokens
                .to_string_lossy()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_owned()),
        );
    }
    cfg.torii.require_api_token = true;
    cfg.torii.api_tokens = tokens.clone();

    let (kiso, _child) = KisoHandle::start(cfg.clone());

    let kura = Kura::blank_kura_for_testing();
    let query_store = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query_store,
    ));

    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(Default::default());
    let _peers_tx_guard = peers_tx;

    let torii = iroha_torii::Torii::new_with_handle(
        cfg.common.chain.clone(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );

    let router = torii.api_router_for_tests();
    let spec = fetch_openapi_from_router(router, OPENAPI_ENDPOINT_CANDIDATES).await;
    Ok(spec)
}

async fn fetch_openapi_from_router(router: Router, candidates: &[&str]) -> Option<Value> {
    let mut token_header = std::env::var("TORII_OPENAPI_TOKEN")
        .ok()
        .filter(|value| !value.is_empty());
    if token_header.is_none() {
        token_header = std::env::var("IROHA_TORII_OPENAPI_TOKENS")
            .ok()
            .and_then(|list| list.split(',').find(|s| !s.is_empty()).map(str::to_owned));
    }
    if token_header.is_none() {
        token_header = Some("Test-Token".to_owned());
    }

    for path in candidates {
        let mut builder = Request::builder().uri(*path);
        if let Some(token) = token_header.as_deref() {
            builder = builder.header("x-api-token", token);
        }
        let mut request = builder.body(Body::empty()).ok()?;
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))));
        let response = router.clone().into_service().oneshot(request).await.ok()?;
        if !response.status().is_success() {
            continue;
        }

        let bytes = match body::to_bytes(response.into_body(), usize::MAX).await {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("failed to read OpenAPI body from {path}: {err}");
                continue;
            }
        };
        if bytes.is_empty() {
            continue;
        }

        match norito::json::from_slice::<Value>(bytes.as_ref()) {
            Ok(value) => return Some(value),
            Err(err) => {
                eprintln!("failed to parse OpenAPI JSON from {path}: {err}");
            }
        }
    }
    None
}

fn build_stub_spec() -> Value {
    iroha_torii::openapi::stub_spec()
}

fn generate_vote_tally_bundle(
    output: PathBuf,
    verify: bool,
    print_hashes: bool,
    summary_target: Option<JsonTarget>,
    attestation_target: Option<JsonTarget>,
) -> Result<(), Box<dyn Error>> {
    if verify {
        if !output.exists() {
            return Err(format!(
                "expected fixture directory {} to exist; run without --verify first to seed the baseline bundle",
                output.display()
            )
            .into());
        }
        for name in bundle_file_names() {
            let baseline_path = output.join(name);
            if !baseline_path.exists() {
                return Err(format!(
                    "expected fixture {} to exist; run `cargo xtask zk-vote-tally-bundle --out {} --print-hashes` first to materialize the baseline artifacts",
                    baseline_path.display(),
                    output.display()
                )
                .into());
            }
        }
        let temp = TempDir::new()?;
        let _summary = write_bundle(temp.path())?;
        compare_bundle_dirs(temp.path(), &output)?;
        println!("vote tally bundle matches fixtures at {}", output.display());
        if print_hashes {
            print_bundle_hashes(&output)?;
        }
        let summary = read_summary(&output)?;
        println!("{summary}");
        if let Some(target) = summary_target {
            write_summary_json(&summary, target)?;
        }
        if let Some(target) = attestation_target {
            handle_attestation_manifest(&summary, &output, target, true)?;
        }
        return Ok(());
    }

    let summary = write_bundle(&output)?;
    if print_hashes {
        print_bundle_hashes(&output)?;
    }
    println!("{summary}");
    if let Some(target) = summary_target {
        write_summary_json(&summary, target)?;
    }
    if let Some(target) = attestation_target {
        handle_attestation_manifest(&summary, &output, target, false)?;
    }
    Ok(())
}

fn compare_bundle_dirs(generated: &Path, expected: &Path) -> Result<(), Box<dyn Error>> {
    for name in bundle_file_names() {
        let fresh_path = generated.join(name);
        let baseline_path = expected.join(name);
        if !baseline_path.exists() {
            return Err(format!(
                "expected fixture {} to exist; regenerate without --verify",
                baseline_path.display()
            )
            .into());
        }
        let fresh = fs::read(&fresh_path)?;
        let baseline = fs::read(&baseline_path)?;
        if fresh != baseline {
            let fresh_hash = compute_hash(&fresh);
            let baseline_hash = compute_hash(&baseline);
            return Err(format!(
                "fixture mismatch for {name}: generated {fresh_hash}, baseline {baseline_hash}"
            )
            .into());
        }
    }
    Ok(())
}

fn write_summary_json(summary: &BundleSummary, target: JsonTarget) -> Result<(), Box<dyn Error>> {
    let value = summary_to_json(summary);
    write_json_output(&value, target)
}

pub(crate) fn write_json_output(value: &Value, target: JsonTarget) -> Result<(), Box<dyn Error>> {
    let mut json_text = norito::json::to_string_pretty(value)?;
    json_text.push('\n');
    match target {
        JsonTarget::Stdout => {
            print!("{json_text}");
        }
        JsonTarget::File(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, json_text)?;
        }
    }
    Ok(())
}

fn write_address_vectors(target: JsonTarget) -> Result<(), Box<dyn Error>> {
    let value = compliance_vectors_json();
    write_json_output(&value, target)
}

fn verify_address_vectors(path: &Path) -> Result<(), Box<dyn Error>> {
    let generated = compliance_vectors_json();
    let raw = fs::read_to_string(path)?;
    let existing: Value = json::from_str(&raw)?;
    if existing != generated {
        return Err(format!(
            "address vector fixture `{}` is stale; regenerate with `cargo xtask address-vectors`",
            path.display()
        )
        .into());
    }
    Ok(())
}

fn handle_attestation_manifest(
    summary: &BundleSummary,
    dir: &Path,
    target: JsonTarget,
    verify_mode: bool,
) -> Result<(), Box<dyn Error>> {
    let manifest = vote_tally::attestation_manifest(summary, dir)?;
    match target {
        JsonTarget::Stdout => write_json_output(&manifest, JsonTarget::Stdout),
        JsonTarget::File(path) => {
            if verify_mode {
                let existing = fs::read_to_string(&path).map_err(|err| {
                    format!(
                        "failed to read attestation manifest {} during verification: {err}",
                        path.display()
                    )
                })?;
                let parsed: Value = norito::json::from_str(&existing)?;
                if parsed["hash_algorithm"] != manifest["hash_algorithm"] {
                    return Err(format!(
                        "attestation manifest at {} has hash_algorithm {:?}, expected {:?}",
                        path.display(),
                        parsed["hash_algorithm"],
                        manifest["hash_algorithm"]
                    )
                    .into());
                }
                if parsed["bundle"] != manifest["bundle"] {
                    return Err(format!(
                        "attestation manifest at {} has mismatched bundle metadata",
                        path.display()
                    )
                    .into());
                }
                let parsed_artifacts = parsed["artifacts"]
                    .as_array()
                    .ok_or("attestation manifest artifacts must be an array")?;
                let manifest_artifacts = manifest["artifacts"]
                    .as_array()
                    .ok_or("regenerated attestation artifacts must be an array")?;
                if parsed_artifacts.len() != manifest_artifacts.len() {
                    return Err(format!(
                        "attestation manifest at {} lists {} artifacts, regenerated bundle has {}",
                        path.display(),
                        parsed_artifacts.len(),
                        manifest_artifacts.len()
                    )
                    .into());
                }
                let parsed_map = artifacts_to_map(parsed_artifacts)?;
                let manifest_map = artifacts_to_map(manifest_artifacts)?;
                if parsed_map.keys().ne(manifest_map.keys()) {
                    return Err(format!(
                        "attestation manifest at {} lists different artifact set than regenerated bundle",
                        path.display()
                    )
                    .into());
                }
                for (name, (len, hash)) in manifest_map.iter() {
                    let Some((existing_len, existing_hash)) = parsed_map.get(name) else {
                        return Err(format!(
                            "attestation manifest at {} missing artifact {}",
                            path.display(),
                            name
                        )
                        .into());
                    };
                    if existing_len != len {
                        return Err(format!(
                            "attestation manifest at {} records len {} for {}, but regenerated bundle len is {}",
                            path.display(),
                            existing_len,
                            name,
                            len
                        )
                        .into());
                    }
                    if existing_hash != hash {
                        return Err(format!(
                            "attestation manifest at {} records digest {} for {}, but regenerated bundle digest is {} (deterministic artifacts must match)",
                            path.display(),
                            existing_hash,
                            name,
                            hash
                        )
                        .into());
                    }
                }
                Ok(())
            } else {
                let mut text = norito::json::to_string_pretty(&manifest)?;
                text.push('\n');
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(path, text)?;
                Ok(())
            }
        }
    }
}

fn artifacts_to_map(
    artifacts: &[Value],
) -> Result<BTreeMap<String, (u64, String)>, Box<dyn Error>> {
    let mut map = BTreeMap::new();
    for entry in artifacts {
        let file = entry["file"]
            .as_str()
            .ok_or("artifact missing file name")?
            .to_owned();
        let len = entry["len"].as_u64().ok_or("artifact missing len")?;
        let hash = entry["blake2b_256"]
            .as_str()
            .ok_or("artifact missing blake2b_256")?
            .to_owned();
        map.insert(file, (len, hash));
    }
    Ok(map)
}

fn print_bundle_hashes(dir: &Path) -> Result<(), Box<dyn Error>> {
    for name in bundle_file_names() {
        let path = dir.join(name);
        let hash = compute_hash(&fs::read(&path)?);
        println!("{}  {}", hash, path.display());
    }
    Ok(())
}

fn compute_hash(bytes: &[u8]) -> String {
    hex::encode(iroha_hash(bytes))
}

pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask resides in workspace root")
        .to_path_buf()
}

fn default_openapi_path() -> PathBuf {
    workspace_root().join("docs/portal/static/openapi/torii.json")
}

fn default_openapi_manifest_path() -> PathBuf {
    workspace_root().join("docs/portal/static/openapi/manifest.json")
}

fn default_openapi_allowed_signers_path() -> PathBuf {
    workspace_root().join("docs/portal/static/openapi/allowed_signers.json")
}

fn default_da_report_path() -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("da")
        .join("threat_model_report.json")
}

fn default_da_replication_audit_path() -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("da")
        .join("replication_audit.json")
}

fn default_da_commitment_reconcile_path() -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("da")
        .join("commitment_reconciliation.json")
}

fn default_da_privilege_audit_path() -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("da")
        .join("privilege_audit.json")
}

fn default_da_proof_bench_json_path() -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("da")
        .join("proof_bench")
        .join("benchmark.json")
}

fn default_da_proof_bench_markdown_path() -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("da")
        .join("proof_bench")
        .join("benchmark.md")
}

fn default_da_proof_manifest() -> PathBuf {
    workspace_root()
        .join("fixtures")
        .join("da")
        .join("reconstruct")
        .join("rs_parity_v1")
        .join("manifest.json")
}

fn default_da_proof_payload() -> PathBuf {
    workspace_root()
        .join("fixtures")
        .join("da")
        .join("reconstruct")
        .join("rs_parity_v1")
        .join("payload.bin")
}

fn default_da_proof_generated_payload() -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("da")
        .join("proof_bench")
        .join("payload.bin")
}

fn default_gar_receipts_dir(pop_label: &str) -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("soranet")
        .join("gateway")
        .join(pop_label)
        .join("gar_receipts")
}

fn default_gar_acks_dir(pop_label: &str) -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("soranet")
        .join("gateway")
        .join(pop_label)
        .join("gar_acks")
}

fn default_gar_summary_path(pop_label: &str) -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("soranet")
        .join("gateway")
        .join(pop_label)
        .join("gar_receipts_summary.json")
}

fn default_gar_bus_dir(pop_label: Option<&str>) -> PathBuf {
    let mut path = workspace_root()
        .join("artifacts")
        .join("soranet")
        .join("gateway");
    if let Some(pop) = pop_label {
        path = path.join(pop);
    }
    path.join("gar_bus")
}

fn run_config_debug(path: &Path) -> Result<(), Box<dyn Error>> {
    println!("reading config {}", path.display());
    let reader = ConfigReader::new();
    let reader = match reader.read_toml_with_extends(path) {
        Ok(reader) => reader,
        Err(err) => {
            eprintln!(
                "config reader failed to load `{}`:\n{err:?}",
                path.display()
            );
            return Err("failed to read config source".into());
        }
    };
    match reader.read_and_complete::<user::Root>() {
        Ok(_cfg) => {
            println!("config OK");
            Ok(())
        }
        Err(err) => {
            eprintln!("config `{}` failed to parse:\n{err:?}", path.display());
            Err("config parse failed".into())
        }
    }
}

fn load_actual_config(path: &Path) -> eyre::Result<actual::Root> {
    let reader = ConfigReader::new();
    let reader = reader
        .read_toml_with_extends(path)
        .map_err(|err| eyre!("failed to read `{}`: {err}", path.display()))?;
    let user_cfg = reader
        .read_and_complete::<user::Root>()
        .map_err(|err| eyre!("failed to parse `{}`: {err:?}", path.display()))?;
    user_cfg
        .parse()
        .map_err(|err| eyre!("configuration `{}` invalid: {err:?}", path.display()))
}

fn default_nexus_lane_commitment_dir() -> PathBuf {
    workspace_root()
        .join("fixtures")
        .join("nexus")
        .join("lane_commitments")
}

fn default_address_vectors_path() -> Result<PathBuf, Box<dyn Error>> {
    normalize_path(Path::new("fixtures/account/address_vectors.json"))
}

fn default_vote_tally_path() -> PathBuf {
    workspace_root().join("fixtures/zk/vote_tally")
}

fn default_mochi_bundle_path() -> PathBuf {
    workspace_root().join("target/mochi-bundle")
}

fn default_taikai_spool_dir() -> PathBuf {
    workspace_root()
        .join("storage")
        .join("da_manifests")
        .join("taikai")
}

fn default_testnet_kit_path() -> PathBuf {
    workspace_root()
        .join("docs")
        .join("examples")
        .join("soranet_testnet_operator_kit")
}

fn default_rollout_plan_path() -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("soranet_pq_rollout_plan.json")
}

fn default_rollout_markdown_path() -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("soranet_pq_rollout_plan.md")
}

fn default_rollout_capture_dir() -> PathBuf {
    workspace_root()
        .join("artifacts")
        .join("soranet_pq_rollout")
}

fn default_soranet_chaos_dir(now: SystemTime) -> PathBuf {
    let datetime = OffsetDateTime::from(now);
    let slug = format!(
        "{:04}{:02}{:02}",
        datetime.year(),
        datetime.month() as u8,
        datetime.day()
    );
    workspace_root()
        .join("artifacts")
        .join("soranet")
        .join("chaos_game_day")
        .join(slug)
}

fn default_directory_release_output_root() -> PathBuf {
    workspace_root().join("artifacts").join("soradns_directory")
}

fn default_pin_fixture_path() -> PathBuf {
    workspace_root()
        .join("crates")
        .join("iroha_core")
        .join("tests")
        .join("fixtures")
        .join("sorafs_pin_registry")
        .join("snapshot.json")
}

fn parse_seed(value: &str) -> Result<u64, Box<dyn Error>> {
    if let Some(stripped) = value.strip_prefix("0x") {
        u64::from_str_radix(stripped, 16).map_err(|err| err.into())
    } else if let Some(stripped) = value.strip_prefix("0X") {
        u64::from_str_radix(stripped, 16).map_err(|err| err.into())
    } else {
        value.parse::<u64>().map_err(|err| err.into())
    }
}

fn default_sm_wycheproof_path() -> PathBuf {
    workspace_root()
        .join("crates")
        .join("iroha_crypto")
        .join("tests")
        .join("fixtures")
        .join("wycheproof_sm2.json")
}

pub(crate) fn normalize_path(path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(workspace_root().join(path))
    }
}

fn parse_gate_date(value: &str) -> Result<Date, Box<dyn Error>> {
    let spec = format_description::parse("[year]-[month]-[day]")
        .map_err(|err| -> Box<dyn Error> { format!("invalid gate date spec: {err}").into() })?;
    Date::parse(value, &spec)
        .map_err(|err| -> Box<dyn Error> { format!("invalid date `{value}`: {err}").into() })
}

fn parse_attachment_spec(spec: &str) -> Result<(String, PathBuf), Box<dyn Error>> {
    let Some((label, path)) = spec.split_once('=') else {
        return Err(
            "invalid --attachment spec; expected label=path (e.g., bond=artifacts/bond.json)"
                .into(),
        );
    };
    let label = label.trim();
    let path = path.trim();
    if label.is_empty() {
        return Err("attachment label cannot be empty".into());
    }
    if path.is_empty() {
        return Err("attachment path cannot be empty".into());
    }
    let normalized = normalize_path(Path::new(path))?;
    Ok((label.to_string(), normalized))
}

fn load_relays_from_file(path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let raw = fs::read_to_string(path)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        let value: Value = json::from_str(trimmed)?;
        let array = value
            .as_array()
            .ok_or("relays file must contain a JSON array of strings")?;
        let mut relays = Vec::with_capacity(array.len());
        for entry in array {
            let text = entry
                .as_str()
                .ok_or("relays file entries must be strings")?
                .trim();
            if !text.is_empty() {
                relays.push(text.to_string());
            }
        }
        Ok(relays)
    } else {
        Ok(raw
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .map(|line| line.to_string())
            .collect())
    }
}

fn parse_label_value(spec: &str, flag: &str) -> Result<(String, String), Box<dyn Error>> {
    let Some((label, value)) = spec.split_once('=') else {
        return Err(format!("{flag} expects label=value").into());
    };
    let label = label.trim();
    let value = value.trim();
    if label.is_empty() {
        return Err(format!("{flag} label cannot be empty").into());
    }
    if value.is_empty() {
        return Err(format!("{flag} value cannot be empty").into());
    }
    Ok((label.to_string(), value.to_string()))
}

fn parse_labelled_path(spec: &str, flag: &str) -> Result<(String, PathBuf), Box<dyn Error>> {
    let (label, path_str) = parse_label_value(spec, flag)?;
    let path = normalize_path(Path::new(&path_str))?;
    Ok((label, path))
}

fn parse_operation_limit(spec: &str, flag: &str) -> Result<(String, f64), Box<dyn Error>> {
    let (label, value_str) = parse_label_value(spec, flag)?;
    let value = value_str.parse::<f64>().map_err(|err| {
        format!("{flag} expects label=float; failed to parse `{value_str}`: {err}")
    })?;
    Ok((label, value))
}

fn parse_rollout_artifact_spec(spec: &str) -> Result<(String, String), Box<dyn Error>> {
    let mut kind: Option<String> = None;
    let mut path: Option<String> = None;
    for entry in spec.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            return Err(
                format!("invalid --artifact spec `{spec}`; expected key=value pairs").into(),
            );
        };
        let key = key.trim();
        let value = value.trim();
        if value.is_empty() {
            return Err(format!("empty value for `{key}` in --artifact spec `{spec}`").into());
        }
        match key {
            "kind" => kind = Some(value.to_string()),
            "path" => path = Some(value.to_string()),
            other => {
                return Err(format!("unknown key `{other}` in --artifact spec `{spec}`").into());
            }
        }
    }
    let kind = kind.ok_or("missing `kind=` in --artifact spec")?;
    let path = path.ok_or("missing `path=` in --artifact spec")?;
    Ok((kind, path))
}

fn print_usage() {
    eprintln!("xtask usage:");
    eprintln!("  cargo xtask openapi [--output <path>] [--allow-stub]");
    eprintln!(
        "    Generate the Torii OpenAPI spec from a live Torii router. Defaults to docs/portal/static/openapi/torii.json; use --allow-stub only for emergency stub output"
    );
    eprintln!(
        "  cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]"
    );
    eprintln!(
        "    Run the DA PDP/PoTR simulator and emit a JSON summary for docs/automation. Defaults to artifacts/da/threat_model_report.json."
    );
    eprintln!(
        "  cargo xtask da-replication-audit --config <torii.toml> --manifest <path> [--manifest <path> ...] [--replication-order <path> ...] [--json-out <path|->] [--plan-out <path|->] [--allow-mismatch]"
    );
    eprintln!(
        "    Audit DA manifests and replication orders against the configured policy, emitting a JSON report and optional remediation plan."
    );
    eprintln!(
        "  cargo xtask da-commitment-reconcile --receipt <path> [--receipt <path> ...] --block <path> [--block <path> ...] [--json-out <path|->] [--allow-unexpected]"
    );
    eprintln!(
        "    Compare DA ingest receipts to DA commitment bundles (SignedBlockWire, .norito, or JSON) and fail on missing/mismatched tickets."
    );
    eprintln!(
        "  cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]"
    );
    eprintln!(
        "    Check DA ingest spool/cursor directories (and any extra paths) for missing/non-directory/world-writable permissions and emit an audit report."
    );
    eprintln!(
        "  cargo xtask da-proof-bench [--manifest <path>] [--payload <path>] [--payload-bytes <n>] [--sample-count <n>] [--sample-seed <u64|0xhex>] [--budget-ms <u64>] [--iterations <n>] [--json-out <path|->] [--markdown-out <path|->]"
    );
    eprintln!(
        "    Measure PoR verification time against the Halo2 verifier budget using the provided manifest/payload (defaults to fixtures/da/reconstruct/rs_parity_v1). Fails if verification exceeds the budget."
    );
    eprintln!(
        "  cargo xtask kagami-profiles [--profile <iroha3-dev|iroha3-testus|iroha3-nexus>] [--out <dir>] [--kagami <path>]"
    );
    eprintln!(
        "    Rebuild the canned Kagami profile bundles (genesis + PoPs + snippets) under defaults/kagami for Iroha 3 smoke tests; use --kagami to point at a specific binary."
    );
    eprintln!(
        "    Lint and migrate Iroha 2 configs/genesis to Iroha 3 defaults (Nexus lanes, SoraFS, fee asset). Writes migrated copies when output paths are provided."
    );
    eprintln!("  cargo xtask address-manifest verify --bundle <dir> [--previous <dir>]");
    eprintln!(
        "    Validate address manifest bundles (checksums, entry schema, monotonic sequence/digest). See docs/source/runbooks/address_manifest_ops.md for required inputs."
    );
    eprintln!("  cargo xtask address-vectors [--out <path>] [--stdout] [--verify]");
    eprintln!(
        "    Emit or verify the ADDR-2 IH58 (preferred)/snx1 (second-best)/multisig fixture. Defaults to fixtures/account/address_vectors.json"
    );
    eprintln!(
        "  cargo xtask address-local8-gate --input <prom-range.json> [--window-days 30] [--json-out <path|->] [--skip-collisions]"
    );
    eprintln!(
        "    Check torii_address_local8_total / torii_address_collision_total counters over a Prometheus range query response and fail when the 30-day zero-usage gate is not met."
    );
    eprintln!(
        "  cargo xtask zk-vote-tally-bundle [--out <path>] [--verify] [--print-hashes] [--summary-json <path|->] [--attestation <path|->]"
    );
    eprintln!(
        "    Rebuild or verify the Halo2 vote tally fixtures. Defaults to fixtures/zk/vote_tally"
    );
    eprintln!("    Run once without --verify to seed fixtures before using --verify.");
    eprintln!("    Use --summary-json - to emit JSON on stdout or provide a path to write a file.");
    eprintln!(
        "    Use --attestation - to emit artifact hashes on stdout or provide a path to write a file."
    );
    eprintln!("    Proof envelope hashes are deterministic; mismatches indicate drift.");
    eprintln!("  cargo xtask soranet-fixtures [--out <path>] [--verify]");
    eprintln!(
        "    Generate SoraNet capability negotiation and downgrade telemetry fixtures. Defaults to tests/interop/soranet/capabilities"
    );
    eprintln!(
        "  cargo xtask soranet-chaos-kit [--out <dir>] [--pop <label>] [--gateway <host>] [--resolver <host>] [--quarter <label>] [--now <unix-secs>]"
    );
    eprintln!(
        "    Generate the SNNet-15F1 chaos GameDay kit (plan, scripts, shared log) under artifacts/soranet/chaos_game_day/<date>."
    );
    eprintln!("  cargo xtask soranet-chaos-report --log <path> [--out <path|->]");
    eprintln!(
        "    Summarise a chaos GameDay log (NDJSON) into JSON detection/recovery timings; defaults to stdout."
    );
    eprintln!(
        "  cargo xtask soranet-pop-bundle --input <path> [--roa <path>] [--output-dir <path>] [--edns-resolver <addr>] [--edns-tool <bin>] [--skip-edns] [--skip-ds] [--image-tag <tag>]"
    );
    eprintln!(
        "    Build a PoP provisioning bundle (SN15-M0-2): FRR + resolver templates, optional EDNS/DS evidence, PXE/env/secret/checklist/CI stubs, sign-off bundle, and a checksum manifest. Defaults to artifacts/soranet_pop/bundle."
    );
    eprintln!(
        "  cargo xtask soranet-popctl --input <path> [--roa <path>] [--output-dir <path>] [--edns-resolver <addr>] [--edns-tool <bin>] [--skip-edns] [--skip-ds] [--image-tag <tag>]"
    );
    eprintln!(
        "    Alias for soranet-pop-bundle with the popctl naming used in SN15-M0-2 docs; emits the same bundle with the signoff/CI assets staged for attestation."
    );
    eprintln!(
        "  cargo xtask soranet-pop-template --input <path> [--output <path>] [--resolver-config <path>] [--edns-out <path>] [--edns-resolver <addr>] [--edns-tool <bin>] [--ds-out <path>]"
    );
    eprintln!(
        "    Render an FRR config for a SoraNet PoP (SNNet-15A2) and optionally emit resolver templates plus EDNS/DS evidence. Defaults to artifacts/soranet_pop/frr.conf when --output is omitted."
    );
    eprintln!(
        "  cargo xtask soranet-pop-validate --input <path> [--roa <path>] [--json-out <path|->]"
    );
    eprintln!(
        "    Validate a PoP descriptor (SNNet-15A1/A2) with optional ROA bundle, surfacing BFD/RPKI defaults and ROA gaps. Defaults to stdout when --json-out is omitted."
    );
    eprintln!(
        "  cargo xtask soranet-pop-policy-report --input <path> [--roa <path>] [--output-dir <path>] [--grafana-out <path>] [--alert-rules-out <path>] [--json-out <path>]"
    );
    eprintln!(
        "    Generate the SN15-M0-3 BGP policy harness: Prometheus alert rules, Grafana dashboard, and a JSON report with route-health baselines. Defaults to artifacts/soranet_pop/policy."
    );
    eprintln!(
        "  cargo xtask soranet-pop-plan --input <path> [--roa <path>] [--frr-out <path>] [--json-out <path|->]"
    );
    eprintln!(
        "    Render FRR config and emit a validation JSON report in one run. Defaults: artifacts/soranet_pop/frr.conf for config, stdout for JSON."
    );
    eprintln!("  cargo xtask soranet-gateway-m0 [--output-dir <path>] [--edge-name <name>]");
    eprintln!(
        "    Emit the SNNet-15M0 gateway baseline pack (H3 edge config, trustless verifier skeleton, WAF/rate policy) with a JSON summary. Defaults to configs/soranet/gateway_m0."
    );
    eprintln!("  cargo xtask soranet-gateway-m1 [--config <path>] [--output-dir <path>]");
    eprintln!(
        "    Build the SNNet-15M1 alpha bundle (PoP provisioning, gateway baselines, ops packs, billing dry-run) into a single evidence root. Defaults to configs/soranet/gateway_m1/alpha_config.json and artifacts/soranet/gateway_m1/alpha."
    );
    eprintln!("  cargo xtask soranet-gateway-m2 [--config <path>] [--output-dir <path>]");
    eprintln!(
        "    Build the SNNet-15M2 beta bundle (DoQ/ODoH preview configs, trustless verifier wiring, PQ readiness, compliance + hardening summaries, prepaid billing). Defaults to configs/soranet/gateway_m2/beta_config.json and artifacts/soranet/gateway_m2/beta."
    );
    eprintln!(
        "  cargo xtask soranet-gateway-m3 --m2-summary <path> --autoscale-plan <path> --worker-pack <path> [--out <path>] [--sla-target <label>]"
    );
    eprintln!(
        "    Emit the SNNet-15M3 GA readiness bundle by hashing autoscale/worker artefacts and linking the M2 summary; outputs JSON + Markdown evidence."
    );
    eprintln!("  cargo xtask soranet-gateway-ops-m0 [--output-dir <path>] [--pop <name>]");
    eprintln!(
        "    Emit the SN15-M0 ops pack (OTEL pipeline, alerts, GameDay, GAR/security outlines) with a JSON summary. Defaults to configs/soranet/gateway_m0/observability."
    );
    eprintln!(
        "  cargo xtask soranet-gateway-pq --srcv2 <path> --tls-bundle <dir> --trustless-config <path> [--pop <name>] [--out <dir>] [--canary <host> ...] [--phase <1|2|3>]"
    );
    eprintln!(
        "    Generate the SNNet-15PQ readiness bundle (SRCv2 dual-sig validation, TLS/ECH evidence, trustless verifier config, canary host list) under artifacts/soranet/gateway_pq unless overridden."
    );
    eprintln!("  cargo xtask soranet-bug-bounty --config <path> [--output-dir <path>]");
    eprintln!(
        "    Generate the SNNet-15H1 pen-test and bug bounty kit (overview, triage checklist, remediation template, summary). Defaults to artifacts/soranet/gateway/bug_bounty."
    );
    eprintln!(
        "  cargo xtask soranet-gateway-ops-m1 [--output-dir <path>] [--pops <a,b,c>] [--pop <name>...]"
    );
    eprintln!(
        "    Emit the federated SNNet-15F ops pack for multiple PoPs (per-pop bundles, federated OTEL collector, and GameDay rotation). Defaults to configs/soranet/gateway_m1."
    );
    eprintln!(
        "  cargo xtask soranet-gateway-chaos [--config <path>] [--out <dir>] [--pop <name>] [--scenario <id|all>] [--execute] [--note <text>]"
    );
    eprintln!(
        "    Run or dry-run SNNet-15F1 chaos drills from the scenario pack; defaults to artifacts/soranet/gateway_chaos and the ops-pack scenarios."
    );
    eprintln!(
        "  cargo xtask soranet-gateway-hardening [--sbom <path>] [--vuln-report <path>] [--hsm-policy <path>] [--sandbox-profile <path>] [--data-retention-days <u32>] [--log-retention-days <u32>] [--out <dir>]"
    );
    eprintln!(
        "    Generate the SNNet-15H hardening summary (SBOM/vuln/HSM/sandbox evidence + retention defaults) with JSON + Markdown outputs. Defaults to artifacts/soranet/gateway_hardening."
    );
    eprintln!(
        "  cargo xtask soranet-gar-controller [--config <path>] [--output-dir <path>] [--markdown-out <path>] [--now <unix-secs>]"
    );
    eprintln!(
        "    Build the SNNet-15G GAR controller bundle (NATS event spool, per-pop receipts, summary, Markdown report). Defaults to configs/soranet/gateway_m0/gar_controller.sample.json and artifacts/soranet/gateway/gar_controller."
    );
    eprintln!(
        "  cargo xtask soranet-gar-export [--pop <name>] [--receipts-dir <path>] [--acks-dir <path>] [--json-out <path|->] [--markdown-out <path>] [--now <unix-secs>]"
    );
    eprintln!(
        "    Bundle GAR enforcement receipts and ACK files into a JSON summary (and optional Markdown). Defaults to artifacts/soranet/gateway/<pop>/gar_receipts*, using stdout when no pop/paths are supplied."
    );
    eprintln!(
        "  cargo xtask soranet-gar-bus --policy <path> [--out-dir <path>] [--pop <label>] [--json-out <path|->]"
    );
    eprintln!(
        "    Publish a GAR CDN policy payload to the file-based bus (writes JSON + Markdown into artifacts/soranet/gateway/<pop>/gar_bus by default)."
    );
    eprintln!(
        "  cargo xtask soranet-gateway-billing [--usage <path>] [--catalog <path>] [--guardrails <path>] [--output-dir <path>] [--payer <account>] [--treasury <account>] [--asset <definition>] [--allow-hard-cap]"
    );
    eprintln!(
        "    Rate Gateway usage against the SN15-M0 catalog, exporting JSON/CSV/Parquet invoices plus ledger projection and reconciliation report. Defaults use configs/soranet/gateway_m0/billing_usage_sample.json and configs/soranet/gateway_m0/meter_catalog.json."
    );
    eprintln!(
        "  cargo xtask soranet-gateway-billing-m0 [--output-dir <path>] [--billing-period <period>]"
    );
    eprintln!(
        "    Emit the SN15-M0 billing preview pack (meter catalog + CSV/Parquet exports, rating plan, ledger hooks + projection, guardrails, invoice/dispute/reconciliation templates) with a JSON summary. Defaults to configs/soranet/gateway_m0/billing."
    );
    eprintln!("  cargo xtask sorafs-gateway-fixtures [--out <dir>] [--verify]");
    eprintln!(
        "    Generate the canonical SoraFS gateway conformance fixtures. Defaults to fixtures/sorafs_gateway/<version>"
    );
    eprintln!("  cargo xtask sorafs-admission-fixtures [--out <dir>]");
    eprintln!(
        "    Regenerate the provider admission proposal/envelope fixtures. Defaults to fixtures/sorafs_manifest/provider_admission"
    );
    eprintln!(
        "  cargo xtask soradns-hosts --name <fqdn> [--name <fqdn> ...] [--json-out <path|->] [--verify-host-patterns <path> ...]"
    );
    eprintln!(
        "    Derive canonical and pretty gateway hosts for SoraDNS names. Use --json-out to write structured output and --verify-host-patterns to compare derived hosts against GAR host_patterns JSON."
    );
    eprintln!(
        "  cargo xtask soradns-binding-template --manifest <path> --alias <alias> --hostname <host> [--route-label <label>] [--proof-status <status>] [--csp-template <string>] [--permissions-template <string>] [--hsts-template <string>] [--json-out <path|->] [--headers-out <path>] [--generated-at <RFC3339>]"
    );
    eprintln!(
        "    Generate a portal.gateway.binding.json replacement plus the matching headers.txt block so DG-3 tickets can diff alias/CID/route metadata without running the Node helper."
    );
    eprintln!(
        "  cargo xtask soradns-gar-template --name <fqdn> [--manifest <path> | --manifest-cid <cid>] [--manifest-digest <hex>] [--valid-from <secs>] [--valid-until <secs>] [--csp-template <string>] [--hsts-template <string>] [--permissions-template <string>] [--telemetry-label <label> ...] [--json-out <path|->]"
    );
    eprintln!(
        "    Scaffold a Gateway Authorization Record payload with canonical host patterns, default CSP/HSTS templates, and optional telemetry labels. Use --json-out - to emit the JSON on stdout."
    );
    eprintln!(
        "  cargo xtask soradns-acme-plan --name <fqdn> [--name <fqdn> ...] [--directory-url <url>] [--no-pretty] [--no-canonical-wildcard] [--generated-at <RFC3339>] [--json-out <path|->]"
    );
    eprintln!(
        "  cargo xtask soradns-cache-plan --name <fqdn> [--name <fqdn> ...] [--path <path> ...] [--http-method <verb>] [--no-pretty] [--auth-header <name>] [--auth-env <env>] [--generated-at <RFC3339>] [--json-out <path|->]"
    );
    eprintln!(
        "    Emit deterministic cache invalidation plans (hosts + paths + auth hints) so DG-3 change packets can include purge flows alongside GAR/binding templates."
    );
    eprintln!(
        "  cargo xtask soradns-route-plan --name <fqdn> [--name <fqdn> ...] [--no-pretty] [--generated-at <RFC3339>] [--json-out <path|->]"
    );
    eprintln!(
        "    Produce promotion + rollback checklists per alias, including canonical/pretty hosts and staging notes, so DG-3 route changes ship with preflight + revert evidence."
    );
    eprintln!(
        "    Render the wildcard + pretty-host SAN plan with recommended ACME challenges and DNS-01 labels so TLS automation and GAR reviewers share the same evidence bundle."
    );
    eprintln!(
        "  cargo xtask soradns-verify-gar --gar <path> --name <fqdn> [--manifest-cid <cid>] [--manifest-digest <hex>] [--telemetry-label <label> ...] [--json-out <path|->]"
    );
    eprintln!(
        "    Validate a GAR payload against the deterministic host policy before signing or attaching it to DG-3 tickets. Confirms canonical/pretty hosts, manifest metadata, and required telemetry labels."
    );
    eprintln!(
        "  cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> [--alias <alias>] [--content-cid <cid>] [--hostname <host>] [--proof-status <status>] [--manifest-json <path>]"
    );
    eprintln!(
        "    Validate docs portal gateway binding artefacts before attaching them to DG-3 change tickets. Confirms Sora-Name/Proof headers, route metadata, and proof payloads match the expected alias/content CID."
    );
    eprintln!(
        "  cargo xtask sorafs-adoption-check [--scoreboard <path>] [--summary <path>] [--min-providers <count>] [--allow-zero-weight] [--allow-single-source] [--allow-implicit-metadata] [--require-direct-only] [--require-telemetry] [--require-telemetry-region] [--report <path>]"
    );
    eprintln!(
        "    Validate multi-source adoption evidence by inspecting persisted orchestrator scoreboards and summaries."
    );
    eprintln!(
        "    Provide --report to persist the aggregated JSON summary alongside other release artefacts."
    );
    eprintln!(
        "  cargo xtask sorafs-scoreboard-diff --previous <path> --current <path> [--threshold-percent <float>] [--report <path>]"
    );
    eprintln!(
        "    Compare eligible provider weights between scoreboards; highlights deltas and flags entries exceeding the configured threshold."
    );
    eprintln!(
        "  cargo xtask docs-preview summary [--log <path>] [--wave <label>] [--json <path|->]"
    );
    eprintln!(
        "    Summarise docs portal preview invite logs (DOCS-SORA) and emit human or JSON output. Defaults to artifacts/docs_portal_preview/feedback_log.json."
    );
    eprintln!(
        "  cargo xtask compute slo-report [--manifest <path>] [--json-out <path>] [--markdown-out <path|->] [--samples <n>]"
    );
    eprintln!(
        "    Generate a deterministic compute SLO report (JSON + optional Markdown) using the built-in harness; defaults to fixtures/compute/manifest_compute_payments.json and artifacts/compute/compute_slo_report.{{json,md}}."
    );
    eprintln!("  cargo xtask compute fixtures [--output <dir>]");
    eprintln!(
        "    Emit cross-SDK compute fixtures (call, receipt, rejection catalog) into fixtures/compute/sdk_parity or the provided directory."
    );
    eprintln!("  cargo xtask sorafs-taikai-cache-bundle [--profile <id|path>] [--out <dir>]");
    eprintln!(
        "    Package Taikai cache profiles (JSON + Norito + manifest) into artifacts/taikai_cache or the provided directory."
    );
    eprintln!(
        "  cargo xtask taikai-anchor-bundle [--spool <dir>] [--copy-dir <dir>] [--signing-key <path>] [--out <path|->]"
    );
    eprintln!(
        "    Scan the Taikai spool for anchor artefacts, emit a JSON summary (pending + delivered), optionally copy files into a bundle dir, and sign the report with an Ed25519 key."
    );
    eprintln!(
        "  cargo xtask taikai-rpt-verify --envelope <path> [--gar <path>] [--cek-receipt <path>] [--bundle <path>] [--json-out <path|->]"
    );
    eprintln!(
        "    Decode a replication proof token (.to or JSON) and optionally verify the referenced GAR, CEK receipt, and bundle digests. Use --json-out - to emit the structured report on stdout."
    );
    eprintln!(
        "  cargo xtask sorafs-burn-in-check --log <telemetry.log> [--log <telemetry.log>] [--window-days <days>] [--min-pq-ratio <ratio>] [--max-brownout-ratio <ratio>] [--max-no-provider-errors <count>] [--min-fetches <count>] [--out <path>]"
    );
    eprintln!(
        "    Parse telemetry::sorafs.fetch.* logs, enforce the burn-in SLO (window + PQ/brownout ratios + failure limits), and emit a JSON summary (stdout by default)."
    );
    eprintln!(
        "  cargo xtask sorafs-reserve-matrix [--capacity <GiB>]... [--storage-class <hot|warm|cold>]... [--tier <tier-a|tier-b|tier-c>]... [--duration <monthly|quarterly|annual>] [--policy-json <path>] [--policy-norito <path>] [--reserve-balance <XOR>] [--out <path|->]"
    );
    eprintln!(
        "    Generate a rent/reserve quote matrix for dashboards and economics tooling. Defaults to stdout when --out is omitted."
    );
    eprintln!("  cargo xtask sorafs-pin-fixtures [--out <path>]");
    eprintln!(
        "    Rebuild the pin registry snapshot fixture (manifests, aliases, replication orders). Defaults to crates/iroha_core/tests/fixtures/sorafs_pin_registry/snapshot.json"
    );
    eprintln!(
        "  cargo xtask offline-pos-provision --spec <path> [--output <dir>] [--operator-key <ed25519:...>]"
    );
    eprintln!(
        "    Generate POS manifest and revocation bundles for OA12 pilots. Defaults to artifacts/offline_pos_provision."
    );
    eprintln!(
        "  cargo xtask offline-pos-verify --bundle <revocations.json> [--api-snapshot <path|->]"
    );
    eprintln!(
        "    Verify the signature on a revocation bundle and optionally emit a Torii-style `/v1/offline/revocations` snapshot for USB/QR distribution."
    );
    eprintln!("  cargo xtask nexus-fixtures [--out <dir>] [--verify]");
    eprintln!(
        "    Regenerate Nexus lane commitment fixtures (defaults to fixtures/nexus/lane_commitments); pass --verify to ensure existing files match the generated payloads."
    );
    eprintln!(
        "  cargo xtask nexus-lane-maintenance --config <path> [--json-out <path|->] [--compact-retired]"
    );
    eprintln!(
        "    Survey Kura lane storage using the lane catalog, listing active segments and retired directories/logs; pass --compact-retired to archive retired paths under <store>/retired."
    );
    eprintln!(
        "  cargo xtask nexus-lane-audit --status <status.json> [--json-out <path>] [--parquet-out <path>] [--markdown-out <path>] [--captured-at <iso8601>] [--lane-compliance <path>]"
    );
    eprintln!(
        "    Export the current lane telemetry snapshot (JSON + Parquet + Markdown) for regulators. Defaults to artifacts/nexus_lane_audit.{{json,parquet,md}}; pass --lane-compliance to embed policy/review evidence."
    );
    eprintln!("  cargo xtask space-directory encode --json <path> [--out <path>]");
    eprintln!(
        "    Encode an AssetPermissionManifest JSON file into Norito bytes (.to). Defaults to replacing the input extension with .to"
    );
    eprintln!(
        "  cargo xtask sorafs-fetch-fixture --signatures <path|url> [--manifest <path|url>] [--out <dir>] [--profile <handle>] [--allow-unsigned]"
    );
    eprintln!(
        "    Download the Parliament-approved chunker manifest + signature envelope, verify digests/signatures, and write them to fixtures/sorafs_chunker."
    );
    eprintln!(
        "  cargo xtask sorafs-gateway-attest --signing-key <path> --signer-account <account> [--gateway <url>] [--out <dir>]"
    );
    eprintln!(
        "    Run the SoraFS gateway conformance harness, then write the JSON report, attestation envelope, and summary to artifacts/sorafs_gateway_attest unless --out is provided."
    );
    eprintln!(
        "  cargo xtask sorafs-gateway-probe --gateway <url>|--headers-file <path> --gar <path> --gar-key kid=hex [options]"
    );
    eprintln!(
        "    Fetch gateway headers (or parse a captured dump) and verify Sora-* headers, GAR manifest metadata, CSP/HSTS templates, cache TTLs, and TLS state."
    );
    eprintln!(
        "    Use --report-json <path|-> to capture a machine-readable summary for paging/drill automation."
    );
    eprintln!(
        "  cargo xtask sorafs-gateway tls renew --host <hostname>... --out <dir> [--account-email <email>] [--directory-url <url>] [--dns-provider-id <id>] [--force]"
    );
    eprintln!(
        "    Generate a TLS bundle using the self-signed ACME client and write fullchain.pem, privkey.pem, and ech.json to the target directory."
    );
    eprintln!(
        "  cargo xtask sorafs-gateway tls revoke --out <dir> [--archive-dir <dir>] [--reason <text>] [--force]"
    );
    eprintln!(
        "    Archive the current TLS bundle into a timestamped backup to simulate revocation on hosts without production ACME wiring."
    );
    eprintln!(
        "  cargo xtask sorafs-gateway key rotate --kind token-signing --out <path> [--public-out <path>] [--force]"
    );
    eprintln!(
        "    Rotate the stream-token signing key, writing the private key to disk and printing the new public key fingerprint."
    );
    eprintln!(
        "  cargo xtask soranet-privacy-report --input <ndjson> [--input <ndjson> ...] [--bucket-secs <n>] [--min-contributors <n>] [--json-out <path|->] [--max-buckets <n>] [--max-suppression-ratio <0-1>]"
    );
    eprintln!(
        "    Summarise SNNet-8 privacy buckets from NDJSON/Prio share exports, highlight suppression reasons, optionally emit machine-readable reports, and fail fast when suppression exceeds the provided ratio budget."
    );
    eprintln!(
        "  cargo xtask soranet-constant-rate-profile [--profile core|home] [--format table|json|markdown] [--tick-table] [--tick-values <v1,v2,...>]"
    );
    eprintln!(
        "    Print the SNNet-17B constant-rate presets (core/home) and optional tick→bandwidth tables so relay operators and SDK tooling can apply consistent lane budgets."
    );
    eprintln!(
        "  cargo xtask norito-rpc-fixtures [--fixtures <path>] [--exporter <Cargo.toml>] [--out-dir <dir>] [--selection <path>] [--all] [--skip-encoded-check]"
    );
    eprintln!(
        "    Regenerate the canonical Norito-RPC fixtures/manifest under fixtures/norito_rpc/"
    );
    eprintln!("  cargo xtask norito-rpc-verify [--json-out <path|->]");
    eprintln!(
        "    Start a local Torii router, ensure selected endpoints return identical responses for JSON/Norito payloads, and optionally emit a JSON verification report."
    );
    eprintln!("  cargo xtask soranet-testnet-kit [--out <dir>]");
    eprintln!(
        "    Materialise the SoraNet testnet operator kit. Defaults to docs/examples/soranet_testnet_operator_kit"
    );
    eprintln!("  cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]");
    eprintln!(
        "    Evaluate SNNet-10 success metrics from an aggregated snapshot. Emits a pass/fail report (use --out - for stdout)."
    );
    eprintln!(
        "  cargo xtask soranet-testnet-feed --promotion <label> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --metrics-report <path> [--relay <id>]... [--relays-file <path>] [--drill-log <path>] [--stage-report <path>] [--attachment label=path]... [--out <path|->]"
    );
    eprintln!(
        "    Aggregate metrics, relay roster, and artefact hashes into a deterministic JSON feed for SNNet-10 stage-gate reviews."
    );
    eprintln!(
        "  cargo xtask soranet-testnet-drill-bundle --log <path> --signing-key <path> [--promotion <label>] [--window-start <YYYY-MM-DD>] [--window-end <YYYY-MM-DD>] [--attachment label=path]... [--out <path|->]"
    );
    eprintln!(
        "    Sign SNNet-10 drill logs plus attachments and emit a deterministic bundle for governance packets and feed ingestion."
    );
    eprintln!(
        "  cargo xtask fastpq-bench-manifest --bench <label=path>... [--require-rows <count>] [--max-operation-ms op=value]... [--min-operation-speedup op=value]... [--matrix <path>] [--signing-key <path>] [--out <path>]"
    );
    eprintln!(
        "    Validate Metal/CUDA benchmark bundles, enforce latency/speedup thresholds, and emit a (optionally signed) manifest with BLAKE3/SHA-256 digests for release gating."
    );
    eprintln!(
        "  cargo xtask fastpq-stage-profile [--rows <count>] [--warmups <count>] [--iterations <count>] [--out-dir <path>] [--trace] [--trace-dir <path>] [--trace-template <template>] [--trace-seconds <seconds>] [--stage <fft|ifft|lde|poseidon>] [--debug] [--no-gpu-probe]"
    );
    eprintln!(
        "    Run the Metal bench across selected stages, capture optional traces, and emit per-stage summaries for local profiling."
    );
    eprintln!(
        "  cargo xtask fastpq-cuda-suite [--rows <count>] [--warmups <count>] [--iterations <count>] [--columns <count>] [--output <path>] [--raw-output <path>] [--row-usage <path>] [--label key=value]... [--device <label>] [--notes <text>] [--require-gpu] [--sign-output] [--gpg-key <id>] [--accel-instance <label>] [--accel-state-json <path>] [--accel-state-prom <path>] [--no-wrap] [--dry-run]"
    );
    eprintln!(
        "    Drive the CUDA bench harness, optionally wrap/sign the bundle with row-usage/acceleration-state metadata, and record a plan JSON so GPU runners produce reproducible Stage7 evidence."
    );
    eprintln!(
        "  cargo xtask soranet-rollout-plan --regions <r1,r2,...> --start <RFC3339> [--window <dur>] [--spacing <dur>] [--client-offset <dur>] [--phase <label>] [--environment <name>] [--out <path>] [--markdown-out <path>]"
    );
    eprintln!(
        "    Generate SNNet-16 rollout scheduling artefacts (JSON/Markdown). Duration arguments accept s/m/h/d suffixes."
    );
    eprintln!(
        "  cargo xtask soranet-rollout-capture --log <path> --key <ed25519_hex> [--artifact kind=...,path=...]... [--out <dir>] [--phase <label>] [--environment <name>] [--label <tag>] [--note <text>]"
    );
    eprintln!(
        "    Copy rollout drill artefacts, compute BLAKE3 digests, and emit a signed metadata package for rollback rehearsals."
    );
    eprintln!(
        "  cargo xtask sm-wycheproof-sync (--input <path>|--input-url <url>) [--output <path>] [--generator-version <tag>] [--minify|--pretty]"
    );
    eprintln!(
        "    Sanitize an upstream Wycheproof SM2 JSON suite and write the trimmed fixture to crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json."
    );
    eprintln!(
        "  cargo xtask sm-operator-snippet [--distid <id>] [--seed-hex <hex>] [--json-out <path|->] [--snippet-out <path|->]"
    );
    eprintln!(
        "    Generate SM2 operator artifacts (`sm2-key.json`, `client-sm2.toml`) without relying on jq (use `-` to stream to stdout)."
    );
    eprintln!(
        "  cargo xtask codec rans-tables [--seed <u64>] [--bundle-width <2-4>] [--output <path>] [--format json|toml|csv] [--signing-key <path>] [--verify [path]]"
    );
    eprintln!(
        "    Generate deterministic rANS initialisation tables for NSC-55. Defaults to artifacts/nsc/rans_tables.(json|toml). Use --bundle-width to pick the maximum bundle size; narrower tables are derived deterministically."
    );
    eprintln!("  cargo xtask verify-tables [--tables <path> ...]");
    eprintln!(
        "    Verify SignedRansTablesV1 artefacts by re-running deterministic generation and signature checks. Defaults to codec/rans/tables/rans_seed0.toml."
    );
    eprintln!(
        "    Use --signing-key to attach an Ed25519 signature and --verify to validate existing artefacts against the current generator."
    );
    eprintln!(
        "  cargo xtask streaming-bundle-check --config <path> [--tables <path>] [--json-out <path|->]"
    );
    eprintln!(
        "    Inspect the streaming codec config, load the SignedRansTablesV1 artefact, and emit checksum metadata in JSON form for rollout/runbook evidence."
    );
    eprintln!(
        "  cargo xtask streaming-entropy-bench [--frames <count>] [--segments <count>] [--width <n>] [--quantizer <qp>]... [--target-bitrate-mbps <mbps>]... [--tiny-clip-preset] [--psnr-mode y|yuv] [--json-out <path|->]"
    );
    eprintln!(
        "    Benchmark baseline vs bundled (when enabled) entropy encoding/decoding and emit chunk/latency metrics as JSON for CI dashboards. PSNR supports luma-only (`--psnr-mode y`) and full YUV (`--psnr-mode yuv`). Pass `--quantizer` multiple times to sweep QPs, `--target-bitrate-mbps` to record ladder selections, and `--tiny-clip-preset` for 16–32 px clips."
    );
    eprintln!(
        "  cargo xtask streaming-decode --bundle <path> --y4m-out <path> [--psnr-ref <y4m>] [--psnr-mode y|yuv] [--json-out <path|->]"
    );
    eprintln!(
        "    Decode serialized Norito streaming bundles into Y4M clips for RD tooling; optionally compute PSNR/PSNR-YUV against a reference Y4M."
    );
    eprintln!(
        "  cargo xtask stage1-bench [--size <bytes|Nk|Nm>]... [--iterations <n>] [--json-out <path>] [--markdown-out <path>] [--allow-overwrite]"
    );
    eprintln!(
        "    Benchmark Norito JSON stage-1 (scalar vs accelerated) across sizes, emit JSON/Markdown summaries, and report a recommended acceleration threshold."
    );
    eprintln!(
        "  cargo xtask poseidon-cuda-bench [--batch-size <n>] [--iterations <n>] [--json-out <path>] [--markdown-out <path>] [--allow-overwrite]"
    );
    eprintln!(
        "    Run Poseidon2/6 parity + throughput for scalar vs CUDA backends, emitting JSON/Markdown under benchmarks/poseidon with CUDA/Metal runtime health."
    );
    eprintln!(
        "  cargo xtask streaming-context-remap --input <bundled_telemetry.json> [--top <n>] [--json-out <path|->]"
    );
    eprintln!(
        "    Analyse bundled telemetry context frequencies, emit a deterministic remap table for the top-N contexts, and record the dominant symbol mix for pruning/refresh runbooks."
    );
    eprintln!(
        "  cargo xtask ministry-transparency ingest --quarter <YYYY-Q> --ledger <path> --appeals <path> --denylist <path> --treasury <path> [--volunteer <path>] [--red-team-report <path> ...] --output <path>"
    );
    eprintln!(
        "    Run the Ministry transparency ingest job to build the quarterly snapshot described in docs/source/ministry/transparency_plan.md."
    );
    eprintln!(
        "  cargo xtask ministry-transparency build --ingest <path> --metrics-out <path> --manifest-out <path> [--note <text>]"
    );
    eprintln!(
        "    Produce dashboard-ready metrics and a signed manifest from an ingest snapshot so MINFO-8 transparency packets are deterministic."
    );
    eprintln!(
        "  cargo xtask ministry-transparency sanitize --ingest <path> --output <path> --report <path> [--epsilon-counts <f64> --epsilon-accuracy <f64> --delta <f64> --suppress-threshold <u64> --min-accuracy-samples <u64> --seed <u64>]"
    );
    eprintln!(
        "    Apply the DP sanitizer from docs/source/ministry/transparency_plan.md to an ingest snapshot, emitting sanitized metrics and an audit report."
    );
    eprintln!(
        "  cargo xtask ministry-transparency volunteer-validate --input <path> [--input <path> ...] [--json-output <path>]"
    );
    eprintln!(
        "    Validate volunteer brief payloads against docs/source/ministry/volunteer_brief_template.md before publishing."
    );
    eprintln!(
        "  cargo xtask ministry-agenda validate --proposal <path> [--registry <path>] [--allow-registry-conflicts]"
    );
    eprintln!(
        "    Validate Agenda Council proposal payloads (docs/source/ministry/agenda_council_proposal.md) and detect duplicate target fingerprints."
    );
    eprintln!(
        "  cargo xtask ministry-agenda sortition --roster <path> --slots <count> --seed <hex> [--out <path>]"
    );
    eprintln!(
        "    Generate a deterministic Agenda Council draw with Merkle proofs for audit (docs/source/ministry/agenda_council_proposal.md#sortition-cli)."
    );
    eprintln!(
        "  cargo xtask ministry-agenda impact [--proposal <path>]... [--proposal-dir <dir>]... [--registry <path>] [--policy-snapshot <path>] [--out <path>]"
    );
    eprintln!(
        "    Summarize proposal hash families, counting duplicate-registry hits and policy conflicts for MINFO-4b referendum packets."
    );
    eprintln!(
        "  cargo xtask ministry-panel synthesize --proposal <path> --volunteer <path> --ai-manifest <path> --panel-round <RP-YYYY-##> --output <path> [--language <tag> --generated-at <unix-ms>]"
    );
    eprintln!(
        "    Generate the review panel neutral summary + lint report for roadmap item MINFO-4a (docs/source/ministry/review_panel_summary.md)."
    );
    eprintln!(
        "  cargo xtask ministry-jury sortition --roster <path> --proposal <id> --round <id> --beacon <hex> --committee-size <count> --waitlist-size <count> --drawn-at <RFC3339> [--waitlist-ttl-hours <hours>] [--grace-period <secs>] [--failover-grace <secs>] [--out <path>]"
    );
    eprintln!(
        "    Produce a PolicyJurySortitionV1 manifest for roadmap item MINFO-5 (docs/source/ministry/policy_jury_ballots.md), wiring deterministic draws + waitlists into referendum packets."
    );
    eprintln!(
        "  cargo xtask mochi-bundle [--out <path>] [--profile <name>] [--no-archive] [--kagami <path>] [--matrix <path>] [--smoke] [--stage <path>]"
    );
    eprintln!("    Build the MOCHI desktop bundle with a manifest and optional .tar.gz archive.");
    eprintln!(
        "    Use --kagami to point at a prebuilt kagami binary instead of building one from the workspace."
    );
    eprintln!(
        "    Use --matrix to append the bundle metadata to a JSON matrix (created if missing)."
    );
    eprintln!(
        "    Use --stage to copy the bundle (and archive when present) into a shared staging directory."
    );
    eprintln!("    Use --smoke to run the packaged `mochi --help` as a basic execution gate.");
    eprintln!(
        "  cargo xtask iso-bridge-lint [--isin <path>] [--bic-lei <path>] [--mic <path>] [--fixtures <path>]"
    );
    eprintln!(
        "    Lint ISO bridge reference data and fixture bundles (defaults to repository samples)."
    );
    eprintln!(
        "  cargo xtask offline-pos-provision --spec <path> [--output <dir>] [--operator-key <key>]"
    );
    eprintln!(
        "    Generate OfflinePosProvisionManifest fixtures (and optional revocation bundles). Defaults to artifacts/offline_pos_provision."
    );
    eprintln!(
        "  cargo xtask offline-provision --spec <path> [--output <dir>] [--inspector-key <key>]"
    );
    eprintln!(
        "    Produce AndroidProvisioned proofs (inspector manifests) for kiosks. Defaults to artifacts/offline_provision."
    );
    eprintln!(
        "  cargo xtask offline-topup --spec <path> [--output <dir>] [--operator-key <key>] [--register-config <path>] [--register-mode blocking|immediate]"
    );
    eprintln!("  cargo xtask offline-bundle --spec <path> [--output <dir>]");
    eprintln!(
        "    Emit OfflineToOnlineTransfer fixtures plus FASTPQ witness requests. Defaults to artifacts/offline_bundle."
    );
    println!(
        "  cargo xtask offline-poseidon-fixtures [--constants <path>] [--vectors <path>] [--tag <domain_tag>] [--no-sdk-mirror]"
    );
    eprintln!(
        "    Generate offline allowance fixtures and optionally register them against a Torii config. Defaults to fixtures/offline_allowance."
    );
    eprintln!("  cargo xtask acceleration-state [--format table|json]");
    eprintln!(
        "    Print the applied acceleration configuration and Metal/CUDA runtime status to feed parity dashboards."
    );
}

#[cfg(test)]
mod tests {
    use norito::json::Value;

    use super::*;
    #[test]
    fn stub_spec_contains_minimal_metadata() {
        let spec = build_stub_spec();
        assert_eq!(spec["openapi"], norito::json!("3.1.0"));
        assert_eq!(spec["info"]["title"], norito::json!("Iroha Torii API"));
        assert!(spec["paths"].as_object().unwrap().is_empty());
    }

    #[test]
    fn vote_tally_default_path_points_into_fixtures() {
        let default = default_vote_tally_path();
        assert!(default.ends_with("fixtures/zk/vote_tally"));
    }

    #[test]
    fn soranet_fixture_default_path_points_into_tests() {
        let default = soranet::default_fixture_dir(&workspace_root());
        assert!(default.ends_with("tests/interop/soranet/capabilities"));
    }

    #[test]
    fn sm_operator_snippet_with_seed_emits_expected_files() {
        let temp = TempDir::new().expect("temp dir");
        let json_path = temp.path().join("output").join("sm2-key.json");
        let snippet_path = temp.path().join("output").join("client-sm2.toml");
        let options = crate::sm::SmOperatorSnippetOptions {
            distid: Some("CN12345678901234".to_string()),
            seed_hex: Some(
                "00112233445566778899AABBCCDDEEFF00112233445566778899AABBCCDDEEFF".to_string(),
            ),
            json_out: Some(crate::sm::OutputTarget::file(json_path.clone())),
            snippet_out: Some(crate::sm::OutputTarget::file(snippet_path.clone())),
        };
        crate::sm::generate_sm_operator_snippet(options).expect("generate snippet");

        let json_text = std::fs::read_to_string(&json_path).expect("read sm2-key.json");
        let value: Value = norito::json::from_str(&json_text).expect("parse sm2 json");
        assert_eq!(
            value["distid"],
            norito::json!("CN12345678901234"),
            "distid should match input"
        );
        assert!(
            value["public_key_config"]
                .as_str()
                .expect("public key string")
                .parse::<iroha_crypto::PublicKey>()
                .is_ok(),
            "public key config should round-trip via PublicKey::from_str"
        );

        let snippet = std::fs::read_to_string(&snippet_path).expect("read client-sm2.toml");
        assert!(
            snippet.contains("public_key = \""),
            "snippet should contain public key entry"
        );
        assert!(
            snippet.contains("[crypto]"),
            "snippet should include crypto section"
        );
        assert!(
            snippet.contains("default_hash = \"sm3-256\""),
            "snippet should set default_hash to sm3-256"
        );
        assert!(
            snippet.contains("allowed_signing = [\"ed25519\", \"sm2\"]"),
            "snippet should include sm2 in allowed_signing by default (with guidance comment)"
        );
        assert!(
            snippet.contains("sm2_distid_default = \"CN12345678901234\""),
            "snippet should embed the configured sm2_distid_default"
        );
        assert!(
            snippet.contains("# enable_sm_openssl_preview"),
            "snippet should mention optional OpenSSL preview toggle"
        );
    }

    #[test]
    fn sm_operator_snippet_supports_stdout_targets() {
        let temp = TempDir::new().expect("temp dir");
        let snippet_path = temp.path().join("client-sm2.toml");
        let options = crate::sm::SmOperatorSnippetOptions {
            distid: Some("CN5555444433332222".to_string()),
            seed_hex: Some(
                "AA11223344556677889900AABBCCDDEEFF00112233445566778899AABBCCDD00".to_string(),
            ),
            json_out: Some(crate::sm::OutputTarget::Stdout),
            snippet_out: Some(crate::sm::OutputTarget::file(snippet_path.clone())),
        };
        crate::sm::generate_sm_operator_snippet(options).expect("generate snippet");

        assert!(
            !snippet_path.parent().unwrap().join("sm2-key.json").exists(),
            "JSON file should not be created when streamed to stdout"
        );
        let snippet = std::fs::read_to_string(&snippet_path).expect("read snippet");
        assert!(
            snippet.contains("[crypto]"),
            "stdout run should still produce snippet file when requested"
        );
    }

    #[test]
    fn iso_bridge_lint_uses_default_reference_data() {
        lint_iso_bridge(IsoLintOptions::default()).expect("default iso lint should succeed");
    }

    #[cfg(feature = "vote-tally")]
    #[test]
    fn vote_tally_bundle_matches_expected_hashes() {
        let temp = TempDir::new().expect("temp dir");
        let summary = write_bundle(temp.path()).expect("write bundle");
        let attestation =
            vote_tally::attestation_manifest(&summary, temp.path()).expect("attestation manifest");
        assert_eq!(
            attestation["generated_unix_ms"],
            norito::json!(3513801751697071715u64)
        );
        assert_eq!(
            attestation["hash_algorithm"],
            norito::json!("blake2b-256"),
            "attestation must record the hash function"
        );
        let artifacts = attestation["artifacts"]
            .as_array()
            .expect("artifacts array");
        assert_eq!(
            artifacts.len(),
            vote_tally::bundle_file_names().len(),
            "expected attestation to include all bundle artifacts"
        );
        for expected in vote_tally::bundle_file_names() {
            let present = artifacts
                .iter()
                .any(|entry| entry["file"] == norito::json!(*expected));
            assert!(present, "missing artifact entry for {expected}");
        }
        let artifact_map = artifacts_to_map(artifacts).expect("artifact map");
        let meta_entry = artifact_map
            .get("vote_tally_meta.json")
            .expect("meta entry present");
        assert!(meta_entry.0 > 0);
        let proof_entry = artifact_map
            .get("vote_tally_proof.zk1")
            .expect("proof entry present");
        assert_eq!(proof_entry.0, summary.proof_len as u64);
        let vk_entry = artifact_map
            .get("vote_tally_vk.zk1")
            .expect("vk entry present");
        assert_eq!(vk_entry.0, summary.vk_len as u64);

        assert_eq!(
            summary.backend,
            "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1"
        );
        assert_eq!(
            summary.circuit_id,
            "halo2/pasta/vote-bool-commit-merkle8-v1"
        );
        assert_eq!(
            summary.commit_hex,
            "20574662a58708e02e0000000000000000000000000000000000000000000000"
        );
        assert_eq!(
            summary.root_hex,
            "b63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817"
        );
        assert_eq!(
            summary.schema_hash_hex,
            "fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3"
        );
        assert_eq!(
            summary.vk_commit_hex,
            "6f4749f5f75fee2a40880d4798123033b2b8036284225bad106b04daca5fb10e"
        );
        assert!(summary.vk_len > 0);
        assert!(summary.proof_len > 0);
    }

    #[cfg(feature = "vote-tally")]
    #[test]
    fn attestation_verification_rejects_proof_digest_drift() {
        let baseline = TempDir::new().expect("baseline dir");
        let summary = write_bundle(baseline.path()).expect("write bundle");
        let manifest_value =
            vote_tally::attestation_manifest(&summary, baseline.path()).expect("manifest");
        let manifest_path = baseline.path().join("bundle.attestation.json");
        let mut manifest_text = norito::json::to_string_pretty(&manifest_value).unwrap();
        manifest_text.push('\n');
        std::fs::write(&manifest_path, manifest_text).unwrap();

        let mut parsed: Value =
            norito::json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        if let Some(object) = parsed.as_object_mut() {
            if let Some(Value::Array(artifacts)) = object.get_mut("artifacts") {
                for entry in artifacts {
                    if let Some(map) = entry.as_object_mut() {
                        if map.get("file") == Some(&norito::json!("vote_tally_proof.zk1")) {
                            map.insert(
                                "blake2b_256".into(),
                                norito::json!(
                                    "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
                                ),
                            );
                        }
                    }
                }
            }
        }
        let mut mutated = norito::json::to_string_pretty(&parsed).unwrap();
        mutated.push('\n');
        std::fs::write(&manifest_path, mutated).unwrap();
        let err = handle_attestation_manifest(
            &summary,
            baseline.path(),
            JsonTarget::File(manifest_path.clone()),
            true,
        )
        .expect_err("proof digest drift must be rejected");
        assert!(
            err.to_string().contains("vote_tally_proof.zk1"),
            "error must cite proof artefact"
        );
    }

    #[cfg(feature = "vote-tally")]
    #[test]
    fn attestation_verification_rejects_metadata_drift() {
        let baseline = TempDir::new().expect("baseline dir");
        let summary = write_bundle(baseline.path()).expect("write bundle");
        let manifest_value =
            vote_tally::attestation_manifest(&summary, baseline.path()).expect("manifest");
        let manifest_path = baseline.path().join("bundle.attestation.json");
        let mut manifest_text = norito::json::to_string_pretty(&manifest_value).unwrap();
        manifest_text.push('\n');
        std::fs::write(&manifest_path, manifest_text).unwrap();

        // Mutate the meta file digest (deterministic artefact) and ensure verification fails.
        let mut parsed: Value =
            norito::json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        if let Some(object) = parsed.as_object_mut() {
            if let Some(Value::Array(artifacts)) = object.get_mut("artifacts") {
                for entry in artifacts {
                    if let Some(map) = entry.as_object_mut() {
                        if map.get("file") == Some(&norito::json!("vote_tally_meta.json")) {
                            map.insert(
                                "blake2b_256".into(),
                                norito::json!(
                                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                                ),
                            );
                        }
                    }
                }
            }
        }
        let mut mutated = norito::json::to_string_pretty(&parsed).unwrap();
        mutated.push('\n');
        std::fs::write(&manifest_path, mutated).unwrap();
        let err = handle_attestation_manifest(
            &summary,
            baseline.path(),
            JsonTarget::File(manifest_path.clone()),
            true,
        )
        .expect_err("metadata drift must be rejected");
        assert!(
            err.to_string().contains("vote_tally_meta.json"),
            "error should reference the divergent artefact"
        );
    }

    #[test]
    fn verify_requires_seeded_baseline() {
        let baseline = TempDir::new().expect("baseline dir");
        let err =
            generate_vote_tally_bundle(baseline.path().to_path_buf(), true, false, None, None)
                .expect_err("verify without seeded fixtures must fail");
        let message = err.to_string();
        assert!(
            message.contains("run `cargo xtask zk-vote-tally-bundle"),
            "error message should suggest seeding baseline, got: {message}"
        );
    }
}
