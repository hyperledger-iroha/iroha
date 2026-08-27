//! `SoraFS` helper commands for interacting with Torii REST endpoints.
#![allow(clippy::size_of_ref)]
use super::{
    da::{normalize_ticket_hex, persist_manifest_bundle},
    da_common::DaManifestFetcher,
};
mod hedging_billing_response;
use crate::{CliOutputFormat, Run, RunContext, cli_output::print_with_optional_text};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use eyre::{Result, WrapErr, eyre};
use hex::{decode, decode_to_slice, encode};
use iroha::{
    client::{
        Client, SorafsAliasListFilter, SorafsAppealFinanceReadbackFilter,
        SorafsBillingAcknowledgementProof, SorafsBillingStatementListFilter,
        SorafsGatewayFetchOptions, SorafsGatewayScoreboardOptions, SorafsHedgingProjectionFilter,
        SorafsModerationBallotEventsFilter, SorafsModerationBallotsFilter,
        SorafsModerationModelRegistryFilter, SorafsModerationQuarantineFilter,
        SorafsModerationQuarantineObjectStoreRequest, SorafsModerationQuarantineReleaseRequest,
        SorafsModerationQuarantineReviewRequest, SorafsModerationScreeningResultRequest,
        SorafsModerationScreeningResultsFilter, SorafsPinAlias, SorafsPinFinalizedAnchor,
        SorafsPinListFilter, SorafsPinRegisterArgs, SorafsRepairFinalizedAnchor,
        SorafsRepairTasksFilter, SorafsReplicationListFilter, SorafsReplicationStatus,
        SorafsTokenOverrides, SorafsTransparencyReadbackFilter,
    },
    http::{Response, StatusCode},
};
use iroha_config::{
    client_api::{
        ConfigUpdateDTO, Logger as LoggerDTO, NetworkUpdate, ResumeHashDirective,
        SoranetHandshakePowUpdate, SoranetHandshakePuzzleUpdate, SoranetHandshakeSummary,
        SoranetHandshakeUpdate,
    },
    parameters::defaults,
};
use iroha_core::soranet_incentives::{RelayEarningsAccumulator, RelayPayoutLedger};
use iroha_crypto::{
    HashOf, HybridPublicKey, HybridSuite,
    soranet::{
        blinding::canonical_cache_key,
        directory::{
            GuardDirectorySnapshotV2, compute_snapshot_digest, read_guard_directory_snapshot_file,
        },
        token::{AdmissionToken, MintError as AdmissionTokenMintError, compute_issuer_fingerprint},
    },
};
use iroha_data_model::sorafs::pin_registry::PinStatusKindV1;
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    isi::{
        InstructionBox, Transfer,
        sorafs::{
            ApplySorafsRepairTaskAction, FinalizeSorafsModerationCase, SorafsRepairClaimV1,
            SorafsRepairCompleteV1, SorafsRepairEscalateV1, SorafsRepairFailV1,
            SorafsRepairRenewV1, SorafsRepairTaskActionV1, SubmitSorafsModerationCommit,
            SubmitSorafsModerationReveal,
        },
    },
    metadata::Metadata,
    name::Name,
    prelude::ChainId,
    sorafs::{
        gar::{GarEnforcementActionV1, GarEnforcementReceiptV1},
        moderation::{
            AdversarialCorpusManifestV1, ModerationReproManifestV1, SoraFsModerationBallotCommitV1,
            SoraFsModerationBallotRevealV1,
        },
        pin_registry::StorageClass,
        reserve::{
            ReserveDuration, ReserveLedgerProjection, ReserveLifecycleProjection,
            ReserveLifecycleStage, ReservePolicyV1, ReserveQuote, ReserveTier,
        },
    },
    soranet::{
        RelayId,
        incentives::{
            RelayBondLedgerEntryV1, RelayBondPolicyV1, RelayComplianceStatusV1,
            RelayEpochMetricsV1, RelayRewardInstructionV1,
        },
    },
    transaction::{FeePaymentIntent, SignedTransaction},
};
use iroha_primitives::numeric::{Numeric, Quantity};
use iroha_torii_shared::sorafs_hedging_billing_api::BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1 as SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1;
use norito::json::{Map, Number, Value};
use norito::{NoritoSerialize, decode_from_bytes};
use rand::{
    CryptoRng, RngCore, SeedableRng,
    rand_core::{TryCryptoRng, TryRngCore},
    rngs::{OsRng, StdRng},
};
use reqwest::blocking::Client as BlockingHttpClient;
use sorafs_car::{
    CarBuildPlan, CarChunk, CarWriteStats, CarWriter, ChunkStore, FilePlan, PorMerkleTree,
    fetch_plan::{
        TOOLKIT_PACK_REPORT_SCHEMA_V1, chunk_fetch_plan_from_json, parse_digest_hex,
        try_chunk_fetch_specs_to_json,
    },
};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::chunker_registry;
use sorafs_manifest::deal::XorQuantity;
use sorafs_manifest::repair::{
    REPAIR_SLASH_PROPOSAL_VERSION_V1, RepairSlashProposalV1, RepairTicketId,
};
use sorafs_manifest::{
    ChunkingProfileV1, DagCodecId, GovernanceProofs, ManifestBuilder, ManifestV1, PinPolicy,
    StorageClass as ManifestStorageClass,
    hosts::{DirectCarLocator, HostMappingInput, HostMappingSummary},
    hybrid_envelope::{
        HYBRID_PAYLOAD_ENVELOPE_VERSION_V1, HybridPayloadEnvelopeV1, encrypt_payload,
    },
    manifest_capabilities::{
        ChunkProfileSummary, ManifestCapabilitySummary, detect_manifest_capabilities,
    },
    provider_admission::ProviderAdmissionEnvelopeV1,
    provider_advert::{CapabilityType, ProviderCapabilityRangeV1},
};
use sorafs_orchestrator::{
    AnonymityPolicy, PolicyOverride, TransportPolicy, WriteModeHint,
    incentives::{RelayRewardEngine, RewardConfig},
    prelude::{
        BrowserExtensionManifest, GUARD_CACHE_MAX_BYTES_V1, GatewayFetchConfig,
        GatewayProviderInput, GuardCacheKey, GuardRetention, GuardSelector, GuardSet,
        PayoutServiceError, RelayDirectory, RewardLedgerError,
    },
    treasury::{
        AdjustmentKind, AdjustmentRequest, DisputeId, DisputeResolution, DisputeStatus,
        EarningsDashboard, EarningsRow, LedgerAmountArithmeticError, LedgerAmountSource,
        LedgerReconciliationReport, LedgerTransferMismatch, LedgerTransferRecord, MismatchReason,
        PayoutInput, QuantityToNanosError, RelayPayoutService, ResolutionKind, RewardDispute,
        RewardLedgerSnapshot, TransferKind,
    },
};
use soranet_pq::MlDsaSuite;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    convert::TryFrom,
    fmt::{self, Write as _},
    fs,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    num::{NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use time::{Duration as TimeDelta, OffsetDateTime, format_description::well_known::Rfc3339};
use tiny_keccak::{Hasher as _, Sha3};
use tokio::runtime::Runtime;
use zeroize::{Zeroize as _, Zeroizing};

macro_rules! impl_run_with_client_methods {
    ($args:ty, $($method:path),+ $(,)?) => {
        impl Run for $args {
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                self.run_with(context, $($method),+)
            }
        }
    };
}

macro_rules! impl_run_for_subcommand {
    ($(#[$attribute:meta])* $command:ident => $($variant:ident),+ $(,)?) => {
        impl Run for $command {
            $(#[$attribute])*
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                match self {
                    $(Self::$variant(args) => args.run(context)),+
                }
            }
        }
    };
}

macro_rules! impl_json_limit_run_with {
    ($args:ident => $filter:ident) => {
        impl $args {
            fn run_with<C, F>(&self, context: &mut C, request: F) -> Result<()>
            where
                C: RunContext,
                F: FnOnce(&Client, $filter) -> Result<Response<Vec<u8>>>,
            {
                let filter = $filter { limit: self.limit };
                let client = context.client_from_config();
                let response = request(&client, filter)?;
                render_json_response(context, response)
            }
        }
    };
}

macro_rules! impl_appeal_finance_submit_run_with {
    ($args:ident => $label:literal, $status:expr) => {
        impl $args {
            fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
            where
                C: RunContext,
                F: FnOnce(&Client, &[u8]) -> Result<Response<Vec<u8>>>,
            {
                run_appeal_finance_json_submit(context, &self.input, $label, submit, $status)
            }
        }
    };
}

macro_rules! impl_json_payload_run_with {
    ($args:ident.$field:ident => $label:literal, $render:path) => {
        impl $args {
            fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
            where
                C: RunContext,
                F: FnOnce(&Client, &[u8]) -> Result<Response<Vec<u8>>>,
            {
                let payload = load_sorafs_json_payload(&self.$field, $label)?;
                let client = context.client_from_config();
                let response = submit(&client, &payload)?;
                $render(context, response)
            }
        }
    };
}

macro_rules! impl_moderation_operator_derived_response {
    ($name:ident => $builder:path, $output:ident, $error:literal) => {
        fn $name(
            &self,
            quarantine_id_hex: &str,
            limit: Option<u32>,
        ) -> ModerationOperatorHttpResponse {
            let body = match self.operator_panel_body(quarantine_id_hex, limit) {
                Ok(body) => body,
                Err(response) => return response,
            };
            match moderation_operator_payload_free_panel_json(&body)
                .and_then(|panel| $builder(quarantine_id_hex, &panel))
            {
                Ok($output) => moderation_operator_json_response(StatusCode::OK, &$output),
                Err(err) => moderation_operator_json_error(
                    StatusCode::BAD_GATEWAY,
                    format!($error, err = err),
                ),
            }
        }
    };
}

#[cfg(test)]
macro_rules! assert_eq_compact {
    ($left:expr => $right:expr $(; $($arg:tt)*)?) => {
        assert_eq!($left, $right $(, $($arg)*)?)
    };
}
#[cfg(test)]
macro_rules! assert_compact {
    ($condition:expr $(; $($arg:tt)*)?) => {
        assert!($condition $(, $($arg)*)?)
    };
}

#[cfg(test)]
macro_rules! json_response_fixture {
    ($status:expr, $body:expr $(,)?) => {
        Ok(Response::builder()
            .status($status)
            .header("Content-Type", "application/json")
            .body(norito::json::to_vec($body)?)
            .unwrap())
    };
    ($status:expr, $body:expr, $message:expr) => {
        Ok(Response::builder()
            .status($status)
            .header("Content-Type", "application/json")
            .body(norito::json::to_vec($body)?)
            .expect($message))
    };
}

#[cfg(test)]
macro_rules! test_items {
    ($($item:item)*) => {
        $(#[test] $item)*
    };
}

#[allow(dead_code)]
const ML_KEM_768_PUBLIC_LEN: usize = 1184;
#[derive(clap::ValueEnum, Clone, Copy, Debug, Default)]
enum MlDsaSuiteArg {
    #[default]
    #[value(name = "mldsa44")]
    MlDsa44,
    #[value(name = "mldsa65")]
    MlDsa65,
    #[value(name = "mldsa87")]
    MlDsa87,
}
impl MlDsaSuiteArg {
    fn as_suite(self) -> MlDsaSuite {
        match self {
            Self::MlDsa44 => MlDsaSuite::MlDsa44,
            Self::MlDsa65 => MlDsaSuite::MlDsa65,
            Self::MlDsa87 => MlDsaSuite::MlDsa87,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::MlDsa44 => "ML-DSA-44",
            Self::MlDsa65 => "ML-DSA-65",
            Self::MlDsa87 => "ML-DSA-87",
        }
    }
}
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum StorageClassArg {
    #[value(name = "hot")]
    Hot,
    #[value(name = "warm")]
    Warm,
    #[value(name = "cold")]
    Cold,
}
impl StorageClassArg {
    fn to_storage_class(self) -> StorageClass {
        match self {
            Self::Hot => StorageClass::Hot,
            Self::Warm => StorageClass::Warm,
            Self::Cold => StorageClass::Cold,
        }
    }
}
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum ReserveTierArg {
    #[value(name = "tier-a")]
    TierA,
    #[value(name = "tier-b")]
    TierB,
    #[value(name = "tier-c")]
    TierC,
}
impl ReserveTierArg {
    fn to_policy_tier(self) -> ReserveTier {
        match self {
            Self::TierA => ReserveTier::TierA,
            Self::TierB => ReserveTier::TierB,
            Self::TierC => ReserveTier::TierC,
        }
    }
}
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum ReserveDurationArg {
    #[value(name = "monthly")]
    Monthly,
    #[value(name = "quarterly")]
    Quarterly,
    #[value(name = "annual")]
    Annual,
}
impl ReserveDurationArg {
    fn to_policy_duration(self) -> ReserveDuration {
        match self {
            Self::Monthly => ReserveDuration::Monthly,
            Self::Quarterly => ReserveDuration::Quarterly,
            Self::Annual => ReserveDuration::Annual,
        }
    }
}
#[cfg(test)]
mod capture_path_tests {
    use super::{default_orchestrator_capture_dir, scoreboard_capture_paths};
    use std::path::PathBuf;
    #[test]
    fn defaults_use_artifacts_directory() {
        let capture = scoreboard_capture_paths(None, None);
        let expected_dir = default_orchestrator_capture_dir();
        assert_eq!(capture.scoreboard, expected_dir.join("scoreboard.json"));
        assert_eq_compact! { capture.summary.as_ref() => Some(&expected_dir.join("summary.json")) };
    }
    #[test]
    fn scoreboard_override_preserves_parent_for_summary() {
        let base = PathBuf::from("/tmp/custom");
        let capture = scoreboard_capture_paths(Some(base.join("sb.json")), None);
        assert_eq!(capture.scoreboard, base.join("sb.json"));
        assert_eq!(capture.summary.as_ref(), Some(&base.join("summary.json")));
    }
    #[test]
    fn summary_override_wins() {
        let summary = PathBuf::from("/tmp/out.json");
        let capture = scoreboard_capture_paths(None, Some(summary.clone()));
        assert_eq!(capture.summary.as_ref(), Some(&summary));
    }
}
#[cfg(test)]
mod provider_count_tests {
    use super::{ProviderCounts, insert_provider_counts};
    use norito::json::Value;
    #[test]
    fn provider_counts_include_gateway_only_runs() {
        let mut summary = norito::json::Map::new();
        insert_provider_counts(&mut summary, ProviderCounts::new(0, 3));
        assert_eq_compact! { summary.get("provider_count").and_then(Value::as_u64) => Some(0) };
        assert_eq_compact! { summary.get("gateway_provider_count").and_then(Value::as_u64) => Some(3) };
        assert_eq_compact! { summary.get("provider_mix").and_then(Value::as_str) => Some("gateway-only") };
    }
    #[test]
    fn provider_counts_report_mixed_classifications() {
        let mut summary = norito::json::Map::new();
        insert_provider_counts(&mut summary, ProviderCounts::new(2, 2));
        assert_eq_compact! { summary.get("provider_mix").and_then(Value::as_str) => Some("mixed") };
    }
}
#[cfg(test)]
mod transport_policy_summary_tests {
    use super::{TransportPolicy, insert_transport_policy};
    use norito::json::Value;
    #[test]
    fn summary_records_transport_policy_overrides() {
        let mut summary = norito::json::Map::new();
        insert_transport_policy(
            &mut summary,
            Some(TransportPolicy::SoranetPreferred),
            Some(TransportPolicy::DirectOnly),
        );
        assert_eq_compact! { summary.get("transport_policy").and_then(Value::as_str) => Some("direct-only") };
        assert_eq_compact! { summary.get("transport_policy_override").and_then(Value::as_bool) => Some(true) };
        assert_eq_compact! { summary.get("transport_policy_override_label").and_then(Value::as_str) => Some("direct-only") };
    }
    #[test]
    fn summary_defaults_transport_policy_without_override() {
        let mut summary = norito::json::Map::new();
        insert_transport_policy(&mut summary, None, None);
        assert_eq_compact! { summary.get("transport_policy").and_then(Value::as_str) => Some("soranet-first") };
        assert_eq_compact! { summary.get("transport_policy_override").and_then(Value::as_bool) => Some(false) };
        assert_compact! { summary.get("transport_policy_override_label").is_none_or(Value::is_null) };
    }
}
#[cfg(test)]
mod telemetry_summary_tests {
    use super::{insert_summary_telemetry_region, insert_summary_telemetry_source};
    use norito::json::Value;
    #[test]
    fn summary_records_telemetry_label() {
        let mut summary = norito::json::Map::new();
        insert_summary_telemetry_source(&mut summary, Some("otel::prod"));
        assert_eq_compact! { summary.get("telemetry_source").and_then(Value::as_str) => Some("otel::prod") };
    }
    #[test]
    fn summary_omits_telemetry_label_when_missing() {
        let mut summary = norito::json::Map::new();
        insert_summary_telemetry_source(&mut summary, None);
        assert!(!summary.contains_key("telemetry_source"));
    }
    #[test]
    fn summary_records_telemetry_region() {
        let mut summary = norito::json::Map::new();
        insert_summary_telemetry_region(&mut summary, Some("iad-prod"));
        assert_eq_compact! { summary.get("telemetry_region").and_then(Value::as_str) => Some("iad-prod") };
    }
    #[test]
    fn summary_omits_telemetry_region_when_missing() {
        let mut summary = norito::json::Map::new();
        insert_summary_telemetry_region(&mut summary, None);
        assert!(!summary.contains_key("telemetry_region"));
    }
}
impl fmt::Display for MlDsaSuiteArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
#[derive(clap::ValueEnum, Clone, Copy, Debug, Default)]
enum TokenOutputFormat {
    #[default]
    #[value(name = "base64")]
    Base64,
    #[value(name = "hex")]
    Hex,
    #[value(name = "binary")]
    Binary,
}
impl TokenOutputFormat {
    fn describe(self) -> &'static str {
        match self {
            Self::Base64 => "base64url",
            Self::Hex => "hex",
            Self::Binary => "binary",
        }
    }
}
#[derive(clap::Subcommand, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Command {
    /// Interact with the pin registry.
    #[command(subcommand)]
    Pin(PinCommand),
    /// List alias bindings.
    #[command(subcommand)]
    Alias(AliasCommand),
    /// List replication orders.
    #[command(subcommand)]
    Replication(ReplicationCommand),
    /// Storage token helpers.
    #[command(subcommand)]
    Storage(StorageCommand),
    /// Gateway policy and configuration helpers.
    #[command(subcommand)]
    Gateway(GatewayCommand),
    /// Offline helpers for relay payouts, disputes, and dashboards.
    #[command(subcommand)]
    Incentives(IncentivesCommand),
    /// Observe or modify the Torii `SoraNet` handshake configuration.
    #[command(subcommand)]
    Handshake(HandshakeCommand),
    /// Local tooling for packaging manifests and payloads.
    #[command(subcommand)]
    Toolkit(ToolkitCommand),
    /// Guard directory helpers (fetch/verify snapshots).
    #[command(subcommand)]
    GuardDirectory(GuardDirectoryCommand),
    /// Reserve + rent policy helpers.
    #[command(subcommand)]
    Reserve(ReserveCommand),
    /// Appeal pricing and finance handoff helpers.
    #[command(subcommand)]
    Appeals(AppealsCommand),
    /// GAR policy evidence helpers.
    #[command(subcommand)]
    Gar(GarCommand),
    /// Transparency ledger readback and source-entry ingest helpers.
    #[command(subcommand)]
    Transparency(TransparencyCommand),
    /// Moderation queue and quarantine workflow helpers.
    #[command(subcommand)]
    Moderation(ModerationCommand),
    /// Repair queue helpers (list, claim, close, escalate).
    #[command(subcommand)]
    Repair(RepairCommand),
    /// Authenticated billing statement and reconciliation reads.
    #[command(subcommand)]
    Billing(BillingCommand),
    /// Authenticated finalized hedging projection reads.
    #[command(subcommand)]
    Hedging(HedgingCommand),
    /// GC inspection helpers (no manual deletions).
    #[command(subcommand)]
    Gc(GcCommand),
    /// Orchestrate multi-provider chunk fetches via gateways.
    Fetch(FetchArgs),
}
#[derive(clap::Subcommand, Debug)]
pub enum ReserveCommand {
    /// Quote reserve requirements and effective rent for a given tier/capacity.
    Quote(ReserveQuoteArgs),
    /// Convert a reserve quote into rent/reserve transfer instructions.
    Ledger(ReserveLedgerArgs),
    /// Project reserve lifecycle stage and automatic credit draw state.
    Lifecycle(ReserveLifecycleArgs),
}
impl_run_for_subcommand!(ReserveCommand => Quote, Ledger, Lifecycle);
const SORAFS_HEDGING_BILLING_MAX_PAGE_ITEMS_V1: u16 = 100;
fn required_nonzero_lower_hex32(value: &str, flag: &str) -> Result<String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(eyre!(
            "{flag} must be exactly 64 lowercase hexadecimal characters"
        ));
    }
    if value.bytes().all(|byte| byte == b'0') {
        return Err(eyre!("{flag} must be non-zero"));
    }
    Ok(value.to_owned())
}
fn required_hedging_billing_page_limit(limit: u16) -> Result<u16> {
    if !(1..=SORAFS_HEDGING_BILLING_MAX_PAGE_ITEMS_V1).contains(&limit) {
        return Err(eyre!(
            "--limit must be within 1..={SORAFS_HEDGING_BILLING_MAX_PAGE_ITEMS_V1}"
        ));
    }
    Ok(limit)
}
/// Authenticated SoraFS billing statement and reconciliation commands.
#[derive(clap::Subcommand, Debug)]
pub enum BillingCommand {
    /// Fetch the supervised billing projector status and current anchor.
    Status(BillingStatusArgs),
    /// List owner-isolated published statements from an exact checkpoint.
    Statements(BillingStatementsArgs),
    /// Fetch one exact published statement as canonical Norito.
    Statement(BillingStatementArgs),
    /// Submit an externally authenticated owner acknowledgement.
    Acknowledge(BillingAcknowledgeArgs),
    /// Fetch payload-free delivery reconciliation status.
    Reconciliation(BillingReconciliationArgs),
}
impl_run_for_subcommand!(BillingCommand => Status, Statements, Statement, Acknowledge, Reconciliation);
/// Fetch supervised billing projector status.
#[derive(clap::Args, Debug, Default)]
pub struct BillingStatusArgs {}
impl_run_with_client_methods!(BillingStatusArgs, Client::get_sorafs_billing_status);
impl BillingStatusArgs {
    fn run_with<C, F>(&self, context: &mut C, get: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client) -> Result<Response<Vec<u8>>>,
    {
        let client = context.client_from_config();
        render_json_response(context, get(&client)?)
    }
}
/// List owner-isolated published billing statements.
#[derive(clap::Args, Debug)]
pub struct BillingStatementsArgs {
    /// Exact non-zero lowercase checkpoint fingerprint from billing status.
    #[arg(long = "expected-checkpoint-fingerprint", value_name = "HEX")]
    expected_checkpoint_fingerprint: String,
    /// Optional exclusive non-zero lowercase statement identifier.
    #[arg(long = "after-statement-id", value_name = "HEX")]
    after_statement_id: Option<String>,
    /// Required page size in the inclusive range 1 through 100.
    #[arg(long, value_name = "COUNT")]
    limit: u16,
}
impl_run_with_client_methods!(BillingStatementsArgs, Client::get_sorafs_billing_statements);
impl BillingStatementsArgs {
    fn run_with<C, F>(&self, context: &mut C, list: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, SorafsBillingStatementListFilter<'_>) -> Result<Response<Vec<u8>>>,
    {
        let checkpoint = required_nonzero_lower_hex32(
            &self.expected_checkpoint_fingerprint,
            "--expected-checkpoint-fingerprint",
        )?;
        let after_statement_id = self
            .after_statement_id
            .as_deref()
            .map(|value| required_nonzero_lower_hex32(value, "--after-statement-id"))
            .transpose()?;
        let limit = required_hedging_billing_page_limit(self.limit)?;
        let filter = SorafsBillingStatementListFilter {
            expected_checkpoint_fingerprint_hex: &checkpoint,
            after_statement_id_hex: after_statement_id.as_deref(),
            limit,
        };
        let client = context.client_from_config();
        hedging_billing_response::render(context, list(&client, filter)?, &checkpoint)
    }
}
/// Fetch one published billing statement.
#[derive(clap::Args, Debug)]
pub struct BillingStatementArgs {
    /// Exact non-zero lowercase statement identifier.
    #[arg(long = "statement-id", value_name = "HEX")]
    statement_id: String,
    /// Exact non-zero lowercase checkpoint fingerprint from billing status.
    #[arg(long = "expected-checkpoint-fingerprint", value_name = "HEX")]
    expected_checkpoint_fingerprint: String,
    /// Destination for the canonical Norito statement bytes.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}
impl_run_with_client_methods!(BillingStatementArgs, Client::get_sorafs_billing_statement);
impl BillingStatementArgs {
    fn run_with<C, F>(&self, context: &mut C, get: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &str, &str) -> Result<Response<Vec<u8>>>,
    {
        let statement_id = required_nonzero_lower_hex32(&self.statement_id, "--statement-id")?;
        let checkpoint = required_nonzero_lower_hex32(
            &self.expected_checkpoint_fingerprint,
            "--expected-checkpoint-fingerprint",
        )?;
        let client = context.client_from_config();
        let response = get(&client, &statement_id, &checkpoint)?;
        write_billing_statement_response(
            context,
            response,
            &self.output,
            &statement_id,
            &checkpoint,
        )
    }
}
fn write_billing_statement_response<C: RunContext>(
    context: &mut C,
    response: Response<Vec<u8>>,
    output: &Path,
    statement_id: &str,
    checkpoint: &str,
) -> Result<()> {
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .map(str::to_owned);
    let body = response.into_body();
    if status != StatusCode::OK {
        return Err(make_http_error(status, &body));
    }
    if content_type.as_deref() != Some("application/x-norito") {
        return Err(eyre!(
            "billing statement response for `{statement_id}` must use application/x-norito"
        ));
    }
    if body.is_empty() {
        return Err(eyre!(
            "billing statement response for `{statement_id}` was empty"
        ));
    }
    let mut output_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .wrap_err_with(|| {
            format!(
                "failed to create canonical billing statement `{}` without replacing an existing path",
                output.display()
            )
        })?;
    let output_metadata = output_file.metadata().wrap_err_with(|| {
        format!(
            "failed to inspect newly created canonical billing statement `{}`",
            output.display()
        )
    })?;
    if !output_metadata.is_file() {
        return Err(eyre!(
            "canonical billing statement output `{}` must be a regular file",
            output.display()
        ));
    }
    output_file.write_all(&body).wrap_err_with(|| {
        format!(
            "failed to write canonical billing statement `{}`",
            output.display()
        )
    })?;
    output_file.flush().wrap_err_with(|| {
        format!(
            "failed to flush canonical billing statement `{}`",
            output.display()
        )
    })?;
    context.print_data(&norito::json!({
        "statement_id": statement_id,
        "expected_checkpoint_fingerprint": checkpoint,
        "output": (output.display().to_string()),
        "bytes_written": (u64::try_from(body.len()).unwrap_or(u64::MAX)),
        "content_type": "application/x-norito"
    }))
}
/// Submit one owner acknowledgement for a published billing statement.
#[derive(clap::Args, Debug)]
pub struct BillingAcknowledgeArgs {
    /// Exact non-zero lowercase statement identifier.
    #[arg(long = "statement-id", value_name = "HEX")]
    statement_id: String,
    /// Exact non-zero lowercase checkpoint fingerprint from billing status.
    #[arg(long = "expected-checkpoint-fingerprint", value_name = "HEX")]
    expected_checkpoint_fingerprint: String,
    /// Non-zero lowercase 32-byte idempotency nonce authenticated by the external proof.
    #[arg(long = "request-nonce", value_name = "HEX")]
    request_nonce: String,
    /// Binary external-authority authentication proof, bounded to 64 KiB.
    #[arg(long = "authentication-proof", value_name = "PATH")]
    authentication_proof: PathBuf,
}
impl_run_with_client_methods!(
    BillingAcknowledgeArgs,
    Client::post_sorafs_billing_statement_acknowledgement,
);
impl BillingAcknowledgeArgs {
    fn run_with<C, F>(&self, context: &mut C, acknowledge: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(
            &Client,
            &str,
            &str,
            &SorafsBillingAcknowledgementProof,
        ) -> Result<Response<Vec<u8>>>,
    {
        let statement_id = required_nonzero_lower_hex32(&self.statement_id, "--statement-id")?;
        let checkpoint = required_nonzero_lower_hex32(
            &self.expected_checkpoint_fingerprint,
            "--expected-checkpoint-fingerprint",
        )?;
        let authentication_proof = read_billing_acknowledgement_proof(&self.authentication_proof)?;
        let proof = SorafsBillingAcknowledgementProof::try_from_hex(
            &self.request_nonce,
            authentication_proof,
        )?;
        let client = context.client_from_config();
        render_json_response(
            context,
            acknowledge(&client, &statement_id, &checkpoint, &proof)?,
        )
    }
}
#[cfg(unix)]
type BillingProofFileIdentity = (u64, u64);
#[cfg(windows)]
type BillingProofFileIdentity = (Option<u32>, Option<u64>);
#[cfg(not(any(unix, windows)))]
type BillingProofFileIdentity = ();
#[cfg(unix)]
fn billing_proof_file_identity(metadata: &fs::Metadata) -> BillingProofFileIdentity {
    use std::os::unix::fs::MetadataExt as _;
    (metadata.dev(), metadata.ino())
}
#[cfg(windows)]
fn billing_proof_file_identity(metadata: &fs::Metadata) -> BillingProofFileIdentity {
    use std::os::windows::fs::MetadataExt as _;
    (metadata.volume_serial_number(), metadata.file_index())
}
#[cfg(not(any(unix, windows)))]
fn billing_proof_file_identity(_metadata: &fs::Metadata) -> BillingProofFileIdentity {}
#[cfg(unix)]
const fn billing_proof_file_identity_available(_identity: BillingProofFileIdentity) -> bool {
    true
}
#[cfg(windows)]
const fn billing_proof_file_identity_available(identity: BillingProofFileIdentity) -> bool {
    identity.0.is_some() && identity.1.is_some()
}
#[cfg(not(any(unix, windows)))]
const fn billing_proof_file_identity_available(_identity: BillingProofFileIdentity) -> bool {
    false
}
fn billing_proof_file_is_single_link(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        metadata.nlink() == 1
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        metadata.number_of_links() == Some(1)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        false
    }
}
#[cfg(windows)]
fn billing_proof_file_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}
#[cfg(not(windows))]
fn billing_proof_file_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}
fn billing_proof_file_is_indirect(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || billing_proof_file_is_reparse_point(metadata)
}
#[cfg(unix)]
fn billing_proof_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    billing_proof_file_identity(left) == billing_proof_file_identity(right)
        && left.nlink() == 1
        && right.nlink() == 1
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
#[cfg(windows)]
fn billing_proof_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    billing_proof_file_identity_available(billing_proof_file_identity(left))
        && billing_proof_file_identity(left) == billing_proof_file_identity(right)
        && left.number_of_links() == Some(1)
        && right.number_of_links() == Some(1)
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
}
#[cfg(not(any(unix, windows)))]
fn billing_proof_metadata_unchanged(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}
#[cfg(unix)]
fn open_direct_billing_acknowledgement_proof(path: &Path) -> Result<fs::File> {
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .wrap_err_with(|| {
        format!(
            "failed to securely open billing acknowledgement proof `{}`",
            path.display()
        )
    })?;
    Ok(fs::File::from(descriptor))
}
#[cfg(windows)]
fn open_direct_billing_acknowledgement_proof(path: &Path) -> Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path).wrap_err_with(|| {
        format!(
            "failed to securely open billing acknowledgement proof `{}`",
            path.display()
        )
    })
}
#[cfg(not(any(unix, windows)))]
fn open_direct_billing_acknowledgement_proof(path: &Path) -> Result<fs::File> {
    Err(eyre!(
        "billing acknowledgement proof `{}` cannot be opened because this platform does not expose a stable direct-file identity",
        path.display()
    ))
}
fn read_billing_acknowledgement_proof(path: &Path) -> Result<Vec<u8>> {
    let path_metadata = fs::symlink_metadata(path).wrap_err_with(|| {
        format!(
            "failed to inspect billing acknowledgement proof `{}`",
            path.display()
        )
    })?;
    if billing_proof_file_is_indirect(&path_metadata)
        || !path_metadata.file_type().is_file()
        || !billing_proof_file_identity_available(billing_proof_file_identity(&path_metadata))
        || !billing_proof_file_is_single_link(&path_metadata)
    {
        return Err(eyre!(
            "billing acknowledgement proof `{}` must be a regular non-symlink file with a stable single-link identity",
            path.display()
        ));
    }
    if path_metadata.len() == 0
        || path_metadata.len() > SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1 as u64
    {
        return Err(eyre!(
            "billing acknowledgement proof `{}` must contain between 1 and {SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1} bytes",
            path.display()
        ));
    }
    let expected_len = usize::try_from(path_metadata.len()).map_err(|_| {
        eyre!(
            "billing acknowledgement proof `{}` length is not representable on this host",
            path.display()
        )
    })?;
    let mut file = open_direct_billing_acknowledgement_proof(path)?;
    let opened_metadata = file.metadata().wrap_err_with(|| {
        format!(
            "failed to inspect opened billing acknowledgement proof `{}`",
            path.display()
        )
    })?;
    if billing_proof_file_is_indirect(&opened_metadata)
        || !opened_metadata.is_file()
        || !billing_proof_metadata_unchanged(&path_metadata, &opened_metadata)
    {
        return Err(eyre!(
            "billing acknowledgement proof `{}` changed between inspection and open",
            path.display()
        ));
    }
    let bytes = read_billing_acknowledgement_proof_exact(path, &mut file, expected_len)?;
    let after_file_metadata = file.metadata().wrap_err_with(|| {
        format!(
            "failed to re-inspect opened billing acknowledgement proof `{}`",
            path.display()
        )
    })?;
    let after_path_metadata = fs::symlink_metadata(path).wrap_err_with(|| {
        format!(
            "failed to re-inspect billing acknowledgement proof `{}`",
            path.display()
        )
    })?;
    if billing_proof_file_is_indirect(&after_file_metadata)
        || !after_file_metadata.is_file()
        || billing_proof_file_is_indirect(&after_path_metadata)
        || !after_path_metadata.file_type().is_file()
        || !billing_proof_metadata_unchanged(&opened_metadata, &after_file_metadata)
        || !billing_proof_metadata_unchanged(&opened_metadata, &after_path_metadata)
        || after_file_metadata.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(eyre!(
            "billing acknowledgement proof `{}` changed while it was read",
            path.display()
        ));
    }
    Ok(bytes)
}
fn read_billing_acknowledgement_proof_exact(
    path: &Path,
    reader: &mut impl Read,
    expected_len: usize,
) -> Result<Vec<u8>> {
    let mut bytes = vec![0_u8; expected_len];
    if let Err(error) = reader.read_exact(&mut bytes) {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            return Err(eyre!(
                "billing acknowledgement proof `{}` changed length while it was read",
                path.display()
            ));
        }
        return Err(error).wrap_err_with(|| {
            format!(
                "failed to read billing acknowledgement proof `{}`",
                path.display()
            )
        });
    }
    let mut trailing = [0_u8; 1];
    if reader.read(&mut trailing).wrap_err_with(|| {
        format!(
            "failed to finish reading billing acknowledgement proof `{}`",
            path.display()
        )
    })? != 0
    {
        return Err(eyre!(
            "billing acknowledgement proof `{}` changed length while it was read",
            path.display()
        ));
    }
    Ok(bytes)
}
/// Fetch payload-free billing reconciliation status.
#[derive(clap::Args, Debug, Default)]
pub struct BillingReconciliationArgs {}
impl_run_with_client_methods!(
    BillingReconciliationArgs,
    Client::get_sorafs_billing_reconciliation,
);
impl BillingReconciliationArgs {
    fn run_with<C, F>(&self, context: &mut C, get: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client) -> Result<Response<Vec<u8>>>,
    {
        let client = context.client_from_config();
        render_json_response(context, get(&client)?)
    }
}
/// Read-only finalized SoraFS hedging projections.
#[derive(clap::Subcommand, Debug)]
pub enum HedgingCommand {
    /// List finalized XOR exposure, including below-threshold periods.
    Exposure(HedgingProjectionArgs),
    /// List deterministic governed hedge intents without executing them.
    Intents(HedgingProjectionArgs),
}
impl Run for HedgingCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Exposure(args) => args.run_with(context, Client::get_sorafs_hedging_exposure),
            Self::Intents(args) => args.run_with(context, Client::get_sorafs_hedging_intents),
        }
    }
}
/// Exact-checkpoint pagination arguments shared by hedging projections.
#[derive(clap::Args, Debug)]
pub struct HedgingProjectionArgs {
    /// Exact non-zero lowercase checkpoint fingerprint from billing status.
    #[arg(long = "expected-checkpoint-fingerprint", value_name = "HEX")]
    expected_checkpoint_fingerprint: String,
    /// Optional exclusive non-zero lowercase opaque cursor.
    #[arg(long, value_name = "HEX")]
    after: Option<String>,
    /// Required page size in the inclusive range 1 through 100.
    #[arg(long, value_name = "COUNT")]
    limit: u16,
}
impl HedgingProjectionArgs {
    fn run_with<C, F>(&self, context: &mut C, get: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, SorafsHedgingProjectionFilter<'_>) -> Result<Response<Vec<u8>>>,
    {
        let checkpoint = required_nonzero_lower_hex32(
            &self.expected_checkpoint_fingerprint,
            "--expected-checkpoint-fingerprint",
        )?;
        let after = self
            .after
            .as_deref()
            .map(|value| required_nonzero_lower_hex32(value, "--after"))
            .transpose()?;
        let limit = required_hedging_billing_page_limit(self.limit)?;
        let filter = SorafsHedgingProjectionFilter {
            expected_checkpoint_fingerprint_hex: &checkpoint,
            after_hex: after.as_deref(),
            limit,
        };
        let client = context.client_from_config();
        hedging_billing_response::render(context, get(&client, filter)?, &checkpoint)
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum AppealsCommand {
    /// Appeal pricing helpers.
    #[command(subcommand)]
    Pricing(AppealsPricingCommand),
    /// Appeal finance helpers.
    #[command(subcommand)]
    Finance(AppealsFinanceCommand),
}
impl_run_for_subcommand!(AppealsCommand => Pricing, Finance);
#[derive(clap::Subcommand, Debug)]
pub enum AppealsPricingCommand {
    /// Print the active local appeal pricing config.
    Config(AppealsPricingConfigArgs),
    /// Print appeal pricing status and supported classes.
    Status(AppealsPricingStatusArgs),
    /// Quote a deposit from a Torii pricing quote JSON payload.
    Quote(AppealsPricingQuoteArgs),
}
impl_run_for_subcommand!(AppealsPricingCommand => Config, Status, Quote);
#[derive(clap::Args, Debug)]
pub struct AppealsPricingConfigArgs;
impl Run for AppealsPricingConfigArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        render_json_response(context, client.get_sorafs_appeal_pricing_config()?)
    }
}
#[derive(clap::Args, Debug)]
pub struct AppealsPricingStatusArgs;
impl Run for AppealsPricingStatusArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        render_json_response(context, client.get_sorafs_appeal_pricing_status()?)
    }
}
#[derive(clap::Args, Debug)]
pub struct AppealsPricingQuoteArgs {
    /// JSON quote request payload path.
    #[arg(long = "input", value_name = "PATH")]
    input: PathBuf,
}
impl_run_with_client_methods!(
    AppealsPricingQuoteArgs,
    Client::post_sorafs_appeal_pricing_quote_json,
);
impl_json_payload_run_with!(AppealsPricingQuoteArgs.input => "appeal pricing quote", render_json_response);
#[derive(clap::Subcommand, Debug)]
pub enum AppealsFinanceCommand {
    /// Runtime asset-lock deposit helpers.
    #[command(subcommand)]
    Deposits(AppealsFinanceDepositsCommand),
    /// List published appeal finance reports.
    Reports(AppealsFinanceReportsArgs),
    /// List published weekly appeal finance rollups.
    WeeklyRollups(AppealsFinanceWeeklyRollupsArgs),
    /// List published appeal finance settlement receipts.
    SettlementReceipts(AppealsFinanceSettlementReceiptsArgs),
}
impl_run_for_subcommand!(AppealsFinanceCommand => Deposits, Reports, WeeklyRollups, SettlementReceipts);
#[derive(clap::Subcommand, Debug)]
pub enum AppealsFinanceDepositsCommand {
    /// Build a runtime asset-lock deposit transaction request.
    Create(AppealsFinanceDepositCreateArgs),
    /// Confirm a runtime asset-lock deposit after ledger submission.
    Confirm(AppealsFinanceDepositConfirmArgs),
    /// Fetch one visible appeal deposit status.
    Get(AppealsFinanceDepositGetArgs),
    /// Settle a confirmed deposit locally.
    Settle(AppealsFinanceDepositSettleArgs),
    /// Reconcile a confirmed deposit against runtime ledger state.
    Reconcile(AppealsFinanceDepositReconcileArgs),
    /// Submit the next settlement transaction step.
    SubmitSettlement(AppealsFinanceDepositSubmitSettlementArgs),
}
impl_run_for_subcommand!(AppealsFinanceDepositsCommand => Create, Confirm, Get, Settle, Reconcile, SubmitSettlement);
#[derive(clap::Args, Debug)]
pub struct AppealsFinanceDepositCreateArgs {
    /// JSON deposit request payload path.
    #[arg(long = "input", value_name = "PATH")]
    input: PathBuf,
}
impl_run_with_client_methods!(
    AppealsFinanceDepositCreateArgs,
    Client::post_sorafs_appeal_finance_deposit_json,
);
impl_appeal_finance_submit_run_with!(AppealsFinanceDepositCreateArgs => "appeal finance deposit", StatusCode::OK);
#[derive(clap::Args, Debug)]
pub struct AppealsFinanceDepositConfirmArgs {
    /// JSON deposit confirmation payload path.
    #[arg(long = "input", value_name = "PATH")]
    input: PathBuf,
}
impl_run_with_client_methods!(
    AppealsFinanceDepositConfirmArgs,
    Client::post_sorafs_appeal_finance_deposit_confirm_json,
);
impl_appeal_finance_submit_run_with!(AppealsFinanceDepositConfirmArgs => "appeal finance deposit confirmation", StatusCode::OK);
#[derive(clap::Args, Debug)]
pub struct AppealsFinanceDepositGetArgs {
    /// Hex-encoded asset-lock escrow id.
    #[arg(long = "escrow-id", value_name = "HEX")]
    escrow_id: String,
}
impl_run_with_client_methods!(
    AppealsFinanceDepositGetArgs,
    Client::get_sorafs_appeal_finance_deposit,
);
impl AppealsFinanceDepositGetArgs {
    fn run_with<C, F>(&self, context: &mut C, get: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &str) -> Result<Response<Vec<u8>>>,
    {
        let escrow_id = required_trimmed_text(&self.escrow_id, "--escrow-id")?;
        let client = context.client_from_config();
        let response = get(&client, &escrow_id)?;
        render_json_response(context, response)
    }
}
#[derive(clap::Args, Debug)]
pub struct AppealsFinanceDepositSettleArgs {
    /// JSON deposit settlement payload path.
    #[arg(long = "input", value_name = "PATH")]
    input: PathBuf,
}
impl_run_with_client_methods!(
    AppealsFinanceDepositSettleArgs,
    Client::post_sorafs_appeal_finance_deposit_settle_json,
);
impl_appeal_finance_submit_run_with!(AppealsFinanceDepositSettleArgs => "appeal finance deposit settlement", StatusCode::OK);
#[derive(clap::Args, Debug)]
pub struct AppealsFinanceDepositReconcileArgs {
    /// JSON deposit settlement reconciliation payload path.
    #[arg(long = "input", value_name = "PATH")]
    input: PathBuf,
}
impl_run_with_client_methods!(
    AppealsFinanceDepositReconcileArgs,
    Client::post_sorafs_appeal_finance_deposit_reconcile_json,
);
impl_appeal_finance_submit_run_with!(AppealsFinanceDepositReconcileArgs => "appeal finance deposit settlement reconciliation", StatusCode::OK);
#[derive(clap::Args, Debug)]
pub struct AppealsFinanceDepositSubmitSettlementArgs {
    /// JSON deposit settlement submission payload path.
    #[arg(long = "input", value_name = "PATH")]
    input: PathBuf,
}
impl_run_with_client_methods!(
    AppealsFinanceDepositSubmitSettlementArgs,
    Client::post_sorafs_appeal_finance_deposit_submit_settlement_json,
);
impl_appeal_finance_submit_run_with!(AppealsFinanceDepositSubmitSettlementArgs => "appeal finance deposit settlement submission", StatusCode::ACCEPTED);
#[derive(clap::Args, Debug)]
pub struct AppealsFinanceReportsArgs {
    /// Maximum number of report entries to return.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    AppealsFinanceReportsArgs,
    Client::get_sorafs_appeal_finance_reports
);
impl_json_limit_run_with!(AppealsFinanceReportsArgs => SorafsAppealFinanceReadbackFilter);
#[derive(clap::Args, Debug)]
pub struct AppealsFinanceWeeklyRollupsArgs {
    /// Maximum number of rollup entries to return.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    AppealsFinanceWeeklyRollupsArgs,
    Client::get_sorafs_appeal_finance_weekly_rollups,
);
impl_json_limit_run_with!(AppealsFinanceWeeklyRollupsArgs => SorafsAppealFinanceReadbackFilter);
#[derive(clap::Args, Debug)]
pub struct AppealsFinanceSettlementReceiptsArgs {
    /// Maximum number of settlement receipt entries to return.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    AppealsFinanceSettlementReceiptsArgs,
    Client::get_sorafs_appeal_finance_settlement_receipts,
);
impl_json_limit_run_with!(AppealsFinanceSettlementReceiptsArgs => SorafsAppealFinanceReadbackFilter);
fn run_appeal_finance_json_submit<C, F>(
    context: &mut C,
    input: &Path,
    payload_label: &str,
    submit: F,
    accepted_status: StatusCode,
) -> Result<()>
where
    C: RunContext,
    F: FnOnce(&Client, &[u8]) -> Result<Response<Vec<u8>>>,
{
    let payload = load_sorafs_json_payload(input, payload_label)?;
    let client = context.client_from_config();
    let response = submit(&client, &payload)?;
    match accepted_status {
        StatusCode::ACCEPTED => render_json_response_ok_or_accepted(context, response),
        StatusCode::OK => render_json_response(context, response),
        status => Err(eyre!(
            "unsupported SoraFS appeal finance success status {status}"
        )),
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum GarCommand {
    /// Render a GAR enforcement receipt artefact (JSON + optional Norito bytes).
    Receipt(GarReceiptArgs),
}
impl_run_for_subcommand!(GarCommand => Receipt);
#[derive(clap::Subcommand, Debug)]
pub enum TransparencyCommand {
    /// Inspect published transparency cycles and entry proofs.
    #[command(subcommand)]
    Cycles(TransparencyCyclesCommand),
    /// Fetch the explorer-ready transparency snapshot.
    Explorer(TransparencyExplorerArgs),
    /// Probe deployed transparency explorer routes and emit payload-free rollout evidence.
    ExplorerCanary(TransparencyExplorerCanaryArgs),
    /// Probe deployed transparency publication readback and emit payload-free evidence.
    PublicationCanary(TransparencyPublicationCanaryArgs),
    /// List published proof-token issuance summaries.
    Tokens(TransparencyTokensArgs),
    /// Submit proof-token issuance feed payloads and rollout canaries.
    #[command(subcommand)]
    TokenIssuance(TransparencyTokenIssuanceCommand),
    /// Submit privacy aggregate source events and trigger configured due publication.
    #[command(subcommand)]
    PrivacyAggregate(TransparencyPrivacyAggregateCommand),
}
impl_run_for_subcommand!(TransparencyCommand => Cycles, Explorer, ExplorerCanary, PublicationCanary, Tokens, TokenIssuance, PrivacyAggregate);
#[derive(clap::Subcommand, Debug)]
pub enum TransparencyCyclesCommand {
    /// List locally published transparency cycle summaries.
    List(TransparencyCyclesListArgs),
    /// Fetch and verify one published transparency cycle.
    Get(TransparencyCyclesGetArgs),
    /// Fetch and verify one published transparency entry proof.
    Entry(TransparencyCyclesEntryArgs),
}
impl_run_for_subcommand!(TransparencyCyclesCommand => List, Get, Entry);
#[derive(clap::Args, Debug)]
pub struct TransparencyCyclesListArgs {
    /// Maximum number of cycle summaries to return.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    TransparencyCyclesListArgs,
    Client::get_sorafs_transparency_cycles,
);
impl_json_limit_run_with!(TransparencyCyclesListArgs => SorafsTransparencyReadbackFilter);
#[derive(clap::Args, Debug)]
pub struct TransparencyCyclesGetArgs {
    /// 16-byte cycle id encoded as hexadecimal.
    #[arg(long = "cycle-id", value_name = "HEX")]
    cycle_id: String,
    /// Maximum number of publication proofs to return.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    TransparencyCyclesGetArgs,
    Client::get_sorafs_transparency_cycle,
);
impl TransparencyCyclesGetArgs {
    fn run_with<C, F>(&self, context: &mut C, get: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &str, SorafsTransparencyReadbackFilter) -> Result<Response<Vec<u8>>>,
    {
        let cycle_id = normalize_hex_16_lower(&self.cycle_id, "--cycle-id")?;
        let filter = SorafsTransparencyReadbackFilter { limit: self.limit };
        let client = context.client_from_config();
        let response = get(&client, &cycle_id, filter)?;
        render_json_response(context, response)
    }
}
#[derive(clap::Args, Debug)]
pub struct TransparencyCyclesEntryArgs {
    /// 16-byte cycle id encoded as hexadecimal.
    #[arg(long = "cycle-id", value_name = "HEX")]
    cycle_id: String,
    /// 16-byte entry id encoded as hexadecimal.
    #[arg(long = "entry-id", value_name = "HEX")]
    entry_id: String,
}
impl_run_with_client_methods!(
    TransparencyCyclesEntryArgs,
    Client::get_sorafs_transparency_cycle_entry,
);
impl TransparencyCyclesEntryArgs {
    fn run_with<C, F>(&self, context: &mut C, get: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &str, &str) -> Result<Response<Vec<u8>>>,
    {
        let cycle_id = normalize_hex_16_lower(&self.cycle_id, "--cycle-id")?;
        let entry_id = normalize_hex_16_lower(&self.entry_id, "--entry-id")?;
        let client = context.client_from_config();
        let response = get(&client, &cycle_id, &entry_id)?;
        render_json_response(context, response)
    }
}
#[derive(clap::Args, Debug)]
pub struct TransparencyExplorerArgs {
    /// Maximum number of cycle summaries and token issuance entries per array.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    TransparencyExplorerArgs,
    Client::get_sorafs_transparency_explorer,
);
impl_json_limit_run_with!(TransparencyExplorerArgs => SorafsTransparencyReadbackFilter);
#[derive(clap::Args, Debug)]
pub struct TransparencyExplorerCanaryArgs {
    /// Base URL of the deployed Torii or public explorer gateway.
    #[arg(long = "torii-url", value_name = "URL")]
    torii_url: Option<String>,
    /// Maximum number of cycle and proof-token summaries to request.
    #[arg(long)]
    limit: Option<u32>,
    /// HTTP timeout in seconds.
    #[arg(long = "timeout-secs", default_value_t = 30)]
    timeout_secs: u64,
    /// Optional path where the canary evidence JSON will be written.
    #[arg(long = "out", value_name = "PATH")]
    out: Option<PathBuf>,
}
impl Run for TransparencyExplorerCanaryArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let timeout = Duration::from_secs(self.timeout_secs.max(1));
        let client = BlockingHttpClient::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .user_agent("sorafs-cli transparency-explorer-canary")
            .build()
            .wrap_err("failed to construct SoraFS transparency explorer canary HTTP client")?;
        self.run_with_fetch(context, |url| {
            transparency_explorer_canary_http_get(&client, url)
        })
    }
}
impl TransparencyExplorerCanaryArgs {
    fn run_with_fetch<C, F>(&self, context: &mut C, mut fetch: F) -> Result<()>
    where
        C: RunContext,
        F: FnMut(&str) -> Result<TransparencyExplorerCanaryHttpResponse>,
    {
        let torii_url = match self.torii_url.as_deref() {
            Some(url) => required_trimmed_text(url, "--torii-url")?,
            None => context.config().torii_api_url.as_str().trim().to_owned(),
        };
        if torii_url.is_empty() {
            return Err(eyre!("configured Torii API URL must not be empty"));
        }
        let evidence =
            transparency_explorer_canary_evidence_json(&torii_url, self.limit, &mut fetch)?;
        if let Some(path) = &self.out {
            ensure_parent_dir(path)?;
            let bytes = norito::json::to_vec_pretty(&evidence)
                .wrap_err("failed to serialize SoraFS transparency explorer canary evidence")?;
            fs::write(path, bytes).wrap_err_with(|| {
                format!(
                    "failed to write SoraFS transparency explorer canary evidence to `{}`",
                    path.display()
                )
            })?;
        }
        context.print_data(&evidence)
    }
}
#[derive(clap::Args, Debug)]
pub struct TransparencyPublicationCanaryArgs {
    /// Base URL of the deployed Torii or public transparency gateway.
    #[arg(long = "torii-url", value_name = "URL")]
    torii_url: Option<String>,
    /// Published cycle id to verify through the cycle detail route.
    #[arg(long = "cycle-id", value_name = "HEX")]
    cycle_ids: Vec<String>,
    /// Maximum number of cycle summaries or publication proofs to request.
    #[arg(long)]
    limit: Option<u32>,
    /// HTTP timeout in seconds.
    #[arg(long = "timeout-secs", default_value_t = 30)]
    timeout_secs: u64,
    /// Optional path where the canary evidence JSON will be written.
    #[arg(long = "out", value_name = "PATH")]
    out: Option<PathBuf>,
}
impl Run for TransparencyPublicationCanaryArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let timeout = Duration::from_secs(self.timeout_secs.max(1));
        let client = BlockingHttpClient::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .user_agent("sorafs-cli transparency-publication-canary")
            .build()
            .wrap_err("failed to construct SoraFS transparency publication canary HTTP client")?;
        self.run_with_fetch(context, |url| {
            transparency_publication_canary_http_get(&client, url)
        })
    }
}
impl TransparencyPublicationCanaryArgs {
    fn run_with_fetch<C, F>(&self, context: &mut C, mut fetch: F) -> Result<()>
    where
        C: RunContext,
        F: FnMut(&str) -> Result<TransparencyExplorerCanaryHttpResponse>,
    {
        let torii_url = match self.torii_url.as_deref() {
            Some(url) => required_trimmed_text(url, "--torii-url")?,
            None => context.config().torii_api_url.as_str().trim().to_owned(),
        };
        if torii_url.is_empty() {
            return Err(eyre!("configured Torii API URL must not be empty"));
        }
        let cycle_ids = self
            .cycle_ids
            .iter()
            .map(|cycle_id| normalize_hex_16_lower(cycle_id, "--cycle-id"))
            .collect::<Result<Vec<_>>>()?;
        let evidence = transparency_publication_canary_evidence_json(
            &torii_url, &cycle_ids, self.limit, &mut fetch,
        )?;
        if let Some(path) = &self.out {
            write_json_artifact(path, &evidence, "transparency publication canary evidence")?;
        }
        context.print_data(&evidence)
    }
}
#[derive(clap::Args, Debug)]
pub struct TransparencyTokensArgs {
    /// Maximum number of proof-token issuance entries to return.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    TransparencyTokensArgs,
    Client::get_sorafs_transparency_token_issuances,
);
impl_json_limit_run_with!(TransparencyTokensArgs => SorafsTransparencyReadbackFilter);
#[derive(clap::Subcommand, Debug)]
pub enum TransparencyTokenIssuanceCommand {
    /// Submit one proof-token issuance JSON payload.
    Submit(TransparencyTokenIssuanceSubmitArgs),
    /// Probe deployed proof-token issuance producer feed routes.
    Canary(TransparencyTokenIssuanceCanaryArgs),
}
impl_run_for_subcommand!(TransparencyTokenIssuanceCommand => Submit, Canary);
#[derive(clap::Args, Debug)]
pub struct TransparencyTokenIssuanceSubmitArgs {
    /// JSON proof-token issuance payload path.
    #[arg(long = "payload", value_name = "PATH")]
    payload: PathBuf,
}
impl_run_with_client_methods!(
    TransparencyTokenIssuanceSubmitArgs,
    Client::post_sorafs_transparency_token_issuance_json,
);
impl_json_payload_run_with!(TransparencyTokenIssuanceSubmitArgs.payload => "transparency proof-token issuance", render_json_response_ok_or_accepted);
#[derive(clap::Args, Debug)]
pub struct TransparencyTokenIssuanceCanaryArgs {
    /// Proof-token issuance JSON payload path to submit.
    #[arg(long = "issuance", value_name = "PATH")]
    issuances: Vec<PathBuf>,
    /// Optional path where payload-free canary evidence JSON is written.
    #[arg(long = "out", value_name = "PATH")]
    out: Option<PathBuf>,
}
impl_run_with_client_methods!(
    TransparencyTokenIssuanceCanaryArgs,
    Client::post_sorafs_transparency_token_issuance_json,
);
impl TransparencyTokenIssuanceCanaryArgs {
    fn run_with<C, F>(&self, context: &mut C, mut submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnMut(&Client, &[u8]) -> Result<Response<Vec<u8>>>,
    {
        if self.issuances.is_empty() {
            return Err(eyre!("at least one --issuance payload is required"));
        }
        let client = context.client_from_config();
        let mut probes = Vec::new();
        for path in &self.issuances {
            let payload =
                load_sorafs_json_payload(path, "transparency proof-token issuance canary")?;
            let response = submit(&client, &payload)?;
            probes.push(transparency_token_issuance_canary_probe_json(
                path, &payload, response,
            ));
        }
        let passed_count = probes
            .iter()
            .filter(|probe| {
                probe
                    .get("response_success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            .count();
        let mut evidence = Map::new();
        evidence.insert(
            "schema".into(),
            Value::from("sorafs.transparency.proof_token_issuance.canary.v1"),
        );
        evidence.insert("source".into(), Value::from("iroha_cli"));
        evidence.insert(
            "status".into(),
            Value::from(if passed_count == probes.len() {
                "passed"
            } else {
                "failed"
            }),
        );
        evidence.insert(
            "probe_count".into(),
            Value::from(u64::try_from(probes.len()).unwrap_or(u64::MAX)),
        );
        evidence.insert(
            "passed_probe_count".into(),
            Value::from(u64::try_from(passed_count).unwrap_or(u64::MAX)),
        );
        evidence.insert(
            "issuance_probe_count".into(),
            Value::from(u64::try_from(self.issuances.len()).unwrap_or(u64::MAX)),
        );
        evidence.insert("payload_bytes_included".into(), Value::Bool(false));
        evidence.insert("proof_token_frames_included".into(), Value::Bool(false));
        evidence.insert("private_digest_keys_included".into(), Value::Bool(false));
        evidence.insert("response_bodies_included".into(), Value::Bool(false));
        evidence.insert("probes".into(), Value::Array(probes));
        let evidence = Value::Object(evidence);
        if let Some(path) = &self.out {
            write_json_artifact(
                path,
                &evidence,
                "transparency proof-token issuance canary evidence",
            )?;
        }
        context.print_data(&evidence)
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum TransparencyPrivacyAggregateCommand {
    /// Submit one privacy aggregate source-event JSON payload.
    SourceEvent(TransparencyPrivacyAggregateSourceEventArgs),
    /// Trigger configured due privacy aggregate publication.
    PublishDue(TransparencyPrivacyAggregatePublishDueArgs),
    /// Probe deployed privacy aggregate producer/scheduler routes.
    Canary(TransparencyPrivacyAggregateCanaryArgs),
}
impl_run_for_subcommand!(TransparencyPrivacyAggregateCommand => SourceEvent, PublishDue, Canary);
#[derive(clap::Args, Debug)]
pub struct TransparencyPrivacyAggregateSourceEventArgs {
    /// JSON payload path.
    #[arg(long = "payload", value_name = "PATH")]
    payload: PathBuf,
}
impl_run_with_client_methods!(
    TransparencyPrivacyAggregateSourceEventArgs,
    Client::post_sorafs_transparency_privacy_aggregate_source_event_json,
);
impl_json_payload_run_with!(TransparencyPrivacyAggregateSourceEventArgs.payload => "transparency privacy aggregate source-event", render_json_response_ok_or_accepted);
#[derive(clap::Args, Debug)]
pub struct TransparencyPrivacyAggregatePublishDueArgs {
    /// JSON payload path.
    #[arg(long = "payload", value_name = "PATH")]
    payload: PathBuf,
}
impl_run_with_client_methods!(
    TransparencyPrivacyAggregatePublishDueArgs,
    Client::post_sorafs_transparency_privacy_aggregate_publish_due_json,
);
impl_json_payload_run_with!(TransparencyPrivacyAggregatePublishDueArgs.payload => "transparency privacy aggregate publish-due", render_json_response);
#[derive(clap::Args, Debug)]
pub struct TransparencyPrivacyAggregateCanaryArgs {
    /// Privacy aggregate source-event JSON payload path to submit.
    #[arg(long = "source-event", value_name = "PATH")]
    source_events: Vec<PathBuf>,
    /// Privacy aggregate publish-due JSON payload path to submit.
    #[arg(long = "publish-due", value_name = "PATH")]
    publish_due: Vec<PathBuf>,
    /// Optional path where payload-free canary evidence JSON is written.
    #[arg(long = "out", value_name = "PATH")]
    out: Option<PathBuf>,
}
impl_run_with_client_methods!(
    TransparencyPrivacyAggregateCanaryArgs,
    Client::post_sorafs_transparency_privacy_aggregate_source_event_json,
    Client::post_sorafs_transparency_privacy_aggregate_publish_due_json,
);
impl TransparencyPrivacyAggregateCanaryArgs {
    fn run_with<C, FSource, FPublish>(
        &self,
        context: &mut C,
        mut submit_source_event: FSource,
        mut submit_publish_due: FPublish,
    ) -> Result<()>
    where
        C: RunContext,
        FSource: FnMut(&Client, &[u8]) -> Result<Response<Vec<u8>>>,
        FPublish: FnMut(&Client, &[u8]) -> Result<Response<Vec<u8>>>,
    {
        if self.source_events.is_empty() && self.publish_due.is_empty() {
            return Err(eyre!(
                "at least one --source-event or --publish-due payload is required"
            ));
        }
        let client = context.client_from_config();
        let mut probes = Vec::new();
        for path in &self.source_events {
            let payload = load_sorafs_json_payload(
                path,
                "transparency privacy aggregate canary source-event",
            )?;
            let response = submit_source_event(&client, &payload)?;
            probes.push(transparency_privacy_aggregate_canary_probe_json(
                "source_event",
                path,
                &payload,
                response,
            ));
        }
        for path in &self.publish_due {
            let payload = load_sorafs_json_payload(
                path,
                "transparency privacy aggregate canary publish-due",
            )?;
            let response = submit_publish_due(&client, &payload)?;
            probes.push(transparency_privacy_aggregate_canary_probe_json(
                "publish_due",
                path,
                &payload,
                response,
            ));
        }
        let passed_count = probes
            .iter()
            .filter(|probe| {
                probe
                    .get("response_success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            .count();
        let mut evidence = Map::new();
        evidence.insert(
            "schema".into(),
            Value::from("sorafs.transparency.privacy_aggregate.canary.v1"),
        );
        evidence.insert("source".into(), Value::from("iroha_cli"));
        evidence.insert(
            "status".into(),
            Value::from(if passed_count == probes.len() {
                "passed"
            } else {
                "failed"
            }),
        );
        evidence.insert(
            "probe_count".into(),
            Value::from(u64::try_from(probes.len()).unwrap_or(u64::MAX)),
        );
        evidence.insert(
            "passed_probe_count".into(),
            Value::from(u64::try_from(passed_count).unwrap_or(u64::MAX)),
        );
        evidence.insert(
            "source_event_probe_count".into(),
            Value::from(u64::try_from(self.source_events.len()).unwrap_or(u64::MAX)),
        );
        evidence.insert(
            "publish_due_probe_count".into(),
            Value::from(u64::try_from(self.publish_due.len()).unwrap_or(u64::MAX)),
        );
        evidence.insert("payload_bytes_included".into(), Value::Bool(false));
        evidence.insert("raw_metric_values_included".into(), Value::Bool(false));
        evidence.insert("private_payloads_included".into(), Value::Bool(false));
        evidence.insert("probes".into(), Value::Array(probes));
        let evidence = Value::Object(evidence);
        if let Some(path) = &self.out {
            write_json_artifact(
                path,
                &evidence,
                "transparency privacy aggregate canary evidence",
            )?;
        }
        context.print_data(&evidence)
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum ModerationCommand {
    /// Inspect finalized moderation cases and submit native ledger actions.
    #[command(subcommand)]
    Ballots(ModerationBallotsCommand),
    /// Admit and inspect local moderation model registry records.
    #[command(subcommand)]
    Registry(ModerationRegistryCommand),
    /// Submit and inspect deterministic local moderation screening results.
    #[command(subcommand)]
    Screening(ModerationScreeningCommand),
    /// Inspect and advance local moderation quarantine records.
    #[command(subcommand)]
    Quarantine(ModerationQuarantineCommand),
}
impl_run_for_subcommand!(ModerationCommand => Ballots, Registry, Screening, Quarantine);
#[derive(clap::Subcommand, Debug)]
pub enum ModerationBallotsCommand {
    /// List finalized chain-authoritative moderation case projections.
    List(ModerationBallotsListArgs),
    /// Get one finalized chain-authoritative moderation case projection.
    Get(ModerationBallotsGetArgs),
    /// Get the payload-free no-show plan for one closed moderation ballot.
    #[command(name = "no-show-plan")]
    NoShowPlan(ModerationBallotsNoShowPlanArgs),
    /// List typed committed moderation events.
    Events(ModerationBallotsEventsArgs),
    /// Submit a juror commit as an exact caller-signed native transaction.
    Commit(ModerationBallotsCommitArgs),
    /// Submit a juror reveal as an exact caller-signed native transaction.
    Reveal(ModerationBallotsRevealArgs),
    /// Submit governed native moderation finalization.
    Tally(ModerationBallotsTallyArgs),
    /// Execute pending commit/reveal/tally actions from a coordination status.
    Execute(ModerationBallotsExecuteArgs),
    /// Generate supervised commit/reveal executor deployment artifacts.
    ExecutorBundle(ModerationBallotsExecutorBundleArgs),
    /// Verify a deployed commit/reveal executor bundle and captured run summary.
    ExecutorCanary(ModerationBallotsExecutorCanaryArgs),
}
impl_run_for_subcommand!(ModerationBallotsCommand => List, Get, NoShowPlan, Events, Commit, Reveal, Tally, Execute, ExecutorBundle, ExecutorCanary);
#[derive(clap::Args, Debug)]
pub struct ModerationBallotsListArgs {
    /// Maximum number of ballots, commits, and reveals to return.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    ModerationBallotsListArgs,
    Client::get_sorafs_moderation_ballots,
);
impl_json_limit_run_with!(ModerationBallotsListArgs => SorafsModerationBallotsFilter);
#[derive(clap::Args, Debug)]
pub struct ModerationBallotsGetArgs {
    /// Moderation or appeal case identifier.
    #[arg(long = "case-id", value_name = "TEXT")]
    case_id: String,
    /// Moderation ballot round identifier.
    #[arg(long = "round-id", value_name = "TEXT")]
    round_id: String,
    /// Maximum number of commits and reveals to return.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    ModerationBallotsGetArgs,
    Client::get_sorafs_moderation_ballot,
);
impl ModerationBallotsGetArgs {
    fn run_with<C, F>(&self, context: &mut C, get: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &str, &str, SorafsModerationBallotsFilter) -> Result<Response<Vec<u8>>>,
    {
        let case_id = required_trimmed_text(&self.case_id, "--case-id")?;
        let round_id = required_trimmed_text(&self.round_id, "--round-id")?;
        let filter = SorafsModerationBallotsFilter { limit: self.limit };
        let client = context.client_from_config();
        let response = get(&client, &case_id, &round_id, filter)?;
        render_json_response(context, response)
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationBallotsNoShowPlanArgs {
    /// Moderation or appeal case identifier.
    #[arg(long = "case-id", value_name = "TEXT")]
    case_id: String,
    /// Moderation ballot round identifier.
    #[arg(long = "round-id", value_name = "TEXT")]
    round_id: String,
}
impl_run_with_client_methods!(
    ModerationBallotsNoShowPlanArgs,
    Client::get_sorafs_moderation_ballot_no_show_plan,
);
impl ModerationBallotsNoShowPlanArgs {
    fn run_with<C, F>(&self, context: &mut C, get: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &str, &str) -> Result<Response<Vec<u8>>>,
    {
        let case_id = required_trimmed_text(&self.case_id, "--case-id")?;
        let round_id = required_trimmed_text(&self.round_id, "--round-id")?;
        let client = context.client_from_config();
        let response = get(&client, &case_id, &round_id)?;
        render_json_response(context, response)
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationBallotsEventsArgs {
    /// Optional event sequence to resume from.
    #[arg(long)]
    since: Option<u64>,
    /// Maximum number of events to return.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    ModerationBallotsEventsArgs,
    Client::get_sorafs_moderation_ballot_events,
);
impl ModerationBallotsEventsArgs {
    fn run_with<C, F>(&self, context: &mut C, list: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, SorafsModerationBallotEventsFilter) -> Result<Response<Vec<u8>>>,
    {
        let filter = SorafsModerationBallotEventsFilter {
            since: self.since,
            limit: self.limit,
        };
        let client = context.client_from_config();
        let response = list(&client, filter)?;
        render_json_response(context, response)
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationBallotsCommitArgs {
    /// Commit payload path.
    #[arg(long = "payload", value_name = "PATH")]
    payload: PathBuf,
    /// Input format: json or norito.
    #[arg(long = "format", default_value = "json")]
    format: String,
}
impl_run_with_client_methods!(
    ModerationBallotsCommitArgs,
    Client::post_sorafs_moderation_ballot_commit,
);
impl ModerationBallotsCommitArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SignedTransaction) -> Result<HashOf<SignedTransaction>>,
    {
        let commit = load_moderation_ballot_commit_payload(&self.payload, self.format.as_str())?;
        let client = context.client_from_config();
        let transaction = build_moderation_commit_transaction(&client, &commit)?;
        let hash = submit(&client, &transaction)?;
        render_moderation_transaction_hash(context, &hash)
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationBallotsRevealArgs {
    /// Reveal payload path.
    #[arg(long = "payload", value_name = "PATH")]
    payload: PathBuf,
    /// Input format: json or norito.
    #[arg(long = "format", default_value = "json")]
    format: String,
}
impl_run_with_client_methods!(
    ModerationBallotsRevealArgs,
    Client::post_sorafs_moderation_ballot_reveal,
);
impl ModerationBallotsRevealArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SignedTransaction) -> Result<HashOf<SignedTransaction>>,
    {
        let reveal = load_moderation_ballot_reveal_payload(&self.payload, self.format.as_str())?;
        let client = context.client_from_config();
        let transaction = build_moderation_reveal_transaction(&client, &reveal)?;
        let hash = submit(&client, &transaction)?;
        render_moderation_transaction_hash(context, &hash)
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationBallotsTallyArgs {
    /// Moderation or appeal case identifier.
    #[arg(long = "case-id", value_name = "TEXT")]
    case_id: String,
    /// Moderation ballot round identifier.
    #[arg(long = "round-id", value_name = "TEXT")]
    round_id: String,
}
impl_run_with_client_methods!(
    ModerationBallotsTallyArgs,
    Client::post_sorafs_moderation_ballot_tally,
);
impl ModerationBallotsTallyArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SignedTransaction) -> Result<HashOf<SignedTransaction>>,
    {
        let case_id = required_trimmed_text(&self.case_id, "--case-id")?;
        let round_id = required_trimmed_text(&self.round_id, "--round-id")?;
        let client = context.client_from_config();
        let transaction = build_moderation_finalization_transaction(&client, case_id, round_id)?;
        let hash = submit(&client, &transaction)?;
        render_moderation_transaction_hash(context, &hash)
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationBallotsExecuteArgs {
    /// Payload-free commit/reveal status JSON from the operator workflow service.
    #[arg(long = "status", value_name = "PATH")]
    status: PathBuf,
    /// Commit payload path to submit if the status says the juror is pending.
    #[arg(long = "commit-payload", value_name = "PATH")]
    commit_payloads: Vec<PathBuf>,
    /// Reveal payload path to submit if the status says the juror is pending.
    #[arg(long = "reveal-payload", value_name = "PATH")]
    reveal_payloads: Vec<PathBuf>,
    /// Commit input format: json or norito.
    #[arg(long = "commit-format", default_value = "json")]
    commit_format: String,
    /// Reveal input format: json or norito.
    #[arg(long = "reveal-format", default_value = "json")]
    reveal_format: String,
    /// Submit tally requests for ballots already marked ready in the status.
    #[arg(long = "submit-tally")]
    submit_tally: bool,
}
impl_run_with_client_methods!(
    ModerationBallotsExecuteArgs,
    Client::post_sorafs_moderation_ballot_commit,
    Client::post_sorafs_moderation_ballot_reveal,
    Client::post_sorafs_moderation_ballot_tally,
);
impl ModerationBallotsExecuteArgs {
    fn run_with<C, FCommit, FReveal, FTally>(
        &self,
        context: &mut C,
        mut submit_commit: FCommit,
        mut submit_reveal: FReveal,
        mut submit_tally: FTally,
    ) -> Result<()>
    where
        C: RunContext,
        FCommit: FnMut(&Client, &SignedTransaction) -> Result<HashOf<SignedTransaction>>,
        FReveal: FnMut(&Client, &SignedTransaction) -> Result<HashOf<SignedTransaction>>,
        FTally: FnMut(&Client, &SignedTransaction) -> Result<HashOf<SignedTransaction>>,
    {
        if self.commit_payloads.is_empty() && self.reveal_payloads.is_empty() && !self.submit_tally
        {
            return Err(eyre!(
                "at least one --commit-payload, --reveal-payload, or --submit-tally is required"
            ));
        }
        let status = load_moderation_commit_reveal_status_payload(&self.status)?;
        let coordination = moderation_commit_reveal_coordination_from_status(&status)?;
        let client = context.client_from_config();
        let mut actions = Vec::new();
        for path in &self.commit_payloads {
            let commit = load_moderation_ballot_commit_payload(path, self.commit_format.as_str())?;
            let key = ModerationBallotExecutionKey::from_commit(&commit);
            if !coordination.pending_commits.contains(&key) {
                return Err(eyre!(
                    "commit payload for juror `{}` ballot `{}/{}` is not pending in --status",
                    key.juror_id,
                    key.case_id,
                    key.round_id
                ));
            }
            let transaction = build_moderation_commit_transaction(&client, &commit)?;
            let hash = submit_commit(&client, &transaction)?;
            actions.push(moderation_ballot_execution_action_json(
                "commit",
                &key.case_id,
                &key.round_id,
                Some(&key.juror_id),
                &hash,
            )?);
        }
        for path in &self.reveal_payloads {
            let reveal = load_moderation_ballot_reveal_payload(path, self.reveal_format.as_str())?;
            let key = ModerationBallotExecutionKey::from_reveal(&reveal);
            if !coordination.pending_reveals.contains(&key) {
                return Err(eyre!(
                    "reveal payload for juror `{}` ballot `{}/{}` is not pending in --status",
                    key.juror_id,
                    key.case_id,
                    key.round_id
                ));
            }
            let transaction = build_moderation_reveal_transaction(&client, &reveal)?;
            let hash = submit_reveal(&client, &transaction)?;
            actions.push(moderation_ballot_execution_action_json(
                "reveal",
                &key.case_id,
                &key.round_id,
                Some(&key.juror_id),
                &hash,
            )?);
        }
        if self.submit_tally {
            for (case_id, round_id) in &coordination.tally_ready {
                let transaction =
                    build_moderation_finalization_transaction(&client, case_id, round_id)?;
                let hash = submit_tally(&client, &transaction)?;
                actions.push(moderation_ballot_execution_action_json(
                    "tally", case_id, round_id, None, &hash,
                )?);
            }
        }
        let mut output = Map::new();
        output.insert(
            "schema".into(),
            Value::from("sorafs.moderation.ballots.execution.v1"),
        );
        output.insert("source".into(), Value::from("commit-reveal-status"));
        output.insert("status".into(), Value::from("executed"));
        output.insert("action_count".into(), Value::from(actions.len() as u64));
        output.insert(
            "commit_action_count".into(),
            Value::from(self.commit_payloads.len() as u64),
        );
        output.insert(
            "reveal_action_count".into(),
            Value::from(self.reveal_payloads.len() as u64),
        );
        output.insert(
            "tally_action_count".into(),
            Value::from(if self.submit_tally {
                coordination.tally_ready.len() as u64
            } else {
                0
            }),
        );
        output.insert("payload_bytes_included".into(), Value::Bool(false));
        output.insert("private_payloads_included".into(), Value::Bool(false));
        output.insert("actions".into(), Value::Array(actions));
        context.print_data(&Value::Object(output))
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationBallotsExecutorBundleArgs {
    /// Runtime path to the payload-free commit/reveal status JSON.
    #[arg(long = "status", value_name = "PATH")]
    status: PathBuf,
    /// Directory to write deployment artifacts into.
    #[arg(long = "bundle-out", value_name = "DIR")]
    bundle_out: PathBuf,
    /// Runtime commit payload path to submit if the status says the juror is pending.
    #[arg(long = "commit-payload", value_name = "PATH")]
    commit_payloads: Vec<PathBuf>,
    /// Runtime reveal payload path to submit if the status says the juror is pending.
    #[arg(long = "reveal-payload", value_name = "PATH")]
    reveal_payloads: Vec<PathBuf>,
    /// Commit input format: json or norito.
    #[arg(long = "commit-format", default_value = "json")]
    commit_format: String,
    /// Reveal input format: json or norito.
    #[arg(long = "reveal-format", default_value = "json")]
    reveal_format: String,
    /// Submit tally requests for ballots already marked ready in the status.
    #[arg(long = "submit-tally")]
    submit_tally: bool,
    /// Iroha CLI binary path used by the generated runner.
    #[arg(long = "iroha-bin", default_value = "iroha", value_name = "PATH")]
    iroha_bin: String,
    /// Service label used for generated systemd and launchd artifacts.
    #[arg(
        long = "service-name",
        default_value = "org.sora.sorafs.ballots-executor"
    )]
    service_name: String,
    /// Service user for the generated systemd unit.
    #[arg(long = "service-user", default_value = "sorafs-moderation")]
    service_user: String,
    /// Service group for the generated systemd unit.
    #[arg(long = "service-group", default_value = "sorafs-moderation")]
    service_group: String,
    /// Scheduler interval for the generated systemd timer and launchd job.
    #[arg(long = "interval-secs", default_value_t = 60)]
    interval_secs: u64,
}
impl Run for ModerationBallotsExecutorBundleArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        self.run_with(context)
    }
}
impl ModerationBallotsExecutorBundleArgs {
    fn run_with<C: RunContext>(&self, context: &mut C) -> Result<()> {
        let summary = write_moderation_ballots_executor_bundle(self)?;
        context.print_data(&summary)
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationBallotsExecutorCanaryArgs {
    /// Executor bundle directory produced by `executor-bundle`.
    #[arg(long = "bundle", value_name = "DIR")]
    bundle: PathBuf,
    /// Optional payload-free `ballots execute` summary captured from a deployed job run.
    #[arg(long = "execution-summary", value_name = "PATH")]
    execution_summary: Option<PathBuf>,
    /// Optional path to write canary evidence JSON.
    #[arg(long = "out", value_name = "PATH")]
    out: Option<PathBuf>,
}
impl Run for ModerationBallotsExecutorCanaryArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        self.run_with(context)
    }
}
impl ModerationBallotsExecutorCanaryArgs {
    fn run_with<C: RunContext>(&self, context: &mut C) -> Result<()> {
        let evidence = moderation_ballots_executor_canary_evidence(self)?;
        if let Some(path) = &self.out {
            write_json_artifact(
                path,
                &evidence,
                "moderation ballots executor canary evidence",
            )?;
        }
        context.print_data(&evidence)
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum ModerationRegistryCommand {
    /// List local moderation model registry records.
    List(ModerationRegistryListArgs),
    /// Admit a governance-signed reproducibility manifest.
    SubmitRepro(ModerationRegistrySubmitReproArgs),
    /// Admit an adversarial corpus manifest.
    SubmitCorpus(ModerationRegistrySubmitCorpusArgs),
}
impl_run_for_subcommand!(ModerationRegistryCommand => List, SubmitRepro, SubmitCorpus);
#[derive(clap::Args, Debug)]
pub struct ModerationRegistryListArgs {
    /// Maximum number of records to return from each registry section.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    ModerationRegistryListArgs,
    Client::get_sorafs_moderation_model_registry,
);
impl_json_limit_run_with!(ModerationRegistryListArgs => SorafsModerationModelRegistryFilter);
#[derive(clap::Args, Debug)]
pub struct ModerationRegistrySubmitReproArgs {
    /// Reproducibility manifest path.
    #[arg(long = "manifest", value_name = "PATH")]
    manifest: PathBuf,
    /// Input format: json or norito.
    #[arg(long = "format", default_value = "json")]
    format: String,
}
impl_run_with_client_methods!(
    ModerationRegistrySubmitReproArgs,
    Client::post_sorafs_moderation_model_registry_repro_manifest,
);
impl ModerationRegistrySubmitReproArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &[u8]) -> Result<Response<Vec<u8>>>,
    {
        let manifest_bytes =
            load_moderation_registry_repro_manifest_bytes(&self.manifest, self.format.as_str())?;
        let client = context.client_from_config();
        let response = submit(&client, &manifest_bytes)?;
        render_json_response_ok_or_accepted(context, response)
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationRegistrySubmitCorpusArgs {
    /// Adversarial corpus manifest path.
    #[arg(long = "manifest", value_name = "PATH")]
    manifest: PathBuf,
    /// Input format: json or norito.
    #[arg(long = "format", default_value = "json")]
    format: String,
}
impl_run_with_client_methods!(
    ModerationRegistrySubmitCorpusArgs,
    Client::post_sorafs_moderation_model_registry_corpus,
);
impl ModerationRegistrySubmitCorpusArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &[u8]) -> Result<Response<Vec<u8>>>,
    {
        let manifest_bytes =
            load_moderation_registry_corpus_manifest_bytes(&self.manifest, self.format.as_str())?;
        let client = context.client_from_config();
        let response = submit(&client, &manifest_bytes)?;
        render_json_response_ok_or_accepted(context, response)
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum ModerationScreeningCommand {
    /// List local moderation screening records.
    List(ModerationScreeningListArgs),
    /// Submit one deterministic local screening result JSON file.
    Submit(ModerationScreeningSubmitArgs),
}
impl_run_for_subcommand!(ModerationScreeningCommand => List, Submit);
#[derive(clap::Args, Debug)]
pub struct ModerationScreeningListArgs {
    /// Maximum number of screening records to return.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    ModerationScreeningListArgs,
    Client::get_sorafs_moderation_screening_results,
);
impl_json_limit_run_with!(ModerationScreeningListArgs => SorafsModerationScreeningResultsFilter);
#[derive(clap::Args, Debug)]
pub struct ModerationScreeningSubmitArgs {
    /// JSON request containing canonical signed-result or committee authority.
    #[arg(long = "input", value_name = "PATH")]
    input: PathBuf,
}
impl_run_with_client_methods!(
    ModerationScreeningSubmitArgs,
    Client::post_sorafs_moderation_screening_result,
);
impl ModerationScreeningSubmitArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(
            &Client,
            &SorafsModerationScreeningResultRequest<'_>,
        ) -> Result<Response<Vec<u8>>>,
    {
        let payload = load_moderation_screening_submit_payload(&self.input)?;
        let request = payload.as_request();
        let client = context.client_from_config();
        let response = submit(&client, &request)?;
        render_json_response_ok_or_accepted(context, response)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct ModerationScreeningSubmitPayload {
    idempotency_key_hex: String,
    evidence_kind: String,
    authority_b64: String,
    committee_member_results_b64: Vec<String>,
}
impl ModerationScreeningSubmitPayload {
    fn as_request(&self) -> SorafsModerationScreeningResultRequest<'_> {
        SorafsModerationScreeningResultRequest {
            idempotency_key_hex: self.idempotency_key_hex.as_str(),
            evidence_kind: self.evidence_kind.as_str(),
            authority_b64: self.authority_b64.as_str(),
            committee_member_results_b64: self.committee_member_results_b64.as_slice(),
        }
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum ModerationQuarantineCommand {
    /// List local moderation quarantine records.
    List(ModerationQuarantineListArgs),
    /// Store or read local encrypted quarantine payload objects.
    #[command(subcommand)]
    Object(ModerationQuarantineObjectCommand),
    /// Deliver payload-free juror notification manifests.
    #[command(subcommand)]
    Notifications(ModerationQuarantineNotificationsCommand),
    /// Mark a local moderation quarantine record reviewed.
    Review(ModerationQuarantineReviewArgs),
    /// Release a reviewed local moderation quarantine record.
    Release(ModerationQuarantineReleaseArgs),
    /// Build a reviewed quarantine appeal finance handoff.
    AppealHandoff(ModerationQuarantineAppealHandoffArgs),
    /// Read one role-gated local quarantine operator-panel workflow view.
    OperatorPanel(ModerationQuarantineOperatorPanelArgs),
    /// Build a payload-free bridge automation plan from the operator-panel view.
    BridgePlan(ModerationQuarantineBridgePlanArgs),
    /// Run a local payload-free operator-panel workflow service.
    OperatorServe(ModerationQuarantineOperatorServeArgs),
    /// Probe a deployed operator workflow service and emit payload-free evidence.
    OperatorCanary(ModerationQuarantineOperatorCanaryArgs),
}
impl_run_for_subcommand!(ModerationQuarantineCommand => List, Object, Notifications, Review, Release, AppealHandoff, OperatorPanel, BridgePlan, OperatorServe, OperatorCanary);
#[derive(clap::Args, Debug)]
pub struct ModerationQuarantineListArgs {
    /// Maximum number of quarantine records to return.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    ModerationQuarantineListArgs,
    Client::get_sorafs_moderation_quarantine,
);
impl_json_limit_run_with!(ModerationQuarantineListArgs => SorafsModerationQuarantineFilter);
#[derive(clap::Subcommand, Debug)]
pub enum ModerationQuarantineObjectCommand {
    /// Seal payload bytes into the local encrypted quarantine object store.
    Store(ModerationQuarantineObjectStoreArgs),
    /// Read and verify one local encrypted quarantine object.
    Read(ModerationQuarantineObjectReadArgs),
}
impl_run_for_subcommand!(ModerationQuarantineObjectCommand => Store, Read);
#[derive(clap::Args, Debug)]
pub struct ModerationQuarantineObjectStoreArgs {
    /// 16-byte local quarantine id encoded as hexadecimal.
    #[arg(long = "quarantine-id", value_name = "HEX")]
    quarantine_id: String,
    /// Path to the quarantined payload bytes to seal.
    #[arg(long = "payload-file", value_name = "PATH")]
    payload_file: PathBuf,
    /// Capture timestamp (RFC3339 or `@unix_seconds`; defaults to local now).
    #[arg(long = "captured-at", value_name = "RFC3339|@UNIX")]
    captured_at: Option<String>,
    /// Optional content type label recorded with the object.
    #[arg(long = "content-type", value_name = "TEXT")]
    content_type: Option<String>,
    /// Optional object-store notes recorded with the object.
    #[arg(long = "notes", value_name = "TEXT")]
    notes: Option<String>,
}
impl_run_with_client_methods!(
    ModerationQuarantineObjectStoreArgs,
    Client::post_sorafs_moderation_quarantine_object,
);
impl ModerationQuarantineObjectStoreArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(
            &Client,
            &str,
            &SorafsModerationQuarantineObjectStoreRequest<'_>,
        ) -> Result<Response<Vec<u8>>>,
    {
        let quarantine_id = normalize_hex_digest::<16>(&self.quarantine_id, "--quarantine-id")?;
        let payload = fs::read(&self.payload_file).wrap_err_with(|| {
            format!(
                "failed to read quarantine payload file `{}`",
                self.payload_file.display()
            )
        })?;
        if payload.is_empty() {
            return Err(eyre!(
                "--payload-file `{}` must not be empty",
                self.payload_file.display()
            ));
        }
        let captured_at_unix = parse_timestamp_or_now(self.captured_at.as_deref(), "captured-at")?;
        let content_type = optional_trimmed_text(self.content_type.as_deref(), "--content-type")?;
        let notes = optional_trimmed_text(self.notes.as_deref(), "--notes")?;
        let request = SorafsModerationQuarantineObjectStoreRequest {
            payload: &payload,
            captured_at_unix: Some(captured_at_unix),
            content_type: content_type.as_deref(),
            notes: notes.as_deref(),
        };
        let client = context.client_from_config();
        let response = submit(&client, &quarantine_id, &request)?;
        render_json_response_ok_or_accepted(context, response)
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationQuarantineObjectReadArgs {
    /// 16-byte local quarantine id encoded as hexadecimal.
    #[arg(long = "quarantine-id", value_name = "HEX")]
    quarantine_id: String,
}
impl_run_with_client_methods!(
    ModerationQuarantineObjectReadArgs,
    Client::get_sorafs_moderation_quarantine_object,
);
impl ModerationQuarantineObjectReadArgs {
    fn run_with<C, F>(&self, context: &mut C, read: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &str) -> Result<Response<Vec<u8>>>,
    {
        let quarantine_id = normalize_hex_digest::<16>(&self.quarantine_id, "--quarantine-id")?;
        let client = context.client_from_config();
        let response = read(&client, &quarantine_id)?;
        render_json_response(context, response)
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum ModerationQuarantineNotificationsCommand {
    /// Deliver one payload-free juror notification manifest.
    Deliver(ModerationQuarantineNotificationsDeliverArgs),
    /// Probe a deployed juror notification transport and emit payload-free evidence.
    Canary(ModerationQuarantineNotificationsCanaryArgs),
}
impl_run_for_subcommand!(ModerationQuarantineNotificationsCommand => Deliver, Canary);
#[derive(clap::Args, Debug)]
pub struct ModerationQuarantineNotificationsDeliverArgs {
    /// Payload-free juror notification manifest JSON.
    #[arg(long = "manifest", value_name = "PATH")]
    manifest: PathBuf,
    /// Directory where canonical notification JSON files are written.
    #[arg(long = "out-dir", value_name = "DIR")]
    out_dir: Option<PathBuf>,
    /// Optional webhook endpoint that receives each notification JSON.
    #[arg(long = "webhook-url", value_name = "URL")]
    webhook_url: Option<String>,
    /// Webhook request timeout in seconds.
    #[arg(long = "timeout-secs", default_value_t = 10)]
    timeout_secs: u64,
}
impl Run for ModerationQuarantineNotificationsDeliverArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let timeout = Duration::from_secs(self.timeout_secs.max(1));
        let http_client = BlockingHttpClient::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .user_agent("iroha-cli sorafs-moderation-notifications")
            .build()
            .wrap_err("failed to build SoraFS moderation notification HTTP client")?;
        self.run_with(context, |url, body| {
            post_moderation_juror_notification_webhook(&http_client, url, body)
        })
    }
}
impl ModerationQuarantineNotificationsDeliverArgs {
    fn run_with<C, F>(&self, context: &mut C, mut post_webhook: F) -> Result<()>
    where
        C: RunContext,
        F: FnMut(&str, &[u8]) -> Result<Response<Vec<u8>>>,
    {
        if self.out_dir.is_none() && self.webhook_url.is_none() {
            return Err(eyre!(
                "at least one of --out-dir or --webhook-url is required"
            ));
        }
        let webhook_url = self
            .webhook_url
            .as_deref()
            .map(|url| required_trimmed_text(url, "--webhook-url"))
            .transpose()?;
        let manifest = load_moderation_juror_notifications_manifest(&self.manifest)?;
        let notifications = moderation_juror_notification_entries(&manifest)?;
        if notifications.is_empty() {
            return Err(eyre!(
                "juror notification manifest `{}` does not contain notifications to deliver",
                self.manifest.display()
            ));
        }
        if let Some(out_dir) = &self.out_dir {
            fs::create_dir_all(out_dir).wrap_err_with(|| {
                format!(
                    "failed to create juror notification outbox `{}`",
                    out_dir.display()
                )
            })?;
        }
        let mut deliveries = Vec::with_capacity(notifications.len());
        for notification in notifications {
            let canonical = norito::json::to_vec(notification.value)
                .wrap_err("failed to encode juror notification JSON")?;
            let mut outbox_path = Value::Null;
            if let Some(out_dir) = &self.out_dir {
                let path = out_dir.join(format!(
                    "{}.json",
                    safe_moderation_notification_filename(notification.delivery_id)
                ));
                fs::write(&path, &canonical).wrap_err_with(|| {
                    format!(
                        "failed to write juror notification outbox file `{}`",
                        path.display()
                    )
                })?;
                outbox_path = Value::from(path.to_string_lossy().into_owned());
            }
            let mut webhook_status = Value::Null;
            let mut webhook_response_bytes = Value::Null;
            let mut webhook_response_body_blake3 = Value::Null;
            if let Some(url) = webhook_url.as_deref() {
                let response = post_webhook(url, &canonical)?;
                let status = response.status();
                let body = response.into_body();
                if !status.is_success() {
                    return Err(make_http_error(status, &body));
                }
                webhook_status = Value::from(u64::from(status.as_u16()));
                webhook_response_bytes = Value::from(u64::try_from(body.len()).unwrap_or(u64::MAX));
                webhook_response_body_blake3 = Value::from(encode(blake3::hash(&body).as_bytes()));
            }
            deliveries.push(moderation_juror_notification_delivery_result_json(
                notification,
                canonical.len(),
                &canonical,
                outbox_path,
                webhook_status,
                webhook_response_bytes,
                webhook_response_body_blake3,
            ));
        }
        let mut output = Map::new();
        output.insert(
            "schema".into(),
            Value::from("sorafs.moderation.juror_notifications.delivery.v1"),
        );
        output.insert("source".into(), Value::from("juror-notifications"));
        output.insert("status".into(), Value::from("delivered"));
        output.insert(
            "manifest_path".into(),
            Value::from(self.manifest.to_string_lossy().into_owned()),
        );
        output.insert(
            "out_dir".into(),
            self.out_dir.as_ref().map_or(Value::Null, |path| {
                Value::from(path.to_string_lossy().into_owned())
            }),
        );
        output.insert(
            "webhook_url".into(),
            webhook_url.as_deref().map_or(Value::Null, Value::from),
        );
        output.insert(
            "delivery_count".into(),
            Value::from(deliveries.len() as u64),
        );
        output.insert("payload_bytes_included".into(), Value::Bool(false));
        output.insert("private_payloads_included".into(), Value::Bool(false));
        output.insert("deliveries".into(), Value::Array(deliveries));
        context.print_data(&Value::Object(output))
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationQuarantineNotificationsCanaryArgs {
    /// Payload-free juror notification manifest JSON used as the canary probe.
    #[arg(long = "manifest", value_name = "PATH")]
    manifest: PathBuf,
    /// Deployed webhook endpoint to probe.
    #[arg(long = "webhook-url", value_name = "URL")]
    webhook_url: String,
    /// Optional path where payload-free canary evidence JSON is written.
    #[arg(long = "out", value_name = "PATH")]
    out: Option<PathBuf>,
    /// Webhook request timeout in seconds.
    #[arg(long = "timeout-secs", default_value_t = 10)]
    timeout_secs: u64,
}
impl Run for ModerationQuarantineNotificationsCanaryArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let timeout = Duration::from_secs(self.timeout_secs.max(1));
        let http_client = BlockingHttpClient::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .user_agent("iroha-cli sorafs-moderation-notification-canary")
            .build()
            .wrap_err("failed to build SoraFS moderation notification canary HTTP client")?;
        self.run_with(context, |url, body| {
            post_moderation_juror_notification_webhook(&http_client, url, body)
        })
    }
}
impl ModerationQuarantineNotificationsCanaryArgs {
    fn run_with<C, F>(&self, context: &mut C, mut post_webhook: F) -> Result<()>
    where
        C: RunContext,
        F: FnMut(&str, &[u8]) -> Result<Response<Vec<u8>>>,
    {
        let webhook_url = required_trimmed_text(&self.webhook_url, "--webhook-url")?;
        let manifest = load_moderation_juror_notifications_manifest(&self.manifest)?;
        let notifications = moderation_juror_notification_entries(&manifest)?;
        if notifications.is_empty() {
            return Err(eyre!(
                "juror notification canary manifest `{}` does not contain notifications to probe",
                self.manifest.display()
            ));
        }
        let mut probes = Vec::with_capacity(notifications.len());
        for notification in notifications {
            let canonical = norito::json::to_vec(notification.value)
                .wrap_err("failed to encode juror notification canary JSON")?;
            let response = post_webhook(&webhook_url, &canonical)?;
            probes.push(moderation_juror_notification_canary_probe_json(
                notification,
                &canonical,
                response,
            )?);
        }
        let status = if probes.iter().all(moderation_canary_probe_ok) {
            "passed"
        } else {
            "failed"
        };
        let mut evidence = Map::new();
        evidence.insert(
            "schema".into(),
            Value::from("sorafs.moderation.juror_notifications.transport_canary.v1"),
        );
        evidence.insert("source".into(), Value::from("juror-notifications"));
        evidence.insert("status".into(), Value::from(status));
        evidence.insert(
            "manifest_path".into(),
            Value::from(self.manifest.to_string_lossy().into_owned()),
        );
        evidence.insert(
            "manifest_body_blake3_hex".into(),
            Value::from(encode(
                blake3::hash(
                    &norito::json::to_vec(&manifest)
                        .wrap_err("failed to encode juror notification canary manifest")?,
                )
                .as_bytes(),
            )),
        );
        evidence.insert("webhook_url".into(), Value::from(webhook_url));
        evidence.insert("probe_count".into(), Value::from(probes.len() as u64));
        evidence.insert(
            "accepted_count".into(),
            Value::from(
                probes
                    .iter()
                    .filter(|probe| moderation_canary_probe_ok(probe))
                    .count() as u64,
            ),
        );
        evidence.insert("payload_bytes_included".into(), Value::Bool(false));
        evidence.insert("private_payloads_included".into(), Value::Bool(false));
        evidence.insert("probes".into(), Value::Array(probes));
        let evidence = Value::Object(evidence);
        if let Some(path) = &self.out {
            write_json_artifact(path, &evidence, "juror notification canary evidence")?;
        }
        context.print_data(&evidence)
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationQuarantineReviewArgs {
    /// 16-byte local quarantine id encoded as hexadecimal.
    #[arg(long = "quarantine-id", value_name = "HEX")]
    quarantine_id: String,
    /// Operator identity recorded in the checkpoint (defaults to the CLI account).
    #[arg(long = "reviewed-by", value_name = "TEXT")]
    reviewed_by: Option<String>,
    /// Review timestamp (RFC3339 or `@unix_seconds`; defaults to local now).
    #[arg(long = "reviewed-at", value_name = "RFC3339|@UNIX")]
    reviewed_at: Option<String>,
    /// Optional review notes recorded with the transition.
    #[arg(long = "notes", value_name = "TEXT")]
    notes: Option<String>,
}
impl_run_with_client_methods!(
    ModerationQuarantineReviewArgs,
    Client::post_sorafs_moderation_quarantine_review,
);
impl ModerationQuarantineReviewArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(
            &Client,
            &str,
            &SorafsModerationQuarantineReviewRequest<'_>,
        ) -> Result<Response<Vec<u8>>>,
    {
        let quarantine_id = normalize_hex_digest::<16>(&self.quarantine_id, "--quarantine-id")?;
        let reviewed_by =
            moderation_actor_or_default(context, self.reviewed_by.as_deref(), "--reviewed-by")?;
        let notes = optional_trimmed_text(self.notes.as_deref(), "--notes")?;
        let reviewed_at_unix = parse_timestamp_or_now(self.reviewed_at.as_deref(), "reviewed-at")?;
        let request = SorafsModerationQuarantineReviewRequest {
            reviewed_by: reviewed_by.as_str(),
            reviewed_at_unix: Some(reviewed_at_unix),
            notes: notes.as_deref(),
        };
        let client = context.client_from_config();
        let response = submit(&client, &quarantine_id, &request)?;
        render_json_response_ok_or_accepted(context, response)
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationQuarantineReleaseArgs {
    /// 16-byte local quarantine id encoded as hexadecimal.
    #[arg(long = "quarantine-id", value_name = "HEX")]
    quarantine_id: String,
    /// Release authority recorded in the checkpoint (defaults to the CLI account).
    #[arg(long = "release-authority", value_name = "TEXT")]
    release_authority: Option<String>,
    /// Release timestamp (RFC3339 or `@unix_seconds`; defaults to local now).
    #[arg(long = "released-at", value_name = "RFC3339|@UNIX")]
    released_at: Option<String>,
    /// Optional release notes recorded with the transition.
    #[arg(long = "notes", value_name = "TEXT")]
    notes: Option<String>,
}
impl_run_with_client_methods!(
    ModerationQuarantineReleaseArgs,
    Client::post_sorafs_moderation_quarantine_release,
);
impl ModerationQuarantineReleaseArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(
            &Client,
            &str,
            &SorafsModerationQuarantineReleaseRequest<'_>,
        ) -> Result<Response<Vec<u8>>>,
    {
        let quarantine_id = normalize_hex_digest::<16>(&self.quarantine_id, "--quarantine-id")?;
        let release_authority = moderation_actor_or_default(
            context,
            self.release_authority.as_deref(),
            "--release-authority",
        )?;
        let notes = optional_trimmed_text(self.notes.as_deref(), "--notes")?;
        let released_at_unix = parse_timestamp_or_now(self.released_at.as_deref(), "released-at")?;
        let request = SorafsModerationQuarantineReleaseRequest {
            release_authority: release_authority.as_str(),
            released_at_unix: Some(released_at_unix),
            notes: notes.as_deref(),
        };
        let client = context.client_from_config();
        let response = submit(&client, &quarantine_id, &request)?;
        render_json_response_ok_or_accepted(context, response)
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationQuarantineAppealHandoffArgs {
    /// 16-byte local quarantine id encoded as hexadecimal.
    #[arg(long = "quarantine-id", value_name = "HEX")]
    quarantine_id: String,
    /// JSON appeal handoff request payload path.
    #[arg(long = "input", value_name = "PATH")]
    input: PathBuf,
}
impl_run_with_client_methods!(
    ModerationQuarantineAppealHandoffArgs,
    Client::post_sorafs_moderation_quarantine_appeal_handoff_json,
);
impl ModerationQuarantineAppealHandoffArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &str, &[u8]) -> Result<Response<Vec<u8>>>,
    {
        let quarantine_id = normalize_hex_digest::<16>(&self.quarantine_id, "--quarantine-id")?;
        let payload =
            load_sorafs_json_payload(&self.input, "moderation quarantine appeal handoff")?;
        let client = context.client_from_config();
        let response = submit(&client, &quarantine_id, &payload)?;
        render_json_response(context, response)
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationQuarantineOperatorPanelArgs {
    /// 16-byte local quarantine id encoded as hexadecimal.
    #[arg(long = "quarantine-id", value_name = "HEX")]
    quarantine_id: String,
    /// Maximum number of matching ballots to return.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    ModerationQuarantineOperatorPanelArgs,
    Client::get_sorafs_moderation_quarantine_operator_panel,
);
impl ModerationQuarantineOperatorPanelArgs {
    fn run_with<C, F>(&self, context: &mut C, get: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &str, SorafsModerationQuarantineFilter) -> Result<Response<Vec<u8>>>,
    {
        let quarantine_id = normalize_hex_digest::<16>(&self.quarantine_id, "--quarantine-id")?;
        let filter = SorafsModerationQuarantineFilter { limit: self.limit };
        let client = context.client_from_config();
        let response = get(&client, &quarantine_id, filter)?;
        render_json_response(context, response)
    }
}
#[derive(clap::Args, Debug)]
pub struct ModerationQuarantineBridgePlanArgs {
    /// 16-byte local quarantine id encoded as hexadecimal.
    #[arg(long = "quarantine-id", value_name = "HEX")]
    quarantine_id: String,
    /// Maximum number of matching ballots to inspect.
    #[arg(long)]
    limit: Option<u32>,
}
impl_run_with_client_methods!(
    ModerationQuarantineBridgePlanArgs,
    Client::get_sorafs_moderation_quarantine_operator_panel,
);
const MODERATION_OPERATOR_SERVICE_DEFAULT_LISTEN: &str = "127.0.0.1:9201";
const MODERATION_OPERATOR_SERVICE_DEFAULT_MAX_BODY_BYTES: usize = 1024 * 1024;
const MODERATION_OPERATOR_CSRF_HEADER: &str = "X-SoraFS-Operator-CSRF";
#[derive(clap::Args, Debug)]
pub struct ModerationQuarantineOperatorServeArgs {
    /// Local host:port for the operator workflow service.
    #[arg(long, default_value = MODERATION_OPERATOR_SERVICE_DEFAULT_LISTEN)]
    listen: String,
    /// Default ballot limit for operator-panel and bridge-plan reads.
    #[arg(long)]
    limit: Option<u32>,
    /// Maximum accepted HTTP request body bytes.
    #[arg(long, default_value_t = MODERATION_OPERATOR_SERVICE_DEFAULT_MAX_BODY_BYTES)]
    max_body_bytes: usize,
}
#[derive(clap::Args, Debug)]
pub struct ModerationQuarantineOperatorCanaryArgs {
    /// Base URL of the deployed operator workflow service.
    #[arg(long = "operator-url", value_name = "URL")]
    operator_url: String,
    /// 16-byte local quarantine id encoded as hexadecimal.
    #[arg(long = "quarantine-id", value_name = "HEX")]
    quarantine_id: String,
    /// Maximum number of matching ballots to request from readback routes.
    #[arg(long)]
    limit: Option<u32>,
    /// HTTP timeout in seconds.
    #[arg(long = "timeout-secs", default_value_t = 30)]
    timeout_secs: u64,
    /// Optional path where the canary evidence JSON will be written.
    #[arg(long = "out", value_name = "PATH")]
    out: Option<PathBuf>,
}
impl Run for ModerationQuarantineOperatorServeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        let service = self.service(
            Arc::new(client),
            context.config().torii_api_url.as_str().to_string(),
            context.config().account.to_string(),
        )?;
        let listener = TcpListener::bind(&service.listen).wrap_err_with(|| {
            format!(
                "failed to bind SoraFS moderation operator service to `{}`",
                service.listen
            )
        })?;
        context.print_data(&service.status_json())?;
        let service = Arc::new(service);
        for stream in listener.incoming() {
            let service = Arc::clone(&service);
            match stream {
                Ok(stream) => {
                    thread::spawn(move || {
                        if let Err(err) = moderation_operator_handle_stream(stream, &service) {
                            eprintln!("SoraFS moderation operator service request failed: {err}");
                        }
                    });
                }
                Err(err) => {
                    return Err(eyre!(
                        "failed to accept SoraFS moderation operator service connection: {err}"
                    ));
                }
            }
        }
        Ok(())
    }
}
impl Run for ModerationQuarantineOperatorCanaryArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let timeout = Duration::from_secs(self.timeout_secs.max(1));
        let client = BlockingHttpClient::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .user_agent("sorafs-cli moderation-operator-canary")
            .build()
            .wrap_err("failed to construct SoraFS moderation operator canary HTTP client")?;
        self.run_with_fetch(context, |url| {
            moderation_operator_canary_http_get(&client, url)
        })
    }
}
impl ModerationQuarantineOperatorCanaryArgs {
    fn run_with_fetch<C, F>(&self, context: &mut C, mut fetch: F) -> Result<()>
    where
        C: RunContext,
        F: FnMut(&str) -> Result<ModerationOperatorCanaryHttpResponse>,
    {
        let operator_url = required_trimmed_text(&self.operator_url, "--operator-url")?;
        let quarantine_id = normalize_hex_digest::<16>(&self.quarantine_id, "--quarantine-id")?;
        let evidence = moderation_operator_canary_evidence_json(
            &operator_url,
            &quarantine_id,
            self.limit,
            &mut fetch,
        )?;
        if let Some(path) = &self.out {
            ensure_parent_dir(path)?;
            let bytes = norito::json::to_vec_pretty(&evidence)
                .wrap_err("failed to serialize SoraFS moderation operator canary evidence")?;
            fs::write(path, bytes).wrap_err_with(|| {
                format!(
                    "failed to write SoraFS moderation operator canary evidence to `{}`",
                    path.display()
                )
            })?;
        }
        context.print_data(&evidence)
    }
}
impl ModerationQuarantineOperatorServeArgs {
    fn service(
        &self,
        workflow_source: Arc<dyn ModerationOperatorWorkflowSource>,
        upstream: String,
        default_actor: String,
    ) -> Result<ModerationOperatorService> {
        if self.listen.trim().is_empty() {
            return Err(eyre!("--listen must not be empty"));
        }
        if self.max_body_bytes == 0 {
            return Err(eyre!("--max-body-bytes must be greater than zero"));
        }
        let csrf_token = generate_moderation_operator_csrf_token()?;
        Ok(ModerationOperatorService {
            listen: self.listen.trim().to_string(),
            default_limit: self.limit,
            max_body_bytes: self.max_body_bytes,
            upstream,
            default_actor,
            csrf_token,
            workflow_source,
        })
    }
}
impl ModerationQuarantineBridgePlanArgs {
    fn run_with<C, F>(&self, context: &mut C, get: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &str, SorafsModerationQuarantineFilter) -> Result<Response<Vec<u8>>>,
    {
        let quarantine_id = normalize_hex_digest::<16>(&self.quarantine_id, "--quarantine-id")?;
        let filter = SorafsModerationQuarantineFilter { limit: self.limit };
        let client = context.client_from_config();
        let response = get(&client, &quarantine_id, filter)?;
        render_moderation_quarantine_bridge_plan_response(context, response, &quarantine_id)
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum RepairCommand {
    /// List finalized chain-authoritative repair tasks.
    List(RepairListArgs),
    /// Claim a queued repair task with a native ledger action.
    Claim(RepairClaimArgs),
    /// Renew the current repair lease with a native ledger action.
    Renew(RepairRenewArgs),
    /// Commit a successful terminal repair outcome.
    Complete(RepairCompleteArgs),
    /// Commit an unsuccessful terminal repair outcome.
    Fail(RepairFailArgs),
    /// Atomically escalate a repair task into a terminal slash proposal.
    Escalate(RepairEscalateArgs),
}
impl_run_for_subcommand!(RepairCommand => List, Claim, Renew, Complete, Fail, Escalate);
#[derive(clap::Args, Debug)]
pub struct RepairListArgs {
    /// Fetch one canonical repair ticket instead of a page.
    #[arg(long = "ticket-id", value_name = "ID")]
    ticket_id: Option<String>,
    /// Bounded task page size (1 through 500).
    #[arg(long, value_name = "COUNT")]
    limit: Option<u32>,
    /// Optional finalized block height; requires `--expected-finalized-block-hash`.
    #[arg(long = "expected-finalized-height", value_name = "HEIGHT")]
    expected_finalized_height: Option<u64>,
    /// Optional finalized block hash; requires `--expected-finalized-height`.
    #[arg(long = "expected-finalized-block-hash", value_name = "HEX")]
    expected_finalized_block_hash: Option<String>,
    /// Optional exclusive immutable task-id cursor.
    #[arg(long = "after-task-id", value_name = "HEX")]
    after_task_id: Option<String>,
}
impl_run_with_client_methods!(
    RepairListArgs,
    Client::get_sorafs_repair_tasks,
    Client::get_sorafs_repair_task,
);
impl RepairListArgs {
    fn run_with<C, F, G>(&self, context: &mut C, list: F, get: G) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SorafsRepairTasksFilter<'_>) -> Result<Response<Vec<u8>>>,
        G: FnOnce(&Client, &str, &SorafsRepairFinalizedAnchor<'_>) -> Result<Response<Vec<u8>>>,
    {
        if self.limit.is_some_and(|limit| !(1..=500).contains(&limit)) {
            return Err(eyre!("--limit must be within 1..=500"));
        }
        if self.expected_finalized_height.is_some() != self.expected_finalized_block_hash.is_some()
        {
            return Err(eyre!(
                "--expected-finalized-height and --expected-finalized-block-hash must be supplied together"
            ));
        }
        if self.expected_finalized_height == Some(0) {
            return Err(eyre!("--expected-finalized-height must be non-zero"));
        }
        let finalized_block_hash = self
            .expected_finalized_block_hash
            .as_deref()
            .map(|hex| normalize_hex_digest::<32>(hex, "--expected-finalized-block-hash"))
            .transpose()?;
        let after_task_id = self
            .after_task_id
            .as_deref()
            .map(|hex| normalize_hex_digest::<32>(hex, "--after-task-id"))
            .transpose()?;
        let finalized = SorafsRepairFinalizedAnchor {
            expected_finalized_height: self.expected_finalized_height,
            expected_finalized_block_hash_hex: finalized_block_hash.as_deref(),
        };
        let client = context.client_from_config();
        let response = match self.ticket_id.as_deref() {
            Some(ticket_id) => {
                if self.limit.is_some() || after_task_id.is_some() {
                    return Err(eyre!(
                        "--limit and --after-task-id cannot be combined with --ticket-id"
                    ));
                }
                let ticket_id = parse_repair_ticket_id(ticket_id, "--ticket-id")?;
                get(&client, &ticket_id.0, &finalized)?
            }
            None => {
                let filter = SorafsRepairTasksFilter {
                    finalized,
                    limit: self.limit,
                    after_task_id_hex: after_task_id.as_deref(),
                };
                list(&client, &filter)?
            }
        };
        render_json_response(context, response)
    }
}
#[derive(clap::Args, Debug)]
pub struct RepairClaimArgs {
    /// Repair ticket identifier (e.g., `REP-401`).
    #[arg(long = "ticket-id", value_name = "ID")]
    ticket_id: String,
    /// Exact task revision observed before claiming.
    #[arg(long = "expected-revision", value_name = "REVISION")]
    expected_revision: u64,
    /// Requested lease duration measured from the committing block time.
    #[arg(long = "lease-duration-ms", default_value_t = 60_000)]
    lease_duration_ms: u64,
    /// Optional idempotency key (auto-generated when omitted).
    #[arg(long = "idempotency-key", value_name = "KEY")]
    idempotency_key: Option<String>,
}
impl_run_with_client_methods!(RepairClaimArgs, Client::post_sorafs_repair_claim);
impl RepairClaimArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SignedTransaction) -> Result<HashOf<SignedTransaction>>,
    {
        ensure_optional_non_empty(self.idempotency_key.as_deref(), "idempotency-key")?;
        let ticket_id = parse_repair_ticket_id(&self.ticket_id, "--ticket-id")?;
        validate_repair_revision(self.expected_revision, "--expected-revision")?;
        if self.lease_duration_ms == 0 {
            return Err(eyre!("--lease-duration-ms must be non-zero"));
        }
        let idempotency_key = match self.idempotency_key.clone() {
            Some(idempotency_key) => idempotency_key,
            None => generate_nonce_hex(12)?,
        };
        let action = SorafsRepairTaskActionV1::Claim(SorafsRepairClaimV1 {
            lease_duration_ms: self.lease_duration_ms,
            idempotency_key,
        });
        let client = context.client_from_config();
        let transaction =
            build_repair_action_transaction(&client, &ticket_id, self.expected_revision, action)?;
        let hash = submit(&client, &transaction)?;
        render_repair_transaction_hash(context, &hash)
    }
}
#[derive(clap::Args, Debug)]
pub struct RepairRenewArgs {
    /// Repair ticket identifier (e.g., `REP-401`).
    #[arg(long = "ticket-id", value_name = "ID")]
    ticket_id: String,
    /// Exact task revision observed before renewing.
    #[arg(long = "expected-revision", value_name = "REVISION")]
    expected_revision: u64,
    /// Exact current lease generation.
    #[arg(long = "lease-generation", value_name = "GENERATION")]
    lease_generation: u64,
    /// Requested lease duration measured from the committing block time.
    #[arg(long = "lease-duration-ms", default_value_t = 60_000)]
    lease_duration_ms: u64,
    /// Optional idempotency key (auto-generated when omitted).
    #[arg(long = "idempotency-key", value_name = "KEY")]
    idempotency_key: Option<String>,
}
impl_run_with_client_methods!(RepairRenewArgs, Client::post_sorafs_repair_heartbeat);
impl RepairRenewArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SignedTransaction) -> Result<HashOf<SignedTransaction>>,
    {
        ensure_optional_non_empty(self.idempotency_key.as_deref(), "idempotency-key")?;
        let ticket_id = parse_repair_ticket_id(&self.ticket_id, "--ticket-id")?;
        validate_repair_revision(self.expected_revision, "--expected-revision")?;
        validate_repair_revision(self.lease_generation, "--lease-generation")?;
        if self.lease_duration_ms == 0 {
            return Err(eyre!("--lease-duration-ms must be non-zero"));
        }
        let idempotency_key = match self.idempotency_key.clone() {
            Some(idempotency_key) => idempotency_key,
            None => generate_nonce_hex(12)?,
        };
        let action = SorafsRepairTaskActionV1::Renew(SorafsRepairRenewV1 {
            lease_generation: self.lease_generation,
            lease_duration_ms: self.lease_duration_ms,
            idempotency_key,
        });
        let client = context.client_from_config();
        let transaction =
            build_repair_action_transaction(&client, &ticket_id, self.expected_revision, action)?;
        let hash = submit(&client, &transaction)?;
        render_repair_transaction_hash(context, &hash)
    }
}
#[derive(clap::Args, Debug)]
pub struct RepairCompleteArgs {
    /// Repair ticket identifier (e.g., `REP-401`).
    #[arg(long = "ticket-id", value_name = "ID")]
    ticket_id: String,
    /// Exact task revision observed before completion.
    #[arg(long = "expected-revision", value_name = "REVISION")]
    expected_revision: u64,
    /// Exact current lease generation.
    #[arg(long = "lease-generation", value_name = "GENERATION")]
    lease_generation: u64,
    /// Digest of external completion evidence.
    #[arg(long = "evidence-digest", value_name = "HEX")]
    evidence_digest: String,
    /// Optional idempotency key (auto-generated when omitted).
    #[arg(long = "idempotency-key", value_name = "KEY")]
    idempotency_key: Option<String>,
}
impl_run_with_client_methods!(RepairCompleteArgs, Client::post_sorafs_repair_complete);
impl RepairCompleteArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SignedTransaction) -> Result<HashOf<SignedTransaction>>,
    {
        ensure_optional_non_empty(self.idempotency_key.as_deref(), "idempotency-key")?;
        let ticket_id = parse_repair_ticket_id(&self.ticket_id, "--ticket-id")?;
        validate_repair_revision(self.expected_revision, "--expected-revision")?;
        validate_repair_revision(self.lease_generation, "--lease-generation")?;
        let evidence_digest = parse_hex_array::<32>(&self.evidence_digest, "--evidence-digest")?;
        let idempotency_key = match self.idempotency_key.clone() {
            Some(idempotency_key) => idempotency_key,
            None => generate_nonce_hex(12)?,
        };
        let action = SorafsRepairTaskActionV1::Complete(SorafsRepairCompleteV1 {
            lease_generation: self.lease_generation,
            evidence_digest,
            idempotency_key,
        });
        let client = context.client_from_config();
        let transaction =
            build_repair_action_transaction(&client, &ticket_id, self.expected_revision, action)?;
        let hash = submit(&client, &transaction)?;
        render_repair_transaction_hash(context, &hash)
    }
}
#[derive(clap::Args, Debug)]
pub struct RepairFailArgs {
    /// Repair ticket identifier (e.g., `REP-401`).
    #[arg(long = "ticket-id", value_name = "ID")]
    ticket_id: String,
    /// Exact task revision observed before failure.
    #[arg(long = "expected-revision", value_name = "REVISION")]
    expected_revision: u64,
    /// Exact current lease generation.
    #[arg(long = "lease-generation", value_name = "GENERATION")]
    lease_generation: u64,
    /// Digest of the external failure reason or evidence.
    #[arg(long = "failure-digest", value_name = "HEX")]
    failure_digest: String,
    /// Optional idempotency key (auto-generated when omitted).
    #[arg(long = "idempotency-key", value_name = "KEY")]
    idempotency_key: Option<String>,
}
impl_run_with_client_methods!(RepairFailArgs, Client::post_sorafs_repair_fail);
impl RepairFailArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SignedTransaction) -> Result<HashOf<SignedTransaction>>,
    {
        ensure_optional_non_empty(self.idempotency_key.as_deref(), "idempotency-key")?;
        let ticket_id = parse_repair_ticket_id(&self.ticket_id, "--ticket-id")?;
        validate_repair_revision(self.expected_revision, "--expected-revision")?;
        validate_repair_revision(self.lease_generation, "--lease-generation")?;
        let failure_digest = parse_hex_array::<32>(&self.failure_digest, "--failure-digest")?;
        let idempotency_key = match self.idempotency_key.clone() {
            Some(idempotency_key) => idempotency_key,
            None => generate_nonce_hex(12)?,
        };
        let action = SorafsRepairTaskActionV1::Fail(SorafsRepairFailV1 {
            lease_generation: self.lease_generation,
            failure_digest,
            idempotency_key,
        });
        let client = context.client_from_config();
        let transaction =
            build_repair_action_transaction(&client, &ticket_id, self.expected_revision, action)?;
        let hash = submit(&client, &transaction)?;
        render_repair_transaction_hash(context, &hash)
    }
}
#[derive(clap::Args, Debug)]
pub struct RepairEscalateArgs {
    /// Repair ticket identifier (e.g., `REP-401`).
    #[arg(long = "ticket-id", value_name = "ID")]
    ticket_id: String,
    /// Exact task revision observed before escalation.
    #[arg(long = "expected-revision", value_name = "REVISION")]
    expected_revision: u64,
    /// Exact current lease generation.
    #[arg(long = "lease-generation", value_name = "GENERATION")]
    lease_generation: u64,
    /// Manifest digest bound to the ticket (hex-encoded).
    #[arg(long = "manifest-digest", value_name = "HEX")]
    manifest_digest: String,
    /// Provider identifier owning the ticket (hex-encoded).
    #[arg(long = "provider-id", value_name = "HEX")]
    provider_id: String,
    /// Proposed exact XOR-denominated penalty.
    #[arg(long = "penalty", value_name = "QUANTITY")]
    penalty: String,
    /// Escalation rationale for governance review.
    #[arg(long = "rationale", value_name = "TEXT")]
    rationale: String,
    /// Optional auditor account (defaults to the CLI account).
    #[arg(long = "auditor", value_name = "ACCOUNT_ID")]
    auditor: Option<String>,
    /// Optional timestamp for the proposal (RFC3339 or `@unix_seconds`).
    #[arg(long = "submitted-at", value_name = "RFC3339|@UNIX")]
    submitted_at: Option<String>,
    /// Optional idempotency key (auto-generated when omitted).
    #[arg(long = "idempotency-key", value_name = "KEY")]
    idempotency_key: Option<String>,
}
impl_run_with_client_methods!(RepairEscalateArgs, Client::post_sorafs_repair_slash);
impl RepairEscalateArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SignedTransaction) -> Result<HashOf<SignedTransaction>>,
    {
        ensure_optional_non_empty(self.idempotency_key.as_deref(), "idempotency-key")?;
        if self.rationale.trim().is_empty() {
            return Err(eyre!("--rationale must not be empty"));
        }
        let ticket_id = parse_repair_ticket_id(&self.ticket_id, "--ticket-id")?;
        validate_repair_revision(self.expected_revision, "--expected-revision")?;
        validate_repair_revision(self.lease_generation, "--lease-generation")?;
        let manifest_digest = parse_hex_array::<32>(&self.manifest_digest, "--manifest-digest")?;
        let provider_id = parse_hex_array::<32>(&self.provider_id, "--provider-id")?;
        let auditor_account = match self.auditor.as_deref() {
            Some(raw) => parse_account_id_str(context, raw, "--auditor")?.to_string(),
            None => context.config().account.to_string(),
        };
        let submitted_at_unix =
            parse_timestamp_or_now(self.submitted_at.as_deref(), "submitted-at")?;
        let proposed_penalty = parse_xor_quantity_labeled(&self.penalty, "--penalty")?;
        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: ticket_id.clone(),
            provider_id,
            manifest_digest,
            auditor_account,
            proposed_penalty,
            submitted_at_unix,
            rationale: self.rationale.clone(),
            // Approval summaries embedded by the proposal submitter are not an
            // authority source. Governance decisions derive only from
            // authenticated records committed to the native repair ledger.
            approval: None,
        };
        proposal
            .validate()
            .map_err(|err| eyre!("invalid repair slash proposal payload: {err}"))?;
        let idempotency_key = match self.idempotency_key.clone() {
            Some(idempotency_key) => idempotency_key,
            None => generate_nonce_hex(12)?,
        };
        let action = SorafsRepairTaskActionV1::Escalate(SorafsRepairEscalateV1 {
            lease_generation: self.lease_generation,
            slash_proposal_payload: norito::to_bytes(&proposal)
                .wrap_err("failed to encode canonical repair slash proposal")?,
            idempotency_key,
        });
        let client = context.client_from_config();
        let transaction =
            build_repair_action_transaction(&client, &ticket_id, self.expected_revision, action)?;
        let hash = submit(&client, &transaction)?;
        render_repair_transaction_hash(context, &hash)
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum GcCommand {
    /// Inspect retained manifests and retention deadlines.
    Inspect(GcInspectArgs),
    /// Report which manifests would be evicted by GC (dry-run only).
    DryRun(GcDryRunArgs),
}
impl_run_for_subcommand!(GcCommand => Inspect, DryRun);
#[derive(clap::Args, Debug)]
pub struct GcInspectArgs {
    /// Root directory for SoraFS storage data (defaults to the node config default).
    #[arg(long = "data-dir", value_name = "PATH")]
    data_dir: Option<PathBuf>,
    /// Override the reference timestamp (RFC3339 or `@unix_seconds`).
    #[arg(long = "now", value_name = "RFC3339|@UNIX")]
    now: Option<String>,
    /// Override the retention grace window in seconds.
    #[arg(long = "grace-secs", value_name = "SECONDS")]
    grace_secs: Option<u64>,
}
impl Run for GcInspectArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let report = build_gc_report(
            "inspect",
            self.data_dir.as_deref(),
            self.now.as_deref(),
            self.grace_secs,
            false,
        )?;
        context.print_data(&report)
    }
}
#[derive(clap::Args, Debug)]
pub struct GcDryRunArgs {
    /// Root directory for SoraFS storage data (defaults to the node config default).
    #[arg(long = "data-dir", value_name = "PATH")]
    data_dir: Option<PathBuf>,
    /// Override the reference timestamp (RFC3339 or `@unix_seconds`).
    #[arg(long = "now", value_name = "RFC3339|@UNIX")]
    now: Option<String>,
    /// Override the retention grace window in seconds.
    #[arg(long = "grace-secs", value_name = "SECONDS")]
    grace_secs: Option<u64>,
}
impl Run for GcDryRunArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let report = build_gc_report(
            "dry_run",
            self.data_dir.as_deref(),
            self.now.as_deref(),
            self.grace_secs,
            true,
        )?;
        context.print_data(&report)
    }
}
#[derive(Debug)]
struct GcManifestEntry {
    manifest_id: String,
    manifest_digest_hex: String,
    storage_class: ManifestStorageClass,
    retention_epoch: u64,
    retention_sources: Vec<String>,
    payload_bytes: u64,
    car_bytes: u64,
}
#[derive(Debug, norito::json::JsonSerialize)]
struct GcReportOutput {
    mode: String,
    data_dir: String,
    now_unix: u64,
    grace_secs: u64,
    total_manifests: usize,
    total_payload_bytes: u64,
    total_car_bytes: u64,
    expired_count: usize,
    expired_payload_bytes: u64,
    expired_car_bytes: u64,
    entries: Vec<GcReportEntry>,
}
#[derive(Debug, norito::json::JsonSerialize)]
struct GcReportEntry {
    manifest_id: String,
    manifest_digest_hex: String,
    storage_class: String,
    retention_epoch: u64,
    retention_sources: Vec<String>,
    expires_at_unix: Option<u64>,
    expired: bool,
    payload_bytes: u64,
    car_bytes: u64,
}
const SORAFS_MANIFEST_DIR: &str = "manifests";
const SORAFS_MANIFEST_FILE: &str = "manifest.to";
fn build_gc_report(
    mode: &str,
    data_dir: Option<&Path>,
    now: Option<&str>,
    grace_secs: Option<u64>,
    only_expired: bool,
) -> Result<GcReportOutput> {
    let data_dir = data_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(defaults::sorafs::storage::data_dir);
    let now_unix = parse_timestamp_or_now(now, "now")?;
    let grace_secs = grace_secs.unwrap_or(defaults::sorafs::gc::RETENTION_GRACE_SECS);
    let mut entries = load_gc_manifest_entries(&data_dir)?;
    entries.sort_by(|left, right| left.manifest_id.cmp(&right.manifest_id));
    let total_manifests = entries.len();
    let mut report_entries = Vec::with_capacity(entries.len());
    let mut total_payload_bytes = 0_u64;
    let mut total_car_bytes = 0_u64;
    let mut expired_count = 0_usize;
    let mut expired_payload_bytes = 0_u64;
    let mut expired_car_bytes = 0_u64;
    for entry in entries {
        total_payload_bytes = total_payload_bytes.saturating_add(entry.payload_bytes);
        total_car_bytes = total_car_bytes.saturating_add(entry.car_bytes);
        let expires_at_unix = retention_deadline(entry.retention_epoch, grace_secs);
        let expired = expires_at_unix.is_some_and(|deadline| now_unix >= deadline);
        if expired {
            expired_count += 1;
            expired_payload_bytes = expired_payload_bytes.saturating_add(entry.payload_bytes);
            expired_car_bytes = expired_car_bytes.saturating_add(entry.car_bytes);
        }
        if only_expired && !expired {
            continue;
        }
        report_entries.push(GcReportEntry {
            manifest_id: entry.manifest_id,
            manifest_digest_hex: entry.manifest_digest_hex,
            storage_class: manifest_storage_class_label(entry.storage_class).to_string(),
            retention_epoch: entry.retention_epoch,
            retention_sources: entry.retention_sources,
            expires_at_unix,
            expired,
            payload_bytes: entry.payload_bytes,
            car_bytes: entry.car_bytes,
        });
    }
    Ok(GcReportOutput {
        mode: mode.to_string(),
        data_dir: data_dir.display().to_string(),
        now_unix,
        grace_secs,
        total_manifests,
        total_payload_bytes,
        total_car_bytes,
        expired_count,
        expired_payload_bytes,
        expired_car_bytes,
        entries: report_entries,
    })
}
fn load_gc_manifest_entries(data_dir: &Path) -> Result<Vec<GcManifestEntry>> {
    let manifests_dir = data_dir.join(SORAFS_MANIFEST_DIR);
    if !manifests_dir.exists() {
        return Err(eyre!(
            "SoraFS manifests directory `{}` does not exist",
            manifests_dir.display()
        ));
    }
    let mut entries = Vec::new();
    for dir_entry in fs::read_dir(&manifests_dir)
        .wrap_err_with(|| format!("failed to read `{}`", manifests_dir.display()))?
    {
        let dir_entry = dir_entry?;
        let file_type = dir_entry.file_type()?;
        if !file_type.is_dir() {
            continue;
        }
        let manifest_id = dir_entry.file_name().to_string_lossy().to_string();
        let manifest_path = dir_entry.path().join(SORAFS_MANIFEST_FILE);
        let manifest_bytes = fs::read(&manifest_path)
            .wrap_err_with(|| format!("failed to read manifest `{}`", manifest_path.display()))?;
        let manifest: ManifestV1 = norito::decode_from_bytes(&manifest_bytes)
            .wrap_err_with(|| format!("failed to decode `{}`", manifest_path.display()))?;
        let digest = manifest
            .digest()
            .wrap_err_with(|| format!("failed to hash `{}`", manifest_path.display()))?;
        let retention_source = sorafs_manifest::retention::RetentionSourceV1::from_manifest(
            &manifest,
        )
        .wrap_err_with(|| {
            format!(
                "failed to parse retention metadata for `{}`",
                manifest_path.display()
            )
        })?;
        let retention_sources = retention_source
            .sources
            .iter()
            .map(|source| source.to_string())
            .collect::<Vec<_>>();
        entries.push(GcManifestEntry {
            manifest_id,
            manifest_digest_hex: encode(digest.as_bytes()),
            storage_class: manifest.pin_policy.storage_class,
            retention_epoch: retention_source.effective_epoch(),
            retention_sources,
            payload_bytes: manifest.content_length,
            car_bytes: manifest.car_size,
        });
    }
    Ok(entries)
}
fn retention_deadline(retention_epoch: u64, grace_secs: u64) -> Option<u64> {
    if retention_epoch == 0 {
        return None;
    }
    Some(retention_epoch.saturating_add(grace_secs))
}
const fn manifest_storage_class_label(class: ManifestStorageClass) -> &'static str {
    match class {
        ManifestStorageClass::Hot => "hot",
        ManifestStorageClass::Warm => "warm",
        ManifestStorageClass::Cold => "cold",
    }
}
#[derive(clap::Args, Debug)]
pub struct ReserveQuoteArgs {
    /// Storage class targeted by the commitment (hot, warm, cold).
    #[arg(long = "storage-class", value_enum)]
    storage_class: StorageClassArg,
    /// Provider tier (tier-a, tier-b, tier-c).
    #[arg(long = "tier", value_enum)]
    tier: ReserveTierArg,
    /// Commitment duration (`monthly`, `quarterly`, `annual`).
    #[arg(long = "duration", value_enum, default_value = "monthly")]
    duration: ReserveDurationArg,
    /// Logical GiB covered by the quote.
    #[arg(long = "gib", value_name = "GIB")]
    pub capacity_gib: u64,
    /// Canonical XOR reserve balance applied while computing effective rent (up to 9 fractional digits).
    #[arg(long = "reserve-balance", value_name = "XOR", default_value = "0")]
    pub reserve_balance: String,
    /// Optional path to a JSON-encoded reserve policy (`ReservePolicyV1`).
    #[arg(long = "policy-json", value_name = "PATH")]
    pub policy_json: Option<PathBuf>,
    /// Optional path to a Norito-encoded reserve policy (`ReservePolicyV1`).
    #[arg(long = "policy-norito", value_name = "PATH")]
    pub policy_norito: Option<PathBuf>,
    /// Optional path for persisting the rendered quote JSON.
    #[arg(long = "quote-out", value_name = "PATH")]
    pub quote_out: Option<PathBuf>,
}
impl ReserveQuoteArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let storage_class = self.storage_class.to_storage_class();
        let tier = self.tier.to_policy_tier();
        let duration = self.duration.to_policy_duration();
        let reserve_balance = parse_xor_quantity(&self.reserve_balance)?;
        let (policy, source_label) = load_reserve_policy_from_paths(
            self.policy_json.as_deref(),
            self.policy_norito.as_deref(),
        )?;
        let quote = policy
            .quote(
                storage_class,
                self.capacity_gib,
                duration,
                tier,
                reserve_balance.clone(),
            )
            .wrap_err("failed to compute reserve quote")?;
        let value = build_reserve_quote_value(
            &policy,
            storage_class,
            tier,
            duration,
            self.capacity_gib,
            &reserve_balance,
            &quote,
            &source_label,
        )?;
        if let Some(path) = self.quote_out.as_deref() {
            write_reserve_quote_artifact(path, &value)?;
        }
        context.print_data(&value)
    }
}
#[derive(clap::Args, Debug)]
pub struct ReserveLedgerArgs {
    /// Path to the reserve quote JSON (output of `sorafs reserve quote`).
    #[arg(long = "quote", value_name = "PATH")]
    pub quote_path: PathBuf,
    /// Provider account paying the rent and reserve top-ups.
    #[arg(long = "provider-account", value_name = "ACCOUNT_ID")]
    pub provider_account: String,
    /// Treasury account receiving the rent payment.
    #[arg(long = "treasury-account", value_name = "ACCOUNT_ID")]
    pub treasury_account: String,
    /// Reserve escrow account receiving the reserve top-up.
    #[arg(long = "reserve-account", value_name = "ACCOUNT_ID")]
    pub reserve_account: String,
    /// Asset definition identifier used for transfers (canonical unprefixed Base58 address).
    #[arg(long = "asset-definition", value_name = "AID")]
    pub asset_definition: String,
}
impl ReserveLedgerArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let quote_contents = fs::read_to_string(&self.quote_path).wrap_err_with(|| {
            format!(
                "failed to read reserve quote `{}`",
                self.quote_path.display()
            )
        })?;
        let quote_value: Value =
            norito::json::from_str(&quote_contents).wrap_err("failed to parse reserve quote")?;
        let projection = extract_ledger_projection(&quote_value)?;
        let provider = crate::resolve_account_id(context, &self.provider_account)
            .wrap_err("failed to resolve --provider-account")?;
        let treasury = crate::resolve_account_id(context, &self.treasury_account)
            .wrap_err("failed to resolve --treasury-account")?;
        let reserve = crate::resolve_account_id(context, &self.reserve_account)
            .wrap_err("failed to resolve --reserve-account")?;
        let asset_definition = AssetDefinitionId::parse_address_literal(&self.asset_definition)
            .wrap_err("failed to parse --asset-definition")?;
        let plan = build_reserve_ledger_plan(
            &self.quote_path,
            projection,
            &provider,
            &treasury,
            &reserve,
            &asset_definition,
        )?;
        context.print_data(&plan)
    }
}
#[derive(clap::Args, Debug)]
pub struct ReserveLifecycleArgs {
    /// Path to the reserve quote JSON (output of `sorafs reserve quote`).
    #[arg(long = "quote", value_name = "PATH")]
    pub quote_path: PathBuf,
    /// Days since rent became due.
    #[arg(long = "days-past-due", value_name = "DAYS", default_value_t = 0)]
    pub days_past_due: u16,
    /// Grace window before delinquency.
    #[arg(long = "grace-days", value_name = "DAYS", default_value_t = 7)]
    pub grace_period_days: u16,
    /// Default threshold after the due date.
    #[arg(long = "default-after-days", value_name = "DAYS", default_value_t = 30)]
    pub default_after_days: u16,
}
impl ReserveLifecycleArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let quote_contents = fs::read_to_string(&self.quote_path).wrap_err_with(|| {
            format!(
                "failed to read reserve quote `{}`",
                self.quote_path.display()
            )
        })?;
        let quote_value: Value =
            norito::json::from_str(&quote_contents).wrap_err("failed to parse reserve quote")?;
        let quote = extract_reserve_quote(&quote_value)?;
        let lifecycle = quote
            .lifecycle_projection(
                self.days_past_due,
                self.grace_period_days,
                self.default_after_days,
            )
            .wrap_err("failed to compute reserve lifecycle projection")?;
        let value = build_reserve_lifecycle_value(&self.quote_path, &lifecycle)?;
        context.print_data(&value)
    }
}
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum GarActionArg {
    #[value(name = "purge-static-zone")]
    PurgeStaticZone,
    #[value(name = "cache-bypass")]
    CacheBypass,
    #[value(name = "ttl-override")]
    TtlOverride,
    #[value(name = "rate-limit-override")]
    RateLimitOverride,
    #[value(name = "geo-fence")]
    GeoFence,
    #[value(name = "legal-hold")]
    LegalHold,
    #[value(name = "moderation")]
    Moderation,
    #[value(name = "audit-notice")]
    AuditNotice,
    #[value(name = "custom")]
    Custom,
}
impl GarActionArg {
    fn to_enforcement_action(self, custom_slug: Option<&str>) -> Result<GarEnforcementActionV1> {
        Ok(match self {
            Self::PurgeStaticZone => GarEnforcementActionV1::PurgeStaticZone,
            Self::CacheBypass => GarEnforcementActionV1::CacheBypass,
            Self::TtlOverride => GarEnforcementActionV1::TtlOverride,
            Self::RateLimitOverride => GarEnforcementActionV1::RateLimitOverride,
            Self::GeoFence => GarEnforcementActionV1::GeoFence,
            Self::LegalHold => GarEnforcementActionV1::LegalHold,
            Self::Moderation => GarEnforcementActionV1::Moderation,
            Self::AuditNotice => GarEnforcementActionV1::AuditNotice,
            Self::Custom => {
                let slug = custom_slug.ok_or_else(|| {
                    eyre!("--custom-action-slug must be supplied when --action=custom is used")
                })?;
                GarEnforcementActionV1::Custom(slug.to_string())
            }
        })
    }
}
#[derive(clap::Args, Debug)]
pub struct GarReceiptArgs {
    /// Registered GAR name (`SoraDNS` label, e.g., `docs.sora`).
    #[arg(long = "gar-name", value_name = "LABEL")]
    gar_name: String,
    /// Canonical host affected by the enforcement action.
    #[arg(long = "canonical-host", value_name = "HOST")]
    canonical_host: String,
    /// Enforcement action recorded in the receipt.
    #[arg(long = "action", value_enum, default_value = "audit-notice")]
    action: GarActionArg,
    /// Slug recorded when `--action custom` is selected.
    #[arg(long = "custom-action-slug", value_name = "SLUG")]
    custom_action_slug: Option<String>,
    /// Optional receipt identifier (32 hex chars / 16 bytes). Defaults to a random ULID-like value.
    #[arg(long = "receipt-id", value_name = "HEX16")]
    receipt_id_hex: Option<String>,
    /// Override the triggered timestamp (RFC3339 or `@unix_seconds`). Defaults to `now`.
    #[arg(long = "triggered-at", value_name = "RFC3339|@UNIX")]
    triggered_at: Option<String>,
    /// Optional expiry timestamp (RFC3339 or `@unix_seconds`).
    #[arg(long = "expires-at", value_name = "RFC3339|@UNIX")]
    expires_at: Option<String>,
    /// Policy version label recorded in the receipt.
    #[arg(long = "policy-version", value_name = "STRING")]
    policy_version: Option<String>,
    /// Policy digest (64 hex chars / 32 bytes) referenced by the receipt.
    #[arg(long = "policy-digest", value_name = "HEX32")]
    policy_digest_hex: Option<String>,
    /// Operator account that executed the action.
    #[arg(long = "operator", value_name = "ACCOUNT_ID")]
    operator: String,
    /// Human-readable reason for the enforcement action.
    #[arg(long = "reason", value_name = "TEXT")]
    reason: String,
    /// Optional notes captured for auditors.
    #[arg(long = "notes", value_name = "TEXT")]
    notes: Option<String>,
    /// Evidence URIs (repeatable) recorded with the receipt.
    #[arg(long = "evidence-uri", value_name = "URI")]
    evidence_uri: Vec<String>,
    /// Machine-readable labels (repeatable) applied to the receipt.
    #[arg(long = "label", value_name = "TAG")]
    labels: Vec<String>,
    /// Path for persisting the JSON artefact (pretty-printed).
    #[arg(long = "json-out", value_name = "PATH")]
    json_out: Option<PathBuf>,
    /// Path for persisting the Norito-encoded receipt (`.to` bytes).
    #[arg(long = "norito-out", value_name = "PATH")]
    norito_out: Option<PathBuf>,
}
impl GarReceiptArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let receipt_id = parse_receipt_id(self.receipt_id_hex.as_deref())?;
        let triggered_at = parse_timestamp_or_now(self.triggered_at.as_deref(), "triggered-at")?;
        let expires_at = parse_optional_timestamp(self.expires_at.as_deref(), "expires-at")?;
        let policy_digest =
            parse_optional_hex_array::<32>(self.policy_digest_hex.as_deref(), "--policy-digest")?;
        let operator = crate::resolve_account_id(context, &self.operator)
            .wrap_err("failed to resolve --operator")?;
        let action = self
            .action
            .to_enforcement_action(self.custom_action_slug.as_deref())?;
        let receipt = GarEnforcementReceiptV1 {
            receipt_id,
            gar_name: self.gar_name,
            canonical_host: self.canonical_host,
            action,
            triggered_at_unix: triggered_at,
            expires_at_unix: expires_at,
            policy_version: self.policy_version,
            policy_digest,
            operator,
            reason: self.reason,
            notes: self.notes,
            evidence_uris: self.evidence_uri,
            labels: self.labels,
        };
        if let Some(path) = self.norito_out.as_deref() {
            let bytes = norito::to_bytes(&receipt).wrap_err("failed to encode receipt (Norito)")?;
            fs::write(path, bytes)
                .wrap_err_with(|| format!("failed to write Norito receipt `{}`", path.display()))?;
        }
        let json_value =
            norito::json::to_value(&receipt).wrap_err("failed to encode receipt JSON")?;
        if let Some(path) = self.json_out.as_deref() {
            let pretty = norito::json::to_string_pretty(&json_value)
                .wrap_err("failed to render receipt JSON")?;
            fs::write(path, pretty)
                .wrap_err_with(|| format!("failed to write receipt JSON `{}`", path.display()))?;
        }
        context.print_data(&json_value)
    }
}
fn parse_receipt_id(receipt_id_hex: Option<&str>) -> Result<[u8; 16]> {
    parse_receipt_id_with_rng(receipt_id_hex, &mut OsRng)
}
fn parse_receipt_id_with_rng<R: TryCryptoRng>(
    receipt_id_hex: Option<&str>,
    rng: &mut R,
) -> Result<[u8; 16]> {
    if let Some(hex) = receipt_id_hex {
        return parse_hex_array::<16>(hex, "--receipt-id");
    }
    let mut bytes = [0u8; 16];
    rng.try_fill_bytes(&mut bytes)
        .map_err(|error| eyre!("SoraFS receipt-id OS RNG failed: {error}"))?;
    Ok(bytes)
}
fn parse_optional_hex_array<const N: usize>(
    value: Option<&str>,
    field: &str,
) -> Result<Option<[u8; N]>> {
    value
        .map(|hex| parse_hex_array::<N>(hex, field))
        .transpose()
}
fn parse_timestamp_value(input: &str, field: &str) -> Result<u64> {
    if let Some(rest) = input.strip_prefix('@') {
        let value = rest
            .parse::<u64>()
            .wrap_err_with(|| format!("invalid unix timestamp for {field}"))?;
        return Ok(value);
    }
    let dt = OffsetDateTime::parse(input, &Rfc3339)
        .wrap_err_with(|| format!("failed to parse {field} (expected RFC3339 or @unix format)"))?;
    dt.unix_timestamp()
        .try_into()
        .wrap_err("timestamp is negative")
}
fn parse_timestamp_or_now(value: Option<&str>, field: &str) -> Result<u64> {
    value.map_or_else(
        || {
            let now = OffsetDateTime::now_utc();
            now.unix_timestamp()
                .try_into()
                .wrap_err("current timestamp overflowed i64")
        },
        |input| parse_timestamp_value(input, field),
    )
}
fn parse_optional_timestamp(value: Option<&str>, field: &str) -> Result<Option<u64>> {
    value
        .map(|input| parse_timestamp_value(input, field))
        .transpose()
}
#[cfg(test)]
mod gar_receipt_cli_tests {
    use super::*;
    #[test]
    fn parse_timestamp_accepts_rfc3339() {
        let ts = parse_timestamp_value("2026-05-10T10:15:00Z", "triggered-at")
            .expect("timestamp parsed");
        assert_eq!(ts, 1_778_408_100);
    }
    #[test]
    fn parse_timestamp_accepts_unix_prefix() {
        let ts = parse_timestamp_value("@1778408100", "triggered-at").expect("timestamp parsed");
        assert_eq!(ts, 1_778_408_100);
    }
    #[test]
    fn custom_action_requires_slug() {
        let err = GarActionArg::Custom
            .to_enforcement_action(None)
            .expect_err("missing slug should fail");
        assert!(err.to_string().contains("--custom-action-slug"));
    }
}
#[derive(clap::Args, Debug)]
pub struct FetchArgs {
    /// Path to the Norito-encoded manifest (`.to`) describing the payload layout.
    #[arg(long, value_name = "PATH", required_unless_present = "storage_ticket")]
    pub manifest: Option<PathBuf>,
    /// Path to a canonical payload-bound `sorafs.chunk_fetch_plan.v1` JSON envelope.
    #[arg(long, value_name = "PATH", required_unless_present = "storage_ticket")]
    pub plan: Option<PathBuf>,
    /// Hex-encoded manifest hash used as the manifest identifier on gateways.
    #[arg(
        long = "manifest-id",
        value_name = "HEX",
        required_unless_present = "storage_ticket"
    )]
    pub manifest_id: Option<String>,
    /// Gateway provider descriptor (`name=... , provider-id=... , base-url=... , stream-token=...`).
    #[arg(long = "gateway-provider", value_name = "SPEC", required = true)]
    pub gateway_provider: Vec<String>,
    /// Storage ticket identifier to fetch manifest + chunk plan automatically from Torii.
    #[arg(long = "storage-ticket", value_name = "HEX")]
    pub storage_ticket: Option<String>,
    /// Optional override for the Torii manifest endpoint used with `--storage-ticket`.
    #[arg(
        long = "manifest-endpoint",
        value_name = "URL",
        requires = "storage_ticket"
    )]
    pub manifest_endpoint: Option<String>,
    /// Directory for storing manifest/chunk-plan artefacts fetched via `--storage-ticket`.
    #[arg(
        long = "manifest-cache-dir",
        value_name = "PATH",
        requires = "storage_ticket"
    )]
    pub manifest_cache_dir: Option<PathBuf>,
    /// Optional client identifier forwarded to the gateway for auditing.
    #[arg(long = "client-id", value_name = "STRING")]
    pub client_id: Option<String>,
    /// Optional path to a Norito-encoded manifest envelope to satisfy gateway policy checks.
    #[arg(long = "manifest-envelope", value_name = "PATH")]
    pub manifest_envelope: Option<PathBuf>,
    /// Override the expected manifest CID (defaults to the manifest digest).
    #[arg(long = "manifest-cid", value_name = "HEX")]
    pub manifest_cid: Option<String>,
    /// Canonical blinded CID (base64url, no padding) forwarded via `SoraNet` headers.
    #[arg(long = "blinded-cid", value_name = "BASE64", requires = "salt_epoch")]
    pub blinded_cid: Option<String>,
    /// Salt epoch corresponding to the blinded CID headers.
    #[arg(long = "salt-epoch", value_name = "EPOCH")]
    pub salt_epoch: Option<u32>,
    /// Hex-encoded 32-byte salt used to derive the canonical blinded CID (computes `--blinded-cid`).
    #[arg(long = "salt-hex", value_name = "HEX", requires = "salt_epoch")]
    pub salt_hex: Option<String>,
    /// Override the chunker handle advertised to gateways.
    #[arg(long = "chunker-handle", value_name = "STRING")]
    pub chunker_handle: Option<String>,
    /// Limit the number of providers participating in the session.
    #[arg(long = "max-peers", value_name = "COUNT")]
    pub max_peers: Option<usize>,
    /// Maximum retry attempts per chunk (0 disables the cap).
    #[arg(long = "retry-budget", value_name = "COUNT")]
    pub retry_budget: Option<usize>,
    /// Override the default `soranet-first` transport policy (`soranet-first`, `soranet-strict`, or
    /// `direct-only`). Supply `direct-only` only when staging a downgrade or rehearsing the
    /// compliance drills captured in `roadmap.md`.
    #[arg(long = "transport-policy", value_name = "POLICY")]
    pub transport_policy: Option<String>,
    /// Override the anonymity policy with an exact V1 label (`anon-guard-pq`,
    /// `anon-majority-pq`, or `anon-strict-pq`).
    #[arg(long = "anonymity-policy", value_name = "POLICY")]
    pub anonymity_policy: Option<String>,
    /// Hint that tightens PQ expectations for write paths (`read-only` or `upload-pq-only`).
    #[arg(long = "write-mode", value_name = "MODE")]
    pub write_mode: Option<String>,
    /// Force the orchestrator to stay on a specific transport stage (`soranet-first`, `soranet-strict`, or `direct-only`).
    #[arg(long = "transport-policy-override", value_name = "POLICY")]
    pub transport_policy_override: Option<String>,
    /// Force the orchestrator to stay on an exact V1 anonymity policy.
    #[arg(long = "anonymity-policy-override", value_name = "POLICY")]
    pub anonymity_policy_override: Option<String>,
    /// Path to the persisted guard cache (Norito-encoded guard set).
    #[arg(
        long = "guard-cache",
        value_name = "PATH",
        requires_all = ["guard_cache_key_file", "guard_directory"]
    )]
    pub guard_cache: Option<PathBuf>,
    /// Owner-private file containing the exact 32 raw bytes used to authenticate the guard cache.
    #[arg(
        long = "guard-cache-key-file",
        value_name = "PATH",
        requires = "guard_cache"
    )]
    pub guard_cache_key_file: Option<PathBuf>,
    /// Path to a Norito guard directory snapshot used to refresh guard selections.
    #[arg(long = "guard-directory", value_name = "PATH")]
    pub guard_directory: Option<PathBuf>,
    /// Trusted domain-separated BLAKE3 digest of the exact guard directory bytes.
    #[arg(
        long = "guard-directory-digest",
        value_name = "HEX",
        requires = "guard_directory"
    )]
    pub guard_directory_digest: Option<String>,
    /// Target number of entry guards to pin (defaults to 3 when the guard directory is provided).
    #[arg(long = "guard-target", value_name = "COUNT")]
    pub guard_target: Option<usize>,
    /// Guard retention window in days (defaults to 30 when the guard directory is provided).
    #[arg(long = "guard-retention-days", value_name = "DAYS")]
    pub guard_retention_days: Option<u64>,
    /// Write the assembled payload to a file.
    #[arg(long = "output", value_name = "PATH")]
    pub output: Option<PathBuf>,
    /// Override the summary JSON path (defaults to `artifacts/sorafs_orchestrator/latest/summary.json`).
    #[arg(long = "json-out", value_name = "PATH")]
    pub json_out: Option<PathBuf>,
    /// Override the scoreboard JSON path (defaults to `artifacts/sorafs_orchestrator/latest/scoreboard.json`).
    #[arg(long = "scoreboard-out", value_name = "PATH")]
    pub scoreboard_out: Option<PathBuf>,
    /// Override the Unix timestamp used when evaluating provider adverts.
    #[arg(long = "scoreboard-now", value_name = "UNIX_SECS")]
    pub scoreboard_now: Option<u64>,
    /// Label describing the telemetry stream captured alongside the scoreboard (persisted in metadata).
    #[arg(long = "telemetry-source-label", value_name = "LABEL")]
    pub telemetry_source_label: Option<String>,
    /// Optional telemetry region label persisted in both the scoreboard metadata and summary JSON.
    #[arg(long = "telemetry-region", value_name = "LABEL")]
    pub telemetry_region: Option<String>,
}
#[derive(Debug)]
struct ManifestInputs {
    manifest_path: PathBuf,
    plan_path: PathBuf,
    manifest_id: String,
}
#[derive(Debug)]
struct DownloadedManifest {
    manifest_path: PathBuf,
    plan_path: PathBuf,
    manifest_id: String,
}
#[derive(Debug, Clone)]
struct ScoreboardCapturePaths {
    scoreboard: PathBuf,
    summary: Option<PathBuf>,
}
fn default_orchestrator_capture_dir() -> PathBuf {
    PathBuf::from("artifacts")
        .join("sorafs_orchestrator")
        .join("latest")
}
fn scoreboard_capture_paths(
    scoreboard_override: Option<PathBuf>,
    summary_override: Option<PathBuf>,
) -> ScoreboardCapturePaths {
    let scoreboard = scoreboard_override
        .unwrap_or_else(|| default_orchestrator_capture_dir().join("scoreboard.json"));
    let summary = summary_override.or_else(|| {
        scoreboard
            .parent()
            .map(|parent| parent.join("summary.json"))
    });
    ScoreboardCapturePaths {
        scoreboard,
        summary,
    }
}
fn insert_provider_counts(summary: &mut norito::json::Map, counts: ProviderCounts) {
    summary.insert(
        "provider_count".into(),
        norito::json::Value::from(counts.direct_u64()),
    );
    summary.insert(
        "gateway_provider_count".into(),
        norito::json::Value::from(counts.gateway_u64()),
    );
    summary.insert(
        "provider_mix".into(),
        norito::json::Value::from(counts.mix_label()),
    );
}
fn insert_transport_policy(
    summary: &mut norito::json::Map,
    transport_policy: Option<TransportPolicy>,
    transport_policy_override: Option<TransportPolicy>,
) {
    let (label, override_flag, override_label) =
        transport_policy_labels(transport_policy, transport_policy_override);
    summary.insert("transport_policy".into(), norito::json::Value::from(label));
    summary.insert(
        "transport_policy_override".into(),
        norito::json::Value::from(override_flag),
    );
    summary.insert(
        "transport_policy_override_label".into(),
        override_label.map_or(norito::json::Value::Null, norito::json::Value::from),
    );
}
fn insert_summary_telemetry_source(summary: &mut norito::json::Map, label: Option<&str>) {
    if let Some(value) = label {
        summary.insert("telemetry_source".into(), norito::json::Value::from(value));
    }
}
fn insert_summary_telemetry_region(summary: &mut norito::json::Map, label: Option<&str>) {
    if let Some(value) = label {
        summary.insert("telemetry_region".into(), norito::json::Value::from(value));
    }
}
fn public_local_proxy_manifest_value(manifest: &BrowserExtensionManifest) -> norito::json::Value {
    let mut public_manifest = manifest.clone();
    public_manifest.client_capability_hex = None;
    let mut value = norito::json::to_value(&public_manifest)
        .expect("local proxy manifest should serialise to JSON");
    if let norito::json::Value::Object(fields) = &mut value {
        fields.remove("client_capability_hex");
    }
    value
}
impl Run for FetchArgs {
    #[allow(clippy::too_many_lines)]
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if let Some(peers) = self.max_peers
            && peers == 0
        {
            return Err(eyre!("--max-peers must be at least 1 when provided"));
        }
        if self.guard_target.is_some() && self.guard_directory.is_none() {
            return Err(eyre!("--guard-target requires --guard-directory"));
        }
        if self.guard_retention_days.is_some() && self.guard_directory.is_none() {
            return Err(eyre!("--guard-retention-days requires --guard-directory"));
        }
        if self.guard_directory.is_some() && self.guard_directory_digest.is_none() {
            return Err(eyre!(
                "--guard-directory requires --guard-directory-digest from an independent trusted source"
            ));
        }
        if self.guard_cache.is_some() != self.guard_cache_key_file.is_some() {
            return Err(eyre!(
                "--guard-cache and --guard-cache-key-file must be supplied together"
            ));
        }
        if self.guard_cache.is_some() && self.guard_directory.is_none() {
            return Err(eyre!(
                "--guard-cache requires --guard-directory and its independently trusted --guard-directory-digest; cached guard state is not a freshness trust anchor"
            ));
        }
        let guard_cache_key = self
            .guard_cache_key_file
            .as_deref()
            .map(load_guard_cache_key_file)
            .transpose()?;
        let manifest_inputs = resolve_manifest_inputs(context, &self)?;
        let ManifestInputs {
            manifest_path,
            plan_path,
            manifest_id,
        } = manifest_inputs;
        let manifest_bytes = fs::read(&manifest_path).wrap_err_with(|| {
            format!("failed to read manifest from `{}`", manifest_path.display())
        })?;
        let manifest: ManifestV1 =
            norito::decode_from_bytes(&manifest_bytes).wrap_err("failed to decode manifest")?;
        let manifest_digest = manifest
            .digest()
            .wrap_err("failed to compute manifest digest")?;
        let cid_hex_override = if let Some(cid) = &self.manifest_cid {
            validate_hex_digest(cid, "--manifest-cid")?
        } else {
            hex::encode(manifest_digest.as_bytes())
        };
        let plan_bytes = fs::read(&plan_path).wrap_err_with(|| {
            format!("failed to read chunk plan from `{}`", plan_path.display())
        })?;
        let plan_value: norito::json::Value =
            norito::json::from_slice(&plan_bytes).wrap_err("failed to parse chunk plan JSON")?;
        let parsed_plan = chunk_fetch_plan_from_json(&plan_value)
            .map_err(|err| eyre!("failed to parse canonical chunk fetch plan: {err}"))?;
        let plan_payload_digest = parsed_plan.payload_digest;
        let mut chunk_specs = parsed_plan.chunk_fetch_specs;
        if chunk_specs.is_empty() {
            return Err(eyre!("chunk fetch plan contained no entries"));
        }
        chunk_specs.sort_by_key(|spec| spec.chunk_index);
        for (idx, spec) in chunk_specs.iter().enumerate() {
            if spec.chunk_index != idx {
                return Err(eyre!(
                    "chunk fetch plan missing chunk index {idx} (found {})",
                    spec.chunk_index
                ));
            }
        }
        let content_length = chunk_specs
            .iter()
            .map(|spec| spec.offset + u64::from(spec.length))
            .max()
            .expect("non-empty chunk specs");
        let manifest_id_bytes = parse_digest_hex(&manifest_id)
            .map_err(|_| eyre!("--manifest-id must be a 64-character hex-encoded BLAKE3 digest"))?;
        if manifest_id_bytes != *manifest_digest.as_bytes() {
            return Err(eyre!(
                "--manifest-id must match the manifest hash (expected {})",
                hex::encode(manifest_digest.as_bytes())
            ));
        }
        let manifest_id_hex = hex::encode(manifest_id_bytes);
        let payload_digest_hex = hex::encode(plan_payload_digest);
        let payload_digest = blake3::Hash::from_bytes(plan_payload_digest);
        let transport_policy =
            parse_transport_policy_flag(self.transport_policy.as_ref(), "--transport-policy")?;
        let anonymity_policy =
            parse_anonymity_policy_flag(self.anonymity_policy.as_ref(), "--anonymity-policy")?;
        let write_mode = parse_write_mode_flag(self.write_mode.as_ref(), "--write-mode")?;
        let transport_policy_override = parse_transport_policy_flag(
            self.transport_policy_override.as_ref(),
            "--transport-policy-override",
        )?;
        let anonymity_policy_override = parse_anonymity_policy_flag(
            self.anonymity_policy_override.as_ref(),
            "--anonymity-policy-override",
        )?;
        let policy_override =
            PolicyOverride::new(transport_policy_override, anonymity_policy_override);
        let chunk_profile = chunker_registry::lookup(manifest.chunking.profile_id).map_or_else(
            || ChunkProfile {
                min_size: manifest.chunking.min_size as usize,
                target_size: manifest.chunking.target_size as usize,
                max_size: manifest.chunking.max_size as usize,
                break_mask: u64::from(manifest.chunking.break_mask),
            },
            |descriptor| descriptor.profile,
        );
        let chunks: Vec<CarChunk> = chunk_specs
            .iter()
            .map(|spec| CarChunk {
                offset: spec.offset,
                length: spec.length,
                digest: spec.digest,
            })
            .collect();
        let plan = CarBuildPlan {
            chunk_profile,
            payload_digest,
            content_length,
            chunks,
            files: vec![FilePlan {
                path: Vec::new(),
                first_chunk: 0,
                chunk_count: chunk_specs.len(),
                size: content_length,
            }],
        };
        let chunker_handle = self.chunker_handle.unwrap_or_else(|| {
            format!(
                "{}.{}@{}",
                manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
            )
        });
        let salt_epoch_cli = self.salt_epoch;
        let mut blinded_cid_b64 = self
            .blinded_cid
            .as_ref()
            .map(|value| value.trim().to_string());
        if let Some(value) = blinded_cid_b64.as_ref()
            && value.is_empty()
        {
            return Err(eyre!("--blinded-cid must not be empty"));
        }
        if blinded_cid_b64.is_none()
            && let Some(salt_hex) = self.salt_hex.as_ref()
        {
            let trimmed = salt_hex.trim();
            let decoded =
                hex::decode(trimmed).map_err(|err| eyre!("invalid --salt-hex value: {err}"))?;
            if decoded.len() != 32 {
                return Err(eyre!("--salt-hex must decode to 32 bytes"));
            }
            let mut salt = [0u8; 32];
            salt.copy_from_slice(&decoded);
            let blinded = canonical_cache_key(&salt, manifest.root_cid.as_slice());
            blinded_cid_b64 = Some(URL_SAFE_NO_PAD.encode(blinded.as_bytes()));
        }
        let salt_epoch = match (blinded_cid_b64.as_ref(), salt_epoch_cli) {
            (Some(_), Some(epoch)) => Some(epoch),
            (Some(_), None) => {
                return Err(eyre!(
                    "--salt-epoch must be supplied when providing --blinded-cid or --salt-hex"
                ));
            }
            (None, Some(_)) => {
                return Err(eyre!(
                    "--salt-epoch requires --blinded-cid or --salt-hex to compute the header"
                ));
            }
            (None, None) => None,
        };
        let manifest_envelope_b64 = match self.manifest_envelope.as_ref() {
            Some(path) => Some(load_manifest_envelope(path)?),
            None => None,
        };
        let guard_cache_path = self.guard_cache.clone();
        let mut guard_set = if let Some(path) = guard_cache_path.as_ref() {
            load_guard_set(path, guard_cache_key.as_ref())
                .wrap_err_with(|| format!("failed to load guard cache from `{}`", path.display()))?
        } else {
            None
        };
        let mut guard_updated = false;
        let relay_directory = if let Some(directory_path) = self.guard_directory.as_ref() {
            let expected_digest = self
                .guard_directory_digest
                .as_deref()
                .expect("checked guard directory digest above");
            let now_unix = OffsetDateTime::now_utc().unix_timestamp();
            let directory = load_guard_directory(directory_path, expected_digest, now_unix)
                .wrap_err_with(|| {
                    format!(
                        "failed to parse guard directory from `{}`",
                        directory_path.display()
                    )
                })?;
            let target = self.guard_target.unwrap_or(3);
            if target == 0 {
                return Err(eyre!("--guard-target must be at least 1 when provided"));
            }
            let retention_days = self.guard_retention_days.unwrap_or(30);
            if retention_days == 0 {
                return Err(eyre!(
                    "--guard-retention-days must be at least 1 when provided"
                ));
            }
            let retention_secs = retention_days.saturating_mul(24 * 60 * 60);
            let retention = GuardRetention::new(
                NonZeroU64::new(retention_secs)
                    .ok_or_else(|| eyre!("guard retention window must be at least one second"))?,
            );
            let selector = GuardSelector::new(
                NonZeroUsize::new(target)
                    .ok_or_else(|| eyre!("guard target must be at least 1 when provided"))?,
            )
            .with_retention(retention);
            let now_unix = u64::try_from(now_unix).unwrap_or(0);
            let policy = anonymity_policy.unwrap_or(AnonymityPolicy::GuardPq);
            let selected = selector
                .select(&directory, guard_set.as_ref(), now_unix, policy)
                .wrap_err("guard directory is not active at the selection timestamp")?;
            guard_set = Some(selected);
            guard_updated = true;
            Some(directory)
        } else {
            None
        };
        let mut provider_inputs = Vec::with_capacity(self.gateway_provider.len());
        let mut provider_aliases = Vec::with_capacity(self.gateway_provider.len());
        let mut provider_label_by_id = HashMap::with_capacity(self.gateway_provider.len());
        for spec in &self.gateway_provider {
            let parsed = parse_gateway_provider_spec(spec)?;
            provider_label_by_id.insert(
                parsed.provider_id_hex.to_ascii_lowercase(),
                parsed.name.clone(),
            );
            provider_aliases.push(parsed.name.clone());
            provider_inputs.push(GatewayProviderInput {
                name: parsed.name,
                provider_id_hex: parsed.provider_id_hex,
                gateway_public_key_hex: parsed.gateway_public_key_hex,
                base_url: parsed.base_url,
                stream_token_b64: parsed.stream_token_b64,
                privacy_events_url: parsed.privacy_events_url,
            });
        }
        let gateway_config = GatewayFetchConfig {
            manifest_id_hex: manifest_id_hex.clone(),
            chunker_handle,
            manifest_envelope_b64: manifest_envelope_b64.clone(),
            client_id: self.client_id.clone(),
            expected_manifest_cid_hex: Some(cid_hex_override.clone()),
            blinded_cid_b64,
            salt_epoch,
            expected_cache_version: None,
        };
        let telemetry_source_label = self
            .telemetry_source_label
            .as_ref()
            .map(|label| {
                let trimmed = label.trim();
                if trimmed.is_empty() {
                    Err(eyre!(
                        "--telemetry-source-label must not be empty when provided"
                    ))
                } else {
                    Ok(trimmed.to_string())
                }
            })
            .transpose()?;
        let telemetry_region_label = self
            .telemetry_region
            .as_ref()
            .map(|label| {
                let trimmed = label.trim();
                if trimmed.is_empty() {
                    Err(eyre!("--telemetry-region must not be empty when provided"))
                } else {
                    Ok(trimmed.to_string())
                }
            })
            .transpose()?;
        let gateway_provider_count = provider_inputs.len();
        let capture_paths =
            scoreboard_capture_paths(self.scoreboard_out.clone(), self.json_out.clone());
        let write_mode_hint = write_mode.unwrap_or(WriteModeHint::ReadOnly);
        let mut scoreboard_options = SorafsGatewayScoreboardOptions::default();
        if let Some(parent) = capture_paths
            .scoreboard
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!(
                    "failed to create scoreboard directory `{}`",
                    parent.display()
                )
            })?;
        }
        scoreboard_options.persist_path = Some(capture_paths.scoreboard.clone());
        if let Some(now) = self.scoreboard_now {
            scoreboard_options.now_unix_secs = Some(now);
        }
        let provider_counts = ProviderCounts::new(0, gateway_provider_count);
        let metadata = cli_scoreboard_metadata(&ScoreboardMetadataInput {
            provider_counts,
            max_peers: self.max_peers,
            retry_budget: self.retry_budget,
            manifest_envelope_present: manifest_envelope_b64.is_some(),
            gateway_manifest_id: Some(manifest_id_hex.clone()),
            gateway_manifest_cid: Some(cid_hex_override.clone()),
            transport_policy,
            transport_policy_override,
            anonymity_policy,
            anonymity_policy_override,
            write_mode: write_mode_hint,
            scoreboard_now: self.scoreboard_now,
            telemetry_source: telemetry_source_label.clone(),
            telemetry_region: telemetry_region_label.clone(),
        });
        scoreboard_options.metadata = Some(metadata);
        scoreboard_options
            .telemetry_source_label
            .clone_from(&telemetry_source_label);
        let scoreboard_options = Some(scoreboard_options);
        let fetch_options = SorafsGatewayFetchOptions {
            retry_budget: self.retry_budget,
            max_peers: self.max_peers,
            telemetry_region: telemetry_region_label.clone(),
            transport_policy,
            anonymity_policy,
            guard_set: guard_set.clone(),
            relay_directory,
            write_mode_hint: Some(write_mode_hint),
            policy_override,
            scoreboard: scoreboard_options,
            expected_cache_version: gateway_config.expected_cache_version.clone(),
        };
        let client = context.client_from_config();
        let runtime = Runtime::new().wrap_err("failed to create Tokio runtime")?;
        let session = runtime
            .block_on(client.sorafs_fetch_via_gateway(
                &plan,
                gateway_config,
                provider_inputs,
                fetch_options,
            ))
            .map_err(|err| eyre!("SoraFS fetch failed: {err}"))?;
        let outcome = &session.outcome;
        let policy_report = &session.policy_report;
        let assembled = outcome.assemble_payload();
        let computed_digest = blake3::hash(&assembled);
        if computed_digest.as_bytes() != payload_digest.as_bytes() {
            return Err(eyre!(
                "assembled payload digest {} did not match expected payload digest {}",
                hex::encode(computed_digest.as_bytes()),
                payload_digest_hex
            ));
        }
        if guard_updated
            && let (Some(path), Some(guard_state)) = (guard_cache_path.as_ref(), guard_set.as_ref())
        {
            persist_guard_set(path, guard_state, guard_cache_key.as_ref()).wrap_err_with(|| {
                format!("failed to persist guard cache to `{}`", path.display())
            })?;
        }
        let policy = policy_report.policy;
        let soranet_selected = policy_report.selected_soranet_total as u64;
        let pq_selected = policy_report.selected_pq as u64;
        let classical_selected = policy_report.selected_classical() as u64;
        let pq_ratio = policy_report.pq_ratio();
        if let Some(path) = &self.output {
            fs::write(path, &assembled)
                .wrap_err_with(|| format!("failed to write payload to `{}`", path.display()))?;
        }
        let provider_reports_json: Vec<norito::json::Value> = outcome
            .provider_reports
            .iter()
            .map(|report| {
                let provider_id = report.provider.id().as_str();
                let alias = provider_label_by_id
                    .get(&provider_id.to_ascii_lowercase())
                    .cloned()
                    .unwrap_or_else(|| provider_id.to_string());
                let mut map = norito::json::Map::new();
                map.insert("provider_id".into(), norito::json::Value::from(provider_id));
                map.insert("alias".into(), norito::json::Value::from(alias));
                map.insert(
                    "successes".into(),
                    norito::json::Value::from(report.successes as u64),
                );
                map.insert(
                    "failures".into(),
                    norito::json::Value::from(report.failures as u64),
                );
                map.insert(
                    "disabled".into(),
                    norito::json::Value::from(report.disabled),
                );
                norito::json::Value::Object(map)
            })
            .collect();
        let chunk_receipts_json: Vec<norito::json::Value> = outcome
            .chunk_receipts
            .iter()
            .map(|receipt| {
                let provider_id_lower = receipt.provider.as_str().to_ascii_lowercase();
                let alias = provider_label_by_id
                    .get(&provider_id_lower)
                    .cloned()
                    .unwrap_or_else(|| receipt.provider.as_str().to_string());
                let mut map = norito::json::Map::new();
                map.insert(
                    "chunk_index".into(),
                    norito::json::Value::from(receipt.chunk_index as u64),
                );
                map.insert(
                    "provider_id".into(),
                    norito::json::Value::from(receipt.provider.as_str()),
                );
                map.insert("alias".into(), norito::json::Value::from(alias));
                map.insert(
                    "attempts".into(),
                    norito::json::Value::from(receipt.attempts as u64),
                );
                map.insert(
                    "latency_ms".into(),
                    norito::json::Value::from(receipt.latency_ms),
                );
                map.insert(
                    "bytes".into(),
                    norito::json::Value::from(u64::from(receipt.bytes)),
                );
                norito::json::Value::Object(map)
            })
            .collect();
        let mut summary = norito::json::Map::new();
        summary.insert(
            "manifest_id".into(),
            norito::json::Value::from(manifest_id_hex),
        );
        summary.insert(
            "manifest_cid".into(),
            norito::json::Value::from(cid_hex_override),
        );
        summary.insert(
            "chunk_count".into(),
            norito::json::Value::from(outcome.chunks.len() as u64),
        );
        summary.insert(
            "fetched_bytes".into(),
            norito::json::Value::from(assembled.len() as u64),
        );
        insert_provider_counts(&mut summary, provider_counts);
        insert_transport_policy(&mut summary, transport_policy, transport_policy_override);
        summary.insert(
            "gateway_manifest_provided".into(),
            norito::json::Value::from(manifest_envelope_b64.is_some()),
        );
        summary.insert(
            "guard_cache_tagged".into(),
            norito::json::Value::from(guard_cache_key.is_some()),
        );
        summary.insert(
            "providers".into(),
            norito::json::Value::Array(
                provider_aliases
                    .iter()
                    .cloned()
                    .map(norito::json::Value::from)
                    .collect(),
            ),
        );
        summary.insert(
            "provider_reports".into(),
            norito::json::Value::Array(provider_reports_json),
        );
        summary.insert(
            "chunk_receipts".into(),
            norito::json::Value::Array(chunk_receipts_json),
        );
        if let Some(manifest) = &session.local_proxy_manifest {
            summary.insert(
                "local_proxy_manifest".into(),
                public_local_proxy_manifest_value(manifest),
            );
        }
        if let Some(budget) = self.retry_budget {
            summary.insert(
                "retry_budget".into(),
                norito::json::Value::from(budget as u64),
            );
        }
        if let Some(peers) = self.max_peers {
            summary.insert("max_peers".into(), norito::json::Value::from(peers as u64));
        }
        if let Some(client_id) = &self.client_id {
            summary.insert(
                "client_id".into(),
                norito::json::Value::from(client_id.clone()),
            );
        }
        insert_summary_telemetry_source(&mut summary, telemetry_source_label.as_deref());
        insert_summary_telemetry_region(&mut summary, telemetry_region_label.as_deref());
        summary.insert(
            "anonymity_policy".into(),
            norito::json::Value::from(anonymity_policy_label(policy).to_string()),
        );
        summary.insert(
            "anonymity_status".into(),
            norito::json::Value::from(policy_report.status_label()),
        );
        summary.insert(
            "anonymity_reason".into(),
            norito::json::Value::from(policy_report.reason_label()),
        );
        summary.insert(
            "anonymity_soranet_selected".into(),
            norito::json::Value::from(soranet_selected),
        );
        summary.insert(
            "anonymity_pq_selected".into(),
            norito::json::Value::from(pq_selected),
        );
        summary.insert(
            "anonymity_classical_selected".into(),
            norito::json::Value::from(classical_selected),
        );
        summary.insert(
            "anonymity_classical_ratio".into(),
            norito::json::Value::from(policy_report.classical_ratio()),
        );
        summary.insert(
            "anonymity_pq_ratio".into(),
            norito::json::Value::from(pq_ratio),
        );
        summary.insert(
            "anonymity_candidate_ratio".into(),
            norito::json::Value::from(policy_report.candidate_ratio()),
        );
        summary.insert(
            "anonymity_deficit_ratio".into(),
            norito::json::Value::from(policy_report.deficit_ratio()),
        );
        summary.insert(
            "anonymity_supply_delta".into(),
            norito::json::Value::from(policy_report.supply_delta_ratio()),
        );
        summary.insert(
            "anonymity_brownout".into(),
            norito::json::Value::from(policy_report.is_brownout()),
        );
        summary.insert(
            "anonymity_brownout_effective".into(),
            norito::json::Value::from(policy_report.should_flag_brownout()),
        );
        summary.insert(
            "anonymity_uses_classical".into(),
            norito::json::Value::from(policy_report.uses_classical()),
        );
        let summary_value = norito::json::Value::Object(summary);
        if let Some(path) = capture_paths.summary.as_ref() {
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                fs::create_dir_all(parent).wrap_err_with(|| {
                    format!("failed to create summary directory `{}`", parent.display())
                })?;
            }
            let rendered = norito::json::to_string_pretty(&summary_value)?;
            fs::write(path, rendered.as_bytes())
                .wrap_err_with(|| format!("failed to write summary to `{}`", path.display()))?;
        }
        context.print_data(&summary_value)
    }
}
#[derive(Debug)]
struct ParsedGatewayProvider {
    name: String,
    provider_id_hex: String,
    gateway_public_key_hex: String,
    base_url: String,
    stream_token_b64: String,
    privacy_events_url: Option<String>,
}
fn parse_gateway_provider_spec(value: &str) -> Result<ParsedGatewayProvider> {
    let mut name: Option<String> = None;
    let mut provider_id: Option<String> = None;
    let mut gateway_public_key: Option<String> = None;
    let mut base_url: Option<String> = None;
    let mut stream_token: Option<String> = None;
    let mut privacy_events_url: Option<String> = None;
    for pair in value.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let (key, val) = pair.split_once('=').ok_or_else(|| {
            eyre!("--gateway-provider expects comma-separated key=value pairs (got `{value}`)")
        })?;
        let val = val.trim();
        match key {
            "name" => {
                if val.is_empty() {
                    return Err(eyre!("--gateway-provider name must not be empty"));
                }
                name = Some(val.to_string());
            }
            "provider-id" | "provider_id" => {
                let normalised = validate_hex_digest(val, "--gateway-provider provider-id")?;
                provider_id = Some(normalised);
            }
            "gateway-key" | "gateway_key" | "gateway-public-key" | "gateway_public_key" => {
                let normalised = validate_hex_digest(val, "--gateway-provider gateway-key")?;
                gateway_public_key = Some(normalised);
            }
            "base-url" | "base_url" => {
                if val.is_empty() {
                    return Err(eyre!("--gateway-provider base-url must not be empty"));
                }
                base_url = Some(val.to_string());
            }
            "stream-token" | "stream_token" => {
                if val.is_empty() {
                    return Err(eyre!("--gateway-provider stream-token must not be empty"));
                }
                stream_token = Some(val.to_string());
            }
            "privacy-url" | "privacy_url" => {
                if val.is_empty() {
                    return Err(eyre!("--gateway-provider privacy-url must not be empty"));
                }
                privacy_events_url = Some(val.to_string());
            }
            other => {
                return Err(eyre!(
                    "unknown --gateway-provider key `{other}`; expected name, provider-id, gateway-key, base-url, stream-token, privacy-url"
                ));
            }
        }
    }
    let name = name.ok_or_else(|| eyre!("--gateway-provider requires name=<alias>"))?;
    let provider_id_hex =
        provider_id.ok_or_else(|| eyre!("--gateway-provider requires provider-id=<hex>"))?;
    let gateway_public_key_hex =
        gateway_public_key.ok_or_else(|| eyre!("--gateway-provider requires gateway-key=<hex>"))?;
    let base_url =
        base_url.ok_or_else(|| eyre!("--gateway-provider requires base-url=<https://...>"))?;
    let stream_token_b64 =
        stream_token.ok_or_else(|| eyre!("--gateway-provider requires stream-token=<base64>"))?;
    Ok(ParsedGatewayProvider {
        name,
        provider_id_hex,
        gateway_public_key_hex,
        base_url,
        stream_token_b64,
        privacy_events_url,
    })
}
fn option_usize_to_json_value(value: Option<usize>) -> Value {
    value
        .and_then(|val| u64::try_from(val).ok())
        .map_or(Value::Null, Value::from)
}
fn transport_policy_labels(
    requested: Option<TransportPolicy>,
    override_policy: Option<TransportPolicy>,
) -> (&'static str, bool, Option<&'static str>) {
    let override_flag = override_policy.is_some();
    let override_label = override_policy.map(TransportPolicy::label);
    let effective = override_policy.unwrap_or_else(|| requested.unwrap_or_default());
    (effective.label(), override_flag, override_label)
}
fn anonymity_policy_labels(
    requested: Option<AnonymityPolicy>,
    override_policy: Option<AnonymityPolicy>,
) -> (&'static str, bool, Option<&'static str>) {
    let override_flag = override_policy.is_some();
    let override_label = override_policy.map(AnonymityPolicy::label);
    let effective = override_policy.unwrap_or_else(|| requested.unwrap_or_default());
    (effective.label(), override_flag, override_label)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProviderCounts {
    direct: usize,
    gateway: usize,
}
impl ProviderCounts {
    const fn new(direct: usize, gateway: usize) -> Self {
        Self { direct, gateway }
    }
    fn direct_u64(self) -> u64 {
        u64::try_from(self.direct).unwrap_or(u64::MAX)
    }
    fn gateway_u64(self) -> u64 {
        u64::try_from(self.gateway).unwrap_or(u64::MAX)
    }
    fn mix_label(self) -> &'static str {
        match (self.direct > 0, self.gateway > 0) {
            (true, true) => "mixed",
            (true, false) => "direct-only",
            (false, true) => "gateway-only",
            (false, false) => "none",
        }
    }
}
#[derive(Clone)]
struct ScoreboardMetadataInput {
    provider_counts: ProviderCounts,
    max_peers: Option<usize>,
    retry_budget: Option<usize>,
    manifest_envelope_present: bool,
    gateway_manifest_id: Option<String>,
    gateway_manifest_cid: Option<String>,
    transport_policy: Option<TransportPolicy>,
    transport_policy_override: Option<TransportPolicy>,
    anonymity_policy: Option<AnonymityPolicy>,
    anonymity_policy_override: Option<AnonymityPolicy>,
    write_mode: WriteModeHint,
    scoreboard_now: Option<u64>,
    telemetry_source: Option<String>,
    telemetry_region: Option<String>,
}
fn cli_scoreboard_metadata(input: &ScoreboardMetadataInput) -> Value {
    let mut metadata = Map::new();
    metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));
    metadata.insert("use_scoreboard".into(), Value::from(true));
    metadata.insert("allow_implicit_metadata".into(), Value::from(false));
    metadata.insert(
        "provider_count".into(),
        Value::from(input.provider_counts.direct_u64()),
    );
    metadata.insert(
        "gateway_provider_count".into(),
        Value::from(input.provider_counts.gateway_u64()),
    );
    metadata.insert(
        "provider_mix".into(),
        Value::from(input.provider_counts.mix_label()),
    );
    metadata.insert("max_parallel".into(), Value::Null);
    metadata.insert(
        "max_peers".into(),
        option_usize_to_json_value(input.max_peers),
    );
    metadata.insert(
        "retry_budget".into(),
        option_usize_to_json_value(input.retry_budget),
    );
    metadata.insert("provider_failure_threshold".into(), Value::Null);
    metadata.insert(
        "assume_now".into(),
        input.scoreboard_now.map_or(Value::Null, Value::from),
    );
    metadata.insert(
        "telemetry_source".into(),
        input
            .telemetry_source
            .as_ref()
            .map_or(Value::Null, |label| Value::from(label.as_str())),
    );
    metadata.insert(
        "telemetry_region".into(),
        input
            .telemetry_region
            .as_ref()
            .map_or(Value::Null, |label| Value::from(label.as_str())),
    );
    metadata.insert(
        "gateway_manifest_id".into(),
        input
            .gateway_manifest_id
            .as_deref()
            .map_or(Value::Null, Value::from),
    );
    metadata.insert(
        "gateway_manifest_cid".into(),
        input
            .gateway_manifest_cid
            .as_deref()
            .map_or(Value::Null, Value::from),
    );
    metadata.insert(
        "gateway_manifest_provided".into(),
        Value::from(input.manifest_envelope_present),
    );
    let (transport_label, transport_override_flag, transport_override_label) =
        transport_policy_labels(input.transport_policy, input.transport_policy_override);
    metadata.insert("transport_policy".into(), Value::from(transport_label));
    metadata.insert(
        "transport_policy_override".into(),
        Value::from(transport_override_flag),
    );
    metadata.insert(
        "transport_policy_override_label".into(),
        transport_override_label.map_or(Value::Null, Value::from),
    );
    let (anonymity_label, anonymity_override_flag, anonymity_override_label) =
        anonymity_policy_labels(input.anonymity_policy, input.anonymity_policy_override);
    metadata.insert("anonymity_policy".into(), Value::from(anonymity_label));
    metadata.insert(
        "anonymity_policy_override".into(),
        Value::from(anonymity_override_flag),
    );
    metadata.insert(
        "anonymity_policy_override_label".into(),
        anonymity_override_label.map_or(Value::Null, Value::from),
    );
    let write_mode_label = input.write_mode.label().replace('_', "-");
    metadata.insert("write_mode".into(), Value::from(write_mode_label));
    metadata.insert(
        "write_mode_enforces_pq".into(),
        Value::from(input.write_mode.enforces_pq_only()),
    );
    Value::Object(metadata)
}
fn load_guard_cache_key_file(path: &Path) -> Result<GuardCacheKey> {
    let bytes = read_owner_private_handshake_file(
        path,
        GuardCacheKey::LENGTH,
        Some(GuardCacheKey::LENGTH),
        "guard cache authentication key",
    )?;
    let mut key_bytes = [0_u8; GuardCacheKey::LENGTH];
    key_bytes.copy_from_slice(bytes.as_slice());
    let key = GuardCacheKey::from_bytes(key_bytes);
    key_bytes.zeroize();
    key.map_err(|error| {
        eyre!(
            "invalid guard cache authentication key file `{}`: {error}",
            path.display()
        )
    })
}
fn load_guard_set(path: &Path, key: Option<&GuardCacheKey>) -> Result<Option<GuardSet>> {
    let key = key.ok_or_else(|| eyre!("guard cache authentication key is required"))?;
    let named_metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .wrap_err_with(|| format!("failed to inspect guard cache `{}`", path.display()));
        }
    };
    if named_metadata.file_type().is_symlink() {
        return Err(eyre!(
            "guard cache `{}` must be a direct owner-private file",
            path.display()
        ));
    }
    let direct_path = canonical_guard_cache_path(path, false)?;
    let bytes = read_owner_private_handshake_file(
        &direct_path,
        GUARD_CACHE_MAX_BYTES_V1,
        None,
        "guard cache",
    )?;
    let guard_set = GuardSet::decode_authenticated(&bytes, key).map_err(|err| {
        eyre!(
            "failed to decode guard cache from `{}`: {err}",
            path.display()
        )
    })?;
    Ok(Some(guard_set))
}
fn persist_guard_set(path: &Path, guard_set: &GuardSet, key: Option<&GuardCacheKey>) -> Result<()> {
    let key = key.ok_or_else(|| eyre!("guard cache authentication key is required"))?;
    let payload = guard_set
        .encode_authenticated(key)
        .map_err(|err| eyre!("failed to encode guard cache: {err}"))?;
    if payload.is_empty() || payload.len() > GUARD_CACHE_MAX_BYTES_V1 {
        return Err(eyre!(
            "encoded guard cache must contain between 1 and {GUARD_CACHE_MAX_BYTES_V1} bytes"
        ));
    }
    persist_owner_private_guard_cache(path, &payload)
}
#[cfg(unix)]
fn canonical_guard_cache_path(path: &Path, create_parent: bool) -> Result<PathBuf> {
    let file_name = match path.components().next_back() {
        Some(std::path::Component::Normal(file_name)) => file_name,
        _ => {
            return Err(eyre!(
                "guard cache path `{}` must name a regular file",
                path.display()
            ));
        }
    };
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if create_parent {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!(
                "failed to create guard cache directory `{}`",
                parent.display()
            )
        })?;
    }
    let canonical_parent = fs::canonicalize(parent).wrap_err_with(|| {
        format!(
            "failed to canonicalize guard cache directory `{}`",
            parent.display()
        )
    })?;
    validate_guard_cache_parent_chain(&canonical_parent)?;
    Ok(canonical_parent.join(file_name))
}
#[cfg(not(unix))]
fn canonical_guard_cache_path(path: &Path, _create_parent: bool) -> Result<PathBuf> {
    Err(eyre!(
        "guard cache `{}` is unsupported because this platform does not expose the required owner/mode/link custody checks",
        path.display()
    ))
}
#[cfg(unix)]
fn validate_guard_cache_parent_chain(parent: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;

    let effective_uid = rustix::process::geteuid().as_raw();
    let mut ancestors = parent
        .ancestors()
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    ancestors.reverse();
    let mut metadata = Vec::with_capacity(ancestors.len());
    for ancestor in &ancestors {
        let observed = fs::symlink_metadata(ancestor).wrap_err_with(|| {
            format!(
                "failed to inspect guard cache directory ancestor `{}`",
                ancestor.display()
            )
        })?;
        if observed.file_type().is_symlink() || !observed.is_dir() {
            return Err(eyre!(
                "guard cache directory ancestor `{}` must be a direct directory",
                ancestor.display()
            ));
        }
        if observed.uid() != 0 && observed.uid() != effective_uid {
            return Err(eyre!(
                "guard cache directory ancestor `{}` must be owned by root or effective UID {effective_uid}",
                ancestor.display()
            ));
        }
        metadata.push(observed);
    }
    for (index, observed) in metadata.iter().enumerate() {
        if observed.mode() & 0o022 == 0 {
            continue;
        }
        let protected_sticky_boundary = observed.uid() == 0
            && observed.mode() & 0o1000 != 0
            && metadata
                .get(index + 1)
                .is_some_and(|child| child.uid() == effective_uid && child.mode() & 0o022 == 0);
        if !protected_sticky_boundary {
            return Err(eyre!(
                "guard cache directory ancestor `{}` is writable by another principal",
                ancestors[index].display()
            ));
        }
    }
    let parent_metadata = metadata
        .last()
        .ok_or_else(|| eyre!("guard cache path has no parent directory metadata"))?;
    if parent_metadata.uid() != effective_uid || parent_metadata.mode() & 0o022 != 0 {
        return Err(eyre!(
            "guard cache directory `{}` must be owned by effective UID {effective_uid} and not be group/world writable",
            parent.display()
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn validate_guard_cache_destination(metadata: &fs::Metadata, path: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;

    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1
    {
        return Err(eyre!(
            "guard cache `{}` must be an owner-private regular non-symlink file with exactly one link",
            path.display()
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn same_guard_cache_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.uid() == right.uid()
        && left.mode() == right.mode()
        && left.nlink() == 1
        && right.nlink() == 1
        && left.len() == right.len()
}
#[cfg(unix)]
fn persist_owner_private_guard_cache(path: &Path, payload: &[u8]) -> Result<()> {
    let direct_path = canonical_guard_cache_path(path, true)?;
    match fs::symlink_metadata(&direct_path) {
        Ok(metadata) => validate_guard_cache_destination(&metadata, &direct_path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).wrap_err_with(|| {
                format!(
                    "failed to inspect existing guard cache `{}`",
                    direct_path.display()
                )
            });
        }
    }

    let parent = direct_path
        .parent()
        .ok_or_else(|| eyre!("guard cache path has no parent directory"))?;
    let mut nonce = [0_u8; 16];
    OsRng
        .try_fill_bytes(&mut nonce)
        .map_err(|error| eyre!("failed to generate guard cache staging name: {error}"))?;
    let staging_path = parent.join(format!(".guard-cache-{}.tmp", hex::encode(nonce)));
    nonce.zeroize();
    let descriptor = rustix::fs::open(
        &staging_path,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .wrap_err_with(|| {
        format!(
            "failed to create owner-private guard cache staging file `{}`",
            staging_path.display()
        )
    })?;
    let mut staging = fs::File::from(descriptor);
    let write_result = (|| -> Result<()> {
        rustix::fs::fchmod(&staging, rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR)
            .wrap_err_with(|| {
                format!(
                    "failed to enforce owner-private guard cache staging permissions `{}`",
                    staging_path.display()
                )
            })?;
        let created_metadata = staging.metadata().wrap_err_with(|| {
            format!(
                "failed to inspect guard cache staging file `{}`",
                staging_path.display()
            )
        })?;
        validate_guard_cache_destination(&created_metadata, &staging_path)?;
        staging.write_all(payload).wrap_err_with(|| {
            format!(
                "failed to write guard cache staging file `{}`",
                staging_path.display()
            )
        })?;
        staging.sync_all().wrap_err_with(|| {
            format!(
                "failed to sync guard cache staging file `{}`",
                staging_path.display()
            )
        })?;
        let staged_metadata = staging.metadata().wrap_err_with(|| {
            format!(
                "failed to re-inspect guard cache staging file `{}`",
                staging_path.display()
            )
        })?;
        validate_guard_cache_destination(&staged_metadata, &staging_path)?;
        if staged_metadata.len() != u64::try_from(payload.len()).unwrap_or(u64::MAX) {
            return Err(eyre!(
                "guard cache staging file `{}` changed while it was written",
                staging_path.display()
            ));
        }
        fs::rename(&staging_path, &direct_path).wrap_err_with(|| {
            format!(
                "failed to atomically replace guard cache `{}`",
                direct_path.display()
            )
        })?;
        let published_metadata = fs::symlink_metadata(&direct_path).wrap_err_with(|| {
            format!(
                "failed to inspect published guard cache `{}`",
                direct_path.display()
            )
        })?;
        validate_guard_cache_destination(&published_metadata, &direct_path)?;
        if !same_guard_cache_file_identity(&staged_metadata, &published_metadata) {
            return Err(eyre!(
                "published guard cache `{}` does not match the staged file",
                direct_path.display()
            ));
        }
        Ok(())
    })();
    if write_result.is_err() {
        drop(staging);
        let _ = fs::remove_file(&staging_path);
    }
    write_result
}
#[cfg(not(unix))]
fn persist_owner_private_guard_cache(path: &Path, _payload: &[u8]) -> Result<()> {
    Err(eyre!(
        "guard cache `{}` is unsupported because this platform does not expose the required owner/mode/link custody checks",
        path.display()
    ))
}
fn load_guard_directory(
    path: &Path,
    expected_snapshot_digest_hex: &str,
    at_unix: i64,
) -> Result<RelayDirectory> {
    let bytes = read_guard_directory_snapshot_file(path)
        .wrap_err_with(|| format!("failed to read guard directory from `{}`", path.display()))?;
    let expected_digest = parse_snapshot_digest_hex(expected_snapshot_digest_hex)?;
    RelayDirectory::from_guard_directory_bytes_at(&bytes, expected_digest, at_unix).map_err(|err| {
        eyre!(
            "failed to authenticate guard directory from `{}`: {err} (expected pinned SRCv2 Norito snapshot)",
            path.display(),
        )
    })
}
#[derive(Debug, Clone, norito::json::JsonSerialize)]
struct GuardDirectorySummary {
    version: u8,
    snapshot_digest_hex: String,
    authentication: &'static str,
    directory_hash_hex: Option<String>,
    published_at_unix: Option<i64>,
    valid_after_unix: Option<i64>,
    valid_until_unix: Option<i64>,
    issuer_count: usize,
    relay_count: usize,
    entry_guards: usize,
    entry_guards_pq: usize,
    entry_guard_pq_ratio: f64,
    exit_relays: usize,
    pq_handshake_relays: usize,
    snapshot_size_bytes: usize,
}
impl GuardDirectorySummary {
    fn from_components(
        snapshot: &GuardDirectorySnapshotV2,
        directory: &RelayDirectory,
        snapshot_size_bytes: usize,
        snapshot_digest_hex: String,
        authenticated: bool,
    ) -> Self {
        let mut entry_guards = 0usize;
        let mut pq_entry_guards = 0usize;
        let mut exit_relays = 0usize;
        let mut pq_handshake_relays = 0usize;
        for descriptor in directory.entries() {
            if descriptor.is_entry_guard() {
                entry_guards += 1;
                if descriptor.is_pq_capable() {
                    pq_entry_guards += 1;
                }
            }
            if descriptor.roles.exit() {
                exit_relays += 1;
            }
            if descriptor.is_pq_capable() {
                pq_handshake_relays += 1;
            }
        }
        #[allow(clippy::cast_precision_loss)]
        let pq_ratio = if entry_guards == 0 {
            0.0
        } else {
            pq_entry_guards as f64 / entry_guards as f64
        };
        Self {
            version: snapshot.version,
            snapshot_digest_hex,
            authentication: if authenticated {
                "authenticated"
            } else {
                "structural_inspection_only"
            },
            directory_hash_hex: directory.directory_hash().map(hex::encode),
            published_at_unix: directory.published_at(),
            valid_after_unix: directory.valid_after(),
            valid_until_unix: directory.valid_until(),
            issuer_count: snapshot.issuers.len(),
            relay_count: directory.entries().len(),
            entry_guards,
            entry_guards_pq: pq_entry_guards,
            entry_guard_pq_ratio: pq_ratio,
            exit_relays,
            pq_handshake_relays,
            snapshot_size_bytes,
        }
    }
}
fn inspect_guard_directory_bytes(bytes: &[u8]) -> Result<GuardDirectorySummary> {
    let snapshot = GuardDirectorySnapshotV2::inspect_bytes(bytes)
        .wrap_err("failed to decode guard directory snapshot")?;
    let directory = RelayDirectory::inspect_guard_directory_bytes(bytes)
        .wrap_err("guard directory structural inspection failed")?;
    Ok(GuardDirectorySummary::from_components(
        &snapshot,
        &directory,
        bytes.len(),
        hex::encode(compute_snapshot_digest(bytes)),
        false,
    ))
}
fn authenticate_guard_directory_bytes(
    bytes: &[u8],
    expected_snapshot_digest_hex: &str,
    at_unix: i64,
) -> Result<GuardDirectorySummary> {
    let expected_digest = parse_snapshot_digest_hex(expected_snapshot_digest_hex)?;
    let snapshot = GuardDirectorySnapshotV2::authenticate_bytes_at(bytes, expected_digest, at_unix)
        .wrap_err("failed to authenticate guard directory snapshot")?;
    let directory = RelayDirectory::from_guard_directory_bytes_at(bytes, expected_digest, at_unix)
        .wrap_err("guard directory authentication failed")?;
    Ok(GuardDirectorySummary::from_components(
        &snapshot,
        &directory,
        bytes.len(),
        hex::encode(expected_digest),
        true,
    ))
}
fn parse_snapshot_digest_hex(value: &str) -> Result<[u8; 32]> {
    let trimmed = value.trim();
    if trimmed.len() != 64 {
        return Err(eyre!(
            "snapshot digest must contain 64 hex characters (got length {})",
            trimmed.len()
        ));
    }
    if !trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(eyre!(
            "snapshot digest `{trimmed}` must only contain hexadecimal characters"
        ));
    }
    let decoded = hex::decode(trimmed).wrap_err("failed to decode snapshot digest")?;
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&decoded);
    Ok(digest)
}
fn write_guard_directory_snapshot(path: &Path, bytes: &[u8], overwrite: bool) -> Result<()> {
    if path.exists() && !overwrite {
        return Err(eyre!(
            "refusing to overwrite existing guard directory snapshot `{}` (pass --overwrite to replace)",
            path.display()
        ));
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!(
                "failed to create parent directory `{}` for guard directory snapshot",
                parent.display()
            )
        })?;
    }
    fs::write(path, bytes).wrap_err_with(|| {
        format!(
            "failed to write guard directory snapshot to `{}`",
            path.display()
        )
    })
}
fn parse_transport_policy_flag(
    value: Option<&String>,
    flag: &'static str,
) -> Result<Option<TransportPolicy>> {
    if let Some(raw) = value {
        if raw.is_empty() {
            return Err(eyre!("{flag} must not be empty"));
        }
        TransportPolicy::parse(raw)
            .ok_or_else(|| {
                eyre!("{flag} must be one of `soranet-first`, `soranet-strict`, or `direct-only`")
            })
            .map(Some)
    } else {
        Ok(None)
    }
}
fn parse_anonymity_policy_flag(
    value: Option<&String>,
    flag: &'static str,
) -> Result<Option<AnonymityPolicy>> {
    if let Some(raw) = value {
        if raw.is_empty() {
            return Err(eyre!("{flag} must not be empty"));
        }
        AnonymityPolicy::parse(raw)
            .ok_or_else(|| {
                eyre!(
                    "{flag} must be one of `anon-guard-pq`, `anon-majority-pq`, or \
                     `anon-strict-pq`"
                )
            })
            .map(Some)
    } else {
        Ok(None)
    }
}
fn parse_write_mode_flag(
    value: Option<&String>,
    flag: &'static str,
) -> Result<Option<WriteModeHint>> {
    if let Some(raw) = value {
        if raw.is_empty() {
            return Err(eyre!("{flag} must not be empty"));
        }
        WriteModeHint::parse(raw)
            .ok_or_else(|| eyre!("{flag} must be one of `read-only` or `upload-pq-only`"))
            .map(Some)
    } else {
        Ok(None)
    }
}
fn validate_hex_digest(value: &str, flag: &str) -> Result<String> {
    if value.len() != 64 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(eyre!("{flag} must be 64 hex characters"));
    }
    Ok(value.to_ascii_lowercase())
}
fn anonymity_policy_label(policy: AnonymityPolicy) -> &'static str {
    match policy {
        AnonymityPolicy::GuardPq => "anon-guard-pq",
        AnonymityPolicy::MajorityPq => "anon-majority-pq",
        AnonymityPolicy::StrictPq => "anon-strict-pq",
    }
}
impl_run_for_subcommand!(#[allow(clippy::too_many_lines)] Command => Pin, Alias, Replication, Storage, Gateway, Incentives, Handshake, Toolkit, GuardDirectory, Reserve, Appeals, Gar, Transparency, Moderation, Repair, Billing, Hedging, Gc, Fetch);
#[derive(clap::Subcommand, Debug)]
pub enum IncentivesCommand {
    /// Compute a relay reward instruction from metrics and bond state.
    Compute(IncentivesComputeArgs),
    /// Open a dispute against an existing reward instruction.
    OpenDispute(IncentivesOpenDisputeArgs),
    /// Summarise reward instructions into an earnings dashboard.
    Dashboard(IncentivesDashboardArgs),
    /// Manage the persistent treasury payout state and disputes.
    #[command(subcommand)]
    Service(IncentivesServiceCommand),
}
impl_run_for_subcommand!(IncentivesCommand => Compute, OpenDispute, Dashboard, Service);
#[derive(clap::Args, Debug)]
pub struct IncentivesComputeArgs {
    /// Path to the reward configuration JSON.
    #[arg(long = "config", value_name = "PATH")]
    pub config: PathBuf,
    /// Norito-encoded relay metrics (`RelayEpochMetricsV1`).
    #[arg(long = "metrics", value_name = "PATH")]
    pub metrics: PathBuf,
    /// Norito-encoded bond ledger entry (`RelayBondLedgerEntryV1`).
    #[arg(long = "bond", value_name = "PATH")]
    pub bond: PathBuf,
    /// Account ID that will receive the payout.
    #[arg(long = "beneficiary", value_name = "ACCOUNT_ID")]
    pub beneficiary: String,
    /// Optional path where the Norito-encoded reward instruction will be written.
    #[arg(long = "norito-out", value_name = "PATH")]
    pub norito_out: Option<PathBuf>,
    /// Emit pretty-printed JSON.
    ///
    /// Ignored when `--output-format json` is used.
    #[arg(long = "pretty", default_value_t = false)]
    pub pretty: bool,
}
impl Run for IncentivesComputeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let config = read_reward_config(&self.config)?;
        let engine = RelayRewardEngine::new(config)
            .map_err(|err| eyre!("invalid reward configuration: {err}"))?;
        let metrics = read_metrics_file(&self.metrics)?;
        let bond = read_bond_entry(&self.bond)?;
        let beneficiary = parse_account_id_str(context, &self.beneficiary, "--beneficiary")?;
        let instruction = engine.compute_reward(&metrics, &bond, beneficiary, Metadata::default());
        if let Some(path) = &self.norito_out {
            write_norito_payload(path, &instruction)?;
        }
        let json_bytes = if self.pretty {
            norito::json::to_vec_pretty(&instruction)?
        } else {
            norito::json::to_vec(&instruction)?
        };
        let output = String::from_utf8(json_bytes)
            .map_err(|err| eyre!("instruction JSON is not valid UTF-8: {err}"))?;
        context.println(output)
    }
}
#[derive(clap::Args, Debug)]
pub struct IncentivesOpenDisputeArgs {
    /// Norito-encoded reward instruction (`RelayRewardInstructionV1`).
    #[arg(long = "instruction", value_name = "PATH")]
    pub instruction: PathBuf,
    /// Treasury account initiating the dispute.
    #[arg(long = "treasury-account", value_name = "ACCOUNT_ID")]
    pub treasury_account: String,
    /// Account ID submitting the dispute.
    #[arg(long = "submitted-by", value_name = "ACCOUNT_ID")]
    pub submitted_by: String,
    /// Requested adjustment quantity.
    #[arg(long = "requested-amount", value_name = "QUANTITY")]
    pub requested_amount: String,
    /// Reason provided by the operator.
    #[arg(long = "reason", value_name = "TEXT")]
    pub reason: String,
    /// Optional UNIX timestamp when the dispute is filed.
    #[arg(long = "submitted-at", value_name = "SECONDS")]
    pub submitted_at: Option<u64>,
    /// Optional path where the Norito-encoded dispute will be written.
    #[arg(long = "norito-out", value_name = "PATH")]
    pub norito_out: Option<PathBuf>,
    /// Emit pretty-printed JSON.
    ///
    /// Ignored when `--output-format json` is used.
    #[arg(long = "pretty", default_value_t = false)]
    pub pretty: bool,
}
impl Run for IncentivesOpenDisputeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let instruction = read_reward_instruction(&self.instruction)?;
        let treasury = parse_account_id_str(context, &self.treasury_account, "--treasury-account")?;
        let submitted_by = parse_account_id_str(context, &self.submitted_by, "--submitted-by")?;
        let requested_amount = parse_quantity_str(&self.requested_amount, "--requested-amount")?;
        let submitted_at = self.submitted_at.unwrap_or_else(unix_now);
        let ledger = RelayPayoutLedger::new(treasury);
        let dispute = ledger.open_dispute(
            instruction,
            requested_amount,
            submitted_by,
            submitted_at,
            self.reason,
        );
        if let Some(path) = &self.norito_out {
            write_norito_payload(path, &dispute)?;
        }
        let json_bytes = if self.pretty {
            norito::json::to_vec_pretty(&dispute)?
        } else {
            norito::json::to_vec(&dispute)?
        };
        let output = String::from_utf8(json_bytes)
            .map_err(|err| eyre!("dispute JSON is not valid UTF-8: {err}"))?;
        context.println(output)
    }
}
#[derive(clap::Args, Debug)]
pub struct IncentivesDashboardArgs {
    /// Reward instruction payloads to include in the dashboard.
    #[arg(
        long = "instruction",
        value_name = "PATH",
        required = true,
        num_args = 1..
    )]
    pub instructions: Vec<PathBuf>,
}
impl Run for IncentivesDashboardArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let mut accumulator = RelayEarningsAccumulator::default();
        for path in &self.instructions {
            let instruction = read_reward_instruction(path)?;
            accumulator.record(&instruction)?;
        }
        let mut rows: Vec<_> = accumulator
            .entries()
            .iter()
            .map(|(relay_id, entry)| IncentivesDashboardRow {
                relay: hex::encode(relay_id),
                payout_count: entry.payout_count,
                payout_amount: entry.payout_amount.clone(),
            })
            .collect();
        rows.sort_by(|a, b| a.relay.cmp(&b.relay));
        let total_payout = rows.iter().try_fold(Quantity::zero(), |acc, row| {
            acc.checked_add(&row.payout_amount)
        })?;
        let summary = IncentivesDashboardSummary {
            total_relays: rows.len(),
            total_payout,
            rows,
        };
        context.print_data(&summary)
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum IncentivesServiceCommand {
    /// Initialise a new payout ledger state file.
    Init(IncentivesServiceInitArgs),
    /// Evaluate metrics, record the payout, and persist the updated state.
    Process(IncentivesServiceProcessArgs),
    /// Record an externally prepared reward instruction into the state.
    Record(IncentivesServiceRecordArgs),
    /// Manage payout disputes recorded in the state.
    #[command(subcommand)]
    Dispute(IncentivesServiceDisputeCommand),
    /// Render an earnings dashboard sourced from the persisted ledger.
    Dashboard(IncentivesServiceDashboardArgs),
    /// Audit bond/payout governance readiness for relay incentives.
    Audit(IncentivesServiceAuditArgs),
    /// Run a shadow simulation across relay metrics and summarise fairness.
    ShadowRun(IncentivesServiceShadowRunArgs),
    /// Reconcile recorded payouts against XOR ledger exports.
    Reconcile(IncentivesServiceReconcileArgs),
    /// Run the treasury daemon against a metrics spool.
    Daemon(IncentivesServiceDaemonArgs),
}
impl_run_for_subcommand!(IncentivesServiceCommand => Init, Process, Record, Dispute, Dashboard, Audit, ShadowRun, Reconcile, Daemon);
#[derive(clap::Args, Debug)]
pub struct IncentivesServiceInitArgs {
    /// Path where the incentives state JSON will be stored.
    #[arg(long = "state", value_name = "PATH")]
    pub state: PathBuf,
    /// Reward configuration JSON consumed by the payout engine.
    #[arg(long = "config", value_name = "PATH")]
    pub config: PathBuf,
    /// Treasury account debited when materialising payouts.
    #[arg(long = "treasury-account", value_name = "ACCOUNT_ID")]
    pub treasury_account: String,
    /// Overwrite an existing state file if it already exists.
    #[arg(long = "force", default_value_t = false)]
    pub force: bool,
}
impl Run for IncentivesServiceInitArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if self.state.exists() && !self.force {
            return Err(eyre!(
                "state file `{}` already exists (pass --force to overwrite)",
                self.state.display()
            ));
        }
        let config = read_reward_config(&self.config)?;
        if config.budget_approval_id.is_none() {
            return Err(eyre!(
                "reward_config.budget_approval_id is required for incentives"
            ));
        }
        let treasury_account =
            parse_account_id_str(context, &self.treasury_account, "--treasury-account")?;
        let state = IncentivesState::new(&config, treasury_account);
        save_incentives_state(&self.state, &state)?;
        context.println(format_args!(
            "initialised incentives state at `{}`",
            self.state.display()
        ))
    }
}
#[derive(clap::Args, Debug)]
pub struct IncentivesServiceProcessArgs {
    /// Path to the persisted incentives state JSON.
    #[arg(long = "state", value_name = "PATH")]
    pub state: PathBuf,
    /// Norito-encoded relay metrics (`RelayEpochMetricsV1`).
    #[arg(long = "metrics", value_name = "PATH", num_args = 1..)]
    pub metrics: Vec<PathBuf>,
    /// Norito-encoded bond ledger entry (`RelayBondLedgerEntryV1`).
    #[arg(long = "bond", value_name = "PATH", num_args = 1..)]
    pub bond: Vec<PathBuf>,
    /// Beneficiary account that receives the payout.
    #[arg(long = "beneficiary", value_name = "ACCOUNT_ID", num_args = 1..)]
    pub beneficiary: Vec<String>,
    /// Write the Norito-encoded reward instruction to this path.
    #[arg(long = "instruction-out", value_name = "PATH")]
    pub instruction_out: Option<PathBuf>,
    /// Write the Norito-encoded transfer instruction to this path.
    #[arg(long = "transfer-out", value_name = "PATH")]
    pub transfer_out: Option<PathBuf>,
    /// Submit the resulting transfer to Torii after recording the payout.
    #[arg(long = "submit-transfer", default_value_t = false)]
    pub submit_transfer: bool,
    /// Emit pretty JSON instead of a compact payload.
    ///
    /// Ignored when `--output-format json` is used.
    #[arg(long = "pretty", default_value_t = false)]
    pub pretty: bool,
}
impl Run for IncentivesServiceProcessArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if self.metrics.is_empty() {
            return Err(eyre!("at least one --metrics file must be provided"));
        }
        let (mut state, mut service) = load_state_service(&self.state)?;
        let budget_approval_id =
            require_budget_approval_id(state.reward_config.budget_approval_id.as_ref())?;
        let metrics: Vec<_> = self
            .metrics
            .iter()
            .map(|path| read_metrics_file(path.as_path()))
            .collect::<Result<_, _>>()?;
        let metrics_count = metrics.len();
        let bonds: Vec<_> = if self.bond.is_empty() {
            return Err(eyre!("at least one --bond file must be provided"));
        } else if self.bond.len() == 1 {
            let entry = read_bond_entry(&self.bond[0])?;
            vec![entry; metrics_count]
        } else if self.bond.len() == metrics_count {
            self.bond
                .iter()
                .map(|path| read_bond_entry(path.as_path()))
                .collect::<Result<_, _>>()?
        } else {
            return Err(eyre!(
                "number of --bond entries ({}) must be 1 or match the number of --metrics entries ({})",
                self.bond.len(),
                metrics_count
            ));
        };
        let beneficiaries: Vec<_> = if self.beneficiary.is_empty() {
            return Err(eyre!("at least one --beneficiary value must be provided"));
        } else if self.beneficiary.len() == 1 {
            let account = parse_account_id_str(context, &self.beneficiary[0], "--beneficiary")?;
            vec![account; metrics_count]
        } else if self.beneficiary.len() == metrics_count {
            self.beneficiary
                .iter()
                .map(|value| parse_account_id_str(context, value, "--beneficiary"))
                .collect::<Result<_, _>>()?
        } else {
            return Err(eyre!(
                "number of --beneficiary values ({}) must be 1 or match the number of --metrics entries ({})",
                self.beneficiary.len(),
                metrics_count
            ));
        };
        if metrics_count > 1 && (self.instruction_out.is_some() || self.transfer_out.is_some()) {
            return Err(eyre!(
                "`--instruction-out` and `--transfer-out` are only supported when processing a single metrics entry"
            ));
        }
        let inputs: Vec<_> = metrics
            .iter()
            .zip(bonds.iter())
            .zip(beneficiaries.iter())
            .map(|((metrics, bond), beneficiary)| PayoutInput {
                metrics,
                bond_entry: bond,
                beneficiary: beneficiary.clone(),
                metadata: Metadata::default(),
            })
            .collect();
        let outcomes = service
            .process_batch(inputs)
            .map_err(|err| eyre!("failed to process epoch: {err}"))?;
        if metrics_count == 1 {
            if let Some(path) = &self.instruction_out {
                write_norito_payload(path, &outcomes[0].instruction)?;
            }
            if let Some(path) = &self.transfer_out {
                write_norito_payload(path, &outcomes[0].transfer)?;
            }
        }
        let mut transfers_to_submit = Vec::new();
        let mut summaries = Vec::new();
        for outcome in &outcomes {
            ensure_instruction_budget_approval(&outcome.instruction, &budget_approval_id)?;
            if self.submit_transfer && !outcome.instruction.is_zero_amount() {
                transfers_to_submit.push(outcome.transfer.clone());
            }
            store_payout_instruction(&mut state, &outcome.instruction);
            let snapshot = ServiceLedgerSnapshot::from_snapshot(&outcome.ledger_snapshot);
            summaries.push(ServicePayoutSummary::new(&outcome.instruction, snapshot));
        }
        save_incentives_state(&self.state, &state)?;
        if self.submit_transfer && !transfers_to_submit.is_empty() {
            context
                .finish(transfers_to_submit)
                .wrap_err("failed to submit payout transfer")?;
        }
        if summaries.len() == 1 {
            output_summary(context, &summaries[0], self.pretty)
        } else {
            output_summary(context, &summaries, self.pretty)
        }
    }
}
#[derive(clap::Args, Debug)]
pub struct IncentivesServiceRecordArgs {
    /// Path to the persisted incentives state JSON.
    #[arg(long = "state", value_name = "PATH")]
    pub state: PathBuf,
    /// Norito-encoded reward instruction to record.
    #[arg(long = "instruction", value_name = "PATH")]
    pub instruction: PathBuf,
    /// Write the Norito-encoded transfer instruction to this path if non-zero.
    #[arg(long = "transfer-out", value_name = "PATH")]
    pub transfer_out: Option<PathBuf>,
    /// Submit the transfer to Torii after recording the payout.
    #[arg(long = "submit-transfer", default_value_t = false)]
    pub submit_transfer: bool,
    /// Emit pretty JSON instead of a compact payload.
    ///
    /// Ignored when `--output-format json` is used.
    #[arg(long = "pretty", default_value_t = false)]
    pub pretty: bool,
}
impl Run for IncentivesServiceRecordArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let instruction = read_reward_instruction(&self.instruction)?;
        let (mut state, mut service) = load_state_service(&self.state)?;
        let budget_approval_id =
            require_budget_approval_id(state.reward_config.budget_approval_id.as_ref())?;
        ensure_instruction_budget_approval(&instruction, &budget_approval_id)?;
        let transfer_instruction = service.payout_ledger().to_transfer(&instruction);
        if self.submit_transfer
            && !instruction.is_zero_amount()
            && let Some(transfer) = transfer_instruction.as_ref()
        {
            context
                .finish(vec![transfer.clone()])
                .wrap_err("failed to submit payout transfer")?;
        }
        service
            .record_reward(instruction.clone())
            .map_err(|err| eyre!("failed to record reward instruction: {err}"))?;
        if let (Some(path), Some(transfer)) = (&self.transfer_out, transfer_instruction.clone()) {
            write_norito_payload(path, &transfer)?;
        }
        let dashboard = service
            .earnings_dashboard()
            .map_err(|err| eyre!("failed to build earnings dashboard: {err}"))?;
        let ledger = dashboard
            .rows
            .iter()
            .find(|row| row.relay_id == instruction.relay_id)
            .map(ServiceLedgerSnapshot::from_row)
            .ok_or_else(|| eyre!("recorded relay not present in earnings dashboard"))?;
        store_payout_instruction(&mut state, &instruction);
        save_incentives_state(&self.state, &state)?;
        let summary = ServicePayoutSummary::new(&instruction, ledger);
        output_summary(context, &summary, self.pretty)
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum IncentivesServiceDisputeCommand {
    /// File a new dispute against a recorded payout.
    File(IncentivesServiceDisputeFileArgs),
    /// Resolve a dispute with the supplied outcome.
    Resolve(IncentivesServiceDisputeResolveArgs),
    /// Reject a dispute without altering the ledger.
    Reject(IncentivesServiceDisputeRejectArgs),
}
impl_run_for_subcommand!(IncentivesServiceDisputeCommand => File, Resolve, Reject);
#[derive(clap::Args, Debug)]
pub struct IncentivesServiceDisputeFileArgs {
    /// Path to the persisted incentives state JSON.
    #[arg(long = "state", value_name = "PATH")]
    pub state: PathBuf,
    /// Hex-encoded relay identifier (32 bytes, 64 hex chars).
    #[arg(long = "relay-id", value_name = "HEX")]
    pub relay_id: String,
    /// Epoch number associated with the disputed payout.
    #[arg(long = "epoch", value_name = "EPOCH")]
    pub epoch: u32,
    /// Account ID submitting the dispute.
    #[arg(long = "submitted-by", value_name = "ACCOUNT_ID")]
    pub submitted_by: String,
    /// Requested payout quantity.
    #[arg(long = "requested-amount", value_name = "QUANTITY")]
    pub requested_amount: String,
    /// Free-form reason describing the dispute.
    #[arg(long = "reason", value_name = "TEXT")]
    pub reason: String,
    /// Optional UNIX timestamp indicating when the dispute was filed (defaults to now).
    #[arg(long = "filed-at", value_name = "SECONDS")]
    pub filed_at: Option<u64>,
    /// Credit adjustment requested by the operator.
    #[arg(
        long = "adjust-credit",
        value_name = "QUANTITY",
        conflicts_with = "adjust_debit"
    )]
    pub adjust_credit: Option<String>,
    /// Debit adjustment requested by the operator.
    #[arg(
        long = "adjust-debit",
        value_name = "QUANTITY",
        conflicts_with = "adjust_credit"
    )]
    pub adjust_debit: Option<String>,
    /// Write the Norito-encoded dispute payload to this path.
    #[arg(long = "norito-out", value_name = "PATH")]
    pub norito_out: Option<PathBuf>,
    /// Emit pretty JSON instead of a compact payload.
    ///
    /// Ignored when `--output-format json` is used.
    #[arg(long = "pretty", default_value_t = false)]
    pub pretty: bool,
}
impl Run for IncentivesServiceDisputeFileArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let (mut state, mut service) = load_state_service(&self.state)?;
        let relay_id = relay_id_from_hex(&self.relay_id)?;
        let submitted_by = parse_account_id_str(context, &self.submitted_by, "--submitted-by")?;
        let requested_amount = parse_quantity_str(&self.requested_amount, "--requested-amount")?;
        let requested_adjustment =
            parse_adjustment_flags(self.adjust_credit.as_ref(), self.adjust_debit.as_ref())?;
        let filed_at = self.filed_at.unwrap_or_else(unix_now);
        let dispute = service
            .file_dispute(
                relay_id,
                self.epoch,
                submitted_by,
                requested_amount,
                self.reason,
                filed_at,
                requested_adjustment,
            )
            .map_err(|err| eyre!("failed to file dispute: {err}"))?;
        if let Some(path) = &self.norito_out {
            write_norito_payload(path, dispute.norito_record())?;
        }
        upsert_dispute_record(&mut state, &dispute);
        save_incentives_state(&self.state, &state)?;
        let record = StoredDisputeRecord::from(&dispute);
        output_summary(context, &record, self.pretty)
    }
}
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum IncentivesDisputeResolutionKind {
    #[clap(name = "no-change")]
    NoChange,
    #[clap(name = "credit")]
    Credit,
    #[clap(name = "debit")]
    Debit,
}
#[derive(clap::Args, Debug)]
pub struct IncentivesServiceDisputeResolveArgs {
    /// Path to the persisted incentives state JSON.
    #[arg(long = "state", value_name = "PATH")]
    pub state: PathBuf,
    /// Dispute identifier to resolve.
    #[arg(long = "dispute-id", value_name = "ID")]
    pub dispute_id: DisputeId,
    /// Resolution kind (`no-change`, `credit`, or `debit`).
    #[arg(long = "resolution", value_enum)]
    pub resolution: IncentivesDisputeResolutionKind,
    /// Amount applied when resolving with `credit` or `debit`.
    #[arg(long = "amount", value_name = "QUANTITY")]
    pub amount: Option<String>,
    /// Resolution notes recorded in the dispute metadata.
    #[arg(long = "notes", value_name = "TEXT")]
    pub notes: String,
    /// Optional UNIX timestamp when the dispute was resolved (defaults to now).
    #[arg(long = "resolved-at", value_name = "SECONDS")]
    pub resolved_at: Option<u64>,
    /// Write the Norito-encoded transfer instruction generated by the resolution (if any).
    #[arg(long = "transfer-out", value_name = "PATH")]
    pub transfer_out: Option<PathBuf>,
    /// Emit pretty JSON instead of a compact payload.
    ///
    /// Ignored when `--output-format json` is used.
    #[arg(long = "pretty", default_value_t = false)]
    pub pretty: bool,
}
impl Run for IncentivesServiceDisputeResolveArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let (mut state, mut service) = load_state_service(&self.state)?;
        let resolved_at = self.resolved_at.unwrap_or_else(unix_now);
        let resolution = match self.resolution {
            IncentivesDisputeResolutionKind::NoChange => {
                if self.amount.is_some() {
                    return Err(eyre!("--amount is not valid for no-change resolutions"));
                }
                DisputeResolution::NoChange {
                    notes: self.notes.clone(),
                }
            }
            IncentivesDisputeResolutionKind::Credit => {
                let amount = self
                    .amount
                    .as_ref()
                    .ok_or_else(|| eyre!("--amount is required for credit resolutions"))?;
                DisputeResolution::Credit {
                    amount: parse_quantity_str(amount, "--amount")?,
                    notes: self.notes.clone(),
                }
            }
            IncentivesDisputeResolutionKind::Debit => {
                let amount = self
                    .amount
                    .as_ref()
                    .ok_or_else(|| eyre!("--amount is required for debit resolutions"))?;
                DisputeResolution::Debit {
                    amount: parse_quantity_str(amount, "--amount")?,
                    notes: self.notes.clone(),
                }
            }
        };
        let outcome = service
            .resolve_dispute(self.dispute_id, resolution, resolved_at)
            .map_err(|err| eyre!("failed to resolve dispute: {err}"))?;
        if let (Some(path), Some(transfer)) = (&self.transfer_out, outcome.transfer.as_ref()) {
            write_norito_payload(path, transfer)?;
        }
        upsert_dispute_record(&mut state, &outcome.dispute);
        save_incentives_state(&self.state, &state)?;
        let record = StoredDisputeRecord::from(&outcome.dispute);
        output_summary(context, &record, self.pretty)
    }
}
#[derive(clap::Args, Debug)]
pub struct IncentivesServiceDisputeRejectArgs {
    /// Path to the persisted incentives state JSON.
    #[arg(long = "state", value_name = "PATH")]
    pub state: PathBuf,
    /// Dispute identifier to reject.
    #[arg(long = "dispute-id", value_name = "ID")]
    pub dispute_id: DisputeId,
    /// Rejection notes captured in the dispute metadata.
    #[arg(long = "notes", value_name = "TEXT")]
    pub notes: String,
    /// Optional UNIX timestamp when the dispute was rejected (defaults to now).
    #[arg(long = "rejected-at", value_name = "SECONDS")]
    pub rejected_at: Option<u64>,
    /// Emit pretty JSON instead of a compact payload.
    ///
    /// Ignored when `--output-format json` is used.
    #[arg(long = "pretty", default_value_t = false)]
    pub pretty: bool,
}
impl Run for IncentivesServiceDisputeRejectArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let (mut state, mut service) = load_state_service(&self.state)?;
        let rejected_at = self.rejected_at.unwrap_or_else(unix_now);
        let dispute = service
            .reject_dispute(self.dispute_id, rejected_at, self.notes.clone())
            .map_err(|err| eyre!("failed to reject dispute: {err}"))?;
        upsert_dispute_record(&mut state, &dispute);
        save_incentives_state(&self.state, &state)?;
        let record = StoredDisputeRecord::from(&dispute);
        output_summary(context, &record, self.pretty)
    }
}
#[derive(clap::Args, Debug)]
pub struct IncentivesServiceDashboardArgs {
    /// Path to the persisted incentives state JSON.
    #[arg(long = "state", value_name = "PATH")]
    pub state: PathBuf,
}
impl Run for IncentivesServiceDashboardArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let (_state, service) = load_state_service(&self.state)?;
        let dashboard = service
            .earnings_dashboard()
            .map_err(|err| eyre!("failed to build earnings dashboard: {err}"))?;
        let summary = ServiceDashboardSummary::new(&dashboard);
        context.print_data(&summary)
    }
}
#[derive(clap::ValueEnum, Clone, Debug, PartialEq, Eq, Hash)]
pub enum IncentiveAuditScope {
    Bond,
    Budget,
    All,
}
#[derive(clap::Args, Debug)]
pub struct IncentivesServiceAuditArgs {
    /// Path to the persisted incentives state JSON.
    #[arg(long = "state", value_name = "PATH")]
    pub state: PathBuf,
    /// Daemon configuration describing relay beneficiaries and bond sources.
    #[arg(long = "config", value_name = "PATH")]
    pub config: PathBuf,
    /// Audit scopes to evaluate (repeat to combine); defaults to bond checks.
    #[arg(
        long = "scope",
        value_enum,
        default_values_t = vec![IncentiveAuditScope::Bond],
        action = clap::ArgAction::Append
    )]
    pub scopes: Vec<IncentiveAuditScope>,
    /// Emit pretty JSON instead of a compact payload.
    ///
    /// Ignored when `--output-format json` is used.
    #[arg(long = "pretty", default_value_t = false)]
    pub pretty: bool,
}
impl Run for IncentivesServiceAuditArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let state = load_incentives_state(&self.state)?;
        let config = load_daemon_config(&self.config, &|literal| {
            crate::resolve_account_id(context, literal)
        })?;
        let (audit_bond_enabled, audit_budget_enabled) = audit_scope_flags(&self.scopes);
        let mut summary = IncentivesAuditSummary::default();
        if audit_bond_enabled {
            let bond_summary = audit_bonds(&config, &state.reward_config)?;
            summary.bond = Some(bond_summary);
        }
        if audit_budget_enabled {
            let budget_summary = audit_budget(&state)?;
            summary.budget = Some(budget_summary);
        }
        let failures = summary.failure_count();
        output_summary(context, &summary, self.pretty)?;
        if failures > 0 {
            return Err(eyre!("incentives audit found {failures} issue(s)"));
        }
        Ok(())
    }
}
#[derive(clap::Args, Debug)]
pub struct IncentivesServiceShadowRunArgs {
    /// Path to the persisted incentives state JSON.
    #[arg(long = "state", value_name = "PATH")]
    pub state: PathBuf,
    /// Shadow simulation configuration mapping relays to beneficiaries and bonds.
    #[arg(long = "config", value_name = "PATH")]
    pub config: PathBuf,
    /// Directory containing Norito-encoded relay metrics snapshots (`relay-<id>-epoch-<n>.to`).
    #[arg(long = "metrics-dir", value_name = "PATH")]
    pub metrics_dir: PathBuf,
    /// Optional path to write the shadow simulation report JSON.
    #[arg(long = "report-out", value_name = "PATH")]
    pub report_out: Option<PathBuf>,
    /// Emit pretty JSON instead of a compact payload.
    ///
    /// Ignored when `--output-format json` is used.
    #[arg(long = "pretty", default_value_t = false)]
    pub pretty: bool,
}
impl Run for IncentivesServiceShadowRunArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let state = load_incentives_state(&self.state)?;
        let mut state_for_run = state.clone();
        let expected_budget =
            require_budget_approval_id(state.reward_config.budget_approval_id.as_ref())?;
        let mut service = build_clean_payout_service(&state_for_run)?;
        let config = load_daemon_config(&self.config, &|literal| {
            crate::resolve_account_id(context, literal)
        })?;
        let iteration_summary = process_daemon_iteration(
            &mut state_for_run,
            &mut service,
            &config,
            &self.metrics_dir,
            None,
            None,
            None,
            Some(&expected_budget),
        )?;
        if iteration_summary.missing_budget_approval > 0
            || iteration_summary.mismatched_budget_approval > 0
        {
            return Err(eyre!(
                "shadow run found {} payout(s) missing or mismatching budget_approval_id",
                iteration_summary
                    .missing_budget_approval
                    .saturating_add(iteration_summary.mismatched_budget_approval)
            ));
        }
        let report = build_shadow_run_summary(&iteration_summary);
        if let Some(path) = &self.report_out {
            let bytes = norito::json::to_vec_pretty(&report)
                .wrap_err("failed to serialise shadow run report")?;
            fs::write(path, &bytes).wrap_err_with(|| {
                format!("failed to write shadow run report to `{}`", path.display())
            })?;
        }
        output_summary(context, &report, self.pretty)
    }
}
#[derive(clap::Args, Debug)]
pub struct IncentivesServiceReconcileArgs {
    /// Path to the persisted incentives state JSON.
    #[arg(long = "state", value_name = "PATH")]
    pub state: PathBuf,
    /// Norito-encoded XOR ledger export to reconcile against.
    #[arg(long = "ledger-export", value_name = "PATH")]
    pub ledger_export: PathBuf,
    /// Emit pretty JSON instead of a compact payload.
    ///
    /// Ignored when `--output-format json` is used.
    #[arg(long = "pretty", default_value_t = false)]
    pub pretty: bool,
}
impl Run for IncentivesServiceReconcileArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let (_, service) = load_state_service(&self.state)?;
        let export = read_ledger_export(&self.ledger_export)?;
        let report = service.reconcile_ledger(&export.transfers);
        let summary = ReconciliationReportSummary::from_report(&report);
        output_summary(context, &summary, self.pretty)
    }
}
#[derive(clap::Args, Debug)]
pub struct IncentivesServiceDaemonArgs {
    /// Path to the persisted incentives state JSON.
    #[arg(long = "state", value_name = "PATH")]
    pub state: PathBuf,
    /// Daemon configuration describing relay beneficiaries and bond sources.
    #[arg(long = "config", value_name = "PATH")]
    pub config: PathBuf,
    /// Directory containing Norito-encoded relay metrics snapshots.
    #[arg(long = "metrics-dir", value_name = "PATH")]
    pub metrics_dir: PathBuf,
    /// Directory where reward instructions will be written.
    #[arg(long = "instruction-out-dir", value_name = "PATH")]
    pub instruction_out_dir: Option<PathBuf>,
    /// Directory where transfer instructions will be written.
    #[arg(long = "transfer-out-dir", value_name = "PATH")]
    pub transfer_out_dir: Option<PathBuf>,
    /// Directory where processed metrics snapshots will be archived.
    #[arg(long = "archive-dir", value_name = "PATH")]
    pub archive_dir: Option<PathBuf>,
    /// Poll interval (seconds) when running continuously.
    #[arg(long = "poll-interval", value_name = "SECONDS", default_value_t = 30)]
    pub poll_interval: u64,
    /// Process the spool once and exit (do not watch for changes).
    #[arg(long = "once", default_value_t = false)]
    pub once: bool,
    /// Emit JSON summaries instead of plain-text logs.
    ///
    /// Ignored when `--output-format json` is used.
    #[arg(long = "pretty", default_value_t = false)]
    pub pretty: bool,
}
impl Run for IncentivesServiceDaemonArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let config = load_daemon_config(&self.config, &|literal| {
            crate::resolve_account_id(context, literal)
        })?;
        if let Some(dir) = &self.instruction_out_dir {
            fs::create_dir_all(dir).wrap_err_with(|| {
                format!(
                    "failed to create instruction output directory `{}`",
                    dir.display()
                )
            })?;
        }
        if let Some(dir) = &self.transfer_out_dir {
            fs::create_dir_all(dir).wrap_err_with(|| {
                format!(
                    "failed to create transfer output directory `{}`",
                    dir.display()
                )
            })?;
        }
        if let Some(dir) = &self.archive_dir {
            fs::create_dir_all(dir).wrap_err_with(|| {
                format!("failed to create archive directory `{}`", dir.display())
            })?;
        }
        let poll_interval = self.poll_interval.max(1);
        let (mut state, mut service) = load_state_service(&self.state)?;
        let expected_budget =
            require_budget_approval_id(state.reward_config.budget_approval_id.as_ref())?;
        loop {
            let summary = process_daemon_iteration(
                &mut state,
                &mut service,
                &config,
                &self.metrics_dir,
                self.instruction_out_dir.as_deref(),
                self.transfer_out_dir.as_deref(),
                self.archive_dir.as_deref(),
                Some(&expected_budget),
            )?;
            if !summary.processed.is_empty() {
                save_incentives_state(&self.state, &state)?;
            }
            log_daemon_summary(context, &summary, self.pretty)?;
            if summary.missing_budget_approval > 0 || summary.mismatched_budget_approval > 0 {
                return Err(eyre!(
                    "daemon detected {} payout(s) missing or mismatching budget_approval_id",
                    summary
                        .missing_budget_approval
                        .saturating_add(summary.mismatched_budget_approval)
                ));
            }
            if self.once {
                break;
            }
            thread::sleep(Duration::from_secs(poll_interval));
        }
        Ok(())
    }
}
fn resolve_manifest_inputs<C: RunContext>(
    context: &mut C,
    args: &FetchArgs,
) -> Result<ManifestInputs> {
    let downloaded = maybe_download_manifest(context, args)?;
    merge_manifest_inputs(
        args.manifest.as_ref(),
        args.plan.as_ref(),
        args.manifest_id.as_ref(),
        downloaded.as_ref(),
    )
}
fn maybe_download_manifest<C: RunContext>(
    context: &mut C,
    args: &FetchArgs,
) -> Result<Option<DownloadedManifest>> {
    let needs_fetch = args.storage_ticket.is_some()
        && (args.manifest.is_none() || args.plan.is_none() || args.manifest_id.is_none());
    if !needs_fetch {
        return Ok(None);
    }
    let ticket = args
        .storage_ticket
        .as_ref()
        .expect("storage ticket present when fetch is required");
    let normalized_ticket = normalize_ticket_hex(ticket)?;
    let fetcher = DaManifestFetcher::new(context.config(), args.manifest_endpoint.as_deref())?;
    let bundle = fetcher.fetch(&normalized_ticket)?;
    let persisted = persist_manifest_bundle(
        context,
        &bundle,
        args.manifest_cache_dir.clone(),
        &normalized_ticket,
    )?;
    Ok(Some(DownloadedManifest {
        manifest_path: persisted.manifest,
        plan_path: persisted.chunk_plan,
        manifest_id: bundle.manifest_hash_hex,
    }))
}
fn merge_manifest_inputs(
    manifest: Option<&PathBuf>,
    plan: Option<&PathBuf>,
    manifest_id: Option<&String>,
    fallback: Option<&DownloadedManifest>,
) -> Result<ManifestInputs> {
    let manifest_path = match manifest {
        Some(path) => path.clone(),
        None => fallback.map(|dl| dl.manifest_path.clone()).ok_or_else(|| {
            eyre!("--manifest is required unless `--storage-ticket` provides one")
        })?,
    };
    let plan_path = match plan {
        Some(path) => path.clone(),
        None => fallback
            .map(|dl| dl.plan_path.clone())
            .ok_or_else(|| eyre!("--plan is required unless `--storage-ticket` provides one"))?,
    };
    let manifest_id = match manifest_id {
        Some(id) => id.clone(),
        None => fallback.map(|dl| dl.manifest_id.clone()).ok_or_else(|| {
            eyre!("--manifest-id is required unless `--storage-ticket` provides one")
        })?,
    };
    Ok(ManifestInputs {
        manifest_path,
        plan_path,
        manifest_id,
    })
}
fn validate_manifest_envelope(bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() {
        return Err(eyre!("manifest envelope must not be empty"));
    }
    let envelope: HybridPayloadEnvelopeV1 =
        decode_from_bytes(bytes).wrap_err("failed to decode manifest envelope")?;
    if envelope.version != HYBRID_PAYLOAD_ENVELOPE_VERSION_V1 {
        return Err(eyre!(
            "manifest envelope version {} is not supported (expected {})",
            envelope.version,
            HYBRID_PAYLOAD_ENVELOPE_VERSION_V1
        ));
    }
    let suite = HybridSuite::from_str(&envelope.suite).map_err(|()| {
        eyre!(
            "unsupported hybrid suite `{}` in manifest envelope",
            envelope.suite
        )
    })?;
    if suite != HybridSuite::X25519MlKem768ChaCha20Poly1305 {
        return Err(eyre!(
            "manifest envelope must use the X25519+ML-KEM-768 suite"
        ));
    }
    if envelope.kem.ephemeral_public.is_empty()
        || envelope.kem.kyber_ciphertext.is_empty()
        || envelope.ciphertext.is_empty()
    {
        return Err(eyre!(
            "manifest envelope is missing required KEM or ciphertext fields"
        ));
    }
    Ok(())
}
fn load_manifest_envelope(path: &Path) -> Result<String> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read manifest envelope from `{}`", path.display()))?;
    validate_manifest_envelope(&bytes)?;
    Ok(STANDARD.encode(bytes))
}
#[cfg(test)]
mod fetch_args_manifest_tests {
    use super::{DownloadedManifest, merge_manifest_inputs};
    use std::path::PathBuf;
    #[test]
    fn merge_inputs_prefers_explicit_values() {
        let manifest = PathBuf::from("/tmp/manifest_explicit.to");
        let plan = PathBuf::from("/tmp/plan_explicit.json");
        let manifest_id = "11".repeat(32);
        let fallback = DownloadedManifest {
            manifest_path: PathBuf::from("/tmp/fallback_manifest.to"),
            plan_path: PathBuf::from("/tmp/fallback_plan.json"),
            manifest_id: "22".repeat(32),
        };
        let inputs = merge_manifest_inputs(
            Some(&manifest),
            Some(&plan),
            Some(&manifest_id),
            Some(&fallback),
        )
        .expect("inputs");
        assert_eq!(inputs.manifest_path, manifest);
        assert_eq!(inputs.plan_path, plan);
        assert_eq!(inputs.manifest_id, manifest_id);
    }
    #[test]
    fn merge_inputs_uses_fallback_when_missing() {
        let manifest_id = "aa".repeat(32);
        let fallback = DownloadedManifest {
            manifest_path: PathBuf::from("/tmp/fetched_manifest.to"),
            plan_path: PathBuf::from("/tmp/fetched_plan.json"),
            manifest_id: manifest_id.clone(),
        };
        let inputs =
            merge_manifest_inputs(None, None, None, Some(&fallback)).expect("resolved inputs");
        assert_eq!(inputs.manifest_path, fallback.manifest_path);
        assert_eq!(inputs.plan_path, fallback.plan_path);
        assert_eq!(inputs.manifest_id, manifest_id);
    }
    #[test]
    fn merge_inputs_errors_without_source() {
        let err = merge_manifest_inputs(None, None, None, None).expect_err("expected failure");
        assert_compact! { err.to_string().contains("`--storage-ticket` provides one"); "error message should mention storage ticket fallback" };
    }
}
#[cfg(test)]
mod manifest_envelope_tests {
    use super::{HybridSuite, load_manifest_envelope};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use iroha_crypto::HybridKeyPair;
    use norito::to_bytes;
    use rand::{SeedableRng, rngs::StdRng};
    use sorafs_manifest::hybrid_envelope::{
        HYBRID_PAYLOAD_ENVELOPE_VERSION_V1, HybridKemBundleV1, HybridPayloadEnvelopeV1,
        encrypt_payload,
    };
    use std::io::Write;
    use tempfile::NamedTempFile;
    #[test]
    fn load_manifest_envelope_rejects_empty_files() {
        let file = NamedTempFile::new().expect("temp file");
        let err = load_manifest_envelope(file.path()).expect_err("empty envelope must fail");
        assert_compact! { err.to_string().contains("must not be empty"); "error should mention empty envelope" };
    }
    #[test]
    fn load_manifest_envelope_encodes_valid_envelope() {
        let mut rng = StdRng::seed_from_u64(7);
        let key_pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let envelope = encrypt_payload(
            b"manifest payload",
            b"sorafs:manifest:test",
            key_pair.public(),
            &mut rng,
        )
        .expect("envelope encrypts");
        let mut file = NamedTempFile::new().expect("temp file");
        let encoded_bytes = to_bytes(&envelope).expect("encode envelope");
        file.write_all(&encoded_bytes).expect("write envelope");
        let encoded = load_manifest_envelope(file.path()).expect("manifest envelope should load");
        let expected = STANDARD.encode(encoded_bytes);
        assert_eq!(encoded, expected);
    }
    #[test]
    fn load_manifest_envelope_rejects_invalid_contents() {
        let mut file = NamedTempFile::new().expect("temp file");
        let envelope = HybridPayloadEnvelopeV1 {
            version: HYBRID_PAYLOAD_ENVELOPE_VERSION_V1,
            suite: HybridSuite::X25519MlKem768ChaCha20Poly1305.to_string(),
            kem: HybridKemBundleV1 {
                ephemeral_public: Vec::new(),
                kyber_ciphertext: Vec::new(),
            },
            nonce: [0u8; 12],
            ciphertext: Vec::new(),
        };
        let encoded = to_bytes(&envelope).expect("encode envelope");
        file.write_all(&encoded).expect("write envelope");
        let err =
            load_manifest_envelope(file.path()).expect_err("invalid manifest envelope must fail");
        assert_compact! { err.to_string().contains("manifest envelope is missing required KEM or ciphertext fields"); "error should call out missing fields" };
    }
}
#[cfg(test)]
mod cli_scoreboard_metadata_tests {
    use super::*;
    use norito::json::Value;
    #[test]
    fn cli_scoreboard_metadata_records_policy_overrides() {
        let value = cli_scoreboard_metadata(&ScoreboardMetadataInput {
            provider_counts: ProviderCounts::new(2, 2),
            max_peers: Some(3),
            retry_budget: Some(5),
            manifest_envelope_present: true,
            gateway_manifest_id: Some("deadbeef".to_string()),
            gateway_manifest_cid: Some("c0ffee".to_string()),
            transport_policy: Some(TransportPolicy::SoranetPreferred),
            transport_policy_override: Some(TransportPolicy::DirectOnly),
            anonymity_policy: Some(AnonymityPolicy::GuardPq),
            anonymity_policy_override: Some(AnonymityPolicy::StrictPq),
            write_mode: WriteModeHint::ReadOnly,
            scoreboard_now: None,
            telemetry_source: None,
            telemetry_region: None,
        });
        let object = value.as_object().expect("metadata should be a JSON object");
        assert_eq_compact! { object.get("transport_policy").and_then(Value::as_str).expect("transport_policy string") => "direct-only" };
        assert_eq_compact! { object.get("transport_policy_override").and_then(Value::as_bool) => Some(true) };
        assert_eq_compact! { object.get("transport_policy_override_label").and_then(Value::as_str) => Some("direct-only") };
        assert_eq_compact! { object.get("anonymity_policy").and_then(Value::as_str).expect("anonymity label") => "anon-strict-pq" };
        assert_eq_compact! { object.get("anonymity_policy_override").and_then(Value::as_bool) => Some(true) };
        assert_eq_compact! { object.get("anonymity_policy_override_label").and_then(Value::as_str) => Some("anon-strict-pq") };
        assert_eq_compact! { object.get("gateway_manifest_id").and_then(Value::as_str) => Some("deadbeef") };
        assert_eq_compact! { object.get("gateway_manifest_cid").and_then(Value::as_str) => Some("c0ffee") };
    }
    #[test]
    fn cli_scoreboard_metadata_includes_timestamp_and_telemetry_label() {
        let value = cli_scoreboard_metadata(&ScoreboardMetadataInput {
            provider_counts: ProviderCounts::new(1, 0),
            max_peers: None,
            retry_budget: None,
            manifest_envelope_present: false,
            gateway_manifest_id: None,
            gateway_manifest_cid: None,
            transport_policy: None,
            transport_policy_override: None,
            anonymity_policy: None,
            anonymity_policy_override: None,
            write_mode: WriteModeHint::ReadOnly,
            scoreboard_now: Some(1_700_000_000),
            telemetry_source: Some("otel::prod".to_string()),
            telemetry_region: Some("iad-prod".to_string()),
        });
        let object = value.as_object().expect("metadata should be a JSON object");
        assert_eq_compact! { object.get("assume_now").and_then(Value::as_u64) => Some(1_700_000_000) };
        assert_eq_compact! { object.get("telemetry_source").and_then(Value::as_str) => Some("otel::prod") };
        assert_eq_compact! { object.get("telemetry_region").and_then(Value::as_str) => Some("iad-prod") };
    }
    #[test]
    fn cli_scoreboard_metadata_defaults_to_soranet_first_transport() {
        let value = cli_scoreboard_metadata(&ScoreboardMetadataInput {
            provider_counts: ProviderCounts::new(0, 2),
            max_peers: None,
            retry_budget: None,
            manifest_envelope_present: false,
            gateway_manifest_id: None,
            gateway_manifest_cid: None,
            transport_policy: None,
            transport_policy_override: None,
            anonymity_policy: None,
            anonymity_policy_override: None,
            write_mode: WriteModeHint::ReadOnly,
            scoreboard_now: None,
            telemetry_source: None,
            telemetry_region: None,
        });
        let object = value.as_object().expect("metadata should be a JSON object");
        assert_eq_compact! { object.get("transport_policy").and_then(Value::as_str).expect("transport_policy string") => "soranet-first" };
        assert_eq_compact! { object.get("transport_policy_override").and_then(Value::as_bool) => Some(false) };
        assert_eq_compact! { object.get("provider_count").and_then(Value::as_u64) => Some(0) };
        assert_eq_compact! { object.get("gateway_provider_count").and_then(Value::as_u64) => Some(2) };
    }
    #[test]
    fn cli_scoreboard_metadata_distinguishes_gateway_providers() {
        let value = cli_scoreboard_metadata(&ScoreboardMetadataInput {
            provider_counts: ProviderCounts::new(5, 7),
            max_peers: Some(4),
            retry_budget: Some(6),
            manifest_envelope_present: true,
            gateway_manifest_id: Some("abc123".to_string()),
            gateway_manifest_cid: Some("def456".to_string()),
            transport_policy: Some(TransportPolicy::SoranetPreferred),
            transport_policy_override: None,
            anonymity_policy: Some(AnonymityPolicy::MajorityPq),
            anonymity_policy_override: None,
            write_mode: WriteModeHint::ReadOnly,
            scoreboard_now: Some(123),
            telemetry_source: Some("ci".to_string()),
            telemetry_region: None,
        });
        let object = value.as_object().expect("metadata should be a JSON object");
        assert_eq_compact! { object.get("provider_count").and_then(Value::as_u64) => Some(5) };
        assert_eq_compact! { object.get("gateway_provider_count").and_then(Value::as_u64) => Some(7) };
        assert_eq_compact! { object.get("transport_policy").and_then(Value::as_str) => Some("soranet-first") };
        assert_eq_compact! { object.get("anonymity_policy").and_then(Value::as_str) => Some("anon-majority-pq") };
    }
    #[test]
    fn cli_scoreboard_metadata_sets_provider_mix() {
        let value = cli_scoreboard_metadata(&ScoreboardMetadataInput {
            provider_counts: ProviderCounts::new(0, 1),
            max_peers: None,
            retry_budget: None,
            manifest_envelope_present: false,
            gateway_manifest_id: None,
            gateway_manifest_cid: None,
            transport_policy: None,
            transport_policy_override: None,
            anonymity_policy: None,
            anonymity_policy_override: None,
            write_mode: WriteModeHint::ReadOnly,
            scoreboard_now: None,
            telemetry_source: None,
            telemetry_region: None,
        });
        let object = value.as_object().expect("metadata object");
        assert_eq_compact! { object.get("provider_mix").and_then(Value::as_str) => Some("gateway-only") };
    }
}
#[derive(Debug, norito::json::JsonSerialize)]
struct IncentivesDashboardRow {
    relay: String,
    payout_count: u64,
    payout_amount: Quantity,
}
#[derive(Debug, norito::json::JsonSerialize)]
struct IncentivesDashboardSummary {
    total_relays: usize,
    total_payout: Quantity,
    rows: Vec<IncentivesDashboardRow>,
}
#[derive(Debug, norito::json::JsonSerialize)]
struct DaemonProcessedPayoutSummary {
    relay_id_hex: String,
    epoch: u32,
    payout_amount: Quantity,
    budget_approval_id: Option<String>,
    metrics: PayoutMetricsSnapshot,
    instruction_path: Option<String>,
    transfer_path: Option<String>,
    metrics_archived_to: Option<String>,
}
#[derive(Debug, Default, norito::json::JsonSerialize)]
struct DaemonIterationSummary {
    processed: Vec<DaemonProcessedPayoutSummary>,
    skipped_missing_config: usize,
    skipped_missing_bond: usize,
    skipped_duplicate: usize,
    missing_budget_approval: usize,
    mismatched_budget_approval: usize,
    expected_budget_approval: Option<String>,
    errors: Vec<String>,
}
#[derive(Debug, Default, norito::json::JsonSerialize)]
struct IncentivesAuditSummary {
    bond: Option<BondAuditSummary>,
    budget: Option<BudgetAuditSummary>,
}
impl IncentivesAuditSummary {
    fn failure_count(&self) -> usize {
        let bond = self
            .bond
            .as_ref()
            .map_or(0, BondAuditSummary::failure_count);
        let budget = self
            .budget
            .as_ref()
            .map_or(0, BudgetAuditSummary::failure_count);
        bond.saturating_add(budget)
    }
}
#[derive(Debug, Default, norito::json::JsonSerialize)]
struct BondAuditSummary {
    total_relays: usize,
    exit_relays: usize,
    satisfied: usize,
    missing_bond: usize,
    insufficient_bond: usize,
    asset_mismatch: usize,
    policy_minimum_exit_bond: String,
    policy_bond_asset_id: String,
    errors: Vec<String>,
}
impl BondAuditSummary {
    fn failure_count(&self) -> usize {
        self.missing_bond
            .saturating_add(self.insufficient_bond)
            .saturating_add(self.asset_mismatch)
            .saturating_add(self.errors.len())
    }
}
#[derive(Debug, Default, norito::json::JsonSerialize)]
struct BudgetAuditSummary {
    configured_budget_approval_id: Option<String>,
    total_payouts: usize,
    payouts_without_budget: usize,
    mismatched_budget_approval: usize,
}
impl BudgetAuditSummary {
    fn failure_count(&self) -> usize {
        let missing_config: usize = usize::from(self.configured_budget_approval_id.is_none());
        missing_config
            .saturating_add(self.payouts_without_budget)
            .saturating_add(self.mismatched_budget_approval)
    }
}
#[derive(Debug)]
struct MetricsCandidate {
    relay_id: RelayId,
    relay_hex: String,
    epoch: u32,
    path: PathBuf,
    file_name: String,
}
#[derive(Debug, Clone, norito::json::JsonSerialize)]
struct PayoutMetricsSnapshot {
    availability_per_mille: u16,
    bandwidth_per_mille: u16,
    compliance_per_mille: u16,
    compliance_status: String,
    score_per_mille: u16,
    exit_bonus_applied: bool,
}
#[derive(Debug, Clone)]
struct DaemonConfig {
    relays: HashMap<RelayId, DaemonRelayEntry>,
}
impl DaemonConfig {
    fn entry(&self, relay_id: &RelayId) -> Option<&DaemonRelayEntry> {
        self.relays.get(relay_id)
    }
}
#[derive(Debug, Clone)]
struct DaemonRelayEntry {
    relay_hex: String,
    beneficiary: AccountId,
    bond_path: PathBuf,
}
#[derive(Debug, norito::json::JsonDeserialize)]
struct DaemonConfigFile {
    relays: Vec<DaemonRelayConfigFile>,
}
#[derive(Debug, norito::json::JsonDeserialize)]
struct DaemonRelayConfigFile {
    relay_id: String,
    beneficiary: String,
    bond_path: String,
}
fn read_reward_config(path: &Path) -> Result<RewardConfig> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read reward config from `{}`", path.display()))?;
    let state: RewardConfigState =
        norito::json::from_slice(&bytes).wrap_err("failed to parse reward configuration JSON")?;
    RewardConfig::try_from(state)
}
fn read_metrics_file(path: &Path) -> Result<RelayEpochMetricsV1> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read metrics from `{}`", path.display()))?;
    norito::decode_from_bytes(&bytes).wrap_err("failed to decode RelayEpochMetricsV1 payload")
}
fn read_bond_entry(path: &Path) -> Result<RelayBondLedgerEntryV1> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read bond entry from `{}`", path.display()))?;
    norito::decode_from_bytes(&bytes).wrap_err("failed to decode RelayBondLedgerEntryV1 payload")
}
fn read_reward_instruction(path: &Path) -> Result<RelayRewardInstructionV1> {
    let bytes = fs::read(path).wrap_err_with(|| {
        format!(
            "failed to read reward instruction from `{}`",
            path.display()
        )
    })?;
    norito::decode_from_bytes(&bytes).wrap_err("failed to decode RelayRewardInstructionV1 payload")
}
fn read_ledger_export(path: &Path) -> Result<LedgerExportFile> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read ledger export from `{}`", path.display()))?;
    let export: LedgerExportFile = norito::decode_from_bytes(&bytes)
        .map_err(|err| {
            if matches!(err, norito::Error::SchemaMismatch) {
                const SCHEMA_OFFSET: usize = 4 + 1 + 1;
                const SCHEMA_LEN: usize = 16;
                let expected = LedgerExportFile::schema_hash();
                let actual = bytes
                    .get(SCHEMA_OFFSET..SCHEMA_OFFSET + SCHEMA_LEN)
                    .map(|slice| {
                        let mut buf = [0_u8; SCHEMA_LEN];
                        buf.copy_from_slice(slice);
                        buf
                    })
                    .map_or_else(|| "<missing>".to_string(), hex::encode);
                eyre!(
                    "schema mismatch (expected {}, got {actual})",
                    hex::encode(expected)
                )
            } else {
                eyre!(err)
            }
        })
        .wrap_err("failed to decode ledger export payload")?;
    export.ensure_current()?;
    Ok(export)
}
fn write_norito_payload<T>(path: &Path, value: &T) -> Result<()>
where
    T: NoritoSerialize,
{
    let bytes = norito::to_bytes(value).wrap_err("failed to encode Norito payload")?;
    fs::write(path, bytes)
        .wrap_err_with(|| format!("failed to write Norito payload to `{}`", path.display()))
}
fn load_daemon_config(
    path: &Path,
    resolve: &dyn Fn(&str) -> Result<AccountId>,
) -> Result<DaemonConfig> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read daemon config from `{}`", path.display()))?;
    let file: DaemonConfigFile =
        norito::json::from_slice(&bytes).wrap_err("failed to parse daemon config JSON")?;
    let base_dir = path
        .parent()
        .map_or_else(|| Path::new(".").to_path_buf(), Path::to_path_buf);
    let mut relays = HashMap::new();
    for entry in file.relays {
        let normalised = validate_hex_digest(&entry.relay_id, "daemon_config.relays[].relay_id")
            .map_err(|err| eyre!("invalid relay_id `{}`: {err}", entry.relay_id))?;
        let mut relay_id = [0_u8; 32];
        decode_to_slice(&normalised, &mut relay_id)
            .map_err(|err| eyre!("failed to decode relay_id `{}`: {err}", entry.relay_id))?;
        let beneficiary = resolve(entry.beneficiary.trim()).map_err(|err| {
            eyre!(
                "invalid beneficiary `{}` for relay {}: {err}",
                entry.beneficiary,
                normalised
            )
        })?;
        let bond_path = resolve_relative_path(&base_dir, entry.bond_path.trim());
        let relay_entry = DaemonRelayEntry {
            relay_hex: normalised.clone(),
            beneficiary,
            bond_path,
        };
        if relays.insert(relay_id, relay_entry).is_some() {
            return Err(eyre!(
                "duplicate daemon config entry for relay {}",
                normalised
            ));
        }
    }
    Ok(DaemonConfig { relays })
}
fn audit_scope_flags(scopes: &[IncentiveAuditScope]) -> (bool, bool) {
    let mut bond = false;
    let mut budget = false;
    if scopes.is_empty() {
        return (true, false);
    }
    for scope in scopes {
        match scope {
            IncentiveAuditScope::Bond => bond = true,
            IncentiveAuditScope::Budget => budget = true,
            IncentiveAuditScope::All => {
                bond = true;
                budget = true;
            }
        }
    }
    if !bond && !budget {
        bond = true;
    }
    (bond, budget)
}
fn audit_bonds(
    config: &DaemonConfig,
    reward_config: &RewardConfigState,
) -> Result<BondAuditSummary> {
    let policy = RelayBondPolicyV1::try_from(reward_config.policy.clone())
        .map_err(|err| eyre!("invalid reward policy in state: {err}"))?;
    let mut summary = BondAuditSummary {
        total_relays: config.relays.len(),
        policy_minimum_exit_bond: reward_config.policy.minimum_exit_bond.clone(),
        policy_bond_asset_id: reward_config.policy.bond_asset_id.clone(),
        ..BondAuditSummary::default()
    };
    for entry in config.relays.values() {
        let bond_entry = match read_bond_entry(&entry.bond_path) {
            Ok(entry) => entry,
            Err(err) => {
                summary.missing_bond = summary.missing_bond.saturating_add(1);
                summary.errors.push(format!(
                    "relay {} bond missing or unreadable at `{}`: {err}",
                    entry.relay_hex,
                    entry.bond_path.display()
                ));
                continue;
            }
        };
        if bond_entry.exit_capable {
            summary.exit_relays = summary.exit_relays.saturating_add(1);
        }
        if bond_entry.meets_exit_minimum(&policy) {
            summary.satisfied = summary.satisfied.saturating_add(1);
            continue;
        }
        if bond_entry.bond_asset_id != policy.bond_asset_id {
            summary.asset_mismatch = summary.asset_mismatch.saturating_add(1);
            summary.errors.push(format!(
                "relay {} bond uses asset {} (expected {})",
                entry.relay_hex, bond_entry.bond_asset_id, policy.bond_asset_id
            ));
            continue;
        }
        summary.insufficient_bond = summary.insufficient_bond.saturating_add(1);
        summary.errors.push(format!(
            "relay {} bonded {} below minimum {}",
            entry.relay_hex, bond_entry.bonded_amount, policy.minimum_exit_bond
        ));
    }
    Ok(summary)
}
#[allow(clippy::unnecessary_wraps)]
fn audit_budget(state: &IncentivesState) -> Result<BudgetAuditSummary> {
    let mut summary = BudgetAuditSummary {
        configured_budget_approval_id: state.reward_config.budget_approval_id.clone(),
        total_payouts: state.payouts.len(),
        ..BudgetAuditSummary::default()
    };
    let expected_budget =
        match require_budget_approval_id(state.reward_config.budget_approval_id.as_ref()) {
            Ok(id) => id,
            Err(_) => return Ok(summary),
        };
    for payout in &state.payouts {
        match payout.budget_approval_id {
            Some(value) if value == expected_budget => {}
            Some(_) => {
                summary.mismatched_budget_approval =
                    summary.mismatched_budget_approval.saturating_add(1);
            }
            None => {
                summary.payouts_without_budget = summary.payouts_without_budget.saturating_add(1);
            }
        }
    }
    Ok(summary)
}
fn resolve_relative_path(base: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
fn process_daemon_iteration(
    state: &mut IncentivesState,
    service: &mut RelayPayoutService,
    config: &DaemonConfig,
    metrics_dir: &Path,
    instruction_out_dir: Option<&Path>,
    transfer_out_dir: Option<&Path>,
    archive_dir: Option<&Path>,
    expected_budget: Option<&[u8; 32]>,
) -> Result<DaemonIterationSummary> {
    let mut summary = DaemonIterationSummary {
        expected_budget_approval: expected_budget.map(hex::encode),
        ..DaemonIterationSummary::default()
    };
    let entries = match fs::read_dir(metrics_dir) {
        Ok(entries) => entries,
        Err(err) => {
            return Err(err).wrap_err_with(|| {
                format!(
                    "failed to read metrics directory `{}`",
                    metrics_dir.display()
                )
            });
        }
    };
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                summary
                    .errors
                    .push(format!("failed to read metrics directory entry: {err}"));
                continue;
            }
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default();
        if !extension.eq_ignore_ascii_case("to") {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_string();
        let stem = stem.trim();
        if !stem.starts_with("relay-") {
            summary.errors.push(format!(
                "metrics file `{file_name}` does not start with `relay-`; skipping"
            ));
            continue;
        }
        let relay_split = if let Some(split) = stem[6..].split_once("-epoch-") {
            split
        } else {
            summary.errors.push(format!(
                "metrics file `{file_name}` is missing `-epoch-` segment; skipping"
            ));
            continue;
        };
        let relay_hex_raw = relay_split.0;
        let epoch_segment = relay_split.1;
        let epoch_str = epoch_segment
            .split(['-', '.'])
            .next()
            .unwrap_or(epoch_segment);
        let epoch = match epoch_str.parse::<u32>() {
            Ok(epoch) => epoch,
            Err(err) => {
                summary.errors.push(format!(
                    "metrics file `{file_name}` contains invalid epoch `{epoch_str}`: {err}"
                ));
                continue;
            }
        };
        let normalised = match validate_hex_digest(relay_hex_raw, "metrics relay id") {
            Ok(hex) => hex,
            Err(err) => {
                summary.errors.push(format!(
                    "metrics file `{file_name}` has invalid relay id `{relay_hex_raw}`: {err}"
                ));
                continue;
            }
        };
        let mut relay_id = [0_u8; 32];
        if let Err(err) = decode_to_slice(&normalised, &mut relay_id) {
            summary.errors.push(format!(
                "metrics file `{file_name}` has undecodable relay id `{relay_hex_raw}`: {err}"
            ));
            continue;
        }
        candidates.push(MetricsCandidate {
            relay_id,
            relay_hex: normalised,
            epoch,
            path,
            file_name,
        });
    }
    candidates.sort_by(|left, right| {
        left.epoch
            .cmp(&right.epoch)
            .then_with(|| left.relay_hex.cmp(&right.relay_hex))
    });
    for candidate in candidates {
        let Some(relay_entry) = config.entry(&candidate.relay_id) else {
            summary.skipped_missing_config = summary.skipped_missing_config.saturating_add(1);
            summary.errors.push(format!(
                "no daemon config entry found for relay {} (metrics `{}`).",
                candidate.relay_hex, candidate.file_name
            ));
            continue;
        };
        let bond_entry = match read_bond_entry(&relay_entry.bond_path) {
            Ok(entry) => entry,
            Err(err) => {
                summary.skipped_missing_bond = summary.skipped_missing_bond.saturating_add(1);
                summary.errors.push(format!(
                    "failed to load bond entry for relay {} from `{}`: {err}",
                    relay_entry.relay_hex,
                    relay_entry.bond_path.display()
                ));
                continue;
            }
        };
        let metrics = match read_metrics_file(&candidate.path) {
            Ok(metrics) => metrics,
            Err(err) => {
                summary.errors.push(format!(
                    "failed to decode metrics snapshot `{}`: {err}",
                    candidate.file_name
                ));
                continue;
            }
        };
        if metrics.relay_id != candidate.relay_id {
            summary.errors.push(format!(
                "metrics snapshot `{}` relay id mismatch (expected {}, found {})",
                candidate.file_name,
                candidate.relay_hex,
                hex::encode(metrics.relay_id)
            ));
            continue;
        }
        if metrics.epoch != candidate.epoch {
            summary.errors.push(format!(
                "metrics snapshot `{}` epoch mismatch (expected {}, found {})",
                candidate.file_name, candidate.epoch, metrics.epoch
            ));
            continue;
        }
        let outcome = match service.process_epoch(
            &metrics,
            &bond_entry,
            relay_entry.beneficiary.clone(),
            Metadata::default(),
        ) {
            Ok(outcome) => outcome,
            Err(PayoutServiceError::Ledger(RewardLedgerError::DuplicateEpoch { .. })) => {
                summary.skipped_duplicate = summary.skipped_duplicate.saturating_add(1);
                continue;
            }
            Err(err) => {
                summary.errors.push(format!(
                    "failed to process metrics `{}`: {err}",
                    candidate.file_name
                ));
                continue;
            }
        };
        if let Some(expected) = expected_budget {
            if let Err(err) = ensure_instruction_budget_approval(&outcome.instruction, expected) {
                if outcome.instruction.budget_approval_id.is_some() {
                    summary.mismatched_budget_approval =
                        summary.mismatched_budget_approval.saturating_add(1);
                } else {
                    summary.missing_budget_approval =
                        summary.missing_budget_approval.saturating_add(1);
                }
                summary.errors.push(err.to_string());
            }
        } else if outcome.instruction.budget_approval_id.is_none() {
            summary.missing_budget_approval = summary.missing_budget_approval.saturating_add(1);
        }
        store_payout_instruction(state, &outcome.instruction);
        let metrics_snapshot = extract_payout_metrics(&outcome.instruction, &metrics);
        let budget_approval_id = outcome.instruction.budget_approval_id.map(hex::encode);
        let instruction_path = if let Some(dir) = instruction_out_dir {
            let file_name = format!(
                "relay-{}-epoch-{}.reward.to",
                relay_entry.relay_hex, candidate.epoch
            );
            let path = dir.join(&file_name);
            match write_norito_payload(&path, &outcome.instruction) {
                Ok(()) => Some(path.to_string_lossy().into_owned()),
                Err(err) => {
                    summary.errors.push(format!(
                        "failed to write reward instruction `{}`: {err}",
                        path.display()
                    ));
                    None
                }
            }
        } else {
            None
        };
        let transfer_path = if let Some(dir) = transfer_out_dir {
            if outcome.instruction.is_zero_amount() {
                None
            } else {
                let file_name = format!(
                    "relay-{}-epoch-{}.transfer.to",
                    relay_entry.relay_hex, candidate.epoch
                );
                let path = dir.join(&file_name);
                match write_norito_payload(&path, &outcome.transfer) {
                    Ok(()) => Some(path.to_string_lossy().into_owned()),
                    Err(err) => {
                        summary.errors.push(format!(
                            "failed to write transfer instruction `{}`: {err}",
                            path.display()
                        ));
                        None
                    }
                }
            }
        } else {
            None
        };
        let metrics_archived_to = if let Some(dir) = archive_dir {
            match archive_metrics_snapshot(&candidate.path, dir, &candidate.file_name) {
                Ok(archived_path) => Some(archived_path.to_string_lossy().into_owned()),
                Err(err) => {
                    summary.errors.push(err.to_string());
                    None
                }
            }
        } else {
            None
        };
        summary.processed.push(DaemonProcessedPayoutSummary {
            relay_id_hex: relay_entry.relay_hex.clone(),
            epoch: candidate.epoch,
            payout_amount: outcome.instruction.payout_amount,
            budget_approval_id,
            metrics: metrics_snapshot,
            instruction_path,
            transfer_path,
            metrics_archived_to,
        });
    }
    if summary.missing_budget_approval > 0 && summary.expected_budget_approval.is_some() {
        summary.errors.push(format!(
            "{} payout(s) missing budget_approval_id; set reward_config.budget_approval_id to the signed Parliament hash",
            summary.missing_budget_approval
        ));
    }
    Ok(summary)
}
fn archive_metrics_snapshot(path: &Path, archive_dir: &Path, file_name: &str) -> Result<PathBuf> {
    let mut attempt = 0_u32;
    loop {
        let candidate = if attempt == 0 {
            archive_dir.join(file_name)
        } else {
            archive_dir.join(format!("{file_name}.{attempt}"))
        };
        match fs::rename(path, &candidate) {
            Ok(()) => return Ok(candidate),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                attempt = attempt.saturating_add(1);
            }
            Err(err) => {
                return Err(err).wrap_err_with(|| {
                    format!(
                        "failed to archive metrics snapshot `{}` to `{}`",
                        path.display(),
                        candidate.display()
                    )
                });
            }
        }
    }
}
fn log_daemon_summary<C: RunContext>(
    context: &mut C,
    summary: &DaemonIterationSummary,
    pretty: bool,
) -> Result<()> {
    match context.output_format() {
        CliOutputFormat::Json => {
            context.print_data(summary)?;
        }
        CliOutputFormat::Text => {
            if pretty {
                context.print_data(summary)?;
            } else {
                let _ = context.println(format_args!(
                    "Processed {} payout(s); skipped {} missing config, {} missing bond, {} duplicate.",
                    summary.processed.len(),
                    summary.skipped_missing_config,
                    summary.skipped_missing_bond,
                    summary.skipped_duplicate
                ));
                if summary.missing_budget_approval > 0 {
                    let _ = context.println(format_args!(
                        "  missing budget approval id on {} payout(s)",
                        summary.missing_budget_approval
                    ));
                }
                if summary.mismatched_budget_approval > 0 {
                    let _ = context.println(format_args!(
                        "  mismatched budget approval id on {} payout(s)",
                        summary.mismatched_budget_approval
                    ));
                }
                if let Some(expected) = &summary.expected_budget_approval {
                    let _ =
                        context.println(format_args!("  expected budget approval id: {expected}"));
                }
                for payout in &summary.processed {
                    let _ = context.println(format_args!(
                        "  relay {} epoch {} payout {}",
                        payout.relay_id_hex, payout.epoch, payout.payout_amount
                    ));
                    if let Some(budget) = &payout.budget_approval_id {
                        let _ = context.println(format_args!("    budget approval: {budget}"));
                    } else {
                        let _ = context.println("    budget approval: <missing>");
                    }
                    if let Some(path) = &payout.instruction_path {
                        let _ = context.println(format_args!("    instruction: {path}"));
                    }
                    if let Some(path) = &payout.transfer_path {
                        let _ = context.println(format_args!("    transfer: {path}"));
                    }
                    if let Some(path) = &payout.metrics_archived_to {
                        let _ = context.println(format_args!("    archived metrics: {path}"));
                    }
                }
                if !summary.errors.is_empty() {
                    let _ = context.println(format_args!(
                        "Encountered {} error(s):",
                        summary.errors.len()
                    ));
                    for err in &summary.errors {
                        let _ = context.println(format_args!("  - {err}"));
                    }
                }
            }
        }
    }
    if summary.expected_budget_approval.is_some() && summary.missing_budget_approval > 0 {
        return Err(eyre!(
            "budget_approval_id missing for {} payout(s); configure reward_config.budget_approval_id before running payouts",
            summary.missing_budget_approval
        ));
    }
    Ok(())
}
#[derive(
    Debug,
    Clone,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
struct RewardConfigState {
    policy: RewardPolicyState,
    base_reward: String,
    uptime_weight_per_mille: u16,
    bandwidth_weight_per_mille: u16,
    compliance_penalty_basis_points: u16,
    bandwidth_target_bytes: u128,
    budget_approval_id: Option<String>,
    metrics_log_path: Option<String>,
}
#[derive(
    Debug,
    Clone,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
struct RewardPolicyState {
    minimum_exit_bond: String,
    bond_asset_id: String,
    uptime_floor_per_mille: u16,
    slash_penalty_basis_points: u16,
    activation_grace_epochs: u16,
}
impl From<&RewardConfig> for RewardConfigState {
    fn from(config: &RewardConfig) -> Self {
        Self {
            policy: RewardPolicyState::from(&config.policy),
            base_reward: config.base_reward.to_string(),
            uptime_weight_per_mille: config.uptime_weight_per_mille,
            bandwidth_weight_per_mille: config.bandwidth_weight_per_mille,
            compliance_penalty_basis_points: config.compliance_penalty_basis_points,
            bandwidth_target_bytes: config.bandwidth_target_bytes,
            budget_approval_id: config.budget_approval_id.map(hex::encode),
            metrics_log_path: config
                .metrics_log_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
        }
    }
}
impl TryFrom<RewardConfigState> for RewardConfig {
    type Error = eyre::Report;
    fn try_from(value: RewardConfigState) -> Result<Self> {
        let RewardConfigState {
            policy: policy_state,
            base_reward,
            uptime_weight_per_mille,
            bandwidth_weight_per_mille,
            compliance_penalty_basis_points,
            bandwidth_target_bytes,
            budget_approval_id,
            metrics_log_path,
        } = value;
        let policy = RelayBondPolicyV1::try_from(policy_state)?;
        let base_reward =
            Quantity::from_str(&base_reward).map_err(|err| eyre!("invalid base_reward: {err}"))?;
        let budget_approval_id = match budget_approval_id {
            Some(hex_value) => {
                let normalised =
                    validate_hex_digest(&hex_value, "reward_config.budget_approval_id")?;
                let mut digest = [0_u8; 32];
                decode_to_slice(normalised, &mut digest)
                    .map_err(|err| eyre!("invalid budget_approval_id hex: {err}"))?;
                Some(digest)
            }
            None => None,
        };
        let metrics_log_path = metrics_log_path.map(PathBuf::from);
        Ok(Self {
            policy,
            base_reward,
            uptime_weight_per_mille,
            bandwidth_weight_per_mille,
            compliance_penalty_basis_points,
            bandwidth_target_bytes,
            budget_approval_id,
            metrics_log_path,
        })
    }
}
impl From<&RelayBondPolicyV1> for RewardPolicyState {
    fn from(policy: &RelayBondPolicyV1) -> Self {
        Self {
            minimum_exit_bond: policy.minimum_exit_bond.to_string(),
            bond_asset_id: policy.bond_asset_id.to_string(),
            uptime_floor_per_mille: policy.uptime_floor_per_mille,
            slash_penalty_basis_points: policy.slash_penalty_basis_points,
            activation_grace_epochs: policy.activation_grace_epochs,
        }
    }
}
impl TryFrom<RewardPolicyState> for RelayBondPolicyV1 {
    type Error = eyre::Report;
    fn try_from(value: RewardPolicyState) -> Result<Self> {
        let minimum_exit_bond = Quantity::from_str(&value.minimum_exit_bond)
            .map_err(|err| eyre!("invalid minimum_exit_bond: {err}"))?;
        let bond_asset_id = AssetDefinitionId::parse_address_literal(&value.bond_asset_id)
            .map_err(|err| eyre!("invalid bond_asset_id: {err}"))?;
        Ok(Self {
            minimum_exit_bond,
            bond_asset_id,
            uptime_floor_per_mille: value.uptime_floor_per_mille,
            slash_penalty_basis_points: value.slash_penalty_basis_points,
            activation_grace_epochs: value.activation_grace_epochs,
        })
    }
}
fn require_budget_approval_id(budget_hex: Option<&String>) -> Result<[u8; 32]> {
    let budget_hex = budget_hex
        .map(String::as_str)
        .ok_or_else(|| eyre!("reward_config.budget_approval_id is required for incentives"))?;
    let normalised = validate_hex_digest(budget_hex, "reward_config.budget_approval_id")?;
    let mut digest = [0_u8; 32];
    decode_to_slice(normalised, &mut digest)
        .map_err(|err| eyre!("invalid budget_approval_id hex: {err}"))?;
    Ok(digest)
}
#[derive(
    Debug,
    Clone,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito(decode_from_slice)]
struct IncentivesState {
    version: u16,
    reward_config: RewardConfigState,
    treasury_account: AccountId,
    payouts: Vec<RelayRewardInstructionV1>,
    disputes: Vec<StoredDisputeRecord>,
}
#[derive(
    Debug,
    Clone,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito(decode_from_slice)]
struct LedgerExportFile {
    version: u16,
    transfers: Vec<LedgerTransferRecord>,
}
impl LedgerExportFile {
    const VERSION: u16 = 1;
    fn ensure_current(&self) -> Result<()> {
        if self.version != Self::VERSION {
            return Err(eyre!(
                "unsupported ledger export version {} (expected {})",
                self.version,
                Self::VERSION
            ));
        }
        Ok(())
    }
}
impl IncentivesState {
    const VERSION: u16 = 1;
    fn new(reward_config: &RewardConfig, treasury_account: AccountId) -> Self {
        Self {
            version: Self::VERSION,
            reward_config: RewardConfigState::from(reward_config),
            treasury_account,
            payouts: Vec::new(),
            disputes: Vec::new(),
        }
    }
    fn ensure_current(&self) -> Result<()> {
        if self.version != Self::VERSION {
            return Err(eyre!(
                "unsupported incentives state version {} (expected {})",
                self.version,
                Self::VERSION
            ));
        }
        Ok(())
    }
}
#[derive(
    Debug,
    Clone,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito(decode_from_slice)]
struct StoredDisputeRecord {
    id: DisputeId,
    relay_id_hex: String,
    epoch: u32,
    submitted_by: AccountId,
    requested_amount: Quantity,
    filed_at_unix: u64,
    reason: String,
    requested_adjustment: Option<StoredAdjustmentRequest>,
    status: StoredDisputeStatus,
}
#[derive(
    Debug,
    Clone,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito(decode_from_slice)]
struct StoredAdjustmentRequest {
    kind: StoredAdjustmentKind,
    amount: Quantity,
}
impl StoredAdjustmentRequest {
    fn to_adjustment_request(&self) -> AdjustmentRequest {
        AdjustmentRequest {
            kind: self.kind.into(),
            amount: self.amount.clone(),
        }
    }
}
impl From<&AdjustmentRequest> for StoredAdjustmentRequest {
    fn from(request: &AdjustmentRequest) -> Self {
        Self {
            kind: request.kind.into(),
            amount: request.amount.clone(),
        }
    }
}
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito(tag = "kind", content = "details")]
#[norito(decode_from_slice)]
enum StoredAdjustmentKind {
    Credit,
    Debit,
}
impl From<AdjustmentKind> for StoredAdjustmentKind {
    fn from(kind: AdjustmentKind) -> Self {
        match kind {
            AdjustmentKind::Credit => Self::Credit,
            AdjustmentKind::Debit => Self::Debit,
        }
    }
}
impl From<StoredAdjustmentKind> for AdjustmentKind {
    fn from(kind: StoredAdjustmentKind) -> Self {
        match kind {
            StoredAdjustmentKind::Credit => AdjustmentKind::Credit,
            StoredAdjustmentKind::Debit => AdjustmentKind::Debit,
        }
    }
}
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito(tag = "kind", content = "details")]
#[norito(decode_from_slice)]
enum StoredResolutionKind {
    NoChange,
    Credit,
    Debit,
}
impl From<ResolutionKind> for StoredResolutionKind {
    fn from(kind: ResolutionKind) -> Self {
        match kind {
            ResolutionKind::NoChange => Self::NoChange,
            ResolutionKind::Credit => Self::Credit,
            ResolutionKind::Debit => Self::Debit,
        }
    }
}
#[derive(
    Debug,
    Clone,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito(tag = "status", content = "details")]
#[norito(decode_from_slice)]
enum StoredDisputeStatus {
    Open,
    Rejected {
        rejected_at_unix: u64,
        notes: String,
    },
    Resolved {
        resolved_at_unix: u64,
        kind: StoredResolutionKind,
        amount: Option<Quantity>,
        notes: String,
    },
}
impl From<&DisputeStatus> for StoredDisputeStatus {
    fn from(status: &DisputeStatus) -> Self {
        match status {
            DisputeStatus::Open => Self::Open,
            DisputeStatus::Rejected {
                rejected_at_unix,
                notes,
            } => Self::Rejected {
                rejected_at_unix: *rejected_at_unix,
                notes: notes.clone(),
            },
            DisputeStatus::Resolved {
                resolved_at_unix,
                outcome,
            } => Self::Resolved {
                resolved_at_unix: *resolved_at_unix,
                kind: outcome.kind.into(),
                amount: outcome.amount.clone(),
                notes: outcome.notes.clone(),
            },
        }
    }
}
impl From<&RewardDispute> for StoredDisputeRecord {
    fn from(dispute: &RewardDispute) -> Self {
        let norito_record = dispute.norito_record();
        Self {
            id: dispute.id,
            relay_id_hex: relay_id_to_hex(dispute.relay_id),
            epoch: dispute.epoch,
            submitted_by: norito_record.submitted_by.clone(),
            requested_amount: norito_record.requested_amount.clone(),
            filed_at_unix: dispute.filed_at_unix,
            reason: dispute.reason.clone(),
            requested_adjustment: dispute
                .requested_adjustment
                .as_ref()
                .map(StoredAdjustmentRequest::from),
            status: StoredDisputeStatus::from(&dispute.status),
        }
    }
}
impl StoredDisputeRecord {
    fn apply_to_service(&self, service: &mut RelayPayoutService) -> Result<()> {
        let relay_id = relay_id_from_hex(&self.relay_id_hex)
            .wrap_err_with(|| format!("invalid relay id for dispute {}", self.id))?;
        let requested_adjustment = self
            .requested_adjustment
            .as_ref()
            .map(StoredAdjustmentRequest::to_adjustment_request);
        let dispute = service
            .file_dispute(
                relay_id,
                self.epoch,
                self.submitted_by.clone(),
                self.requested_amount.clone(),
                self.reason.clone(),
                self.filed_at_unix,
                requested_adjustment,
            )
            .wrap_err_with(|| format!("failed to replay dispute {}", self.id))?;
        if dispute.id != self.id {
            return Err(eyre!(
                "dispute id mismatch when replaying state: expected {}, got {}",
                self.id,
                dispute.id
            ));
        }
        match &self.status {
            StoredDisputeStatus::Open => Ok(()),
            StoredDisputeStatus::Rejected {
                rejected_at_unix,
                notes,
            } => service
                .reject_dispute(self.id, *rejected_at_unix, notes.clone())
                .map(|_| ())
                .wrap_err_with(|| format!("failed to replay rejection for dispute {}", self.id)),
            StoredDisputeStatus::Resolved {
                resolved_at_unix,
                kind,
                amount,
                notes,
            } => {
                let resolution = stored_resolution_to_resolution(*kind, amount.clone(), notes)
                    .wrap_err_with(|| format!("invalid resolution for dispute {}", self.id))?;
                service
                    .resolve_dispute(self.id, resolution, *resolved_at_unix)
                    .map(|_| ())
                    .wrap_err_with(|| {
                        format!("failed to replay resolution for dispute {}", self.id)
                    })
            }
        }
    }
}
fn stored_resolution_to_resolution(
    kind: StoredResolutionKind,
    amount: Option<Quantity>,
    notes: &str,
) -> Result<DisputeResolution> {
    Ok(match kind {
        StoredResolutionKind::NoChange => DisputeResolution::NoChange {
            notes: notes.to_owned(),
        },
        StoredResolutionKind::Credit => DisputeResolution::Credit {
            amount: amount.ok_or_else(|| eyre!("credit resolution requires an amount"))?,
            notes: notes.to_owned(),
        },
        StoredResolutionKind::Debit => DisputeResolution::Debit {
            amount: amount.ok_or_else(|| eyre!("debit resolution requires an amount"))?,
            notes: notes.to_owned(),
        },
    })
}
fn parse_incentives_state_snapshot(bytes: &[u8]) -> Result<IncentivesState> {
    norito::json::from_slice(bytes).wrap_err("failed to parse incentives state JSON")
}
fn load_incentives_state(path: &Path) -> Result<IncentivesState> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read incentives state from `{}`", path.display()))?;
    let state = parse_incentives_state_snapshot(&bytes)?;
    state.ensure_current()?;
    Ok(state)
}
fn save_incentives_state(path: &Path, state: &IncentivesState) -> Result<()> {
    let bytes =
        norito::json::to_vec_pretty(state).wrap_err("failed to render incentives state JSON")?;
    fs::write(path, bytes)
        .wrap_err_with(|| format!("failed to write incentives state to `{}`", path.display()))
}
fn build_clean_payout_service(state: &IncentivesState) -> Result<RelayPayoutService> {
    let config = RewardConfig::try_from(state.reward_config.clone())
        .map_err(|err| eyre!("invalid reward configuration in state: {err}"))?;
    let engine = RelayRewardEngine::new(config)
        .map_err(|err| eyre!("invalid reward configuration in state: {err}"))?;
    Ok(RelayPayoutService::new(
        engine,
        RelayPayoutLedger::new(state.treasury_account.clone()),
    ))
}
fn build_payout_service(state: &IncentivesState) -> Result<RelayPayoutService> {
    state.ensure_current()?;
    let mut service = build_clean_payout_service(state)?;
    for instruction in &state.payouts {
        service
            .record_reward(instruction.clone())
            .wrap_err_with(|| {
                format!(
                    "failed to replay reward instruction for relay {} epoch {}",
                    hex::encode(instruction.relay_id),
                    instruction.epoch
                )
            })?;
    }
    let mut disputes = state.disputes.clone();
    disputes.sort_by_key(|d| d.id);
    for dispute in disputes {
        dispute.apply_to_service(&mut service)?;
    }
    Ok(service)
}
fn store_payout_instruction(state: &mut IncentivesState, instruction: &RelayRewardInstructionV1) {
    state.payouts.push(instruction.clone());
}
fn upsert_dispute_record(state: &mut IncentivesState, dispute: &RewardDispute) {
    let record = StoredDisputeRecord::from(dispute);
    if let Some(existing) = state
        .disputes
        .iter_mut()
        .find(|entry| entry.id == record.id)
    {
        *existing = record;
    } else {
        state.disputes.push(record);
        state.disputes.sort_by_key(|entry| entry.id);
    }
}
fn relay_id_to_hex(relay_id: RelayId) -> String {
    hex::encode(relay_id)
}
fn saturating_u16(value: u64) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}
#[allow(clippy::cast_precision_loss)]
fn u64_to_f64(value: u64) -> f64 {
    value as f64
}
#[allow(clippy::cast_precision_loss)]
fn u128_to_f64(value: u128) -> f64 {
    value as f64
}
#[allow(clippy::cast_precision_loss)]
fn usize_to_f64(value: usize) -> f64 {
    value as f64
}
fn transfer_kind_label(kind: TransferKind) -> &'static str {
    match kind {
        TransferKind::Payout => "payout",
        TransferKind::Credit => "credit",
        TransferKind::Debit => "debit",
    }
}
fn mismatch_reason_label(reason: MismatchReason) -> &'static str {
    match reason {
        MismatchReason::Amount => "amount",
        MismatchReason::SourceAsset => "source_asset",
        MismatchReason::Destination => "destination",
    }
}
fn ledger_amount_source_label(source: LedgerAmountSource) -> &'static str {
    match source {
        LedgerAmountSource::Expected => "expected",
        LedgerAmountSource::Exported => "exported",
    }
}
fn quantity_to_nanos_error_label(error: QuantityToNanosError) -> &'static str {
    match error {
        QuantityToNanosError::TooWideMantissa => "too_wide_mantissa",
        QuantityToNanosError::ScaleOverflow => "scale_overflow",
        QuantityToNanosError::InexactNanos => "inexact_nanos",
        QuantityToNanosError::NanosOverflow => "nanos_overflow",
        QuantityToNanosError::TotalOverflow => "total_overflow",
    }
}
fn quantity_to_nanos_checked(amount: &Quantity) -> Result<u128, QuantityToNanosError> {
    let scale = amount.scale();
    let mantissa = amount
        .as_numeric()
        .try_mantissa_u128()
        .ok_or(QuantityToNanosError::TooWideMantissa)?;
    if scale >= 9 {
        let divisor = 10u128
            .checked_pow(scale.saturating_sub(9))
            .ok_or(QuantityToNanosError::ScaleOverflow)?;
        if mantissa % divisor != 0 {
            return Err(QuantityToNanosError::InexactNanos);
        }
        Ok(mantissa / divisor)
    } else {
        let multiplier = 10u128
            .checked_pow(9 - scale)
            .ok_or(QuantityToNanosError::ScaleOverflow)?;
        mantissa
            .checked_mul(multiplier)
            .ok_or(QuantityToNanosError::NanosOverflow)
    }
}
fn metadata_get_u64(metadata: &Metadata, key: &str) -> Option<u64> {
    let name = Name::from_str(key).ok()?;
    metadata.get(&name)?.try_into_any::<u64>().ok()
}
fn metadata_get_bool(metadata: &Metadata, key: &str) -> Option<bool> {
    let name = Name::from_str(key).ok()?;
    metadata.get(&name)?.try_into_any::<bool>().ok()
}
fn extract_payout_metrics(
    instruction: &RelayRewardInstructionV1,
    metrics: &RelayEpochMetricsV1,
) -> PayoutMetricsSnapshot {
    let availability_raw = metadata_get_u64(&instruction.metadata, "availability_per_mille")
        .unwrap_or_else(|| u64::from(metrics.uptime_ratio_per_mille()));
    let bandwidth_raw = metadata_get_u64(&instruction.metadata, "bandwidth_per_mille").unwrap_or(0);
    let compliance_raw = metadata_get_u64(&instruction.metadata, "compliance_per_mille").unwrap_or(
        match metrics.compliance {
            RelayComplianceStatusV1::Clean => 1_000,
            RelayComplianceStatusV1::Warning => 900,
            RelayComplianceStatusV1::Suspended => 0,
        },
    );
    let exit_bonus_applied =
        metadata_get_bool(&instruction.metadata, "exit_bonus_applied").unwrap_or(false);
    let score_per_mille = instruction.reward_score.try_into().unwrap_or(u16::MAX);
    let compliance_status = match metrics.compliance {
        RelayComplianceStatusV1::Clean => "clean",
        RelayComplianceStatusV1::Warning => "warning",
        RelayComplianceStatusV1::Suspended => "suspended",
    }
    .to_string();
    PayoutMetricsSnapshot {
        availability_per_mille: saturating_u16(availability_raw),
        bandwidth_per_mille: saturating_u16(bandwidth_raw),
        compliance_per_mille: saturating_u16(compliance_raw),
        compliance_status,
        score_per_mille,
        exit_bonus_applied,
    }
}
fn relay_id_from_hex(value: &str) -> Result<RelayId> {
    if value.len() != 64 {
        return Err(eyre!("relay id must be 64 hex characters"));
    }
    let mut bytes = [0_u8; 32];
    decode_to_slice(value, &mut bytes)
        .map_err(|err| eyre!("failed to decode relay id hex: {err}"))?;
    Ok(bytes)
}
fn ensure_instruction_budget_approval(
    instruction: &RelayRewardInstructionV1,
    expected_budget: &[u8; 32],
) -> Result<()> {
    match instruction.budget_approval_id {
        Some(value) if value == *expected_budget => Ok(()),
        Some(value) => Err(eyre!(
            "reward instruction for relay {} epoch {} carries unexpected budget_approval_id {} (expected {})",
            hex::encode(instruction.relay_id),
            instruction.epoch,
            hex::encode(value),
            hex::encode(expected_budget)
        )),
        None => Err(eyre!(
            "reward instruction for relay {} epoch {} missing budget_approval_id",
            hex::encode(instruction.relay_id),
            instruction.epoch
        )),
    }
}
fn load_state_service(path: &Path) -> Result<(IncentivesState, RelayPayoutService)> {
    let state = load_incentives_state(path)?;
    let service = build_payout_service(&state)?;
    Ok((state, service))
}
fn parse_adjustment_flags(
    credit: Option<&String>,
    debit: Option<&String>,
) -> Result<Option<AdjustmentRequest>> {
    if let Some(value) = credit {
        let amount = parse_quantity_str(value, "--adjust-credit")?;
        return Ok(Some(AdjustmentRequest {
            kind: AdjustmentKind::Credit,
            amount,
        }));
    }
    if let Some(value) = debit {
        let amount = parse_quantity_str(value, "--adjust-debit")?;
        return Ok(Some(AdjustmentRequest {
            kind: AdjustmentKind::Debit,
            amount,
        }));
    }
    Ok(None)
}
fn output_summary<C, T>(context: &mut C, summary: &T, pretty: bool) -> Result<()>
where
    C: RunContext,
    T: norito::json::JsonSerialize,
{
    match context.output_format() {
        CliOutputFormat::Json => context.print_data(summary),
        CliOutputFormat::Text => {
            if pretty {
                context.print_data(summary)
            } else {
                let bytes = norito::json::to_vec(summary)
                    .map_err(|err| eyre!("failed to serialise summary: {err}"))?;
                let output = String::from_utf8(bytes)
                    .map_err(|err| eyre!("summary JSON is not valid UTF-8: {err}"))?;
                context.println(output)
            }
        }
    }
}
#[derive(Debug, norito::json::JsonSerialize)]
struct ServicePayoutSummary {
    relay_id_hex: String,
    epoch: u32,
    payout_amount: Quantity,
    reward_score: u64,
    ledger: ServiceLedgerSnapshot,
}
impl ServicePayoutSummary {
    fn new(instruction: &RelayRewardInstructionV1, ledger: ServiceLedgerSnapshot) -> Self {
        Self {
            relay_id_hex: relay_id_to_hex(instruction.relay_id),
            epoch: instruction.epoch,
            payout_amount: instruction.payout_amount.clone(),
            reward_score: instruction.reward_score,
            ledger,
        }
    }
}
#[derive(Debug, norito::json::JsonSerialize)]
struct ServiceLedgerSnapshot {
    total_paid: Quantity,
    total_rebated: Quantity,
    total_withheld: Quantity,
    net_paid: Numeric,
    epochs_recorded: usize,
    last_epoch: Option<u32>,
    last_reward_score: Option<u64>,
    open_disputes: usize,
}
impl ServiceLedgerSnapshot {
    fn from_snapshot(snapshot: &RewardLedgerSnapshot) -> Self {
        Self {
            total_paid: snapshot.total_paid.clone(),
            total_rebated: snapshot.total_rebated.clone(),
            total_withheld: snapshot.total_withheld.clone(),
            net_paid: snapshot.net_paid.clone(),
            epochs_recorded: snapshot.epochs_recorded,
            last_epoch: snapshot.last_epoch,
            last_reward_score: snapshot.last_reward_score,
            open_disputes: 0,
        }
    }
    fn from_row(row: &EarningsRow) -> Self {
        Self {
            total_paid: row.total_paid.clone(),
            total_rebated: row.total_rebated.clone(),
            total_withheld: row.total_withheld.clone(),
            net_paid: row.net_paid.clone(),
            epochs_recorded: row.epochs_recorded,
            last_epoch: row.last_epoch,
            last_reward_score: row.last_reward_score,
            open_disputes: row.open_disputes,
        }
    }
}
#[derive(Debug, norito::json::JsonSerialize)]
struct ServiceDashboardSummary {
    total_relays: usize,
    total_open_disputes: usize,
    rows: Vec<ServiceDashboardRow>,
}
#[derive(Debug, norito::json::JsonSerialize)]
struct ReconciliationTransferSummary {
    relay_id: String,
    epoch: u32,
    kind: String,
    dispute_id: Option<DisputeId>,
    amount: String,
    amount_nanos: Option<u128>,
    amount_conversion_error: Option<String>,
    source_asset: String,
    destination: String,
}
impl ReconciliationTransferSummary {
    fn from_record(record: &LedgerTransferRecord) -> Self {
        let (amount_nanos, amount_conversion_error) =
            match quantity_to_nanos_checked(&record.amount) {
                Ok(nanos) => (Some(nanos), None),
                Err(error) => (None, Some(quantity_to_nanos_error_label(error).to_string())),
            };
        Self {
            relay_id: relay_id_to_hex(record.relay_id),
            epoch: record.epoch,
            kind: transfer_kind_label(record.kind).to_string(),
            dispute_id: record.dispute_id,
            amount: record.amount.to_string(),
            amount_nanos,
            amount_conversion_error,
            source_asset: record.source_asset.to_string(),
            destination: record.destination.to_string(),
        }
    }
}
#[derive(Debug, norito::json::JsonSerialize)]
struct ReconciliationMismatchSummary {
    expected: ReconciliationTransferSummary,
    actual: ReconciliationTransferSummary,
    reasons: Vec<String>,
}
impl ReconciliationMismatchSummary {
    fn from_mismatch(mismatch: &LedgerTransferMismatch) -> Self {
        let reasons = mismatch
            .reasons
            .iter()
            .map(|reason| mismatch_reason_label(*reason))
            .map(str::to_string)
            .collect();
        Self {
            expected: ReconciliationTransferSummary::from_record(&mismatch.expected),
            actual: ReconciliationTransferSummary::from_record(&mismatch.actual),
            reasons,
        }
    }
}
#[derive(Debug, norito::json::JsonSerialize)]
struct ReconciliationAmountArithmeticSummary {
    source: String,
    record: ReconciliationTransferSummary,
}
impl ReconciliationAmountArithmeticSummary {
    fn from_error(error: &LedgerAmountArithmeticError) -> Self {
        Self {
            source: ledger_amount_source_label(error.source).to_string(),
            record: ReconciliationTransferSummary::from_record(&error.record),
        }
    }
}
#[derive(Debug, norito::json::JsonSerialize)]
struct ReconciliationReportSummary {
    clean: bool,
    matched_transfers: usize,
    total_expected_transfers: usize,
    expected_amount: String,
    exported_amount: String,
    missing_transfers: Vec<ReconciliationTransferSummary>,
    unexpected_transfers: Vec<ReconciliationTransferSummary>,
    mismatched_transfers: Vec<ReconciliationMismatchSummary>,
    amount_arithmetic_errors: Vec<ReconciliationAmountArithmeticSummary>,
}
impl ReconciliationReportSummary {
    fn from_report(report: &LedgerReconciliationReport) -> Self {
        let missing_transfers = report
            .missing_transfers
            .iter()
            .map(|entry| ReconciliationTransferSummary::from_record(&entry.record))
            .collect();
        let unexpected_transfers = report
            .unexpected_transfers
            .iter()
            .map(ReconciliationTransferSummary::from_record)
            .collect();
        let mismatched_transfers = report
            .mismatched_transfers
            .iter()
            .map(ReconciliationMismatchSummary::from_mismatch)
            .collect();
        let amount_arithmetic_errors = report
            .amount_arithmetic_errors
            .iter()
            .map(ReconciliationAmountArithmeticSummary::from_error)
            .collect();
        Self {
            clean: report.is_clean(),
            matched_transfers: report.matched_transfers,
            total_expected_transfers: report.total_expected_transfers,
            expected_amount: report.expected_amount.to_string(),
            exported_amount: report.exported_amount.to_string(),
            missing_transfers,
            unexpected_transfers,
            mismatched_transfers,
            amount_arithmetic_errors,
        }
    }
}
#[derive(Debug, norito::json::JsonSerialize)]
struct ShadowRunRelaySummary {
    relay_id_hex: String,
    epochs: usize,
    payout_nanos: u128,
    amount_conversion_errors: usize,
    average_payout_nanos: f64,
    average_score_per_mille: f64,
    average_availability_per_mille: f64,
    average_bandwidth_per_mille: f64,
    warning_epochs: usize,
    suspended_epochs: usize,
    zero_score_epochs: usize,
}
#[derive(Debug, norito::json::JsonSerialize)]
struct ShadowRunAmountConversionError {
    relay_id_hex: String,
    epoch: u32,
    amount: String,
    reason: String,
}
#[derive(Debug, norito::json::JsonSerialize)]
struct ShadowRunSummary {
    processed_payouts: usize,
    total_relays: usize,
    total_payout_nanos: u128,
    payout_amount_conversion_errors: Vec<ShadowRunAmountConversionError>,
    gini_coefficient: f64,
    top_relay_share: f64,
    zero_score_epochs: usize,
    warning_epochs: usize,
    suspended_epochs: usize,
    average_availability_per_mille: f64,
    average_bandwidth_per_mille: f64,
    skipped_missing_config: usize,
    skipped_missing_bond: usize,
    skipped_duplicate: usize,
    missing_budget_approval: usize,
    mismatched_budget_approval: usize,
    expected_budget_approval: Option<String>,
    errors: Vec<String>,
    relays: Vec<ShadowRunRelaySummary>,
}
#[allow(clippy::too_many_lines)]
fn build_shadow_run_summary(summary: &DaemonIterationSummary) -> ShadowRunSummary {
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct RelayAccumulator {
        epochs: usize,
        payout_nanos: u128,
        total_score: u64,
        total_availability: u64,
        total_bandwidth: u64,
        warning_epochs: usize,
        suspended_epochs: usize,
        zero_score_epochs: usize,
        amount_conversion_errors: usize,
    }
    let mut accumulators: BTreeMap<&str, RelayAccumulator> = BTreeMap::new();
    let mut payout_amount_conversion_errors = Vec::new();
    let mut payout_totals: Vec<u128> = Vec::new();
    let mut sum_availability = 0_u64;
    let mut sum_bandwidth = 0_u64;
    let mut total_epochs = 0_usize;
    let mut warning_epochs_total = 0_usize;
    let mut suspended_epochs_total = 0_usize;
    let mut zero_score_epochs_total = 0_usize;
    let mut max_relay_payout = 0_u128;
    for payout in &summary.processed {
        let relay_entry = accumulators.entry(&payout.relay_id_hex).or_default();
        let payout_nanos = match quantity_to_nanos_checked(&payout.payout_amount) {
            Ok(nanos) => nanos,
            Err(error) => {
                relay_entry.amount_conversion_errors =
                    relay_entry.amount_conversion_errors.saturating_add(1);
                payout_amount_conversion_errors.push(ShadowRunAmountConversionError {
                    relay_id_hex: payout.relay_id_hex.clone(),
                    epoch: payout.epoch,
                    amount: payout.payout_amount.to_string(),
                    reason: quantity_to_nanos_error_label(error).to_string(),
                });
                0
            }
        };
        relay_entry.epochs = relay_entry.epochs.saturating_add(1);
        relay_entry.payout_nanos = relay_entry.payout_nanos.saturating_add(payout_nanos);
        relay_entry.total_score = relay_entry
            .total_score
            .saturating_add(u64::from(payout.metrics.score_per_mille));
        relay_entry.total_availability = relay_entry
            .total_availability
            .saturating_add(u64::from(payout.metrics.availability_per_mille));
        relay_entry.total_bandwidth = relay_entry
            .total_bandwidth
            .saturating_add(u64::from(payout.metrics.bandwidth_per_mille));
        match payout.metrics.compliance_status.as_str() {
            "warning" => {
                relay_entry.warning_epochs = relay_entry.warning_epochs.saturating_add(1);
                warning_epochs_total = warning_epochs_total.saturating_add(1);
            }
            "suspended" => {
                relay_entry.suspended_epochs = relay_entry.suspended_epochs.saturating_add(1);
                suspended_epochs_total = suspended_epochs_total.saturating_add(1);
            }
            _ => {}
        }
        if payout.metrics.score_per_mille == 0 {
            relay_entry.zero_score_epochs = relay_entry.zero_score_epochs.saturating_add(1);
            zero_score_epochs_total = zero_score_epochs_total.saturating_add(1);
        }
        sum_availability =
            sum_availability.saturating_add(u64::from(payout.metrics.availability_per_mille));
        sum_bandwidth = sum_bandwidth.saturating_add(u64::from(payout.metrics.bandwidth_per_mille));
        total_epochs = total_epochs.saturating_add(1);
        payout_totals.push(payout_nanos);
    }
    let total_payout_nanos: u128 = accumulators.values().map(|acc| acc.payout_nanos).sum();
    for acc in accumulators.values() {
        if acc.payout_nanos > max_relay_payout {
            max_relay_payout = acc.payout_nanos;
        }
    }
    let mut relay_summaries: Vec<ShadowRunRelaySummary> = accumulators
        .into_iter()
        .map(|(relay_id_hex, acc)| {
            let epochs = acc.epochs.max(1); // avoid division by zero
            let epochs_f64 = usize_to_f64(epochs);
            ShadowRunRelaySummary {
                relay_id_hex: relay_id_hex.to_string(),
                epochs,
                payout_nanos: acc.payout_nanos,
                amount_conversion_errors: acc.amount_conversion_errors,
                average_payout_nanos: u128_to_f64(acc.payout_nanos) / epochs_f64,
                average_score_per_mille: u64_to_f64(acc.total_score) / epochs_f64,
                average_availability_per_mille: u64_to_f64(acc.total_availability) / epochs_f64,
                average_bandwidth_per_mille: u64_to_f64(acc.total_bandwidth) / epochs_f64,
                warning_epochs: acc.warning_epochs,
                suspended_epochs: acc.suspended_epochs,
                zero_score_epochs: acc.zero_score_epochs,
            }
        })
        .collect();
    relay_summaries.sort_by(|left, right| {
        right
            .payout_nanos
            .cmp(&left.payout_nanos)
            .then_with(|| left.relay_id_hex.cmp(&right.relay_id_hex))
    });
    let gini_coefficient = compute_gini(&payout_totals);
    let top_share = if total_payout_nanos == 0 {
        0.0
    } else {
        u128_to_f64(max_relay_payout) / u128_to_f64(total_payout_nanos)
    };
    let average_availability = if total_epochs == 0 {
        0.0
    } else {
        u64_to_f64(sum_availability) / usize_to_f64(total_epochs)
    };
    let average_bandwidth = if total_epochs == 0 {
        0.0
    } else {
        u64_to_f64(sum_bandwidth) / usize_to_f64(total_epochs)
    };
    ShadowRunSummary {
        processed_payouts: total_epochs,
        total_relays: relay_summaries.len(),
        total_payout_nanos,
        payout_amount_conversion_errors,
        gini_coefficient,
        top_relay_share: top_share,
        zero_score_epochs: zero_score_epochs_total,
        warning_epochs: warning_epochs_total,
        suspended_epochs: suspended_epochs_total,
        average_availability_per_mille: average_availability,
        average_bandwidth_per_mille: average_bandwidth,
        skipped_missing_config: summary.skipped_missing_config,
        skipped_missing_bond: summary.skipped_missing_bond,
        skipped_duplicate: summary.skipped_duplicate,
        missing_budget_approval: summary.missing_budget_approval,
        mismatched_budget_approval: summary.mismatched_budget_approval,
        expected_budget_approval: summary.expected_budget_approval.clone(),
        errors: summary.errors.clone(),
        relays: relay_summaries,
    }
}
fn compute_gini(values: &[u128]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f64> = values.iter().map(|value| u128_to_f64(*value)).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let sum: f64 = sorted.iter().sum();
    if sum == 0.0 {
        return 0.0;
    }
    let n = usize_to_f64(sorted.len());
    let mut cumulative = 0.0;
    for (index, value) in sorted.iter().enumerate() {
        cumulative += (usize_to_f64(index) + 1.0) * value;
    }
    (2.0 * cumulative / (n * sum)) - (n + 1.0) / n
}
impl ServiceDashboardSummary {
    fn new(dashboard: &EarningsDashboard) -> Self {
        let rows = dashboard
            .rows
            .iter()
            .map(ServiceDashboardRow::from_row)
            .collect();
        Self {
            total_relays: dashboard.total_relays,
            total_open_disputes: dashboard.total_open_disputes,
            rows,
        }
    }
}
#[derive(Debug, norito::json::JsonSerialize)]
struct ServiceDashboardRow {
    relay_id_hex: String,
    total_paid: Quantity,
    total_rebated: Quantity,
    total_withheld: Quantity,
    net_paid: Numeric,
    epochs_recorded: usize,
    last_epoch: Option<u32>,
    last_reward_score: Option<u64>,
    open_disputes: usize,
}
impl ServiceDashboardRow {
    fn from_row(row: &EarningsRow) -> Self {
        Self {
            relay_id_hex: relay_id_to_hex(row.relay_id),
            total_paid: row.total_paid.clone(),
            total_rebated: row.total_rebated.clone(),
            total_withheld: row.total_withheld.clone(),
            net_paid: row.net_paid.clone(),
            epochs_recorded: row.epochs_recorded,
            last_epoch: row.last_epoch,
            last_reward_score: row.last_reward_score,
            open_disputes: row.open_disputes,
        }
    }
}
fn parse_account_id_str<C: RunContext>(context: &C, value: &str, flag: &str) -> Result<AccountId> {
    let trimmed = value.trim();
    crate::resolve_account_id(context, trimmed)
        .wrap_err_with(|| format!("{flag} must be a valid account identifier"))
}
fn moderation_actor_or_default<C: RunContext>(
    context: &C,
    value: Option<&str>,
    flag: &str,
) -> Result<String> {
    match value {
        Some(raw) => required_trimmed_text(raw, flag),
        None => Ok(context.config().account.to_string()),
    }
}
fn required_trimmed_text(value: &str, flag: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(eyre!("{flag} must not be empty"));
    }
    Ok(trimmed.to_owned())
}
fn optional_trimmed_text(value: Option<&str>, flag: &str) -> Result<Option<String>> {
    value
        .map(|text| required_trimmed_text(text, flag))
        .transpose()
}
fn required_path_string(path: &Path, flag: &str) -> Result<String> {
    required_trimmed_text(&path.display().to_string(), flag)
}
fn required_path_strings(paths: &[PathBuf], flag: &str) -> Result<Vec<String>> {
    paths
        .iter()
        .map(|path| required_path_string(path, flag))
        .collect()
}
fn shell_single_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn systemd_quote(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}
fn xml_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}
const MODERATION_NATIVE_ACTION_INPUT_MAX_BYTES_V1: usize = 2 * 1024 * 1024;
const MODERATION_COORDINATION_STATUS_MAX_BYTES_V1: usize = 4 * 1024 * 1024;
fn load_moderation_ballot_commit_payload(
    path: &Path,
    format: &str,
) -> Result<SoraFsModerationBallotCommitV1> {
    let format = normalize_moderation_ballot_payload_format(format)?;
    let bytes = read_moderation_ballot_payload_file(path)?;
    let commit: SoraFsModerationBallotCommitV1 = match format {
        "json" => norito::json::from_slice(&bytes).wrap_err_with(|| {
            format!(
                "failed to parse moderation ballot commit JSON `{}`",
                path.display()
            )
        })?,
        "norito" => decode_from_bytes(&bytes).wrap_err_with(|| {
            format!(
                "failed to decode moderation ballot commit Norito `{}`",
                path.display()
            )
        })?,
        _ => unreachable!("format normalized"),
    };
    commit
        .validate()
        .wrap_err("moderation ballot commit validation failed")?;
    Ok(commit)
}
fn load_moderation_ballot_reveal_payload(
    path: &Path,
    format: &str,
) -> Result<SoraFsModerationBallotRevealV1> {
    let format = normalize_moderation_ballot_payload_format(format)?;
    let bytes = read_moderation_ballot_payload_file(path)?;
    let reveal: SoraFsModerationBallotRevealV1 = match format {
        "json" => norito::json::from_slice(&bytes).wrap_err_with(|| {
            format!(
                "failed to parse moderation ballot reveal JSON `{}`",
                path.display()
            )
        })?,
        "norito" => decode_from_bytes(&bytes).wrap_err_with(|| {
            format!(
                "failed to decode moderation ballot reveal Norito `{}`",
                path.display()
            )
        })?,
        _ => unreachable!("format normalized"),
    };
    reveal
        .validate()
        .wrap_err("moderation ballot reveal validation failed")?;
    Ok(reveal)
}
fn read_moderation_ballot_payload_file(path: &Path) -> Result<Vec<u8>> {
    read_bounded_moderation_file(
        path,
        "moderation ballot payload",
        MODERATION_NATIVE_ACTION_INPUT_MAX_BYTES_V1,
    )
}
fn load_moderation_commit_reveal_status_payload(path: &Path) -> Result<Value> {
    let bytes = read_bounded_moderation_file(
        path,
        "moderation commit/reveal status",
        MODERATION_COORDINATION_STATUS_MAX_BYTES_V1,
    )?;
    let status: Value = norito::json::from_slice(&bytes).wrap_err_with(|| {
        format!(
            "failed to parse moderation commit/reveal status JSON `{}`",
            path.display()
        )
    })?;
    ensure_moderation_bridge_plan_has_no_payload(&status)?;
    Ok(status)
}
fn read_bounded_moderation_file(path: &Path, label: &str, maximum: usize) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path)
        .wrap_err_with(|| format!("failed to inspect {label} `{}`", path.display()))?;
    if metadata.len() == 0 || metadata.len() > maximum as u64 {
        return Err(eyre!(
            "{label} `{}` must contain between 1 and {maximum} bytes",
            path.display(),
        ));
    }
    let bytes =
        fs::read(path).wrap_err_with(|| format!("failed to read {label} `{}`", path.display()))?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(eyre!(
            "{label} `{}` must contain between 1 and {maximum} bytes",
            path.display(),
        ));
    }
    Ok(bytes)
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ModerationBallotExecutionKey {
    case_id: String,
    round_id: String,
    juror_id: String,
}
impl ModerationBallotExecutionKey {
    fn from_commit(commit: &SoraFsModerationBallotCommitV1) -> Self {
        Self {
            case_id: commit.context.case_id.clone(),
            round_id: commit.round_id.clone(),
            juror_id: commit.juror_id.clone(),
        }
    }
    fn from_reveal(reveal: &SoraFsModerationBallotRevealV1) -> Self {
        Self {
            case_id: reveal.context.case_id.clone(),
            round_id: reveal.round_id.clone(),
            juror_id: reveal.juror_id.clone(),
        }
    }
    fn new(case_id: &str, round_id: &str, juror_id: &str) -> Result<Self> {
        Ok(Self {
            case_id: required_trimmed_text(case_id, "case_id")?,
            round_id: required_trimmed_text(round_id, "round_id")?,
            juror_id: required_trimmed_text(juror_id, "juror_id")?,
        })
    }
}
#[derive(Debug, Default)]
struct ModerationCommitRevealCoordination {
    pending_commits: BTreeSet<ModerationBallotExecutionKey>,
    pending_reveals: BTreeSet<ModerationBallotExecutionKey>,
    tally_ready: BTreeSet<(String, String)>,
}
fn moderation_commit_reveal_coordination_from_status(
    status: &Value,
) -> Result<ModerationCommitRevealCoordination> {
    let root = value_object(status, "commit/reveal execution status")?;
    let schema = required_string_field(root, "schema", "commit/reveal execution status")?;
    if schema != "sorafs.moderation.quarantine.commit_reveal_status.v1" {
        return Err(eyre!(
            "commit/reveal execution status schema `{schema}` is not supported"
        ));
    }
    let ballots = root
        .get("ballots")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut coordination = ModerationCommitRevealCoordination::default();
    for ballot in ballots {
        let ballot_obj = value_object(ballot, "commit/reveal execution ballot")?;
        let case_id =
            required_string_field(ballot_obj, "case_id", "commit/reveal execution ballot")?;
        let round_id =
            required_string_field(ballot_obj, "round_id", "commit/reveal execution ballot")?;
        for juror_id in moderation_commit_reveal_juror_list(ballot_obj, "missing_commit_jurors")? {
            coordination
                .pending_commits
                .insert(ModerationBallotExecutionKey::new(
                    case_id, round_id, juror_id,
                )?);
        }
        for juror_id in moderation_commit_reveal_juror_list(ballot_obj, "missing_reveal_jurors")? {
            coordination
                .pending_reveals
                .insert(ModerationBallotExecutionKey::new(
                    case_id, round_id, juror_id,
                )?);
        }
        if ballot_obj
            .get("ready_to_tally")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            coordination.tally_ready.insert((
                required_trimmed_text(case_id, "case_id")?,
                required_trimmed_text(round_id, "round_id")?,
            ));
        }
    }
    Ok(coordination)
}
fn moderation_commit_reveal_juror_list<'a>(
    ballot_obj: &'a Map,
    field: &str,
) -> Result<Vec<&'a str>> {
    let Some(values) = ballot_obj.get(field) else {
        return Ok(Vec::new());
    };
    let values = values
        .as_array()
        .ok_or_else(|| eyre!("commit/reveal execution ballot `{field}` must be an array"))?;
    values
        .iter()
        .map(|value| {
            value.as_str().ok_or_else(|| {
                eyre!("commit/reveal execution ballot `{field}` entries must be strings")
            })
        })
        .collect()
}
fn build_moderation_commit_transaction(
    client: &Client,
    commit: &SoraFsModerationBallotCommitV1,
) -> Result<SignedTransaction> {
    commit
        .validate()
        .wrap_err("moderation ballot commit validation failed")?;
    if commit.committed_at_unix_ms != 0 {
        return Err(eyre!(
            "moderation commit committed_at_unix_ms must be zero; the ledger records the accepted timestamp"
        ));
    }
    if commit.juror_id != client.account.to_string() {
        return Err(eyre!(
            "moderation commit juror_id must equal the configured transaction authority"
        ));
    }
    let payload = norito::to_bytes(commit).wrap_err("encode canonical moderation commit")?;
    client
        .try_build_sorafs_moderation_transaction(SubmitSorafsModerationCommit::new(payload))
        .wrap_err("build caller-signed native moderation commit transaction")
}
fn build_moderation_reveal_transaction(
    client: &Client,
    reveal: &SoraFsModerationBallotRevealV1,
) -> Result<SignedTransaction> {
    reveal
        .validate()
        .wrap_err("moderation ballot reveal validation failed")?;
    if reveal.revealed_at_unix_ms != 0 {
        return Err(eyre!(
            "moderation reveal revealed_at_unix_ms must be zero; the ledger records the accepted timestamp"
        ));
    }
    if reveal.juror_id != client.account.to_string() {
        return Err(eyre!(
            "moderation reveal juror_id must equal the configured transaction authority"
        ));
    }
    let payload = norito::to_bytes(reveal).wrap_err("encode canonical moderation reveal")?;
    client
        .try_build_sorafs_moderation_transaction(SubmitSorafsModerationReveal::new(payload))
        .wrap_err("build caller-signed native moderation reveal transaction")
}
fn build_moderation_finalization_transaction(
    client: &Client,
    case_id: impl Into<String>,
    round_id: impl Into<String>,
) -> Result<SignedTransaction> {
    client
        .try_build_sorafs_moderation_transaction(FinalizeSorafsModerationCase::new(
            case_id.into(),
            round_id.into(),
        ))
        .wrap_err("build governed native moderation finalization transaction")
}
fn render_moderation_transaction_hash<C: RunContext>(
    context: &mut C,
    hash: &HashOf<SignedTransaction>,
) -> Result<()> {
    context.print_data(&norito::json!({
        "transaction_hash_hex": (encode(hash.as_ref()))
    }))
}
fn moderation_ballot_execution_action_json(
    action: &str,
    case_id: &str,
    round_id: &str,
    juror_id: Option<&str>,
    hash: &HashOf<SignedTransaction>,
) -> Result<Value> {
    let mut fields = Map::new();
    fields.insert("action".into(), Value::from(action.to_string()));
    fields.insert("case_id".into(), Value::from(case_id.to_string()));
    fields.insert("round_id".into(), Value::from(round_id.to_string()));
    fields.insert(
        "juror_id".into(),
        juror_id.map_or(Value::Null, |value| Value::from(value.to_string())),
    );
    fields.insert(
        "transaction_hash_hex".into(),
        Value::from(encode(hash.as_ref())),
    );
    fields.insert("payload_bytes_included".into(), Value::Bool(false));
    fields.insert("private_payloads_included".into(), Value::Bool(false));
    Ok(Value::Object(fields))
}
fn write_moderation_ballots_executor_bundle(
    args: &ModerationBallotsExecutorBundleArgs,
) -> Result<Value> {
    if args.commit_payloads.is_empty() && args.reveal_payloads.is_empty() && !args.submit_tally {
        return Err(eyre!(
            "at least one --commit-payload, --reveal-payload, or --submit-tally is required"
        ));
    }
    if args.interval_secs == 0 {
        return Err(eyre!("--interval-secs must be greater than zero"));
    }
    let status_path = required_path_string(&args.status, "--status")?;
    let commit_format = normalize_moderation_ballot_payload_format(&args.commit_format)?;
    let reveal_format = normalize_moderation_ballot_payload_format(&args.reveal_format)?;
    let iroha_bin = required_trimmed_text(&args.iroha_bin, "--iroha-bin")?;
    let service_name = required_trimmed_text(&args.service_name, "--service-name")?;
    if service_name.contains('/') || service_name.contains('\\') {
        return Err(eyre!("--service-name must not contain path separators"));
    }
    let service_user = required_trimmed_text(&args.service_user, "--service-user")?;
    let service_group = required_trimmed_text(&args.service_group, "--service-group")?;
    let commit_payloads = required_path_strings(&args.commit_payloads, "--commit-payload")?;
    let reveal_payloads = required_path_strings(&args.reveal_payloads, "--reveal-payload")?;
    fs::create_dir_all(&args.bundle_out).wrap_err_with(|| {
        format!(
            "failed to create moderation ballots executor bundle directory `{}`",
            args.bundle_out.display()
        )
    })?;
    let bundle_dir = args
        .bundle_out
        .canonicalize()
        .unwrap_or_else(|_| args.bundle_out.clone());
    let env_path = bundle_dir.join("executor.env");
    let run_path = bundle_dir.join("run.sh");
    let systemd_unit_name = format!("{service_name}.service");
    let systemd_timer_name = format!("{service_name}.timer");
    let launchd_plist_name = format!("{service_name}.plist");
    let metadata_path = bundle_dir.join("bundle.json");
    let readme_path = bundle_dir.join("README.md");
    let env = moderation_ballots_executor_bundle_env(
        &iroha_bin,
        &status_path,
        commit_format,
        reveal_format,
    );
    write_text_artifact(&env_path, &env, "moderation ballots executor environment")?;
    let run_script = moderation_ballots_executor_bundle_run_script(
        &commit_payloads,
        &reveal_payloads,
        args.submit_tally,
    );
    write_text_artifact(
        &run_path,
        &run_script,
        "moderation ballots executor run script",
    )?;
    set_executable_if_supported(&run_path)?;
    let systemd_unit = moderation_ballots_executor_bundle_systemd_unit(
        &service_name,
        &service_user,
        &service_group,
        &bundle_dir,
        &run_path,
        &env_path,
    );
    write_text_artifact(
        &bundle_dir.join(&systemd_unit_name),
        &systemd_unit,
        "moderation ballots executor systemd unit",
    )?;
    let systemd_timer =
        moderation_ballots_executor_bundle_systemd_timer(&service_name, args.interval_secs);
    write_text_artifact(
        &bundle_dir.join(&systemd_timer_name),
        &systemd_timer,
        "moderation ballots executor systemd timer",
    )?;
    let launchd = moderation_ballots_executor_bundle_launchd_plist(
        &service_name,
        &bundle_dir,
        &run_path,
        args.interval_secs,
    );
    write_text_artifact(
        &bundle_dir.join(&launchd_plist_name),
        &launchd,
        "moderation ballots executor launchd plist",
    )?;
    let readme = moderation_ballots_executor_bundle_readme(
        &status_path,
        commit_payloads.len(),
        reveal_payloads.len(),
        args.submit_tally,
        args.interval_secs,
        &systemd_unit_name,
        &systemd_timer_name,
        &launchd_plist_name,
    );
    write_text_artifact(
        &readme_path,
        &readme,
        "moderation ballots executor bundle README",
    )?;
    let files = vec![
        "executor.env",
        "run.sh",
        systemd_unit_name.as_str(),
        systemd_timer_name.as_str(),
        launchd_plist_name.as_str(),
        "bundle.json",
        "README.md",
    ];
    let summary = moderation_ballots_executor_bundle_summary_json(
        &bundle_dir,
        &status_path,
        commit_format,
        reveal_format,
        commit_payloads.len(),
        reveal_payloads.len(),
        args.submit_tally,
        args.interval_secs,
        &iroha_bin,
        &service_name,
        &service_user,
        &service_group,
        &systemd_unit_name,
        &systemd_timer_name,
        &launchd_plist_name,
        &files,
    );
    write_json_artifact(
        &metadata_path,
        &summary,
        "moderation ballots executor bundle metadata",
    )?;
    Ok(summary)
}
fn moderation_ballots_executor_bundle_env(
    iroha_bin: &str,
    status_path: &str,
    commit_format: &str,
    reveal_format: &str,
) -> String {
    format!(
        "IROHA_BIN={}\nSORAFS_BALLOTS_EXECUTOR_STATUS_PATH={}\nSORAFS_BALLOTS_EXECUTOR_COMMIT_FORMAT={}\nSORAFS_BALLOTS_EXECUTOR_REVEAL_FORMAT={}\n",
        shell_single_quote(iroha_bin),
        shell_single_quote(status_path),
        shell_single_quote(commit_format),
        shell_single_quote(reveal_format)
    )
}
fn moderation_ballots_executor_bundle_run_script(
    commit_payloads: &[String],
    reveal_payloads: &[String],
    submit_tally: bool,
) -> String {
    let mut command_args = vec![
        "  --status=\"$SORAFS_BALLOTS_EXECUTOR_STATUS_PATH\"".to_string(),
        "  --commit-format=\"$SORAFS_BALLOTS_EXECUTOR_COMMIT_FORMAT\"".to_string(),
        "  --reveal-format=\"$SORAFS_BALLOTS_EXECUTOR_REVEAL_FORMAT\"".to_string(),
    ];
    command_args.extend(
        commit_payloads
            .iter()
            .map(|path| format!("  --commit-payload={}", shell_single_quote(path))),
    );
    command_args.extend(
        reveal_payloads
            .iter()
            .map(|path| format!("  --reveal-payload={}", shell_single_quote(path))),
    );
    if submit_tally {
        command_args.push("  --submit-tally".to_string());
    }
    format!(
        "#!/usr/bin/env sh\nset -eu\nSCRIPT_DIR=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\nif [ -f \"$SCRIPT_DIR/executor.env\" ]; then\n  . \"$SCRIPT_DIR/executor.env\"\nfi\n: \"${{IROHA_BIN:=iroha}}\"\n: \"${{SORAFS_BALLOTS_EXECUTOR_STATUS_PATH:?set SORAFS_BALLOTS_EXECUTOR_STATUS_PATH in executor.env}}\"\n: \"${{SORAFS_BALLOTS_EXECUTOR_COMMIT_FORMAT:=json}}\"\n: \"${{SORAFS_BALLOTS_EXECUTOR_REVEAL_FORMAT:=json}}\"\nexec \"$IROHA_BIN\" sorafs moderation ballots execute \\\n{}\n",
        command_args.join(" \\\n")
    )
}
fn moderation_ballots_executor_bundle_systemd_unit(
    service_name: &str,
    service_user: &str,
    service_group: &str,
    bundle_dir: &Path,
    run_path: &Path,
    env_path: &Path,
) -> String {
    format!(
        "[Unit]\nDescription=SoraFS moderation ballot executor ({})\nWants=network-online.target\nAfter=network-online.target\n\n[Service]\nType=oneshot\nUser={}\nGroup={}\nWorkingDirectory={}\nEnvironmentFile={}\nExecStart={}\nNoNewPrivileges=true\nPrivateTmp=true\nProtectSystem=full\n\n[Install]\nWantedBy=multi-user.target\n",
        service_name,
        service_user,
        service_group,
        systemd_quote(&bundle_dir.display().to_string()),
        systemd_quote(&env_path.display().to_string()),
        systemd_quote(&run_path.display().to_string())
    )
}
fn moderation_ballots_executor_bundle_systemd_timer(
    service_name: &str,
    interval_secs: u64,
) -> String {
    format!(
        "[Unit]\nDescription=Schedule SoraFS moderation ballot executor ({})\n\n[Timer]\nOnBootSec=30s\nOnUnitActiveSec={}s\nAccuracySec=5s\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n",
        service_name, interval_secs
    )
}
fn moderation_ballots_executor_bundle_launchd_plist(
    service_name: &str,
    bundle_dir: &Path,
    run_path: &Path,
    interval_secs: u64,
) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>{}</string>\n  <key>ProgramArguments</key>\n  <array>\n    <string>{}</string>\n  </array>\n  <key>WorkingDirectory</key>\n  <string>{}</string>\n  <key>RunAtLoad</key>\n  <true/>\n  <key>StartInterval</key>\n  <integer>{}</integer>\n  <key>StandardOutPath</key>\n  <string>{}</string>\n  <key>StandardErrorPath</key>\n  <string>{}</string>\n</dict>\n</plist>\n",
        xml_escape(service_name),
        xml_escape(&run_path.display().to_string()),
        xml_escape(&bundle_dir.display().to_string()),
        interval_secs,
        xml_escape(&bundle_dir.join("executor.out.log").display().to_string()),
        xml_escape(&bundle_dir.join("executor.err.log").display().to_string())
    )
}
fn moderation_ballots_executor_bundle_readme(
    status_path: &str,
    commit_payload_count: usize,
    reveal_payload_count: usize,
    submit_tally: bool,
    interval_secs: u64,
    systemd_unit_name: &str,
    systemd_timer_name: &str,
    launchd_plist_name: &str,
) -> String {
    format!(
        "# SoraFS Moderation Ballot Executor Bundle\n\nThis bundle runs `iroha sorafs moderation ballots execute` as a scheduled local job. It does not copy private commit or reveal payload files; keep those files in an operator-controlled location and update `run.sh` only if their runtime paths change.\n\n- Status path: `{}`\n- Commit payload paths referenced: `{}`\n- Reveal payload paths referenced: `{}`\n- Submit tally requests: `{}`\n- Interval seconds: `{}`\n\nRun directly:\n\n```sh\n./run.sh\n```\n\nInstall with systemd:\n\n```sh\nsudo cp {} {} /etc/systemd/system/\nsudo systemctl daemon-reload\nsudo systemctl enable --now {}\n```\n\nInstall with launchd:\n\n```sh\ncp {} ~/Library/LaunchAgents/\nlaunchctl load ~/Library/LaunchAgents/{}\n```\n\nReplace `IROHA_BIN` in `executor.env` with the absolute path to the audited `iroha` binary on the target host before installing.\n",
        status_path,
        commit_payload_count,
        reveal_payload_count,
        submit_tally,
        interval_secs,
        systemd_unit_name,
        systemd_timer_name,
        systemd_timer_name,
        launchd_plist_name,
        launchd_plist_name
    )
}
#[allow(clippy::too_many_arguments)]
fn moderation_ballots_executor_bundle_summary_json(
    bundle_dir: &Path,
    status_path: &str,
    commit_format: &str,
    reveal_format: &str,
    commit_payload_count: usize,
    reveal_payload_count: usize,
    submit_tally: bool,
    interval_secs: u64,
    iroha_bin: &str,
    service_name: &str,
    service_user: &str,
    service_group: &str,
    systemd_unit_name: &str,
    systemd_timer_name: &str,
    launchd_plist_name: &str,
    files: &[&str],
) -> Value {
    let mut summary = Map::new();
    summary.insert(
        "schema".into(),
        Value::from("sorafs.moderation.ballots.executor_bundle.v1"),
    );
    summary.insert("source".into(), Value::from("iroha_cli"));
    summary.insert(
        "bundle_dir".into(),
        Value::from(bundle_dir.display().to_string()),
    );
    summary.insert("status_path".into(), Value::from(status_path.to_string()));
    summary.insert(
        "commit_format".into(),
        Value::from(commit_format.to_string()),
    );
    summary.insert(
        "reveal_format".into(),
        Value::from(reveal_format.to_string()),
    );
    summary.insert(
        "commit_payload_count".into(),
        Value::from(u64::try_from(commit_payload_count).unwrap_or(u64::MAX)),
    );
    summary.insert(
        "reveal_payload_count".into(),
        Value::from(u64::try_from(reveal_payload_count).unwrap_or(u64::MAX)),
    );
    summary.insert("submit_tally".into(), Value::Bool(submit_tally));
    summary.insert("interval_secs".into(), Value::from(interval_secs));
    summary.insert("iroha_bin".into(), Value::from(iroha_bin.to_string()));
    summary.insert("service_name".into(), Value::from(service_name.to_string()));
    summary.insert("service_user".into(), Value::from(service_user.to_string()));
    summary.insert(
        "service_group".into(),
        Value::from(service_group.to_string()),
    );
    summary.insert(
        "systemd_unit".into(),
        Value::from(systemd_unit_name.to_string()),
    );
    summary.insert(
        "systemd_timer".into(),
        Value::from(systemd_timer_name.to_string()),
    );
    summary.insert(
        "launchd_plist".into(),
        Value::from(launchd_plist_name.to_string()),
    );
    summary.insert(
        "files".into(),
        Value::Array(
            files
                .iter()
                .map(|file| Value::from((*file).to_string()))
                .collect(),
        ),
    );
    summary.insert("payload_bytes_included".into(), Value::Bool(false));
    summary.insert("private_payloads_included".into(), Value::Bool(false));
    summary.insert("private_payload_files_copied".into(), Value::Bool(false));
    Value::Object(summary)
}
fn moderation_ballots_executor_canary_evidence(
    args: &ModerationBallotsExecutorCanaryArgs,
) -> Result<Value> {
    let bundle_dir = args
        .bundle
        .canonicalize()
        .unwrap_or_else(|_| args.bundle.clone());
    let metadata_path = bundle_dir.join("bundle.json");
    let (metadata, metadata_bytes) = read_json_artifact(
        &metadata_path,
        "moderation ballots executor bundle metadata",
    )?;
    ensure_moderation_bridge_plan_has_no_payload(&metadata)?;
    let metadata_fields = value_object(&metadata, "moderation ballots executor bundle metadata")?;
    let schema = required_string_field(
        metadata_fields,
        "schema",
        "moderation ballots executor bundle metadata",
    )?;
    if schema != "sorafs.moderation.ballots.executor_bundle.v1" {
        return Err(eyre!(
            "moderation ballots executor bundle metadata schema `{schema}` is not supported"
        ));
    }
    require_json_bool_false(
        metadata_fields,
        "payload_bytes_included",
        "moderation ballots executor bundle metadata",
    )?;
    require_json_bool_false(
        metadata_fields,
        "private_payloads_included",
        "moderation ballots executor bundle metadata",
    )?;
    require_json_bool_false(
        metadata_fields,
        "private_payload_files_copied",
        "moderation ballots executor bundle metadata",
    )?;
    let service_name = required_nonblank_string_field(
        metadata_fields,
        "service_name",
        "moderation ballots executor bundle metadata",
    )?;
    let interval_secs = metadata_fields
        .get("interval_secs")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let systemd_unit = required_nonblank_string_field(
        metadata_fields,
        "systemd_unit",
        "moderation ballots executor bundle metadata",
    )?;
    let systemd_timer = required_nonblank_string_field(
        metadata_fields,
        "systemd_timer",
        "moderation ballots executor bundle metadata",
    )?;
    let launchd_plist = required_nonblank_string_field(
        metadata_fields,
        "launchd_plist",
        "moderation ballots executor bundle metadata",
    )?;
    let artifact_specs = [
        ("executor.env", "env"),
        ("run.sh", "run_script"),
        (systemd_unit, "systemd_unit"),
        (systemd_timer, "systemd_timer"),
        (launchd_plist, "launchd_plist"),
        ("README.md", "readme"),
        ("bundle.json", "metadata"),
    ];
    let mut artifacts = Vec::new();
    let mut passed_artifact_count = 0_u64;
    for (name, kind) in artifact_specs {
        let probe = moderation_ballots_executor_canary_artifact(&bundle_dir, name, kind)?;
        if probe
            .get("passed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            passed_artifact_count = passed_artifact_count.saturating_add(1);
        }
        artifacts.push(probe);
    }
    let execution_summary = args
        .execution_summary
        .as_deref()
        .map(moderation_ballots_executor_canary_execution_summary)
        .transpose()?;
    let execution_summary_passed = execution_summary
        .as_ref()
        .and_then(|summary| summary.get("passed"))
        .and_then(Value::as_bool)
        .unwrap_or(args.execution_summary.is_none());
    let artifact_count = u64::try_from(artifacts.len()).unwrap_or(u64::MAX);
    let artifacts_passed = passed_artifact_count == artifact_count;
    let status = if artifacts_passed && execution_summary_passed {
        "passed"
    } else {
        "failed"
    };
    let mut evidence = Map::new();
    evidence.insert(
        "schema".into(),
        Value::from("sorafs.moderation.ballots.executor_canary.v1"),
    );
    evidence.insert("source".into(), Value::from("executor-bundle"));
    evidence.insert("status".into(), Value::from(status));
    evidence.insert(
        "bundle_dir".into(),
        Value::from(bundle_dir.display().to_string()),
    );
    evidence.insert(
        "bundle_metadata_bytes".into(),
        Value::from(u64::try_from(metadata_bytes.len()).unwrap_or(u64::MAX)),
    );
    evidence.insert(
        "bundle_metadata_blake3".into(),
        Value::from(encode(blake3::hash(&metadata_bytes).as_bytes())),
    );
    evidence.insert("service_name".into(), Value::from(service_name.to_string()));
    evidence.insert("interval_secs".into(), Value::from(interval_secs));
    evidence.insert("artifact_count".into(), Value::from(artifact_count));
    evidence.insert(
        "passed_artifact_count".into(),
        Value::from(passed_artifact_count),
    );
    evidence.insert(
        "execution_summary_present".into(),
        Value::Bool(args.execution_summary.is_some()),
    );
    evidence.insert(
        "execution_summary".into(),
        execution_summary.unwrap_or(Value::Null),
    );
    evidence.insert("payload_bytes_included".into(), Value::Bool(false));
    evidence.insert("private_payloads_included".into(), Value::Bool(false));
    evidence.insert("private_payload_files_copied".into(), Value::Bool(false));
    evidence.insert("artifacts".into(), Value::Array(artifacts));
    Ok(Value::Object(evidence))
}
fn moderation_ballots_executor_canary_artifact(
    bundle_dir: &Path,
    name: &str,
    kind: &str,
) -> Result<Value> {
    let path = bundle_dir.join(name);
    let mut fields = Map::new();
    fields.insert("name".into(), Value::from(name.to_string()));
    fields.insert("kind".into(), Value::from(kind.to_string()));
    fields.insert("path".into(), Value::from(path.display().to_string()));
    fields.insert("payload_bytes_included".into(), Value::Bool(false));
    fields.insert("private_payloads_included".into(), Value::Bool(false));
    if !path.exists() {
        fields.insert("exists".into(), Value::Bool(false));
        fields.insert("passed".into(), Value::Bool(false));
        fields.insert("checks".into(), Value::Array(Vec::new()));
        return Ok(Value::Object(fields));
    }
    let bytes = fs::read(&path).wrap_err_with(|| {
        format!(
            "failed to read executor canary artifact `{}`",
            path.display()
        )
    })?;
    let body = String::from_utf8_lossy(&bytes);
    if body.contains("payload_b64") {
        return Err(eyre!(
            "executor canary artifact `{}` unexpectedly contains `payload_b64`",
            path.display()
        ));
    }
    let checks = moderation_ballots_executor_artifact_checks(kind, &body, &path)?;
    let passed = checks.iter().all(|check| {
        check
            .get("passed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    });
    fields.insert("exists".into(), Value::Bool(true));
    fields.insert(
        "bytes".into(),
        Value::from(u64::try_from(bytes.len()).unwrap_or(u64::MAX)),
    );
    fields.insert(
        "body_blake3".into(),
        Value::from(encode(blake3::hash(&bytes).as_bytes())),
    );
    fields.insert("passed".into(), Value::Bool(passed));
    fields.insert("checks".into(), Value::Array(checks));
    Ok(Value::Object(fields))
}
fn moderation_ballots_executor_artifact_checks(
    kind: &str,
    body: &str,
    path: &Path,
) -> Result<Vec<Value>> {
    let mut checks = Vec::new();
    match kind {
        "env" => {
            checks.push(check_json(
                "status_path_env",
                body.contains("SORAFS_BALLOTS_EXECUTOR_STATUS_PATH="),
            ));
            checks.push(check_json(
                "commit_format_env",
                body.contains("SORAFS_BALLOTS_EXECUTOR_COMMIT_FORMAT="),
            ));
            checks.push(check_json(
                "reveal_format_env",
                body.contains("SORAFS_BALLOTS_EXECUTOR_REVEAL_FORMAT="),
            ));
        }
        "run_script" => {
            checks.push(check_json(
                "executes_ballots_execute",
                body.contains("sorafs moderation ballots execute"),
            ));
            checks.push(check_json(
                "uses_status_env",
                body.contains("--status=\"$SORAFS_BALLOTS_EXECUTOR_STATUS_PATH\""),
            ));
            checks.push(check_json("executable", file_is_executable(path)));
        }
        "systemd_unit" => {
            checks.push(check_json("oneshot", body.contains("Type=oneshot")));
            checks.push(check_json("exec_start", body.contains("ExecStart=")));
            checks.push(check_json(
                "no_new_privileges",
                body.contains("NoNewPrivileges=true"),
            ));
        }
        "systemd_timer" => {
            checks.push(check_json(
                "active_interval",
                body.contains("OnUnitActiveSec="),
            ));
            checks.push(check_json("persistent", body.contains("Persistent=true")));
        }
        "launchd_plist" => {
            checks.push(check_json("start_interval", body.contains("StartInterval")));
            checks.push(check_json("run_at_load", body.contains("RunAtLoad")));
        }
        "readme" => {
            checks.push(check_json(
                "documents_private_payload_posture",
                body.contains("does not copy private commit or reveal payload files"),
            ));
        }
        "metadata" => {
            checks.push(check_json(
                "metadata_schema",
                body.contains("sorafs.moderation.ballots.executor_bundle.v1"),
            ));
            checks.push(check_json(
                "metadata_payload_free",
                body.contains("\"payload_bytes_included\": false"),
            ));
        }
        _ => checks.push(check_json("known_artifact_kind", false)),
    }
    Ok(checks)
}
fn moderation_ballots_executor_canary_execution_summary(path: &Path) -> Result<Value> {
    let (summary, bytes) =
        read_json_artifact(path, "moderation ballots executor execution summary")?;
    ensure_moderation_bridge_plan_has_no_payload(&summary)?;
    let fields = value_object(&summary, "moderation ballots executor execution summary")?;
    let schema = required_string_field(
        fields,
        "schema",
        "moderation ballots executor execution summary",
    )?;
    if schema != "sorafs.moderation.ballots.execution.v1" {
        return Err(eyre!(
            "moderation ballots executor execution summary schema `{schema}` is not supported"
        ));
    }
    require_json_bool_false(
        fields,
        "payload_bytes_included",
        "moderation ballots executor execution summary",
    )?;
    require_json_bool_false(
        fields,
        "private_payloads_included",
        "moderation ballots executor execution summary",
    )?;
    let actions = fields
        .get("actions")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for action in actions {
        let action_fields = value_object(action, "moderation ballots executor action summary")?;
        let transaction_hash_hex = required_string_field(
            action_fields,
            "transaction_hash_hex",
            "moderation ballots executor action summary",
        )?;
        let canonical_transaction_hash = normalize_hex_digest::<32>(
            transaction_hash_hex,
            "moderation ballots executor action transaction_hash_hex",
        )?;
        if transaction_hash_hex != canonical_transaction_hash {
            return Err(eyre!(
                "moderation ballots executor action transaction_hash_hex must be canonical lowercase hex"
            ));
        }
        for stale_field in ["response_status", "response_bytes", "response_body_blake3"] {
            if action_fields.contains_key(stale_field) {
                return Err(eyre!(
                    "moderation ballots executor action must not contain legacy `{stale_field}`"
                ));
            }
        }
        require_json_bool_false(
            action_fields,
            "payload_bytes_included",
            "moderation ballots executor action summary",
        )?;
        require_json_bool_false(
            action_fields,
            "private_payloads_included",
            "moderation ballots executor action summary",
        )?;
    }
    let mut evidence = Map::new();
    evidence.insert("passed".into(), Value::Bool(true));
    evidence.insert("path".into(), Value::from(path.display().to_string()));
    evidence.insert(
        "bytes".into(),
        Value::from(u64::try_from(bytes.len()).unwrap_or(u64::MAX)),
    );
    evidence.insert(
        "body_blake3".into(),
        Value::from(encode(blake3::hash(&bytes).as_bytes())),
    );
    evidence.insert(
        "action_count".into(),
        fields.get("action_count").cloned().unwrap_or(Value::Null),
    );
    evidence.insert(
        "commit_action_count".into(),
        fields
            .get("commit_action_count")
            .cloned()
            .unwrap_or(Value::Null),
    );
    evidence.insert(
        "reveal_action_count".into(),
        fields
            .get("reveal_action_count")
            .cloned()
            .unwrap_or(Value::Null),
    );
    evidence.insert(
        "tally_action_count".into(),
        fields
            .get("tally_action_count")
            .cloned()
            .unwrap_or(Value::Null),
    );
    evidence.insert("payload_bytes_included".into(), Value::Bool(false));
    evidence.insert("private_payloads_included".into(), Value::Bool(false));
    Ok(Value::Object(evidence))
}
fn check_json(name: &str, passed: bool) -> Value {
    let mut fields = Map::new();
    fields.insert("name".into(), Value::from(name.to_string()));
    fields.insert("passed".into(), Value::Bool(passed));
    Value::Object(fields)
}
fn file_is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}
fn post_moderation_juror_notification_webhook(
    client: &BlockingHttpClient,
    url: &str,
    body: &[u8],
) -> Result<Response<Vec<u8>>> {
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(body.to_vec())
        .send()
        .wrap_err_with(|| format!("failed to deliver juror notification webhook `{url}`"))?;
    let status = StatusCode::from_u16(response.status().as_u16())
        .wrap_err("failed to convert juror notification webhook status")?;
    let body = response
        .bytes()
        .wrap_err("failed to read juror notification webhook response body")?
        .to_vec();
    Ok(Response::builder().status(status).body(body).unwrap())
}
fn load_moderation_juror_notifications_manifest(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).wrap_err_with(|| {
        format!(
            "failed to read juror notification manifest `{}`",
            path.display()
        )
    })?;
    if bytes.is_empty() {
        return Err(eyre!(
            "juror notification manifest `{}` must not be empty",
            path.display()
        ));
    }
    let manifest: Value = norito::json::from_slice(&bytes).wrap_err_with(|| {
        format!(
            "failed to parse juror notification manifest JSON `{}`",
            path.display()
        )
    })?;
    ensure_moderation_bridge_plan_has_no_payload(&manifest)?;
    Ok(manifest)
}
#[derive(Clone, Copy)]
struct ModerationJurorNotificationEntry<'a> {
    value: &'a Value,
    delivery_id: &'a str,
    dedup_key: &'a str,
    action: &'a str,
    case_id: &'a str,
    round_id: &'a str,
    juror_id: &'a str,
}
fn moderation_juror_notification_entries(
    manifest: &Value,
) -> Result<Vec<ModerationJurorNotificationEntry<'_>>> {
    let root = value_object(manifest, "juror notification manifest")?;
    let schema = required_string_field(root, "schema", "juror notification manifest")?;
    if schema != "sorafs.moderation.quarantine.juror_notifications.v1" {
        return Err(eyre!(
            "juror notification manifest schema `{schema}` is not supported"
        ));
    }
    require_json_bool_false(
        root,
        "payload_bytes_included",
        "juror notification manifest",
    )?;
    require_json_bool_false(
        root,
        "private_payloads_included",
        "juror notification manifest",
    )?;
    let notifications = root
        .get("notifications")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("juror notification manifest is missing `notifications` array"))?;
    notifications
        .iter()
        .map(moderation_juror_notification_entry)
        .collect()
}
fn moderation_juror_notification_entry(
    value: &Value,
) -> Result<ModerationJurorNotificationEntry<'_>> {
    let fields = value_object(value, "juror notification entry")?;
    let schema = required_string_field(fields, "schema", "juror notification entry")?;
    if schema != "sorafs.moderation.juror_notification.v1" {
        return Err(eyre!(
            "juror notification entry schema `{schema}` is not supported"
        ));
    }
    require_json_bool_false(fields, "payload_bytes_included", "juror notification entry")?;
    require_json_bool_false(
        fields,
        "private_payload_included",
        "juror notification entry",
    )?;
    Ok(ModerationJurorNotificationEntry {
        value,
        delivery_id: required_nonblank_string_field(
            fields,
            "delivery_id",
            "juror notification entry",
        )?,
        dedup_key: required_nonblank_string_field(fields, "dedup_key", "juror notification entry")?,
        action: required_nonblank_string_field(fields, "action", "juror notification entry")?,
        case_id: required_nonblank_string_field(fields, "case_id", "juror notification entry")?,
        round_id: required_nonblank_string_field(fields, "round_id", "juror notification entry")?,
        juror_id: required_nonblank_string_field(fields, "juror_id", "juror notification entry")?,
    })
}
fn require_json_bool_false(fields: &Map, field: &str, context: &str) -> Result<()> {
    match fields.get(field).and_then(Value::as_bool) {
        Some(false) => Ok(()),
        Some(true) => Err(eyre!("{context} must set `{field}` to false")),
        None => Err(eyre!("{context} is missing boolean `{field}`")),
    }
}
fn required_nonblank_string_field<'a>(
    fields: &'a Map,
    field: &str,
    context: &str,
) -> Result<&'a str> {
    let value = required_string_field(fields, field, context)?;
    if value.trim().is_empty() {
        return Err(eyre!("{context} string `{field}` must not be empty"));
    }
    Ok(value)
}
fn safe_moderation_notification_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
fn moderation_juror_notification_delivery_result_json(
    notification: ModerationJurorNotificationEntry<'_>,
    notification_bytes: usize,
    canonical: &[u8],
    outbox_path: Value,
    webhook_status: Value,
    webhook_response_bytes: Value,
    webhook_response_body_blake3: Value,
) -> Value {
    let mut fields = Map::new();
    fields.insert(
        "delivery_id".into(),
        Value::from(notification.delivery_id.to_string()),
    );
    fields.insert(
        "dedup_key".into(),
        Value::from(notification.dedup_key.to_string()),
    );
    fields.insert(
        "action".into(),
        Value::from(notification.action.to_string()),
    );
    fields.insert(
        "case_id".into(),
        Value::from(notification.case_id.to_string()),
    );
    fields.insert(
        "round_id".into(),
        Value::from(notification.round_id.to_string()),
    );
    fields.insert(
        "juror_id".into(),
        Value::from(notification.juror_id.to_string()),
    );
    fields.insert(
        "notification_bytes".into(),
        Value::from(u64::try_from(notification_bytes).unwrap_or(u64::MAX)),
    );
    fields.insert(
        "notification_body_blake3".into(),
        Value::from(encode(blake3::hash(canonical).as_bytes())),
    );
    fields.insert("outbox_path".into(), outbox_path);
    fields.insert("webhook_status".into(), webhook_status);
    fields.insert("webhook_response_bytes".into(), webhook_response_bytes);
    fields.insert(
        "webhook_response_body_blake3".into(),
        webhook_response_body_blake3,
    );
    fields.insert("payload_bytes_included".into(), Value::Bool(false));
    fields.insert("private_payloads_included".into(), Value::Bool(false));
    Value::Object(fields)
}
fn moderation_juror_notification_canary_probe_json(
    notification: ModerationJurorNotificationEntry<'_>,
    canonical: &[u8],
    response: Response<Vec<u8>>,
) -> Result<Value> {
    let status = response.status();
    let body = response.into_body();
    let mut fields = Map::new();
    fields.insert(
        "delivery_id".into(),
        Value::from(notification.delivery_id.to_string()),
    );
    fields.insert(
        "dedup_key".into(),
        Value::from(notification.dedup_key.to_string()),
    );
    fields.insert(
        "action".into(),
        Value::from(notification.action.to_string()),
    );
    fields.insert(
        "case_id".into(),
        Value::from(notification.case_id.to_string()),
    );
    fields.insert(
        "round_id".into(),
        Value::from(notification.round_id.to_string()),
    );
    fields.insert(
        "juror_id".into(),
        Value::from(notification.juror_id.to_string()),
    );
    fields.insert(
        "notification_bytes".into(),
        Value::from(u64::try_from(canonical.len()).unwrap_or(u64::MAX)),
    );
    fields.insert(
        "notification_body_blake3".into(),
        Value::from(encode(blake3::hash(canonical).as_bytes())),
    );
    fields.insert(
        "response_status".into(),
        Value::from(u64::from(status.as_u16())),
    );
    fields.insert("response_success".into(), Value::Bool(status.is_success()));
    fields.insert(
        "response_bytes".into(),
        Value::from(u64::try_from(body.len()).unwrap_or(u64::MAX)),
    );
    fields.insert(
        "response_body_blake3".into(),
        Value::from(encode(blake3::hash(&body).as_bytes())),
    );
    fields.insert("payload_bytes_included".into(), Value::Bool(false));
    fields.insert("private_payloads_included".into(), Value::Bool(false));
    Ok(Value::Object(fields))
}
fn moderation_canary_probe_ok(probe: &Value) -> bool {
    probe
        .get("response_success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
fn write_json_artifact(path: &Path, value: &Value, label: &str) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!("failed to create {label} directory `{}`", parent.display())
        })?;
    }
    fs::write(path, norito::json::to_vec_pretty(value)?)
        .wrap_err_with(|| format!("failed to write {label} `{}`", path.display()))
}
fn read_json_artifact(path: &Path, label: &str) -> Result<(Value, Vec<u8>)> {
    let bytes =
        fs::read(path).wrap_err_with(|| format!("failed to read {label} `{}`", path.display()))?;
    if bytes.is_empty() {
        return Err(eyre!("{label} `{}` must not be empty", path.display()));
    }
    let value = norito::json::from_slice(&bytes)
        .wrap_err_with(|| format!("failed to parse {label} JSON `{}`", path.display()))?;
    Ok((value, bytes))
}
fn write_text_artifact(path: &Path, value: &str, label: &str) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!("failed to create {label} directory `{}`", parent.display())
        })?;
    }
    fs::write(path, value).wrap_err_with(|| format!("failed to write {label} `{}`", path.display()))
}
fn set_executable_if_supported(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut permissions = fs::metadata(path)
            .wrap_err_with(|| format!("failed to stat `{}`", path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .wrap_err_with(|| format!("failed to chmod `{}`", path.display()))?;
    }
    Ok(())
}
fn transparency_token_issuance_canary_probe_json(
    path: &Path,
    payload: &[u8],
    response: Response<Vec<u8>>,
) -> Value {
    let status = response.status();
    let body = response.into_body();
    let mut fields = Map::new();
    fields.insert(
        "payload_path".into(),
        Value::from(path.display().to_string()),
    );
    fields.insert(
        "request_bytes".into(),
        Value::from(u64::try_from(payload.len()).unwrap_or(u64::MAX)),
    );
    fields.insert(
        "request_body_blake3".into(),
        Value::from(encode(blake3::hash(payload).as_bytes())),
    );
    fields.insert(
        "response_status".into(),
        Value::from(u64::from(status.as_u16())),
    );
    fields.insert("response_success".into(), Value::Bool(status.is_success()));
    fields.insert(
        "response_bytes".into(),
        Value::from(u64::try_from(body.len()).unwrap_or(u64::MAX)),
    );
    fields.insert(
        "response_body_blake3".into(),
        Value::from(encode(blake3::hash(&body).as_bytes())),
    );
    fields.insert("payload_bytes_included".into(), Value::Bool(false));
    fields.insert("proof_token_frame_included".into(), Value::Bool(false));
    fields.insert("private_digest_keys_included".into(), Value::Bool(false));
    fields.insert("response_body_included".into(), Value::Bool(false));
    Value::Object(fields)
}
fn transparency_privacy_aggregate_canary_probe_json(
    action: &str,
    path: &Path,
    payload: &[u8],
    response: Response<Vec<u8>>,
) -> Value {
    let status = response.status();
    let body = response.into_body();
    let mut fields = Map::new();
    fields.insert("action".into(), Value::from(action.to_string()));
    fields.insert(
        "payload_path".into(),
        Value::from(path.display().to_string()),
    );
    fields.insert(
        "request_bytes".into(),
        Value::from(u64::try_from(payload.len()).unwrap_or(u64::MAX)),
    );
    fields.insert(
        "request_body_blake3".into(),
        Value::from(encode(blake3::hash(payload).as_bytes())),
    );
    fields.insert(
        "response_status".into(),
        Value::from(u64::from(status.as_u16())),
    );
    fields.insert("response_success".into(), Value::Bool(status.is_success()));
    fields.insert(
        "response_bytes".into(),
        Value::from(u64::try_from(body.len()).unwrap_or(u64::MAX)),
    );
    fields.insert(
        "response_body_blake3".into(),
        Value::from(encode(blake3::hash(&body).as_bytes())),
    );
    fields.insert("payload_bytes_included".into(), Value::Bool(false));
    fields.insert("raw_metric_values_included".into(), Value::Bool(false));
    fields.insert("private_payloads_included".into(), Value::Bool(false));
    Value::Object(fields)
}
fn load_sorafs_json_payload(path: &Path, label: &str) -> Result<Vec<u8>> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read {label} payload `{}`", path.display(),))?;
    if bytes.is_empty() {
        return Err(eyre!(
            "{label} payload `{}` must not be empty",
            path.display()
        ));
    }
    let value: Value = norito::json::from_slice(&bytes)
        .wrap_err_with(|| format!("failed to parse {label} JSON `{}`", path.display()))?;
    norito::json::to_vec(&value).wrap_err_with(|| format!("failed to encode {label} JSON"))
}
fn normalize_moderation_ballot_payload_format(format: &str) -> Result<&'static str> {
    match format.trim().to_ascii_lowercase().as_str() {
        "json" => Ok("json"),
        "norito" => Ok("norito"),
        other => Err(eyre!(
            "--format must be `json` or `norito` for moderation ballot payloads, got `{other}`"
        )),
    }
}
fn load_moderation_registry_repro_manifest_bytes(path: &Path, format: &str) -> Result<Vec<u8>> {
    let format = normalize_moderation_registry_manifest_format(format)?;
    let bytes = read_moderation_registry_manifest_file(path)?;
    let manifest: ModerationReproManifestV1 = match format {
        "json" => norito::json::from_slice(&bytes).wrap_err_with(|| {
            format!(
                "failed to parse reproducibility manifest JSON `{}`",
                path.display()
            )
        })?,
        "norito" => decode_from_bytes(&bytes).wrap_err_with(|| {
            format!(
                "failed to decode reproducibility manifest Norito `{}`",
                path.display()
            )
        })?,
        _ => unreachable!("format normalized"),
    };
    manifest
        .validate()
        .wrap_err("reproducibility manifest validation failed")?;
    norito::to_bytes(&manifest).wrap_err("failed to encode canonical reproducibility manifest")
}
fn load_moderation_registry_corpus_manifest_bytes(path: &Path, format: &str) -> Result<Vec<u8>> {
    let format = normalize_moderation_registry_manifest_format(format)?;
    let bytes = read_moderation_registry_manifest_file(path)?;
    let manifest: AdversarialCorpusManifestV1 = match format {
        "json" => norito::json::from_slice(&bytes).wrap_err_with(|| {
            format!(
                "failed to parse adversarial corpus manifest JSON `{}`",
                path.display()
            )
        })?,
        "norito" => decode_from_bytes(&bytes).wrap_err_with(|| {
            format!(
                "failed to decode adversarial corpus manifest Norito `{}`",
                path.display()
            )
        })?,
        _ => unreachable!("format normalized"),
    };
    manifest
        .validate()
        .wrap_err("adversarial corpus manifest validation failed")?;
    norito::to_bytes(&manifest).wrap_err("failed to encode canonical adversarial corpus manifest")
}
fn read_moderation_registry_manifest_file(path: &Path) -> Result<Vec<u8>> {
    let bytes = fs::read(path).wrap_err_with(|| {
        format!(
            "failed to read moderation registry manifest `{}`",
            path.display()
        )
    })?;
    if bytes.is_empty() {
        return Err(eyre!(
            "moderation registry manifest `{}` must not be empty",
            path.display()
        ));
    }
    Ok(bytes)
}
fn normalize_moderation_registry_manifest_format(format: &str) -> Result<&'static str> {
    match format.trim().to_ascii_lowercase().as_str() {
        "json" => Ok("json"),
        "norito" => Ok("norito"),
        other => Err(eyre!(
            "--format must be `json` or `norito` for moderation registry manifests, got `{other}`"
        )),
    }
}
fn load_moderation_screening_submit_payload(
    path: &Path,
) -> Result<ModerationScreeningSubmitPayload> {
    const MAX_AUTHENTICATED_SCREENING_JSON_BYTES: u64 = 8 * 1024 * 1024;
    let metadata = fs::symlink_metadata(path).wrap_err_with(|| {
        format!(
            "failed to inspect moderation screening authority JSON `{}`",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(eyre!(
            "moderation screening authority JSON `{}` must be a regular non-symlink file",
            path.display()
        ));
    }
    if metadata.len() == 0 || metadata.len() > MAX_AUTHENTICATED_SCREENING_JSON_BYTES {
        return Err(eyre!(
            "moderation screening authority JSON `{}` must contain 1..={} bytes",
            path.display(),
            MAX_AUTHENTICATED_SCREENING_JSON_BYTES
        ));
    }
    let bytes = fs::read(path).wrap_err_with(|| {
        format!(
            "failed to read moderation screening authority JSON `{}`",
            path.display()
        )
    })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != metadata.len() {
        return Err(eyre!(
            "moderation screening authority JSON `{}` changed while it was read",
            path.display()
        ));
    }
    let value: Value = norito::json::from_slice(&bytes).wrap_err_with(|| {
        format!(
            "failed to parse moderation screening authority JSON `{}`",
            path.display()
        )
    })?;
    moderation_screening_submit_payload_from_json(&value)
}
fn moderation_screening_submit_payload_from_json(
    value: &Value,
) -> Result<ModerationScreeningSubmitPayload> {
    let Value::Object(fields) = value else {
        return Err(eyre!("--input must contain a JSON object"));
    };
    let idempotency_key_hex = required_json_hex_digest::<32>(fields, "idempotency_key_hex")?;
    if idempotency_key_hex.bytes().all(|byte| byte == b'0') {
        return Err(eyre!("idempotency_key_hex must not be all zeroes"));
    }
    let evidence_kind = required_json_text(fields, "evidence_kind")?;
    if !matches!(
        evidence_kind.as_str(),
        "signed_result" | "committee_aggregate"
    ) {
        return Err(eyre!(
            "evidence_kind must be `signed_result` or `committee_aggregate`"
        ));
    }
    let authority_b64 = required_json_text(fields, "authority_b64")?;
    let committee_member_results_b64 =
        required_json_string_array(fields, "committee_member_results_b64")?;
    match evidence_kind.as_str() {
        "signed_result" if !committee_member_results_b64.is_empty() => {
            return Err(eyre!(
                "signed_result must not include committee_member_results_b64"
            ));
        }
        "committee_aggregate"
            if committee_member_results_b64.is_empty()
                || committee_member_results_b64.len() > 64 =>
        {
            return Err(eyre!(
                "committee_member_results_b64 must contain 1..=64 signed results"
            ));
        }
        _ => {}
    }
    Ok(ModerationScreeningSubmitPayload {
        idempotency_key_hex,
        evidence_kind,
        authority_b64,
        committee_member_results_b64,
    })
}
fn required_json_string_array(fields: &Map, field: &str) -> Result<Vec<String>> {
    let Some(Value::Array(values)) = fields.get(field) else {
        return if fields.contains_key(field) {
            Err(eyre!("{field} must be a JSON string array"))
        } else {
            Err(eyre!("{field} is required"))
        };
    };
    values
        .iter()
        .enumerate()
        .map(|(index, value)| match value {
            Value::String(value) if !value.is_empty() && value.trim() == value => Ok(value.clone()),
            Value::String(_) => Err(eyre!("{field}[{index}] must be non-empty and unpadded")),
            _ => Err(eyre!("{field}[{index}] must be a JSON string")),
        })
        .collect()
}
fn required_json_text(fields: &Map, field: &str) -> Result<String> {
    match fields.get(field) {
        Some(Value::String(value)) => required_trimmed_text(value, field),
        Some(_) => Err(eyre!("{field} must be a JSON string")),
        None => Err(eyre!("{field} is required")),
    }
}
fn optional_json_text(fields: &Map, field: &str) -> Result<Option<String>> {
    match fields.get(field) {
        Some(Value::String(value)) => optional_trimmed_text(Some(value), field),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(eyre!("{field} must be a JSON string or null")),
    }
}
fn required_json_hex_digest<const N: usize>(fields: &Map, field: &str) -> Result<String> {
    let value = required_json_text(fields, field)?;
    normalize_hex_digest::<N>(&value, field)
}
fn optional_json_u64(fields: &Map, field: &str) -> Result<Option<u64>> {
    match fields.get(field) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| eyre!("{field} must be an unsigned integer or null")),
    }
}
fn parse_repair_ticket_id(value: &str, flag: &str) -> Result<RepairTicketId> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(eyre!("{flag} must not be empty"));
    }
    let ticket_id = RepairTicketId(trimmed.to_string());
    ticket_id
        .validate()
        .map_err(|err| eyre!("{flag} is invalid: {err}"))?;
    Ok(ticket_id)
}
fn validate_repair_revision(value: u64, flag: &str) -> Result<()> {
    if value == 0 {
        return Err(eyre!("{flag} must be non-zero"));
    }
    Ok(())
}
fn build_repair_action_transaction(
    client: &Client,
    ticket_id: &RepairTicketId,
    expected_revision: u64,
    action: SorafsRepairTaskActionV1,
) -> Result<SignedTransaction> {
    let instruction =
        ApplySorafsRepairTaskAction::new(ticket_id.0.clone(), expected_revision, action);
    client
        .try_build_transaction_from_items(
            [instruction],
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        )
        .wrap_err("failed to build caller-signed native SoraFS repair transaction")
}
fn render_repair_transaction_hash<C: RunContext>(
    context: &mut C,
    hash: &HashOf<SignedTransaction>,
) -> Result<()> {
    context.print_data(&norito::json!({
        "transaction_hash_hex": (encode(hash.as_ref()))
    }))
}
fn parse_quantity_str(value: &str, flag: &str) -> Result<Quantity> {
    Quantity::from_str(value)
        .map_err(|err| eyre!("{flag} must be a valid non-negative quantity: {err}"))
}
fn unix_now() -> u64 {
    let seconds = OffsetDateTime::now_utc().unix_timestamp();
    u64::try_from(seconds.max(0)).unwrap_or(0)
}
#[derive(clap::Subcommand, Debug)]
pub enum HandshakeCommand {
    /// Display the current `SoraNet` handshake summary as reported by Torii.
    Show,
    /// Update one or more `SoraNet` handshake parameters via `/v1/config`.
    Update(HandshakeUpdateArgs),
    /// Admission token helpers (issuance, fingerprinting, revocation digests).
    #[command(subcommand)]
    Token(HandshakeTokenCommand),
}
impl Run for HandshakeCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            HandshakeCommand::Show => {
                let client = context.client_from_config();
                let config = client
                    .get_config()
                    .wrap_err("failed to fetch configuration")?;
                render_handshake_summary(context, &config.network.soranet_handshake)?;
                context.println(format_args!(
                    "require_sm_handshake_match: {}",
                    config.network.require_sm_handshake_match
                ))?;
                context.println(format_args!(
                    "require_sm_openssl_preview_match: {}",
                    config.network.require_sm_openssl_preview_match
                ))?;
                Ok(())
            }
            HandshakeCommand::Update(args) => args.run(context),
            HandshakeCommand::Token(cmd) => cmd.run(context),
        }
    }
}
#[derive(clap::Args, Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct HandshakeUpdateArgs {
    /// Override the descriptor commitment advertised during handshake (hex).
    #[arg(long = "descriptor-commit", value_name = "HEX")]
    descriptor_commit: Option<String>,
    /// Override the client capability TLV vector (hex).
    #[arg(long = "client-capabilities", value_name = "HEX")]
    client_capabilities: Option<String>,
    /// Override the relay capability TLV vector (hex).
    #[arg(long = "relay-capabilities", value_name = "HEX")]
    relay_capabilities: Option<String>,
    /// Override the negotiated ML-KEM identifier.
    #[arg(long = "kem-id", value_parser = clap::value_parser!(u8))]
    kem_id: Option<u8>,
    /// Override the negotiated signature suite identifier.
    #[arg(long = "sig-id", value_parser = clap::value_parser!(u8))]
    sig_id: Option<u8>,
    /// Override the resume hash advertised to peers (64 hex chars).
    #[arg(
        long = "resume-hash",
        value_name = "HEX",
        conflicts_with = "clear_resume_hash"
    )]
    resume_hash: Option<String>,
    /// Clear the configured resume hash.
    #[arg(long = "clear-resume-hash", action = clap::ArgAction::SetTrue)]
    clear_resume_hash: bool,
    /// Override the proof-of-work difficulty.
    #[arg(long = "pow-difficulty", value_parser = clap::value_parser!(u8))]
    pow_difficulty: Option<u8>,
    /// Override the maximum clock skew accepted on `PoW` tickets (seconds).
    #[arg(long = "pow-max-future-skew", value_parser = clap::value_parser!(u64))]
    pow_max_future_skew: Option<u64>,
    /// Override the minimum `PoW` ticket TTL (seconds).
    #[arg(long = "pow-min-ttl", value_parser = clap::value_parser!(u64))]
    pow_min_ttl: Option<u64>,
    /// Override the `PoW` ticket TTL (seconds).
    #[arg(long = "pow-ttl", value_parser = clap::value_parser!(u64))]
    pow_ttl: Option<u64>,
    /// Override the puzzle memory cost (KiB).
    #[arg(long = "pow-puzzle-memory", value_parser = clap::value_parser!(u32))]
    pow_puzzle_memory: Option<u32>,
    /// Override the puzzle time cost (iterations).
    #[arg(long = "pow-puzzle-time", value_parser = clap::value_parser!(u32))]
    pow_puzzle_time: Option<u32>,
    /// Override the puzzle parallelism (lanes).
    #[arg(long = "pow-puzzle-lanes", value_parser = clap::value_parser!(u32))]
    pow_puzzle_lanes: Option<u32>,
    /// Require peers to match SM helper availability.
    #[arg(
        long = "require-sm-handshake-match",
        action = clap::ArgAction::SetTrue,
    )]
    require_sm_handshake_match: bool,
    /// Require peers to match the OpenSSL preview flag.
    #[arg(long = "require-sm-openssl-preview-match", action = clap::ArgAction::SetTrue)]
    require_sm_openssl_preview_match: bool,
}
impl HandshakeUpdateArgs {
    #[cfg(test)]
    fn into_update(self) -> Result<SoranetHandshakeUpdate> {
        let (handshake, _) = self.into_payload()?;
        handshake.ok_or_else(|| {
            eyre!("no handshake overrides provided; specify at least one handshake option")
        })
    }
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        let config = client
            .get_config()
            .wrap_err("failed to fetch configuration for update")?;
        let (handshake_update, network_update) = self.into_payload()?;
        let dto = ConfigUpdateDTO {
            logger: LoggerDTO {
                level: config.logger.level,
                filter: config.logger.filter.clone(),
            },
            network_acl: None,
            network: network_update,
            soranet_handshake: handshake_update,
            transport: None,
            compute_pricing: None,
        };
        client
            .set_config(&dto)
            .wrap_err("failed to submit SoraNet handshake update")?;
        context.println("SoraNet handshake updated.")?;
        Ok(())
    }
    #[allow(clippy::too_many_lines)]
    fn into_payload(self) -> Result<(Option<SoranetHandshakeUpdate>, Option<NetworkUpdate>)> {
        let mut update = SoranetHandshakeUpdate::default();
        let descriptor_commit_touched = if let Some(value) = self.descriptor_commit {
            update.descriptor_commit_hex =
                Some(Self::normalise_hex(&value, "--descriptor-commit")?);
            true
        } else {
            false
        };
        let client_capabilities_touched = if let Some(value) = self.client_capabilities {
            update.client_capabilities_hex =
                Some(Self::normalise_hex(&value, "--client-capabilities")?);
            true
        } else {
            false
        };
        let relay_capabilities_touched = if let Some(value) = self.relay_capabilities {
            update.relay_capabilities_hex =
                Some(Self::normalise_hex(&value, "--relay-capabilities")?);
            true
        } else {
            false
        };
        let kem_touched = if let Some(kem_id) = self.kem_id {
            update.kem_id = Some(kem_id);
            true
        } else {
            false
        };
        let sig_touched = if let Some(sig_id) = self.sig_id {
            update.sig_id = Some(sig_id);
            true
        } else {
            false
        };
        let resume_hash_touched = if let Some(hash_hex) = self.resume_hash {
            update.resume_hash_hex = Some(ResumeHashDirective::Set(Self::normalise_resume_hash(
                &hash_hex,
            )?));
            true
        } else if self.clear_resume_hash {
            update.resume_hash_hex = Some(ResumeHashDirective::Clear);
            true
        } else {
            false
        };
        let mut pow_update = SoranetHandshakePowUpdate::default();
        if let Some(value) = self.pow_difficulty {
            pow_update.difficulty = Some(value);
        }
        if let Some(value) = self.pow_max_future_skew {
            pow_update.max_future_skew_secs = Some(value);
        }
        if let Some(value) = self.pow_min_ttl {
            pow_update.min_ticket_ttl_secs = Some(value);
        }
        if let Some(value) = self.pow_ttl {
            pow_update.ticket_ttl_secs = Some(value);
        }
        let mut pow_touched = self.pow_difficulty.is_some()
            || self.pow_max_future_skew.is_some()
            || self.pow_min_ttl.is_some()
            || self.pow_ttl.is_some();
        let mut puzzle_update = SoranetHandshakePuzzleUpdate::default();
        if let Some(value) = self.pow_puzzle_memory {
            puzzle_update.memory_kib = Some(value);
        }
        if let Some(value) = self.pow_puzzle_time {
            puzzle_update.time_cost = Some(value);
        }
        if let Some(value) = self.pow_puzzle_lanes {
            puzzle_update.lanes = Some(value);
        }
        let puzzle_touched = self.pow_puzzle_memory.is_some()
            || self.pow_puzzle_time.is_some()
            || self.pow_puzzle_lanes.is_some();
        if puzzle_touched {
            pow_update.puzzle = Some(puzzle_update);
            pow_touched = true;
        }
        if pow_touched {
            update.pow = Some(pow_update);
        }
        let handshake_update = if descriptor_commit_touched
            || client_capabilities_touched
            || relay_capabilities_touched
            || kem_touched
            || sig_touched
            || resume_hash_touched
            || pow_touched
        {
            Some(update)
        } else {
            None
        };
        let mut network_update = NetworkUpdate::default();
        if self.require_sm_handshake_match {
            network_update.require_sm_handshake_match = Some(true);
        }
        if self.require_sm_openssl_preview_match {
            network_update.require_sm_openssl_preview_match = Some(true);
        }
        let network_touched =
            self.require_sm_handshake_match || self.require_sm_openssl_preview_match;
        let network_update = if network_touched {
            Some(network_update)
        } else {
            None
        };
        if handshake_update.is_none() && network_update.is_none() {
            return Err(eyre!(
                "no handshake or SM policy overrides provided; specify at least one option"
            ));
        }
        Ok((handshake_update, network_update))
    }
    fn normalise_hex(value: &str, flag: &str) -> Result<String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(eyre!("{flag} must not be empty"));
        }
        let stripped = trimmed.strip_prefix("0x").unwrap_or(trimmed);
        if !stripped.len().is_multiple_of(2) {
            return Err(eyre!(
                "{flag} must contain an even number of hex characters"
            ));
        }
        if !stripped.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(eyre!("{flag} must contain only hex characters [0-9a-fA-F]"));
        }
        Ok(stripped.to_ascii_lowercase())
    }
    fn normalise_resume_hash(value: &str) -> Result<String> {
        let hex = Self::normalise_hex(value, "--resume-hash")?;
        if hex.len() != 64 {
            return Err(eyre!(
                "--resume-hash must be exactly 64 hex characters (32 bytes)"
            ));
        }
        Ok(hex)
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum HandshakeTokenCommand {
    /// Issue an ML-DSA admission token bound to a relay and transcript hash.
    Issue(HandshakeTokenIssueArgs),
    /// Compute the canonical revocation identifier for an admission token.
    Id(HandshakeTokenIdArgs),
    /// Compute the issuer fingerprint from an ML-DSA public key.
    Fingerprint(HandshakeTokenFingerprintArgs),
}
impl_run_for_subcommand!(HandshakeTokenCommand => Issue, Id, Fingerprint);
// Four-byte magic, fixed v1 body, u16 signature length, and the largest
// signature the frame can structurally advertise.
const HANDSHAKE_TOKEN_FILE_MAX_BYTES_V1: usize = 65_671;
const HANDSHAKE_MLDSA_PUBLIC_KEY_MAX_BYTES_V1: usize = 2_592;
#[derive(clap::Args, Debug)]
pub struct HandshakeTokenIssueArgs {
    /// ML-DSA suite used to sign the token (mldsa44, mldsa65, mldsa87).
    #[arg(long = "suite", value_enum, default_value_t = MlDsaSuiteArg::default())]
    suite: MlDsaSuiteArg,
    /// Path to the issuer ML-DSA secret key (raw bytes).
    ///
    /// The file must be owner-private, single-link, and opened without following
    /// symbolic links. Secret key bytes are never accepted directly on argv.
    #[arg(long = "issuer-secret-key", value_name = "PATH")]
    issuer_secret_key: PathBuf,
    /// Path to the issuer ML-DSA public key (raw bytes).
    #[arg(
        long = "issuer-public-key",
        value_name = "PATH",
        conflicts_with = "issuer_public_hex"
    )]
    issuer_public_key: Option<PathBuf>,
    /// Hex-encoded issuer ML-DSA public key.
    #[arg(
        long = "issuer-public-hex",
        value_name = "HEX",
        conflicts_with = "issuer_public_key"
    )]
    issuer_public_hex: Option<String>,
    /// Hex-encoded 32-byte relay identifier bound into the token.
    #[arg(long = "relay-id", value_name = "HEX")]
    relay_id: String,
    /// Hex-encoded 32-byte transcript hash bound into the token.
    #[arg(long = "transcript-hash", value_name = "HEX")]
    transcript_hash: String,
    /// RFC3339 issuance timestamp (defaults to current UTC time).
    #[arg(long = "issued-at", value_name = "RFC3339")]
    issued_at: Option<String>,
    /// RFC3339 expiry timestamp.
    #[arg(
        long = "expires-at",
        value_name = "RFC3339",
        conflicts_with = "ttl_secs"
    )]
    expires_at: Option<String>,
    /// Token lifetime in seconds (defaults to 600s when --expires-at is omitted).
    #[arg(long = "ttl", value_name = "SECONDS", conflicts_with = "expires_at")]
    ttl_secs: Option<u64>,
    /// Token flags (reserved; must be 0 for v1 tokens).
    #[arg(long = "flags", value_parser = clap::value_parser!(u8))]
    flags: Option<u8>,
    /// New path to write the encoded token as an owner-private file.
    ///
    /// Existing paths are never overwritten, and the bearer token is not
    /// printed to standard output.
    #[arg(long = "output", value_name = "PATH")]
    output: PathBuf,
    /// Encoding used when writing the token to --output (base64, hex, binary).
    #[arg(long = "token-encoding", value_enum, default_value_t = TokenOutputFormat::Base64)]
    token_encoding: TokenOutputFormat,
}
struct TokenIssueArtifacts {
    token: AdmissionToken,
    token_bytes: Vec<u8>,
    suite: MlDsaSuiteArg,
    issued_dt: OffsetDateTime,
    expires_dt: OffsetDateTime,
    ttl_secs: u64,
    issuer_fingerprint: [u8; 32],
    relay_id: [u8; 32],
    transcript_hash: [u8; 32],
}
impl TokenIssueArtifacts {
    fn zeroize_encoded_token(&mut self) {
        self.token_bytes.zeroize();
    }
}
impl Drop for TokenIssueArtifacts {
    fn drop(&mut self) {
        self.zeroize_encoded_token();
    }
}
impl HandshakeTokenIssueArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let mut rng = token_issue_rng()?;
        let now = SystemTime::now();
        let artifacts = self.issue_with_rng(context, &mut rng, now)?;
        Self::emit(context, &artifacts, &self.output, self.token_encoding)?;
        Ok(())
    }
    fn issue_with_rng<C, R>(
        &self,
        context: &mut C,
        rng: &mut R,
        default_now: SystemTime,
    ) -> Result<TokenIssueArtifacts>
    where
        C: RunContext,
        R: RngCore + CryptoRng,
    {
        let suite = self.suite;
        let secret_key = read_owner_private_handshake_file(
            &self.issuer_secret_key,
            suite.as_suite().secret_key_len(),
            Some(suite.as_suite().secret_key_len()),
            "--issuer-secret-key",
        )?;
        let public_key = materialise_key_bytes(
            self.issuer_public_key.as_ref(),
            self.issuer_public_hex.as_deref(),
            "--issuer-public-key",
            "--issuer-public-hex",
            suite.as_suite().public_key_len(),
            Some(suite.as_suite().public_key_len()),
        )?;
        let relay_id = parse_hex_array::<32>(&self.relay_id, "--relay-id")?;
        let transcript_hash = parse_hex_array::<32>(&self.transcript_hash, "--transcript-hash")?;
        let issued_dt = match parse_timestamp(self.issued_at.as_deref(), "--issued-at")? {
            Some(explicit) => require_whole_second_token_timestamp(explicit, "--issued-at")?,
            None => canonical_default_token_timestamp(default_now)?,
        };
        let issued_secs = issued_dt.unix_timestamp();
        let expires_dt =
            if let Some(explicit) = parse_timestamp(self.expires_at.as_deref(), "--expires-at")? {
                require_whole_second_token_timestamp(explicit, "--expires-at")?
            } else {
                let ttl = self.ttl_secs.unwrap_or(600);
                if ttl == 0 {
                    return Err(eyre!("--ttl must be greater than zero"));
                }
                issued_dt
                    .checked_add(TimeDelta::seconds(
                        i64::try_from(ttl)
                            .map_err(|_| eyre!("--ttl must fit into a signed 64-bit value"))?,
                    ))
                    .ok_or_else(|| eyre!("computed expiry timestamp overflowed"))?
            };
        let expires_secs = expires_dt.unix_timestamp();
        if expires_secs <= issued_secs {
            return Err(eyre!("token expiry must be greater than the issuance time"));
        }
        let ttl_secs = u64::try_from(expires_secs - issued_secs).map_err(|_| {
            eyre!("token lifetime overflowed when computing expires_at - issued_at")
        })?;
        let issuer_fingerprint = compute_issuer_fingerprint(&public_key);
        let issued_at_instant =
            UNIX_EPOCH
                + Duration::from_secs(u64::try_from(issued_secs).map_err(|_| {
                    eyre!("--issued-at must not be earlier than 1970-01-01T00:00:00Z")
                })?);
        let expires_at_instant = UNIX_EPOCH
            + Duration::from_secs(u64::try_from(expires_secs).map_err(|_| {
                eyre!("--expires-at must not be earlier than 1970-01-01T00:00:00Z")
            })?);
        let flags = self.flags.unwrap_or(0);
        let token = AdmissionToken::mint(
            suite.as_suite(),
            &secret_key,
            issuer_fingerprint,
            relay_id,
            transcript_hash,
            issued_at_instant,
            expires_at_instant,
            flags,
            rng,
        )
        .map_err(|err| map_mint_error(&err, context))?;
        let token_bytes = token.encode();
        Ok(TokenIssueArtifacts {
            token,
            token_bytes,
            suite,
            issued_dt,
            expires_dt,
            ttl_secs,
            issuer_fingerprint,
            relay_id,
            transcript_hash,
        })
    }
    fn emit<C: RunContext>(
        context: &mut C,
        artifacts: &TokenIssueArtifacts,
        output: &Path,
        format: TokenOutputFormat,
    ) -> Result<()> {
        write_token_to_file(output, format, &artifacts.token_bytes)?;
        let token_id = artifacts.token.token_id();
        let token_id_hex = hex::encode(token_id);
        let token_id_b64 = URL_SAFE_NO_PAD.encode(token_id);
        let fingerprint_hex = hex::encode(artifacts.issuer_fingerprint);
        let fingerprint_b64 = URL_SAFE_NO_PAD.encode(artifacts.issuer_fingerprint);
        let relay_id_hex = hex::encode(artifacts.relay_id);
        let transcript_hash_hex = hex::encode(artifacts.transcript_hash);
        let issued_str = artifacts
            .issued_dt
            .format(&Rfc3339)
            .map_err(|err| eyre!("failed to format issued_at: {err}"))?;
        let expires_str = artifacts
            .expires_dt
            .format(&Rfc3339)
            .map_err(|err| eyre!("failed to format expires_at: {err}"))?;
        let mut obj = Map::new();
        obj.insert("suite".into(), Value::from(artifacts.suite.to_string()));
        obj.insert(
            "token_length".into(),
            Value::from(artifacts.token_bytes.len() as u64),
        );
        obj.insert("token_id_hex".into(), Value::from(token_id_hex));
        obj.insert("token_id_base64url".into(), Value::from(token_id_b64));
        obj.insert(
            "issuer_fingerprint_hex".into(),
            Value::from(fingerprint_hex),
        );
        obj.insert(
            "issuer_fingerprint_base64url".into(),
            Value::from(fingerprint_b64),
        );
        obj.insert("relay_id_hex".into(), Value::from(relay_id_hex));
        obj.insert(
            "transcript_hash_hex".into(),
            Value::from(transcript_hash_hex),
        );
        obj.insert(
            "flags".into(),
            Value::from(u64::from(artifacts.token.flags())),
        );
        obj.insert("issued_at".into(), Value::from(issued_str));
        obj.insert("expires_at".into(), Value::from(expires_str));
        obj.insert("ttl_secs".into(), Value::from(artifacts.ttl_secs));
        obj.insert("token_encoding".into(), Value::from(format.describe()));
        obj.insert(
            "output_path".into(),
            Value::from(output.to_string_lossy().into_owned()),
        );
        let text = render_token_issue_text(artifacts, &obj, output, format.describe());
        print_with_optional_text(context, Some(text), &Value::Object(obj))
    }
}
fn require_whole_second_token_timestamp(
    timestamp: OffsetDateTime,
    field: &str,
) -> Result<OffsetDateTime> {
    if timestamp.nanosecond() != 0 {
        return Err(eyre!(
            "{field} must use whole-second precision because admission-token v1 stores seconds"
        ));
    }
    Ok(timestamp)
}
fn canonical_default_token_timestamp(now: SystemTime) -> Result<OffsetDateTime> {
    let seconds = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| eyre!("current time is earlier than the Unix epoch"))?
        .as_secs();
    let seconds = i64::try_from(seconds)
        .map_err(|_| eyre!("current time cannot be represented as an RFC3339 timestamp"))?;
    OffsetDateTime::from_unix_timestamp(seconds).map_err(|error| {
        eyre!("current time cannot be represented as an RFC3339 timestamp: {error}")
    })
}
fn token_issue_rng() -> Result<StdRng> {
    token_issue_rng_from_rng(&mut OsRng)
}
fn token_issue_rng_from_rng<R: TryCryptoRng>(rng: &mut R) -> Result<StdRng> {
    StdRng::try_from_rng(rng).map_err(|error| {
        eyre!("failed to seed SoraNet admission-token RNG from OS entropy: {error}")
    })
}
fn render_token_issue_text(
    artifacts: &TokenIssueArtifacts,
    payload: &Map,
    output: &Path,
    encoding_label: &str,
) -> String {
    let token_id_hex = payload
        .get("token_id_hex")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let fingerprint_hex = payload
        .get("issuer_fingerprint_hex")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let relay_id_hex = payload
        .get("relay_id_hex")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let transcript_hash_hex = payload
        .get("transcript_hash_hex")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let issued_at = payload
        .get("issued_at")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let expires_at = payload
        .get("expires_at")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let ttl_secs = payload
        .get("ttl_secs")
        .and_then(Value::as_u64)
        .unwrap_or(artifacts.ttl_secs);
    let mut out = String::new();
    let _ = writeln!(out, "SoraNet admission token issued");
    let _ = writeln!(out, "suite: {}", artifacts.suite);
    let _ = writeln!(out, "token_id_hex: {token_id_hex}");
    let _ = writeln!(out, "issuer_fingerprint_hex: {fingerprint_hex}");
    let _ = writeln!(out, "relay_id_hex: {relay_id_hex}");
    let _ = writeln!(out, "transcript_hash_hex: {transcript_hash_hex}");
    let _ = writeln!(out, "issued_at: {issued_at}");
    let _ = writeln!(out, "expires_at: {expires_at}");
    let _ = writeln!(out, "ttl_secs: {ttl_secs}");
    let _ = writeln!(out, "output: {} ({encoding_label})", output.display());
    out
}
#[derive(clap::Args, Debug)]
pub struct HandshakeTokenIdArgs {
    /// Path to the admission token frame (binary).
    ///
    /// The bearer token must be supplied through an owner-private, single-link
    /// file and is never accepted directly on argv.
    #[arg(long = "token", value_name = "PATH")]
    path: PathBuf,
}
impl HandshakeTokenIdArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = read_owner_private_handshake_file(
            &self.path,
            HANDSHAKE_TOKEN_FILE_MAX_BYTES_V1,
            None,
            "--token",
        )?;
        let token =
            AdmissionToken::decode(&bytes).map_err(|err| eyre!("failed to decode token: {err}"))?;
        let token_id = token.token_id();
        let token_id_hex = hex::encode(token_id);
        let token_id_b64 = URL_SAFE_NO_PAD.encode(token_id);
        let fingerprint_hex = hex::encode(token.issuer_fingerprint());
        let fingerprint_b64 = URL_SAFE_NO_PAD.encode(token.issuer_fingerprint());
        let issued_dt = OffsetDateTime::from_unix_timestamp(
            i64::try_from(token.issued_at()).map_err(|_| eyre!("issued_at does not fit in i64"))?,
        )
        .map_err(|err| eyre!("invalid issued_at timestamp: {err}"))?;
        let expires_dt = OffsetDateTime::from_unix_timestamp(
            i64::try_from(token.expires_at())
                .map_err(|_| eyre!("expires_at does not fit in i64"))?,
        )
        .map_err(|err| eyre!("invalid expires_at timestamp: {err}"))?;
        let issued_str = issued_dt
            .format(&Rfc3339)
            .map_err(|err| eyre!("failed to format issued_at: {err}"))?;
        let expires_str = expires_dt
            .format(&Rfc3339)
            .map_err(|err| eyre!("failed to format expires_at: {err}"))?;
        let ttl_secs = token.expires_at().saturating_sub(token.issued_at());
        let mut obj = Map::new();
        obj.insert("token_id_hex".into(), Value::from(token_id_hex));
        obj.insert("token_id_base64url".into(), Value::from(token_id_b64));
        obj.insert(
            "issuer_fingerprint_hex".into(),
            Value::from(fingerprint_hex),
        );
        obj.insert(
            "issuer_fingerprint_base64url".into(),
            Value::from(fingerprint_b64),
        );
        obj.insert("flags".into(), Value::from(u64::from(token.flags())));
        obj.insert("issued_at".into(), Value::from(issued_str));
        obj.insert("expires_at".into(), Value::from(expires_str));
        obj.insert("ttl_secs".into(), Value::from(ttl_secs));
        context.print_data(&Value::Object(obj))
    }
}
#[derive(clap::Args, Debug)]
pub struct HandshakeTokenFingerprintArgs {
    /// Path to the ML-DSA public key (raw bytes).
    #[arg(
        long = "public-key",
        value_name = "PATH",
        conflicts_with = "public_key_hex"
    )]
    public_key: Option<PathBuf>,
    /// Hex-encoded ML-DSA public key.
    #[arg(
        long = "public-key-hex",
        value_name = "HEX",
        conflicts_with = "public_key"
    )]
    public_key_hex: Option<String>,
}
impl HandshakeTokenFingerprintArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let public_key = materialise_key_bytes(
            self.public_key.as_ref(),
            self.public_key_hex.as_deref(),
            "--public-key",
            "--public-key-hex",
            HANDSHAKE_MLDSA_PUBLIC_KEY_MAX_BYTES_V1,
            None,
        )?;
        let fingerprint = compute_issuer_fingerprint(&public_key);
        let fingerprint_hex = hex::encode(fingerprint);
        let fingerprint_b64 = URL_SAFE_NO_PAD.encode(fingerprint);
        let mut obj = Map::new();
        obj.insert(
            "public_key_len".into(),
            Value::from(public_key.len() as u64),
        );
        obj.insert(
            "issuer_fingerprint_hex".into(),
            Value::from(fingerprint_hex),
        );
        obj.insert(
            "issuer_fingerprint_base64url".into(),
            Value::from(fingerprint_b64),
        );
        context.print_data(&Value::Object(obj))
    }
}
fn map_mint_error<C: RunContext>(err: &AdmissionTokenMintError, _context: &C) -> eyre::Report {
    eyre!("failed to mint admission token: {err}")
}
fn materialise_key_bytes(
    path: Option<&PathBuf>,
    hex: Option<&str>,
    path_flag: &str,
    hex_flag: &str,
    maximum_bytes: usize,
    exact_bytes: Option<usize>,
) -> Result<Vec<u8>> {
    match (path, hex) {
        (Some(path), None) => {
            read_bounded_direct_handshake_public_file(path, maximum_bytes, exact_bytes, path_flag)
        }
        (None, Some(hex)) => {
            let bytes = decode_hex_string(hex, hex_flag)?;
            validate_handshake_file_length(
                bytes.len(),
                maximum_bytes,
                exact_bytes,
                hex_flag,
                None,
            )?;
            Ok(bytes)
        }
        (Some(_), Some(_)) => Err(eyre!(
            "exactly one of {path_flag} or {hex_flag} must be provided"
        )),
        (None, None) => Err(eyre!("either {path_flag} or {hex_flag} must be provided")),
    }
}
fn validate_handshake_file_length(
    len: usize,
    maximum_bytes: usize,
    exact_bytes: Option<usize>,
    label: &str,
    path: Option<&Path>,
) -> Result<()> {
    let location = path.map_or_else(String::new, |path| format!(" {}", path.display()));
    if len == 0 {
        return Err(eyre!(
            "{label}{location} must contain between 1 and {maximum_bytes} bytes"
        ));
    }
    if let Some(expected) = exact_bytes
        && len != expected
    {
        return Err(eyre!(
            "{label}{location} must contain exactly {expected} bytes, got {len}"
        ));
    }
    if len > maximum_bytes {
        return Err(eyre!(
            "{label}{location} must contain between 1 and {maximum_bytes} bytes"
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn read_bounded_direct_handshake_public_file(
    path: &Path,
    maximum_bytes: usize,
    exact_bytes: Option<usize>,
    label: &str,
) -> Result<Vec<u8>> {
    let named_before = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to inspect {label} {}", path.display()))?;
    if named_before.file_type().is_symlink() || !named_before.is_file() {
        return Err(eyre!(
            "{label} {} must be a regular non-symlink file",
            path.display()
        ));
    }
    let named_len = usize::try_from(named_before.len())
        .map_err(|_| eyre!("{label} length cannot be represented on this host"))?;
    validate_handshake_file_length(named_len, maximum_bytes, exact_bytes, label, Some(path))?;
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .wrap_err_with(|| format!("failed to securely open {label} {}", path.display()))?;
    let mut file = fs::File::from(descriptor);
    let opened = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect opened {label} {}", path.display()))?;
    if !opened.is_file() || !same_direct_handshake_file(&named_before, &opened) {
        return Err(eyre!(
            "{label} {} changed between inspection and open",
            path.display()
        ));
    }
    let expected_len = usize::try_from(opened.len())
        .map_err(|_| eyre!("{label} length cannot be represented on this host"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected_len)
        .map_err(|error| eyre!("failed to reserve {label} buffer: {error}"))?;
    bytes.resize(expected_len, 0);
    file.read_exact(&mut bytes)
        .wrap_err_with(|| format!("failed to read {label} {}", path.display()))?;
    let mut extra = [0u8; 1];
    let grew = file
        .read(&mut extra)
        .wrap_err_with(|| format!("failed to finish reading {label} {}", path.display()))?
        != 0;
    let opened_after = file
        .metadata()
        .wrap_err_with(|| format!("failed to re-inspect opened {label} {}", path.display()))?;
    let named_after = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to re-inspect {label} {}", path.display()))?;
    if grew
        || !same_direct_handshake_file(&opened, &opened_after)
        || !same_direct_handshake_file(&opened, &named_after)
        || opened_after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(eyre!(
            "{label} {} changed while it was read",
            path.display()
        ));
    }
    Ok(bytes)
}
#[cfg(not(unix))]
fn read_bounded_direct_handshake_public_file(
    path: &Path,
    _maximum_bytes: usize,
    _exact_bytes: Option<usize>,
    label: &str,
) -> Result<Vec<u8>> {
    Err(eyre!(
        "{label} {} is unsupported because this platform does not expose a direct no-follow file open",
        path.display()
    ))
}
#[cfg(unix)]
fn same_direct_handshake_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.uid() == right.uid()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
#[cfg(unix)]
fn read_owner_private_handshake_file(
    path: &Path,
    maximum_bytes: usize,
    exact_bytes: Option<usize>,
    label: &str,
) -> Result<Zeroizing<Vec<u8>>> {
    let named_before = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to inspect {label} {}", path.display()))?;
    validate_owner_private_handshake_metadata(
        &named_before,
        maximum_bytes,
        exact_bytes,
        label,
        path,
    )?;
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .wrap_err_with(|| format!("failed to securely open {label} {}", path.display()))?;
    let mut file = fs::File::from(descriptor);
    let opened = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect opened {label} {}", path.display()))?;
    validate_owner_private_handshake_metadata(&opened, maximum_bytes, exact_bytes, label, path)?;
    if !same_owner_private_handshake_file(&named_before, &opened) {
        return Err(eyre!(
            "{label} {} changed between inspection and open",
            path.display()
        ));
    }
    let expected_len = usize::try_from(opened.len())
        .map_err(|_| eyre!("{label} length cannot be represented on this host"))?;
    let mut bytes = Zeroizing::new(Vec::new());
    bytes
        .try_reserve_exact(expected_len)
        .map_err(|error| eyre!("failed to reserve {label} buffer: {error}"))?;
    bytes.resize(expected_len, 0);
    file.read_exact(bytes.as_mut_slice())
        .wrap_err_with(|| format!("failed to read {label} {}", path.display()))?;
    let mut extra = [0u8; 1];
    let grew = file
        .read(&mut extra)
        .wrap_err_with(|| format!("failed to finish reading {label} {}", path.display()))?
        != 0;
    extra.zeroize();
    let opened_after = file
        .metadata()
        .wrap_err_with(|| format!("failed to re-inspect opened {label} {}", path.display()))?;
    let named_after = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to re-inspect {label} {}", path.display()))?;
    if grew
        || !same_owner_private_handshake_file(&opened, &opened_after)
        || !same_owner_private_handshake_file(&opened, &named_after)
        || opened_after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(eyre!(
            "{label} {} changed while it was read",
            path.display()
        ));
    }
    Ok(bytes)
}
#[cfg(unix)]
fn validate_owner_private_handshake_metadata(
    metadata: &fs::Metadata,
    maximum_bytes: usize,
    exact_bytes: Option<usize>,
    label: &str,
    path: &Path,
) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1
    {
        return Err(eyre!(
            "{label} {} must be an owner-private regular non-symlink file with exactly one link",
            path.display()
        ));
    }
    let len = usize::try_from(metadata.len())
        .map_err(|_| eyre!("{label} length cannot be represented on this host"))?;
    validate_handshake_file_length(len, maximum_bytes, exact_bytes, label, Some(path))
}
#[cfg(unix)]
fn same_owner_private_handshake_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    same_direct_handshake_file(left, right) && left.nlink() == 1 && right.nlink() == 1
}
#[cfg(not(unix))]
fn read_owner_private_handshake_file(
    path: &Path,
    _maximum_bytes: usize,
    _exact_bytes: Option<usize>,
    label: &str,
) -> Result<Zeroizing<Vec<u8>>> {
    Err(eyre!(
        "{label} {} is unsupported because this platform does not expose the required owner/mode/link custody checks",
        path.display()
    ))
}
fn write_token_to_file(path: &Path, format: TokenOutputFormat, bytes: &[u8]) -> Result<()> {
    let mut file = create_owner_private_token_output(path)?;
    match format {
        TokenOutputFormat::Base64 => {
            let encoded = Zeroizing::new(URL_SAFE_NO_PAD.encode(bytes));
            file.write_all(encoded.as_bytes())?;
            file.write_all(b"\n")?;
        }
        TokenOutputFormat::Hex => {
            let encoded = Zeroizing::new(hex::encode(bytes));
            file.write_all(encoded.as_bytes())?;
            file.write_all(b"\n")?;
        }
        TokenOutputFormat::Binary => {
            file.write_all(bytes)?;
        }
    }
    file.flush()
        .wrap_err_with(|| format!("failed to flush token output {}", path.display()))?;
    file.sync_all()
        .wrap_err_with(|| format!("failed to sync token output {}", path.display()))?;
    Ok(())
}
#[cfg(unix)]
fn create_owner_private_token_output(path: &Path) -> Result<fs::File> {
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .wrap_err_with(|| {
        format!(
            "failed to create new owner-private token output {}",
            path.display()
        )
    })?;
    let file = fs::File::from(descriptor);
    let metadata = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect token output {}", path.display()))?;
    use std::os::unix::fs::MetadataExt as _;
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1
    {
        return Err(eyre!(
            "token output {} is not an owner-private single-link regular file",
            path.display()
        ));
    }
    Ok(file)
}
#[cfg(not(unix))]
fn create_owner_private_token_output(path: &Path) -> Result<fs::File> {
    Err(eyre!(
        "token output {} is unsupported because this platform does not expose the required owner/mode/link custody checks",
        path.display()
    ))
}
fn decode_hex_string(value: &str, flag: &str) -> Result<Vec<u8>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(eyre!("{flag} must not be empty"));
    }
    if !trimmed.len().is_multiple_of(2) {
        return Err(eyre!(
            "{flag} must contain an even number of hex characters"
        ));
    }
    hex::decode(trimmed).map_err(|err| eyre!("failed to decode {flag}: {err}"))
}
fn parse_hex_array<const N: usize>(value: &str, flag: &str) -> Result<[u8; N]> {
    let trimmed = value.trim();
    let without_prefix = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    if without_prefix.len() != N * 2 {
        return Err(eyre!(
            "{flag} must contain exactly {} hex characters",
            N * 2
        ));
    }
    let mut bytes = [0u8; N];
    decode_to_slice(without_prefix, &mut bytes)
        .map_err(|err| eyre!("failed to decode {flag}: {err}"))?;
    Ok(bytes)
}
fn normalize_hex_digest<const N: usize>(value: &str, flag: &str) -> Result<String> {
    let bytes = parse_hex_array::<N>(value, flag)?;
    Ok(encode(bytes))
}
fn parse_alias_label(raw: &str) -> Result<String> {
    let (namespace_raw, name_raw) = raw
        .split_once(':')
        .ok_or_else(|| eyre!("alias `{raw}` must use the `namespace:name` form"))?;
    let namespace = Name::from_str(namespace_raw.trim())
        .map_err(|err| eyre!("invalid alias namespace `{namespace_raw}`: {err}"))?;
    let name = Name::from_str(name_raw.trim())
        .map_err(|err| eyre!("invalid alias name `{name_raw}`: {err}"))?;
    Ok(format!("{namespace}:{name}"))
}
const ROUTE_HEADER_ORDER: &[&str] = &[
    "Sora-Name",
    "Sora-Content-CID",
    "Sora-Proof",
    "Sora-Proof-Status",
    "Sora-Route-Binding",
    "Content-Security-Policy",
    "Strict-Transport-Security",
    "Permissions-Policy",
];
const DEFAULT_ROUTE_CSP: &str = "default-src 'self'; img-src 'self' data:; font-src 'self'; style-src 'self' 'unsafe-inline'; object-src 'none'; frame-ancestors 'none'; base-uri 'self'";
const DEFAULT_ROUTE_PERMISSIONS: &str = "accelerometer=(), ambient-light-sensor=(), autoplay=(), camera=(), clipboard-read=(self), clipboard-write=(self), encrypted-media=(), fullscreen=(self), geolocation=(), gyroscope=(), hid=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), speaker-selection=(), usb=(), xr-spatial-tracking=()";
const DEFAULT_ROUTE_HSTS_MAX_AGE: u32 = 63_072_000;
struct RouteBindingContext {
    manifest_json: PathBuf,
    alias: Option<String>,
    hostname: String,
    route_label: Option<String>,
    proof_status: Option<String>,
    include_csp: bool,
    include_permissions: bool,
    include_hsts: bool,
    generated_at: OffsetDateTime,
}
struct RouteBindingOutput {
    content_cid: String,
    route_binding: String,
    headers: BTreeMap<String, String>,
    headers_template: String,
}
fn build_route_binding(context: &RouteBindingContext) -> Result<RouteBindingOutput> {
    let manifest_bytes = fs::read(&context.manifest_json).wrap_err_with(|| {
        format!(
            "failed to read manifest JSON from `{}`",
            context.manifest_json.display()
        )
    })?;
    let manifest: Value = norito::json::from_slice(&manifest_bytes).wrap_err_with(|| {
        format!(
            "failed to parse manifest JSON from `{}`",
            context.manifest_json.display()
        )
    })?;
    let root_bytes = manifest_root_bytes(&manifest)?;
    if root_bytes.is_empty() {
        return Err(eyre!("manifest root CID payload was empty"));
    }
    let content_cid = format!("b{}", encode_base32_lower(&root_bytes));
    let mut headers = BTreeMap::new();
    headers.insert("Sora-Content-CID".into(), content_cid.clone());
    if let Some(alias) = context.alias.as_deref() {
        headers.insert("Sora-Name".into(), alias.to_string());
        let proof_payload = norito::json!({
            "alias": alias,
            "manifest": content_cid,
        });
        let proof_bytes = norito::json::to_vec(&proof_payload)
            .map_err(|err| eyre!("failed to encode proof payload: {err}"))?;
        headers.insert("Sora-Proof".into(), STANDARD.encode(proof_bytes));
        let status = context
            .proof_status
            .clone()
            .unwrap_or_else(|| "ok".to_string());
        headers.insert("Sora-Proof-Status".into(), status);
    }
    let generated_at = context
        .generated_at
        .format(&Rfc3339)
        .map_err(|err| eyre!("failed to format timestamp: {err}"))?;
    let mut binding_parts = vec![
        format!("host={}", context.hostname),
        format!("cid={content_cid}"),
        format!("generated_at={generated_at}"),
    ];
    if let Some(label) = context.route_label.as_deref() {
        binding_parts.push(format!("label={label}"));
    }
    let route_binding = binding_parts.join(";");
    headers.insert("Sora-Route-Binding".into(), route_binding.clone());
    if context.include_csp {
        headers.insert("Content-Security-Policy".into(), DEFAULT_ROUTE_CSP.into());
    }
    if context.include_hsts {
        headers.insert(
            "Strict-Transport-Security".into(),
            format!("max-age={DEFAULT_ROUTE_HSTS_MAX_AGE}; includeSubDomains; preload"),
        );
    }
    if context.include_permissions {
        headers.insert(
            "Permissions-Policy".into(),
            DEFAULT_ROUTE_PERMISSIONS.into(),
        );
    }
    let headers_template = format_headers_template(&headers);
    Ok(RouteBindingOutput {
        content_cid,
        route_binding,
        headers,
        headers_template,
    })
}
fn manifest_root_bytes(manifest: &Value) -> Result<Vec<u8>> {
    if let Some(array) = manifest.get("root_cid").and_then(Value::as_array) {
        let mut bytes = Vec::with_capacity(array.len());
        for value in array {
            let number = value.as_i64().ok_or_else(|| {
                eyre!("root_cid entries must be integers, found {value:?} instead")
            })?;
            if !(0..=255).contains(&number) {
                return Err(eyre!(
                    "root_cid entries must be between 0 and 255 inclusive (found {number})"
                ));
            }
            bytes.push(u8::try_from(number).expect("checked root_cid bounds"));
        }
        return Ok(bytes);
    }
    if let Some(array) = manifest.get("root_cids_hex").and_then(Value::as_array) {
        for value in array {
            if let Some(hex_str) = value.as_str()
                && let Ok(decoded) = decode(hex_str.trim())
                && !decoded.is_empty()
            {
                return Ok(decoded);
            }
        }
    }
    if let Some(hex_value) = manifest.get("root_cid_hex").and_then(Value::as_str) {
        return decode(hex_value.trim())
            .map_err(|err| eyre!("failed to decode root_cid_hex value: {err}"));
    }
    Err(eyre!(
        "manifest JSON is missing `root_cid`, `root_cids_hex`, or `root_cid_hex` fields"
    ))
}
fn encode_base32_lower(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    if bytes.is_empty() {
        return String::new();
    }
    let mut acc: u32 = 0;
    let mut bits = 0;
    let mut output = String::new();
    for &byte in bytes {
        acc = (acc << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            let index = ((acc >> (bits - 5)) & 0x1F) as usize;
            output.push(ALPHABET[index] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        let index = ((acc << (5 - bits)) & 0x1F) as usize;
        output.push(ALPHABET[index] as char);
    }
    output
}
fn format_headers_template(headers: &BTreeMap<String, String>) -> String {
    let mut lines = Vec::new();
    for &key in ROUTE_HEADER_ORDER {
        if let Some(value) = headers.get(key) {
            lines.push(format!("{key}: {value}"));
        }
    }
    for (key, value) in headers {
        if ROUTE_HEADER_ORDER.contains(&key.as_str()) {
            continue;
        }
        lines.push(format!("{key}: {value}"));
    }
    let mut rendered = lines.join("\n");
    rendered.push('\n');
    rendered
}
fn headers_to_value(headers: &BTreeMap<String, String>) -> Map {
    let mut map = Map::new();
    for (key, value) in headers {
        map.insert(key.clone(), Value::from(value.clone()));
    }
    map
}
fn write_optional_output(path: Option<&PathBuf>, contents: &str) -> Result<()> {
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!("failed to create parent directory for `{}`", path.display())
            })?;
        }
        fs::write(path, contents)
            .wrap_err_with(|| format!("failed to write `{}`", path.display()))?;
    }
    Ok(())
}
fn build_cache_invalidation_payload(
    aliases: &[String],
    manifest_digest: &str,
    car_digest: Option<&str>,
    release_tag: Option<&str>,
) -> Value {
    let mut map = Map::new();
    map.insert(
        "aliases".into(),
        Value::Array(aliases.iter().cloned().map(Value::from).collect()),
    );
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(manifest_digest.to_owned()),
    );
    map.insert(
        "car_digest_hex".into(),
        car_digest.map_or(Value::Null, |hex| Value::from(hex.to_owned())),
    );
    map.insert(
        "release_tag".into(),
        release_tag.map_or(Value::Null, |tag| Value::from(tag.to_owned())),
    );
    Value::Object(map)
}
fn render_cache_invalidation_curl(endpoint: &str, auth_env: &str, payload_json: &str) -> String {
    let mut lines = Vec::new();
    lines.push(format!("curl -X POST {endpoint}"));
    lines.push("  -H 'Content-Type: application/json'".to_string());
    if !auth_env.trim().is_empty() {
        lines.push(format!("  -H \"Authorization: Bearer ${auth_env}\""));
    }
    let escaped = shell_escape_single_quotes(payload_json);
    lines.push(format!("  --data '{escaped}'"));
    lines.join(" \\\n")
}
fn shell_escape_single_quotes(input: &str) -> String {
    if input.contains('\'') {
        input.replace('\'', "'\"'\"'")
    } else {
        input.to_owned()
    }
}
fn render_handshake_summary<C: RunContext>(
    context: &mut C,
    summary: &SoranetHandshakeSummary,
) -> Result<()> {
    context.println(format_args!(
        "descriptor_commit_hex: {}",
        summary.descriptor_commit_hex
    ))?;
    context.println(format_args!(
        "client_capabilities_hex: {}",
        summary.client_capabilities_hex
    ))?;
    context.println(format_args!(
        "relay_capabilities_hex: {}",
        summary.relay_capabilities_hex
    ))?;
    context.println(format_args!("kem_id: {}", summary.kem_id))?;
    context.println(format_args!("sig_id: {}", summary.sig_id))?;
    context.println(format_args!(
        "resume_hash_hex: {}",
        summary
            .resume_hash_hex
            .as_deref()
            .unwrap_or("<not configured>")
    ))?;
    context.println(format_args!("pow.difficulty: {}", summary.pow.difficulty))?;
    context.println(format_args!(
        "pow.max_future_skew_secs: {}",
        summary.pow.max_future_skew_secs
    ))?;
    context.println(format_args!(
        "pow.min_ticket_ttl_secs: {}",
        summary.pow.min_ticket_ttl_secs
    ))?;
    context.println(format_args!(
        "pow.ticket_ttl_secs: {}",
        summary.pow.ticket_ttl_secs
    ))?;
    let puzzle = summary.pow.puzzle;
    context.println(format_args!("pow.puzzle.memory_kib: {}", puzzle.memory_kib))?;
    context.println(format_args!("pow.puzzle.time_cost: {}", puzzle.time_cost))?;
    context.println(format_args!("pow.puzzle.lanes: {}", puzzle.lanes))?;
    Ok(())
}
#[derive(clap::Subcommand, Debug)]
pub enum GatewayCommand {
    /// Emit a TOML snippet with gateway configuration defaults.
    TemplateConfig(GatewayTemplateConfigArgs),
    /// Derive canonical/vanity hostnames for a provider.
    GenerateHosts(GatewayGenerateHostsArgs),
    /// Render the headers + route binding plan for a manifest rollout.
    RoutePlan(GatewayRoutePlanArgs),
    /// Generate a cache invalidation payload and curl snippet for GAR/SoraFS gateways.
    CacheInvalidate(GatewayCacheInvalidateArgs),
    /// Direct-mode planning and configuration helpers.
    #[command(subcommand)]
    DirectMode(GatewayDirectModeCommand),
}
impl_run_for_subcommand!(GatewayCommand => TemplateConfig, GenerateHosts, RoutePlan, CacheInvalidate, DirectMode);
#[derive(clap::Subcommand, Debug)]
pub enum ToolkitCommand {
    /// Package a payload into a CAR + manifest bundle using the canonical tooling.
    Pack(ToolkitPackArgs),
}
impl_run_for_subcommand!(ToolkitCommand => Pack);
#[derive(clap::Subcommand, Debug)]
pub enum GuardDirectoryCommand {
    /// Fetch a guard directory snapshot over HTTPS, verify it, and emit a summary.
    Fetch(GuardDirectoryFetchArgs),
    /// Authenticate a guard directory snapshot stored on disk.
    Verify(GuardDirectoryVerifyArgs),
    /// Inspect snapshot structure without claiming authenticity or freshness.
    Inspect(GuardDirectoryInspectArgs),
}
impl_run_for_subcommand!(GuardDirectoryCommand => Fetch, Verify, Inspect);
#[derive(clap::Args, Debug)]
pub struct GuardDirectoryFetchArgs {
    /// URLs publishing the guard directory snapshot (first success wins).
    #[arg(long = "url", value_name = "URL", required = true)]
    pub url: Vec<String>,
    /// Path where the verified snapshot will be stored (optional).
    #[arg(long = "output", value_name = "PATH")]
    pub output: Option<PathBuf>,
    /// Trusted domain-separated BLAKE3 digest of the exact snapshot bytes.
    #[arg(long = "expected-snapshot-digest", value_name = "HEX")]
    pub expected_snapshot_digest: String,
    /// HTTP timeout in seconds (defaults to 30s).
    #[arg(long = "timeout-secs", value_name = "SECS", default_value = "30")]
    pub timeout_secs: u64,
    /// Allow overwriting an existing file at --output.
    #[arg(long = "overwrite")]
    pub overwrite: bool,
}
impl Run for GuardDirectoryFetchArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if self.url.is_empty() {
            return Err(eyre!(
                "at least one --url must be supplied when fetching guard directory snapshots"
            ));
        }
        let timeout = Duration::from_secs(self.timeout_secs.max(1));
        let client = BlockingHttpClient::builder()
            .timeout(timeout)
            .user_agent("sorafs-cli guard-directory")
            .build()
            .wrap_err("failed to construct HTTP client")?;
        let mut errors = Vec::new();
        let mut snapshot: Option<Vec<u8>> = None;
        for url in &self.url {
            match client.get(url).send() {
                Ok(response) => match response.error_for_status() {
                    Ok(mut success) => {
                        let content_length = success.content_length();
                        match read_guard_directory_http_body_bounded(&mut success, content_length) {
                            Ok(bytes) => {
                                snapshot = Some(bytes);
                                break;
                            }
                            Err(err) => {
                                errors.push(format!("{url}: failed to read body: {err}"));
                            }
                        }
                    }
                    Err(err) => {
                        errors.push(format!("{url}: HTTP error {err}"));
                    }
                },
                Err(err) => {
                    errors.push(format!("{url}: {err}"));
                }
            }
        }
        let bytes = snapshot.ok_or_else(|| {
            eyre!(
                "failed to fetch guard directory from {} url(s): {}",
                self.url.len(),
                errors.join("; ")
            )
        })?;
        let now_unix = OffsetDateTime::now_utc().unix_timestamp();
        let summary =
            authenticate_guard_directory_bytes(&bytes, &self.expected_snapshot_digest, now_unix)?;
        if let Some(path) = &self.output {
            write_guard_directory_snapshot(path, &bytes, self.overwrite)?;
        }
        context.print_data(&summary)
    }
}
fn read_guard_directory_http_body_bounded<R: Read>(
    reader: R,
    content_length: Option<u64>,
) -> io::Result<Vec<u8>> {
    read_guard_directory_http_body_with_limit(
        reader,
        content_length,
        iroha_crypto::soranet::directory::GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1,
    )
}
fn read_guard_directory_http_body_with_limit<R: Read>(
    mut reader: R,
    content_length: Option<u64>,
    max_bytes: usize,
) -> io::Result<Vec<u8>> {
    let max_bytes_u64 = u64::try_from(max_bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "guard directory HTTP body limit cannot be represented as u64",
        )
    })?;
    if content_length.is_some_and(|length| length > max_bytes_u64) {
        return Err(guard_directory_http_body_too_large(max_bytes));
    }
    let capacity = content_length
        .and_then(|length| usize::try_from(length).ok())
        .unwrap_or(0)
        .min(max_bytes);
    let mut bytes = Vec::with_capacity(capacity);
    reader
        .by_ref()
        .take(max_bytes_u64.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(guard_directory_http_body_too_large(max_bytes));
    }
    Ok(bytes)
}
fn guard_directory_http_body_too_large(max_bytes: usize) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("guard directory HTTP body exceeds the {max_bytes}-byte first-release limit"),
    )
}
#[derive(clap::Args, Debug)]
pub struct GuardDirectoryVerifyArgs {
    /// Path to the guard directory snapshot to verify.
    #[arg(long = "path", value_name = "PATH")]
    pub path: PathBuf,
    /// Trusted domain-separated BLAKE3 digest of the exact snapshot bytes.
    #[arg(long = "expected-snapshot-digest", value_name = "HEX")]
    pub expected_snapshot_digest: String,
}
impl Run for GuardDirectoryVerifyArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = read_guard_directory_snapshot_file(&self.path).wrap_err_with(|| {
            format!(
                "failed to read guard directory snapshot from `{}`",
                self.path.display()
            )
        })?;
        let now_unix = OffsetDateTime::now_utc().unix_timestamp();
        let summary =
            authenticate_guard_directory_bytes(&bytes, &self.expected_snapshot_digest, now_unix)?;
        context.print_data(&summary)
    }
}
#[derive(clap::Args, Debug)]
pub struct GuardDirectoryInspectArgs {
    /// Path to the guard directory snapshot to inspect.
    #[arg(long = "path", value_name = "PATH")]
    pub path: PathBuf,
}
impl Run for GuardDirectoryInspectArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = read_guard_directory_snapshot_file(&self.path).wrap_err_with(|| {
            format!(
                "failed to read guard directory snapshot from `{}`",
                self.path.display()
            )
        })?;
        let summary = inspect_guard_directory_bytes(&bytes)?;
        context.print_data(&summary)
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum GatewayDirectModeCommand {
    /// Analyse manifest/admission data and emit a direct-mode readiness plan.
    Plan(GatewayDirectModePlanArgs),
    /// Emit a configuration snippet enabling direct-mode overrides from a plan.
    Enable(GatewayDirectModeEnableArgs),
    /// Emit a configuration snippet restoring default gateway security settings.
    Rollback(GatewayDirectModeRollbackArgs),
}
impl_run_for_subcommand!(GatewayDirectModeCommand => Plan, Enable, Rollback);
#[derive(clap::Args, Debug)]
pub struct GatewayDirectModePlanArgs {
    /// Path to the Norito-encoded manifest (`.to`) file to analyse.
    #[arg(long, value_name = "PATH")]
    pub manifest: PathBuf,
    /// Optional provider admission envelope (`.to`) for capability detection.
    #[arg(long = "admission-envelope", value_name = "PATH")]
    pub admission_envelope: Option<PathBuf>,
    /// Override provider identifier (hex) when no admission envelope is supplied.
    #[arg(long = "provider-id", value_name = "HEX")]
    pub provider_id: Option<String>,
    /// Override chain id (defaults to the CLI configuration chain id).
    #[arg(long = "chain-id")]
    pub chain_id: Option<String>,
    /// URL scheme to use for generated direct-CAR endpoints (default: https).
    #[arg(long, default_value = "https")]
    pub scheme: String,
}
impl Run for GatewayDirectModePlanArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let manifest_bytes = fs::read(&self.manifest).wrap_err_with(|| {
            format!("failed to read manifest from `{}`", self.manifest.display())
        })?;
        let manifest: ManifestV1 =
            norito::decode_from_bytes(&manifest_bytes).wrap_err("failed to decode manifest")?;
        let manifest_digest = manifest
            .digest()
            .wrap_err("failed to compute manifest digest")?;
        let manifest_digest_hex = hex::encode(manifest_digest.as_bytes());
        let envelope = if let Some(path) = &self.admission_envelope {
            let bytes = fs::read(path).wrap_err_with(|| {
                format!(
                    "failed to read admission envelope from `{}`",
                    path.display()
                )
            })?;
            let decoded: ProviderAdmissionEnvelopeV1 = norito::decode_from_bytes(&bytes)
                .wrap_err("failed to decode admission envelope")?;
            decoded
                .validate()
                .wrap_err("admission envelope validation failed")?;
            Some(decoded)
        } else {
            None
        };
        let provider_id = if let Some(hex) = self.provider_id {
            parse_hex_array::<32>(&hex, "provider_id")?
        } else if let Some(env) = envelope.as_ref() {
            env.advert_body.provider_id
        } else {
            return Err(eyre!(
                "provider identifier required; pass --provider-id or --admission-envelope"
            ));
        };
        let chain_id = self
            .chain_id
            .unwrap_or_else(|| context.config().chain.as_str().to_owned());
        let host_input = HostMappingInput {
            chain_id: chain_id.as_str(),
            provider_id: &provider_id,
        };
        let host_summary = host_input.to_summary();
        let direct_car = host_input
            .direct_car_locator(&self.scheme, &manifest_digest_hex)
            .wrap_err("invalid URL scheme for direct CAR locator")?;
        let capability_summary = detect_manifest_capabilities(
            Some(&manifest),
            envelope.as_ref().map(|env| &env.advert_body),
        );
        let plan = DirectModePlanOutput::from_components(
            &chain_id,
            provider_id,
            manifest_digest_hex,
            host_summary,
            direct_car,
            capability_summary,
        );
        context.print_data(&plan)
    }
}
#[derive(clap::Args, Debug)]
pub struct ToolkitPackArgs {
    /// Payload path (file or directory) to package into a CAR archive.
    pub input: PathBuf,
    /// Path to write the Norito manifest (`.to`). If omitted, no manifest file is emitted.
    #[arg(long = "manifest-out", value_name = "PATH")]
    pub manifest_out: Option<PathBuf>,
    /// Path to write the CAR archive.
    #[arg(long = "car-out", value_name = "PATH")]
    pub car_out: Option<PathBuf>,
    /// Path to write the JSON report (defaults to stdout).
    #[arg(long = "json-out", value_name = "PATH")]
    pub json_out: Option<PathBuf>,
    /// Path to write the hybrid payload envelope (binary).
    #[arg(long = "hybrid-envelope-out", value_name = "PATH")]
    pub hybrid_envelope_out: Option<PathBuf>,
    /// Path to write the hybrid payload envelope (JSON).
    #[arg(long = "hybrid-envelope-json-out", value_name = "PATH")]
    pub hybrid_envelope_json_out: Option<PathBuf>,
    /// Hex-encoded X25519 public key used for hybrid envelope encryption.
    #[arg(long = "hybrid-recipient-x25519", value_name = "HEX")]
    pub hybrid_recipient_x25519: Option<String>,
    /// Hex-encoded Kyber public key used for hybrid envelope encryption.
    #[arg(long = "hybrid-recipient-kyber", value_name = "HEX")]
    pub hybrid_recipient_kyber: Option<String>,
}
impl Run for ToolkitPackArgs {
    #[allow(clippy::too_many_lines)]
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let ToolkitPackArgs {
            input,
            manifest_out,
            car_out,
            json_out,
            hybrid_envelope_out,
            hybrid_envelope_json_out,
            hybrid_recipient_x25519,
            hybrid_recipient_kyber,
        } = self;
        let descriptor = chunker_registry::default_descriptor();
        let (plan, payload) = build_pack_plan(&input, descriptor.profile)?;
        if plan.chunk_profile != descriptor.profile {
            return Err(eyre!("computed chunk plan used unexpected profile"));
        }
        let mut chunk_store = ChunkStore::with_profile(descriptor.profile);
        chunk_store
            .ingest_plan(&payload, &plan)
            .wrap_err("failed to ingest the validated CAR plan into the PoR chunk store")?;
        if chunk_store.por_tree().chunks().len() != plan.chunks.len() {
            return Err(eyre!("chunk store PoR layout diverged from CAR plan"));
        }
        let car_stats = write_pack_car(car_out.as_ref(), &plan, &payload)?;
        if car_stats.chunk_profile != descriptor.profile {
            return Err(eyre!("computed CAR used unexpected chunking profile"));
        }
        let root_cid = car_stats
            .root_cids
            .first()
            .cloned()
            .ok_or_else(|| eyre!("CAR emission produced no root CID"))?;
        let car_archive_digest = *car_stats.car_archive_digest.as_bytes();
        let produce_hybrid_envelope = hybrid_envelope_out.is_some()
            || hybrid_envelope_json_out.is_some()
            || hybrid_recipient_x25519.is_some()
            || hybrid_recipient_kyber.is_some();
        let mut metadata: Vec<(String, String)> = Vec::new();
        let hybrid_recipient = if produce_hybrid_envelope {
            let x25519_hex = hybrid_recipient_x25519.as_deref().ok_or_else(|| {
                eyre!("hybrid manifest envelopes require --hybrid-recipient-x25519")
            })?;
            let kyber_hex = hybrid_recipient_kyber.as_deref().ok_or_else(|| {
                eyre!("hybrid manifest envelopes require --hybrid-recipient-kyber")
            })?;
            let x25519_bytes =
                decode(x25519_hex).wrap_err("invalid hex for --hybrid-recipient-x25519")?;
            let kyber_bytes =
                decode(kyber_hex).wrap_err("invalid hex for --hybrid-recipient-kyber")?;
            ensure_metadata_entry(&mut metadata, "manifest.requires_envelope", "true");
            let suite_label = HybridSuite::X25519MlKem768ChaCha20Poly1305.to_string();
            ensure_metadata_entry(&mut metadata, "manifest.hybrid_suite", &suite_label);
            Some(
                HybridPublicKey::from_bytes(&x25519_bytes, &kyber_bytes)
                    .wrap_err("invalid hybrid recipient key material")?,
            )
        } else {
            None
        };
        let chunk_profile = ChunkingProfileV1::from_descriptor(descriptor);
        let chunk_digest_sha3 = compute_chunk_digest_sha3(&plan.chunks);
        let mut builder = ManifestBuilder::new()
            .root_cid(root_cid.clone())
            .dag_codec(DagCodecId(car_stats.dag_codec))
            .chunking_profile(chunk_profile.clone())
            .chunk_digest_sha3_256(chunk_digest_sha3)
            .por_root(*chunk_store.por_tree().root())
            .content_length(plan.content_length)
            .car_digest(car_archive_digest)
            .car_size(car_stats.car_size)
            .pin_policy(PinPolicy {
                min_replicas: 3,
                storage_class: ManifestStorageClass::Hot,
                retention_epoch: 86_400,
            })
            .governance(GovernanceProofs::default());
        if !metadata.is_empty() {
            builder = builder.extend_metadata(metadata);
        }
        let manifest = builder.build().wrap_err("failed to build manifest")?;
        let manifest_bytes = manifest.encode().wrap_err("failed to encode manifest")?;
        let manifest_digest = manifest
            .digest()
            .wrap_err("failed to compute manifest digest")?;
        let manifest_filename = manifest_out.as_ref().and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        });
        let hybrid_output = if let Some(recipient) = hybrid_recipient {
            let aad = build_hybrid_manifest_aad(
                &manifest_digest,
                chunk_digest_sha3,
                manifest_filename.as_deref(),
            );
            let mut rng = OsRng;
            let envelope = encrypt_payload(&manifest_bytes, &aad, &recipient, &mut rng)
                .wrap_err("failed to encrypt hybrid payload envelope")?;
            let envelope_bytes =
                norito::to_bytes(&envelope).wrap_err("failed to encode hybrid payload envelope")?;
            Some(HybridEnvelopeArtefact {
                envelope,
                bytes: envelope_bytes,
                aad,
            })
        } else {
            None
        };
        if let Some(path) = manifest_out.as_ref() {
            ensure_parent_dir(path)?;
            fs::write(path, &manifest_bytes)
                .wrap_err_with(|| format!("failed to write manifest to `{}`", path.display()))?;
        }
        if let Some(hybrid) = hybrid_output.as_ref() {
            if let Some(path) = hybrid_envelope_out.as_ref() {
                ensure_parent_dir(path)?;
                fs::write(path, &hybrid.bytes).wrap_err_with(|| {
                    format!("failed to write hybrid envelope to `{}`", path.display())
                })?;
            }
            if let Some(path) = hybrid_envelope_json_out.as_ref() {
                ensure_parent_dir(path)?;
                let json_value = norito::json::to_value(&hybrid.envelope)
                    .wrap_err("failed to encode hybrid envelope JSON")?;
                let mut json_string = norito::json::to_string_pretty(&json_value)
                    .wrap_err("failed to render hybrid envelope JSON")?;
                json_string.push('\n');
                fs::write(path, json_string.as_bytes()).wrap_err_with(|| {
                    format!(
                        "failed to write hybrid envelope JSON to `{}`",
                        path.display()
                    )
                })?;
            }
        }
        let mut report = build_pack_report(&PackReportContext {
            profile: &chunk_profile,
            plan: &plan,
            car_stats: &car_stats,
            root_cid: &root_cid,
            manifest: &manifest,
            manifest_bytes: &manifest_bytes,
            manifest_digest: &manifest_digest,
            por_tree: chunk_store.por_tree(),
        })?;
        let report_object = report
            .as_object_mut()
            .ok_or_else(|| eyre!("internal error: report root is not a JSON object"))?;
        if let Some(hybrid) = hybrid_output.as_ref() {
            let mut obj = Map::new();
            obj.insert("suite".into(), Value::from(hybrid.envelope.suite.clone()));
            obj.insert(
                "nonce_hex".into(),
                Value::from(encode(hybrid.envelope.nonce)),
            );
            obj.insert(
                "ciphertext_len".into(),
                Value::from(hybrid.envelope.ciphertext.len() as u64),
            );
            obj.insert(
                "ciphertext_blake3".into(),
                Value::from(encode(blake3::hash(&hybrid.envelope.ciphertext).as_bytes())),
            );
            obj.insert("aad_hex".into(), Value::from(encode(&hybrid.aad)));
            obj.insert(
                "encoded_base64".into(),
                Value::from(STANDARD.encode(&hybrid.bytes)),
            );
            if let Some(path) = hybrid_envelope_out.as_ref() {
                obj.insert("binary_out".into(), Value::from(path.display().to_string()));
            }
            if let Some(path) = hybrid_envelope_json_out.as_ref() {
                obj.insert("json_out".into(), Value::from(path.display().to_string()));
            }
            report_object.insert("hybrid_envelope".into(), Value::Object(obj));
        }
        let mut report_string =
            norito::json::to_string_pretty(&report).wrap_err("failed to render JSON report")?;
        if !report_string.ends_with('\n') {
            report_string.push('\n');
        }
        let mut report_written_to_stdout = false;
        if let Some(path) = json_out.as_ref() {
            if path == Path::new("-") {
                context.println(report_string.trim_end())?;
                report_written_to_stdout = true;
            } else {
                ensure_parent_dir(path)?;
                fs::write(path, report_string.as_bytes()).wrap_err_with(|| {
                    format!("failed to write JSON report to `{}`", path.display())
                })?;
            }
        }
        if !report_written_to_stdout {
            context.println(report_string.trim_end())?;
        }
        Ok(())
    }
}
const HYBRID_MANIFEST_AAD_DOMAIN: &[u8] = b"sorafs.hybrid.manifest.v1";
struct HybridEnvelopeArtefact {
    envelope: HybridPayloadEnvelopeV1,
    bytes: Vec<u8>,
    aad: Vec<u8>,
}
struct PackReportContext<'a> {
    profile: &'a ChunkingProfileV1,
    plan: &'a CarBuildPlan,
    car_stats: &'a CarWriteStats,
    root_cid: &'a [u8],
    manifest: &'a ManifestV1,
    manifest_bytes: &'a [u8],
    manifest_digest: &'a blake3::Hash,
    por_tree: &'a PorMerkleTree,
}
fn build_pack_plan(input: &Path, profile: ChunkProfile) -> Result<(CarBuildPlan, Vec<u8>)> {
    let metadata =
        fs::metadata(input).wrap_err_with(|| format!("failed to access `{}`", input.display()))?;
    if metadata.is_dir() {
        CarBuildPlan::from_directory_with_profile(input, profile)
            .map_err(|err| eyre!("car planning failed: {err}"))
    } else if metadata.is_file() {
        let payload = fs::read(input)
            .wrap_err_with(|| format!("failed to read input `{}`", input.display()))?;
        let plan = CarBuildPlan::single_file_with_profile(&payload, profile)
            .map_err(|err| eyre!("car planning failed: {err}"))?;
        Ok((plan, payload))
    } else {
        Err(eyre!("input must be a file or directory"))
    }
}
fn write_pack_car(
    car_out: Option<&PathBuf>,
    plan: &CarBuildPlan,
    payload: &[u8],
) -> Result<CarWriteStats> {
    let writer = CarWriter::new(plan, payload).wrap_err("failed to prepare CAR writer")?;
    if let Some(path) = car_out {
        ensure_parent_dir(path)?;
        let file = fs::File::create(path)
            .wrap_err_with(|| format!("failed to create `{}`", path.display()))?;
        let mut buf = io::BufWriter::new(file);
        let stats = writer.write_to(&mut buf).wrap_err("failed to write CAR")?;
        buf.flush()
            .wrap_err_with(|| format!("failed to flush `{}`", path.display()))?;
        Ok(stats)
    } else {
        let mut sink = io::sink();
        writer
            .write_to(&mut sink)
            .wrap_err("failed to compute CAR metadata")
    }
}
fn ensure_metadata_entry(metadata: &mut Vec<(String, String)>, key: &str, value: &str) {
    if metadata
        .iter()
        .any(|(existing_key, _)| existing_key.eq_ignore_ascii_case(key))
    {
        return;
    }
    metadata.push((key.to_string(), value.to_string()));
}
fn compute_chunk_digest_sha3(chunks: &[CarChunk]) -> [u8; 32] {
    let mut hasher = Sha3::v256();
    for chunk in chunks {
        hasher.update(&chunk.offset.to_le_bytes());
        hasher.update(&u64::from(chunk.length).to_le_bytes());
        hasher.update(&chunk.digest);
    }
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}
fn build_hybrid_manifest_aad(
    manifest_digest: &blake3::Hash,
    chunk_digest_sha3: [u8; 32],
    manifest_filename: Option<&str>,
) -> Vec<u8> {
    let mut aad = Vec::with_capacity(
        HYBRID_MANIFEST_AAD_DOMAIN.len()
            + manifest_digest.as_bytes().len()
            + chunk_digest_sha3.len()
            + manifest_filename.map_or(0, |name| 4 + name.len()),
    );
    aad.extend_from_slice(HYBRID_MANIFEST_AAD_DOMAIN);
    aad.extend_from_slice(manifest_digest.as_bytes());
    aad.extend_from_slice(&chunk_digest_sha3);
    if let Some(name) = manifest_filename {
        let name_bytes = name.as_bytes();
        let name_len = u32::try_from(name_bytes.len()).expect("manifest filename length fits u32");
        aad.extend_from_slice(&name_len.to_be_bytes());
        aad.extend_from_slice(name_bytes);
    }
    aad
}
#[allow(clippy::too_many_lines)]
fn build_pack_report(ctx: &PackReportContext<'_>) -> Result<Value> {
    let chunk_digests: Vec<Value> = ctx
        .plan
        .chunks
        .iter()
        .map(|chunk| {
            let mut obj = Map::new();
            obj.insert("offset".into(), Value::from(chunk.offset));
            obj.insert("length".into(), Value::from(chunk.length));
            obj.insert("digest_blake3".into(), Value::from(encode(chunk.digest)));
            Value::Object(obj)
        })
        .collect();
    let chunk_fetch_specs = try_chunk_fetch_specs_to_json(ctx.plan)
        .map_err(|err| eyre!("failed to derive chunk fetch plan: {err}"))?;
    let mut chunking_obj = Map::new();
    chunking_obj.insert(
        "namespace".into(),
        Value::from(ctx.profile.namespace.clone()),
    );
    chunking_obj.insert("name".into(), Value::from(ctx.profile.name.clone()));
    chunking_obj.insert("semver".into(), Value::from(ctx.profile.semver.clone()));
    chunking_obj.insert(
        "handle".into(),
        Value::from(format!(
            "{}.{}@{}",
            ctx.profile.namespace, ctx.profile.name, ctx.profile.semver
        )),
    );
    chunking_obj.insert("profile_id".into(), Value::from(ctx.profile.profile_id.0));
    let alias_values: Vec<Value> = ctx
        .profile
        .aliases
        .iter()
        .cloned()
        .map(Value::from)
        .collect();
    chunking_obj.insert("profile_aliases".into(), Value::Array(alias_values.clone()));
    chunking_obj.insert(
        "min_size".into(),
        Value::from(u64::from(ctx.profile.min_size)),
    );
    chunking_obj.insert(
        "target_size".into(),
        Value::from(u64::from(ctx.profile.target_size)),
    );
    chunking_obj.insert(
        "max_size".into(),
        Value::from(u64::from(ctx.profile.max_size)),
    );
    chunking_obj.insert(
        "break_mask".into(),
        Value::from(format!("0x{:04x}", ctx.profile.break_mask)),
    );
    chunking_obj.insert(
        "multihash_code".into(),
        Value::from(ctx.profile.multihash_code),
    );
    let mut pin_policy_obj = Map::new();
    pin_policy_obj.insert(
        "min_replicas".into(),
        Value::from(u64::from(ctx.manifest.pin_policy.min_replicas)),
    );
    pin_policy_obj.insert(
        "storage_class".into(),
        Value::from(format!("{:?}", ctx.manifest.pin_policy.storage_class)),
    );
    pin_policy_obj.insert(
        "retention_epoch".into(),
        Value::from(ctx.manifest.pin_policy.retention_epoch),
    );
    let alias_claims: Vec<Value> = ctx
        .manifest
        .alias_claims
        .iter()
        .map(|alias| {
            let mut obj = Map::new();
            obj.insert("name".into(), Value::from(alias.name.clone()));
            obj.insert("namespace".into(), Value::from(alias.namespace.clone()));
            obj.insert("proof_hex".into(), Value::from(encode(&alias.proof)));
            Value::Object(obj)
        })
        .collect();
    let metadata_entries: Vec<Value> = ctx
        .manifest
        .metadata
        .iter()
        .map(|entry| {
            let mut obj = Map::new();
            obj.insert("key".into(), Value::from(entry.key.clone()));
            obj.insert("value".into(), Value::from(entry.value.clone()));
            Value::Object(obj)
        })
        .collect();
    let mut manifest_obj = Map::new();
    manifest_obj.insert("version".into(), Value::from(ctx.manifest.version));
    manifest_obj.insert(
        "root_cid_hex".into(),
        Value::from(encode(&ctx.manifest.root_cid)),
    );
    manifest_obj.insert("dag_codec".into(), Value::from(ctx.manifest.dag_codec.0));
    manifest_obj.insert(
        "handle".into(),
        Value::from(format!(
            "{}.{}@{}",
            ctx.profile.namespace, ctx.profile.name, ctx.profile.semver
        )),
    );
    manifest_obj.insert("profile_aliases".into(), Value::Array(alias_values));
    manifest_obj.insert(
        "content_length".into(),
        Value::from(ctx.manifest.content_length),
    );
    manifest_obj.insert(
        "por_root_hex".into(),
        Value::from(encode(ctx.manifest.por_root)),
    );
    manifest_obj.insert(
        "car_digest_hex".into(),
        Value::from(encode(ctx.manifest.car_digest)),
    );
    manifest_obj.insert(
        "car_cid_hex".into(),
        Value::from(encode(&ctx.car_stats.car_cid)),
    );
    manifest_obj.insert("car_size".into(), Value::from(ctx.manifest.car_size));
    manifest_obj.insert("pin_policy".into(), Value::Object(pin_policy_obj));
    manifest_obj.insert(
        "digest_hex".into(),
        Value::from(encode(ctx.manifest_digest.as_bytes())),
    );
    manifest_obj.insert(
        "manifest_hex".into(),
        Value::from(encode(ctx.manifest_bytes)),
    );
    manifest_obj.insert(
        "manifest_len".into(),
        Value::from(ctx.manifest_bytes.len() as u64),
    );
    manifest_obj.insert("alias_claims".into(), Value::Array(alias_claims));
    manifest_obj.insert("metadata".into(), Value::Array(metadata_entries));
    let council_entries: Vec<Value> = ctx
        .manifest
        .governance
        .council_signatures
        .iter()
        .map(|sig| {
            let mut obj = Map::new();
            obj.insert("signer_hex".into(), Value::from(encode(sig.signer)));
            obj.insert("signature_hex".into(), Value::from(encode(&sig.signature)));
            Value::Object(obj)
        })
        .collect();
    manifest_obj.insert("council_signatures".into(), Value::Array(council_entries));
    let mut report_obj = Map::new();
    report_obj.insert("schema".into(), Value::from(TOOLKIT_PACK_REPORT_SCHEMA_V1));
    report_obj.insert("chunking".into(), Value::Object(chunking_obj));
    report_obj.insert("chunk_digests".into(), Value::Array(chunk_digests));
    report_obj.insert("chunk_fetch_specs".into(), chunk_fetch_specs);
    report_obj.insert(
        "payload_digest_hex".into(),
        Value::from(encode(ctx.plan.payload_digest.as_bytes())),
    );
    report_obj.insert("car_size".into(), Value::from(ctx.car_stats.car_size));
    report_obj.insert(
        "car_payload_digest_hex".into(),
        Value::from(encode(ctx.car_stats.car_payload_digest.as_bytes())),
    );
    report_obj.insert(
        "car_archive_digest_hex".into(),
        Value::from(encode(ctx.car_stats.car_archive_digest.as_bytes())),
    );
    report_obj.insert(
        "car_cid_hex".into(),
        Value::from(encode(&ctx.car_stats.car_cid)),
    );
    report_obj.insert("car_root_hex".into(), Value::from(encode(ctx.root_cid)));
    report_obj.insert("dag_codec".into(), Value::from(ctx.car_stats.dag_codec));
    report_obj.insert("manifest".into(), Value::Object(manifest_obj));
    report_obj.insert(
        "manifest_digest_hex".into(),
        Value::from(encode(ctx.manifest_digest.as_bytes())),
    );
    report_obj.insert(
        "manifest_size".into(),
        Value::from(ctx.manifest_bytes.len() as u64),
    );
    report_obj.insert(
        "chunk_count".into(),
        Value::from(ctx.plan.chunks.len() as u64),
    );
    report_obj.insert(
        "por_root_hex".into(),
        Value::from(encode(ctx.por_tree.root())),
    );
    report_obj.insert(
        "por_chunk_count".into(),
        Value::from(ctx.por_tree.chunks().len() as u64),
    );
    Ok(Value::Object(report_obj))
}
#[derive(clap::Args, Debug)]
pub struct GatewayDirectModeEnableArgs {
    /// Path to the JSON output produced by `sorafs gateway direct-mode plan`.
    #[arg(long, value_name = "PATH")]
    pub plan: PathBuf,
}
impl Run for GatewayDirectModeEnableArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = fs::read(&self.plan)
            .wrap_err_with(|| format!("failed to read plan from `{}`", self.plan.display()))?;
        let plan: DirectModePlanOutput =
            norito::json::from_slice(&bytes).wrap_err("failed to parse plan JSON")?;
        validate_direct_mode_enable_plan(&plan)?;
        context.println(render_direct_mode_enable_snippet(&plan))
    }
}
#[derive(clap::Args, Debug)]
pub struct GatewayDirectModeRollbackArgs;
impl Run for GatewayDirectModeRollbackArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        context.println(render_direct_mode_rollback_snippet())
    }
}
#[derive(Debug, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct DirectModePlanOutput {
    provider_id_hex: String,
    chain_id: String,
    manifest_digest_hex: String,
    hosts: DirectModePlanHosts,
    direct_car: DirectModePlanDirectCar,
    capabilities: DirectModePlanCapabilities,
}
impl DirectModePlanOutput {
    fn from_components(
        chain_id: &str,
        provider_id: [u8; 32],
        manifest_digest_hex: String,
        hosts: HostMappingSummary,
        direct_car: DirectCarLocator,
        capabilities: ManifestCapabilitySummary,
    ) -> Self {
        Self {
            provider_id_hex: hex::encode(provider_id),
            chain_id: chain_id.to_owned(),
            manifest_digest_hex,
            hosts: DirectModePlanHosts {
                canonical: hosts.canonical,
                vanity: hosts.vanity,
            },
            direct_car: DirectModePlanDirectCar {
                canonical_url: direct_car.canonical_url,
                vanity_url: direct_car.vanity_url,
            },
            capabilities: DirectModePlanCapabilities::from_summary(capabilities),
        }
    }
}
#[derive(Debug, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct DirectModePlanHosts {
    canonical: String,
    vanity: String,
}
#[derive(Debug, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct DirectModePlanDirectCar {
    canonical_url: String,
    vanity_url: String,
}
#[derive(Debug, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
#[allow(clippy::struct_excessive_bools)]
struct DirectModePlanCapabilities {
    requires_manifest_envelope: bool,
    direct_car_supported: bool,
    supports_torii_gateway: bool,
    supports_quic_noise: bool,
    supports_soranet: bool,
    supports_soranet_hybrid_pq: bool,
    supports_chunk_range_fetch: bool,
    advertised_capabilities: Vec<String>,
    range_capability: Option<DirectModePlanRangeCapability>,
    chunk_profile: Option<DirectModePlanChunkProfile>,
    manifest_metadata: Vec<DirectModePlanMetadataEntry>,
}
impl DirectModePlanCapabilities {
    fn from_summary(summary: ManifestCapabilitySummary) -> Self {
        let ManifestCapabilitySummary {
            chunk_profile,
            metadata_pairs,
            requires_manifest_envelope,
            direct_car_supported,
            supports_torii_gateway,
            supports_quic_noise,
            supports_soranet,
            supports_soranet_hybrid_pq,
            supports_chunk_range_fetch,
            range_capability,
            advertised_capabilities,
        } = summary;
        Self {
            requires_manifest_envelope,
            direct_car_supported,
            supports_torii_gateway,
            supports_quic_noise,
            supports_soranet,
            supports_soranet_hybrid_pq,
            supports_chunk_range_fetch,
            advertised_capabilities: advertised_capabilities
                .into_iter()
                .map(capability_type_label)
                .map(str::to_owned)
                .collect(),
            range_capability: range_capability.map(DirectModePlanRangeCapability::from),
            chunk_profile: chunk_profile.map(DirectModePlanChunkProfile::from),
            manifest_metadata: metadata_pairs
                .into_iter()
                .map(|(key, value)| DirectModePlanMetadataEntry { key, value })
                .collect(),
        }
    }
}
#[derive(Debug, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct DirectModePlanChunkProfile {
    profile_id: u32,
    namespace: String,
    name: String,
    semver: String,
    min_size: u32,
    target_size: u32,
    max_size: u32,
    aliases: Vec<String>,
    multihash_code: u64,
}
impl From<ChunkProfileSummary> for DirectModePlanChunkProfile {
    fn from(summary: ChunkProfileSummary) -> Self {
        Self {
            profile_id: summary.profile_id,
            namespace: summary.namespace,
            name: summary.name,
            semver: summary.semver,
            min_size: summary.min_size,
            target_size: summary.target_size,
            max_size: summary.max_size,
            aliases: summary.aliases,
            multihash_code: summary.multihash_code,
        }
    }
}
#[derive(Debug, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct DirectModePlanMetadataEntry {
    key: String,
    value: String,
}
#[derive(Debug, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct DirectModePlanRangeCapability {
    max_chunk_span: u32,
    min_granularity: u32,
    supports_sparse_offsets: bool,
    requires_alignment: bool,
    supports_merkle_proof: bool,
}
impl From<ProviderCapabilityRangeV1> for DirectModePlanRangeCapability {
    fn from(range: ProviderCapabilityRangeV1) -> Self {
        Self {
            max_chunk_span: range.max_chunk_span,
            min_granularity: range.min_granularity,
            supports_sparse_offsets: range.supports_sparse_offsets,
            requires_alignment: range.requires_alignment,
            supports_merkle_proof: range.supports_merkle_proof,
        }
    }
}
fn capability_type_label(cap: CapabilityType) -> &'static str {
    match cap {
        CapabilityType::ToriiGateway => "torii_gateway",
        CapabilityType::QuicNoise => "quic_noise",
        CapabilityType::SoraNetHybridPq => "soranet_pq",
        CapabilityType::ChunkRangeFetch => "chunk_range_fetch",
        CapabilityType::PotrMlDsa => "potr_mldsa",
        CapabilityType::VendorReserved => "vendor_reserved",
    }
}
fn validate_direct_mode_enable_plan(plan: &DirectModePlanOutput) -> Result<()> {
    let provider_id = parse_hex_array::<32>(&plan.provider_id_hex, "provider_id_hex")?;
    let canonical_provider_id_hex = encode(provider_id);
    if plan.provider_id_hex != canonical_provider_id_hex {
        return Err(eyre!(
            "provider_id_hex must be canonical lowercase hex; expected {canonical_provider_id_hex}"
        ));
    }
    let manifest_digest = parse_hex_array::<32>(&plan.manifest_digest_hex, "manifest_digest_hex")?;
    let canonical_manifest_digest_hex = encode(manifest_digest);
    if plan.manifest_digest_hex != canonical_manifest_digest_hex {
        return Err(eyre!(
            "manifest_digest_hex must be canonical lowercase hex; expected {canonical_manifest_digest_hex}"
        ));
    }
    if plan.chain_id.trim().is_empty() {
        return Err(eyre!("chain_id must not be empty"));
    }
    if !plan.capabilities.requires_manifest_envelope {
        return Err(eyre!(
            "direct-mode enable requires capabilities.requires_manifest_envelope=true; regenerate the manifest with envelope enforcement metadata"
        ));
    }
    if !plan.capabilities.direct_car_supported {
        return Err(eyre!(
            "direct-mode enable requires capabilities.direct_car_supported=true; advertise capability.direct_car=true before emitting config"
        ));
    }
    let host_input = HostMappingInput {
        chain_id: plan.chain_id.as_str(),
        provider_id: &provider_id,
    };
    let expected_hosts = host_input.to_summary();
    if plan.hosts.canonical != expected_hosts.canonical {
        return Err(eyre!(
            "direct-mode plan canonical host mismatch: expected `{}`, got `{}`",
            expected_hosts.canonical,
            plan.hosts.canonical
        ));
    }
    if plan.hosts.vanity != expected_hosts.vanity {
        return Err(eyre!(
            "direct-mode plan vanity host mismatch: expected `{}`, got `{}`",
            expected_hosts.vanity,
            plan.hosts.vanity
        ));
    }
    let expected_direct_car = host_input
        .direct_car_locator("https", &canonical_manifest_digest_hex)
        .wrap_err("failed to derive expected direct-CAR locators")?;
    validate_direct_mode_url(
        &plan.direct_car.canonical_url,
        "direct_car.canonical_url",
        &expected_direct_car.canonical_url,
    )?;
    validate_direct_mode_url(
        &plan.direct_car.vanity_url,
        "direct_car.vanity_url",
        &expected_direct_car.vanity_url,
    )
}
fn validate_direct_mode_url(value: &str, label: &str, expected: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(value)
        .wrap_err_with(|| format!("{label} must be a valid direct-CAR URL"))?;
    if parsed.scheme() != "https" {
        return Err(eyre!("{label} must use https"));
    }
    if parsed.host_str().is_none() {
        return Err(eyre!("{label} must include a host"));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(eyre!("{label} must not include userinfo"));
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(eyre!(
            "{label} must not include query or fragment components"
        ));
    }
    if value != expected {
        return Err(eyre!(
            "{label} mismatch: expected `{expected}`, got `{value}`"
        ));
    }
    Ok(())
}
fn escape_toml_basic_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => {
                write!(&mut escaped, "\\u{:04X}", ch as u32)
                    .expect("writing to a String cannot fail");
            }
            ch => escaped.push(ch),
        }
    }
    escaped
}
fn render_direct_mode_enable_snippet(plan: &DirectModePlanOutput) -> String {
    let provider = escape_toml_basic_string(&plan.provider_id_hex);
    let chain = escape_toml_basic_string(&plan.chain_id);
    let canonical = escape_toml_basic_string(&plan.hosts.canonical);
    let vanity = escape_toml_basic_string(&plan.hosts.vanity);
    let direct_canonical = escape_toml_basic_string(&plan.direct_car.canonical_url);
    let direct_vanity = escape_toml_basic_string(&plan.direct_car.vanity_url);
    let digest = escape_toml_basic_string(&plan.manifest_digest_hex);
    format!(
        r#"# Direct-mode configuration snippet (generated; enforcement remains enabled)
[sorafs.gateway]
require_manifest_envelope = true
enforce_admission = true
enforce_capabilities = true

[sorafs.gateway.direct_mode]
provider_id_hex = "{provider}"
chain_id = "{chain}"
canonical_host = "{canonical}"
vanity_host = "{vanity}"
direct_car_canonical = "{direct_canonical}"
direct_car_vanity = "{direct_vanity}"
manifest_digest_hex = "{digest}"
"#,
    )
}
fn render_direct_mode_rollback_snippet() -> &'static str {
    r"# Direct-mode rollback snippet
[sorafs.gateway]
require_manifest_envelope = true
enforce_admission = true
enforce_capabilities = true

# Remove the `sorafs.gateway.direct_mode` table to disable overrides.
"
}
#[derive(clap::Args, Debug)]
pub struct GatewayTemplateConfigArgs {
    /// Hostname to include in the ACME / gateway sample (repeatable).
    #[arg(long = "host", value_name = "HOSTNAME")]
    pub hosts: Vec<String>,
}
#[derive(clap::Args, Debug)]
pub struct GatewayGenerateHostsArgs {
    /// Provider identifier (hex, 32 bytes).
    #[arg(long = "provider-id", value_name = "HEX")]
    pub provider_id: String,
    /// Chain id (network identifier).
    #[arg(long = "chain-id", default_value = "nexus")]
    pub chain_id: String,
}
impl Run for GatewayTemplateConfigArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let hosts = if self.hosts.is_empty() {
            vec!["gateway.example.com".to_owned()]
        } else {
            self.hosts
        };
        let host_list = hosts
            .iter()
            .map(|h| format!("\"{h}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let template = format!(
            r#"# Paste this snippet into your configuration (e.g. config.toml)
[sorafs.gateway]
require_manifest_envelope = true
enforce_admission = true

[sorafs.gateway.rate_limit]
max_requests = 120
window = {{ secs = 60, nanos = 0 }}
ban = {{ secs = 30, nanos = 0 }}

[sorafs.gateway.acme]
enabled = true
account_email = "ops@example.com"
directory_url = "https://acme-v02.api.letsencrypt.org/directory"
hostnames = [{hosts}]
dns_provider_id = "cloudflare-prod"
renewal_window = {{ secs = 2592000, nanos = 0 }}
retry_backoff = {{ secs = 1800, nanos = 0 }}
retry_jitter = {{ secs = 300, nanos = 0 }}

[sorafs.gateway.acme.challenges]
dns01 = true
tls_alpn_01 = true
"#,
            hosts = host_list,
        );
        context.println(template)
    }
}
impl Run for GatewayGenerateHostsArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let chain_id = self
            .chain_id
            .parse::<ChainId>()
            .wrap_err("--chain-id must be canonical")?;
        let provider = parse_hex_array::<32>(&self.provider_id, "provider_id")?;
        let summary = HostMappingInput {
            chain_id: chain_id.as_str(),
            provider_id: &provider,
        }
        .to_summary();
        let mut map = norito::json::Map::new();
        map.insert(
            "canonical".to_owned(),
            norito::json::Value::from(summary.canonical),
        );
        map.insert(
            "vanity".to_owned(),
            norito::json::Value::from(summary.vanity),
        );
        context.print_data(&norito::json::Value::Object(map))
    }
}
#[derive(clap::Args, Debug)]
pub struct GatewayRoutePlanArgs {
    /// Manifest JSON path for the route being promoted.
    #[arg(long = "manifest-json", value_name = "PATH")]
    pub manifest_json: PathBuf,
    /// Hostname that serves the manifest after promotion.
    #[arg(long = "hostname", value_name = "HOSTNAME")]
    pub hostname: String,
    /// Optional alias binding (`namespace:name`) to embed in the headers.
    #[arg(long = "alias", value_name = "NAMESPACE:NAME")]
    pub alias: Option<String>,
    /// Optional logical label applied to the rendered `Sora-Route-Binding`.
    #[arg(long = "route-label", value_name = "LABEL")]
    pub route_label: Option<String>,
    /// Optional proof-status string for the generated `Sora-Proof-Status`.
    #[arg(long = "proof-status", value_name = "STATUS")]
    pub proof_status: Option<String>,
    /// Optional release tag stored alongside the plan.
    #[arg(long = "release-tag", value_name = "STRING")]
    pub release_tag: Option<String>,
    /// Optional cutover window (RFC3339 interval or freeform note).
    #[arg(long = "cutover-window", value_name = "WINDOW")]
    pub cutover_window: Option<String>,
    /// Path where the JSON plan will be written.
    #[arg(
        long = "out",
        value_name = "PATH",
        default_value = "artifacts/sorafs_gateway/route_plan.json"
    )]
    pub output_path: PathBuf,
    /// Optional path storing the primary header block.
    #[arg(long = "headers-out", value_name = "PATH")]
    pub headers_out: Option<PathBuf>,
    /// Optional rollback manifest path (renders a secondary header block).
    #[arg(long = "rollback-manifest-json", value_name = "PATH")]
    pub rollback_manifest_json: Option<PathBuf>,
    /// Optional path for the rollback header block.
    #[arg(long = "rollback-headers-out", value_name = "PATH")]
    pub rollback_headers_out: Option<PathBuf>,
    /// Optional label applied to the rollback binding.
    #[arg(long = "rollback-route-label", value_name = "LABEL")]
    pub rollback_route_label: Option<String>,
    /// Optional release tag for the rollback binding metadata.
    #[arg(long = "rollback-release-tag", value_name = "STRING")]
    pub rollback_release_tag: Option<String>,
    /// Skip emitting the default Content-Security-Policy header.
    #[arg(long = "no-csp")]
    pub no_csp: bool,
    /// Skip emitting the default Permissions-Policy header.
    #[arg(long = "no-permissions-policy")]
    pub no_permissions_policy: bool,
    /// Skip emitting the default `Strict-Transport-Security` header.
    #[arg(long = "no-hsts")]
    pub no_hsts: bool,
    /// Override the timestamp embedded in the binding (RFC3339, test hook).
    #[arg(long = "now", value_name = "RFC3339", hide = true)]
    pub now_override: Option<String>,
}
fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}
#[allow(clippy::too_many_lines)]
impl Run for GatewayRoutePlanArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let alias = match self.alias {
            Some(alias) => Some(parse_alias_label(&alias)?),
            None => None,
        };
        let now = parse_timestamp(self.now_override.as_deref(), "--now")?
            .unwrap_or_else(OffsetDateTime::now_utc);
        let generated_at = now
            .format(&Rfc3339)
            .map_err(|err| eyre!("failed to format timestamp: {err}"))?;
        let headers_out = self.headers_out.or_else(|| {
            self.output_path
                .parent()
                .map(|parent| parent.join("gateway.route.headers.txt"))
        });
        let rollback_headers_out = self.rollback_headers_out.or_else(|| {
            self.output_path
                .parent()
                .map(|parent| parent.join("gateway.route.rollback.headers.txt"))
        });
        let include_csp = !self.no_csp;
        let include_permissions = !self.no_permissions_policy;
        let include_hsts = !self.no_hsts;
        let primary_context = RouteBindingContext {
            manifest_json: self.manifest_json.clone(),
            alias: alias.clone(),
            hostname: self.hostname.clone(),
            route_label: self.route_label.clone(),
            proof_status: self.proof_status.clone(),
            include_csp,
            include_permissions,
            include_hsts,
            generated_at: now,
        };
        let primary_binding = build_route_binding(&primary_context)?;
        write_optional_output(headers_out.as_ref(), &primary_binding.headers_template)?;
        let rollback_value = if let Some(rollback_manifest) = self.rollback_manifest_json.as_ref() {
            let rollback_context = RouteBindingContext {
                manifest_json: rollback_manifest.clone(),
                alias: alias.clone(),
                hostname: self.hostname.clone(),
                route_label: self.rollback_route_label.clone(),
                proof_status: self.proof_status.clone(),
                include_csp,
                include_permissions,
                include_hsts,
                generated_at: now,
            };
            let binding = build_route_binding(&rollback_context)?;
            write_optional_output(rollback_headers_out.as_ref(), &binding.headers_template)?;
            let mut map = Map::new();
            map.insert(
                "manifest_json".into(),
                Value::from(rollback_manifest.display().to_string()),
            );
            if let Some(tag) = &self.rollback_release_tag {
                map.insert("release_tag".into(), Value::from(tag.clone()));
            }
            map.insert("content_cid".into(), Value::from(binding.content_cid));
            map.insert("route_binding".into(), Value::from(binding.route_binding));
            map.insert(
                "headers_template".into(),
                Value::from(binding.headers_template),
            );
            if let Some(path) = rollback_headers_out.as_ref() {
                map.insert(
                    "headers_path".into(),
                    Value::from(path.display().to_string()),
                );
            }
            Some(Value::Object(map))
        } else {
            None
        };
        if let Some(parent) = self.output_path.parent() {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!(
                    "failed to create parent directory for `{}`",
                    self.output_path.display()
                )
            })?;
        }
        let mut plan = Map::new();
        plan.insert("version".into(), Value::from(1u64));
        plan.insert("generated_at".into(), Value::from(generated_at));
        plan.insert(
            "manifest_json".into(),
            Value::from(self.manifest_json.display().to_string()),
        );
        if let Some(alias) = alias.clone() {
            plan.insert("alias".into(), Value::from(alias));
        }
        plan.insert("hostname".into(), Value::from(self.hostname.clone()));
        if let Some(tag) = &self.release_tag {
            plan.insert("release_tag".into(), Value::from(tag.clone()));
        }
        if let Some(window) = &self.cutover_window {
            plan.insert("cutover_window".into(), Value::from(window.clone()));
        }
        plan.insert(
            "content_cid".into(),
            Value::from(primary_binding.content_cid.clone()),
        );
        plan.insert(
            "route_binding".into(),
            Value::from(primary_binding.route_binding.clone()),
        );
        plan.insert(
            "headers_template".into(),
            Value::from(primary_binding.headers_template.clone()),
        );
        if let Some(path) = headers_out.as_ref() {
            plan.insert(
                "headers_path".into(),
                Value::from(path.display().to_string()),
            );
        }
        plan.insert(
            "headers".into(),
            Value::Object(headers_to_value(&primary_binding.headers)),
        );
        if let Some(rollback) = rollback_value {
            plan.insert("rollback".into(), rollback);
        }
        let mut payload = norito::json::to_vec_pretty(&Value::Object(plan))?;
        payload.push(b'\n');
        fs::write(&self.output_path, &payload).wrap_err_with(|| {
            format!(
                "failed to write route plan `{}`",
                self.output_path.display()
            )
        })?;
        context.println(format_args!("wrote {}", self.output_path.display()))?;
        if let Some(path) = headers_out.as_ref() {
            context.println(format_args!("headers written to {}", path.display()))?;
        }
        if self.rollback_manifest_json.is_some()
            && let Some(path) = rollback_headers_out.as_ref()
        {
            context.println(format_args!(
                "rollback headers written to {}",
                path.display()
            ))?;
        }
        Ok(())
    }
}
#[derive(clap::Args, Debug)]
pub struct GatewayCacheInvalidateArgs {
    /// Cache invalidation API endpoint (HTTP/S).
    #[arg(long = "endpoint", value_name = "URL")]
    pub endpoint: String,
    /// Alias bindings (`namespace:name`) that should be purged (repeatable).
    #[arg(long = "alias", value_name = "NAMESPACE:NAME", required = true)]
    pub aliases: Vec<String>,
    /// Manifest digest (hex, 32 bytes) associated with the release.
    #[arg(long = "manifest-digest", value_name = "HEX")]
    pub manifest_digest_hex: String,
    /// Optional CAR digest (hex, 32 bytes) to attach to the request.
    #[arg(long = "car-digest", value_name = "HEX")]
    pub car_digest_hex: Option<String>,
    /// Optional release tag metadata included in the payload.
    #[arg(long = "release-tag", value_name = "STRING")]
    pub release_tag: Option<String>,
    /// Environment variable that stores the cache purge bearer token.
    #[arg(
        long = "auth-env",
        value_name = "ENV",
        default_value = "CACHE_PURGE_TOKEN"
    )]
    pub auth_env: String,
    /// Optional path where the JSON payload will be written.
    #[arg(long = "output", value_name = "PATH")]
    pub output: Option<PathBuf>,
}
impl Run for GatewayCacheInvalidateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if self.endpoint.trim().is_empty() {
            return Err(eyre!("--endpoint must point to a cache invalidation API"));
        }
        let alias_literals = self
            .aliases
            .iter()
            .map(|alias| parse_alias_label(alias))
            .collect::<Result<Vec<_>>>()?;
        let manifest_digest =
            normalize_hex_digest::<32>(&self.manifest_digest_hex, "--manifest-digest")?;
        let car_digest = if let Some(hex) = &self.car_digest_hex {
            Some(normalize_hex_digest::<32>(hex, "--car-digest")?)
        } else {
            None
        };
        let payload_value = build_cache_invalidation_payload(
            &alias_literals,
            &manifest_digest,
            car_digest.as_deref(),
            self.release_tag.as_deref(),
        );
        let payload_bytes = norito::json::to_vec_pretty(&payload_value)?;
        let payload_str = String::from_utf8(payload_bytes).map_err(|err| eyre!(err.to_string()))?;
        if let Some(path) = &self.output {
            fs::write(path, payload_str.as_bytes())
                .wrap_err_with(|| format!("failed to write payload to `{}`", path.display()))?;
            context.println(format_args!(
                "wrote cache invalidation payload to {}",
                path.display()
            ))?;
        } else {
            context.println(&payload_str)?;
        }
        let compact_bytes = norito::json::to_vec(&payload_value)?;
        let compact_str = String::from_utf8(compact_bytes).map_err(|err| eyre!(err.to_string()))?;
        let curl = render_cache_invalidation_curl(&self.endpoint, &self.auth_env, &compact_str);
        context.println(curl)?;
        Ok(())
    }
}
#[cfg(test)]
mod gateway_tests {
    use super::tests::{TestContext, assert_sorafs_config_snippet_is_schema_valid};
    use super::*;
    #[test]
    fn template_config_uses_host_override() {
        let args = GatewayTemplateConfigArgs {
            hosts: vec![
                "gateway-a.example.com".to_owned(),
                "gateway-b.example.com".to_owned(),
            ],
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("template command runs");
        let rendered = ctx.outputs().join("\n");
        let config = assert_sorafs_config_snippet_is_schema_valid(&rendered);
        assert_eq!(config.gateway.rate_limit.window, Duration::from_secs(60));
        assert_eq!(config.gateway.rate_limit.ban, Some(Duration::from_secs(30)));
        assert_eq_compact! { config.gateway.acme.renewal_window => Duration::from_secs(30 * 24 * 60 * 60) };
        assert_eq_compact! { config.gateway.acme.retry_backoff => Duration::from_secs(30 * 60) };
        assert_eq_compact! { config.gateway.acme.retry_jitter => Duration::from_secs(5 * 60) };
        assert!(rendered.contains("[sorafs.gateway]"));
        assert!(!rendered.contains("[torii.sorafs_gateway]"));
        assert!(rendered.contains("gateway-a.example.com"));
        assert!(rendered.contains("gateway-b.example.com"));
        assert!(!rendered.contains("denylist"));
    }
    #[test]
    fn direct_mode_documentation_fixture_satisfies_config_schema() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/documentation/sorafs_gateway_direct_mode.toml"
        ));
        let config = assert_sorafs_config_snippet_is_schema_valid(fixture);
        assert_eq!(config.gateway.rate_limit.window, Duration::from_secs(60));
        assert_eq_compact! { config.gateway.rate_limit.ban => Some(Duration::from_secs(10 * 60)) };
        assert!(config.gateway.direct_mode.is_some());
    }
    #[test]
    fn generate_hosts_outputs_summary() {
        let args = GatewayGenerateHostsArgs {
            provider_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
            chain_id: "nexus".to_owned(),
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("generate-hosts runs");
        assert_compact! { !ctx.outputs().is_empty(); "expected at least one daemon output entry" };
        let output = &ctx.outputs()[0];
        assert!(output.contains("canonical"));
        assert!(output.contains("vanity"));
        assert!(output.contains("aaaaaaaa.nexus.sorafs"));
        assert!(output.contains("aaaa.nexus.direct.sorafs"));
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum PinCommand {
    /// List manifests registered in the pin registry.
    List(PinListArgs),
    /// Fetch a single manifest, aliases, and replication orders.
    Show(PinShowArgs),
    /// Register a manifest in the pin registry via Torii.
    Register(PinRegisterArgs),
}
impl_run_for_subcommand!(PinCommand => List, Show, Register);
#[derive(clap::Args, Debug)]
pub struct PinListArgs {
    /// Optional closed lifecycle filter.
    #[arg(long, value_enum)]
    pub status: Option<PinStatusSelector>,
    /// Maximum number of bounded summaries to return (1 through 256).
    #[arg(long)]
    pub limit: Option<u32>,
    /// Maximum canonical encoded page bytes (1024 through 262144).
    #[arg(long)]
    pub max_bytes: Option<u32>,
    /// Exact non-zero lowercase 32-byte exclusive manifest-digest cursor.
    #[arg(long, value_name = "HEX")]
    pub after_digest_hex: Option<String>,
    /// Non-zero finalized block height anchoring this page.
    #[arg(long, requires = "expected_finalized_block_hash_hex")]
    pub expected_finalized_height: Option<u64>,
    /// Canonical lowercase finalized block hash anchoring this page.
    #[arg(long, value_name = "HEX", requires = "expected_finalized_height")]
    pub expected_finalized_block_hash_hex: Option<String>,
}
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum PinStatusSelector {
    /// Manifests awaiting governance approval.
    Pending,
    /// Approved manifests charged for replication.
    Approved,
    /// Retired manifests retained as lifecycle evidence.
    Retired,
}
impl From<PinStatusSelector> for PinStatusKindV1 {
    fn from(value: PinStatusSelector) -> Self {
        match value {
            PinStatusSelector::Pending => Self::Pending,
            PinStatusSelector::Approved => Self::Approved,
            PinStatusSelector::Retired => Self::Retired,
        }
    }
}
impl Run for PinListArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        self.run_with(context, |client, filter| {
            client.get_sorafs_pin_registry(filter)
        })
    }
}
impl PinListArgs {
    fn run_with<C, F>(&self, context: &mut C, fetch: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SorafsPinListFilter<'_>) -> Result<Response<Vec<u8>>>,
    {
        let after_digest_hex = self
            .after_digest_hex
            .as_deref()
            .map(|digest| required_nonzero_lower_hex32(digest, "--after-digest-hex"))
            .transpose()?;
        let client = context.client_from_config();
        let filter = SorafsPinListFilter {
            finalized: SorafsPinFinalizedAnchor {
                expected_finalized_height: self.expected_finalized_height,
                expected_finalized_block_hash_hex: self
                    .expected_finalized_block_hash_hex
                    .as_deref(),
            },
            status: self.status.map(Into::into),
            limit: self.limit,
            max_bytes: self.max_bytes,
            after_digest_hex: after_digest_hex.as_deref(),
        };
        let response = fetch(&client, &filter)?;
        render_json_response(context, response)
    }
}
#[derive(clap::Args, Debug)]
pub struct PinShowArgs {
    /// Exact non-zero lowercase 32-byte manifest digest.
    #[arg(long, value_name = "HEX")]
    pub digest: String,
}
impl Run for PinShowArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        self.run_with(context, |client, digest| {
            client.get_sorafs_pin_manifest(digest)
        })
    }
}
impl PinShowArgs {
    fn run_with<C, F>(&self, context: &mut C, fetch: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &str) -> Result<Response<Vec<u8>>>,
    {
        let digest = required_nonzero_lower_hex32(&self.digest, "--digest")?;
        let client = context.client_from_config();
        let response = fetch(&client, &digest)?;
        let status = response.status();
        let body = response.into_body();
        match status {
            StatusCode::OK => render_json_body(context, &body),
            StatusCode::NOT_FOUND => context.println(format_args!("manifest `{digest}` not found")),
            status => Err(make_http_error(status, &body)),
        }
    }
}
#[derive(clap::Args, Debug)]
pub struct PinRegisterArgs {
    /// Path to the Norito-encoded manifest (`.to`) file.
    #[arg(long, value_name = "PATH")]
    pub manifest: PathBuf,
    /// Optional alias namespace to bind alongside the manifest.
    #[arg(long)]
    pub alias_namespace: Option<String>,
    /// Optional alias name to bind alongside the manifest.
    #[arg(long)]
    pub alias_name: Option<String>,
    /// Optional path to the alias proof payload (binary).
    #[arg(long, value_name = "PATH")]
    pub alias_proof: Option<PathBuf>,
    /// Optional predecessor manifest digest (hex).
    #[arg(long, value_name = "HEX")]
    pub successor_of: Option<String>,
}
impl Run for PinRegisterArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let manifest_bytes = fs::read(&self.manifest).wrap_err_with(|| {
            format!("failed to read manifest from `{}`", self.manifest.display())
        })?;
        sorafs_manifest::decode_manifest_v1_canonical(&manifest_bytes)
            .wrap_err("failed to decode exact canonical manifest payload")?;
        let alias_inputs = self.load_alias_inputs()?;
        let successor = self
            .successor_of
            .as_ref()
            .map(|hex| parse_hex_array::<32>(hex, "successor_of"))
            .transpose()?;
        let client = context.client_from_config();
        let alias_ref = alias_inputs.as_ref().map(|alias| SorafsPinAlias {
            namespace: alias.namespace.as_str(),
            name: alias.name.as_str(),
            proof: alias.proof.as_slice(),
        });
        let response = client
            .post_sorafs_pin_register(SorafsPinRegisterArgs {
                manifest_payload: &manifest_bytes,
                alias: alias_ref,
                successor_of: successor,
            })
            .wrap_err("failed to register pin manifest")?;
        context.print_data(&response)
    }
}
struct AliasInputs {
    namespace: String,
    name: String,
    proof: Vec<u8>,
}
impl PinRegisterArgs {
    fn load_alias_inputs(&self) -> Result<Option<AliasInputs>> {
        match (&self.alias_namespace, &self.alias_name, &self.alias_proof) {
            (None, None, None) => Ok(None),
            (Some(namespace), Some(name), Some(path)) => {
                let bytes = fs::read(path).wrap_err_with(|| {
                    format!("failed to read alias proof from `{}`", path.display())
                })?;
                if bytes.is_empty() {
                    return Err(eyre!("alias proof file `{}` is empty", path.display()));
                }
                Ok(Some(AliasInputs {
                    namespace: namespace.clone(),
                    name: name.clone(),
                    proof: bytes,
                }))
            }
            _ => Err(eyre!(
                "alias namespace, name, and proof must be provided together"
            )),
        }
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum AliasCommand {
    /// List alias bindings exposed via Torii.
    List(AliasListArgs),
}
impl_run_for_subcommand!(AliasCommand => List);
#[derive(clap::Args, Debug)]
pub struct AliasListArgs {
    /// Maximum number of aliases to return.
    #[arg(long)]
    pub limit: Option<u32>,
    /// Offset for pagination.
    #[arg(long)]
    pub offset: Option<u32>,
    /// Restrict aliases to an exact canonical lowercase namespace.
    #[arg(long)]
    pub namespace: Option<String>,
    /// Restrict aliases to an exact non-zero lowercase 32-byte manifest digest.
    #[arg(long, value_name = "HEX")]
    pub manifest_digest: Option<String>,
}
impl_run_with_client_methods!(AliasListArgs, Client::get_sorafs_aliases);
impl AliasListArgs {
    fn run_with<C, F>(&self, context: &mut C, fetch: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SorafsAliasListFilter<'_>) -> Result<Response<Vec<u8>>>,
    {
        let manifest_digest = self
            .manifest_digest
            .as_deref()
            .map(|digest| required_nonzero_lower_hex32(digest, "--manifest-digest"))
            .transpose()?;
        let client = context.client_from_config();
        let filter = SorafsAliasListFilter {
            limit: self.limit,
            offset: self.offset,
            namespace: self.namespace.as_deref(),
            manifest_digest: manifest_digest.as_deref(),
        };
        let response = fetch(&client, &filter)?;
        render_json_response(context, response)
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum ReplicationCommand {
    /// List replication orders.
    List(ReplicationListArgs),
}
impl_run_for_subcommand!(ReplicationCommand => List);
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum ReplicationStatusSelector {
    /// Orders still awaiting their required provider completions.
    Pending,
    /// Orders whose required provider completions are committed.
    Completed,
    /// Orders cancelled when their target pin was retired.
    Cancelled,
    /// Incomplete orders expired after their inclusive deadline.
    Expired,
}
impl From<ReplicationStatusSelector> for SorafsReplicationStatus {
    fn from(value: ReplicationStatusSelector) -> Self {
        match value {
            ReplicationStatusSelector::Pending => Self::Pending,
            ReplicationStatusSelector::Completed => Self::Completed,
            ReplicationStatusSelector::Cancelled => Self::Cancelled,
            ReplicationStatusSelector::Expired => Self::Expired,
        }
    }
}
#[derive(clap::Args, Debug)]
pub struct ReplicationListArgs {
    /// Maximum number of orders to return.
    #[arg(long)]
    pub limit: Option<u32>,
    /// Offset for pagination.
    #[arg(long)]
    pub offset: Option<u32>,
    /// Optional exact lifecycle filter.
    #[arg(long, value_enum)]
    pub status: Option<ReplicationStatusSelector>,
    /// Restrict orders to an exact non-zero lowercase 32-byte manifest digest.
    #[arg(long, value_name = "HEX")]
    pub manifest_digest: Option<String>,
}
impl Run for ReplicationListArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        self.run_with(context, |client, filter| {
            client.get_sorafs_replication_orders(filter)
        })
    }
}
impl ReplicationListArgs {
    fn run_with<C, F>(&self, context: &mut C, fetch: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SorafsReplicationListFilter<'_>) -> Result<Response<Vec<u8>>>,
    {
        let manifest_digest = self
            .manifest_digest
            .as_deref()
            .map(|digest| required_nonzero_lower_hex32(digest, "--manifest-digest"))
            .transpose()?;
        let client = context.client_from_config();
        let filter = SorafsReplicationListFilter {
            limit: self.limit,
            offset: self.offset,
            status: self.status.map(Into::into),
            manifest_digest: manifest_digest.as_deref(),
        };
        let response = fetch(&client, &filter)?;
        render_json_response(context, response)
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum StorageCommand {
    /// Issue and inspect stream tokens for chunk-range gateways.
    #[command(subcommand)]
    Token(StorageTokenCommand),
}
impl_run_for_subcommand!(StorageCommand => Token);
#[derive(clap::Subcommand, Debug)]
pub enum StorageTokenCommand {
    /// Issue a stream token for a manifest/provider pair.
    Issue(StorageTokenIssueArgs),
}
impl_run_for_subcommand!(StorageTokenCommand => Issue);
#[derive(clap::Args, Debug)]
pub struct StorageTokenIssueArgs {
    /// Hex-encoded manifest identifier stored on the gateway.
    #[arg(long, value_name = "HEX")]
    pub manifest_id: String,
    /// Hex-encoded provider identifier authorised to serve the manifest.
    #[arg(long, value_name = "HEX")]
    pub provider_id: String,
    /// Logical client identifier used for quota accounting.
    #[arg(long, value_name = "STRING")]
    pub client_id: String,
    /// Optional nonce to send in the request headers (auto-generated when omitted).
    #[arg(long, value_name = "STRING")]
    pub nonce: Option<String>,
    /// Override the default TTL expressed in seconds.
    #[arg(long, value_name = "SECONDS")]
    pub ttl_secs: Option<u64>,
    /// Override the maximum concurrent stream count.
    #[arg(long, value_name = "COUNT")]
    pub max_streams: Option<u16>,
    /// Override the sustained throughput limit in bytes per second.
    #[arg(long, value_name = "BYTES")]
    pub rate_limit_bytes: Option<u64>,
    /// Override the allowed number of refresh requests per minute.
    #[arg(long, value_name = "COUNT")]
    pub requests_per_minute: Option<u32>,
}
impl Run for StorageTokenIssueArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        self.run_with(
            context,
            |client, manifest, provider, client_id, nonce, overrides| {
                client.post_sorafs_storage_token(manifest, provider, client_id, nonce, overrides)
            },
        )
    }
}
impl StorageTokenIssueArgs {
    fn run_with<C, F>(&self, context: &mut C, issue: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(
            &Client,
            &str,
            &str,
            &str,
            &str,
            &SorafsTokenOverrides,
        ) -> Result<Response<Vec<u8>>>,
    {
        let nonce = match self.nonce.clone() {
            Some(nonce) => nonce,
            None => generate_nonce_hex(12)?,
        };
        let overrides = SorafsTokenOverrides {
            ttl_secs: self.ttl_secs,
            max_streams: self.max_streams,
            rate_limit_bytes: self.rate_limit_bytes,
            requests_per_minute: self.requests_per_minute,
        };
        let client = context.client_from_config();
        let response = issue(
            &client,
            &self.manifest_id,
            &self.provider_id,
            &self.client_id,
            &nonce,
            &overrides,
        )?;
        if self.nonce.is_none() && response.status().is_success() {
            context.println(format!("nonce: {nonce}"))?;
        }
        render_json_response(context, response)
    }
}
fn parse_timestamp(raw: Option<&str>, field: &str) -> Result<Option<OffsetDateTime>> {
    let Some(value) = raw else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(eyre!("`{field}` must not be empty when provided"));
    }
    let parsed =
        OffsetDateTime::parse(trimmed, &Rfc3339).wrap_err_with(|| format!("invalid `{field}`"))?;
    Ok(Some(parsed))
}
fn ensure_optional_non_empty(field: Option<&str>, name: &str) -> Result<()> {
    if let Some(value) = field
        && value.trim().is_empty()
    {
        return Err(eyre!("`{name}` must not be empty when provided"));
    }
    Ok(())
}
#[derive(Debug)]
struct ModerationOperatorCanaryHttpResponse {
    status: StatusCode,
    content_type: Option<String>,
    body: Vec<u8>,
}
#[derive(Clone)]
struct ModerationOperatorCanaryRouteSpec {
    name: &'static str,
    path: String,
    expected_schema: Option<&'static str>,
    expect_html_marker: Option<&'static str>,
    include_limit: bool,
}
fn moderation_operator_canary_http_get(
    client: &BlockingHttpClient,
    url: &str,
) -> Result<ModerationOperatorCanaryHttpResponse> {
    let response = client.get(url).send().wrap_err_with(|| {
        format!("failed to GET SoraFS moderation operator canary route `{url}`")
    })?;
    let status = StatusCode::from_u16(response.status().as_u16()).map_err(|err| {
        eyre!("SoraFS moderation operator canary route `{url}` returned unsupported status: {err}")
    })?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let body = response
        .bytes()
        .wrap_err_with(|| {
            format!("failed to read SoraFS moderation operator canary route `{url}` body")
        })?
        .to_vec();
    Ok(ModerationOperatorCanaryHttpResponse {
        status,
        content_type,
        body,
    })
}
#[derive(Debug)]
struct TransparencyExplorerCanaryHttpResponse {
    status: StatusCode,
    content_type: Option<String>,
    body: Vec<u8>,
}
#[derive(Clone)]
struct TransparencyExplorerCanaryRouteSpec {
    name: &'static str,
    path: &'static str,
    expected_schema: Option<&'static str>,
    expect_html_marker: Option<&'static str>,
    include_limit: bool,
}
fn transparency_explorer_canary_http_get(
    client: &BlockingHttpClient,
    url: &str,
) -> Result<TransparencyExplorerCanaryHttpResponse> {
    let response = client.get(url).send().wrap_err_with(|| {
        format!("failed to GET SoraFS transparency explorer canary route `{url}`")
    })?;
    let status = StatusCode::from_u16(response.status().as_u16()).map_err(|err| {
        eyre!(
            "SoraFS transparency explorer canary route `{url}` returned unsupported status: {err}"
        )
    })?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let body = response
        .bytes()
        .wrap_err_with(|| {
            format!("failed to read SoraFS transparency explorer canary route `{url}` body")
        })?
        .to_vec();
    Ok(TransparencyExplorerCanaryHttpResponse {
        status,
        content_type,
        body,
    })
}
fn transparency_publication_canary_http_get(
    client: &BlockingHttpClient,
    url: &str,
) -> Result<TransparencyExplorerCanaryHttpResponse> {
    let response = client.get(url).send().wrap_err_with(|| {
        format!("failed to GET SoraFS transparency publication canary route `{url}`")
    })?;
    let status = StatusCode::from_u16(response.status().as_u16()).map_err(|err| {
        eyre!(
            "SoraFS transparency publication canary route `{url}` returned unsupported status: {err}"
        )
    })?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let body = response
        .bytes()
        .wrap_err_with(|| {
            format!("failed to read SoraFS transparency publication canary route `{url}` body")
        })?
        .to_vec();
    Ok(TransparencyExplorerCanaryHttpResponse {
        status,
        content_type,
        body,
    })
}
fn transparency_explorer_canary_evidence_json<F>(
    torii_url: &str,
    limit: Option<u32>,
    fetch: &mut F,
) -> Result<Value>
where
    F: FnMut(&str) -> Result<TransparencyExplorerCanaryHttpResponse>,
{
    let route_specs = transparency_explorer_canary_route_specs();
    let mut routes = Vec::with_capacity(route_specs.len());
    for spec in &route_specs {
        routes.push(transparency_explorer_canary_probe_route(
            torii_url, spec, limit, fetch,
        )?);
    }
    let mut evidence = Map::new();
    evidence.insert(
        "schema".into(),
        Value::from("sorafs.transparency.explorer_canary.v1"),
    );
    evidence.insert("status".into(), Value::from("passed"));
    evidence.insert("source".into(), Value::from("iroha_cli"));
    evidence.insert("torii_url".into(), Value::from(torii_url.to_string()));
    evidence.insert("limit".into(), limit.map_or(Value::Null, Value::from));
    evidence.insert(
        "generated_at_unix".into(),
        Value::from(current_unix_timestamp()),
    );
    evidence.insert("route_count".into(), Value::from(routes.len() as u64));
    evidence.insert("payload_bytes_included".into(), Value::Bool(false));
    evidence.insert("private_digest_keys_included".into(), Value::Bool(false));
    evidence.insert("routes".into(), Value::Array(routes));
    Ok(Value::Object(evidence))
}
fn transparency_publication_canary_evidence_json<F>(
    torii_url: &str,
    cycle_ids: &[String],
    limit: Option<u32>,
    fetch: &mut F,
) -> Result<Value>
where
    F: FnMut(&str) -> Result<TransparencyExplorerCanaryHttpResponse>,
{
    let mut routes = Vec::with_capacity(1 + cycle_ids.len());
    routes.push(transparency_publication_canary_probe_route(
        torii_url,
        "cycles_list",
        None,
        limit,
        fetch,
    )?);
    for cycle_id in cycle_ids {
        routes.push(transparency_publication_canary_probe_route(
            torii_url,
            "cycle_publication",
            Some(cycle_id),
            limit,
            fetch,
        )?);
    }
    let passed_count = routes
        .iter()
        .filter(|route| {
            route
                .get("passed")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .count();
    let mut evidence = Map::new();
    evidence.insert(
        "schema".into(),
        Value::from("sorafs.transparency.publication_canary.v1"),
    );
    evidence.insert(
        "status".into(),
        Value::from(if passed_count == routes.len() {
            "passed"
        } else {
            "failed"
        }),
    );
    evidence.insert("source".into(), Value::from("iroha_cli"));
    evidence.insert("torii_url".into(), Value::from(torii_url.to_string()));
    evidence.insert("limit".into(), limit.map_or(Value::Null, Value::from));
    evidence.insert(
        "generated_at_unix".into(),
        Value::from(current_unix_timestamp()),
    );
    evidence.insert("route_count".into(), Value::from(routes.len() as u64));
    evidence.insert(
        "passed_route_count".into(),
        Value::from(passed_count as u64),
    );
    evidence.insert(
        "cycle_detail_probe_count".into(),
        Value::from(cycle_ids.len() as u64),
    );
    evidence.insert("publisher_identity_required".into(), Value::Bool(true));
    evidence.insert("payload_bytes_included".into(), Value::Bool(false));
    evidence.insert("publication_bodies_included".into(), Value::Bool(false));
    evidence.insert("private_payloads_included".into(), Value::Bool(false));
    evidence.insert("routes".into(), Value::Array(routes));
    Ok(Value::Object(evidence))
}
fn transparency_publication_canary_probe_route<F>(
    torii_url: &str,
    route_name: &'static str,
    cycle_id: Option<&str>,
    limit: Option<u32>,
    fetch: &mut F,
) -> Result<Value>
where
    F: FnMut(&str) -> Result<TransparencyExplorerCanaryHttpResponse>,
{
    let url = transparency_publication_canary_route_url(torii_url, cycle_id, limit)?;
    let response = fetch(&url)?;
    let status_success = response.status == StatusCode::OK;
    let body_blake3_hex = blake3::hash(&response.body).to_hex().to_string();
    let body_bytes = u64::try_from(response.body.len()).unwrap_or(u64::MAX);
    let mut route = Map::new();
    route.insert("name".into(), Value::from(route_name));
    route.insert("method".into(), Value::from("GET"));
    route.insert(
        "path".into(),
        Value::from(match cycle_id {
            Some(_) => "/v1/sorafs/transparency/cycles/{cycle_id}",
            None => "/v1/sorafs/transparency/cycles",
        }),
    );
    route.insert("url".into(), Value::from(url));
    if let Some(cycle_id) = cycle_id {
        route.insert("cycle_id_hex".into(), Value::from(cycle_id.to_string()));
    }
    route.insert(
        "status_code".into(),
        Value::from(u64::from(response.status.as_u16())),
    );
    route.insert("http_success".into(), Value::Bool(status_success));
    route.insert(
        "content_type".into(),
        response.content_type.map_or(Value::Null, Value::from),
    );
    route.insert("body_blake3_hex".into(), Value::from(body_blake3_hex));
    route.insert("body_bytes".into(), Value::from(body_bytes));
    route.insert("payload_bytes_included".into(), Value::Bool(false));
    route.insert("publication_body_included".into(), Value::Bool(false));
    route.insert("private_payloads_included".into(), Value::Bool(false));
    if !status_success {
        route.insert("passed".into(), Value::Bool(false));
        return Ok(Value::Object(route));
    }
    let value: Value = norito::json::from_slice(&response.body).wrap_err_with(|| {
        format!("failed to decode SoraFS transparency publication canary `{route_name}` JSON")
    })?;
    transparency_explorer_canary_ensure_payload_free(&value)?;
    let expected_schema = if cycle_id.is_some() {
        "sorafs.transparency.cycle_publication.v1"
    } else {
        "sorafs.transparency.cycles.v1"
    };
    let actual_schema = value
        .get("schema")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let schema_ok = actual_schema == expected_schema;
    let anchor_metadata_present =
        transparency_publication_canary_anchor_metadata_present(&value, cycle_id.is_some());
    let publisher_identity_present =
        transparency_publication_canary_publisher_identity_present(&value);
    let verification_valid = if cycle_id.is_some() {
        value
            .get("verification")
            .and_then(|verification| verification.get("valid"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && value
                .get("verification")
                .and_then(|verification| verification.get("all_proofs_verified"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
    } else {
        true
    };
    let passed =
        schema_ok && anchor_metadata_present && verification_valid && publisher_identity_present;
    route.insert("passed".into(), Value::Bool(passed));
    route.insert("schema".into(), Value::from(actual_schema.to_string()));
    route.insert("schema_ok".into(), Value::Bool(schema_ok));
    route.insert(
        "anchor_metadata_present".into(),
        Value::Bool(anchor_metadata_present),
    );
    route.insert(
        "publisher_identity_present".into(),
        Value::Bool(publisher_identity_present),
    );
    route.insert("verification_valid".into(), Value::Bool(verification_valid));
    if cycle_id.is_none() {
        route.insert(
            "published_cycle_count".into(),
            value
                .get("published_cycle_count")
                .cloned()
                .unwrap_or(Value::Null),
        );
        route.insert(
            "returned_cycle_count".into(),
            value
                .get("returned_cycle_count")
                .cloned()
                .unwrap_or(Value::Null),
        );
        route.insert(
            "truncated".into(),
            value.get("truncated").cloned().unwrap_or(Value::Null),
        );
    } else {
        route.insert(
            "proof_count".into(),
            value.get("proof_count").cloned().unwrap_or(Value::Null),
        );
        route.insert(
            "returned_proof_count".into(),
            value
                .get("returned_proof_count")
                .cloned()
                .unwrap_or(Value::Null),
        );
        route.insert(
            "truncated_proofs".into(),
            value
                .get("truncated_proofs")
                .cloned()
                .unwrap_or(Value::Null),
        );
    }
    Ok(Value::Object(route))
}
fn transparency_explorer_canary_route_specs() -> Vec<TransparencyExplorerCanaryRouteSpec> {
    vec![
        TransparencyExplorerCanaryRouteSpec {
            name: "explorer_snapshot",
            path: "/v1/sorafs/transparency/explorer",
            expected_schema: Some("sorafs.transparency.explorer_snapshot.v1"),
            expect_html_marker: None,
            include_limit: true,
        },
        TransparencyExplorerCanaryRouteSpec {
            name: "browser_ui",
            path: "/v1/sorafs/transparency/explorer/ui",
            expected_schema: None,
            expect_html_marker: Some("SoraFS Transparency Explorer"),
            include_limit: false,
        },
        TransparencyExplorerCanaryRouteSpec {
            name: "proof_token_issuance_index",
            path: "/v1/sorafs/transparency/tokens",
            expected_schema: Some("sorafs.transparency.proof_token_issuances.v1"),
            expect_html_marker: None,
            include_limit: true,
        },
    ]
}
fn transparency_explorer_canary_probe_route<F>(
    torii_url: &str,
    spec: &TransparencyExplorerCanaryRouteSpec,
    limit: Option<u32>,
    fetch: &mut F,
) -> Result<Value>
where
    F: FnMut(&str) -> Result<TransparencyExplorerCanaryHttpResponse>,
{
    let url = transparency_explorer_canary_route_url(torii_url, spec, limit)?;
    let response = fetch(&url)?;
    if response.status != StatusCode::OK {
        return Err(eyre!(
            "SoraFS transparency explorer canary route `{}` returned status {}",
            spec.name,
            response.status
        ));
    }
    let body_blake3_hex = blake3::hash(&response.body).to_hex().to_string();
    let mut schema = None;
    if let Some(expected_schema) = spec.expected_schema {
        let value: Value = norito::json::from_slice(&response.body).wrap_err_with(|| {
            format!(
                "failed to decode SoraFS transparency explorer canary `{}` JSON response",
                spec.name
            )
        })?;
        transparency_explorer_canary_ensure_payload_free(&value)?;
        let fields = value_object(&value, "transparency explorer canary JSON response")?;
        let actual_schema = required_string_field(
            fields,
            "schema",
            "transparency explorer canary JSON response",
        )?;
        if actual_schema != expected_schema {
            return Err(eyre!(
                "SoraFS transparency explorer canary route `{}` returned schema `{actual_schema}` (expected `{expected_schema}`)",
                spec.name
            ));
        }
        schema = Some(actual_schema.to_string());
    }
    if let Some(marker) = spec.expect_html_marker {
        let body = std::str::from_utf8(&response.body).wrap_err_with(|| {
            format!(
                "SoraFS transparency explorer canary `{}` response is not UTF-8",
                spec.name
            )
        })?;
        if !body.contains(marker) {
            return Err(eyre!(
                "SoraFS transparency explorer canary route `{}` response is missing `{marker}`",
                spec.name
            ));
        }
        transparency_explorer_canary_ensure_html_payload_free(body, spec.name)?;
    }
    let mut route = Map::new();
    route.insert("name".into(), Value::from(spec.name));
    route.insert("method".into(), Value::from("GET"));
    route.insert("path".into(), Value::from(spec.path));
    route.insert("url".into(), Value::from(url));
    route.insert(
        "status_code".into(),
        Value::from(u64::from(response.status.as_u16())),
    );
    route.insert(
        "content_type".into(),
        response.content_type.map_or(Value::Null, Value::from),
    );
    route.insert("schema".into(), schema.map_or(Value::Null, Value::from));
    route.insert("body_blake3_hex".into(), Value::from(body_blake3_hex));
    route.insert(
        "body_bytes".into(),
        Value::from(u64::try_from(response.body.len()).unwrap_or(u64::MAX)),
    );
    route.insert("payload_bytes_included".into(), Value::Bool(false));
    route.insert("private_digest_keys_included".into(), Value::Bool(false));
    Ok(Value::Object(route))
}
fn transparency_explorer_canary_route_url(
    torii_url: &str,
    spec: &TransparencyExplorerCanaryRouteSpec,
    limit: Option<u32>,
) -> Result<String> {
    let base = format!("{}/", torii_url.trim_end_matches('/'));
    let mut url = reqwest::Url::parse(&base)
        .wrap_err_with(|| format!("failed to parse --torii-url `{torii_url}`"))?
        .join(spec.path.trim_start_matches('/'))
        .wrap_err_with(|| format!("failed to join transparency explorer route `{}`", spec.path))?;
    if spec.include_limit
        && let Some(limit) = limit
    {
        url.query_pairs_mut()
            .append_pair("limit", &limit.to_string());
    }
    Ok(url.to_string())
}
fn transparency_publication_canary_route_url(
    torii_url: &str,
    cycle_id: Option<&str>,
    limit: Option<u32>,
) -> Result<String> {
    let base = format!("{}/", torii_url.trim_end_matches('/'));
    let mut url = reqwest::Url::parse(&base)
        .wrap_err_with(|| format!("failed to parse --torii-url `{torii_url}`"))?
        .join("v1/sorafs/transparency/cycles")
        .wrap_err("failed to join transparency publication cycles route")?;
    if let Some(cycle_id) = cycle_id {
        url.path_segments_mut()
            .map_err(|_| eyre!("failed to append transparency cycle id to --torii-url"))?
            .push(cycle_id);
    }
    if let Some(limit) = limit {
        url.query_pairs_mut()
            .append_pair("limit", &limit.to_string());
    }
    Ok(url.to_string())
}
fn transparency_publication_canary_anchor_metadata_present(
    value: &Value,
    cycle_detail: bool,
) -> bool {
    fn has_string_field(fields: &Map, key: &str) -> bool {
        fields
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty() && !matches!(value, "0" | "0x0"))
    }
    if cycle_detail {
        let Some(fields) = value.as_object() else {
            return false;
        };
        let Some(verification) = fields.get("verification").and_then(Value::as_object) else {
            return false;
        };
        has_string_field(fields, "encoded_blake3")
            && has_string_field(verification, "block_hash_hex")
            && has_string_field(verification, "publication_hash_hex")
            && has_string_field(verification, "entry_root_hex")
    } else {
        let Some(first_cycle) = value
            .get("cycles")
            .and_then(Value::as_array)
            .and_then(|cycles| cycles.first())
            .and_then(Value::as_object)
        else {
            return false;
        };
        has_string_field(first_cycle, "block_hash_hex")
            && has_string_field(first_cycle, "publication_hash_hex")
            && has_string_field(first_cycle, "entry_root_hex")
            && has_string_field(first_cycle, "encoded_blake3")
    }
}
fn transparency_publication_canary_publisher_identity_present(value: &Value) -> bool {
    fn visit(value: &Value) -> bool {
        match value {
            Value::Object(fields) => {
                for key in [
                    "publisher_peer_id",
                    "publisher_peer_id_hex",
                    "publisher_public_key_hex",
                ] {
                    if fields
                        .get(key)
                        .and_then(Value::as_str)
                        .is_some_and(|value| {
                            !value.trim().is_empty() && !matches!(value, "0" | "0x0")
                        })
                    {
                        return true;
                    }
                }
                fields.values().any(visit)
            }
            Value::Array(values) => values.iter().any(visit),
            _ => false,
        }
    }
    visit(value)
}
fn transparency_explorer_canary_ensure_payload_free(value: &Value) -> Result<()> {
    fn visit(path: &str, value: &Value) -> Result<()> {
        match value {
            Value::Object(fields) => {
                for (key, child) in fields {
                    let child_path = if path.is_empty() {
                        key.to_string()
                    } else {
                        format!("{path}.{key}")
                    };
                    if matches!(
                        key.as_str(),
                        "payload_b64"
                            | "payload_bytes"
                            | "payload_body"
                            | "blinded_digest_key"
                            | "digest_key"
                            | "proof_token_digest_key"
                            | "private_digest_key"
                    ) {
                        return Err(eyre!(
                            "transparency explorer canary response included private payload or digest-key material at `{child_path}`"
                        ));
                    }
                    if matches!(
                        key.as_str(),
                        "payload_bytes_included"
                            | "private_payloads_included"
                            | "private_payload_included"
                            | "private_digest_keys_included"
                    ) && child.as_bool() == Some(true)
                    {
                        return Err(eyre!(
                            "transparency explorer canary response advertised private payload or digest-key material at `{child_path}`"
                        ));
                    }
                    visit(&child_path, child)?;
                }
            }
            Value::Array(values) => {
                for (index, child) in values.iter().enumerate() {
                    visit(&format!("{path}[{index}]"), child)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    visit("", value)
}
fn transparency_explorer_canary_ensure_html_payload_free(
    body: &str,
    route_name: &str,
) -> Result<()> {
    for marker in [
        "payload_b64",
        "payload_bytes",
        "blinded_digest_key",
        "digest_key",
        "proof_token_digest_key",
        "private_digest_key",
    ] {
        if body.contains(marker) {
            return Err(eyre!(
                "SoraFS transparency explorer canary route `{route_name}` HTML included private marker `{marker}`"
            ));
        }
    }
    Ok(())
}
fn moderation_operator_canary_evidence_json<F>(
    operator_url: &str,
    quarantine_id_hex: &str,
    limit: Option<u32>,
    fetch: &mut F,
) -> Result<Value>
where
    F: FnMut(&str) -> Result<ModerationOperatorCanaryHttpResponse>,
{
    let route_specs = moderation_operator_canary_route_specs(quarantine_id_hex);
    let mut routes = Vec::with_capacity(route_specs.len());
    for spec in &route_specs {
        routes.push(moderation_operator_canary_probe_route(
            operator_url,
            spec,
            limit,
            fetch,
        )?);
    }
    let mut evidence = Map::new();
    evidence.insert(
        "schema".into(),
        Value::from("sorafs.moderation.quarantine.operator_canary.v1"),
    );
    evidence.insert("status".into(), Value::from("passed"));
    evidence.insert("source".into(), Value::from("iroha_cli"));
    evidence.insert("operator_url".into(), Value::from(operator_url.to_string()));
    evidence.insert(
        "quarantine_id_hex".into(),
        Value::from(quarantine_id_hex.to_string()),
    );
    evidence.insert("limit".into(), limit.map_or(Value::Null, Value::from));
    evidence.insert(
        "generated_at_unix".into(),
        Value::from(current_unix_timestamp()),
    );
    evidence.insert("route_count".into(), Value::from(routes.len() as u64));
    evidence.insert("payload_bytes_included".into(), Value::Bool(false));
    evidence.insert("private_payloads_included".into(), Value::Bool(false));
    evidence.insert("routes".into(), Value::Array(routes));
    Ok(Value::Object(evidence))
}
fn moderation_operator_canary_route_specs(
    quarantine_id_hex: &str,
) -> Vec<ModerationOperatorCanaryRouteSpec> {
    let workflow =
        |suffix: &str| format!("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/{suffix}");
    vec![
        ModerationOperatorCanaryRouteSpec {
            name: "healthz",
            path: "/healthz".to_string(),
            expected_schema: Some("sorafs.moderation.quarantine.operator_service.status.v1"),
            expect_html_marker: None,
            include_limit: false,
        },
        ModerationOperatorCanaryRouteSpec {
            name: "status",
            path: "/v1/sorafs/moderation/operator-panel/status".to_string(),
            expected_schema: Some("sorafs.moderation.quarantine.operator_service.status.v1"),
            expect_html_marker: None,
            include_limit: false,
        },
        ModerationOperatorCanaryRouteSpec {
            name: "browser_ui",
            path: "/v1/sorafs/moderation/operator-panel/ui".to_string(),
            expected_schema: None,
            expect_html_marker: Some("SoraFS Moderation Operator"),
            include_limit: false,
        },
        ModerationOperatorCanaryRouteSpec {
            name: "operator_panel",
            path: workflow("operator-panel"),
            expected_schema: Some("sorafs.moderation.quarantine.operator_panel.v1"),
            expect_html_marker: None,
            include_limit: true,
        },
        ModerationOperatorCanaryRouteSpec {
            name: "bridge_plan",
            path: workflow("bridge-plan"),
            expected_schema: Some("sorafs.moderation.quarantine.bridge_plan.v1"),
            expect_html_marker: None,
            include_limit: true,
        },
        ModerationOperatorCanaryRouteSpec {
            name: "juror_plan",
            path: workflow("juror-plan"),
            expected_schema: Some("sorafs.moderation.quarantine.juror_plan.v1"),
            expect_html_marker: None,
            include_limit: true,
        },
        ModerationOperatorCanaryRouteSpec {
            name: "juror_notifications",
            path: workflow("juror-notifications"),
            expected_schema: Some("sorafs.moderation.quarantine.juror_notifications.v1"),
            expect_html_marker: None,
            include_limit: true,
        },
        ModerationOperatorCanaryRouteSpec {
            name: "commit_reveal_status",
            path: workflow("commit-reveal-status"),
            expected_schema: Some("sorafs.moderation.quarantine.commit_reveal_status.v1"),
            expect_html_marker: None,
            include_limit: true,
        },
    ]
}
fn moderation_operator_canary_probe_route<F>(
    operator_url: &str,
    spec: &ModerationOperatorCanaryRouteSpec,
    limit: Option<u32>,
    fetch: &mut F,
) -> Result<Value>
where
    F: FnMut(&str) -> Result<ModerationOperatorCanaryHttpResponse>,
{
    let url = moderation_operator_canary_route_url(operator_url, spec, limit)?;
    let response = fetch(&url)?;
    if response.status != StatusCode::OK {
        return Err(eyre!(
            "SoraFS moderation operator canary route `{}` returned status {}",
            spec.name,
            response.status
        ));
    }
    let body_blake3_hex = blake3::hash(&response.body).to_hex().to_string();
    let mut schema = None;
    if let Some(expected_schema) = spec.expected_schema {
        let value: Value = norito::json::from_slice(&response.body).wrap_err_with(|| {
            format!(
                "failed to decode SoraFS moderation operator canary `{}` JSON response",
                spec.name
            )
        })?;
        moderation_operator_canary_ensure_payload_free(&value)?;
        let fields = value_object(&value, "operator canary JSON response")?;
        let actual_schema =
            required_string_field(fields, "schema", "operator canary JSON response")?;
        if actual_schema != expected_schema {
            return Err(eyre!(
                "SoraFS moderation operator canary route `{}` returned schema `{actual_schema}` (expected `{expected_schema}`)",
                spec.name
            ));
        }
        schema = Some(actual_schema.to_string());
    }
    if let Some(marker) = spec.expect_html_marker {
        let body = std::str::from_utf8(&response.body).wrap_err_with(|| {
            format!(
                "SoraFS moderation operator canary `{}` response is not UTF-8",
                spec.name
            )
        })?;
        if !body.contains(marker) {
            return Err(eyre!(
                "SoraFS moderation operator canary route `{}` response is missing `{marker}`",
                spec.name
            ));
        }
    }
    let mut route = Map::new();
    route.insert("name".into(), Value::from(spec.name));
    route.insert("method".into(), Value::from("GET"));
    route.insert("path".into(), Value::from(spec.path.clone()));
    route.insert("url".into(), Value::from(url));
    route.insert(
        "status_code".into(),
        Value::from(u64::from(response.status.as_u16())),
    );
    route.insert(
        "content_type".into(),
        response.content_type.map_or(Value::Null, Value::from),
    );
    route.insert("schema".into(), schema.map_or(Value::Null, Value::from));
    route.insert("body_blake3_hex".into(), Value::from(body_blake3_hex));
    route.insert(
        "body_bytes".into(),
        Value::from(u64::try_from(response.body.len()).unwrap_or(u64::MAX)),
    );
    route.insert("payload_bytes_included".into(), Value::Bool(false));
    route.insert("private_payloads_included".into(), Value::Bool(false));
    Ok(Value::Object(route))
}
fn moderation_operator_canary_route_url(
    operator_url: &str,
    spec: &ModerationOperatorCanaryRouteSpec,
    limit: Option<u32>,
) -> Result<String> {
    let base = format!("{}/", operator_url.trim_end_matches('/'));
    let mut url = reqwest::Url::parse(&base)
        .wrap_err_with(|| format!("failed to parse --operator-url `{operator_url}`"))?
        .join(spec.path.trim_start_matches('/'))
        .wrap_err_with(|| format!("failed to join operator route `{}`", spec.path))?;
    if spec.include_limit
        && let Some(limit) = limit
    {
        url.query_pairs_mut()
            .append_pair("limit", &limit.to_string());
    }
    Ok(url.to_string())
}
fn moderation_operator_canary_ensure_payload_free(value: &Value) -> Result<()> {
    fn visit(path: &str, value: &Value) -> Result<()> {
        match value {
            Value::Object(fields) => {
                for (key, child) in fields {
                    let child_path = if path.is_empty() {
                        key.to_string()
                    } else {
                        format!("{path}.{key}")
                    };
                    if key == "payload_b64" {
                        return Err(eyre!(
                            "operator canary response unexpectedly included payload bytes at `{child_path}`"
                        ));
                    }
                    if matches!(
                        key.as_str(),
                        "payload_bytes_included"
                            | "private_payloads_included"
                            | "private_payload_included"
                    ) && child.as_bool() == Some(true)
                    {
                        return Err(eyre!(
                            "operator canary response advertised payload bytes at `{child_path}`"
                        ));
                    }
                    visit(&child_path, child)?;
                }
            }
            Value::Array(values) => {
                for (index, child) in values.iter().enumerate() {
                    visit(&format!("{path}[{index}]"), child)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    visit("", value)
}
fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn generate_nonce_hex(bytes: usize) -> Result<String> {
    generate_nonce_hex_with_rng(bytes, &mut OsRng)
}
fn generate_moderation_operator_csrf_token() -> Result<String> {
    let mut token = [0_u8; 32];
    OsRng
        .try_fill_bytes(&mut token)
        .map_err(|error| eyre!("SoraFS moderation operator CSRF token OS RNG failed: {error}"))?;
    Ok(URL_SAFE_NO_PAD.encode(token))
}
fn generate_nonce_hex_with_rng<R: TryCryptoRng>(bytes: usize, rng: &mut R) -> Result<String> {
    let mut data = vec![0u8; bytes];
    rng.try_fill_bytes(&mut data)
        .map_err(|error| eyre!("SoraFS CLI nonce OS RNG failed: {error}"))?;
    Ok(hex::encode(data))
}
fn render_json_response<C: RunContext>(context: &mut C, response: Response<Vec<u8>>) -> Result<()> {
    let status = response.status();
    let body = response.into_body();
    match status {
        StatusCode::OK => render_json_body(context, &body),
        status => Err(make_http_error(status, &body)),
    }
}
fn render_json_response_ok_or_accepted<C: RunContext>(
    context: &mut C,
    response: Response<Vec<u8>>,
) -> Result<()> {
    let status = response.status();
    let body = response.into_body();
    match status {
        StatusCode::OK | StatusCode::ACCEPTED => render_json_body(context, &body),
        status => Err(make_http_error(status, &body)),
    }
}
fn render_json_body<C: RunContext>(context: &mut C, body: &[u8]) -> Result<()> {
    let value: norito::json::Value = norito::json::from_slice(body)?;
    context.print_data(&value)
}
fn render_moderation_quarantine_bridge_plan_response<C: RunContext>(
    context: &mut C,
    response: Response<Vec<u8>>,
    quarantine_id_hex: &str,
) -> Result<()> {
    let status = response.status();
    let body = response.into_body();
    match status {
        StatusCode::OK => {
            let panel: Value = norito::json::from_slice(&body)
                .wrap_err("failed to decode moderation operator-panel JSON")?;
            let plan = moderation_quarantine_bridge_plan_json(quarantine_id_hex, &panel)?;
            context.print_data(&plan)
        }
        status => Err(make_http_error(status, &body)),
    }
}
fn moderation_quarantine_bridge_plan_json(quarantine_id_hex: &str, panel: &Value) -> Result<Value> {
    ensure_moderation_bridge_plan_has_no_payload(panel)?;
    let root = value_object(panel, "operator panel response")?;
    let schema = required_string_field(root, "schema", "operator panel response")?;
    if schema != "sorafs.moderation.quarantine.operator_panel.v1" {
        return Err(eyre!(
            "operator panel response schema `{schema}` is not supported by bridge-plan"
        ));
    }
    let record = root
        .get("record")
        .ok_or_else(|| eyre!("operator panel response is missing `record`"))?;
    let record_obj = value_object(record, "operator panel record")?;
    let record_state = required_string_field(record_obj, "state", "operator panel record")?;
    let object_status = required_string_field(root, "object_status", "operator panel response")?;
    let case_count = root
        .get("case_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let returned_case_count = root
        .get("returned_case_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let next_actions = root
        .get("next_actions")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("operator panel response is missing `next_actions` array"))?;
    let cases = root
        .get("cases")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let case_ref = first_moderation_case_reference(cases);
    let mut actions = Vec::with_capacity(next_actions.len());
    let mut required_count = 0_u64;
    for (index, action) in next_actions.iter().enumerate() {
        let planned =
            moderation_quarantine_bridge_action_json(index, action, quarantine_id_hex, case_ref)?;
        if planned
            .as_object()
            .and_then(|fields| fields.get("required"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            required_count += 1;
        }
        actions.push(planned);
    }
    let mut plan = Map::new();
    plan.insert(
        "schema".into(),
        Value::from("sorafs.moderation.quarantine.bridge_plan.v1"),
    );
    plan.insert("source".into(), Value::from("operator-panel"));
    plan.insert("status".into(), Value::from("planned"));
    plan.insert(
        "quarantine_id_hex".into(),
        Value::from(quarantine_id_hex.to_string()),
    );
    plan.insert("record_state".into(), Value::from(record_state.to_string()));
    plan.insert(
        "object_status".into(),
        Value::from(object_status.to_string()),
    );
    plan.insert("case_count".into(), Value::from(case_count));
    plan.insert(
        "returned_case_count".into(),
        Value::from(returned_case_count),
    );
    plan.insert("action_count".into(), Value::from(actions.len() as u64));
    plan.insert("required_action_count".into(), Value::from(required_count));
    plan.insert("payload_bytes_included".into(), Value::Bool(false));
    plan.insert("actions".into(), Value::Array(actions));
    Ok(Value::Object(plan))
}
fn moderation_quarantine_bridge_action_json(
    index: usize,
    action: &Value,
    quarantine_id_hex: &str,
    ballot_ref: Option<(&str, &str)>,
) -> Result<Value> {
    let action_obj = value_object(action, "operator panel next action")?;
    let action_name = required_string_field(action_obj, "action", "operator panel next action")?;
    let route = required_string_field(action_obj, "route", "operator panel next action")?;
    let required = action_obj
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut fields = Map::new();
    fields.insert("order".into(), Value::from((index + 1) as u64));
    fields.insert("action".into(), Value::from(action_name.to_string()));
    fields.insert("required".into(), Value::Bool(required));
    fields.insert("route".into(), Value::from(route.to_string()));
    fields.insert(
        "automation_status".into(),
        Value::from(moderation_quarantine_bridge_action_status(
            action_name,
            required,
        )),
    );
    fields.insert(
        "cli".into(),
        Value::Array(
            moderation_quarantine_bridge_action_cli(action_name, quarantine_id_hex, ballot_ref)
                .into_iter()
                .map(Value::from)
                .collect(),
        ),
    );
    Ok(Value::Object(fields))
}
fn moderation_quarantine_bridge_action_status(action: &str, required: bool) -> &'static str {
    match action {
        "store_object" => "blocked_until_payload_is_sealed",
        "read_object" => {
            if required {
                "required_payload_review"
            } else {
                "available_for_operator_review"
            }
        }
        "review" => "operator_review_required",
        "appeal_handoff" => "ready_for_appeal_finance_handoff",
        "submit_native_appeal_intake" => "ready_for_caller_signed_native_appeal",
        "await_chain_sortition_activation" => "waiting_for_finalized_chain_activation",
        "submit_native_case_actions" => "waiting_for_native_commit_reveal_finalization",
        "release_complete" => "complete",
        _ => "operator_attention_required",
    }
}
fn moderation_quarantine_bridge_action_cli(
    action: &str,
    quarantine_id_hex: &str,
    ballot_ref: Option<(&str, &str)>,
) -> Vec<String> {
    let command = |parts: &[&str]| parts.iter().map(|part| (*part).to_string()).collect();
    match action {
        "store_object" => command(&[
            "iroha",
            "sorafs",
            "moderation",
            "quarantine",
            "object",
            "store",
            "--quarantine-id",
            quarantine_id_hex,
            "--payload-file",
            "<payload>",
        ]),
        "read_object" => command(&[
            "iroha",
            "sorafs",
            "moderation",
            "quarantine",
            "object",
            "read",
            "--quarantine-id",
            quarantine_id_hex,
        ]),
        "review" => command(&[
            "iroha",
            "sorafs",
            "moderation",
            "quarantine",
            "review",
            "--quarantine-id",
            quarantine_id_hex,
        ]),
        "appeal_handoff" => command(&[
            "iroha",
            "sorafs",
            "moderation",
            "quarantine",
            "appeal-handoff",
            "--quarantine-id",
            quarantine_id_hex,
            "--input",
            "<appeal-handoff.json>",
        ]),
        "submit_native_appeal_intake" => command(&[
            "iroha",
            "transaction",
            "submit",
            "--file",
            "<signed-native-moderation-appeal.norito>",
        ]),
        "await_chain_sortition_activation" => command(&[
            "iroha",
            "sorafs",
            "moderation",
            "ballots",
            "events",
            "--limit",
            "25",
        ]),
        "submit_native_case_actions" => {
            if let Some((case_id, round_id)) = ballot_ref {
                command(&[
                    "iroha",
                    "sorafs",
                    "moderation",
                    "ballots",
                    "tally",
                    "--case-id",
                    case_id,
                    "--round-id",
                    round_id,
                ])
            } else {
                command(&[
                    "iroha",
                    "sorafs",
                    "moderation",
                    "ballots",
                    "events",
                    "--limit",
                    "25",
                ])
            }
        }
        "release_complete" => command(&[
            "iroha",
            "sorafs",
            "moderation",
            "quarantine",
            "operator-panel",
            "--quarantine-id",
            quarantine_id_hex,
        ]),
        _ => command(&[
            "iroha",
            "sorafs",
            "moderation",
            "quarantine",
            "operator-panel",
            "--quarantine-id",
            quarantine_id_hex,
        ]),
    }
}
fn moderation_quarantine_juror_plan_json(quarantine_id_hex: &str, panel: &Value) -> Result<Value> {
    ensure_moderation_bridge_plan_has_no_payload(panel)?;
    let root = value_object(panel, "operator panel response")?;
    let schema = required_string_field(root, "schema", "operator panel response")?;
    if schema != "sorafs.moderation.quarantine.operator_panel.v1" {
        return Err(eyre!(
            "operator panel response schema `{schema}` is not supported by juror-plan"
        ));
    }
    let ballot_count = root
        .get("case_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let returned_ballot_count = root
        .get("returned_case_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let truncated_ballots = root
        .get("truncated_cases")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let ballots = root
        .get("cases")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut planned_ballots = Vec::with_capacity(ballots.len());
    let mut notification_count = 0_u64;
    let mut pending_commit_count = 0_u64;
    let mut pending_reveal_count = 0_u64;
    for ballot in ballots {
        let (planned, counts) = moderation_quarantine_juror_plan_ballot_json(ballot)?;
        notification_count = notification_count.saturating_add(counts.notification_count);
        pending_commit_count = pending_commit_count.saturating_add(counts.pending_commit_count);
        pending_reveal_count = pending_reveal_count.saturating_add(counts.pending_reveal_count);
        planned_ballots.push(planned);
    }
    let mut plan = Map::new();
    plan.insert(
        "schema".into(),
        Value::from("sorafs.moderation.quarantine.juror_plan.v1"),
    );
    plan.insert("source".into(), Value::from("operator-panel"));
    plan.insert("status".into(), Value::from("planned"));
    plan.insert(
        "quarantine_id_hex".into(),
        Value::from(quarantine_id_hex.to_string()),
    );
    plan.insert("ballot_count".into(), Value::from(ballot_count));
    plan.insert(
        "returned_ballot_count".into(),
        Value::from(returned_ballot_count),
    );
    plan.insert("truncated_ballots".into(), Value::Bool(truncated_ballots));
    plan.insert("notification_count".into(), Value::from(notification_count));
    plan.insert(
        "pending_commit_count".into(),
        Value::from(pending_commit_count),
    );
    plan.insert(
        "pending_reveal_count".into(),
        Value::from(pending_reveal_count),
    );
    plan.insert("payload_bytes_included".into(), Value::Bool(false));
    plan.insert("ballots".into(), Value::Array(planned_ballots));
    Ok(Value::Object(plan))
}
fn moderation_quarantine_juror_notifications_json(
    quarantine_id_hex: &str,
    panel: &Value,
) -> Result<Value> {
    let plan = moderation_quarantine_juror_plan_json(quarantine_id_hex, panel)?;
    moderation_quarantine_juror_notifications_from_plan(quarantine_id_hex, &plan)
}
fn moderation_quarantine_juror_notifications_from_plan(
    quarantine_id_hex: &str,
    plan: &Value,
) -> Result<Value> {
    let root = value_object(plan, "juror notification plan")?;
    let schema = required_string_field(root, "schema", "juror notification plan")?;
    if schema != "sorafs.moderation.quarantine.juror_plan.v1" {
        return Err(eyre!(
            "juror notification plan schema `{schema}` is not supported by notification delivery"
        ));
    }
    let ballots = root
        .get("ballots")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut notifications = Vec::new();
    let mut planned_juror_count = 0_u64;
    let mut skipped_complete_count = 0_u64;
    let mut pending_commit_count = 0_u64;
    let mut pending_reveal_count = 0_u64;
    for ballot in ballots {
        let ballot_obj = value_object(ballot, "juror notification ballot")?;
        let case_id = required_string_field(ballot_obj, "case_id", "juror notification ballot")?;
        let round_id = required_string_field(ballot_obj, "round_id", "juror notification ballot")?;
        let jurors = ballot_obj
            .get("jurors")
            .and_then(Value::as_array)
            .ok_or_else(|| eyre!("juror notification ballot is missing `jurors` array"))?;
        for juror in jurors {
            planned_juror_count = planned_juror_count.saturating_add(1);
            let juror_obj = value_object(juror, "juror notification entry")?;
            let needs_commit = juror_obj
                .get("needs_commit")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let needs_reveal = juror_obj
                .get("needs_reveal")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !needs_commit && !needs_reveal {
                skipped_complete_count = skipped_complete_count.saturating_add(1);
                continue;
            }
            let notification = moderation_quarantine_juror_notification_delivery_json(
                quarantine_id_hex,
                case_id,
                round_id,
                ballot_obj,
                juror_obj,
                needs_commit,
            )?;
            if needs_commit {
                pending_commit_count = pending_commit_count.saturating_add(1);
            } else if needs_reveal {
                pending_reveal_count = pending_reveal_count.saturating_add(1);
            }
            notifications.push(notification);
        }
    }
    let mut delivery = Map::new();
    delivery.insert(
        "schema".into(),
        Value::from("sorafs.moderation.quarantine.juror_notifications.v1"),
    );
    delivery.insert("source".into(), Value::from("juror-plan"));
    delivery.insert("status".into(), Value::from("ready"));
    delivery.insert(
        "quarantine_id_hex".into(),
        Value::from(quarantine_id_hex.to_string()),
    );
    delivery.insert(
        "planned_juror_count".into(),
        Value::from(planned_juror_count),
    );
    delivery.insert(
        "notification_count".into(),
        Value::from(notifications.len() as u64),
    );
    delivery.insert(
        "skipped_complete_count".into(),
        Value::from(skipped_complete_count),
    );
    delivery.insert(
        "pending_commit_count".into(),
        Value::from(pending_commit_count),
    );
    delivery.insert(
        "pending_reveal_count".into(),
        Value::from(pending_reveal_count),
    );
    delivery.insert("delivery_transport".into(), Value::from("operator-managed"));
    delivery.insert(
        "delivery_semantics".into(),
        Value::from("at-least-once-with-dedup-key"),
    );
    delivery.insert("payload_bytes_included".into(), Value::Bool(false));
    delivery.insert("private_payloads_included".into(), Value::Bool(false));
    delivery.insert("notifications".into(), Value::Array(notifications));
    Ok(Value::Object(delivery))
}
fn moderation_quarantine_commit_reveal_status_json(
    quarantine_id_hex: &str,
    panel: &Value,
) -> Result<Value> {
    let plan = moderation_quarantine_juror_plan_json(quarantine_id_hex, panel)?;
    moderation_quarantine_commit_reveal_status_from_plan(quarantine_id_hex, &plan)
}
fn moderation_quarantine_commit_reveal_status_from_plan(
    quarantine_id_hex: &str,
    plan: &Value,
) -> Result<Value> {
    let root = value_object(plan, "commit/reveal coordination plan")?;
    let schema = required_string_field(root, "schema", "commit/reveal coordination plan")?;
    if schema != "sorafs.moderation.quarantine.juror_plan.v1" {
        return Err(eyre!(
            "juror notification plan schema `{schema}` is not supported by commit/reveal coordination"
        ));
    }
    let ballots = root
        .get("ballots")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut ballot_statuses = Vec::with_capacity(ballots.len());
    let mut pending_commit_count = 0_u64;
    let mut pending_reveal_count = 0_u64;
    let mut commit_quorum_count = 0_u64;
    let mut reveal_quorum_count = 0_u64;
    let mut tally_ready_count = 0_u64;
    let mut tallied_count = 0_u64;
    for ballot in ballots {
        let (status, counts) =
            moderation_quarantine_commit_reveal_ballot_status_json(quarantine_id_hex, ballot)?;
        pending_commit_count = pending_commit_count.saturating_add(counts.pending_commit_count);
        pending_reveal_count = pending_reveal_count.saturating_add(counts.pending_reveal_count);
        if counts.commit_quorum_met {
            commit_quorum_count = commit_quorum_count.saturating_add(1);
        }
        if counts.reveal_quorum_met {
            reveal_quorum_count = reveal_quorum_count.saturating_add(1);
        }
        if counts.ready_to_tally {
            tally_ready_count = tally_ready_count.saturating_add(1);
        }
        if counts.tallied {
            tallied_count = tallied_count.saturating_add(1);
        }
        ballot_statuses.push(status);
    }
    let mut status = Map::new();
    status.insert(
        "schema".into(),
        Value::from("sorafs.moderation.quarantine.commit_reveal_status.v1"),
    );
    status.insert("source".into(), Value::from("juror-plan"));
    status.insert("status".into(), Value::from("coordinated"));
    status.insert(
        "quarantine_id_hex".into(),
        Value::from(quarantine_id_hex.to_string()),
    );
    status.insert(
        "ballot_count".into(),
        Value::from(ballot_statuses.len() as u64),
    );
    status.insert(
        "pending_commit_count".into(),
        Value::from(pending_commit_count),
    );
    status.insert(
        "pending_reveal_count".into(),
        Value::from(pending_reveal_count),
    );
    status.insert(
        "commit_quorum_count".into(),
        Value::from(commit_quorum_count),
    );
    status.insert(
        "reveal_quorum_count".into(),
        Value::from(reveal_quorum_count),
    );
    status.insert("tally_ready_count".into(), Value::from(tally_ready_count));
    status.insert("tallied_count".into(), Value::from(tallied_count));
    status.insert("payload_bytes_included".into(), Value::Bool(false));
    status.insert("private_payloads_included".into(), Value::Bool(false));
    status.insert("ballots".into(), Value::Array(ballot_statuses));
    Ok(Value::Object(status))
}
#[derive(Default)]
struct ModerationCommitRevealStatusCounts {
    pending_commit_count: u64,
    pending_reveal_count: u64,
    commit_quorum_met: bool,
    reveal_quorum_met: bool,
    ready_to_tally: bool,
    tallied: bool,
}
fn moderation_quarantine_commit_reveal_ballot_status_json(
    quarantine_id_hex: &str,
    ballot: &Value,
) -> Result<(Value, ModerationCommitRevealStatusCounts)> {
    let ballot_obj = value_object(ballot, "commit/reveal ballot status")?;
    let case_id = required_string_field(ballot_obj, "case_id", "commit/reveal ballot status")?;
    let round_id = required_string_field(ballot_obj, "round_id", "commit/reveal ballot status")?;
    let quorum = ballot_obj.get("quorum").and_then(Value::as_u64);
    let juror_count = ballot_obj
        .get("juror_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let committed_count = ballot_obj
        .get("committed_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let revealed_count = ballot_obj
        .get("revealed_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let tally_status =
        required_string_field(ballot_obj, "tally_status", "commit/reveal ballot status")?;
    let jurors = ballot_obj
        .get("jurors")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("commit/reveal ballot status is missing `jurors` array"))?;
    let mut missing_commit_jurors = Vec::new();
    let mut missing_reveal_jurors = Vec::new();
    for juror in jurors {
        let juror_obj = value_object(juror, "commit/reveal juror status")?;
        let juror_id = required_string_field(juror_obj, "juror_id", "commit/reveal juror status")?;
        if juror_obj
            .get("needs_commit")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            missing_commit_jurors.push(Value::from(juror_id.to_string()));
        }
        if juror_obj
            .get("needs_reveal")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            missing_reveal_jurors.push(Value::from(juror_id.to_string()));
        }
    }
    let quorum_known = quorum.is_some();
    let commit_quorum_met = quorum.is_some_and(|value| committed_count >= value);
    let reveal_quorum_met = quorum.is_some_and(|value| revealed_count >= value);
    let pending_commit_count = missing_commit_jurors.len() as u64;
    let pending_reveal_count = missing_reveal_jurors.len() as u64;
    let tallied = tally_status == "tallied";
    let (next_action, automation_status, ready_to_tally) = if tallied {
        ("complete", "tallied", false)
    } else if !commit_quorum_met {
        ("collect_commits", "awaiting_commit_quorum", false)
    } else if !reveal_quorum_met {
        ("collect_reveals", "awaiting_reveal_quorum", false)
    } else {
        ("submit_tally", "ready_for_tally", true)
    };
    let mut status = Map::new();
    status.insert("case_id".into(), Value::from(case_id.to_string()));
    status.insert("round_id".into(), Value::from(round_id.to_string()));
    status.insert(
        "quarantine_id_hex".into(),
        Value::from(quarantine_id_hex.to_string()),
    );
    status.insert(
        "quorum".into(),
        ballot_obj.get("quorum").cloned().unwrap_or(Value::Null),
    );
    status.insert("quorum_known".into(), Value::Bool(quorum_known));
    status.insert("juror_count".into(), Value::from(juror_count));
    status.insert("committed_count".into(), Value::from(committed_count));
    status.insert("revealed_count".into(), Value::from(revealed_count));
    status.insert("commit_quorum_met".into(), Value::Bool(commit_quorum_met));
    status.insert("reveal_quorum_met".into(), Value::Bool(reveal_quorum_met));
    status.insert(
        "pending_commit_count".into(),
        Value::from(pending_commit_count),
    );
    status.insert(
        "pending_reveal_count".into(),
        Value::from(pending_reveal_count),
    );
    status.insert(
        "missing_commit_jurors".into(),
        Value::Array(missing_commit_jurors),
    );
    status.insert(
        "missing_reveal_jurors".into(),
        Value::Array(missing_reveal_jurors),
    );
    status.insert("tally_status".into(), Value::from(tally_status.to_string()));
    status.insert("next_action".into(), Value::from(next_action));
    status.insert("automation_status".into(), Value::from(automation_status));
    status.insert("ready_to_tally".into(), Value::Bool(ready_to_tally));
    let tally_request = if ready_to_tally {
        let mut request = Map::new();
        request.insert(
            "route".into(),
            Value::from("/v1/sorafs/moderation/ballots/tally"),
        );
        request.insert(
            "instruction".into(),
            Value::from("FinalizeSorafsModerationCase"),
        );
        request.insert(
            "submission".into(),
            Value::from("caller-signed-native-transaction"),
        );
        request.insert(
            "cli".into(),
            Value::Array(
                [
                    "iroha",
                    "sorafs",
                    "moderation",
                    "ballots",
                    "tally",
                    "--case-id",
                    case_id,
                    "--round-id",
                    round_id,
                ]
                .into_iter()
                .map(Value::from)
                .collect(),
            ),
        );
        Value::Object(request)
    } else {
        Value::Null
    };
    status.insert("tally_request".into(), tally_request);
    status.insert("payload_bytes_included".into(), Value::Bool(false));
    status.insert("private_payloads_included".into(), Value::Bool(false));
    Ok((
        Value::Object(status),
        ModerationCommitRevealStatusCounts {
            pending_commit_count,
            pending_reveal_count,
            commit_quorum_met,
            reveal_quorum_met,
            ready_to_tally,
            tallied,
        },
    ))
}
fn moderation_quarantine_juror_notification_delivery_json(
    quarantine_id_hex: &str,
    case_id: &str,
    round_id: &str,
    ballot_obj: &Map,
    juror_obj: &Map,
    needs_commit: bool,
) -> Result<Value> {
    let juror_id = required_string_field(juror_obj, "juror_id", "juror notification entry")?;
    let notification_status =
        required_string_field(juror_obj, "notification_status", "juror notification entry")?;
    let signed_by = required_string_field(juror_obj, "signed_by", "juror notification entry")?;
    let (action, route_key, cli_key, deadline_field, title_action) = if needs_commit {
        (
            "submit_commit",
            "commit",
            "commit_cli",
            "commit_deadline_unix_ms",
            "commit",
        )
    } else {
        (
            "submit_reveal",
            "reveal",
            "reveal_cli",
            "reveal_deadline_unix_ms",
            "reveal",
        )
    };
    let route = juror_obj
        .get("routes")
        .and_then(Value::as_object)
        .and_then(|routes| routes.get(route_key))
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("juror notification entry is missing `{route_key}` route"))?;
    let cli = juror_obj
        .get(cli_key)
        .cloned()
        .ok_or_else(|| eyre!("juror notification entry is missing `{cli_key}`"))?;
    let delivery_id = moderation_juror_notification_delivery_id(
        quarantine_id_hex,
        case_id,
        round_id,
        juror_id,
        action,
    );
    let deadline = ballot_obj
        .get(deadline_field)
        .cloned()
        .unwrap_or(Value::Null);
    let evidence_uri = ballot_obj
        .get("evidence_uri")
        .cloned()
        .unwrap_or(Value::Null);
    let subject = format!("SoraFS moderation {title_action} required for {case_id}/{round_id}");
    let body = format!(
        "Juror {juror_id} must {title_action} moderation ballot {case_id}/{round_id}. Sign as {signed_by} and submit to {route}. Build any private commit/reveal payload locally; this notification intentionally carries no payload bytes."
    );
    let mut notification = Map::new();
    notification.insert(
        "schema".into(),
        Value::from("sorafs.moderation.juror_notification.v1"),
    );
    notification.insert("delivery_id".into(), Value::from(delivery_id.clone()));
    notification.insert(
        "dedup_key".into(),
        Value::from(format!("sorafs-moderation-juror:{delivery_id}")),
    );
    notification.insert("delivery_status".into(), Value::from("ready_for_delivery"));
    notification.insert("delivery_transport".into(), Value::from("operator-managed"));
    notification.insert(
        "quarantine_id_hex".into(),
        Value::from(quarantine_id_hex.to_string()),
    );
    notification.insert("case_id".into(), Value::from(case_id.to_string()));
    notification.insert("round_id".into(), Value::from(round_id.to_string()));
    notification.insert("juror_id".into(), Value::from(juror_id.to_string()));
    notification.insert("signed_by".into(), Value::from(signed_by.to_string()));
    notification.insert("action".into(), Value::from(action));
    notification.insert(
        "notification_status".into(),
        Value::from(notification_status.to_string()),
    );
    notification.insert("route".into(), Value::from(route.to_string()));
    notification.insert("cli".into(), cli);
    notification.insert("subject".into(), Value::from(subject));
    notification.insert("body".into(), Value::from(body));
    notification.insert("deadline_unix_ms".into(), deadline);
    notification.insert("evidence_uri".into(), evidence_uri);
    notification.insert("payload_bytes_included".into(), Value::Bool(false));
    notification.insert("private_payload_included".into(), Value::Bool(false));
    notification.insert("private_payload_source".into(), Value::from("juror-local"));
    Ok(Value::Object(notification))
}
fn moderation_juror_notification_delivery_id(
    quarantine_id_hex: &str,
    case_id: &str,
    round_id: &str,
    juror_id: &str,
    action: &str,
) -> String {
    let mut hasher = blake3::Hasher::new();
    for part in [
        "sorafs.moderation.juror_notification.v1",
        quarantine_id_hex,
        case_id,
        round_id,
        juror_id,
        action,
    ] {
        hasher.update(part.as_bytes());
        hasher.update(&[0]);
    }
    encode(hasher.finalize().as_bytes())
}
#[derive(Default)]
struct ModerationJurorPlanCounts {
    notification_count: u64,
    pending_commit_count: u64,
    pending_reveal_count: u64,
}
fn moderation_quarantine_juror_plan_ballot_json(
    ballot: &Value,
) -> Result<(Value, ModerationJurorPlanCounts)> {
    let ballot_obj = value_object(ballot, "operator panel finalized case")?;
    let case = ballot_obj
        .get("case")
        .ok_or_else(|| eyre!("operator panel finalized case is missing `case`"))?;
    let case_obj = value_object(case, "operator panel finalized case record")?;
    let spec = case_obj
        .get("spec")
        .ok_or_else(|| eyre!("operator panel finalized case record is missing `spec`"))?;
    let spec_obj = value_object(spec, "operator panel finalized case spec")?;
    let context = spec_obj
        .get("context")
        .ok_or_else(|| eyre!("operator panel finalized case spec is missing `context`"))?;
    let context_obj = value_object(context, "operator panel finalized case context")?;
    let case_id = required_string_field(
        context_obj,
        "case_id",
        "operator panel finalized case context",
    )?;
    let round_id =
        required_string_field(spec_obj, "round_id", "operator panel finalized case spec")?;
    let juror_values = spec_obj
        .get("jurors")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("operator panel finalized case spec is missing `jurors` array"))?;
    let commits = moderation_juror_ids_from_entries(ballot_obj, "commits")?;
    let reveals = moderation_juror_ids_from_entries(ballot_obj, "reveals")?;
    let mut jurors = Vec::with_capacity(juror_values.len());
    let mut counts = ModerationJurorPlanCounts::default();
    for juror in juror_values {
        let juror_id = juror
            .as_str()
            .ok_or_else(|| eyre!("operator panel ballot juror id must be a string"))?;
        let planned = moderation_quarantine_juror_plan_entry_json(
            case_id, round_id, juror_id, &commits, &reveals,
        )?;
        let planned_obj = value_object(&planned, "juror notification plan entry")?;
        if planned_obj
            .get("needs_commit")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            counts.pending_commit_count = counts.pending_commit_count.saturating_add(1);
        }
        if planned_obj
            .get("needs_reveal")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            counts.pending_reveal_count = counts.pending_reveal_count.saturating_add(1);
        }
        counts.notification_count = counts.notification_count.saturating_add(1);
        jurors.push(planned);
    }
    let mut fields = Map::new();
    fields.insert("case_id".into(), Value::from(case_id.to_string()));
    fields.insert("round_id".into(), Value::from(round_id.to_string()));
    fields.insert(
        "evidence_uri".into(),
        context_obj
            .get("evidence_uri")
            .cloned()
            .unwrap_or(Value::Null),
    );
    fields.insert(
        "quorum".into(),
        spec_obj.get("quorum").cloned().unwrap_or(Value::Null),
    );
    fields.insert(
        "announced_at_unix_ms".into(),
        case_obj
            .get("opened_at_unix_ms")
            .cloned()
            .unwrap_or(Value::Null),
    );
    for field in [
        "commit_deadline_unix_ms",
        "challenge_deadline_unix_ms",
        "reveal_deadline_unix_ms",
    ] {
        fields.insert(
            field.into(),
            spec_obj.get(field).cloned().unwrap_or(Value::Null),
        );
    }
    fields.insert("juror_count".into(), Value::from(juror_values.len() as u64));
    fields.insert("committed_count".into(), Value::from(commits.len() as u64));
    fields.insert("revealed_count".into(), Value::from(reveals.len() as u64));
    fields.insert(
        "tally_status".into(),
        Value::from(
            if ballot_obj
                .get("outcome")
                .is_some_and(|tally| !tally.is_null())
            {
                "tallied"
            } else {
                "pending"
            },
        ),
    );
    fields.insert("jurors".into(), Value::Array(jurors));
    Ok((Value::Object(fields), counts))
}
fn moderation_juror_ids_from_entries(ballot_obj: &Map, field: &str) -> Result<BTreeSet<String>> {
    let mut jurors = BTreeSet::new();
    let Some(entries) = ballot_obj.get(field).and_then(Value::as_array) else {
        return Ok(jurors);
    };
    for entry in entries {
        let entry_obj = value_object(entry, field)?;
        let juror_id = required_string_field(entry_obj, "juror", field)?;
        jurors.insert(juror_id.to_string());
    }
    Ok(jurors)
}
fn moderation_quarantine_juror_plan_entry_json(
    case_id: &str,
    round_id: &str,
    juror_id: &str,
    commits: &BTreeSet<String>,
    reveals: &BTreeSet<String>,
) -> Result<Value> {
    let committed = commits.contains(juror_id);
    let revealed = reveals.contains(juror_id);
    let needs_commit = !committed;
    let needs_reveal = committed && !revealed;
    let mut entry = Map::new();
    entry.insert("juror_id".into(), Value::from(juror_id.to_string()));
    entry.insert("case_id".into(), Value::from(case_id.to_string()));
    entry.insert("round_id".into(), Value::from(round_id.to_string()));
    entry.insert(
        "notification_status".into(),
        Value::from(if revealed {
            "complete"
        } else if committed {
            "reveal_required"
        } else {
            "commit_required"
        }),
    );
    entry.insert(
        "commit_status".into(),
        Value::from(if committed { "accepted" } else { "pending" }),
    );
    entry.insert(
        "reveal_status".into(),
        Value::from(if revealed {
            "accepted"
        } else if committed {
            "pending"
        } else {
            "waiting_for_commit"
        }),
    );
    entry.insert("needs_commit".into(), Value::Bool(needs_commit));
    entry.insert("needs_reveal".into(), Value::Bool(needs_reveal));
    entry.insert("signed_by".into(), Value::from(juror_id.to_string()));
    entry.insert(
        "routes".into(),
        norito::json!({
            "commit": "/v1/sorafs/moderation/ballots/commits",
            "reveal": "/v1/sorafs/moderation/ballots/reveals"
        }),
    );
    entry.insert(
        "commit_cli".into(),
        Value::Array(
            [
                "iroha",
                "sorafs",
                "moderation",
                "ballots",
                "commit",
                "--payload",
                "<commit-payload.json>",
            ]
            .into_iter()
            .map(Value::from)
            .collect(),
        ),
    );
    entry.insert(
        "reveal_cli".into(),
        Value::Array(
            [
                "iroha",
                "sorafs",
                "moderation",
                "ballots",
                "reveal",
                "--payload",
                "<reveal-payload.json>",
            ]
            .into_iter()
            .map(Value::from)
            .collect(),
        ),
    );
    Ok(Value::Object(entry))
}
fn first_moderation_case_reference(cases: &[Value]) -> Option<(&str, &str)> {
    for case in cases {
        let case_obj = case.as_object()?.get("case")?.as_object()?;
        let spec = case_obj.get("spec")?.as_object()?;
        let round_id = spec.get("round_id")?.as_str()?;
        let context = spec.get("context")?.as_object()?;
        let case_id = context.get("case_id")?.as_str()?;
        return Some((case_id, round_id));
    }
    None
}
fn ensure_moderation_bridge_plan_has_no_payload(value: &Value) -> Result<()> {
    fn visit(path: &str, value: &Value) -> Result<()> {
        match value {
            Value::Object(fields) => {
                for (key, child) in fields {
                    let child_path = if path.is_empty() {
                        key.to_string()
                    } else {
                        format!("{path}.{key}")
                    };
                    if key == "payload_b64" {
                        return Err(eyre!(
                            "operator panel response unexpectedly included payload bytes at `{child_path}`"
                        ));
                    }
                    visit(&child_path, child)?;
                }
            }
            Value::Array(values) => {
                for (index, child) in values.iter().enumerate() {
                    visit(&format!("{path}[{index}]"), child)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    visit("", value)
}
fn value_object<'a>(value: &'a Value, context: &str) -> Result<&'a Map> {
    value
        .as_object()
        .ok_or_else(|| eyre!("{context} must be a JSON object"))
}
fn required_string_field<'a>(fields: &'a Map, field: &str, context: &str) -> Result<&'a str> {
    fields
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("{context} is missing string `{field}`"))
}
fn make_http_error(status: StatusCode, body: &[u8]) -> eyre::Report {
    let message = String::from_utf8_lossy(body);
    eyre!("request failed with status {status}: {message}")
}
trait ModerationOperatorWorkflowSource: Send + Sync {
    fn get_operator_panel(
        &self,
        quarantine_id_hex: &str,
        filter: SorafsModerationQuarantineFilter,
    ) -> Result<Response<Vec<u8>>>;
    fn post_review(
        &self,
        _quarantine_id_hex: &str,
        _request: &SorafsModerationQuarantineReviewRequest<'_>,
    ) -> Result<Response<Vec<u8>>> {
        Err(eyre!(
            "SoraFS moderation operator service review forwarding is unavailable"
        ))
    }
    fn post_release(
        &self,
        _quarantine_id_hex: &str,
        _request: &SorafsModerationQuarantineReleaseRequest<'_>,
    ) -> Result<Response<Vec<u8>>> {
        Err(eyre!(
            "SoraFS moderation operator service release forwarding is unavailable"
        ))
    }
    fn post_appeal_handoff(
        &self,
        _quarantine_id_hex: &str,
        _payload: &[u8],
    ) -> Result<Response<Vec<u8>>> {
        Err(eyre!(
            "SoraFS moderation operator service appeal-handoff forwarding is unavailable"
        ))
    }
}
impl ModerationOperatorWorkflowSource for Client {
    fn get_operator_panel(
        &self,
        quarantine_id_hex: &str,
        filter: SorafsModerationQuarantineFilter,
    ) -> Result<Response<Vec<u8>>> {
        self.get_sorafs_moderation_quarantine_operator_panel(quarantine_id_hex, filter)
    }
    fn post_review(
        &self,
        quarantine_id_hex: &str,
        request: &SorafsModerationQuarantineReviewRequest<'_>,
    ) -> Result<Response<Vec<u8>>> {
        self.post_sorafs_moderation_quarantine_review(quarantine_id_hex, request)
    }
    fn post_release(
        &self,
        quarantine_id_hex: &str,
        request: &SorafsModerationQuarantineReleaseRequest<'_>,
    ) -> Result<Response<Vec<u8>>> {
        self.post_sorafs_moderation_quarantine_release(quarantine_id_hex, request)
    }
    fn post_appeal_handoff(
        &self,
        quarantine_id_hex: &str,
        payload: &[u8],
    ) -> Result<Response<Vec<u8>>> {
        self.post_sorafs_moderation_quarantine_appeal_handoff_json(quarantine_id_hex, payload)
    }
}
struct ModerationOperatorService {
    listen: String,
    default_limit: Option<u32>,
    max_body_bytes: usize,
    upstream: String,
    default_actor: String,
    csrf_token: String,
    workflow_source: Arc<dyn ModerationOperatorWorkflowSource>,
}
impl ModerationOperatorService {
    const HTML_CONTENT_TYPE: &'static str = "text/html; charset=utf-8";
    const JSON_CONTENT_TYPE: &'static str = "application/json";
    fn status_json(&self) -> Value {
        let mut fields = Map::new();
        fields.insert(
            "schema".into(),
            Value::from("sorafs.moderation.quarantine.operator_service.status.v1"),
        );
        fields.insert("status".into(), Value::from("listening"));
        fields.insert("source".into(), Value::from("iroha_cli"));
        fields.insert("listen".into(), Value::from(self.listen.clone()));
        fields.insert("upstream".into(), Value::from(self.upstream.clone()));
        fields.insert(
            "default_actor".into(),
            Value::from(self.default_actor.clone()),
        );
        fields.insert(
            "default_limit".into(),
            self.default_limit.map_or(Value::Null, Value::from),
        );
        fields.insert(
            "max_body_bytes".into(),
            Value::from(u64::try_from(self.max_body_bytes).unwrap_or(u64::MAX)),
        );
        fields.insert(
            "csrf_header".into(),
            Value::from(MODERATION_OPERATOR_CSRF_HEADER),
        );
        fields.insert("csrf_token".into(), Value::from(self.csrf_token.clone()));
        fields.insert("payload_bytes_included".into(), Value::Bool(false));
        fields.insert(
            "routes".into(),
            Value::Array(vec![
                Value::from("/"),
                Value::from("/healthz"),
                Value::from("/v1/sorafs/moderation/operator-panel/ui"),
                Value::from("/v1/sorafs/moderation/operator-panel/status"),
                Value::from("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/operator-panel"),
                Value::from("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/bridge-plan"),
                Value::from("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/juror-plan"),
                Value::from(
                    "/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/juror-notifications",
                ),
                Value::from(
                    "/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/commit-reveal-status",
                ),
                Value::from("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/review"),
                Value::from("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/release"),
                Value::from("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/appeal-handoff"),
            ]),
        );
        Value::Object(fields)
    }
    fn handle_request(
        &self,
        request: &ModerationOperatorHttpRequest<'_>,
    ) -> ModerationOperatorHttpResponse {
        let route = match moderation_operator_route(request.path) {
            Ok(route) => route,
            Err(err) => return err.into_response(),
        };
        match route {
            ModerationOperatorRoute::BrowserUi => {
                if let Err(err) = moderation_operator_expect_method(request, "GET", false) {
                    return err.into_response();
                }
                self.browser_ui_response()
            }
            ModerationOperatorRoute::Status => {
                if let Err(err) = moderation_operator_expect_method(request, "GET", false) {
                    return err.into_response();
                }
                moderation_operator_json_response(StatusCode::OK, &self.status_json())
            }
            ModerationOperatorRoute::OperatorPanel { quarantine_id_hex } => {
                if let Err(err) = moderation_operator_expect_method(request, "GET", false) {
                    return err.into_response();
                }
                let limit = match moderation_operator_query_limit(request.query, self.default_limit)
                {
                    Ok(limit) => limit,
                    Err(err) => return err.into_response(),
                };
                self.operator_panel_response(&quarantine_id_hex, limit)
            }
            ModerationOperatorRoute::BridgePlan { quarantine_id_hex } => {
                if let Err(err) = moderation_operator_expect_method(request, "GET", false) {
                    return err.into_response();
                }
                let limit = match moderation_operator_query_limit(request.query, self.default_limit)
                {
                    Ok(limit) => limit,
                    Err(err) => return err.into_response(),
                };
                self.bridge_plan_response(&quarantine_id_hex, limit)
            }
            ModerationOperatorRoute::JurorPlan { quarantine_id_hex } => {
                if let Err(err) = moderation_operator_expect_method(request, "GET", false) {
                    return err.into_response();
                }
                let limit = match moderation_operator_query_limit(request.query, self.default_limit)
                {
                    Ok(limit) => limit,
                    Err(err) => return err.into_response(),
                };
                self.juror_plan_response(&quarantine_id_hex, limit)
            }
            ModerationOperatorRoute::JurorNotifications { quarantine_id_hex } => {
                if let Err(err) = moderation_operator_expect_method(request, "GET", false) {
                    return err.into_response();
                }
                let limit = match moderation_operator_query_limit(request.query, self.default_limit)
                {
                    Ok(limit) => limit,
                    Err(err) => return err.into_response(),
                };
                self.juror_notifications_response(&quarantine_id_hex, limit)
            }
            ModerationOperatorRoute::CommitRevealStatus { quarantine_id_hex } => {
                if let Err(err) = moderation_operator_expect_method(request, "GET", false) {
                    return err.into_response();
                }
                let limit = match moderation_operator_query_limit(request.query, self.default_limit)
                {
                    Ok(limit) => limit,
                    Err(err) => return err.into_response(),
                };
                self.commit_reveal_status_response(&quarantine_id_hex, limit)
            }
            ModerationOperatorRoute::Review { quarantine_id_hex } => {
                if let Err(err) = moderation_operator_expect_method(request, "POST", true)
                    .and_then(|_| moderation_operator_reject_query(request.query))
                    .and_then(|_| self.require_csrf_token(request))
                {
                    return err.into_response();
                }
                self.review_response(&quarantine_id_hex, request.body)
            }
            ModerationOperatorRoute::Release { quarantine_id_hex } => {
                if let Err(err) = moderation_operator_expect_method(request, "POST", true)
                    .and_then(|_| moderation_operator_reject_query(request.query))
                    .and_then(|_| self.require_csrf_token(request))
                {
                    return err.into_response();
                }
                self.release_response(&quarantine_id_hex, request.body)
            }
            ModerationOperatorRoute::AppealHandoff { quarantine_id_hex } => {
                if let Err(err) = moderation_operator_expect_method(request, "POST", true)
                    .and_then(|_| moderation_operator_reject_query(request.query))
                    .and_then(|_| self.require_csrf_token(request))
                {
                    return err.into_response();
                }
                self.appeal_handoff_response(&quarantine_id_hex, request.body)
            }
        }
    }
    fn require_csrf_token(
        &self,
        request: &ModerationOperatorHttpRequest<'_>,
    ) -> Result<(), ModerationOperatorRequestError> {
        let mut values = request
            .headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case(MODERATION_OPERATOR_CSRF_HEADER))
            .map(|(_, value)| *value);
        let Some(value) = values.next() else {
            return Err(ModerationOperatorRequestError::new(
                StatusCode::FORBIDDEN,
                format!(
                    "SoraFS moderation operator service mutation routes require `{MODERATION_OPERATOR_CSRF_HEADER}`"
                ),
            ));
        };
        if values.next().is_some() {
            return Err(ModerationOperatorRequestError::new(
                StatusCode::FORBIDDEN,
                format!(
                    "SoraFS moderation operator service request must include only one `{MODERATION_OPERATOR_CSRF_HEADER}`"
                ),
            ));
        }
        if value != self.csrf_token {
            return Err(ModerationOperatorRequestError::new(
                StatusCode::FORBIDDEN,
                "invalid SoraFS moderation operator service CSRF token",
            ));
        }
        Ok(())
    }
    fn operator_panel_body(
        &self,
        quarantine_id_hex: &str,
        limit: Option<u32>,
    ) -> std::result::Result<Vec<u8>, ModerationOperatorHttpResponse> {
        let response = match self.workflow_source.get_operator_panel(
            quarantine_id_hex,
            SorafsModerationQuarantineFilter { limit },
        ) {
            Ok(response) => response,
            Err(err) => {
                return Err(moderation_operator_json_error(
                    StatusCode::BAD_GATEWAY,
                    format!("failed to fetch operator-panel response from Torii: {err}"),
                ));
            }
        };
        let status = response.status();
        let body = response.into_body();
        if status != StatusCode::OK {
            return Err(moderation_operator_upstream_response(status, body));
        }
        Ok(body)
    }
    fn operator_panel_response(
        &self,
        quarantine_id_hex: &str,
        limit: Option<u32>,
    ) -> ModerationOperatorHttpResponse {
        let body = match self.operator_panel_body(quarantine_id_hex, limit) {
            Ok(body) => body,
            Err(response) => return response,
        };
        match moderation_operator_payload_free_panel_json(&body) {
            Ok(panel) => moderation_operator_json_response(StatusCode::OK, &panel),
            Err(err) => moderation_operator_json_error(
                StatusCode::BAD_GATEWAY,
                format!("unsafe or invalid operator-panel response from Torii: {err}"),
            ),
        }
    }
    impl_moderation_operator_derived_response!(
        bridge_plan_response => moderation_quarantine_bridge_plan_json,
        plan,
        "failed to build payload-free bridge plan: {err}"
    );
    impl_moderation_operator_derived_response!(
        juror_plan_response => moderation_quarantine_juror_plan_json,
        plan,
        "failed to build payload-free juror notification plan: {err}"
    );
    impl_moderation_operator_derived_response!(
        commit_reveal_status_response => moderation_quarantine_commit_reveal_status_json,
        status,
        "failed to build payload-free commit/reveal coordination status: {err}"
    );
    impl_moderation_operator_derived_response!(
        juror_notifications_response => moderation_quarantine_juror_notifications_json,
        notifications,
        "failed to build payload-free juror notification delivery manifest: {err}"
    );
    fn review_response(
        &self,
        quarantine_id_hex: &str,
        body: &[u8],
    ) -> ModerationOperatorHttpResponse {
        let payload = match moderation_operator_review_payload_from_body(body, &self.default_actor)
        {
            Ok(payload) => payload,
            Err(err) => return err.into_response(),
        };
        let request = payload.as_request();
        let response = match self
            .workflow_source
            .post_review(quarantine_id_hex, &request)
        {
            Ok(response) => response,
            Err(err) => {
                return moderation_operator_json_error(
                    StatusCode::BAD_GATEWAY,
                    format!("failed to forward review request to Torii: {err}"),
                );
            }
        };
        moderation_operator_success_json_response(response, "review")
    }
    fn release_response(
        &self,
        quarantine_id_hex: &str,
        body: &[u8],
    ) -> ModerationOperatorHttpResponse {
        let payload = match moderation_operator_release_payload_from_body(body, &self.default_actor)
        {
            Ok(payload) => payload,
            Err(err) => return err.into_response(),
        };
        let request = payload.as_request();
        let response = match self
            .workflow_source
            .post_release(quarantine_id_hex, &request)
        {
            Ok(response) => response,
            Err(err) => {
                return moderation_operator_json_error(
                    StatusCode::BAD_GATEWAY,
                    format!("failed to forward release request to Torii: {err}"),
                );
            }
        };
        moderation_operator_success_json_response(response, "release")
    }
    fn appeal_handoff_response(
        &self,
        quarantine_id_hex: &str,
        body: &[u8],
    ) -> ModerationOperatorHttpResponse {
        let payload =
            match moderation_operator_payload_free_json_body(body, "appeal-handoff request") {
                Ok(payload) => payload,
                Err(err) => return err.into_response(),
            };
        let response = match self
            .workflow_source
            .post_appeal_handoff(quarantine_id_hex, &payload)
        {
            Ok(response) => response,
            Err(err) => {
                return moderation_operator_json_error(
                    StatusCode::BAD_GATEWAY,
                    format!("failed to forward appeal-handoff request to Torii: {err}"),
                );
            }
        };
        moderation_operator_success_json_response(response, "appeal-handoff")
    }
    fn browser_ui_response(&self) -> ModerationOperatorHttpResponse {
        let html = MODERATION_OPERATOR_BROWSER_UI_HTML
            .replace(
                "__SORAFS_OPERATOR_CSRF_HEADER__",
                MODERATION_OPERATOR_CSRF_HEADER,
            )
            .replace("__SORAFS_OPERATOR_CSRF_TOKEN__", &self.csrf_token);
        ModerationOperatorHttpResponse {
            status: StatusCode::OK,
            content_type: Self::HTML_CONTENT_TYPE,
            body: html.into_bytes(),
        }
    }
}
const MODERATION_OPERATOR_BROWSER_UI_HTML: &str =
    include_str!("sorafs/moderation_operator_browser_ui.v1.html");
#[derive(Debug)]
struct ModerationOperatorHttpRequest<'a> {
    method: &'a str,
    path: &'a str,
    query: Option<&'a str>,
    headers: Vec<(&'a str, &'a str)>,
    body: &'a [u8],
}
#[derive(Debug)]
struct ModerationOperatorHttpResponse {
    status: StatusCode,
    content_type: &'static str,
    body: Vec<u8>,
}
impl ModerationOperatorHttpResponse {
    fn to_http_bytes(&self) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.status.as_u16(),
            moderation_operator_status_reason(self.status),
            self.content_type,
            self.body.len()
        )
        .into_bytes();
        response.extend_from_slice(&self.body);
        response
    }
}
#[derive(Debug)]
struct ModerationOperatorRequestError {
    status: StatusCode,
    message: String,
}
impl ModerationOperatorRequestError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
    fn into_response(self) -> ModerationOperatorHttpResponse {
        moderation_operator_json_error(self.status, self.message)
    }
}
#[derive(Debug, PartialEq, Eq)]
enum ModerationOperatorRoute {
    BrowserUi,
    Status,
    OperatorPanel { quarantine_id_hex: String },
    BridgePlan { quarantine_id_hex: String },
    JurorPlan { quarantine_id_hex: String },
    JurorNotifications { quarantine_id_hex: String },
    CommitRevealStatus { quarantine_id_hex: String },
    Review { quarantine_id_hex: String },
    Release { quarantine_id_hex: String },
    AppealHandoff { quarantine_id_hex: String },
}
fn moderation_operator_handle_stream(
    mut stream: TcpStream,
    service: &ModerationOperatorService,
) -> Result<()> {
    let response = match moderation_operator_read_http_request(&mut stream, service.max_body_bytes)
    {
        Ok(raw) => match moderation_operator_parse_http_request(&raw, service.max_body_bytes) {
            Ok(request) => service.handle_request(&request),
            Err(err) => err.into_response(),
        },
        Err(err) => err.into_response(),
    };
    stream
        .write_all(&response.to_http_bytes())
        .wrap_err("failed to write SoraFS moderation operator service response")?;
    stream
        .flush()
        .wrap_err("failed to flush SoraFS moderation operator service response")
}
fn moderation_operator_read_http_request(
    stream: &mut TcpStream,
    max_body_bytes: usize,
) -> Result<Vec<u8>, ModerationOperatorRequestError> {
    const MAX_HEADER_BYTES: usize = 16 * 1024;
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end = loop {
        if let Some(header_end) = moderation_operator_find_header_end(&buffer) {
            break header_end;
        }
        if buffer.len() > MAX_HEADER_BYTES {
            return Err(ModerationOperatorRequestError::new(
                StatusCode::BAD_REQUEST,
                "SoraFS moderation operator service request headers are too large",
            ));
        }
        let read = stream.read(&mut chunk).map_err(|err| {
            ModerationOperatorRequestError::new(
                StatusCode::BAD_REQUEST,
                format!("failed to read SoraFS moderation operator service request: {err}"),
            )
        })?;
        if read == 0 {
            return Err(ModerationOperatorRequestError::new(
                StatusCode::BAD_REQUEST,
                "connection closed before HTTP request headers were complete",
            ));
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    let header_text = moderation_operator_header_text(&buffer, header_end)?;
    let content_length = moderation_operator_content_length(header_text)?;
    if content_length.is_none() && buffer.len() > header_end {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service request body requires Content-Length",
        ));
    }
    let content_length = content_length.unwrap_or(0);
    if content_length > max_body_bytes {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "SoraFS moderation operator service request body exceeds {max_body_bytes} bytes"
            ),
        ));
    }
    let request_len = header_end.checked_add(content_length).ok_or_else(|| {
        ModerationOperatorRequestError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "SoraFS moderation operator service request length overflowed",
        )
    })?;
    while buffer.len() < request_len {
        let read = stream.read(&mut chunk).map_err(|err| {
            ModerationOperatorRequestError::new(
                StatusCode::BAD_REQUEST,
                format!("failed to read SoraFS moderation operator service request body: {err}"),
            )
        })?;
        if read == 0 {
            return Err(ModerationOperatorRequestError::new(
                StatusCode::BAD_REQUEST,
                "connection closed before HTTP request body was complete",
            ));
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    if buffer.len() > request_len {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service request has trailing bytes after declared body",
        ));
    }
    buffer.truncate(request_len);
    Ok(buffer)
}
fn moderation_operator_parse_http_request(
    raw: &[u8],
    max_body_bytes: usize,
) -> Result<ModerationOperatorHttpRequest<'_>, ModerationOperatorRequestError> {
    let header_end = moderation_operator_find_header_end(raw).ok_or_else(|| {
        ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service request is missing HTTP header terminator",
        )
    })?;
    let header_text = moderation_operator_header_text(raw, header_end)?;
    let mut lines = header_text.lines();
    let request_line = lines.next().ok_or_else(|| {
        ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service request line is missing",
        )
    })?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().ok_or_else(|| {
        ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service request method is missing",
        )
    })?;
    let target = request_parts.next().ok_or_else(|| {
        ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service request target is missing",
        )
    })?;
    let version = request_parts.next().ok_or_else(|| {
        ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service HTTP version is missing",
        )
    })?;
    if request_parts.next().is_some() || !version.starts_with("HTTP/") {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service request line is malformed",
        ));
    }
    let content_length = moderation_operator_content_length(header_text)?;
    if method == "POST" && content_length.is_none() {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service POST request requires Content-Length",
        ));
    }
    if content_length.is_none() && raw.len() > header_end {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service request body requires Content-Length",
        ));
    }
    let content_length = content_length.unwrap_or(0);
    if content_length > max_body_bytes {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "SoraFS moderation operator service request body exceeds {max_body_bytes} bytes"
            ),
        ));
    }
    let body_end = header_end.checked_add(content_length).ok_or_else(|| {
        ModerationOperatorRequestError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "SoraFS moderation operator service request length overflowed",
        )
    })?;
    if raw.len() < body_end {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service request body is incomplete",
        ));
    }
    if raw.len() > body_end {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service request has trailing bytes after declared body",
        ));
    }
    let headers = moderation_operator_headers(header_text);
    let (path, query) = moderation_operator_split_target(target)?;
    Ok(ModerationOperatorHttpRequest {
        method,
        path,
        query,
        headers,
        body: &raw[header_end..body_end],
    })
}
fn moderation_operator_header_text(
    raw: &[u8],
    header_end: usize,
) -> Result<&str, ModerationOperatorRequestError> {
    let header_bytes = raw.get(..header_end.saturating_sub(4)).ok_or_else(|| {
        ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service request headers are malformed",
        )
    })?;
    std::str::from_utf8(header_bytes).map_err(|err| {
        ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            format!("SoraFS moderation operator service headers are not UTF-8: {err}"),
        )
    })
}
fn moderation_operator_content_length(
    header_text: &str,
) -> Result<Option<usize>, ModerationOperatorRequestError> {
    let mut content_length = None;
    for line in header_text.lines().skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err(ModerationOperatorRequestError::new(
                    StatusCode::BAD_REQUEST,
                    "SoraFS moderation operator service request has duplicate Content-Length",
                ));
            }
            let parsed = value.trim().parse::<usize>().map_err(|err| {
                ModerationOperatorRequestError::new(
                    StatusCode::BAD_REQUEST,
                    format!("invalid SoraFS moderation operator service Content-Length: {err}"),
                )
            })?;
            content_length = Some(parsed);
        }
    }
    Ok(content_length)
}
fn moderation_operator_headers(header_text: &str) -> Vec<(&str, &str)> {
    header_text
        .lines()
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim(), value.trim()))
        })
        .collect()
}
fn moderation_operator_split_target(
    target: &str,
) -> Result<(&str, Option<&str>), ModerationOperatorRequestError> {
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    if !path.starts_with('/') {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service request path must be absolute",
        ));
    }
    Ok((path, query))
}
fn moderation_operator_find_header_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|offset| offset + 4)
}
fn moderation_operator_route(
    path: &str,
) -> Result<ModerationOperatorRoute, ModerationOperatorRequestError> {
    if path == "/" || path == "/v1/sorafs/moderation/operator-panel/ui" {
        return Ok(ModerationOperatorRoute::BrowserUi);
    }
    if path == "/healthz" || path == "/v1/sorafs/moderation/operator-panel/status" {
        return Ok(ModerationOperatorRoute::Status);
    }
    const PREFIX: &str = "/v1/sorafs/moderation/quarantine/";
    let Some(remainder) = path.strip_prefix(PREFIX) else {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::NOT_FOUND,
            "unknown SoraFS moderation operator service route",
        ));
    };
    let Some((quarantine_id, suffix)) = remainder.split_once('/') else {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::NOT_FOUND,
            "SoraFS moderation operator service route is missing a workflow endpoint",
        ));
    };
    if suffix.contains('/') {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::NOT_FOUND,
            "unknown SoraFS moderation operator service workflow endpoint",
        ));
    }
    let quarantine_id_hex =
        normalize_hex_digest::<16>(quarantine_id, "quarantine id in request path").map_err(
            |err| ModerationOperatorRequestError::new(StatusCode::BAD_REQUEST, err.to_string()),
        )?;
    match suffix {
        "operator-panel" => Ok(ModerationOperatorRoute::OperatorPanel { quarantine_id_hex }),
        "bridge-plan" => Ok(ModerationOperatorRoute::BridgePlan { quarantine_id_hex }),
        "juror-plan" => Ok(ModerationOperatorRoute::JurorPlan { quarantine_id_hex }),
        "juror-notifications" => {
            Ok(ModerationOperatorRoute::JurorNotifications { quarantine_id_hex })
        }
        "commit-reveal-status" => {
            Ok(ModerationOperatorRoute::CommitRevealStatus { quarantine_id_hex })
        }
        "review" => Ok(ModerationOperatorRoute::Review { quarantine_id_hex }),
        "release" => Ok(ModerationOperatorRoute::Release { quarantine_id_hex }),
        "appeal-handoff" => Ok(ModerationOperatorRoute::AppealHandoff { quarantine_id_hex }),
        _ => Err(ModerationOperatorRequestError::new(
            StatusCode::NOT_FOUND,
            "unknown SoraFS moderation operator service workflow endpoint",
        )),
    }
}
fn moderation_operator_expect_method(
    request: &ModerationOperatorHttpRequest<'_>,
    expected_method: &str,
    body_required: bool,
) -> Result<(), ModerationOperatorRequestError> {
    if request.method != expected_method {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::METHOD_NOT_ALLOWED,
            format!("SoraFS moderation operator service route requires {expected_method}"),
        ));
    }
    if body_required {
        if request.body.is_empty() {
            return Err(ModerationOperatorRequestError::new(
                StatusCode::BAD_REQUEST,
                "SoraFS moderation operator service POST request body must not be empty",
            ));
        }
    } else if !request.body.is_empty() {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service GET requests must not include a body",
        ));
    }
    Ok(())
}
fn moderation_operator_reject_query(
    query: Option<&str>,
) -> Result<(), ModerationOperatorRequestError> {
    if query.is_some_and(|query| !query.is_empty()) {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "SoraFS moderation operator service mutation routes do not accept query parameters",
        ));
    }
    Ok(())
}
fn moderation_operator_query_limit(
    query: Option<&str>,
    default_limit: Option<u32>,
) -> Result<Option<u32>, ModerationOperatorRequestError> {
    let Some(query) = query else {
        return Ok(default_limit);
    };
    let mut limit = default_limit;
    let mut saw_limit = false;
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key != "limit" {
            return Err(ModerationOperatorRequestError::new(
                StatusCode::BAD_REQUEST,
                format!("unsupported SoraFS moderation operator service query parameter `{key}`"),
            ));
        }
        if saw_limit {
            return Err(ModerationOperatorRequestError::new(
                StatusCode::BAD_REQUEST,
                "SoraFS moderation operator service query parameter `limit` was repeated",
            ));
        }
        if value.is_empty() {
            return Err(ModerationOperatorRequestError::new(
                StatusCode::BAD_REQUEST,
                "SoraFS moderation operator service query parameter `limit` must not be empty",
            ));
        }
        let parsed = value.parse::<u32>().map_err(|err| {
            ModerationOperatorRequestError::new(
                StatusCode::BAD_REQUEST,
                format!("invalid SoraFS moderation operator service `limit`: {err}"),
            )
        })?;
        limit = Some(parsed);
        saw_limit = true;
    }
    Ok(limit)
}
fn moderation_operator_payload_free_panel_json(body: &[u8]) -> Result<Value> {
    let panel: Value =
        norito::json::from_slice(body).wrap_err("failed to decode operator-panel JSON")?;
    ensure_moderation_bridge_plan_has_no_payload(&panel)?;
    Ok(panel)
}
struct ModerationOperatorReviewPayload {
    reviewed_by: String,
    reviewed_at_unix: Option<u64>,
    notes: Option<String>,
}
impl ModerationOperatorReviewPayload {
    fn as_request(&self) -> SorafsModerationQuarantineReviewRequest<'_> {
        SorafsModerationQuarantineReviewRequest {
            reviewed_by: self.reviewed_by.as_str(),
            reviewed_at_unix: self.reviewed_at_unix,
            notes: self.notes.as_deref(),
        }
    }
}
struct ModerationOperatorReleasePayload {
    release_authority: String,
    released_at_unix: Option<u64>,
    notes: Option<String>,
}
impl ModerationOperatorReleasePayload {
    fn as_request(&self) -> SorafsModerationQuarantineReleaseRequest<'_> {
        SorafsModerationQuarantineReleaseRequest {
            release_authority: self.release_authority.as_str(),
            released_at_unix: self.released_at_unix,
            notes: self.notes.as_deref(),
        }
    }
}
fn moderation_operator_review_payload_from_body(
    body: &[u8],
    default_actor: &str,
) -> Result<ModerationOperatorReviewPayload, ModerationOperatorRequestError> {
    let value = moderation_operator_payload_free_json_value(body, "review request")?;
    let fields = value_object(&value, "review request").map_err(|err| {
        ModerationOperatorRequestError::new(StatusCode::BAD_REQUEST, err.to_string())
    })?;
    let reviewed_by = optional_json_text(fields, "reviewed_by")
        .map_err(|err| {
            ModerationOperatorRequestError::new(StatusCode::BAD_REQUEST, err.to_string())
        })?
        .unwrap_or_else(|| default_actor.to_string());
    let reviewed_at_unix = optional_json_u64(fields, "reviewed_at_unix").map_err(|err| {
        ModerationOperatorRequestError::new(StatusCode::BAD_REQUEST, err.to_string())
    })?;
    if reviewed_at_unix == Some(0) {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "reviewed_at_unix must be non-zero",
        ));
    }
    let notes = optional_json_text(fields, "notes").map_err(|err| {
        ModerationOperatorRequestError::new(StatusCode::BAD_REQUEST, err.to_string())
    })?;
    Ok(ModerationOperatorReviewPayload {
        reviewed_by,
        reviewed_at_unix,
        notes,
    })
}
fn moderation_operator_release_payload_from_body(
    body: &[u8],
    default_actor: &str,
) -> Result<ModerationOperatorReleasePayload, ModerationOperatorRequestError> {
    let value = moderation_operator_payload_free_json_value(body, "release request")?;
    let fields = value_object(&value, "release request").map_err(|err| {
        ModerationOperatorRequestError::new(StatusCode::BAD_REQUEST, err.to_string())
    })?;
    let release_authority = optional_json_text(fields, "release_authority")
        .map_err(|err| {
            ModerationOperatorRequestError::new(StatusCode::BAD_REQUEST, err.to_string())
        })?
        .unwrap_or_else(|| default_actor.to_string());
    let released_at_unix = optional_json_u64(fields, "released_at_unix").map_err(|err| {
        ModerationOperatorRequestError::new(StatusCode::BAD_REQUEST, err.to_string())
    })?;
    if released_at_unix == Some(0) {
        return Err(ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            "released_at_unix must be non-zero",
        ));
    }
    let notes = optional_json_text(fields, "notes").map_err(|err| {
        ModerationOperatorRequestError::new(StatusCode::BAD_REQUEST, err.to_string())
    })?;
    Ok(ModerationOperatorReleasePayload {
        release_authority,
        released_at_unix,
        notes,
    })
}
fn moderation_operator_payload_free_json_body(
    body: &[u8],
    label: &str,
) -> Result<Vec<u8>, ModerationOperatorRequestError> {
    let value = moderation_operator_payload_free_json_value(body, label)?;
    norito::json::to_vec(&value).map_err(|err| {
        ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            format!("failed to canonicalize SoraFS moderation operator service {label}: {err}"),
        )
    })
}
fn moderation_operator_payload_free_json_value(
    body: &[u8],
    label: &str,
) -> Result<Value, ModerationOperatorRequestError> {
    let value: Value = norito::json::from_slice(body).map_err(|err| {
        ModerationOperatorRequestError::new(
            StatusCode::BAD_REQUEST,
            format!("failed to parse SoraFS moderation operator service {label} JSON: {err}"),
        )
    })?;
    ensure_moderation_bridge_plan_has_no_payload(&value).map_err(|err| {
        ModerationOperatorRequestError::new(StatusCode::BAD_REQUEST, err.to_string())
    })?;
    Ok(value)
}
fn moderation_operator_success_json_response(
    response: Response<Vec<u8>>,
    operation: &str,
) -> ModerationOperatorHttpResponse {
    let status = response.status();
    let body = response.into_body();
    if !matches!(status, StatusCode::OK | StatusCode::ACCEPTED) {
        return moderation_operator_upstream_response(status, body);
    }
    match moderation_operator_payload_free_json_value(&body, operation) {
        Ok(value) => moderation_operator_json_response(status, &value),
        Err(err) => moderation_operator_json_error(
            StatusCode::BAD_GATEWAY,
            format!(
                "unsafe or invalid {operation} response from Torii: {}",
                err.message
            ),
        ),
    }
}
fn moderation_operator_upstream_response(
    status: StatusCode,
    body: Vec<u8>,
) -> ModerationOperatorHttpResponse {
    if body.is_empty() {
        moderation_operator_json_error(status, format!("Torii returned status {status}"))
    } else {
        ModerationOperatorHttpResponse {
            status,
            content_type: ModerationOperatorService::JSON_CONTENT_TYPE,
            body,
        }
    }
}
fn moderation_operator_json_response(
    status: StatusCode,
    value: &Value,
) -> ModerationOperatorHttpResponse {
    let body = norito::json::to_vec(value).unwrap_or_else(|_| {
        br#"{"schema":"sorafs.moderation.quarantine.operator_service.error.v1","error":"failed to encode SoraFS moderation operator service JSON"}"#.to_vec()
    });
    ModerationOperatorHttpResponse {
        status,
        content_type: ModerationOperatorService::JSON_CONTENT_TYPE,
        body,
    }
}
fn moderation_operator_json_error(
    status: StatusCode,
    message: impl Into<String>,
) -> ModerationOperatorHttpResponse {
    let message = message.into();
    moderation_operator_json_response(
        status,
        &norito::json!({
            "schema": "sorafs.moderation.quarantine.operator_service.error.v1",
            "error": (message)
        }),
    )
}
fn moderation_operator_status_reason(status: StatusCode) -> &'static str {
    match status {
        StatusCode::OK => "OK",
        StatusCode::ACCEPTED => "Accepted",
        StatusCode::BAD_REQUEST => "Bad Request",
        StatusCode::UNAUTHORIZED => "Unauthorized",
        StatusCode::FORBIDDEN => "Forbidden",
        StatusCode::NOT_FOUND => "Not Found",
        StatusCode::METHOD_NOT_ALLOWED => "Method Not Allowed",
        StatusCode::PAYLOAD_TOO_LARGE => "Payload Too Large",
        StatusCode::BAD_GATEWAY => "Bad Gateway",
        StatusCode::INTERNAL_SERVER_ERROR => "Internal Server Error",
        _ => "Status",
    }
}
fn normalize_hex_lower(value: &str, flag: &str, byte_len: usize) -> Result<String> {
    let trimmed = required_trimmed_text(value, flag)?;
    let hex_value = trimmed.strip_prefix("0x").unwrap_or(&trimmed);
    let bytes = decode(hex_value)
        .wrap_err_with(|| format!("{flag} must be a {byte_len}-byte hex string"))?;
    if bytes.len() != byte_len {
        return Err(eyre!("{flag} must be a {byte_len}-byte hex string"));
    }
    Ok(encode(bytes))
}
fn normalize_hex_16_lower(value: &str, flag: &str) -> Result<String> {
    normalize_hex_lower(value, flag, 16)
}
fn parse_xor_quantity(input: &str) -> Result<XorQuantity> {
    parse_xor_quantity_labeled(input, "reserve balance")
}
fn parse_xor_quantity_labeled(input: &str, label: &str) -> Result<XorQuantity> {
    if input.is_empty() {
        return Err(eyre!("{label} must not be empty"));
    }
    let amount = input
        .parse::<XorQuantity>()
        .wrap_err_with(|| format!("failed to parse {label} as a canonical XOR quantity"))?;
    let canonical = amount.to_string();
    if canonical != input {
        return Err(eyre!(
            "{label} must use the canonical XOR decimal `{canonical}`"
        ));
    }
    Ok(amount)
}
fn load_reserve_policy_from_paths(
    json_path: Option<&Path>,
    norito_path: Option<&Path>,
) -> Result<(ReservePolicyV1, String)> {
    match (json_path, norito_path) {
        (Some(_), Some(_)) => Err(eyre!(
            "only one of --policy-json or --policy-norito may be supplied"
        )),
        (Some(path), None) => {
            let contents = fs::read_to_string(path).wrap_err_with(|| {
                format!("failed to read reserve policy JSON `{}`", path.display())
            })?;
            let policy: ReservePolicyV1 = norito::json::from_str(&contents)
                .wrap_err("failed to parse reserve policy JSON")?;
            Ok((policy, format!("policy JSON `{}`", path.display())))
        }
        (None, Some(path)) => {
            let bytes = fs::read(path).wrap_err_with(|| {
                format!("failed to read reserve policy Norito `{}`", path.display())
            })?;
            let policy = decode_from_bytes::<ReservePolicyV1>(&bytes)
                .wrap_err("failed to decode reserve policy Norito bytes")?;
            Ok((policy, format!("policy Norito `{}`", path.display())))
        }
        (None, None) => Ok((
            ReservePolicyV1::default(),
            "embedded default policy".to_string(),
        )),
    }
}
#[allow(clippy::too_many_arguments)]
fn build_reserve_quote_value(
    policy: &ReservePolicyV1,
    storage_class: StorageClass,
    tier: ReserveTier,
    duration: ReserveDuration,
    capacity_gib: u64,
    reserve_balance: &XorQuantity,
    quote: &ReserveQuote,
    policy_source: &str,
) -> Result<Value> {
    let mut root = Map::new();
    root.insert(
        "policy_source".into(),
        Value::from(policy_source.to_string()),
    );
    let mut inputs = Map::new();
    inputs.insert(
        "storage_class".into(),
        Value::from(storage_class_label(storage_class)),
    );
    inputs.insert("tier".into(), Value::from(reserve_tier_label(tier)));
    inputs.insert(
        "duration".into(),
        Value::from(reserve_duration_label(duration)),
    );
    inputs.insert(
        "capacity_gib".into(),
        Value::Number(Number::from(capacity_gib)),
    );
    let reserve_value =
        norito::json::to_value(reserve_balance).wrap_err("serialize reserve balance to JSON")?;
    inputs.insert("reserve_balance".into(), reserve_value);
    root.insert("inputs".into(), Value::Object(inputs));
    let policy_value =
        norito::json::to_value(policy).wrap_err("serialize reserve policy to JSON")?;
    root.insert("policy".into(), policy_value);
    let quote_value = norito::json::to_value(quote).wrap_err("serialize reserve quote to JSON")?;
    root.insert("quote".into(), quote_value);
    let projection = quote
        .ledger_projection()
        .wrap_err("failed to compute reserve ledger projection")?;
    let projection_value = norito::json::to_value(&projection)
        .wrap_err("serialize reserve ledger projection to JSON")?;
    root.insert("ledger_projection".into(), projection_value);
    Ok(Value::Object(root))
}
fn write_reserve_quote_artifact(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!(
                "failed to create reserve quote artifact directory `{}`",
                parent.display()
            )
        })?;
    }
    let rendered =
        norito::json::to_json_pretty(value).wrap_err("failed to render reserve quote artifact")?;
    fs::write(path, rendered).wrap_err_with(|| {
        format!(
            "failed to write reserve quote artifact `{}`",
            path.display()
        )
    })
}
#[derive(Clone)]
struct LedgerProjectionAmounts {
    rent_due: XorQuantity,
    reserve_shortfall: XorQuantity,
    top_up_shortfall: XorQuantity,
}
fn extract_ledger_projection(value: &Value) -> Result<LedgerProjectionAmounts> {
    let root = value
        .as_object()
        .ok_or_else(|| eyre!("reserve quote must be a JSON object"))?;
    let ledger_value = root
        .get("ledger_projection")
        .ok_or_else(|| eyre!("reserve quote missing `ledger_projection` block"))?;
    let projection: ReserveLedgerProjection = norito::json::from_value(ledger_value.clone())
        .wrap_err("failed to parse reserve ledger projection from quote")?;
    Ok(LedgerProjectionAmounts {
        rent_due: projection.rent_due,
        reserve_shortfall: projection.reserve_shortfall,
        top_up_shortfall: projection.top_up_shortfall,
    })
}
fn extract_reserve_quote(value: &Value) -> Result<ReserveQuote> {
    let root = value
        .as_object()
        .ok_or_else(|| eyre!("reserve quote must be a JSON object"))?;
    let quote_value = root
        .get("quote")
        .ok_or_else(|| eyre!("reserve quote missing `quote` block"))?;
    norito::json::from_value(quote_value.clone())
        .wrap_err("failed to parse reserve quote from quote artifact")
}
fn build_reserve_ledger_plan(
    quote_path: &Path,
    projection: LedgerProjectionAmounts,
    provider: &AccountId,
    treasury: &AccountId,
    reserve: &AccountId,
    asset_definition: &AssetDefinitionId,
) -> Result<Value> {
    let mut instructions = Vec::new();
    append_transfer_instruction(
        &mut instructions,
        provider,
        treasury,
        &projection.rent_due,
        asset_definition,
    )?;
    append_transfer_instruction(
        &mut instructions,
        provider,
        reserve,
        &projection.reserve_shortfall,
        asset_definition,
    )?;
    let mut root = Map::new();
    root.insert(
        "quote_path".into(),
        Value::from(quote_path.display().to_string()),
    );
    root.insert("rent_due".into(), xor_quantity_value(&projection.rent_due));
    root.insert(
        "reserve_shortfall".into(),
        xor_quantity_value(&projection.reserve_shortfall),
    );
    root.insert(
        "top_up_shortfall".into(),
        xor_quantity_value(&projection.top_up_shortfall),
    );
    root.insert("instructions".into(), Value::Array(instructions));
    Ok(Value::Object(root))
}
fn build_reserve_lifecycle_value(
    quote_path: &Path,
    lifecycle: &ReserveLifecycleProjection,
) -> Result<Value> {
    let mut root = Map::new();
    root.insert(
        "quote_path".into(),
        Value::from(quote_path.display().to_string()),
    );
    root.insert(
        "stage".into(),
        Value::from(reserve_lifecycle_stage_label(lifecycle.stage)),
    );
    root.insert(
        "days_past_due".into(),
        Value::Number(Number::from(u64::from(lifecycle.days_past_due))),
    );
    root.insert(
        "grace_period_days".into(),
        Value::Number(Number::from(u64::from(lifecycle.grace_period_days))),
    );
    root.insert(
        "default_after_days".into(),
        Value::Number(Number::from(u64::from(lifecycle.default_after_days))),
    );
    root.insert("rent_due".into(), xor_quantity_value(&lifecycle.rent_due));
    root.insert(
        "reserve_shortfall".into(),
        xor_quantity_value(&lifecycle.reserve_shortfall),
    );
    root.insert(
        "top_up_shortfall".into(),
        xor_quantity_value(&lifecycle.top_up_shortfall),
    );
    root.insert(
        "credit_draw".into(),
        xor_quantity_value(&lifecycle.credit_draw),
    );
    let available = lifecycle
        .credit_available_after_draw
        .as_ref()
        .map_or(Value::Null, xor_quantity_value);
    root.insert("credit_available_after_draw".into(), available);
    root.insert(
        "credit_shortfall".into(),
        xor_quantity_value(&lifecycle.credit_shortfall),
    );
    root.insert(
        "accrued_interest".into(),
        xor_quantity_value(&lifecycle.accrued_interest),
    );
    root.insert(
        "total_due_after_credit".into(),
        xor_quantity_value(&lifecycle.total_due_after_credit),
    );
    root.insert(
        "restrict_new_manifests".into(),
        Value::from(lifecycle.restrict_new_manifests),
    );
    root.insert(
        "disable_adverts".into(),
        Value::from(lifecycle.disable_adverts),
    );
    root.insert(
        "requires_governance_notification".into(),
        Value::from(lifecycle.requires_governance_notification),
    );
    root.insert(
        "requires_manual_credit_approval".into(),
        Value::from(lifecycle.requires_manual_credit_approval),
    );
    let projection = norito::json::to_value(lifecycle)
        .wrap_err("failed to serialize reserve lifecycle projection")?;
    root.insert("lifecycle_projection".into(), projection);
    Ok(Value::Object(root))
}
fn append_transfer_instruction(
    instructions: &mut Vec<Value>,
    source_account: &AccountId,
    destination_account: &AccountId,
    amount: &XorQuantity,
    asset_definition: &AssetDefinitionId,
) -> Result<()> {
    if amount.is_zero() {
        return Ok(());
    }
    let asset_id = AssetId::new(asset_definition.clone(), source_account.clone());
    let transfer = InstructionBox::from(Transfer::asset_quantity(
        asset_id,
        amount.as_quantity().clone(),
        destination_account.clone(),
    ));
    let value = norito::json::to_value(&transfer)
        .wrap_err("failed to serialize reserve ledger transfer instruction")?;
    instructions.push(value);
    Ok(())
}
fn xor_quantity_value(amount: &XorQuantity) -> Value {
    Value::String(amount.to_string())
}
const fn storage_class_label(class: StorageClass) -> &'static str {
    match class {
        StorageClass::Hot => "hot",
        StorageClass::Warm => "warm",
        StorageClass::Cold => "cold",
    }
}
const fn reserve_tier_label(tier: ReserveTier) -> &'static str {
    match tier {
        ReserveTier::TierA => "tier-a",
        ReserveTier::TierB => "tier-b",
        ReserveTier::TierC => "tier-c",
    }
}
const fn reserve_duration_label(duration: ReserveDuration) -> &'static str {
    match duration {
        ReserveDuration::Monthly => "monthly",
        ReserveDuration::Quarterly => "quarterly",
        ReserveDuration::Annual => "annual",
    }
}
const fn reserve_lifecycle_stage_label(stage: ReserveLifecycleStage) -> &'static str {
    match stage {
        ReserveLifecycleStage::Active => "active",
        ReserveLifecycleStage::Warning => "warning",
        ReserveLifecycleStage::Grace => "grace",
        ReserveLifecycleStage::Delinquent => "delinquent",
        ReserveLifecycleStage::Default => "default",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::CliOutputFormat;
    use blake3::Hasher as Blake3Hasher;
    use ed25519_dalek::{SigningKey, VerifyingKey as Ed25519VerifyingKey};
    use iroha::{
        config::{self, Config},
        crypto::KeyPair,
        data_model::{
            Metadata,
            prelude::{AccountId, ChainId},
        },
    };
    use iroha_config::{
        base::{read::ConfigReader, toml::TomlSource},
        parameters::user::Sorafs as UserSorafsConfig,
    };
    use iroha_crypto::{
        Algorithm, PublicKey,
        soranet::{
            certificate::{
                CapabilityToggle, RelayCapabilityFlagsV1, RelayCertificateV2, RelayEndpointV2,
                RelayRolesV2,
            },
            directory::{
                GuardDirectoryIssuerV1, GuardDirectoryRelayEntryV2, GuardDirectorySnapshotV2,
                compute_issuer_fingerprint,
            },
            handshake::HandshakeSuite,
            token::{
                self, AdmissionTokenVerifier, InMemoryTokenStore, TokenStore, TokenStoreLimits,
            },
        },
    };
    use iroha_data_model::{
        asset::{AssetDefinitionId, AssetId},
        isi::{InstructionBox, TransferBox},
        soranet::incentives::{
            RelayBondLedgerEntryV1, RelayBondPolicyV1, RelayComplianceStatusV1,
            RelayEpochMetricsV1, RelayRewardDisputeV1, RelayRewardInstructionV1,
        },
    };
    use iroha_i18n::{Bundle, Language, Localizer};
    use iroha_primitives::numeric::Quantity;
    use norito::json::{Map, Value};
    use norito::{decode_from_bytes, json::JsonSerialize, to_bytes};
    use rand::{
        RngCore, SeedableRng,
        rand_core::{TryCryptoRng, TryRngCore},
        rngs::StdRng,
    };
    use sorafs_manifest::{
        BLAKE3_256_MULTIHASH_CODE, ChunkingProfileV1, DagCodecId, ManifestBuilder, PinPolicy,
        ProfileId, StorageClass as ManifestStorageClass,
    };
    use sorafs_orchestrator::soranet::EndpointTag;
    use sorafs_orchestrator::{incentives::RewardConfig, treasury::ExpectedLedgerTransfer};
    use soranet_pq::{MlDsaSuite, generate_mldsa_keypair_from_os as generate_mldsa_keypair};
    use std::{
        fmt::{self, Display},
        fs,
        io::Write,
        path::Path,
        str::FromStr,
        sync::{Arc, Mutex},
        time::{Duration, SystemTime},
    };
    use tempfile::{NamedTempFile, TempDir};
    use url::Url;
    include!("sorafs_nonce_rng_tests.rs");
    test_items! {
        fn persisted_guard_cache_requires_a_key_and_enforces_the_file_bound() {
            let temporary = TempDir::new().expect("temporary directory");
            let cache = temporary.path().join("guards.norito");
            let missing_key = load_guard_set(&cache, None)
                .expect_err("configured cache without a key must fail closed");
            assert!(missing_key.to_string().contains("authentication key is required"));

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;

                fs::write(&cache, vec![0_u8; GUARD_CACHE_MAX_BYTES_V1 + 1])
                    .expect("write oversized guard cache");
                fs::set_permissions(&cache, fs::Permissions::from_mode(0o600))
                    .expect("make oversized cache owner-private");
                let key =
                    GuardCacheKey::from_bytes([0x6D; 32]).expect("non-zero guard cache key");
                let oversized = load_guard_set(&cache, Some(&key))
                    .expect_err("oversized guard cache must fail before decode");
                assert!(oversized.to_string().contains("must contain between 1 and"));
            }

            let unsigned_write = persist_guard_set(&cache, &GuardSet::new(Vec::new()), None)
                .expect_err("unsigned guard cache persistence must be unavailable");
            assert!(
                unsigned_write
                    .to_string()
                    .contains("authentication key is required")
            );
        }

        fn guard_cache_cli_requires_a_current_directory() {
            use clap::Parser as _;

            #[allow(dead_code)]
            #[derive(clap::Parser, Debug)]
            struct Parser {
                #[command(flatten)]
                fetch: FetchArgs,
            }

            let error = Parser::try_parse_from([
                "sorafs-fetch-test",
                "--manifest",
                "manifest.to",
                "--plan",
                "plan.json",
                "--manifest-id",
                "00",
                "--gateway-provider",
                "name=relay",
                "--guard-cache",
                "guards.norito",
                "--guard-cache-key-file",
                "guard-cache.key",
            ])
            .expect_err("a persisted guard cache without a current directory must be rejected");
            assert!(error.to_string().contains("--guard-directory"));

            let raw_key = "11".repeat(32);
            let raw_error = Parser::try_parse_from([
                "sorafs-fetch-test",
                "--manifest",
                "manifest.to",
                "--plan",
                "plan.json",
                "--manifest-id",
                "00",
                "--gateway-provider",
                "name=relay",
                "--guard-cache-key",
                raw_key.as_str(),
            ])
            .expect_err("raw guard-cache key material must not be accepted on argv");
            assert!(raw_error.to_string().contains("--guard-cache-key"));
        }

        #[cfg(unix)]
        fn guard_cache_key_file_is_exact_raw_and_owner_private() {
            use std::os::unix::fs::PermissionsExt as _;

            let temporary = TempDir::new().expect("temporary directory");
            let key_path = temporary.path().join("guard-cache.key");
            fs::write(&key_path, [0x6D; GuardCacheKey::LENGTH]).expect("write raw key");
            fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))
                .expect("make raw key owner-private");
            let loaded = load_guard_cache_key_file(&key_path).expect("load raw key");
            let guards = GuardSet::new(Vec::new());
            let encoded = guards
                .encode_authenticated(&loaded)
                .expect("authenticate cache with loaded key");
            let expected = GuardCacheKey::from_bytes([0x6D; GuardCacheKey::LENGTH])
                .expect("fixture key");
            GuardSet::decode_authenticated(&encoded, &expected)
                .expect("loaded key must contain the exact raw bytes");

            fs::write(&key_path, "6d".repeat(GuardCacheKey::LENGTH))
                .expect("replace with argv-style hex text");
            assert!(load_guard_cache_key_file(&key_path).is_err());
            fs::write(&key_path, [0x6D; GuardCacheKey::LENGTH]).expect("restore raw key");
            fs::set_permissions(&key_path, fs::Permissions::from_mode(0o644))
                .expect("make key permissive");
            assert!(load_guard_cache_key_file(&key_path).is_err());
        }

        fn ordinary_proxy_manifest_summary_never_serialises_the_client_capability() {
            let secret = "11".repeat(32);
            let manifest_json = format!(
                r#"{{"version":2,"authority":"127.0.0.1:9443","certificate_pem":"test","client_capability_hex":"{secret}"}}"#
            );
            let manifest: BrowserExtensionManifest = norito::json::from_str(&manifest_json)
                .expect("decode proxy manifest with bootstrap capability");
            let public = public_local_proxy_manifest_value(&manifest);
            let rendered = norito::json::to_json_pretty(&public)
                .expect("render public local-proxy manifest summary");
            assert!(!rendered.contains(&secret));
            assert!(!rendered.contains("client_capability_hex"));
        }

        #[cfg(unix)]
        fn authenticated_guard_cache_is_owner_private_and_atomically_replaceable() {
            use std::os::unix::fs::MetadataExt as _;

            let temporary = TempDir::new().expect("temporary directory");
            let cache = temporary.path().join("guards.norito");
            let key = GuardCacheKey::from_bytes([0x6D; 32]).expect("non-zero guard cache key");
            let guards = GuardSet::new(Vec::new());

            persist_guard_set(&cache, &guards, Some(&key)).expect("persist authenticated cache");
            persist_guard_set(&cache, &guards, Some(&key))
                .expect("atomically replace authenticated cache");
            let metadata = fs::symlink_metadata(&cache).expect("inspect persisted cache");
            assert!(metadata.is_file());
            assert_eq!(metadata.mode() & 0o077, 0);
            assert_eq!(metadata.nlink(), 1);
            let loaded = load_guard_set(&cache, Some(&key))
                .expect("load authenticated cache")
                .expect("cache exists");
            assert!(loaded.guards().is_empty());
            let names = fs::read_dir(temporary.path())
                .expect("list cache directory")
                .map(|entry| entry.expect("directory entry").file_name())
                .collect::<Vec<_>>();
            assert_eq!(names, [std::ffi::OsString::from("guards.norito")]);
        }

        #[cfg(unix)]
        fn guard_cache_rejects_symlink_hardlink_and_permissive_custody() {
            use std::os::unix::fs::{PermissionsExt as _, symlink};

            let temporary = TempDir::new().expect("temporary directory");
            let key = GuardCacheKey::from_bytes([0x6D; 32]).expect("non-zero guard cache key");
            let guards = GuardSet::new(Vec::new());
            let target = temporary.path().join("target.norito");
            fs::write(&target, b"unchanged").expect("write symlink target");
            fs::set_permissions(&target, fs::Permissions::from_mode(0o600))
                .expect("make target owner-private");
            let link = temporary.path().join("link.norito");
            symlink(&target, &link).expect("create cache symlink");
            let error = persist_guard_set(&link, &guards, Some(&key))
                .expect_err("cache symlink must fail closed");
            assert!(error.to_string().contains("owner-private"));
            assert_eq!(fs::read(&target).expect("read target"), b"unchanged");
            let error = load_guard_set(&link, Some(&key))
                .expect_err("cache symlink load must fail closed");
            assert!(error.to_string().contains("direct owner-private"));

            let hardlink = temporary.path().join("hardlink.norito");
            fs::hard_link(&target, &hardlink).expect("create cache hardlink");
            let error = load_guard_set(&target, Some(&key))
                .expect_err("multiply linked cache must fail closed");
            assert!(error.to_string().contains("exactly one link"));

            fs::remove_file(&hardlink).expect("remove hardlink");
            fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
                .expect("make target permissive");
            let error = persist_guard_set(&target, &guards, Some(&key))
                .expect_err("permissive cache must fail closed");
            assert!(error.to_string().contains("owner-private"));
        }

        #[cfg(unix)]
        fn guard_cache_rejects_writable_parent_directory() {
            use std::os::unix::fs::PermissionsExt as _;

            let temporary = TempDir::new().expect("temporary directory");
            fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o777))
                .expect("make cache parent permissive");
            let cache = temporary.path().join("guards.norito");
            let key = GuardCacheKey::from_bytes([0x6D; 32]).expect("non-zero guard cache key");
            let error = persist_guard_set(&cache, &GuardSet::new(Vec::new()), Some(&key))
                .expect_err("writable cache parent must fail closed");
            assert!(error.to_string().contains("writable by another principal"));
            fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o700))
                .expect("restore cache parent custody");
        }

        fn hedging_billing_subcommands_parse_all_read_and_ack_routes() {
            use clap::Parser as _;
            #[derive(clap::Parser)]
            struct Parser {
                #[command(subcommand)]
                command: Command,
            }
            let checkpoint = "11".repeat(32);
            let statement_id = "22".repeat(32);
            let request_nonce = "33".repeat(32);
            let commands = [
                vec![
                    "sorafs-test".to_owned(),
                    "billing".to_owned(),
                    "status".to_owned(),
                ],
                vec![
                    "sorafs-test".to_owned(),
                    "billing".to_owned(),
                    "statements".to_owned(),
                    "--expected-checkpoint-fingerprint".to_owned(),
                    checkpoint.clone(),
                    "--limit".to_owned(),
                    "10".to_owned(),
                ],
                vec![
                    "sorafs-test".to_owned(),
                    "billing".to_owned(),
                    "statement".to_owned(),
                    "--statement-id".to_owned(),
                    statement_id.clone(),
                    "--expected-checkpoint-fingerprint".to_owned(),
                    checkpoint.clone(),
                    "--output".to_owned(),
                    "statement.norito".to_owned(),
                ],
                vec![
                    "sorafs-test".to_owned(),
                    "billing".to_owned(),
                    "acknowledge".to_owned(),
                    "--statement-id".to_owned(),
                    statement_id,
                    "--expected-checkpoint-fingerprint".to_owned(),
                    checkpoint.clone(),
                    "--request-nonce".to_owned(),
                    request_nonce,
                    "--authentication-proof".to_owned(),
                    "proof.bin".to_owned(),
                ],
                vec![
                    "sorafs-test".to_owned(),
                    "billing".to_owned(),
                    "reconciliation".to_owned(),
                ],
                vec![
                    "sorafs-test".to_owned(),
                    "hedging".to_owned(),
                    "exposure".to_owned(),
                    "--expected-checkpoint-fingerprint".to_owned(),
                    checkpoint.clone(),
                    "--limit".to_owned(),
                    "10".to_owned(),
                ],
                vec![
                    "sorafs-test".to_owned(),
                    "hedging".to_owned(),
                    "intents".to_owned(),
                    "--expected-checkpoint-fingerprint".to_owned(),
                    checkpoint,
                    "--limit".to_owned(),
                    "10".to_owned(),
                ],
            ];
            for command in commands {
                let parsed = Parser::try_parse_from(command).expect("hedging/billing command parses");
                let _ = parsed.command;
            }
        }
        fn billing_statements_cli_builds_exact_checkpoint_filter() {
            let checkpoint = "11".repeat(32);
            let after_statement_id = "22".repeat(32);
            let args = BillingStatementsArgs {
                expected_checkpoint_fingerprint: checkpoint.clone(),
                after_statement_id: Some(after_statement_id.clone()),
                limit: 25,
            };
            let mut context = TestContext::new();
            args.run_with(&mut context, |_client, filter| {
                assert_eq_compact! { filter.expected_checkpoint_fingerprint_hex => checkpoint.as_str() };
                assert_eq_compact! { filter.after_statement_id_hex => Some(after_statement_id.as_str()) };
                assert_eq!(filter.limit, 25);
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "anchor": {"checkpoint_fingerprint": (checkpoint.to_ascii_uppercase())},
                    }), "billing statement page response")
            })
            .expect("billing statement list succeeds");
            assert_eq!(context.printed.len(), 1);
            assert!(context.printed[0].contains("\"anchor\""));
        }
        fn hedging_projection_cli_builds_read_only_exact_checkpoint_filter() {
            let checkpoint = "33".repeat(32);
            let after = "44".repeat(32);
            let args = HedgingProjectionArgs {
                expected_checkpoint_fingerprint: checkpoint.clone(),
                after: Some(after.clone()),
                limit: 100,
            };
            let mut context = TestContext::new();
            args.run_with(&mut context, |_client, filter| {
                assert_eq_compact! { filter.expected_checkpoint_fingerprint_hex => checkpoint.as_str() };
                assert_eq!(filter.after_hex, Some(after.as_str()));
                assert_eq!(filter.limit, 100);
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "anchor": {"checkpoint_fingerprint": (checkpoint.to_ascii_uppercase())},
                        "automatic_execution_enabled": false,
                    }), "hedging projection response")
            })
            .expect("hedging projection read succeeds");
            assert_eq!(context.printed.len(), 1);
            let output: Value =
                norito::json::from_str(&context.printed[0]).expect("projection output JSON");
            assert_eq_compact! { output.get("automatic_execution_enabled").and_then(Value::as_bool) => Some(false); "projection output must preserve the disabled execution claim" };
        }
        }
    include!("sorafs/hedging_billing_response_tests.rs");
    #[test]
    fn billing_acknowledgement_cli_reads_bounded_binary_proof() {
        let checkpoint = "55".repeat(32);
        let statement_id = "66".repeat(32);
        let request_nonce = "77".repeat(32);
        let mut proof_file = NamedTempFile::new().expect("proof file");
        proof_file
            .write_all(&[0xA5; 48])
            .expect("write authentication proof");
        let args = BillingAcknowledgeArgs {
            statement_id: statement_id.clone(),
            expected_checkpoint_fingerprint: checkpoint.clone(),
            request_nonce: request_nonce.clone(),
            authentication_proof: proof_file.path().to_path_buf(),
        };
        let expected =
            SorafsBillingAcknowledgementProof::try_from_hex(&request_nonce, vec![0xA5; 48])
                .expect("expected proof");
        let mut context = TestContext::new();
        args.run_with(
            &mut context,
            |_client, actual_statement_id, actual_checkpoint, proof| {
                assert_eq!(actual_statement_id, statement_id);
                assert_eq!(actual_checkpoint, checkpoint);
                assert_eq!(proof, &expected);
                assert_compact! { format!("{proof:?}").contains("[REDACTED]"); "proof debug output must not expose authentication bytes" };
json_response_fixture!(StatusCode::OK, &norito::json!({
                        "acknowledged": true
                    }), "billing acknowledgement response")
            },
        )
        .expect("billing acknowledgement succeeds");
        assert_eq!(context.printed.len(), 1);
        let output: Value =
            norito::json::from_str(&context.printed[0]).expect("acknowledgement output JSON");
        assert_eq_compact! { output.get("acknowledged").and_then(Value::as_bool) => Some(true) };
    }
    #[test]
    fn hedging_billing_cli_rejects_non_regular_and_oversized_proofs() {
        let proof_dir = TempDir::new().expect("proof directory");
        let error = read_billing_acknowledgement_proof(proof_dir.path())
            .expect_err("directory proof must fail closed");
        assert!(error.to_string().contains("regular non-symlink file"));
        let oversized = proof_dir.path().join("oversized-proof.bin");
        fs::write(
            &oversized,
            vec![0xA5; SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1 + 1],
        )
        .expect("write oversized proof");
        let error = read_billing_acknowledgement_proof(&oversized)
            .expect_err("oversized proof must fail closed");
        assert!(error.to_string().contains("must contain between 1 and"));
    }
    #[cfg(any(unix, windows))]
    #[test]
    fn hedging_billing_cli_rejects_multiply_linked_proof() {
        let proof_dir = TempDir::new().expect("proof directory");
        let target = proof_dir.path().join("proof-target.bin");
        let alias = proof_dir.path().join("proof-alias.bin");
        fs::write(&target, [0xA5; 32]).expect("write proof target");
        fs::hard_link(&target, &alias).expect("create proof hard link");
        let error = read_billing_acknowledgement_proof(&target)
            .expect_err("multiply linked proof must fail closed");
        assert!(error.to_string().contains("stable single-link identity"));
    }
    #[cfg(unix)]
    #[test]
    fn hedging_billing_cli_rejects_symlink_proof() {
        use std::os::unix::fs::symlink;
        let proof_dir = TempDir::new().expect("proof directory");
        let target = proof_dir.path().join("proof-target.bin");
        let link = proof_dir.path().join("proof-link.bin");
        fs::write(&target, [0xA5; 32]).expect("write proof target");
        symlink(&target, &link).expect("create proof symlink");
        let error =
            read_billing_acknowledgement_proof(&link).expect_err("symlink proof must fail closed");
        assert!(error.to_string().contains("regular non-symlink file"));
    }
    test_items! {
    fn billing_proof_reader_retains_windows_direct_identity_guards() {
        let source = include_str!("sorafs.rs");
        for required_guard in [
            "FILE_FLAG_OPEN_REPARSE_POINT",
            "metadata.volume_serial_number()",
            "metadata.file_index()",
            "left.file_size() == right.file_size()",
            "left.last_write_time() == right.last_write_time()",
            "left.creation_time() == right.creation_time()",
            "billing_proof_metadata_unchanged(&path_metadata, &opened_metadata)",
            "billing_proof_metadata_unchanged(&opened_metadata, &after_file_metadata)",
            "billing_proof_metadata_unchanged(&opened_metadata, &after_path_metadata)",
            "this platform does not expose a stable direct-file identity",
        ] {
            assert_compact! { source.contains(required_guard); "billing proof reader lost required direct-file guard `{required_guard}`" };
        }
    }
    fn hedging_billing_proof_exact_read_detects_length_drift() {
        let path = Path::new("drifting-proof.bin");
        let mut truncated = std::io::Cursor::new(vec![0xA5; 3]);
        let error = read_billing_acknowledgement_proof_exact(path, &mut truncated, 4)
            .expect_err("truncated proof must fail closed");
        assert!(error.to_string().contains("changed length"));
        let mut extended = std::io::Cursor::new(vec![0xA5; 5]);
        let error = read_billing_acknowledgement_proof_exact(path, &mut extended, 4)
            .expect_err("extended proof must fail closed");
        assert!(error.to_string().contains("changed length"));
    }
    fn billing_statement_cli_writes_exact_norito_response() {
        let checkpoint = "88".repeat(32);
        let statement_id = "99".repeat(32);
        let output_dir = TempDir::new().expect("statement output directory");
        let output = output_dir.path().join("statement.norito");
        let args = BillingStatementArgs {
            statement_id: statement_id.clone(),
            expected_checkpoint_fingerprint: checkpoint.clone(),
            output: output.clone(),
        };
        let expected_bytes = vec![0x4E, 0x52, 0x54, 0x31];
        let mut context = TestContext::new();
        args.run_with(&mut context, |_client, actual_id, actual_checkpoint| {
            assert_eq!(actual_id, statement_id);
            assert_eq!(actual_checkpoint, checkpoint);
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/x-norito")
                .body(expected_bytes.clone())
                .expect("published statement response"))
        })
        .expect("published statement write succeeds");
        assert_eq_compact! { fs::read(output).expect("read written statement") => expected_bytes };
        assert_eq!(context.printed.len(), 1);
        let summary: Value =
            norito::json::from_str(&context.printed[0]).expect("statement summary JSON");
        assert_eq_compact! { summary.get("bytes_written").and_then(Value::as_u64) => Some(4) };
    }
    fn billing_statement_cli_refuses_to_clobber_existing_file() {
        let output_dir = TempDir::new().expect("statement output directory");
        let output = output_dir.path().join("statement.norito");
        let original = b"existing-statement".to_vec();
        fs::write(&output, &original).expect("write existing statement");
        let args = BillingStatementArgs {
            statement_id: "99".repeat(32),
            expected_checkpoint_fingerprint: "88".repeat(32),
            output: output.clone(),
        };
        let mut context = TestContext::new();
        let error = args
            .run_with(&mut context, |_client, _statement_id, _checkpoint| {
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/x-norito")
                    .body(vec![0x4E, 0x52, 0x54, 0x31])
                    .expect("published statement response"))
            })
            .expect_err("existing output must fail closed");
        assert!(error.to_string().contains("without replacing"));
        assert_eq_compact! { fs::read(&output).expect("read preserved statement") => original };
        assert!(context.printed.is_empty());
    }
    }
    #[cfg(unix)]
    #[test]
    fn billing_statement_cli_refuses_to_follow_output_symlink() {
        use std::os::unix::fs::symlink;
        let output_dir = TempDir::new().expect("statement output directory");
        let target = output_dir.path().join("target.norito");
        let output = output_dir.path().join("statement.norito");
        let original = b"target-statement".to_vec();
        fs::write(&target, &original).expect("write statement target");
        symlink(&target, &output).expect("create output symlink");
        let args = BillingStatementArgs {
            statement_id: "99".repeat(32),
            expected_checkpoint_fingerprint: "88".repeat(32),
            output: output.clone(),
        };
        let mut context = TestContext::new();
        let error = args
            .run_with(&mut context, |_client, _statement_id, _checkpoint| {
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/x-norito")
                    .body(vec![0x4E, 0x52, 0x54, 0x31])
                    .expect("published statement response"))
            })
            .expect_err("symlink output must fail closed");
        assert!(error.to_string().contains("without replacing"));
        assert_eq_compact! { fs::read(&target).expect("read preserved target statement") => original };
        assert_compact! { fs::symlink_metadata(&output).expect("inspect preserved output symlink").file_type().is_symlink() };
        assert!(context.printed.is_empty());
    }
    test_items! {
    fn billing_statement_cli_rejects_substituted_media_type() {
        let output_dir = TempDir::new().expect("statement output directory");
        let output = output_dir.path().join("statement.norito");
        let args = BillingStatementArgs {
            statement_id: "99".repeat(32),
            expected_checkpoint_fingerprint: "88".repeat(32),
            output: output.clone(),
        };
        let mut context = TestContext::new();
        let error = args
            .run_with(&mut context, |_client, _statement_id, _checkpoint| {
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(br#"{"substituted":true}"#.to_vec())
                    .expect("substituted statement response"))
            })
            .expect_err("non-Norito response must fail closed");
        assert!(error.to_string().contains("application/x-norito"));
        assert_compact! { !output.exists(); "substituted response must not be persisted" };
        assert!(context.printed.is_empty());
    }
    fn hedging_billing_cli_rejects_aliases_and_invalid_bounds_before_http() {
        let mut context = TestContext::new();
        let uppercase = "AA".repeat(32);
        let list = BillingStatementsArgs {
            expected_checkpoint_fingerprint: uppercase,
            after_statement_id: None,
            limit: 1,
        };
        let error = list
            .run_with(&mut context, |_client, _filter| {
                unreachable!("invalid checkpoint must fail before HTTP")
            })
            .expect_err("uppercase checkpoint rejected");
        assert!(error.to_string().contains("lowercase hexadecimal"));
        let projection = HedgingProjectionArgs {
            expected_checkpoint_fingerprint: "11".repeat(32),
            after: None,
            limit: SORAFS_HEDGING_BILLING_MAX_PAGE_ITEMS_V1 + 1,
        };
        let error = projection
            .run_with(&mut context, |_client, _filter| {
                unreachable!("invalid limit must fail before HTTP")
            })
            .expect_err("out-of-range limit rejected");
        assert!(error.to_string().contains("--limit"));
        let zero_nonce = SorafsBillingAcknowledgementProof::try_from_hex(&"00".repeat(32), vec![1])
            .expect_err("zero nonce rejected");
        assert!(zero_nonce.to_string().contains("request nonce"));
        assert!(context.printed.is_empty());
    }
    fn token_issue_rng_reports_os_seed_failure() {
        let mut rng = FailingSorafsCliNonceRng;
        let error = token_issue_rng_from_rng(&mut rng)
            .expect_err("token RNG seeding should fail when entropy fails");
        let message = format!("{error:?}");
        assert!(message.contains("failed to seed SoraNet admission-token RNG"));
        assert!(message.contains("failing SoraFS CLI nonce RNG"));
    }
    fn parse_xor_quantity_accepts_canonical_sub_micro_and_wide_inputs() {
        for canonical in [
            "12.3456",
            "0.000000001",
            "340282366920938463463374607431768211456.000000001",
        ] {
            let amount = parse_xor_quantity(canonical).expect("canonical quantity parses");
            assert_eq!(amount.to_string(), canonical);
        }
    }
    fn parse_xor_quantity_rejects_noncanonical_negative_and_over_scale_inputs() {
        for invalid in [
            "",
            " 1",
            "1 ",
            "+1",
            "01",
            "1.0",
            ".5",
            "1.",
            "-1",
            "0.0000000001",
        ] {
            assert_compact! { parse_xor_quantity(invalid).is_err(); "invalid XOR quantity must be rejected: {invalid:?}" };
        }
    }
    fn reserve_quote_builder_renders_inputs() {
        let policy = ReservePolicyV1::default();
        let quote = policy
            .quote(
                super::StorageClass::Hot,
                4,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                XorQuantity::zero(),
            )
            .expect("quote");
        let value = build_reserve_quote_value(
            &policy,
            super::StorageClass::Hot,
            ReserveTier::TierA,
            ReserveDuration::Monthly,
            4,
            &XorQuantity::zero(),
            &quote,
            "test policy",
        )
        .expect("build");
        let root = value
            .as_object()
            .expect("quote payload should be a JSON object");
        assert_eq_compact! { root.get("policy_source").and_then(Value::as_str) => Some("test policy") };
        let inputs = root
            .get("inputs")
            .and_then(Value::as_object)
            .expect("inputs object");
        assert_eq_compact! { inputs.get("storage_class").and_then(Value::as_str) => Some("hot") };
        assert_eq!(inputs.get("capacity_gib").and_then(Value::as_u64), Some(4));
        let quote_value = root.get("quote").expect("quote field exists");
        assert_compact! { quote_value.get("monthly_rent").is_some(); "quote field should carry rent breakdown: {quote_value:?}" };
        let ledger_projection = root
            .get("ledger_projection")
            .and_then(Value::as_object)
            .expect("ledger projection should be serialized");
        assert_compact! { ledger_projection.contains_key("rent_due"); "ledger projection exposes rent_due amount: {ledger_projection:?}" };
    }
    fn reserve_ledger_projection_rejects_non_string_and_noncanonical_quantities() {
        let policy = ReservePolicyV1::default();
        let reserve_balance = XorQuantity::zero();
        let quote = policy
            .quote(
                super::StorageClass::Hot,
                4,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                reserve_balance.clone(),
            )
            .expect("quote");
        let valid = build_reserve_quote_value(
            &policy,
            super::StorageClass::Hot,
            ReserveTier::TierA,
            ReserveDuration::Monthly,
            4,
            &reserve_balance,
            &quote,
            "test policy",
        )
        .expect("quote artifact");
        for invalid in [
            Value::Number(Number::from(1_u64)),
            Value::String("+1".into()),
            Value::String("01".into()),
            Value::String("1.0".into()),
            Value::String("-1".into()),
            Value::String("0.0000000001".into()),
        ] {
            let mut artifact = valid.clone();
            artifact
                .as_object_mut()
                .expect("quote object")
                .get_mut("ledger_projection")
                .expect("ledger projection")
                .as_object_mut()
                .expect("ledger object")
                .insert("rent_due".into(), invalid.clone());
            assert_compact! { extract_ledger_projection(&artifact).is_err(); "invalid exact quantity must be rejected: {invalid:?}" };
        }
    }
    fn reserve_ledger_plan_preserves_sub_micro_and_wide_quantities() {
        let sub_micro: XorQuantity = "0.000000001".parse().expect("sub-micro quantity");
        let wide: XorQuantity = "340282366920938463463374607431768211456.000000001"
            .parse()
            .expect("wide quantity");
        let projection = LedgerProjectionAmounts {
            rent_due: sub_micro.clone(),
            reserve_shortfall: wide.clone(),
            top_up_shortfall: XorQuantity::zero(),
        };
        let provider = sample_account_id("reserve-ledger-provider");
        let treasury = sample_account_id("reserve-ledger-treasury");
        let reserve = sample_account_id("reserve-ledger-escrow");
        let plan = build_reserve_ledger_plan(
            Path::new("quote.json"),
            projection,
            &provider,
            &treasury,
            &reserve,
            &xor_asset_id(),
        )
        .expect("exact reserve ledger plan");
        let root = plan.as_object().expect("ledger plan object");
        assert_eq_compact! { root.get("rent_due").and_then(Value::as_str) => Some(sub_micro.to_string().as_str()) };
        assert_eq_compact! { root.get("reserve_shortfall").and_then(Value::as_str) => Some(wide.to_string().as_str()) };
        assert!(!root.contains_key("rent_due_micro_xor"));
        assert_eq_compact! { root.get("instructions").and_then(Value::as_array).map(Vec::len) => Some(2) };
        let rendered = norito::json::to_json(&plan).expect("ledger plan JSON");
        assert!(rendered.contains(&sub_micro.to_string()));
        assert!(rendered.contains(&wide.to_string()));
    }
    fn reserve_lifecycle_builder_renders_stage_and_credit_fields() {
        let policy = ReservePolicyV1::default();
        let quote = policy
            .quote(
                super::StorageClass::Hot,
                10,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                XorQuantity::zero(),
            )
            .expect("quote");
        let lifecycle = quote
            .lifecycle_projection(3, 7, 30)
            .expect("lifecycle projection");
        let value = build_reserve_lifecycle_value(Path::new("quote.json"), &lifecycle)
            .expect("build lifecycle JSON");
        let root = value
            .as_object()
            .expect("lifecycle payload should be a JSON object");
        assert_eq!(root.get("stage").and_then(Value::as_str), Some("grace"));
        assert_eq!(root.get("credit_draw").and_then(Value::as_str), Some("120"));
        assert_eq_compact! { root.get("disable_adverts").and_then(Value::as_bool) => Some(false) };
        assert_compact! { root.get("lifecycle_projection").is_some(); "full projection should be embedded" };
    }
    }
    fn sample_guard_directory_signing_key() -> SigningKey {
        let mut rng = StdRng::seed_from_u64(0x5EED);
        let mut ed_seed = [0u8; 32];
        rng.fill_bytes(&mut ed_seed);
        SigningKey::from_bytes(&ed_seed)
    }
    fn sample_guard_directory_snapshot_bytes() -> Vec<u8> {
        let signing_key = sample_guard_directory_signing_key();
        let ed_public = Ed25519VerifyingKey::from(&signing_key).to_bytes();
        let mldsa_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");
        let mldsa_public = mldsa_keys.public_key().to_vec();
        let fingerprint = compute_issuer_fingerprint(&ed_public, &mldsa_public)
            .expect("sample issuer fingerprint should compute");
        let directory_hash = [0xAB; 32];
        let certificate = RelayCertificateV2 {
            relay_id: ed_public,
            identity_ed25519: ed_public,
            identity_mldsa65: vec![0x44; 1952],
            descriptor_commit: [0x22; 32],
            roles: RelayRolesV2 {
                entry: true,
                middle: false,
                exit: false,
            },
            guard_weight: 12,
            bandwidth_bytes_per_sec: 1_500_000,
            reputation_weight: 80,
            endpoints: vec![RelayEndpointV2 {
                quic_multiaddr: "/dns/pq.guard/udp/443/quic".to_string(),
                tls_server_name: "pq.guard".to_string(),
                tls_spki_sha256: [0xA5; 32],
                priority: 0,
                tags: vec![EndpointTag::NoritoStream.as_label().to_string()],
            }],
            capability_flags: RelayCapabilityFlagsV1::new(
                CapabilityToggle::Enabled,
                CapabilityToggle::Disabled,
                CapabilityToggle::Enabled,
                CapabilityToggle::Disabled,
            ),
            handshake_suites: vec![
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            published_at: 1_734_000_000,
            valid_after: 1_734_000_000,
            valid_until: 1_734_086_400,
            directory_hash,
            issuer_fingerprint: fingerprint,
        };
        let published_at = certificate.published_at;
        let valid_after = certificate.valid_after;
        let valid_until = certificate.valid_until;
        let bundle = certificate
            .issue(&signing_key, mldsa_keys.secret_key())
            .expect("issue certificate");
        let snapshot = GuardDirectorySnapshotV2 {
            version: 2,
            directory_hash,
            published_at_unix: published_at,
            valid_after_unix: valid_after,
            valid_until_unix: valid_until,
            issuers: vec![GuardDirectoryIssuerV1 {
                fingerprint,
                ed25519_public: ed_public,
                mldsa65_public: mldsa_public,
            }],
            relays: vec![GuardDirectoryRelayEntryV2 {
                certificate: bundle
                    .try_to_cbor()
                    .expect("sample relay bundle should encode"),
            }],
        };
        to_bytes(&snapshot).expect("encode snapshot")
    }
    pub(super) struct TestContext {
        cfg: Config,
        printed: Vec<String>,
        i18n: Localizer,
        output_format: CliOutputFormat,
    }
    impl TestContext {
        pub(super) fn new() -> Self {
            Self::with_output_format(CliOutputFormat::Json)
        }
        pub(super) fn with_output_format(output_format: CliOutputFormat) -> Self {
            let kp = checked_sorafs_ed25519_key_fixture();
            let account = AccountId::new(kp.public_key().clone());
            let cfg = Config {
                chain: ChainId::from("test-chain"),
                network_id: iroha::data_model::NetworkId::from_genesis_hash(
                    iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
                        b"iroha-cli-sorafs-test-genesis",
                    )),
                ),
                account,
                account_chain_discriminant:
                    iroha_config::parameters::defaults::common::chain_discriminant(),
                key_pair: kp,
                basic_auth: None,
                torii_api_url: Url::parse("http://localhost/").unwrap(),
                torii_request_timeout: config::DEFAULT_TORII_REQUEST_TIMEOUT,
                transaction_ttl: config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
                transaction_status_timeout: config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
                transaction_add_nonce: config::DEFAULT_TRANSACTION_NONCE,
                connect_queue_root: config::default_connect_queue_root(),
                soracloud_http_witness_file: None,
                sorafs_alias_cache: crate::config_utils::default_alias_cache_policy(),
                sorafs_anonymity_policy: crate::config_utils::default_anonymity_policy(),
                sorafs_rollout_phase: crate::config_utils::default_rollout_phase(),
            };
            Self {
                cfg,
                printed: Vec::new(),
                i18n: Localizer::new(Bundle::Cli, Language::English),
                output_format,
            }
        }
        pub(super) fn outputs(&self) -> &[String] {
            &self.printed
        }
    }
    fn checked_sorafs_ed25519_key_fixture() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .expect("generate checked SoraFS fixture key")
    }
    #[test]
    fn sorafs_fixture_uses_checked_ed25519_key_generation() {
        let key_pair = checked_sorafs_ed25519_key_fixture();
        let actual = key_pair
            .public_key()
            .try_algorithm()
            .expect("SoraFS fixture key advertises a valid algorithm");
        assert_eq!(actual, Algorithm::Ed25519);
    }
    struct OutputModeContext {
        config: Config,
        output_format: CliOutputFormat,
        printed: Vec<String>,
        i18n: Localizer,
    }
    impl OutputModeContext {
        fn new(output_format: CliOutputFormat) -> Self {
            Self {
                config: crate::fallback_config(),
                output_format,
                printed: Vec::new(),
                i18n: Localizer::new(Bundle::Cli, Language::English),
            }
        }
    }
    impl RunContext for OutputModeContext {
        fn config(&self) -> &Config {
            &self.config
        }
        fn transaction_metadata(&self) -> Option<&Metadata> {
            None
        }
        fn input_instructions(&self) -> bool {
            false
        }
        fn output_instructions(&self) -> bool {
            false
        }
        fn i18n(&self) -> &Localizer {
            &self.i18n
        }
        fn output_format(&self) -> CliOutputFormat {
            self.output_format
        }
        fn print_data<T>(&mut self, _data: &T) -> Result<()>
        where
            T: JsonSerialize + ?Sized,
        {
            self.printed.push("json".to_string());
            Ok(())
        }
        fn println(&mut self, _data: impl Display) -> Result<()> {
            self.printed.push("text".to_string());
            Ok(())
        }
    }
    test_items! {
    fn output_summary_prefers_json_in_json_mode() {
        let mut ctx = OutputModeContext::new(CliOutputFormat::Json);
        let summary = DaemonIterationSummary::default();
        output_summary(&mut ctx, &summary, false).expect("summary output");
        assert_eq!(ctx.printed, vec!["json"]);
    }
    fn output_summary_uses_text_in_text_mode() {
        let mut ctx = OutputModeContext::new(CliOutputFormat::Text);
        let summary = DaemonIterationSummary::default();
        output_summary(&mut ctx, &summary, false).expect("summary output");
        assert_eq!(ctx.printed, vec!["text"]);
    }
    fn log_daemon_summary_emits_json_in_json_mode() {
        let mut ctx = OutputModeContext::new(CliOutputFormat::Json);
        let summary = DaemonIterationSummary::default();
        log_daemon_summary(&mut ctx, &summary, false).expect("daemon summary");
        assert_eq!(ctx.printed, vec!["json"]);
    }
    fn handshake_update_requires_flags() {
        let result = HandshakeUpdateArgs::default().into_update();
        assert!(result.is_err(), "expected at least one override");
    }
    fn handshake_update_accepts_pow_overrides() {
        let args = HandshakeUpdateArgs {
            descriptor_commit: Some("aa".into()),
            pow_difficulty: Some(7),
            pow_max_future_skew: Some(120),
            ..Default::default()
        };
        let update = args.into_update().expect("update should succeed");
        assert_eq!(update.descriptor_commit_hex.as_deref(), Some("aa"));
        let pow = update.pow.expect("pow overrides present");
        assert_eq!(pow.difficulty, Some(7));
        assert_eq!(pow.max_future_skew_secs, Some(120));
        assert!(pow.min_ticket_ttl_secs.is_none());
        assert!(pow.ticket_ttl_secs.is_none());
    }
    fn handshake_update_validates_resume_hash_length() {
        let args = HandshakeUpdateArgs {
            descriptor_commit: Some("aa".into()),
            resume_hash: Some("deadbeef".into()),
            ..Default::default()
        };
        assert!(args.into_update().is_err(), "resume hash must be 32 bytes");
        let ok_args = HandshakeUpdateArgs {
            descriptor_commit: Some("aa".into()),
            resume_hash: Some("ab".repeat(32)),
            ..Default::default()
        };
        let update = ok_args.into_update().expect("valid resume hash");
        match update
            .resume_hash_hex
            .expect("resume hash directive present")
        {
            ResumeHashDirective::Set(hex) => assert_eq!(hex.len(), 64),
            ResumeHashDirective::Clear => panic!("expected Set directive"),
        }
    }
    }
    fn sample_reward_config_json() -> norito::json::Value {
        let mut policy = norito::json::Map::new();
        policy.insert(
            "minimum_exit_bond".to_string(),
            norito::json::Value::String("1000".to_string()),
        );
        policy.insert(
            "bond_asset_id".to_string(),
            norito::json::Value::String(xor_asset_id().to_string()),
        );
        policy.insert(
            "uptime_floor_per_mille".to_string(),
            norito::json::Value::Number(900u64.into()),
        );
        policy.insert(
            "slash_penalty_basis_points".to_string(),
            norito::json::Value::Number(250u64.into()),
        );
        policy.insert(
            "activation_grace_epochs".to_string(),
            norito::json::Value::Number(0u64.into()),
        );
        let mut root = norito::json::Map::new();
        root.insert("policy".to_string(), norito::json::Value::Object(policy));
        root.insert(
            "base_reward".to_string(),
            norito::json::Value::String("100".to_string()),
        );
        root.insert(
            "uptime_weight_per_mille".to_string(),
            norito::json::Value::Number(500u64.into()),
        );
        root.insert(
            "bandwidth_weight_per_mille".to_string(),
            norito::json::Value::Number(500u64.into()),
        );
        root.insert(
            "compliance_penalty_basis_points".to_string(),
            norito::json::Value::Number(0u64.into()),
        );
        root.insert(
            "bandwidth_target_bytes".to_string(),
            norito::json::Value::Number(1_000u64.into()),
        );
        root.insert(
            "budget_approval_id".to_string(),
            norito::json::Value::String(sample_budget_id_hex()),
        );
        root.insert("metrics_log_path".to_string(), norito::json::Value::Null);
        norito::json::Value::Object(root)
    }
    fn sample_bond_entry(amount: u32) -> RelayBondLedgerEntryV1 {
        RelayBondLedgerEntryV1 {
            relay_id: [0xAB; 32],
            bonded_amount: Quantity::from(amount),
            bond_asset_id: xor_asset_id(),
            bonded_since_unix: 1,
            exit_capable: true,
        }
    }
    fn sample_metrics() -> RelayEpochMetricsV1 {
        RelayEpochMetricsV1 {
            relay_id: [0xAB; 32],
            epoch: 7,
            uptime_seconds: 3_600,
            scheduled_uptime_seconds: 3_600,
            verified_bandwidth_bytes: 1_000,
            compliance: RelayComplianceStatusV1::Clean,
            reward_score: 0,
            confidence_floor_per_mille: 1_000,
            measurement_ids: Vec::new(),
            metadata: Metadata::default(),
        }
    }
    fn sample_reward_instruction() -> RelayRewardInstructionV1 {
        RelayRewardInstructionV1 {
            relay_id: [0xCD; 32],
            epoch: 9,
            beneficiary: sample_account_id("relay-beneficiary"),
            payout_asset_id: xor_asset_id(),
            payout_amount: Quantity::from(42_u32),
            reward_score: 750,
            budget_approval_id: Some(sample_budget_id()),
            metadata: Metadata::default(),
        }
    }
    fn sample_transfer_record(kind: TransferKind, amount: u32) -> LedgerTransferRecord {
        LedgerTransferRecord {
            relay_id: [0xAA; 32],
            epoch: 3,
            kind,
            dispute_id: None,
            amount: Quantity::from(amount),
            source_asset: AssetId::new(xor_asset_id(), sample_account_id("treasury")),
            destination: sample_account_id("relay"),
        }
    }
    test_items! {
    fn incentive_quantity_parser_rejects_negative_amounts() {
        let error = parse_quantity_str("-1", "--requested-amount")
            .expect_err("negative reward quantities must be rejected");
        assert!(error.to_string().contains("non-negative quantity"));
    }
    fn ledger_export_schema_mismatch_reports_expected_and_actual() {
        const SCHEMA_OFFSET: usize = 4 + 1 + 1;
        let export = LedgerExportFile {
            version: LedgerExportFile::VERSION,
            transfers: vec![sample_transfer_record(TransferKind::Payout, 5)],
        };
        let mut bytes = to_bytes(&export).expect("encode ledger export");
        bytes[SCHEMA_OFFSET] ^= 0xFF;
        let file = NamedTempFile::new().expect("temp file");
        fs::write(file.path(), bytes).expect("write ledger export");
        let err = read_ledger_export(file.path()).expect_err("schema mismatch should fail");
        let messages = err.chain().map(ToString::to_string).collect::<Vec<_>>();
        let combined = messages.join("\n");
        assert_compact! { combined.contains("schema mismatch"); "expected schema mismatch in error chain: {combined}" };
        assert_compact! { combined.contains("expected"); "expected schema hash detail in error chain: {combined}" };
        assert_compact! { combined.contains("got"); "expected actual schema hash detail in error chain: {combined}" };
    }
    fn reconciliation_summary_builds_expected_counts() {
        let missing_record = sample_transfer_record(TransferKind::Payout, 100);
        let unexpected_record = sample_transfer_record(TransferKind::Credit, 25);
        let mismatch_expected = sample_transfer_record(TransferKind::Debit, 40);
        let mut mismatch_actual = sample_transfer_record(TransferKind::Debit, 35);
        mismatch_actual.destination = sample_account_id("alt-treasury");
        let mut invalid_amount_record = sample_transfer_record(TransferKind::Payout, 5);
        invalid_amount_record.amount = "340282366920938463463374607431768211456"
            .parse::<Quantity>()
            .expect("2^128 quantity");
        let report = LedgerReconciliationReport {
            total_expected_transfers: 3,
            matched_transfers: 1,
            expected_amount: 150_u64.into(),
            exported_amount: 70_u64.into(),
            missing_transfers: vec![ExpectedLedgerTransfer {
                record: missing_record.clone(),
            }],
            unexpected_transfers: vec![unexpected_record.clone()],
            mismatched_transfers: vec![LedgerTransferMismatch {
                expected: mismatch_expected.clone(),
                actual: mismatch_actual.clone(),
                reasons: vec![MismatchReason::Amount, MismatchReason::Destination],
            }],
            amount_arithmetic_errors: vec![LedgerAmountArithmeticError {
                source: LedgerAmountSource::Exported,
                record: invalid_amount_record.clone(),
            }],
        };
        let summary = ReconciliationReportSummary::from_report(&report);
        assert!(!summary.clean);
        assert_eq!(summary.matched_transfers, 1);
        assert_eq!(summary.total_expected_transfers, 3);
        assert_eq!(summary.missing_transfers.len(), 1);
        assert_eq!(summary.unexpected_transfers.len(), 1);
        assert_eq!(summary.mismatched_transfers.len(), 1);
        assert_eq_compact! { summary.missing_transfers[0].relay_id => relay_id_to_hex(missing_record.relay_id) };
        assert_eq_compact! { summary.unexpected_transfers[0].kind => transfer_kind_label(unexpected_record.kind) };
        assert_compact! { summary.mismatched_transfers[0].reasons.iter().any(|reason| reason == "amount") };
        assert_eq!(summary.amount_arithmetic_errors.len(), 1);
        assert_eq!(summary.amount_arithmetic_errors[0].source, "exported");
        assert_eq_compact! { summary.amount_arithmetic_errors[0].record.amount => invalid_amount_record.amount.to_string() };
        assert_eq_compact! { summary.amount_arithmetic_errors[0].record.amount_nanos => None };
        assert_compact! { summary.amount_arithmetic_errors[0].record.amount_conversion_error.is_some() };
    }
    }
    fn sample_account_id(name: &str) -> AccountId {
        let mut hasher = Blake3Hasher::new();
        hasher.update(b"sorafs-sample-account");
        hasher.update(name.as_bytes());
        let digest = hasher.finalize();
        let mut seed = [0u8; 32];
        seed.copy_from_slice(digest.as_bytes());
        let signing = SigningKey::from_bytes(&seed);
        let verifying = signing.verifying_key();
        let public_key =
            PublicKey::from_bytes(Algorithm::Ed25519, verifying.as_bytes()).expect("public key");
        AccountId::new(public_key)
    }
    fn sample_account_literal(name: &str) -> String {
        let account = sample_account_id(name);
        account.to_string()
    }
    fn xor_asset_id() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            iroha_data_model::domain::DomainId::try_new("sora", "universal").unwrap(),
            "xor".parse().unwrap(),
        )
    }
    fn sample_budget_id_hex() -> String {
        hex::encode(sample_budget_id())
    }
    fn sample_budget_id() -> [u8; 32] {
        [0x11_u8; 32]
    }
    fn write_reward_config_with_budget(budget_hex: Option<&str>) -> NamedTempFile {
        let mut config = sample_reward_config_json();
        let budget_value = budget_hex.map_or(Value::Null, |hex| Value::String(hex.to_string()));
        config
            .as_object_mut()
            .expect("sample reward config should be an object")
            .insert("budget_approval_id".to_string(), budget_value);
        let mut file = NamedTempFile::new().expect("config file");
        let bytes = norito::json::to_vec(&config).expect("encode config");
        file.write_all(&bytes).expect("write config");
        file
    }
    fn write_sample_reward_config_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("config file");
        let bytes = norito::json::to_vec(&sample_reward_config_json()).expect("encode config");
        file.write_all(&bytes).expect("write config");
        file
    }
    fn write_metrics_file(metrics: &RelayEpochMetricsV1) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("metrics file");
        let bytes = to_bytes(metrics).expect("encode metrics");
        file.write_all(&bytes).expect("write metrics");
        file
    }
    fn write_bond_file(bond: &RelayBondLedgerEntryV1) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("bond file");
        let bytes = to_bytes(bond).expect("encode bond");
        file.write_all(&bytes).expect("write bond");
        file
    }
    fn write_gc_manifest(
        root: &Path,
        manifest_id: &str,
        retention_epoch: u64,
        storage_class: ManifestStorageClass,
        payload_bytes: u64,
        car_bytes: u64,
    ) {
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0x01, 0x02, 0x03])
            .dag_codec(DagCodecId(0x71))
            .chunking_profile(ChunkingProfileV1 {
                profile_id: ProfileId(7),
                namespace: "sorafs".into(),
                name: "sf1".into(),
                semver: "1.0.0".into(),
                min_size: 4096,
                target_size: 262_144,
                max_size: 524_288,
                break_mask: 0,
                multihash_code: BLAKE3_256_MULTIHASH_CODE,
                aliases: vec!["sf1".into()],
            })
            .chunk_digest_sha3_256([0xCD; 32])
            .por_root(if payload_bytes == 0 {
                sorafs_manifest::EMPTY_POR_ROOT_V1
            } else {
                [0xCE; 32]
            })
            .content_length(payload_bytes)
            .car_digest([0xAB; 32])
            .car_size(car_bytes)
            .pin_policy(PinPolicy {
                min_replicas: 1,
                storage_class,
                retention_epoch,
            })
            .build()
            .expect("build manifest");
        let bytes = to_bytes(&manifest).expect("encode manifest");
        let manifest_dir = root.join(SORAFS_MANIFEST_DIR).join(manifest_id);
        fs::create_dir_all(&manifest_dir).expect("create manifest dir");
        fs::write(manifest_dir.join(SORAFS_MANIFEST_FILE), bytes).expect("write manifest file");
    }
    fn read_state(path: &Path) -> IncentivesState {
        load_incentives_state(path).expect("decode incentives state")
    }
    fn initialize_incentives_state(config: &Path, state: &Path) -> TestContext {
        let args = IncentivesServiceInitArgs {
            state: state.to_path_buf(),
            config: config.to_path_buf(),
            treasury_account: sample_account_literal("treasury"),
            force: false,
        };
        let mut context = TestContext::new();
        args.run(&mut context).expect("init command runs");
        context
    }
    fn write_state_without_budget(path: &Path) {
        let config_file = write_reward_config_with_budget(None);
        let reward_config = read_reward_config(config_file.path()).expect("reward config");
        let state = IncentivesState::new(&reward_config, sample_account_id("treasury"));
        save_incentives_state(path, &state).expect("write incentives state");
    }
    test_items! {
    #[cfg(unix)]
    fn handshake_token_issue_generates_verifiable_token() {
        let mut ctx = TestContext::new();
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keypair");
        let mut secret_file = NamedTempFile::new().expect("secret file");
        secret_file
            .write_all(keypair.secret_key())
            .expect("write secret key");
        let public_hex = hex::encode(keypair.public_key());
        let output_dir = TempDir::new().expect("token output directory");
        let output_path = output_dir.path().join("admission.token");
        let args = HandshakeTokenIssueArgs {
            suite: MlDsaSuiteArg::MlDsa44,
            issuer_secret_key: secret_file.path().to_path_buf(),
            issuer_public_key: None,
            issuer_public_hex: Some(public_hex.clone()),
            relay_id: "11".repeat(32),
            transcript_hash: "22".repeat(32),
            issued_at: Some("2026-01-01T00:00:00Z".to_string()),
            expires_at: None,
            ttl_secs: Some(900),
            flags: Some(0),
            output: output_path.clone(),
            token_encoding: TokenOutputFormat::Base64,
        };
        let mut rng = StdRng::seed_from_u64(0x5eed);
        let default_now = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let mut artifacts = args
            .issue_with_rng(&mut ctx, &mut rng, default_now)
            .expect("issue token");
        let replay_limits = TokenStoreLimits::new(4, Duration::from_secs(1_800))
            .expect("fixture replay limits");
        let replay_store: Arc<Mutex<dyn TokenStore + Send>> = Arc::new(Mutex::new(
            InMemoryTokenStore::new(replay_limits).expect("fixture replay store"),
        ));
        let verifier = AdmissionTokenVerifier::try_new(
            MlDsaSuite::MlDsa44,
            keypair.public_key().to_vec(),
            Duration::from_secs(900),
            Duration::from_secs(5),
            replay_store,
        )
        .expect("generated verifier key must match ML-DSA-44");
        let verify_now = SystemTime::UNIX_EPOCH
            + Duration::from_secs(artifacts.token.issued_at().saturating_add(1));
        verifier
            .verify(
                &artifacts.token,
                &artifacts.relay_id,
                &artifacts.transcript_hash,
                verify_now,
            )
            .expect("token should verify");
        HandshakeTokenIssueArgs::emit(
            &mut ctx,
            &artifacts,
            &output_path,
            TokenOutputFormat::Base64,
        )
        .expect("emit output");
        let output = ctx.outputs().last().expect("json output present");
        let json: Value = norito::json::from_str(output).expect("valid json");
        assert_eq!(json["flags"], Value::from(0u64));
        assert_eq_compact! { json["token_id_hex"] => Value::from(hex::encode(artifacts.token.token_id())) };
        assert!(json.get("token_base64url").is_none());
        assert!(json.get("token_hex").is_none());
        use std::os::unix::fs::MetadataExt as _;
        assert_eq!(
            fs::metadata(&output_path).expect("token metadata").mode() & 0o077,
            0
        );
        let error = HandshakeTokenIssueArgs::emit(
            &mut ctx,
            &artifacts,
            &output_path,
            TokenOutputFormat::Base64,
        )
        .expect_err("existing bearer output must not be overwritten");
        let rendered = format!("{error:#}");
        assert!(
            rendered.contains("failed to create new owner-private token output"),
            "unexpected overwrite error: {rendered}"
        );
        artifacts.zeroize_encoded_token();
        assert!(artifacts.token_bytes.is_empty());
    }
    #[cfg(unix)]
    fn handshake_token_issue_rejects_explicit_subsecond_timestamps() {
        let mut ctx = TestContext::new();
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keypair");
        let mut secret_file = NamedTempFile::new().expect("secret file");
        secret_file
            .write_all(keypair.secret_key())
            .expect("write secret key");
        let output_dir = TempDir::new().expect("token output directory");
        let mut args = HandshakeTokenIssueArgs {
            suite: MlDsaSuiteArg::MlDsa44,
            issuer_secret_key: secret_file.path().to_path_buf(),
            issuer_public_key: None,
            issuer_public_hex: Some(hex::encode(keypair.public_key())),
            relay_id: "11".repeat(32),
            transcript_hash: "22".repeat(32),
            issued_at: Some("2026-01-01T00:00:00.123Z".to_string()),
            expires_at: None,
            ttl_secs: Some(900),
            flags: Some(0),
            output: output_dir.path().join("admission.token"),
            token_encoding: TokenOutputFormat::Binary,
        };
        let mut rng = StdRng::seed_from_u64(0x5eed);
        let error = match args.issue_with_rng(&mut ctx, &mut rng, UNIX_EPOCH) {
            Ok(_) => panic!("fractional explicit issuance time must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("--issued-at must use whole-second"));

        args.issued_at = Some("2026-01-01T00:00:00Z".to_string());
        args.expires_at = Some("2026-01-01T00:15:00.123Z".to_string());
        args.ttl_secs = None;
        let error = match args.issue_with_rng(&mut ctx, &mut rng, UNIX_EPOCH) {
            Ok(_) => panic!("fractional explicit expiry time must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("--expires-at must use whole-second"));
    }
    #[cfg(unix)]
    fn handshake_token_issue_floors_only_default_wall_clock_time() {
        let mut ctx = TestContext::new();
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keypair");
        let mut secret_file = NamedTempFile::new().expect("secret file");
        secret_file
            .write_all(keypair.secret_key())
            .expect("write secret key");
        let output_dir = TempDir::new().expect("token output directory");
        let args = HandshakeTokenIssueArgs {
            suite: MlDsaSuiteArg::MlDsa44,
            issuer_secret_key: secret_file.path().to_path_buf(),
            issuer_public_key: None,
            issuer_public_hex: Some(hex::encode(keypair.public_key())),
            relay_id: "33".repeat(32),
            transcript_hash: "44".repeat(32),
            issued_at: None,
            expires_at: None,
            ttl_secs: Some(600),
            flags: None,
            output: output_dir.path().join("admission.token"),
            token_encoding: TokenOutputFormat::Binary,
        };
        let default_seconds = 1_800_000_000;
        let default_now = UNIX_EPOCH
            + Duration::from_secs(default_seconds)
            + Duration::from_nanos(987_654_321);
        let mut rng = StdRng::seed_from_u64(0xabad_1dea);
        let artifacts = args
            .issue_with_rng(&mut ctx, &mut rng, default_now)
            .expect("default wall clock is canonicalized");
        assert_eq!(artifacts.issued_dt.nanosecond(), 0);
        assert_eq!(artifacts.expires_dt.nanosecond(), 0);
        assert_eq!(artifacts.token.issued_at(), default_seconds);
        assert_eq!(artifacts.token.expires_at(), default_seconds + 600);
        assert_eq!(artifacts.issued_dt.unix_timestamp(), default_seconds as i64);
    }
    #[cfg(unix)]
    fn handshake_token_id_reports_expected_digest() {
        let mut ctx = TestContext::new();
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keypair");
        let mut secret_file = NamedTempFile::new().expect("secret file");
        secret_file
            .write_all(keypair.secret_key())
            .expect("write secret key");
        let public_hex = hex::encode(keypair.public_key());
        let output_dir = TempDir::new().expect("token output directory");
        let args = HandshakeTokenIssueArgs {
            suite: MlDsaSuiteArg::MlDsa44,
            issuer_secret_key: secret_file.path().to_path_buf(),
            issuer_public_key: None,
            issuer_public_hex: Some(public_hex),
            relay_id: "33".repeat(32),
            transcript_hash: "44".repeat(32),
            issued_at: Some("2026-02-01T00:00:00Z".to_string()),
            expires_at: None,
            ttl_secs: Some(600),
            flags: None,
            output: output_dir.path().join("unused.token"),
            token_encoding: TokenOutputFormat::Base64,
        };
        let mut rng = StdRng::seed_from_u64(0xabad_1dea);
        let artifacts = args
            .issue_with_rng(
                &mut ctx,
                &mut rng,
                SystemTime::UNIX_EPOCH + Duration::from_secs(10),
            )
            .expect("issue token");
        let token_path = output_dir.path().join("admission.token");
        write_token_to_file(
            &token_path,
            TokenOutputFormat::Binary,
            &artifacts.token_bytes,
        )
        .expect("write private token file");
        let id_args = HandshakeTokenIdArgs { path: token_path };
        id_args.run(&mut ctx).expect("compute id");
        let output = ctx.outputs().last().expect("json output");
        let json: Value = norito::json::from_str(output).expect("valid json");
        assert_eq_compact! { json["token_id_hex"] => Value::from(hex::encode(artifacts.token.token_id())) };
    }
    fn handshake_token_fingerprint_matches_helper() {
        let mut ctx = TestContext::new();
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa65).expect("keypair");
        let public_hex = hex::encode(keypair.public_key());
        let expected = token::compute_issuer_fingerprint(keypair.public_key());
        let args = HandshakeTokenFingerprintArgs {
            public_key: None,
            public_key_hex: Some(public_hex),
        };
        args.run(&mut ctx).expect("fingerprint");
        let output = ctx.outputs().last().expect("json output");
        let json: Value = norito::json::from_str(output).expect("valid json");
        assert_eq_compact! { json["issuer_fingerprint_hex"] => Value::from(hex::encode(expected)) };
    }
    fn handshake_token_cli_rejects_secret_and_bearer_argv_inputs() {
        use clap::Parser as _;
        #[derive(clap::Parser, Debug)]
        struct Parser {
            #[command(subcommand)]
            command: HandshakeTokenCommand,
        }
        let relay_id = "11".repeat(32);
        let transcript_hash = "22".repeat(32);
        let issue_error = Parser::try_parse_from([
            "token-test",
            "issue",
            "--issuer-secret-hex",
            "00",
            "--issuer-public-hex",
            "00",
            "--relay-id",
            &relay_id,
            "--transcript-hash",
            &transcript_hash,
            "--output",
            "token.bin",
        ])
        .expect_err("inline issuer secret must be unknown");
        assert_eq!(issue_error.kind(), clap::error::ErrorKind::UnknownArgument);
        let id_error = Parser::try_parse_from(["token-test", "id", "--token-hex", "00"])
            .expect_err("inline bearer token must be unknown");
        assert_eq!(id_error.kind(), clap::error::ErrorKind::UnknownArgument);
    }
    #[cfg(unix)]
    fn handshake_token_private_reader_rejects_public_links_and_oversize() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let directory = TempDir::new().expect("private input directory");
        let secret = directory.path().join("secret.key");
        fs::write(&secret, [0xA5; 32]).expect("write secret");
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o600))
            .expect("set private mode");
        assert_eq!(
            read_owner_private_handshake_file(&secret, 32, Some(32), "secret")
                .expect("private secret")
                .as_slice(),
            [0xA5; 32]
        );
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o640))
            .expect("set public mode");
        assert!(
            read_owner_private_handshake_file(&secret, 32, Some(32), "secret")
                .expect_err("group-readable secret must fail")
                .to_string()
                .contains("owner-private")
        );
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o600))
            .expect("restore private mode");
        let hard_link = directory.path().join("secret-copy.key");
        fs::hard_link(&secret, &hard_link).expect("create hard link");
        assert!(
            read_owner_private_handshake_file(&secret, 32, Some(32), "secret")
                .expect_err("multiply linked secret must fail")
                .to_string()
                .contains("exactly one link")
        );
        let direct = directory.path().join("direct.key");
        fs::write(&direct, [0xB6; 32]).expect("write direct target");
        fs::set_permissions(&direct, fs::Permissions::from_mode(0o600))
            .expect("set target mode");
        let symbolic = directory.path().join("secret-link.key");
        symlink(&direct, &symbolic).expect("create symbolic link");
        assert!(
            read_owner_private_handshake_file(&symbolic, 32, Some(32), "secret")
                .expect_err("symbolic link must fail")
                .to_string()
                .contains("non-symlink")
        );
        let oversized = directory.path().join("oversized.token");
        let file = fs::File::create(&oversized).expect("create oversized token");
        file.set_len((HANDSHAKE_TOKEN_FILE_MAX_BYTES_V1 + 1) as u64)
            .expect("size oversized token");
        fs::set_permissions(&oversized, fs::Permissions::from_mode(0o600))
            .expect("set oversized mode");
        assert!(
            read_owner_private_handshake_file(
                &oversized,
                HANDSHAKE_TOKEN_FILE_MAX_BYTES_V1,
                None,
                "token",
            )
            .expect_err("oversized token must fail")
            .to_string()
            .contains("must contain between")
        );
        let public_key = directory.path().join("issuer.pub");
        fs::write(&public_key, [0xC7; 32]).expect("write public key");
        assert_eq!(
            materialise_key_bytes(
                Some(&public_key),
                None,
                "--issuer-public-key",
                "--issuer-public-hex",
                32,
                Some(32),
            )
            .expect("exact public key"),
            [0xC7; 32]
        );
        fs::write(&public_key, [0xC7; 33]).expect("grow public key");
        assert!(
            materialise_key_bytes(
                Some(&public_key),
                None,
                "--issuer-public-key",
                "--issuer-public-hex",
                32,
                Some(32),
            )
            .expect_err("oversized public key must fail")
            .to_string()
            .contains("exactly 32 bytes")
        );
    }
    }
    impl RunContext for TestContext {
        fn config(&self) -> &Config {
            &self.cfg
        }
        fn transaction_metadata(&self) -> Option<&Metadata> {
            None
        }
        fn input_instructions(&self) -> bool {
            false
        }
        fn output_instructions(&self) -> bool {
            false
        }
        fn i18n(&self) -> &Localizer {
            &self.i18n
        }
        fn output_format(&self) -> CliOutputFormat {
            self.output_format
        }
        fn print_data<T>(&mut self, data: &T) -> Result<()>
        where
            T: JsonSerialize + ?Sized,
        {
            let bytes = norito::json::to_vec(data)?;
            let out = String::from_utf8(bytes).map_err(|err| eyre!(err.to_string()))?;
            self.printed.push(out);
            Ok(())
        }
        fn println(&mut self, data: impl Display) -> Result<()> {
            self.printed.push(data.to_string());
            Ok(())
        }
    }
    test_items! {
    fn gateway_provider_spec_parses_expected_keys() {
        let id_hex = "11".repeat(32);
        let key_hex = "22".repeat(32);
        let spec = format!(
            "name=alpha, provider-id={id_hex}, gateway-key={key_hex}, base-url=https://example.com, stream-token=YWJj"
        );
        let parsed = parse_gateway_provider_spec(&spec).expect("parse spec");
        assert_eq!(parsed.name, "alpha");
        assert_eq!(parsed.provider_id_hex, id_hex);
        assert_eq!(parsed.gateway_public_key_hex, key_hex);
        assert_eq!(parsed.base_url, "https://example.com");
        assert_eq!(parsed.stream_token_b64, "YWJj");
    }
    fn gateway_provider_spec_rejects_missing_fields() {
        let err = parse_gateway_provider_spec("name=alpha, base-url=https://example.com")
            .expect_err("missing provider-id should fail");
        assert_compact! { err.to_string().contains("provider-id"); "unexpected error: {err}" };
    }
    fn validate_hex_digest_enforces_format() {
        let valid = validate_hex_digest(&"ab".repeat(32), "--flag").expect("valid digest");
        assert_eq!(valid, "ab".repeat(32));
        let err = validate_hex_digest("zz", "--flag").expect_err("invalid digest");
        assert!(err.to_string().contains("--flag"));
    }
    fn parse_transport_policy_flag_accepts_valid_value() {
        let value = "soranet-strict".to_string();
        let parsed = parse_transport_policy_flag(Some(&value), "--transport-policy-override")
            .expect("parse transport policy");
        assert_eq!(parsed, Some(TransportPolicy::SoranetStrict));
    }
    fn parse_transport_policy_flag_rejects_noncanonical_inputs() {
        for rejected in [
            "",
            " ",
            " soranet-first",
            "soranet-first ",
            "SORANET-FIRST",
            "soranet_first",
            "soranet_strict",
            "direct_only",
            "soranet-only",
            "soranet_only",
        ] {
            let rejected_value = rejected.to_owned();
            assert_compact! { parse_transport_policy_flag(Some(&rejected_value), "--transport-policy").is_err(); "noncanonical transport label `{rejected}` must fail" };
        }
    }
    fn parse_anonymity_policy_flag_accepts_canonical_value() {
        let value = "anon-majority-pq".to_string();
        let parsed = parse_anonymity_policy_flag(Some(&value), "--anonymity-policy-override")
            .expect("parse anonymity policy");
        assert_eq!(parsed, Some(AnonymityPolicy::MajorityPq));
    }
    fn parse_anonymity_policy_flag_rejects_noncanonical_inputs() {
        for rejected in [
            "",
            " ",
            " anon-guard-pq",
            "anon-guard-pq ",
            "ANON-GUARD-PQ",
            "anon_guard_pq",
            "anon_majority_pq",
            "anon_strict_pq",
            "stage-a",
            "stage_a",
            "stagea",
            "stage-b",
            "stage_b",
            "stageb",
            "stage-c",
            "stage_c",
            "stagec",
            "anon-unknown",
        ] {
            let rejected_value = rejected.to_owned();
            assert_compact! { parse_anonymity_policy_flag(Some(&rejected_value), "--anonymity-policy").is_err(); "noncanonical anonymity label `{rejected}` must fail" };
        }
    }
    fn parse_write_mode_flag_accepts_only_exact_v1_labels() {
        for (label, expected) in [
            ("read-only", WriteModeHint::ReadOnly),
            ("upload-pq-only", WriteModeHint::UploadPqOnly),
        ] {
            let label_value = label.to_owned();
            assert_eq_compact! { parse_write_mode_flag(Some(&label_value), "--write-mode").expect("canonical write mode") => Some(expected) };
        }
        for rejected in [
            "",
            " ",
            " read-only",
            "read-only ",
            "READ-ONLY",
            "read_only",
            "upload_pq_only",
        ] {
            let rejected_value = rejected.to_owned();
            assert_compact! { parse_write_mode_flag(Some(&rejected_value), "--write-mode").is_err(); "noncanonical write-mode label `{rejected}` must fail" };
        }
    }
    fn anonymity_policy_label_matches_expected_values() {
        assert_eq_compact! { anonymity_policy_label(AnonymityPolicy::GuardPq) => "anon-guard-pq" };
        assert_eq_compact! { anonymity_policy_label(AnonymityPolicy::MajorityPq) => "anon-majority-pq" };
        assert_eq_compact! { anonymity_policy_label(AnonymityPolicy::StrictPq) => "anon-strict-pq" };
    }
    fn load_guard_directory_json_rejected() {
        let mut file = NamedTempFile::new().expect("temp file");
        let id_primary = "01".repeat(32);
        let id_secondary = "02".repeat(32);
        let pq_hex = "aa".repeat(ML_KEM_768_PUBLIC_LEN);
        let json = format!(
            r#"{{
  "relays": [
    {{
      "relay_id_hex": "{id_primary}",
      "guard_weight": 10,
      "roles": {{ "entry": true, "middle": false, "exit": false }},
      "endpoints": [{{ "url": "soranet://pq.guard", "priority": 0 }}],
      "ml_kem_public_hex": "{pq_hex}"
    }},
    {{
      "relay_id_hex": "{id_secondary}",
      "guard_weight": 5,
      "roles": {{ "entry": true, "middle": false, "exit": false }},
      "endpoints": [{{ "url": "soranet://classical.guard", "priority": 0 }}]
    }}
  ]
}}
"#
        );
        write!(file, "{json}").expect("write guard directory");
        let json_bytes = fs::read(file.path()).expect("read fixture");
        let digest = hex::encode(compute_snapshot_digest(&json_bytes));
        let err = load_guard_directory(file.path(), &digest, 1_734_000_000)
            .expect_err("json format must be rejected");
        let msg = err.to_string();
        assert_compact! { msg.contains("failed to authenticate guard directory"); "unexpected error message: {msg}" };
        assert_compact! { msg.contains("SRCv2"); "error should mention the canonical SRCv2 Norito format: {msg}" };
    }
    fn load_guard_directory_decodes_srcv2_bundle() {
        let bytes = sample_guard_directory_snapshot_bytes();
        let mut file = NamedTempFile::new().expect("temp file");
        file.write_all(&bytes).expect("write snapshot");
        let digest = hex::encode(compute_snapshot_digest(&bytes));
        let directory =
            load_guard_directory(file.path(), &digest, 1_734_000_000).expect("load directory");
        let entries = directory.entries();
        assert_eq!(entries.len(), 1);
        let descriptor = &entries[0];
        let expected_relay_id =
            Ed25519VerifyingKey::from(&sample_guard_directory_signing_key()).to_bytes();
        assert_eq!(descriptor.relay_id, expected_relay_id);
        assert!(descriptor.is_pq_capable());
        assert!(descriptor.certificate().is_some());
        assert_eq_compact! { descriptor.certificate_validity() => directory.valid_after().zip(directory.valid_until()) };
        assert_eq!(directory.valid_after(), Some(1_734_000_000));
        assert_eq!(directory.valid_until(), Some(1_734_086_400));
    }
    }
    include!("sorafs_guard_directory_tests.rs");
    test_items! {
        fn authenticated_directory_accepts_matching_snapshot_digest() {
            let bytes = sample_guard_directory_snapshot_bytes();
            let expected = hex::encode(compute_snapshot_digest(&bytes));
            let summary = authenticate_guard_directory_bytes(&bytes, &expected, 1_734_000_000)
                .expect("digest and time should authenticate");
            assert_eq!(summary.authentication, "authenticated");
        }
        fn authenticated_directory_rejects_mismatch_and_expiry() {
            let bytes = sample_guard_directory_snapshot_bytes();
            let mismatch = authenticate_guard_directory_bytes(&bytes, &"00".repeat(32), 1_734_000_000);
            assert!(mismatch.is_err(), "snapshot digest mismatch should fail");
            let expected = hex::encode(compute_snapshot_digest(&bytes));
            let expired = authenticate_guard_directory_bytes(&bytes, &expected, 1_734_086_400);
            assert!(expired.is_err(), "expired snapshot should fail");
        }
        fn write_guard_directory_snapshot_honours_overwrite_flag() {
            use tempfile::TempDir;
            let temp_dir = TempDir::new().expect("temp dir");
            let path = temp_dir.path().join("snapshot.norito");
            let bytes = sample_guard_directory_snapshot_bytes();
            write_guard_directory_snapshot(&path, &bytes, false).expect("first write should succeed");
            let second = write_guard_directory_snapshot(&path, &bytes, false);
            assert!(second.is_err(), "expected overwrite protection");
            write_guard_directory_snapshot(&path, &bytes, true).expect("overwrite when allowed");
        }
        fn pin_list_with_prints_payload() {
            let block_hash = "11".repeat(32);
            let after_digest = "22".repeat(32);
            let args = PinListArgs {
                status: Some(PinStatusSelector::Approved),
                limit: Some(5),
                max_bytes: Some(4096),
                after_digest_hex: Some(after_digest.clone()),
                expected_finalized_height: Some(7),
                expected_finalized_block_hash_hex: Some(block_hash.clone()),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, filter| {
                assert_eq!(filter.status, Some(PinStatusKindV1::Approved));
                assert_eq!(filter.limit, Some(5));
                assert_eq!(filter.max_bytes, Some(4096));
                assert_eq!(filter.after_digest_hex, Some(after_digest.as_str()));
                assert_eq!(filter.finalized.expected_finalized_height, Some(7));
                assert_eq_compact! { filter.finalized.expected_finalized_block_hash_hex => Some(block_hash.as_str()) };
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "manifests": [ { "digest": "aa" } ]
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"manifests\""));
        }
        fn pin_list_with_propagates_error_status() {
            let args = PinListArgs {
                status: None,
                limit: None,
                max_bytes: None,
                after_digest_hex: None,
                expected_finalized_height: None,
                expected_finalized_block_hash_hex: None,
            };
            let mut ctx = TestContext::new();
            let result = args.run_with(&mut ctx, |_client, _| {
    json_response_fixture!(StatusCode::BAD_REQUEST,
                        &norito::json!({ "error": "bad request" }),
                    )
            });
            assert!(result.is_err());
            assert!(ctx.printed.is_empty());
        }
        fn pin_show_with_handles_not_found() {
            let digest = "33".repeat(32);
            let args = PinShowArgs {
                digest: digest.clone(),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, actual_digest| {
                assert_eq!(actual_digest, digest.as_str());
    json_response_fixture!(StatusCode::NOT_FOUND,
                        &norito::json!({ "error": "missing" }),
                    )
            })
            .expect("run should succeed for 404");
            assert_eq_compact! { ctx.printed => vec![format!("manifest `{digest}` not found")] };
        }
        fn alias_list_with_prints_payload() {
            let manifest_digest = "44".repeat(32);
            let args = AliasListArgs {
                limit: Some(3),
                offset: Some(0),
                namespace: Some("docs".to_string()),
                manifest_digest: Some(manifest_digest.clone()),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, filter| {
                assert_eq!(filter.limit, Some(3));
                assert_eq!(filter.namespace, Some("docs"));
                assert_eq!(filter.manifest_digest, Some(manifest_digest.as_str()));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "aliases": [
                            { "alias": "docs/latest", "digest": manifest_digest }
                        ]
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"aliases\""));
        }
        fn replication_list_with_prints_payload() {
            let args = ReplicationListArgs {
                limit: Some(2),
                offset: None,
                status: Some(ReplicationStatusSelector::Completed),
                manifest_digest: None,
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, filter| {
                assert_eq!(filter.status, Some(SorafsReplicationStatus::Completed));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "orders": [
                            { "id": "order1", "status": "completed" }
                        ]
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"orders\""));
        }
        fn replication_status_cli_is_closed_and_includes_cancelled() {
            use clap::Parser as _;
            #[derive(clap::Parser, Debug)]
            struct Parser {
                #[command(flatten)]
                args: ReplicationListArgs,
            }
            let parsed = Parser::try_parse_from(["sorafs-test", "--status", "cancelled"])
                .expect("cancelled must be part of the first-release status set");
            assert!(matches!(
                parsed.args.status,
                Some(ReplicationStatusSelector::Cancelled)
            ));
            let error = Parser::try_parse_from(["sorafs-test", "--status", "Completed"])
                .expect_err("status parsing must remain exact and case-sensitive");
            assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
        }
        fn pin_and_inventory_cli_reject_noncanonical_digests_before_fetch() {
            let mut ctx = TestContext::new();
            let pin_result = PinShowArgs {
                digest: "deadbeef".to_owned(),
            }
            .run_with(&mut ctx, |_client, _digest| {
                panic!("invalid pin digest must fail before fetch")
            });
            assert!(
                pin_result
                    .expect_err("short pin digest must fail")
                    .to_string()
                    .contains("64 lowercase")
            );

            let alias_result = AliasListArgs {
                limit: None,
                offset: None,
                namespace: None,
                manifest_digest: Some("AA".repeat(32)),
            }
            .run_with(&mut ctx, |_client, _filter| {
                panic!("invalid alias digest must fail before fetch")
            });
            assert!(
                alias_result
                    .expect_err("uppercase alias digest must fail")
                    .to_string()
                    .contains("64 lowercase")
            );

            let replication_result = ReplicationListArgs {
                limit: None,
                offset: None,
                status: Some(ReplicationStatusSelector::Pending),
                manifest_digest: Some("00".repeat(32)),
            }
            .run_with(&mut ctx, |_client, _filter| {
                panic!("invalid replication digest must fail before fetch")
            });
            assert!(
                replication_result
                    .expect_err("zero replication digest must fail")
                    .to_string()
                    .contains("non-zero")
            );

            let pin_list_result = PinListArgs {
                status: None,
                limit: None,
                max_bytes: None,
                after_digest_hex: Some("abc123".to_owned()),
                expected_finalized_height: None,
                expected_finalized_block_hash_hex: None,
            }
            .run_with(&mut ctx, |_client, _filter| {
                panic!("invalid pin-list cursor must fail before fetch")
            });
            assert!(
                pin_list_result
                    .expect_err("short pin-list cursor must fail")
                    .to_string()
                    .contains("64 lowercase")
            );
        }
        fn transparency_cycles_list_prints_payload() {
            let args = TransparencyCyclesListArgs { limit: Some(8) };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, filter| {
                assert_eq!(filter.limit, Some(8));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "cycles": [
                            { "cycle_id_hex": "aa" }
                        ]
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"cycles\""));
        }
        fn transparency_cycles_get_normalizes_cycle_id() {
            let args = TransparencyCyclesGetArgs {
                cycle_id: format!(" 0x{} ", "AA".repeat(16)),
                limit: Some(3),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, cycle_id, filter| {
                assert_eq!(cycle_id, "aa".repeat(16));
                assert_eq!(filter.limit, Some(3));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "cycle_id_hex": "aa"
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"cycle_id_hex\""));
        }
        fn transparency_cycles_entry_normalizes_identifiers() {
            let args = TransparencyCyclesEntryArgs {
                cycle_id: format!(" 0x{} ", "AB".repeat(16)),
                entry_id: format!(" 0x{} ", "CD".repeat(16)),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, cycle_id, entry_id| {
                assert_eq!(cycle_id, "ab".repeat(16));
                assert_eq!(entry_id, "cd".repeat(16));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "entry_id_hex": "bb"
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"entry_id_hex\""));
        }
        fn transparency_explorer_prints_payload() {
            let args = TransparencyExplorerArgs { limit: Some(5) };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, filter| {
                assert_eq!(filter.limit, Some(5));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "schema": "sorafs.transparency.explorer_snapshot.v1"
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("explorer_snapshot"));
        }
        }
    fn transparency_explorer_canary_fixture_json(
        value: Value,
    ) -> Result<TransparencyExplorerCanaryHttpResponse> {
        Ok(TransparencyExplorerCanaryHttpResponse {
            status: StatusCode::OK,
            content_type: Some("application/json".to_string()),
            body: norito::json::to_vec(&value)?,
        })
    }
    fn transparency_explorer_canary_fixture_response(
        url: &str,
        include_private_key: bool,
    ) -> Result<TransparencyExplorerCanaryHttpResponse> {
        let parsed = Url::parse(url).expect("canary URL should parse");
        let path = parsed.path();
        if path.ends_with("/v1/sorafs/transparency/explorer") {
            assert_eq_compact! { parsed.query_pairs().find(|(key, _)| key == "limit").map(|(_, value)| value.into_owned()) => Some("6".to_string()) };
            let value = if include_private_key {
                norito::json!({
                    "schema": "sorafs.transparency.explorer_snapshot.v1",
                    "payload_bytes_included": false,
                    "private_digest_keys_included": false,
                    "proof_token_issuances": [
                        { "proof_token_digest_key": "must-not-ship" }
                    ]
                })
            } else {
                norito::json!({
                    "schema": "sorafs.transparency.explorer_snapshot.v1",
                    "payload_bytes_included": false,
                    "private_digest_keys_included": false,
                    "cycles": [],
                    "proof_token_issuances": []
                })
            };
            return transparency_explorer_canary_fixture_json(value);
        }
        if path.ends_with("/v1/sorafs/transparency/explorer/ui") {
            return Ok(TransparencyExplorerCanaryHttpResponse {
                status: StatusCode::OK,
                content_type: Some("text/html; charset=utf-8".to_string()),
                body: b"<main><h1>SoraFS Transparency Explorer</h1></main>".to_vec(),
            });
        }
        if path.ends_with("/v1/sorafs/transparency/tokens") {
            assert_eq_compact! { parsed.query_pairs().find(|(key, _)| key == "limit").map(|(_, value)| value.into_owned()) => Some("6".to_string()) };
            return transparency_explorer_canary_fixture_json(norito::json!({
                "schema": "sorafs.transparency.proof_token_issuances.v1",
                "payload_bytes_included": false,
                "private_digest_keys_included": false,
                "entries": []
            }));
        }
        panic!("unexpected transparency explorer canary route: {url}");
    }
    #[test]
    fn transparency_explorer_canary_builds_payload_free_evidence() {
        let out_dir = TempDir::new().expect("canary evidence dir");
        let out = out_dir.path().join("nested/evidence.json");
        let args = TransparencyExplorerCanaryArgs {
            torii_url: Some(" https://torii.test/root ".to_string()),
            limit: Some(6),
            timeout_secs: 1,
            out: Some(out.clone()),
        };
        let mut ctx = TestContext::new();
        let mut requested = Vec::new();
        args.run_with_fetch(&mut ctx, |url| {
            requested.push(url.to_string());
            transparency_explorer_canary_fixture_response(url, false)
        })
        .expect("transparency explorer canary should render evidence");
        assert_eq!(requested.len(), 3);
        assert_eq!(ctx.printed.len(), 1);
        assert!(!ctx.printed[0].contains("proof_token_digest_key"));
        let value: Value = norito::json::from_str(&ctx.printed[0]).expect("canary evidence JSON");
        let schema = value["schema"].as_str();
        assert_eq!(schema, Some("sorafs.transparency.explorer_canary.v1"));
        assert_eq!(value["status"].as_str(), Some("passed"));
        assert_eq!(value["limit"].as_u64(), Some(6));
        assert_eq!(value["route_count"].as_u64(), Some(3));
        assert_eq!(value["payload_bytes_included"].as_bool(), Some(false));
        assert_eq!(value["private_digest_keys_included"].as_bool(), Some(false));
        let routes = value["routes"].as_array().expect("canary routes");
        for name in ["browser_ui", "proof_token_issuance_index"] {
            let found = routes
                .iter()
                .any(|route| route["name"].as_str() == Some(name));
            assert!(found);
        }
        let explorer = routes
            .iter()
            .find(|route| route["name"].as_str() == Some("explorer_snapshot"))
            .expect("explorer route evidence");
        let explorer_url = explorer["url"].as_str().expect("explorer URL");
        assert!(explorer_url.contains("/root/v1/sorafs/transparency/explorer"));
        assert!(explorer_url.contains("limit=6"));
        let bytes = fs::read(out).expect("written canary evidence");
        let written: Value = norito::json::from_slice(&bytes).expect("written evidence JSON");
        assert_eq!(written["schema"], value["schema"]);
    }
    #[test]
    fn transparency_explorer_canary_rejects_private_digest_keys() {
        let args = TransparencyExplorerCanaryArgs {
            torii_url: Some("https://torii.test/root".to_string()),
            limit: Some(6),
            timeout_secs: 1,
            out: None,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with_fetch(&mut ctx, |url| {
                transparency_explorer_canary_fixture_response(url, true)
            })
            .expect_err("transparency explorer canary must reject private digest keys");
        assert!(err.to_string().contains("digest-key"));
        assert!(ctx.printed.is_empty());
    }
    fn transparency_publication_canary_fixture_response(
        url: &str,
        include_publisher_identity: bool,
        status: StatusCode,
    ) -> Result<TransparencyExplorerCanaryHttpResponse> {
        if status != StatusCode::OK {
            return Ok(TransparencyExplorerCanaryHttpResponse {
                status,
                content_type: Some("application/json".to_string()),
                body: br#"{"error":"publication route unavailable must not leak"}"#.to_vec(),
            });
        }
        let parsed = Url::parse(url).expect("publication canary URL should parse");
        assert_eq_compact! { parsed.query_pairs().find(|(key, _)| key == "limit").map(|(_, value)| value.into_owned()) => Some("3".to_string()) };
        let path = parsed.path();
        let cycle_id = "11".repeat(16);
        let publisher_labels = if include_publisher_identity {
            norito::json!({
                "publisher_peer_id": "peer-a",
                "publisher_public_key_hex": ("a1".repeat(32)),
            })
        } else {
            norito::json!({})
        };
        if path.ends_with("/v1/sorafs/transparency/cycles") {
            return transparency_explorer_canary_fixture_json(norito::json!({
                "schema": "sorafs.transparency.cycles.v1",
                "published_cycle_count": 1_u64,
                "returned_cycle_count": 1_u64,
                "limit": 3_u64,
                "truncated": false,
                "cycles": [
                    {
                        "cycle_id_hex": cycle_id,
                        "block_hash_hex": ("b2".repeat(32)),
                        "publication_hash_hex": ("c3".repeat(32)),
                        "entry_root_hex": ("d4".repeat(32)),
                        "encoded_blake3": ("e5".repeat(32)),
                        "source_entry": {
                            "labels": publisher_labels
                        }
                    }
                ]
            }));
        }
        if path.ends_with(&format!("/v1/sorafs/transparency/cycles/{cycle_id}")) {
            return transparency_explorer_canary_fixture_json(norito::json!({
                "schema": "sorafs.transparency.cycle_publication.v1",
                "cycle_id_hex": cycle_id,
                "encoded_blake3": ("e5".repeat(32)),
                "proof_count": 2_u64,
                "returned_proof_count": 1_u64,
                "limit": 3_u64,
                "truncated_proofs": true,
                "entry": {
                    "labels": publisher_labels
                },
                "verification": {
                    "valid": true,
                    "all_proofs_verified": true,
                    "block_hash_hex": ("b2".repeat(32)),
                    "publication_hash_hex": ("c3".repeat(32)),
                    "entry_root_hex": ("d4".repeat(32)),
                    "proof_count": 2_u64
                },
                "publication": {
                    "proofs": [
                        { "public_subject": "manifest-must-not-leak" }
                    ]
                }
            }));
        }
        panic!("unexpected transparency publication canary route: {url}");
    }
    test_items! {
        fn transparency_publication_canary_builds_payload_free_evidence() {
            let out_dir = TempDir::new().expect("publication canary evidence dir");
            let out = out_dir.path().join("nested/evidence.json");
            let cycle_id = "11".repeat(16);
            let args = TransparencyPublicationCanaryArgs {
                torii_url: Some(" https://torii.test/root ".to_string()),
                cycle_ids: vec![cycle_id],
                limit: Some(3),
                timeout_secs: 1,
                out: Some(out.clone()),
            };
            let mut ctx = TestContext::new();
            let mut requested = Vec::new();
            args.run_with_fetch(&mut ctx, |url| {
                requested.push(url.to_string());
                transparency_publication_canary_fixture_response(url, true, StatusCode::OK)
            })
            .expect("publication canary should render evidence");
            assert_eq!(requested.len(), 2);
            assert_eq!(ctx.printed.len(), 1);
            assert!(!ctx.printed[0].contains("manifest-must-not-leak"));
            let value: Value =
                norito::json::from_str(&ctx.printed[0]).expect("publication canary evidence JSON");
            let schema = value["schema"].as_str();
            assert_eq!(schema, Some("sorafs.transparency.publication_canary.v1"));
            assert_eq!(value["status"].as_str(), Some("passed"));
            assert_eq!(value["route_count"].as_u64(), Some(2));
            assert_eq!(value["passed_route_count"].as_u64(), Some(2));
            assert_eq!(value["cycle_detail_probe_count"].as_u64(), Some(1));
            assert_eq!(value["publication_bodies_included"].as_bool(), Some(false));
            let routes = value["routes"]
                .as_array()
                .expect("publication canary routes");
            for field in ["anchor_metadata_present", "publisher_identity_present"] {
                let all_present = routes
                    .iter()
                    .all(|route| route[field].as_bool() == Some(true));
                assert!(all_present);
            }
            let bytes = fs::read(out).expect("written publication canary evidence");
            let written: Value = norito::json::from_slice(&bytes).expect("written evidence JSON");
            assert_eq!(written["schema"], value["schema"]);
        }
        fn transparency_publication_canary_rejects_malformed_cycle_id_before_fetch() {
            let args = TransparencyPublicationCanaryArgs {
                torii_url: Some("https://torii.test/root".to_string()),
                cycle_ids: vec!["not-a-cycle-id".to_string()],
                limit: Some(3),
                timeout_secs: 1,
                out: None,
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with_fetch(&mut ctx, |_url| {
                    panic!("malformed cycle id must fail before HTTP fetch")
                })
                .expect_err("malformed cycle id must be rejected");
            assert_compact! { err.to_string().contains("--cycle-id must be a 16-byte hex string") };
            assert!(ctx.printed.is_empty());
        }
        fn transparency_publication_canary_fails_missing_publisher_identity() {
            let args = TransparencyPublicationCanaryArgs {
                torii_url: Some("https://torii.test/root".to_string()),
                cycle_ids: Vec::new(),
                limit: Some(3),
                timeout_secs: 1,
                out: None,
            };
            let mut ctx = TestContext::new();
            args.run_with_fetch(&mut ctx, |url| {
                transparency_publication_canary_fixture_response(url, false, StatusCode::OK)
            })
            .expect("publication canary should emit failed evidence");
            let value: Value =
                norito::json::from_str(&ctx.printed[0]).expect("publication canary evidence JSON");
            assert_eq!(value.get("status").and_then(Value::as_str), Some("failed"));
            assert_eq!(value["passed_route_count"].as_u64(), Some(0));
            let routes = value["routes"].as_array().expect("routes");
            assert_compact! { routes.iter().all(|route| route["publisher_identity_present"].as_bool() == Some(false)) };
        }
        fn transparency_publication_canary_records_http_failure_without_body() {
            let args = TransparencyPublicationCanaryArgs {
                torii_url: Some("https://torii.test/root".to_string()),
                cycle_ids: Vec::new(),
                limit: Some(3),
                timeout_secs: 1,
                out: None,
            };
            let mut ctx = TestContext::new();
            args.run_with_fetch(&mut ctx, |url| {
                transparency_publication_canary_fixture_response(url, true, StatusCode::BAD_GATEWAY)
            })
            .expect("HTTP failure should still emit canary evidence");
            assert_eq!(ctx.printed.len(), 1);
            assert!(!ctx.printed[0].contains("publication route unavailable"));
            let value: Value =
                norito::json::from_str(&ctx.printed[0]).expect("publication canary evidence JSON");
            assert_eq!(value.get("status").and_then(Value::as_str), Some("failed"));
            assert_eq!(value["passed_route_count"].as_u64(), Some(0));
        }
        fn transparency_tokens_prints_payload() {
            let args = TransparencyTokensArgs { limit: Some(7) };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, filter| {
                assert_eq!(filter.limit, Some(7));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "entries": [
                            { "payload_kind": "proof_token_issuance" }
                        ]
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"proof_token_issuance\""));
        }
        fn transparency_token_issuance_submit_reads_json_payload() {
            let file = write_json_file(&norito::json!({
                "token_b64": "proof-token-frame",
                "signer_key_hex": ("a1".repeat(32)),
                "evidence_digest_hex": ("b2".repeat(32)),
                "policy_digest_hex": ("c3".repeat(32)),
                "metadata": [
                    { "key": "producer", "value": "gateway-a" }
                ]
            }));
            let args = TransparencyTokenIssuanceSubmitArgs {
                payload: file.path().to_path_buf(),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, payload| {
                let value: Value = norito::json::from_slice(payload).expect("payload is json");
                assert_eq_compact! { value.get("token_b64").and_then(Value::as_str) => Some("proof-token-frame") };
                assert_eq_compact! { value.get("signer_key_hex").and_then(Value::as_str) => Some("a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1") };
    json_response_fixture!(StatusCode::ACCEPTED, &norito::json!({
                        "schema": "sorafs.transparency.proof_token_issuance.ingest.v1"
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("proof_token_issuance"));
        }
        fn transparency_token_issuance_canary_writes_payload_free_evidence() {
            let issuance_file = write_json_file(&norito::json!({
                "token_b64": "proof-token-frame-must-not-leak",
                "signer_key_hex": ("a1".repeat(32)),
                "evidence_digest_hex": ("b2".repeat(32)),
                "policy_digest_hex": ("c3".repeat(32)),
                "metadata": [
                    { "key": "producer", "value": "gateway-a" }
                ]
            }));
            let out_dir = TempDir::new().expect("proof-token issuance canary evidence dir");
            let out = out_dir.path().join("nested/evidence.json");
            let args = TransparencyTokenIssuanceCanaryArgs {
                issuances: vec![issuance_file.path().to_path_buf()],
                out: Some(out.clone()),
            };
            let mut ctx = TestContext::new();
            let mut submitted = 0_usize;
            args.run_with(&mut ctx, |_client, payload| {
                submitted += 1;
                let value: Value = norito::json::from_slice(payload).expect("issuance payload JSON");
                assert_eq_compact! { value.get("token_b64").and_then(Value::as_str) => Some("proof-token-frame-must-not-leak") };
    json_response_fixture!(StatusCode::ACCEPTED, &norito::json!({
                        "schema": "sorafs.transparency.proof_token_issuance.ingest.v1",
                        "token_id_hex": "token-id-must-not-leak"
                    }))
            })
            .expect("proof-token issuance canary should succeed");
            assert_eq!(submitted, 1);
            assert!(out.exists(), "canary evidence should be written");
            assert_eq!(ctx.printed.len(), 1);
            let evidence: Value =
                norito::json::from_str(&ctx.printed[0]).expect("canary evidence JSON");
            assert_eq_compact! { evidence.get("schema").and_then(Value::as_str) => Some("sorafs.transparency.proof_token_issuance.canary.v1") };
            assert_eq_compact! { evidence.get("status").and_then(Value::as_str) => Some("passed") };
            assert_eq!(evidence.get("probe_count").and_then(Value::as_u64), Some(1));
            assert_eq_compact! { evidence.get("passed_probe_count").and_then(Value::as_u64) => Some(1) };
            assert_eq_compact! { evidence.get("issuance_probe_count").and_then(Value::as_u64) => Some(1) };
            assert_eq_compact! { evidence.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
            assert_eq_compact! { evidence.get("proof_token_frames_included").and_then(Value::as_bool) => Some(false) };
            assert_eq_compact! { evidence.get("response_bodies_included").and_then(Value::as_bool) => Some(false) };
            assert_compact! { !ctx.printed[0].contains("proof-token-frame-must-not-leak"); "canary evidence must not include proof-token frames" };
            assert_compact! { !ctx.printed[0].contains("token-id-must-not-leak"); "canary evidence must not archive response bodies" };
        }
        fn transparency_token_issuance_canary_records_failed_probe_without_body() {
            let issuance_file = write_json_file(&norito::json!({
                "token_b64": "proof-token-frame-must-not-leak",
                "signer_key_hex": ("a1".repeat(32)),
                "evidence_digest_hex": ("b2".repeat(32)),
                "policy_digest_hex": ("c3".repeat(32)),
            }));
            let args = TransparencyTokenIssuanceCanaryArgs {
                issuances: vec![issuance_file.path().to_path_buf()],
                out: None,
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, _payload| {
    json_response_fixture!(StatusCode::BAD_GATEWAY, &norito::json!({
                        "error": "proof-token producer unavailable"
                    }))
            })
            .expect("failed probe should still emit canary evidence");
            assert_eq!(ctx.printed.len(), 1);
            let evidence: Value =
                norito::json::from_str(&ctx.printed[0]).expect("canary evidence JSON");
            assert_eq_compact! { evidence.get("status").and_then(Value::as_str) => Some("failed") };
            assert_eq_compact! { evidence.get("passed_probe_count").and_then(Value::as_u64) => Some(0) };
            assert_compact! { !ctx.printed[0].contains("proof-token producer unavailable"); "canary evidence must not archive response bodies" };
        }
        fn transparency_token_issuance_canary_rejects_empty_payload_list() {
            let args = TransparencyTokenIssuanceCanaryArgs {
                issuances: Vec::new(),
                out: None,
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with(&mut ctx, |_client, _| unreachable!("submit must not run"))
                .expect_err("missing issuance payloads must be rejected");
            assert!(err.to_string().contains("at least one --issuance"));
            assert!(ctx.printed.is_empty());
        }
        fn transparency_privacy_aggregate_source_event_reads_json_payload() {
            let mut file = NamedTempFile::new().expect("privacy aggregate source-event file");
            file.write_all(
                &norito::json::to_vec(&norito::json!({
                    "event_id": "privacy-event-1",
                    "occurred_at_unix": 1_800_000_500_u64,
                    "population_label": "moderation.global",
                    "metrics": [
                        { "key": "quarantined", "value": 3_u64 }
                    ],
                    "policy_digest_hex": ("a1".repeat(32)),
                }))
                .expect("serialize privacy aggregate source-event JSON"),
            )
            .expect("write privacy aggregate source-event JSON");
            let args = TransparencyPrivacyAggregateSourceEventArgs {
                payload: file.path().to_path_buf(),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, payload| {
                let value: Value = norito::json::from_slice(payload).expect("payload is json");
                assert_eq_compact! { value.get("event_id").and_then(Value::as_str) => Some("privacy-event-1") };
                assert_eq_compact! { value.get("population_label").and_then(Value::as_str) => Some("moderation.global") };
    json_response_fixture!(StatusCode::ACCEPTED,
                        &norito::json!({ "status": "accepted" }),
                    )
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"accepted\""));
        }
        fn transparency_privacy_aggregate_publish_due_reads_json_payload() {
            let mut file = NamedTempFile::new().expect("privacy aggregate publish-due file");
            file.write_all(
                &norito::json::to_vec(&norito::json!({
                    "now_unix": 1_800_000_800_u64,
                    "previous_block_hash_hex": ("d4".repeat(32)),
                }))
                .expect("serialize privacy aggregate publish-due JSON"),
            )
            .expect("write privacy aggregate publish-due JSON");
            let args = TransparencyPrivacyAggregatePublishDueArgs {
                payload: file.path().to_path_buf(),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, payload| {
                let value: Value = norito::json::from_slice(payload).expect("payload is json");
                assert_eq_compact! { value.get("previous_block_hash_hex").and_then(Value::as_str) => Some("d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4") };
                assert!(value.get("cycle_prf_output_hex").is_none());
    json_response_fixture!(StatusCode::OK,
                        &norito::json!({ "status": "published" }),
                    )
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"published\""));
        }
        fn transparency_privacy_aggregate_commands_reject_empty_payloads() {
            let file = NamedTempFile::new().expect("empty privacy aggregate file");
            let mut ctx = TestContext::new();
            let source_args = TransparencyPrivacyAggregateSourceEventArgs {
                payload: file.path().to_path_buf(),
            };
            let err = source_args
                .run_with(&mut ctx, |_client, _| unreachable!("submit must not run"))
                .expect_err("empty source-event payload must be rejected");
            assert!(err.to_string().contains("source-event payload"));
            let publish_args = TransparencyPrivacyAggregatePublishDueArgs {
                payload: file.path().to_path_buf(),
            };
            let err = publish_args
                .run_with(&mut ctx, |_client, _| unreachable!("submit must not run"))
                .expect_err("empty publish-due payload must be rejected");
            assert!(err.to_string().contains("publish-due payload"));
            assert!(ctx.printed.is_empty());
        }
        fn transparency_privacy_aggregate_canary_writes_payload_free_evidence() {
            let source_file = write_json_file(&norito::json!({
                "event_id": "privacy-event-1",
                "occurred_at_unix": 1_800_000_500_u64,
                "population_label": "moderation.global",
                "subject_digest_hex": ("b2".repeat(32)),
                "metrics": [
                    { "key": "quarantined", "value": 3_u64, "unit": "count" }
                ],
                "policy_digest_hex": ("a1".repeat(32)),
            }));
            let publish_file = write_json_file(&norito::json!({
                "now_unix": 1_800_000_800_u64,
                "previous_block_hash_hex": ("d4".repeat(32)),
            }));
            let out_dir = TempDir::new().expect("privacy aggregate canary evidence dir");
            let out = out_dir.path().join("nested/evidence.json");
            let args = TransparencyPrivacyAggregateCanaryArgs {
                source_events: vec![source_file.path().to_path_buf()],
                publish_due: vec![publish_file.path().to_path_buf()],
                out: Some(out.clone()),
            };
            let mut ctx = TestContext::new();
            let mut submitted_source = 0_usize;
            let mut submitted_publish = 0_usize;
            args.run_with(
                &mut ctx,
                |_client, payload| {
                    submitted_source += 1;
                    let value: Value = norito::json::from_slice(payload).expect("source payload JSON");
                    assert_eq_compact! { value.get("event_id").and_then(Value::as_str) => Some("privacy-event-1") };
    json_response_fixture!(StatusCode::ACCEPTED, &norito::json!({
                            "status": "accepted",
                            "event_id": "privacy-event-1"
                        }))
                },
                |_client, payload| {
                    submitted_publish += 1;
                    let value: Value = norito::json::from_slice(payload).expect("publish payload JSON");
                    assert_eq_compact! { value.get("previous_block_hash_hex").and_then(Value::as_str) => Some("d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4") };
                    assert!(value.get("cycle_prf_output_hex").is_none());
    json_response_fixture!(StatusCode::OK, &norito::json!({
                            "status": "published",
                            "cycle_id_hex": "aa"
                        }))
                },
            )
            .expect("privacy aggregate canary should succeed");
            assert_eq!(submitted_source, 1);
            assert_eq!(submitted_publish, 1);
            assert!(out.exists(), "canary evidence should be written");
            assert_eq!(ctx.printed.len(), 1);
            let evidence: Value =
                norito::json::from_str(&ctx.printed[0]).expect("canary evidence JSON");
            assert_eq_compact! { evidence.get("schema").and_then(Value::as_str) => Some("sorafs.transparency.privacy_aggregate.canary.v1") };
            assert_eq_compact! { evidence.get("status").and_then(Value::as_str) => Some("passed") };
            assert_eq!(evidence.get("probe_count").and_then(Value::as_u64), Some(2));
            assert_eq_compact! { evidence.get("passed_probe_count").and_then(Value::as_u64) => Some(2) };
            assert_eq_compact! { evidence.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
            assert_eq_compact! { evidence.get("raw_metric_values_included").and_then(Value::as_bool) => Some(false) };
            assert_compact! { !ctx.printed[0].contains("\"metrics\""); "canary evidence must not include raw metric arrays" };
            assert_compact! { !ctx.printed[0].contains("\"quarantined\""); "canary evidence must not include raw metric names" };
        }
        fn transparency_privacy_aggregate_canary_records_failed_probe_without_body() {
            let publish_file = write_json_file(&norito::json!({
                "now_unix": 1_800_000_800_u64,
                "aggregate_id_prefix": "moderation",
                "privacy_mode": "suppression",
                "suppression_threshold": 4_u64,
            }));
            let args = TransparencyPrivacyAggregateCanaryArgs {
                source_events: Vec::new(),
                publish_due: vec![publish_file.path().to_path_buf()],
                out: None,
            };
            let mut ctx = TestContext::new();
            args.run_with(
                &mut ctx,
                |_client, _payload| unreachable!("source-event submit must not run"),
                |_client, _payload| {
    json_response_fixture!(StatusCode::BAD_GATEWAY, &norito::json!({
                            "error": "scheduler unavailable"
                        }))
                },
            )
            .expect("failed probe should still emit canary evidence");
            assert_eq!(ctx.printed.len(), 1);
            let evidence: Value =
                norito::json::from_str(&ctx.printed[0]).expect("canary evidence JSON");
            assert_eq_compact! { evidence.get("status").and_then(Value::as_str) => Some("failed") };
            assert_eq_compact! { evidence.get("passed_probe_count").and_then(Value::as_u64) => Some(0) };
            assert_compact! { !ctx.printed[0].contains("scheduler unavailable"); "canary evidence must not archive response bodies" };
        }
        }
    fn write_json_file(value: &Value) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("json file");
        file.write_all(&norito::json::to_vec(value).expect("serialize json"))
            .expect("write json file");
        file
    }
    test_items! {
        fn appeals_pricing_quote_reads_json_payload() {
            let file = write_json_file(&norito::json!({
                "class": "content",
                "backlog": 4_u64,
                "evidence_size_mb": 12_u64,
                "urgency": "normal"
            }));
            let args = AppealsPricingQuoteArgs {
                input: file.path().to_path_buf(),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, payload| {
                let value: Value = norito::json::from_slice(payload).expect("payload is json");
                assert_eq!(value.get("class").and_then(Value::as_str), Some("content"));
    json_response_fixture!(StatusCode::OK,
                        &norito::json!({ "deposit_xor": "123" }),
                    )
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"123\""));
        }
        fn appeals_finance_deposit_create_reads_json_payload() {
            let file = write_json_file(&norito::json!({
                "case_id": "case-401",
                "payer_account": "payer",
                "destination_account": "treasury",
                "asset_definition_id": "xor#wonderland",
                "deposit_xor": "100",
                "idempotency_key": "case-401-round-7"
            }));
            let args = AppealsFinanceDepositCreateArgs {
                input: file.path().to_path_buf(),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, payload| {
                let value: Value = norito::json::from_slice(payload).expect("payload is json");
                assert_eq_compact! { value.get("case_id").and_then(Value::as_str) => Some("case-401") };
    json_response_fixture!(StatusCode::OK,
                        &norito::json!({ "status": "deposit_instruction" }),
                    )
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("deposit_instruction"));
        }
        fn appeals_finance_deposit_get_trims_escrow_id() {
            let args = AppealsFinanceDepositGetArgs {
                escrow_id: " 0xAAAA ".to_string(),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, escrow_id| {
                assert_eq!(escrow_id, "0xAAAA");
    json_response_fixture!(StatusCode::OK,
                        &norito::json!({ "escrow_id_hex": "aa" }),
                    )
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("escrow_id_hex"));
        }
        fn appeals_finance_deposit_submit_settlement_accepts_accepted_status() {
            let file = write_json_file(&norito::json!({
                "deposit_confirmation": {
                    "escrow_id_hex": ("11".repeat(32)),
                    "case_id": "case-401",
                    "payer_account": "payer",
                    "destination_account": "treasury",
                    "asset_definition_id": "xor#wonderland",
                    "deposit_xor": "100",
                    "idempotency_key": "case-401-round-7"
                },
                "outcome": "uphold"
            }));
            let args = AppealsFinanceDepositSubmitSettlementArgs {
                input: file.path().to_path_buf(),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, payload| {
                let value: Value = norito::json::from_slice(payload).expect("payload is json");
                assert_eq!(value.get("outcome").and_then(Value::as_str), Some("uphold"));
    json_response_fixture!(StatusCode::ACCEPTED,
                        &norito::json!({ "status": "queued" }),
                    )
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("queued"));
        }
        fn appeals_finance_reports_list_prints_payload() {
            let args = AppealsFinanceReportsArgs { limit: Some(5) };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, filter| {
                assert_eq!(filter.limit, Some(5));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "entries": [
                            { "payload_kind": "appeal_finance_report" }
                        ]
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("appeal_finance_report"));
        }
        fn appeals_finance_deposit_create_rejects_empty_payload() {
            let file = NamedTempFile::new().expect("empty payload");
            let args = AppealsFinanceDepositCreateArgs {
                input: file.path().to_path_buf(),
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with(&mut ctx, |_client, _| unreachable!("submit must not run"))
                .expect_err("empty payload must be rejected");
            assert!(err.to_string().contains("appeal finance deposit payload"));
            assert!(ctx.printed.is_empty());
        }
        }
    fn signed_moderation_repro_manifest_fixture() -> ModerationReproManifestV1 {
        use iroha_data_model::sorafs::moderation::{
            MODERATION_REPRO_MANIFEST_VERSION_V1, ModerationModelFingerprintV1,
            ModerationReproBodyV1, ModerationReproSignatureV1, ModerationSeedMaterialV1,
            ModerationThresholdsV1,
        };
        let mut body = ModerationReproBodyV1 {
            schema_version: MODERATION_REPRO_MANIFEST_VERSION_V1,
            manifest_id: [0xA1; 16],
            manifest_digest: [0xB2; 32],
            runner_hash: [0xC3; 32],
            runtime_version: "sorafs-ai-runner cli-test".to_string(),
            issued_at_unix: 1_800_000_000,
            seed_material: ModerationSeedMaterialV1 {
                domain_tag: "sfm4a:cli-test".to_string(),
                seed_version: 1,
                run_nonce: [0xD4; 32],
            },
            thresholds: ModerationThresholdsV1 {
                quarantine: 6_000,
                escalate: 8_500,
            },
            models: vec![ModerationModelFingerprintV1 {
                model_id: [0x11; 16],
                artifact_path: "models/model-11.norito".to_string(),
                artifact_bytes: 1,
                artifact_digest: [0x22; 32],
                weights_digest: [0x33; 32],
                engine: iroha_data_model::sorafs::moderation::ModerationModelEngineV1::DeterministicLinearV1,
                feature_profile: iroha_data_model::sorafs::moderation::ModerationFeatureProfileV1::ByteHistogramAndBigramV1,
                calibration_knot_count: 2,
                max_input_bytes: 1024,
                max_operations: 3073,
                working_memory_bytes: 4096,
                weight: Some(10_000),
            }],
            notes: Some("cli registry fixture".to_string()),
        };
        body.refresh_manifest_digest()
            .expect("refresh moderation fixture digest");
        let keypair = KeyPair::try_from_seed(vec![0xE5; 32], Algorithm::Ed25519)
            .expect("derive moderation fixture keypair");
        let signature = iroha_crypto::SignatureOf::try_new(keypair.private_key(), &body)
            .expect("sign moderation fixture body");
        ModerationReproManifestV1 {
            body,
            signatures: vec![ModerationReproSignatureV1 {
                role: "council".to_string(),
                public_key: keypair.public_key().clone(),
                signature,
            }],
        }
    }
    fn adversarial_corpus_manifest_fixture() -> AdversarialCorpusManifestV1 {
        use iroha_data_model::sorafs::moderation::{
            ADVERSARIAL_CORPUS_VERSION_V1, AdversarialPerceptualFamilyV1,
            AdversarialPerceptualVariantV1,
        };
        AdversarialCorpusManifestV1 {
            schema_version: ADVERSARIAL_CORPUS_VERSION_V1,
            issued_at_unix: 1_800_000_100,
            cohort_label: Some("cli-registry-fixture".to_string()),
            families: vec![AdversarialPerceptualFamilyV1 {
                family_id: [0x44; 16],
                description: "jpeg jitter corpus".to_string(),
                variants: vec![AdversarialPerceptualVariantV1 {
                    variant_id: [0x55; 16],
                    attack_vector: "jpeg_jitter".to_string(),
                    reference_cid_b64: None,
                    perceptual_hash: Some([0x66; 32]),
                    hamming_radius: 8,
                    embedding_digest: None,
                    notes: Some("cli registry variant".to_string()),
                }],
            }],
        }
    }
    fn moderation_ballot_reveal_fixture() -> SoraFsModerationBallotRevealV1 {
        use iroha_data_model::sorafs::moderation::{
            SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
            SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1, SoraFsModerationBallotContextV1,
            SoraFsModerationVoteChoice,
        };
        SoraFsModerationBallotRevealV1 {
            version: SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1,
            context: SoraFsModerationBallotContextV1 {
                version: SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
                case_id: "case-401".to_string(),
                evidence_bundle_digest: [0xA1; 32],
                appeal_finance_config_version: "appeal-fee-v1".to_string(),
                panel_roster_hash: [0xB2; 32],
                policy_reference: "moderation-policy-v1".to_string(),
                evidence_uri: Some("dag://evidence/case-401".to_string()),
            },
            round_id: "round-7".to_string(),
            juror_id: "juror-1@moderation".to_string(),
            choice: SoraFsModerationVoteChoice::Overturn,
            nonce: vec![0xC3; 32],
            revealed_at_unix_ms: 0,
        }
    }
    fn moderation_ballot_reveal_fixture_for_juror(
        juror_id: &str,
    ) -> SoraFsModerationBallotRevealV1 {
        let mut reveal = moderation_ballot_reveal_fixture();
        reveal.juror_id = juror_id.to_string();
        reveal
    }
    fn moderation_ballot_commit_fixture_for_juror(
        juror_id: &str,
    ) -> SoraFsModerationBallotCommitV1 {
        use iroha_data_model::sorafs::moderation::SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1;
        let reveal = moderation_ballot_reveal_fixture_for_juror(juror_id);
        SoraFsModerationBallotCommitV1 {
            version: SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1,
            context: reveal.context.clone(),
            round_id: reveal.round_id.clone(),
            juror_id: reveal.juror_id.clone(),
            commitment_blake2b_256: reveal.compute_commitment(),
            committed_at_unix_ms: 0,
        }
    }
    fn moderation_commit_from_transaction(
        transaction: &SignedTransaction,
    ) -> SoraFsModerationBallotCommitV1 {
        let iroha_data_model::transaction::Executable::Instructions(instructions) =
            transaction.instructions()
        else {
            panic!("moderation commit transaction must contain instructions");
        };
        assert_eq!(instructions.len(), 1);
        let instruction = instructions[0]
            .as_any()
            .downcast_ref::<SubmitSorafsModerationCommit>()
            .expect("native moderation commit instruction");
        decode_from_bytes(instruction.commit_payload()).expect("decode embedded moderation commit")
    }
    fn moderation_reveal_from_transaction(
        transaction: &SignedTransaction,
    ) -> SoraFsModerationBallotRevealV1 {
        let iroha_data_model::transaction::Executable::Instructions(instructions) =
            transaction.instructions()
        else {
            panic!("moderation reveal transaction must contain instructions");
        };
        assert_eq!(instructions.len(), 1);
        let instruction = instructions[0]
            .as_any()
            .downcast_ref::<SubmitSorafsModerationReveal>()
            .expect("native moderation reveal instruction");
        decode_from_bytes(instruction.reveal_payload()).expect("decode embedded moderation reveal")
    }
    fn moderation_finalization_from_transaction(
        transaction: &SignedTransaction,
    ) -> &FinalizeSorafsModerationCase {
        let iroha_data_model::transaction::Executable::Instructions(instructions) =
            transaction.instructions()
        else {
            panic!("moderation finalization transaction must contain instructions");
        };
        assert_eq!(instructions.len(), 1);
        instructions[0]
            .as_any()
            .downcast_ref::<FinalizeSorafsModerationCase>()
            .expect("native moderation finalization instruction")
    }
    fn write_commit_reveal_status_file(
        missing_commit_jurors: &[&str],
        missing_reveal_jurors: &[&str],
        ready_to_tally: bool,
    ) -> NamedTempFile {
        let status = norito::json!({
            "schema": "sorafs.moderation.quarantine.commit_reveal_status.v1",
            "status": "coordinated",
            "payload_bytes_included": false,
            "private_payloads_included": false,
            "ballots": [{
                "case_id": "case-401",
                "round_id": "round-7",
                "missing_commit_jurors": (
                    missing_commit_jurors
                        .iter()
                        .copied()
                        .map(Value::from)
                        .collect::<Vec<_>>()
                ),
                "missing_reveal_jurors": (
                    missing_reveal_jurors
                        .iter()
                        .copied()
                        .map(Value::from)
                        .collect::<Vec<_>>()
                ),
                "ready_to_tally": (ready_to_tally)
            }]
        });
        write_json_file(&status)
    }
    fn juror_notifications_manifest_fixture(private_payload_included: bool) -> Value {
        norito::json!({
            "schema": "sorafs.moderation.quarantine.juror_notifications.v1",
            "source": "juror-plan",
            "status": "ready",
            "quarantine_id_hex": "abababababababababababababababab",
            "planned_juror_count": 1_u64,
            "notification_count": 1_u64,
            "skipped_complete_count": 0_u64,
            "pending_commit_count": 1_u64,
            "pending_reveal_count": 0_u64,
            "delivery_transport": "operator-managed",
            "delivery_semantics": "at-least-once-with-dedup-key",
            "payload_bytes_included": false,
            "private_payloads_included": false,
            "notifications": [{
                "schema": "sorafs.moderation.juror_notification.v1",
                "delivery_id": "notify-1",
                "dedup_key": "sorafs-moderation-juror:notify-1",
                "delivery_status": "ready_for_delivery",
                "delivery_transport": "operator-managed",
                "quarantine_id_hex": "abababababababababababababababab",
                "case_id": "case-401",
                "round_id": "round-7",
                "juror_id": "juror-1@moderation",
                "signed_by": "juror-1@moderation",
                "action": "submit_commit",
                "notification_status": "commit_required",
                "route": "/v1/sorafs/moderation/ballots/commits",
                "cli": ["iroha", "sorafs", "moderation", "ballots", "commit"],
                "subject": "SoraFS moderation commit required",
                "body": "Build the private commit payload locally.",
                "deadline_unix_ms": 1_800_000_200_000_u64,
                "evidence_uri": "dag://evidence/case-401",
                "payload_bytes_included": false,
                "private_payload_included": (private_payload_included),
                "private_payload_source": "juror-local"
            }]
        })
    }
    test_items! {
        fn moderation_ballots_list_prints_payload() {
            let args = ModerationBallotsListArgs { limit: Some(8) };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, filter| {
                assert_eq!(filter.limit, Some(8));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "ballots": [
                            { "case_id": "case-401", "round_id": "round-7" }
                        ]
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"ballots\""));
        }
        fn moderation_ballots_get_trims_identifiers() {
            let args = ModerationBallotsGetArgs {
                case_id: " case-401 ".to_string(),
                round_id: " round-7 ".to_string(),
                limit: Some(3),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, case_id, round_id, filter| {
                assert_eq!(case_id, "case-401");
                assert_eq!(round_id, "round-7");
                assert_eq!(filter.limit, Some(3));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "case_id": "case-401",
                        "round_id": "round-7"
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"case-401\""));
        }
        fn moderation_ballots_no_show_plan_trims_identifiers_and_prints_payload() {
            let args = ModerationBallotsNoShowPlanArgs {
                case_id: " case-401 ".to_string(),
                round_id: " round-7 ".to_string(),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, case_id, round_id| {
                assert_eq!(case_id, "case-401");
                assert_eq!(round_id, "round-7");
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "schema": "sorafs.moderation.ballot.no_show_plan.v1",
                        "case_id": "case-401",
                        "round_id": "round-7",
                        "no_show_count": 2,
                        "penalty_plan_digest_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"no_show_count\""));
            assert!(ctx.printed[0].contains("\"penalty_plan_digest_hex\""));
        }
        fn moderation_ballots_events_prints_payload() {
            let args = ModerationBallotsEventsArgs {
                since: Some(12),
                limit: Some(4),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, filter| {
                assert_eq!(filter.since, Some(12));
                assert_eq!(filter.limit, Some(4));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "events": [
                            { "sequence": 13, "kind": "commit_accepted" }
                        ]
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"events\""));
        }
        fn moderation_ballots_commit_reads_json_payload() {
            let mut ctx = TestContext::new();
            let commit = moderation_ballot_commit_fixture_for_juror(&ctx.cfg.account.to_string());
            let mut file = NamedTempFile::new().expect("commit file");
            file.write_all(
                norito::json::to_json_pretty(&commit)
                    .expect("render commit json")
                    .as_bytes(),
            )
            .expect("write commit json");
            let args = ModerationBallotsCommitArgs {
                payload: file.path().to_path_buf(),
                format: "json".to_string(),
            };
            args.run_with(&mut ctx, |_client, transaction| {
                assert_eq!(moderation_commit_from_transaction(transaction), commit);
                Ok(transaction.hash())
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"transaction_hash_hex\""));
        }
        fn moderation_ballots_reveal_reads_norito_payload() {
            let mut ctx = TestContext::new();
            let reveal = moderation_ballot_reveal_fixture_for_juror(&ctx.cfg.account.to_string());
            let encoded = to_bytes(&reveal).expect("encode reveal");
            let mut file = NamedTempFile::new().expect("reveal file");
            file.write_all(&encoded).expect("write reveal norito");
            let args = ModerationBallotsRevealArgs {
                payload: file.path().to_path_buf(),
                format: "norito".to_string(),
            };
            args.run_with(&mut ctx, |_client, transaction| {
                assert_eq!(moderation_reveal_from_transaction(transaction), reveal);
                Ok(transaction.hash())
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"transaction_hash_hex\""));
        }
        fn moderation_ballots_tally_builds_request() {
            let args = ModerationBallotsTallyArgs {
                case_id: " case-401 ".to_string(),
                round_id: " round-7 ".to_string(),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, transaction| {
                let instruction = moderation_finalization_from_transaction(transaction);
                assert_eq!(instruction.case_id(), "case-401");
                assert_eq!(instruction.round_id(), "round-7");
                Ok(transaction.hash())
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"transaction_hash_hex\""));
        }
        fn moderation_ballots_commit_rejects_invalid_format() {
            let file = NamedTempFile::new().expect("commit file");
            let args = ModerationBallotsCommitArgs {
                payload: file.path().to_path_buf(),
                format: "yaml".to_string(),
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with(&mut ctx, |_client, _| unreachable!("submit must not run"))
                .expect_err("invalid format must be rejected");
            assert!(err.to_string().contains("--format"));
            assert!(ctx.printed.is_empty());
        }
        fn moderation_native_action_and_coordination_inputs_are_bounded() {
            let action = NamedTempFile::new().expect("action payload file");
            action
                .as_file()
                .set_len((MODERATION_NATIVE_ACTION_INPUT_MAX_BYTES_V1 + 1) as u64)
                .expect("extend action payload");
            let action_err = read_moderation_ballot_payload_file(action.path())
                .expect_err("oversized native action input must be rejected");
            assert!(action_err.to_string().contains("between 1 and"));
            let status = NamedTempFile::new().expect("coordination status file");
            status
                .as_file()
                .set_len((MODERATION_COORDINATION_STATUS_MAX_BYTES_V1 + 1) as u64)
                .expect("extend coordination status");
            let status_err = load_moderation_commit_reveal_status_payload(status.path())
                .expect_err("oversized coordination input must be rejected");
            assert!(status_err.to_string().contains("between 1 and"));
        }
        fn moderation_native_juror_actions_reject_caller_timestamps() {
            let ctx = TestContext::new();
            let client = ctx.client_from_config();
            let juror_id = client.account.to_string();
            let mut commit = moderation_ballot_commit_fixture_for_juror(&juror_id);
            commit.committed_at_unix_ms = 1;
            let commit_err = build_moderation_commit_transaction(&client, &commit)
                .expect_err("caller-supplied commit timestamp must be rejected");
            assert!(commit_err.to_string().contains("must be zero"));
            let mut reveal = moderation_ballot_reveal_fixture_for_juror(&juror_id);
            reveal.revealed_at_unix_ms = 1;
            let reveal_err = build_moderation_reveal_transaction(&client, &reveal)
                .expect_err("caller-supplied reveal timestamp must be rejected");
            assert!(reveal_err.to_string().contains("must be zero"));
        }
        fn moderation_native_juror_actions_require_transaction_authority() {
            let ctx = TestContext::new();
            let client = ctx.client_from_config();
            let commit = moderation_ballot_commit_fixture_for_juror("other-juror@moderation");
            let commit_err = build_moderation_commit_transaction(&client, &commit)
                .expect_err("substituted commit juror must be rejected");
            assert!(commit_err.to_string().contains("transaction authority"));
            let reveal = moderation_ballot_reveal_fixture_for_juror("other-juror@moderation");
            let reveal_err = build_moderation_reveal_transaction(&client, &reveal)
                .expect_err("substituted reveal juror must be rejected");
            assert!(reveal_err.to_string().contains("transaction authority"));
        }
        fn moderation_ballots_execute_submits_pending_actions_payload_free() {
            let mut ctx = TestContext::new();
            let juror_id = ctx.cfg.account.to_string();
            let commit = moderation_ballot_commit_fixture_for_juror(&juror_id);
            let reveal = moderation_ballot_reveal_fixture_for_juror(&juror_id);
            let mut commit_file = NamedTempFile::new().expect("commit file");
            commit_file
                .write_all(
                    norito::json::to_json_pretty(&commit)
                        .expect("render commit json")
                        .as_bytes(),
                )
                .expect("write commit json");
            let mut reveal_file = NamedTempFile::new().expect("reveal file");
            reveal_file
                .write_all(&to_bytes(&reveal).expect("encode reveal"))
                .expect("write reveal norito");
            let status_file =
                write_commit_reveal_status_file(&[juror_id.as_str()], &[juror_id.as_str()], true);
            let args = ModerationBallotsExecuteArgs {
                status: status_file.path().to_path_buf(),
                commit_payloads: vec![commit_file.path().to_path_buf()],
                reveal_payloads: vec![reveal_file.path().to_path_buf()],
                commit_format: "json".to_string(),
                reveal_format: "norito".to_string(),
                submit_tally: true,
            };
            let mut committed = Vec::new();
            let mut revealed = Vec::new();
            let mut tallied = Vec::new();
            args.run_with(
                &mut ctx,
                |_client, transaction| {
                    committed.push(moderation_commit_from_transaction(transaction).juror_id);
                    Ok(transaction.hash())
                },
                |_client, transaction| {
                    revealed.push(moderation_reveal_from_transaction(transaction).juror_id);
                    Ok(transaction.hash())
                },
                |_client, transaction| {
                    let instruction = moderation_finalization_from_transaction(transaction);
                    tallied.push((
                        instruction.case_id().to_string(),
                        instruction.round_id().to_string(),
                    ));
                    Ok(transaction.hash())
                },
            )
            .expect("execution should succeed");
            assert_eq!(committed, vec![juror_id.clone()]);
            assert_eq!(revealed, vec![juror_id]);
            assert_eq_compact! { tallied => vec![("case-401".to_string(), "round-7".to_string())] };
            assert_eq!(ctx.printed.len(), 1);
            let summary: Value =
                norito::json::from_str(&ctx.printed[0]).expect("execution summary JSON");
            assert_eq_compact! { summary.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.ballots.execution.v1") };
            assert_eq!(summary.get("action_count").and_then(Value::as_u64), Some(3));
            assert_eq_compact! { summary.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
            assert_eq_compact! { summary.get("private_payloads_included").and_then(Value::as_bool) => Some(false) };
            assert_compact! { !ctx.printed[0].contains("nonce"); "execution summary must not print reveal payload internals" };
        }
        fn moderation_ballots_execute_rejects_non_pending_commit() {
            let commit = moderation_ballot_commit_fixture_for_juror("juror-1@moderation");
            let mut commit_file = NamedTempFile::new().expect("commit file");
            commit_file
                .write_all(
                    norito::json::to_json_pretty(&commit)
                        .expect("render commit json")
                        .as_bytes(),
                )
                .expect("write commit json");
            let status_file = write_commit_reveal_status_file(&["juror-other@moderation"], &[], false);
            let args = ModerationBallotsExecuteArgs {
                status: status_file.path().to_path_buf(),
                commit_payloads: vec![commit_file.path().to_path_buf()],
                reveal_payloads: Vec::new(),
                commit_format: "json".to_string(),
                reveal_format: "json".to_string(),
                submit_tally: false,
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with(
                    &mut ctx,
                    |_client, _| unreachable!("commit submit must not run"),
                    |_client, _| unreachable!("reveal submit must not run"),
                    |_client, _| unreachable!("tally submit must not run"),
                )
                .expect_err("non-pending commit must be rejected");
            assert!(err.to_string().contains("not pending in --status"));
            assert!(ctx.printed.is_empty());
        }
        fn moderation_ballots_executor_bundle_writes_supervised_job_payload_free() {
            let temp = TempDir::new().expect("executor bundle temp dir");
            let bundle_dir = temp.path().join("executor-bundle");
            let status_path = temp.path().join("runtime/commit-reveal-status.json");
            let commit_path = temp.path().join("private/commit.json");
            let reveal_path = temp.path().join("private/reveal.to");
            let args = ModerationBallotsExecutorBundleArgs {
                status: status_path.clone(),
                bundle_out: bundle_dir.clone(),
                commit_payloads: vec![commit_path.clone()],
                reveal_payloads: vec![reveal_path.clone()],
                commit_format: "json".to_string(),
                reveal_format: "norito".to_string(),
                submit_tally: true,
                iroha_bin: "/usr/local/bin/iroha".to_string(),
                service_name: "org.sora.sorafs.ballots-executor-test".to_string(),
                service_user: "sorafs-exec".to_string(),
                service_group: "sorafs-exec".to_string(),
                interval_secs: 30,
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx)
                .expect("executor bundle should be written");
            assert_eq!(ctx.printed.len(), 1);
            let summary: Value =
                norito::json::from_str(&ctx.printed[0]).expect("executor bundle summary JSON");
            assert_eq_compact! { summary.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.ballots.executor_bundle.v1") };
            assert_eq_compact! { summary.get("commit_payload_count").and_then(Value::as_u64) => Some(1) };
            assert_eq_compact! { summary.get("reveal_payload_count").and_then(Value::as_u64) => Some(1) };
            assert_eq_compact! { summary.get("submit_tally").and_then(Value::as_bool) => Some(true) };
            assert_eq_compact! { summary.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
            assert_eq_compact! { summary.get("private_payloads_included").and_then(Value::as_bool) => Some(false) };
            assert_eq_compact! { summary.get("private_payload_files_copied").and_then(Value::as_bool) => Some(false) };
            let run_script = fs::read_to_string(bundle_dir.join("run.sh")).expect("read run script");
            assert!(run_script.contains("sorafs moderation ballots execute"));
            assert!(run_script.contains("--submit-tally"));
            assert!(run_script.contains(&format!("--commit-payload='{}'", commit_path.display())));
            assert!(run_script.contains(&format!("--reveal-payload='{}'", reveal_path.display())));
            assert!(!run_script.contains("nonce"));
            let env = fs::read_to_string(bundle_dir.join("executor.env")).expect("read env");
            assert!(env.contains("IROHA_BIN='/usr/local/bin/iroha'"));
            assert_compact! { env.contains(&format!( "SORAFS_BALLOTS_EXECUTOR_STATUS_PATH='{}'", status_path.display() )) };
            assert!(!env.contains("commitment_blake2b_256"));
            let systemd =
                fs::read_to_string(bundle_dir.join("org.sora.sorafs.ballots-executor-test.service"))
                    .expect("read systemd unit");
            assert!(systemd.contains("Type=oneshot"));
            assert!(systemd.contains("NoNewPrivileges=true"));
            let timer =
                fs::read_to_string(bundle_dir.join("org.sora.sorafs.ballots-executor-test.timer"))
                    .expect("read systemd timer");
            assert!(timer.contains("OnUnitActiveSec=30s"));
            let launchd =
                fs::read_to_string(bundle_dir.join("org.sora.sorafs.ballots-executor-test.plist"))
                    .expect("read launchd plist");
            assert!(launchd.contains("<key>StartInterval</key>"));
            assert!(launchd.contains("<integer>30</integer>"));
            let metadata: Value =
                norito::json::from_slice(&fs::read(bundle_dir.join("bundle.json")).expect("metadata"))
                    .expect("metadata JSON");
            assert_eq_compact! { metadata.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.ballots.executor_bundle.v1") };
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                let mode = fs::metadata(bundle_dir.join("run.sh"))
                    .expect("run script metadata")
                    .permissions()
                    .mode();
                assert_ne!(mode & 0o111, 0, "run.sh should be executable");
            }
        }
        fn moderation_ballots_executor_bundle_rejects_empty_action_set() {
            let temp = TempDir::new().expect("executor bundle temp dir");
            let args = ModerationBallotsExecutorBundleArgs {
                status: temp.path().join("status.json"),
                bundle_out: temp.path().join("executor-bundle"),
                commit_payloads: Vec::new(),
                reveal_payloads: Vec::new(),
                commit_format: "json".to_string(),
                reveal_format: "json".to_string(),
                submit_tally: false,
                iroha_bin: "iroha".to_string(),
                service_name: "org.sora.sorafs.ballots-executor-test".to_string(),
                service_user: "sorafs-exec".to_string(),
                service_group: "sorafs-exec".to_string(),
                interval_secs: 60,
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with(&mut ctx)
                .expect_err("empty executor bundle action set must be rejected");
            assert!(err.to_string().contains("at least one --commit-payload"));
            assert!(ctx.printed.is_empty());
            assert_compact! { !temp.path().join("executor-bundle").exists(); "bundle directory must not be created on validation failure" };
        }
        fn moderation_ballots_executor_canary_writes_payload_free_evidence() {
            let temp = TempDir::new().expect("executor canary temp dir");
            let bundle_dir = temp.path().join("executor-bundle");
            let bundle_args = ModerationBallotsExecutorBundleArgs {
                status: temp.path().join("runtime/commit-reveal-status.json"),
                bundle_out: bundle_dir.clone(),
                commit_payloads: vec![temp.path().join("private/commit.json")],
                reveal_payloads: vec![temp.path().join("private/reveal.to")],
                commit_format: "json".to_string(),
                reveal_format: "norito".to_string(),
                submit_tally: true,
                iroha_bin: "/usr/local/bin/iroha".to_string(),
                service_name: "org.sora.sorafs.ballots-executor-test".to_string(),
                service_user: "sorafs-exec".to_string(),
                service_group: "sorafs-exec".to_string(),
                interval_secs: 30,
            };
            let mut setup_ctx = TestContext::new();
            bundle_args
                .run_with(&mut setup_ctx)
                .expect("executor bundle should be written");
            let execution_summary = write_json_file(&norito::json!({
                "schema": "sorafs.moderation.ballots.execution.v1",
                "source": "commit-reveal-status",
                "status": "executed",
                "action_count": 2_u64,
                "commit_action_count": 1_u64,
                "reveal_action_count": 0_u64,
                "tally_action_count": 1_u64,
                "payload_bytes_included": false,
                "private_payloads_included": false,
                "actions": [{
                    "action": "commit",
                    "case_id": "case-401",
                    "round_id": "round-7",
                    "juror_id": "juror-1@moderation",
                    "transaction_hash_hex": ("ab".repeat(32)),
                    "payload_bytes_included": false,
                    "private_payloads_included": false
                }, {
                    "action": "tally",
                    "case_id": "case-401",
                    "round_id": "round-7",
                    "juror_id": null,
                    "transaction_hash_hex": ("cd".repeat(32)),
                    "payload_bytes_included": false,
                    "private_payloads_included": false
                }]
            }));
            let out = temp.path().join("nested/executor-canary.json");
            let args = ModerationBallotsExecutorCanaryArgs {
                bundle: bundle_dir.clone(),
                execution_summary: Some(execution_summary.path().to_path_buf()),
                out: Some(out.clone()),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx)
                .expect("executor canary should emit evidence");
            assert_eq!(ctx.printed.len(), 1);
            assert!(out.exists(), "executor canary evidence should be written");
            let evidence: Value =
                norito::json::from_str(&ctx.printed[0]).expect("executor canary evidence JSON");
            assert_eq_compact! { evidence.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.ballots.executor_canary.v1") };
            assert_eq_compact! { evidence.get("status").and_then(Value::as_str) => Some("passed") };
            assert_eq_compact! { evidence.get("artifact_count").and_then(Value::as_u64) => Some(7) };
            assert_eq_compact! { evidence.get("passed_artifact_count").and_then(Value::as_u64) => Some(7) };
            assert_eq_compact! { evidence.get("execution_summary_present").and_then(Value::as_bool) => Some(true) };
            assert_eq_compact! { evidence.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
            assert_eq_compact! { evidence.get("private_payloads_included").and_then(Value::as_bool) => Some(false) };
            assert_compact! { !ctx.printed[0].contains("payload_b64"); "canary evidence must not include payload bytes" };
            assert_compact! { !ctx.printed[0].contains("nonce"); "canary evidence must not include reveal internals" };
            let artifacts = evidence
                .get("artifacts")
                .and_then(Value::as_array)
                .expect("artifact probes");
            assert_compact! { artifacts.iter().any(|artifact| artifact.get("kind").and_then(Value::as_str) == Some("run_script")) };
        }
        fn moderation_ballots_executor_canary_rejects_payload_bearing_summary() {
            let temp = TempDir::new().expect("executor canary temp dir");
            let bundle_dir = temp.path().join("executor-bundle");
            let bundle_args = ModerationBallotsExecutorBundleArgs {
                status: temp.path().join("runtime/commit-reveal-status.json"),
                bundle_out: bundle_dir.clone(),
                commit_payloads: vec![temp.path().join("private/commit.json")],
                reveal_payloads: Vec::new(),
                commit_format: "json".to_string(),
                reveal_format: "json".to_string(),
                submit_tally: false,
                iroha_bin: "iroha".to_string(),
                service_name: "org.sora.sorafs.ballots-executor-test".to_string(),
                service_user: "sorafs-exec".to_string(),
                service_group: "sorafs-exec".to_string(),
                interval_secs: 60,
            };
            let mut setup_ctx = TestContext::new();
            bundle_args
                .run_with(&mut setup_ctx)
                .expect("executor bundle should be written");
            let execution_summary = write_json_file(&norito::json!({
                "schema": "sorafs.moderation.ballots.execution.v1",
                "payload_bytes_included": false,
                "private_payloads_included": false,
                "payload_b64": "AAAA",
                "actions": []
            }));
            let args = ModerationBallotsExecutorCanaryArgs {
                bundle: bundle_dir,
                execution_summary: Some(execution_summary.path().to_path_buf()),
                out: None,
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with(&mut ctx)
                .expect_err("payload-bearing execution summary must be rejected");
            assert!(err.to_string().contains("payload bytes"));
            assert!(ctx.printed.is_empty());
        }
        fn moderation_quarantine_notifications_deliver_writes_outbox_and_webhook_summary() {
            let manifest_file = write_json_file(&juror_notifications_manifest_fixture(false));
            let out_dir = TempDir::new().expect("notification outbox");
            let args = ModerationQuarantineNotificationsDeliverArgs {
                manifest: manifest_file.path().to_path_buf(),
                out_dir: Some(out_dir.path().to_path_buf()),
                webhook_url: Some("https://moderation.example.test/webhook".to_string()),
                timeout_secs: 5,
            };
            let mut ctx = TestContext::new();
            let mut posts = Vec::new();
            args.run_with(&mut ctx, |url, body| {
            assert_eq!(url, "https://moderation.example.test/webhook");
            posts.push(body.to_vec());
            Ok(Response::builder()
                .status(StatusCode::ACCEPTED)
                .header("Content-Type", "application/json")
                .body(br#"{"status":"accepted"}"#.to_vec())
                .unwrap())
        })
            .expect("notification delivery should succeed");
            assert_eq!(posts.len(), 1);
            let posted: Value = norito::json::from_slice(&posts[0]).expect("posted notification JSON");
            assert_eq!(posted["delivery_id"].as_str(), Some("notify-1"));
            let outbox_file = out_dir.path().join("notify-1.json");
            assert!(outbox_file.exists(), "outbox file should be written");
            let outbox_body = fs::read_to_string(outbox_file).expect("read outbox file");
            assert!(!outbox_body.contains("payload_b64"));
            assert_eq!(ctx.printed.len(), 1);
            let summary: Value =
                norito::json::from_str(&ctx.printed[0]).expect("delivery summary JSON");
            assert_eq_compact! { summary["schema"].as_str() => Some("sorafs.moderation.juror_notifications.delivery.v1") };
            assert_eq!(summary["delivery_count"].as_u64(), Some(1));
            assert_eq!(summary["payload_bytes_included"].as_bool(), Some(false));
            assert_eq!(summary["private_payloads_included"].as_bool(), Some(false));
            assert_compact! { !ctx.printed[0].contains("Build the private commit payload locally."); "delivery summary must not repeat notification body text" };
        }
        fn moderation_quarantine_notifications_deliver_rejects_private_payload_flags() {
            let manifest_file = write_json_file(&juror_notifications_manifest_fixture(true));
            let out_dir = TempDir::new().expect("notification outbox");
            let args = ModerationQuarantineNotificationsDeliverArgs {
                manifest: manifest_file.path().to_path_buf(),
                out_dir: Some(out_dir.path().to_path_buf()),
                webhook_url: None,
                timeout_secs: 5,
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with(&mut ctx, |_url, _body| {
                    unreachable!("webhook delivery must not run")
                })
                .expect_err("private payload flag must be rejected");
            assert!(err.to_string().contains("private_payload_included"));
            assert!(ctx.printed.is_empty());
            assert_compact! { fs::read_dir(out_dir.path()).expect("read outbox dir").next().is_none(); "outbox must stay empty on validation failure" };
        }
        fn moderation_quarantine_notifications_canary_writes_payload_free_evidence() {
            let manifest_file = write_json_file(&juror_notifications_manifest_fixture(false));
            let out_dir = TempDir::new().expect("canary evidence dir");
            let out = out_dir.path().join("nested/evidence.json");
            let args = ModerationQuarantineNotificationsCanaryArgs {
                manifest: manifest_file.path().to_path_buf(),
                webhook_url: "https://moderation.example.test/webhook".to_string(),
                out: Some(out.clone()),
                timeout_secs: 5,
            };
            let mut ctx = TestContext::new();
            let mut posts = Vec::new();
            args.run_with(&mut ctx, |url, body| {
            assert_eq!(url, "https://moderation.example.test/webhook");
            posts.push(body.to_vec());
            Ok(Response::builder()
                .status(StatusCode::ACCEPTED)
                .header("Content-Type", "application/json")
                .body(br#"{"status":"accepted"}"#.to_vec())
                .unwrap())
        })
            .expect("canary should succeed");
            assert_eq!(posts.len(), 1);
            assert!(out.exists(), "canary evidence file should be written");
            assert_eq!(ctx.printed.len(), 1);
            let evidence: Value =
                norito::json::from_str(&ctx.printed[0]).expect("canary evidence JSON");
            assert_eq_compact! { evidence["schema"].as_str() => Some("sorafs.moderation.juror_notifications.transport_canary.v1") };
            assert_eq!(evidence["status"].as_str(), Some("passed"));
            assert_eq!(evidence["probe_count"].as_u64(), Some(1));
            assert_eq!(evidence["accepted_count"].as_u64(), Some(1));
            assert_compact! { evidence["manifest_body_blake3_hex"].as_str().is_some(); "canary evidence should expose the typed manifest digest key" };
            assert_compact! { evidence.get("manifest_body_blake3").is_none(); "canary evidence must not emit the ambiguous manifest digest key" };
            assert_eq!(evidence["payload_bytes_included"].as_bool(), Some(false));
            assert_eq!(evidence["private_payloads_included"].as_bool(), Some(false));
            assert_compact! { !ctx.printed[0].contains("Build the private commit payload locally."); "canary evidence must not repeat notification body text" };
        }
        fn moderation_quarantine_notifications_canary_records_failed_probe_without_body() {
            let manifest_file = write_json_file(&juror_notifications_manifest_fixture(false));
            let args = ModerationQuarantineNotificationsCanaryArgs {
                manifest: manifest_file.path().to_path_buf(),
                webhook_url: "https://moderation.example.test/webhook".to_string(),
                out: None,
                timeout_secs: 5,
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_url, _body| {
            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header("Content-Type", "application/json")
                .body(br#"{"error":"transport unavailable"}"#.to_vec())
                .unwrap())
        })
            .expect("canary should emit failed evidence instead of hiding probe failure");
            let evidence: Value =
                norito::json::from_str(&ctx.printed[0]).expect("canary evidence JSON");
            assert_eq!(evidence["status"].as_str(), Some("failed"));
            assert_eq!(evidence["accepted_count"].as_u64(), Some(0));
            assert_compact! { !ctx.printed[0].contains("transport unavailable"); "canary evidence must hash response bodies instead of archiving them" };
        }
        fn moderation_quarantine_notifications_run_does_not_follow_cross_origin_redirects() {
            for canary in [false, true] {
                let manifest = write_json_file(&juror_notifications_manifest_fixture(false));
                let origin = TcpListener::bind("127.0.0.1:0").expect("bind redirect origin");
                let origin_addr = origin.local_addr().expect("redirect origin address");
                let target = TcpListener::bind("127.0.0.1:0").expect("bind redirect target");
                let target_addr = target.local_addr().expect("redirect target address");
                let server = thread::spawn(move || {
                    let (mut stream, _) = origin.accept().expect("accept webhook request");
                    let mut request = [0_u8; 8192];
                    let len = stream.read(&mut request).expect("read webhook request");
                    assert!(request[..len].starts_with(b"POST "));
                    write!(
                        stream,
                        "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{target_addr}/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .expect("write redirect response");
                });
                let webhook_url = format!("http://{origin_addr}/webhook");
                let mut ctx = TestContext::new();
                if canary {
                    ModerationQuarantineNotificationsCanaryArgs {
                        manifest: manifest.path().to_path_buf(),
                        webhook_url,
                        out: None,
                        timeout_secs: 1,
                    }
                    .run(&mut ctx)
                    .expect("redirect must produce failed canary evidence");
                    let evidence: Value =
                        norito::json::from_str(&ctx.printed[0]).expect("redirect canary evidence JSON");
                    assert_eq!(evidence["status"].as_str(), Some("failed"));
                    assert_eq!(evidence["accepted_count"].as_u64(), Some(0));
                    assert_eq!(evidence["probes"][0]["response_status"].as_u64(), Some(307));
                } else {
                    let err = ModerationQuarantineNotificationsDeliverArgs {
                        manifest: manifest.path().to_path_buf(),
                        out_dir: None,
                        webhook_url: Some(webhook_url),
                        timeout_secs: 1,
                    }
                    .run(&mut ctx)
                    .expect_err("redirected delivery must fail");
                    assert!(err.to_string().contains("status 307"));
                    assert!(ctx.printed.is_empty());
                }
                server.join().expect("redirect server finished");
                target
                    .set_nonblocking(true)
                    .expect("set target nonblocking");
                assert_compact! { matches!(target.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock); "cross-origin redirect target must receive no connection" };
            }
        }
        fn sorafs_get_canary_runs_do_not_follow_cross_origin_redirects() {
            for command in 0_u8..3 {
                let origin = TcpListener::bind("127.0.0.1:0").expect("bind redirect origin");
                let origin_addr = origin.local_addr().expect("redirect origin address");
                let target = TcpListener::bind("127.0.0.1:0").expect("bind redirect target");
                let target_addr = target.local_addr().expect("redirect target address");
                let server = thread::spawn(move || {
                    let (mut stream, _) = origin.accept().expect("accept GET canary request");
                    let mut request = [0_u8; 8192];
                    let len = stream.read(&mut request).expect("read GET canary request");
                    assert!(request[..len].starts_with(b"GET "));
                    write!(
                        stream,
                        "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{target_addr}/substitute\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .expect("write redirect response");
                });
                let base_url = format!("http://{origin_addr}/root");
                let mut ctx = TestContext::new();
                match command {
                    0 => {
                        let error = TransparencyExplorerCanaryArgs {
                            torii_url: Some(base_url),
                            limit: None,
                            timeout_secs: 1,
                            out: None,
                        }
                        .run(&mut ctx)
                        .expect_err("explorer redirect must fail");
                        assert!(error.to_string().contains("307"));
                        assert!(ctx.printed.is_empty());
                    }
                    1 => {
                        TransparencyPublicationCanaryArgs {
                            torii_url: Some(base_url),
                            cycle_ids: Vec::new(),
                            limit: None,
                            timeout_secs: 1,
                            out: None,
                        }
                        .run(&mut ctx)
                        .expect("publication redirect must emit failed evidence");
                        let evidence: Value = norito::json::from_str(&ctx.printed[0])
                            .expect("publication redirect evidence JSON");
                        assert_eq!(evidence["status"].as_str(), Some("failed"));
                        assert_eq!(evidence["routes"][0]["status_code"].as_u64(), Some(307));
                        assert_eq!(evidence["routes"][0]["passed"].as_bool(), Some(false));
                    }
                    2 => {
                        let error = ModerationQuarantineOperatorCanaryArgs {
                            operator_url: base_url,
                            quarantine_id: "ba".repeat(16),
                            limit: None,
                            timeout_secs: 1,
                            out: None,
                        }
                        .run(&mut ctx)
                        .expect_err("operator redirect must fail");
                        assert!(error.to_string().contains("307"));
                        assert!(ctx.printed.is_empty());
                    }
                    _ => unreachable!("bounded GET canary selector"),
                }
                server.join().expect("redirect server finished");
                target.set_nonblocking(true).expect("nonblocking target");
                assert_compact! { matches!(target.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock); "cross-origin redirect target must receive no GET connection" };
            }
        }
        fn moderation_registry_list_with_prints_payload() {
            let args = ModerationRegistryListArgs { limit: Some(5) };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, filter| {
                assert_eq!(filter.limit, Some(5));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "repro_manifests": [
                            { "manifest_id_hex": "aa", "model_count": 1 }
                        ],
                        "adversarial_corpora": []
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"repro_manifests\""));
        }
        fn moderation_registry_submit_repro_reads_json_manifest() {
            let manifest = signed_moderation_repro_manifest_fixture();
            let expected_bytes = to_bytes(&manifest).expect("encode canonical repro manifest");
            let mut file = NamedTempFile::new().expect("repro manifest file");
            file.write_all(
                norito::json::to_json_pretty(&manifest)
                    .expect("render repro json")
                    .as_bytes(),
            )
            .expect("write repro json");
            let args = ModerationRegistrySubmitReproArgs {
                manifest: file.path().to_path_buf(),
                format: "json".to_string(),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, manifest_bytes| {
                assert_eq!(manifest_bytes, expected_bytes.as_slice());
    json_response_fixture!(StatusCode::ACCEPTED,
                        &norito::json!({ "status": "admitted" }),
                    )
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"admitted\""));
        }
        fn moderation_registry_submit_corpus_reads_norito_manifest() {
            let manifest = adversarial_corpus_manifest_fixture();
            let expected_bytes = to_bytes(&manifest).expect("encode canonical corpus manifest");
            let mut file = NamedTempFile::new().expect("corpus manifest file");
            file.write_all(&expected_bytes)
                .expect("write corpus norito");
            let args = ModerationRegistrySubmitCorpusArgs {
                manifest: file.path().to_path_buf(),
                format: "norito".to_string(),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, manifest_bytes| {
                assert_eq!(manifest_bytes, expected_bytes.as_slice());
    json_response_fixture!(StatusCode::ACCEPTED,
                        &norito::json!({ "status": "admitted" }),
                    )
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"admitted\""));
        }
        fn moderation_registry_submit_repro_rejects_invalid_format() {
            let file = NamedTempFile::new().expect("manifest file");
            let args = ModerationRegistrySubmitReproArgs {
                manifest: file.path().to_path_buf(),
                format: "yaml".to_string(),
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with(&mut ctx, |_client, _| unreachable!("submit must not run"))
                .expect_err("invalid format must be rejected");
            assert!(err.to_string().contains("--format"));
            assert!(ctx.printed.is_empty());
        }
        fn moderation_screening_list_with_prints_payload() {
            let args = ModerationScreeningListArgs { limit: Some(6) };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, filter| {
                assert_eq!(filter.limit, Some(6));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "screening_records": [
                            { "record_id_hex": "aa", "verdict": "quarantine" }
                        ]
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"screening_records\""));
        }
        fn moderation_screening_submit_reads_authenticated_authority_json() {
            let idempotency_key = [0xA1_u8; 32];
            let expected_idempotency_key = encode(idempotency_key);
            let uppercase_idempotency_key =
                format!("0x{}", encode(idempotency_key).to_ascii_uppercase());
            let authority_b64 = STANDARD.encode(b"canonical committee aggregate");
            let member_one_b64 = STANDARD.encode(b"canonical signed member one");
            let member_two_b64 = STANDARD.encode(b"canonical signed member two");
            let mut file = NamedTempFile::new().expect("screening result file");
            let mut screening_result = Map::new();
            screening_result.insert(
                "idempotency_key_hex".to_owned(),
                Value::String(uppercase_idempotency_key),
            );
            screening_result.insert(
                "evidence_kind".to_owned(),
                Value::String("committee_aggregate".to_owned()),
            );
            screening_result.insert(
                "authority_b64".to_owned(),
                Value::String(authority_b64.clone()),
            );
            screening_result.insert(
                "committee_member_results_b64".to_owned(),
                Value::Array(vec![
                    Value::String(member_one_b64.clone()),
                    Value::String(member_two_b64.clone()),
                ]),
            );
            file.write_all(
                &norito::json::to_vec(&Value::Object(screening_result))
                    .expect("serialize screening JSON"),
            )
            .expect("write screening JSON");
            let args = ModerationScreeningSubmitArgs {
                input: file.path().to_path_buf(),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, request| {
                assert_eq!(request.idempotency_key_hex, expected_idempotency_key);
                assert_eq!(request.evidence_kind, "committee_aggregate");
                assert_eq!(request.authority_b64, authority_b64);
                assert_eq_compact! { request.committee_member_results_b64 =>[member_one_b64.clone(), member_two_b64.clone()] };
    json_response_fixture!(StatusCode::ACCEPTED,
                        &norito::json!({ "status": "accepted" }),
                    )
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"accepted\""));
        }
        fn moderation_screening_submit_rejects_missing_field() {
            let mut file = NamedTempFile::new().expect("screening result file");
            file.write_all(
                &norito::json::to_vec(&norito::json!({
                    "idempotency_key_hex": (encode([0x11_u8; 32])),
                    "evidence_kind": "signed_result",
                    "committee_member_results_b64": [],
                }))
                .expect("serialize screening JSON"),
            )
            .expect("write screening JSON");
            let args = ModerationScreeningSubmitArgs {
                input: file.path().to_path_buf(),
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with(&mut ctx, |_client, _| unreachable!("submit must not run"))
                .expect_err("missing authority must be rejected");
            assert!(err.to_string().contains("authority_b64"));
            assert!(ctx.printed.is_empty());
        }
        fn moderation_quarantine_list_with_prints_payload() {
            let args = ModerationQuarantineListArgs { limit: Some(4) };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, filter| {
                assert_eq!(filter.limit, Some(4));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "quarantine_records": [
                            { "quarantine_id_hex": "aa", "state": "pending_review" }
                        ]
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"quarantine_records\""));
        }
        fn moderation_quarantine_object_store_reads_payload_file() {
            let quarantine_id = [0xA7_u8; 16];
            let mut file = NamedTempFile::new().expect("temp payload");
            file.write_all(b"quarantine payload bytes")
                .expect("write payload");
            let args = ModerationQuarantineObjectStoreArgs {
                quarantine_id: format!("0x{}", encode(quarantine_id).to_ascii_uppercase()),
                payload_file: file.path().to_path_buf(),
                captured_at: Some("@1800000310".to_string()),
                content_type: Some(" application/octet-stream ".to_string()),
                notes: Some(" sealed via cli ".to_string()),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, id, request| {
                assert_eq!(id, encode(quarantine_id));
                assert_eq!(request.payload, b"quarantine payload bytes");
                assert_eq!(request.captured_at_unix, Some(1_800_000_310));
                assert_eq!(request.content_type, Some("application/octet-stream"));
                assert_eq!(request.notes, Some("sealed via cli"));
    json_response_fixture!(StatusCode::ACCEPTED,
                        &norito::json!({ "status": "stored" }),
                    )
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"stored\""));
        }
        fn moderation_quarantine_object_read_prints_payload_json() {
            let quarantine_id = [0xB8_u8; 16];
            let args = ModerationQuarantineObjectReadArgs {
                quarantine_id: encode(quarantine_id),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, id| {
                assert_eq!(id, encode(quarantine_id));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "status": "read",
                        "payload_b64": "cGF5bG9hZA=="
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"payload_b64\""));
        }
        fn moderation_quarantine_object_store_rejects_empty_payload_file() {
            let file = NamedTempFile::new().expect("empty payload");
            let args = ModerationQuarantineObjectStoreArgs {
                quarantine_id: encode([0xC9_u8; 16]),
                payload_file: file.path().to_path_buf(),
                captured_at: Some("@1800000310".to_string()),
                content_type: None,
                notes: None,
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with(&mut ctx, |_client, _, _| {
                    unreachable!("submit must not run")
                })
                .expect_err("empty payload must be rejected");
            assert!(err.to_string().contains("--payload-file"));
            assert!(ctx.printed.is_empty());
        }
        fn moderation_quarantine_review_builds_request() {
            let quarantine_id = [0xAB_u8; 16];
            let args = ModerationQuarantineReviewArgs {
                quarantine_id: format!("0x{}", encode(quarantine_id).to_ascii_uppercase()),
                reviewed_by: Some(" operator@moderation ".to_string()),
                reviewed_at: Some("@1800000210".to_string()),
                notes: Some(" reviewed locally ".to_string()),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, id, request| {
                assert_eq!(id, encode(quarantine_id));
                assert_eq!(request.reviewed_by, "operator@moderation");
                assert_eq!(request.reviewed_at_unix, Some(1_800_000_210));
                assert_eq!(request.notes, Some("reviewed locally"));
    json_response_fixture!(StatusCode::ACCEPTED,
                        &norito::json!({ "state": "reviewed" }),
                    )
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"reviewed\""));
        }
        fn moderation_quarantine_release_defaults_authority_to_cli_account() {
            let quarantine_id = [0xCD_u8; 16];
            let args = ModerationQuarantineReleaseArgs {
                quarantine_id: encode(quarantine_id),
                release_authority: None,
                released_at: Some("@1800000220".to_string()),
                notes: None,
            };
            let mut ctx = TestContext::new();
            let expected_authority = ctx.config().account.to_string();
            args.run_with(&mut ctx, |_client, id, request| {
                assert_eq!(id, encode(quarantine_id));
                assert_eq!(request.release_authority, expected_authority);
                assert_eq!(request.released_at_unix, Some(1_800_000_220));
                assert_eq!(request.notes, None);
    json_response_fixture!(StatusCode::ACCEPTED,
                        &norito::json!({ "state": "released" }),
                    )
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"released\""));
        }
        fn moderation_quarantine_appeal_handoff_reads_json_payload() {
            let quarantine_id = [0xA4_u8; 16];
            let input = write_json_file(&norito::json!({
                "class": "content",
                "backlog": 2_u64,
                "evidence_size_mb": 8_u64,
                "payer_account": "payer",
                "destination_account": "treasury",
                "asset_definition_id": "xor#wonderland"
            }));
            let args = ModerationQuarantineAppealHandoffArgs {
                quarantine_id: format!("0x{}", encode(quarantine_id).to_ascii_uppercase()),
                input: input.path().to_path_buf(),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, id, payload| {
                assert_eq!(id, encode(quarantine_id));
                let value: Value = norito::json::from_slice(payload)?;
                assert_eq!(value.get("class").and_then(Value::as_str), Some("content"));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "schema": "sorafs.moderation.quarantine.appeal_handoff.v1",
                        "status": "ready_for_deposit"
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("ready_for_deposit"));
        }
        fn moderation_quarantine_appeal_handoff_rejects_empty_payload() {
            let input = NamedTempFile::new().expect("empty appeal handoff payload");
            let args = ModerationQuarantineAppealHandoffArgs {
                quarantine_id: encode([0xA5_u8; 16]),
                input: input.path().to_path_buf(),
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with(&mut ctx, |_client, _, _| {
                    unreachable!("submit must not run")
                })
                .expect_err("empty appeal handoff payload must be rejected");
            assert_compact! { err.to_string().contains("moderation quarantine appeal handoff payload") };
            assert!(ctx.printed.is_empty());
        }
        fn moderation_quarantine_operator_panel_reads_workflow_view() {
            let quarantine_id = [0xA8_u8; 16];
            let args = ModerationQuarantineOperatorPanelArgs {
                quarantine_id: format!("0x{}", encode(quarantine_id).to_ascii_uppercase()),
                limit: Some(4),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, id, filter| {
                assert_eq!(id, encode(quarantine_id));
                assert_eq!(filter.limit, Some(4));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "schema": "sorafs.moderation.quarantine.operator_panel.v1",
                        "status": "ready"
                    }))
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("operator_panel"));
        }
        fn moderation_quarantine_operator_panel_rejects_bad_quarantine_id() {
            let args = ModerationQuarantineOperatorPanelArgs {
                quarantine_id: "abcd".to_owned(),
                limit: None,
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with(&mut ctx, |_client, _, _| unreachable!("get must not run"))
                .expect_err("bad quarantine id must be rejected");
            assert!(err.to_string().contains("--quarantine-id"));
            assert!(ctx.printed.is_empty());
        }
        fn moderation_quarantine_bridge_plan_derives_workflow_actions() {
            let quarantine_id = [0xA9_u8; 16];
            let args = ModerationQuarantineBridgePlanArgs {
                quarantine_id: format!("0x{}", encode(quarantine_id).to_ascii_uppercase()),
                limit: Some(5),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, id, filter| {
                assert_eq!(id, encode(quarantine_id));
                assert_eq!(filter.limit, Some(5));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                        "schema": "sorafs.moderation.quarantine.operator_panel.v1",
                        "status": "ready",
                        "record": {
                            "quarantine_id_hex": (encode(quarantine_id)),
                            "state": "reviewed"
                        },
                        "object_status": "stored",
                        "case_count": 1_u64,
                        "returned_case_count": 1_u64,
                        "cases": [(fixture_finalized_moderation_case(&[], &[], &[]))],
                        "operator_routes": {
                            "object": "/v1/sorafs/moderation/quarantine/object"
                        },
                        "next_actions": [
                            {
                                "action": "read_object",
                                "route": "/v1/sorafs/moderation/quarantine/object",
                                "required": false
                            },
                            {
                                "action": "submit_native_case_actions",
                                "route": "/v1/sorafs/moderation/ballots",
                                "required": true
                            }
                        ]
                    }))
            })
            .expect("bridge plan should render");
            assert_eq!(ctx.printed.len(), 1);
            let value: Value = norito::json::from_str(&ctx.printed[0]).expect("bridge plan json");
            assert_eq_compact! { value.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.quarantine.bridge_plan.v1") };
            assert_eq_compact! { value.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
            let actions = value
                .get("actions")
                .and_then(Value::as_array)
                .expect("actions");
            assert_eq!(actions.len(), 2);
            assert_eq_compact! { actions[1].get("automation_status").and_then(Value::as_str) => Some("waiting_for_native_commit_reveal_finalization") };
            let cli = actions[1]
                .get("cli")
                .and_then(Value::as_array)
                .expect("cli");
            assert_compact! { cli.iter().any(|part| part.as_str() == Some("quarantine-case")) };
            assert!(!ctx.printed[0].contains("payload_b64"));
        }
        fn moderation_quarantine_bridge_plan_rejects_payload_bytes() {
            let quarantine_id = [0xAA_u8; 16];
            let args = ModerationQuarantineBridgePlanArgs {
                quarantine_id: encode(quarantine_id),
                limit: None,
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with(&mut ctx, |_client, _id, _filter| {
    json_response_fixture!(StatusCode::OK, &norito::json!({
                            "schema": "sorafs.moderation.quarantine.operator_panel.v1",
                            "record": {
                                "state": "reviewed"
                            },
                            "object_status": "stored",
                            "payload_b64": "c2hvdWxkLW5vdC1iZS1oZXJl",
                            "next_actions": []
                        }))
                })
                .expect_err("payload bytes must be rejected");
            assert!(err.to_string().contains("payload bytes"));
            assert!(ctx.printed.is_empty());
        }
        fn moderation_quarantine_bridge_plan_rejects_bad_quarantine_id() {
            let args = ModerationQuarantineBridgePlanArgs {
                quarantine_id: "abcd".to_owned(),
                limit: None,
            };
            let mut ctx = TestContext::new();
            let err = args
                .run_with(&mut ctx, |_client, _, _| unreachable!("get must not run"))
                .expect_err("bad quarantine id must be rejected");
            assert!(err.to_string().contains("--quarantine-id"));
            assert!(ctx.printed.is_empty());
        }
        }
    fn moderation_operator_canary_fixture_json(
        value: Value,
    ) -> Result<ModerationOperatorCanaryHttpResponse> {
        Ok(ModerationOperatorCanaryHttpResponse {
            status: StatusCode::OK,
            content_type: Some("application/json".to_string()),
            body: norito::json::to_vec(&value)?,
        })
    }
    fn moderation_operator_canary_fixture_response(
        url: &str,
        quarantine_id_hex: &str,
        include_payload_b64: bool,
        bridge_schema: &str,
    ) -> Result<ModerationOperatorCanaryHttpResponse> {
        let parsed = Url::parse(url).expect("canary URL should parse");
        let path = parsed.path();
        if path.contains("/quarantine/") {
            assert_compact! { path.contains(&format!("/quarantine/{quarantine_id_hex}/")); "unexpected canary quarantine route: {path}" };
        }
        if path.ends_with("/healthz")
            || path.ends_with("/v1/sorafs/moderation/operator-panel/status")
        {
            return moderation_operator_canary_fixture_json(norito::json!({
                "schema": "sorafs.moderation.quarantine.operator_service.status.v1",
                "status": "ready"
            }));
        }
        if path.ends_with("/v1/sorafs/moderation/operator-panel/ui") {
            return Ok(ModerationOperatorCanaryHttpResponse {
                status: StatusCode::OK,
                content_type: Some("text/html; charset=utf-8".to_string()),
                body: b"<main><h1>SoraFS Moderation Operator</h1></main>".to_vec(),
            });
        }
        if path.ends_with("/operator-panel") {
            let value = if include_payload_b64 {
                norito::json!({
                    "schema": "sorafs.moderation.quarantine.operator_panel.v1",
                    "status": "ready",
                    "payload_b64": "c2hvdWxkLW5vdC1iZS1oZXJl",
                    "payload_bytes_included": false,
                    "record": {
                        "quarantine_id_hex": (quarantine_id_hex),
                        "state": "reviewed"
                    },
                    "object_status": "stored",
                    "next_actions": []
                })
            } else {
                norito::json!({
                    "schema": "sorafs.moderation.quarantine.operator_panel.v1",
                    "status": "ready",
                    "payload_bytes_included": false,
                    "record": {
                        "quarantine_id_hex": (quarantine_id_hex),
                        "state": "reviewed"
                    },
                    "object_status": "stored",
                    "next_actions": []
                })
            };
            return moderation_operator_canary_fixture_json(value);
        }
        if path.ends_with("/bridge-plan") {
            return moderation_operator_canary_fixture_json(norito::json!({
                "schema": (bridge_schema),
                "payload_bytes_included": false,
                "private_payloads_included": false,
                "actions": []
            }));
        }
        if path.ends_with("/juror-plan") {
            return moderation_operator_canary_fixture_json(norito::json!({
                "schema": "sorafs.moderation.quarantine.juror_plan.v1",
                "payload_bytes_included": false,
                "private_payloads_included": false,
                "ballots": []
            }));
        }
        if path.ends_with("/juror-notifications") {
            return moderation_operator_canary_fixture_json(norito::json!({
                "schema": "sorafs.moderation.quarantine.juror_notifications.v1",
                "payload_bytes_included": false,
                "private_payloads_included": false,
                "notifications": []
            }));
        }
        if path.ends_with("/commit-reveal-status") {
            return moderation_operator_canary_fixture_json(norito::json!({
                "schema": "sorafs.moderation.quarantine.commit_reveal_status.v1",
                "payload_bytes_included": false,
                "private_payloads_included": false,
                "ballots": []
            }));
        }
        panic!("unexpected canary route: {url}");
    }
    test_items! {
    fn moderation_quarantine_operator_canary_builds_payload_free_evidence() {
        let quarantine_id = [0xBA_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let out_dir = TempDir::new().expect("canary evidence dir");
        let out = out_dir.path().join("nested/evidence.json");
        let args = ModerationQuarantineOperatorCanaryArgs {
            operator_url: " https://operator.test/root ".to_string(),
            quarantine_id: format!("0x{}", quarantine_id_hex.to_ascii_uppercase()),
            limit: Some(4),
            timeout_secs: 1,
            out: Some(out.clone()),
        };
        let mut ctx = TestContext::new();
        let mut requested = Vec::new();
        args.run_with_fetch(&mut ctx, |url| {
            requested.push(url.to_string());
            moderation_operator_canary_fixture_response(
                url,
                &quarantine_id_hex,
                false,
                "sorafs.moderation.quarantine.bridge_plan.v1",
            )
        })
        .expect("operator canary should render evidence");
        assert_eq!(requested.len(), 8);
        assert_eq!(ctx.printed.len(), 1);
        assert!(!ctx.printed[0].contains("payload_b64"));
        let value: Value = norito::json::from_str(&ctx.printed[0]).expect("canary evidence JSON");
        let schema = value["schema"].as_str();
        assert_eq_compact! { schema => Some("sorafs.moderation.quarantine.operator_canary.v1") };
        assert_eq!(value["status"].as_str(), Some("passed"));
        assert_eq!(value["limit"].as_u64(), Some(4));
        assert_eq!(value["route_count"].as_u64(), Some(8));
        assert_eq!(value["payload_bytes_included"].as_bool(), Some(false));
        let routes = value["routes"].as_array().expect("canary routes");
        assert_eq!(routes.len(), 8);
        let has_commit_reveal = routes
            .iter()
            .any(|route| route["name"].as_str() == Some("commit_reveal_status"));
        assert!(has_commit_reveal);
        let operator_panel = routes
            .iter()
            .find(|route| route["name"].as_str() == Some("operator_panel"))
            .expect("operator-panel route evidence");
        let operator_panel_url = operator_panel["url"].as_str().expect("operator-panel URL");
        assert!(operator_panel_url.contains("/root/v1/sorafs/moderation/quarantine/"));
        assert!(operator_panel_url.contains("limit=4"));
        assert_compact! { routes.iter().all(|route| { route.get("payload_bytes_included").and_then(Value::as_bool) == Some(false) }) };
        let bytes = fs::read(out).expect("written canary evidence");
        let written: Value = norito::json::from_slice(&bytes).expect("written evidence JSON");
        assert_eq!(written["schema"], value["schema"]);
    }
    fn moderation_quarantine_operator_canary_rejects_payload_bytes() {
        let quarantine_id = [0xBB_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let args = ModerationQuarantineOperatorCanaryArgs {
            operator_url: "https://operator.test/root".to_string(),
            quarantine_id: quarantine_id_hex.clone(),
            limit: Some(4),
            timeout_secs: 1,
            out: None,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with_fetch(&mut ctx, |url| {
                moderation_operator_canary_fixture_response(
                    url,
                    &quarantine_id_hex,
                    true,
                    "sorafs.moderation.quarantine.bridge_plan.v1",
                )
            })
            .expect_err("operator canary must reject payload bytes");
        assert!(err.to_string().contains("payload bytes"));
        assert!(ctx.printed.is_empty());
    }
    fn moderation_quarantine_operator_canary_rejects_schema_drift() {
        let quarantine_id = [0xBC_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let args = ModerationQuarantineOperatorCanaryArgs {
            operator_url: "https://operator.test/root".to_string(),
            quarantine_id: quarantine_id_hex.clone(),
            limit: None,
            timeout_secs: 1,
            out: None,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with_fetch(&mut ctx, |url| {
                moderation_operator_canary_fixture_response(
                    url,
                    &quarantine_id_hex,
                    false,
                    "sorafs.moderation.quarantine.unexpected_bridge_plan.v1",
                )
            })
            .expect_err("operator canary must reject schema drift");
        assert!(err.to_string().contains("schema"));
        assert!(ctx.printed.is_empty());
    }
    }
    struct FixtureModerationOperatorPanelSource {
        expected_quarantine_id_hex: String,
        expected_limit: Option<u32>,
        status: StatusCode,
        body: Vec<u8>,
    }
    impl ModerationOperatorWorkflowSource for FixtureModerationOperatorPanelSource {
        fn get_operator_panel(
            &self,
            quarantine_id_hex: &str,
            filter: SorafsModerationQuarantineFilter,
        ) -> Result<Response<Vec<u8>>> {
            assert_eq!(quarantine_id_hex, self.expected_quarantine_id_hex);
            assert_eq!(filter.limit, self.expected_limit);
            Ok(Response::builder()
                .status(self.status)
                .header("Content-Type", "application/json")
                .body(self.body.clone())
                .unwrap())
        }
    }
    macro_rules! moderation_operator_panel_fixture {
        ($quarantine_id_hex:ident; $($extra:tt)*) => {
            norito::json!({
                "schema": "sorafs.moderation.quarantine.operator_panel.v1",
                "status": "ready",
                "record": {
                    "quarantine_id_hex": ($quarantine_id_hex.clone()),
                    "state": "reviewed"
                },
                "object_status": "stored",
                $($extra)*
            })
        };
    }
    fn fixture_moderation_operator_service(
        quarantine_id: [u8; 16],
        expected_limit: Option<u32>,
        body: Value,
    ) -> ModerationOperatorService {
        let args = ModerationQuarantineOperatorServeArgs {
            listen: "127.0.0.1:0".to_string(),
            limit: expected_limit,
            max_body_bytes: 1024,
        };
        args.service(
            Arc::new(FixtureModerationOperatorPanelSource {
                expected_quarantine_id_hex: encode(quarantine_id),
                expected_limit,
                status: StatusCode::OK,
                body: norito::json::to_vec(&body).expect("fixture operator-panel JSON"),
            }),
            "http://torii.test/".to_string(),
            "operator@moderation".to_string(),
        )
        .expect("operator service")
    }
    fn fixture_finalized_moderation_case(
        jurors: &[&str],
        committed_jurors: &[&str],
        revealed_jurors: &[&str],
    ) -> Value {
        let jurors = jurors.iter().copied().map(Value::from).collect::<Vec<_>>();
        let commits = committed_jurors
            .iter()
            .map(|juror| norito::json!({ "juror": (*juror) }))
            .collect::<Vec<_>>();
        let reveals = revealed_jurors
            .iter()
            .map(|juror| norito::json!({ "juror": (*juror) }))
            .collect::<Vec<_>>();
        norito::json!({
            "case": {
                "spec": {
                    "context": {
                        "case_id": "quarantine-case",
                        "evidence_uri": "sorafs://moderation/quarantine"
                    },
                    "round_id": "round-7",
                    "jurors": (jurors),
                    "quorum": 2_u64,
                    "commit_deadline_unix_ms": 1_800_000_200_000_u64,
                    "challenge_deadline_unix_ms": 1_800_000_300_000_u64,
                    "reveal_deadline_unix_ms": 1_800_000_400_000_u64
                },
                "opened_at_unix_ms": 1_800_000_100_000_u64
            },
            "commits": (commits),
            "reveals": (reveals),
            "challenges": [],
            "outcome": null,
            "no_shows": []
        })
    }
    fn fixture_payload_bearing_finalized_moderation_case() -> Value {
        let mut case = fixture_finalized_moderation_case(&["juror-a@moderation"], &[], &[]);
        case.as_object_mut()
            .expect("finalized case fixture object")
            .insert(
                "payload_b64".to_owned(),
                Value::from("c2hvdWxkLW5vdC1sZWFr"),
            );
        case
    }
    struct FixtureModerationOperatorMutationSource {
        expected_quarantine_id_hex: String,
        expected_kind: &'static str,
        status: StatusCode,
        body: Vec<u8>,
    }
    impl FixtureModerationOperatorMutationSource {
        fn response(&self) -> Result<Response<Vec<u8>>> {
            Ok(Response::builder()
                .status(self.status)
                .header("Content-Type", "application/json")
                .body(self.body.clone())
                .unwrap())
        }
    }
    impl ModerationOperatorWorkflowSource for FixtureModerationOperatorMutationSource {
        fn get_operator_panel(
            &self,
            _quarantine_id_hex: &str,
            _filter: SorafsModerationQuarantineFilter,
        ) -> Result<Response<Vec<u8>>> {
            unreachable!("mutation fixture does not serve operator-panel reads")
        }
        fn post_review(
            &self,
            quarantine_id_hex: &str,
            request: &SorafsModerationQuarantineReviewRequest<'_>,
        ) -> Result<Response<Vec<u8>>> {
            assert_eq!(self.expected_kind, "review");
            assert_eq!(quarantine_id_hex, self.expected_quarantine_id_hex);
            assert_eq!(request.reviewed_by, "operator@moderation");
            assert_eq!(request.reviewed_at_unix, Some(1_800_000_310));
            assert_eq!(request.notes, Some("reviewed through service"));
            self.response()
        }
        fn post_release(
            &self,
            quarantine_id_hex: &str,
            request: &SorafsModerationQuarantineReleaseRequest<'_>,
        ) -> Result<Response<Vec<u8>>> {
            assert_eq!(self.expected_kind, "release");
            assert_eq!(quarantine_id_hex, self.expected_quarantine_id_hex);
            assert_eq!(request.release_authority, "release@moderation");
            assert_eq!(request.released_at_unix, Some(1_800_000_320));
            assert_eq!(request.notes, Some("released through service"));
            self.response()
        }
        fn post_appeal_handoff(
            &self,
            quarantine_id_hex: &str,
            payload: &[u8],
        ) -> Result<Response<Vec<u8>>> {
            assert_eq!(self.expected_kind, "appeal-handoff");
            assert_eq!(quarantine_id_hex, self.expected_quarantine_id_hex);
            let value: Value = norito::json::from_slice(payload).expect("handoff payload JSON");
            assert_eq!(value.get("class").and_then(Value::as_str), Some("content"));
            assert!(!String::from_utf8_lossy(payload).contains("payload_b64"));
            self.response()
        }
    }
    fn fixture_moderation_operator_mutation_service(
        quarantine_id: [u8; 16],
        expected_kind: &'static str,
        status: StatusCode,
        body: Value,
    ) -> ModerationOperatorService {
        let args = ModerationQuarantineOperatorServeArgs {
            listen: "127.0.0.1:0".to_string(),
            limit: None,
            max_body_bytes: 2048,
        };
        args.service(
            Arc::new(FixtureModerationOperatorMutationSource {
                expected_quarantine_id_hex: encode(quarantine_id),
                expected_kind,
                status,
                body: norito::json::to_vec(&body).expect("fixture mutation response JSON"),
            }),
            "http://torii.test/".to_string(),
            "operator@moderation".to_string(),
        )
        .expect("operator mutation service")
    }
    fn handle_moderation_operator_raw_request(
        service: &ModerationOperatorService,
        raw: String,
    ) -> ModerationOperatorHttpResponse {
        handle_moderation_operator_raw_request_with_csrf(service, raw, true)
    }
    fn handle_moderation_operator_raw_request_without_csrf(
        service: &ModerationOperatorService,
        raw: String,
    ) -> ModerationOperatorHttpResponse {
        handle_moderation_operator_raw_request_with_csrf(service, raw, false)
    }
    fn handle_moderation_operator_raw_request_with_csrf(
        service: &ModerationOperatorService,
        mut raw: String,
        include_csrf: bool,
    ) -> ModerationOperatorHttpResponse {
        if include_csrf && raw.starts_with("POST ") {
            raw = raw.replacen(
                "\r\n\r\n",
                &format!(
                    "\r\n{MODERATION_OPERATOR_CSRF_HEADER}: {}\r\n\r\n",
                    service.csrf_token
                ),
                1,
            );
        }
        let raw = raw.into_bytes();
        let request = moderation_operator_parse_http_request(&raw, 1024).expect("HTTP request");
        service.handle_request(&request)
    }
    fn assert_moderation_operator_payload_rejected(
        quarantine_id: [u8; 16],
        request: String,
        quarantine_id_hex: String,
    ) {
        let service = fixture_moderation_operator_service(
            quarantine_id,
            None,
            moderation_operator_panel_fixture! {
                quarantine_id_hex;
                "case_count": 1_u64,
                "returned_case_count": 1_u64,
                "cases": [(fixture_payload_bearing_finalized_moderation_case())],
                "next_actions": []
            },
        );
        let response = handle_moderation_operator_raw_request(&service, request);
        assert_eq!(response.status, StatusCode::BAD_GATEWAY);
        let body = String::from_utf8(response.body).expect("error JSON is UTF-8");
        assert!(body.contains("payload bytes"));
    }
    test_items! {
    fn moderation_operator_service_routes_operator_panel_with_query_limit() {
        let quarantine_id = [0xAB_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let service = fixture_moderation_operator_service(
            quarantine_id,
            Some(7),
            moderation_operator_panel_fixture! {
                quarantine_id_hex;
                "next_actions": []
            },
        );
        let response = handle_moderation_operator_raw_request(
            &service,
            format!(
                "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/operator-panel?limit=7 HTTP/1.1\r\nHost: local\r\n\r\n"
            ),
        );
        assert_eq!(response.status, StatusCode::OK);
        let value: Value = norito::json::from_slice(&response.body).expect("operator panel JSON");
        assert_eq_compact! { value.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.quarantine.operator_panel.v1") };
        assert!(!String::from_utf8_lossy(&response.body).contains("payload_b64"));
    }
    fn moderation_operator_service_builds_bridge_plan_without_payload() {
        let quarantine_id = [0xAC_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let service = fixture_moderation_operator_service(
            quarantine_id,
            Some(5),
            moderation_operator_panel_fixture! {
                quarantine_id_hex;
                "case_count": 0_u64,
                "returned_case_count": 0_u64,
                "cases": [],
                "next_actions": [{
                    "action": "review",
                    "route": "/v1/sorafs/moderation/quarantine/review",
                    "required": true
                }]
            },
        );
        let response = handle_moderation_operator_raw_request(
            &service,
            format!(
                "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/bridge-plan HTTP/1.1\r\nHost: local\r\n\r\n"
            ),
        );
        assert_eq!(response.status, StatusCode::OK);
        let value: Value = norito::json::from_slice(&response.body).expect("bridge plan JSON");
        assert_eq_compact! { value.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.quarantine.bridge_plan.v1") };
        assert_eq_compact! { value.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
        let actions = value
            .get("actions")
            .and_then(Value::as_array)
            .expect("planned actions");
        assert_eq_compact! { actions[0].get("automation_status").and_then(Value::as_str) => Some("operator_review_required") };
    }
    fn moderation_operator_service_builds_juror_plan_without_payload() {
        let quarantine_id = [0xA4_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let service = fixture_moderation_operator_service(
            quarantine_id,
            Some(2),
            moderation_operator_panel_fixture! {
                quarantine_id_hex;
                "case_count": 1_u64,
                "returned_case_count": 1_u64,
                "truncated_cases": false,
                "cases": [(
                    fixture_finalized_moderation_case(
                        &["juror-a@moderation", "juror-b@moderation"],
                        &["juror-a@moderation"],
                        &[],
                    )
                )],
                "next_actions": []
            },
        );
        let response = handle_moderation_operator_raw_request(
            &service,
            format!(
                "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/juror-plan HTTP/1.1\r\nHost: local\r\n\r\n"
            ),
        );
        assert_eq!(response.status, StatusCode::OK);
        let body = String::from_utf8(response.body.clone()).expect("juror plan body UTF-8");
        assert!(!body.contains("payload_b64"));
        let value: Value = norito::json::from_slice(&response.body).expect("juror plan JSON");
        assert_eq_compact! { value.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.quarantine.juror_plan.v1") };
        assert_eq_compact! { value.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
        assert_eq_compact! { value.get("notification_count").and_then(Value::as_u64) => Some(2) };
        assert_eq_compact! { value.get("pending_commit_count").and_then(Value::as_u64) => Some(1) };
        assert_eq_compact! { value.get("pending_reveal_count").and_then(Value::as_u64) => Some(1) };
        let ballots = value
            .get("ballots")
            .and_then(Value::as_array)
            .expect("planned ballots");
        let jurors = ballots[0]
            .get("jurors")
            .and_then(Value::as_array)
            .expect("planned jurors");
        assert_eq_compact! { jurors[0].get("notification_status").and_then(Value::as_str) => Some("reveal_required") };
        assert_eq_compact! { jurors[0].get("signed_by").and_then(Value::as_str) => Some("juror-a@moderation") };
        assert_eq_compact! { jurors[1].get("notification_status").and_then(Value::as_str) => Some("commit_required") };
        assert_eq_compact! { jurors[1].get("routes").and_then(Value::as_object).and_then(|routes| routes.get("commit")).and_then(Value::as_str) => Some("/v1/sorafs/moderation/ballots/commits") };
    }
    fn moderation_operator_service_builds_juror_notifications_without_payload() {
        let quarantine_id = [0xA6_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let service = fixture_moderation_operator_service(
            quarantine_id,
            Some(3),
            moderation_operator_panel_fixture! {
                quarantine_id_hex;
                "case_count": 1_u64,
                "returned_case_count": 1_u64,
                "truncated_cases": false,
                "cases": [(
                    fixture_finalized_moderation_case(
                        &[
                            "juror-a@moderation",
                            "juror-b@moderation",
                            "juror-c@moderation",
                        ],
                        &["juror-a@moderation", "juror-c@moderation"],
                        &["juror-c@moderation"],
                    )
                )],
                "next_actions": []
            },
        );
        let response = handle_moderation_operator_raw_request(
            &service,
            format!(
                "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/juror-notifications HTTP/1.1\r\nHost: local\r\n\r\n"
            ),
        );
        assert_eq!(response.status, StatusCode::OK);
        let body = String::from_utf8(response.body.clone()).expect("notification body UTF-8");
        assert!(!body.contains("payload_b64"));
        let value: Value =
            norito::json::from_slice(&response.body).expect("juror notifications JSON");
        assert_eq_compact! { value.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.quarantine.juror_notifications.v1") };
        assert_eq_compact! { value.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
        assert_eq_compact! { value.get("private_payloads_included").and_then(Value::as_bool) => Some(false) };
        assert_eq_compact! { value.get("planned_juror_count").and_then(Value::as_u64) => Some(3) };
        assert_eq_compact! { value.get("notification_count").and_then(Value::as_u64) => Some(2) };
        assert_eq_compact! { value.get("skipped_complete_count").and_then(Value::as_u64) => Some(1) };
        let notifications = value
            .get("notifications")
            .and_then(Value::as_array)
            .expect("notifications");
        assert_eq!(notifications.len(), 2);
        assert_eq_compact! { notifications[0].get("action").and_then(Value::as_str) => Some("submit_reveal") };
        assert_eq_compact! { notifications[0].get("route").and_then(Value::as_str) => Some("/v1/sorafs/moderation/ballots/reveals") };
        assert_eq_compact! { notifications[1].get("action").and_then(Value::as_str) => Some("submit_commit") };
        assert_eq_compact! { notifications[1].get("route").and_then(Value::as_str) => Some("/v1/sorafs/moderation/ballots/commits") };
        assert_eq_compact! { notifications[1].get("private_payload_included").and_then(Value::as_bool) => Some(false) };
        assert_eq_compact! { notifications[1].get("delivery_id").and_then(Value::as_str).map(str::len) => Some(64) };
        assert_compact! { notifications[1].get("body").and_then(Value::as_str).expect("notification body").contains("carries no payload bytes") };
    }
    fn moderation_operator_service_builds_commit_reveal_status_without_payload() {
        let quarantine_id = [0xA8_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let service = fixture_moderation_operator_service(
            quarantine_id,
            Some(3),
            moderation_operator_panel_fixture! {
                quarantine_id_hex;
                "case_count": 1_u64,
                "returned_case_count": 1_u64,
                "truncated_cases": false,
                "cases": [(
                    fixture_finalized_moderation_case(
                        &[
                            "juror-a@moderation",
                            "juror-b@moderation",
                            "juror-c@moderation",
                        ],
                        &["juror-a@moderation", "juror-c@moderation"],
                        &["juror-a@moderation", "juror-c@moderation"],
                    )
                )],
                "next_actions": []
            },
        );
        let response = handle_moderation_operator_raw_request(
            &service,
            format!(
                "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/commit-reveal-status HTTP/1.1\r\nHost: local\r\n\r\n"
            ),
        );
        assert_eq!(response.status, StatusCode::OK);
        let body = String::from_utf8(response.body.clone()).expect("status body UTF-8");
        assert!(!body.contains("payload_b64"));
        let value: Value =
            norito::json::from_slice(&response.body).expect("commit/reveal status JSON");
        assert_eq_compact! { value.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.quarantine.commit_reveal_status.v1") };
        assert_eq_compact! { value.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
        assert_eq_compact! { value.get("private_payloads_included").and_then(Value::as_bool) => Some(false) };
        assert_eq_compact! { value.get("tally_ready_count").and_then(Value::as_u64) => Some(1) };
        assert_eq_compact! { value.get("pending_commit_count").and_then(Value::as_u64) => Some(1) };
        assert_eq_compact! { value.get("pending_reveal_count").and_then(Value::as_u64) => Some(0) };
        let ballots = value
            .get("ballots")
            .and_then(Value::as_array)
            .expect("ballot statuses");
        assert_eq_compact! { ballots[0].get("next_action").and_then(Value::as_str) => Some("submit_tally") };
        assert_eq_compact! { ballots[0].get("ready_to_tally").and_then(Value::as_bool) => Some(true) };
        let missing_commit = ballots[0]
            .get("missing_commit_jurors")
            .and_then(Value::as_array)
            .expect("missing commit jurors");
        assert_eq!(missing_commit[0].as_str(), Some("juror-b@moderation"));
        assert_eq_compact! { ballots[0].get("tally_request").and_then(Value::as_object).and_then(|request| request.get("route")).and_then(Value::as_str) => Some("/v1/sorafs/moderation/ballots/tally") };
        assert_eq_compact! { ballots[0].get("tally_request").and_then(Value::as_object).and_then(|request| request.get("submission")).and_then(Value::as_str) => Some("caller-signed-native-transaction") };
        assert_compact! { ballots[0].get("tally_request").and_then(Value::as_object).is_some_and(|request| !request.contains_key("body")) };
    }
    fn moderation_operator_service_rejects_juror_plan_payload_bytes() {
        let quarantine_id = [0xA5_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        assert_moderation_operator_payload_rejected(
            quarantine_id,
            format!(
                "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/juror-plan HTTP/1.1\r\nHost: local\r\n\r\n"
            ),
            quarantine_id_hex,
        );
    }
    fn moderation_operator_service_rejects_juror_notifications_payload_bytes() {
        let quarantine_id = [0xA7_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        assert_moderation_operator_payload_rejected(
            quarantine_id,
            format!(
                "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/juror-notifications HTTP/1.1\r\nHost: local\r\n\r\n"
            ),
            quarantine_id_hex,
        );
    }
    fn moderation_operator_service_rejects_commit_reveal_status_payload_bytes() {
        let quarantine_id = [0xA9_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        assert_moderation_operator_payload_rejected(
            quarantine_id,
            format!(
                "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/commit-reveal-status HTTP/1.1\r\nHost: local\r\n\r\n"
            ),
            quarantine_id_hex,
        );
    }
    fn moderation_operator_service_rejects_payload_b64_from_upstream() {
        let quarantine_id = [0xAD_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let service = fixture_moderation_operator_service(
            quarantine_id,
            None,
            moderation_operator_panel_fixture! {
                quarantine_id_hex;
                "payload_b64": "c2hvdWxkLW5vdC1sZWFr",
                "next_actions": []
            },
        );
        let response = handle_moderation_operator_raw_request(
            &service,
            format!(
                "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/operator-panel HTTP/1.1\r\nHost: local\r\n\r\n"
            ),
        );
        assert_eq!(response.status, StatusCode::BAD_GATEWAY);
        let body = String::from_utf8(response.body).expect("error JSON is UTF-8");
        assert!(body.contains("payload bytes"));
    }
    fn moderation_operator_service_rejects_request_body() {
        let request = b"GET /healthz HTTP/1.1\r\nHost: local\r\nContent-Length: 2\r\n\r\n{}";
        let parsed =
            moderation_operator_parse_http_request(request, 1024).expect("parse request body");
        let args = ModerationQuarantineOperatorServeArgs {
            listen: "127.0.0.1:0".to_string(),
            limit: None,
            max_body_bytes: 1024,
        };
        let service = args
            .service(
                Arc::new(FixtureModerationOperatorPanelSource {
                    expected_quarantine_id_hex: encode([0_u8; 16]),
                    expected_limit: None,
                    status: StatusCode::OK,
                    body: Vec::new(),
                }),
                "http://torii.test/".to_string(),
                "operator@moderation".to_string(),
            )
            .expect("operator service");
        let response = service.handle_request(&parsed);
        assert_eq!(response.status, StatusCode::BAD_REQUEST);
    }
    fn moderation_operator_parse_rejects_post_without_content_length() {
        let quarantine_id_hex = encode([0x42_u8; 16]);
        let request = format!(
            "POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/review HTTP/1.1\r\nHost: local\r\n\r\n{{}}"
        );
        let error = moderation_operator_parse_http_request(request.as_bytes(), 1024)
            .expect_err("POST bodies must declare Content-Length");
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(error.message.contains("requires Content-Length"));
    }
    fn moderation_operator_parse_rejects_body_without_content_length() {
        let request = b"GET /healthz HTTP/1.1\r\nHost: local\r\n\r\n{}";
        let error = moderation_operator_parse_http_request(request, 1024)
            .expect_err("undeclared body bytes must be rejected");
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(error.message.contains("requires Content-Length"));
    }
    fn moderation_operator_parse_rejects_trailing_bytes_after_declared_body() {
        let request = b"GET /healthz HTTP/1.1\r\nHost: local\r\nContent-Length: 0\r\n\r\nGET / HTTP/1.1\r\n\r\n";
        let error = moderation_operator_parse_http_request(request, 1024)
            .expect_err("trailing bytes after declared body must be rejected");
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(error.message.contains("trailing bytes"));
    }
    fn moderation_operator_read_rejects_trailing_bytes_after_declared_body() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("listener address");
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).expect("connect test client");
            stream
                .write_all(
                    b"GET /healthz HTTP/1.1\r\nHost: local\r\nContent-Length: 0\r\n\r\nGET / HTTP/1.1\r\n\r\n",
                )
                .expect("write trailing request bytes");
        });
        let (mut stream, _) = listener.accept().expect("accept test client");
        let error = moderation_operator_read_http_request(&mut stream, 1024)
            .expect_err("socket reader must reject trailing request bytes");
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(error.message.contains("trailing bytes"));
        client.join().expect("client thread finished");
    }
    fn moderation_operator_service_serves_browser_ui() {
        let quarantine_id = [0xA1_u8; 16];
        let service = fixture_moderation_operator_service(quarantine_id, None, norito::json!({}));
        let response = handle_moderation_operator_raw_request(
            &service,
            "GET / HTTP/1.1\r\nHost: local\r\n\r\n".to_string(),
        );
        assert_eq!(response.status, StatusCode::OK);
        assert_eq_compact! { response.content_type => ModerationOperatorService::HTML_CONTENT_TYPE };
        let body = String::from_utf8(response.body.clone()).expect("UI body UTF-8");
        assert!(body.contains("SoraFS Moderation Operator"));
        assert!(body.contains("juror-plan"));
        assert!(body.contains("juror-notifications"));
        assert!(body.contains("commit-reveal-status"));
        assert!(!body.contains(&["ballot", "tally"].join("-")));
        assert!(body.contains(MODERATION_OPERATOR_CSRF_HEADER));
        assert!(body.contains(&service.csrf_token));
        let http = String::from_utf8(response.to_http_bytes()).expect("HTTP response UTF-8");
        assert!(http.contains("Content-Type: text/html; charset=utf-8"));
        assert!(http.contains("X-Content-Type-Options: nosniff"));
    }
    fn moderation_operator_service_status_lists_browser_ui_route() {
        let quarantine_id = [0xA2_u8; 16];
        let service = fixture_moderation_operator_service(quarantine_id, None, norito::json!({}));
        let response = handle_moderation_operator_raw_request(
            &service,
            "GET /v1/sorafs/moderation/operator-panel/status HTTP/1.1\r\nHost: local\r\n\r\n"
                .to_string(),
        );
        assert_eq!(response.status, StatusCode::OK);
        let value: Value = norito::json::from_slice(&response.body).expect("status JSON");
        let routes = value
            .get("routes")
            .and_then(Value::as_array)
            .expect("status routes");
        assert_eq_compact! { value.get("csrf_header").and_then(Value::as_str) => Some(MODERATION_OPERATOR_CSRF_HEADER) };
        assert_eq_compact! { value.get("csrf_token").and_then(Value::as_str) => Some(service.csrf_token.as_str()) };
        assert_compact! { routes.iter().any(|route| { route.as_str() == Some("/v1/sorafs/moderation/operator-panel/ui") }) };
        assert_compact! { routes.iter().any(|route| { route.as_str() == Some("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/juror-plan") }) };
        assert_compact! { routes.iter().any(|route| { route.as_str() == Some("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/juror-notifications") }) };
        assert_compact! { routes.iter().any(|route| { route.as_str() == Some("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/commit-reveal-status") }) };
    }
    fn moderation_operator_service_rejects_browser_ui_request_body() {
        let quarantine_id = [0xA3_u8; 16];
        let service = fixture_moderation_operator_service(quarantine_id, None, norito::json!({}));
        let response = handle_moderation_operator_raw_request(
            &service,
            "GET /v1/sorafs/moderation/operator-panel/ui HTTP/1.1\r\nHost: local\r\nContent-Length: 2\r\n\r\n{}"
                .to_string(),
        );
        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert_eq_compact! { response.content_type => ModerationOperatorService::JSON_CONTENT_TYPE };
    }
    fn moderation_operator_service_rejects_mutation_without_csrf_token() {
        let quarantine_id = [0xA9_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let service = fixture_moderation_operator_mutation_service(
            quarantine_id,
            "review",
            StatusCode::ACCEPTED,
            norito::json!({ "status": "must_not_be_called" }),
        );
        let body = r#"{"notes":"missing token"}"#;
        let response = handle_moderation_operator_raw_request_without_csrf(
            &service,
            format!(
                "POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/review HTTP/1.1\r\nHost: local\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        );
        assert_eq!(response.status, StatusCode::FORBIDDEN);
        let body = String::from_utf8(response.body).expect("error body UTF-8");
        assert!(body.contains(MODERATION_OPERATOR_CSRF_HEADER));
    }
    fn moderation_operator_service_rejects_mutation_with_wrong_csrf_token() {
        let quarantine_id = [0xAA_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let service = fixture_moderation_operator_mutation_service(
            quarantine_id,
            "review",
            StatusCode::ACCEPTED,
            norito::json!({ "status": "must_not_be_called" }),
        );
        let body = r#"{"notes":"wrong token"}"#;
        let response = handle_moderation_operator_raw_request_without_csrf(
            &service,
            format!(
                "POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/review HTTP/1.1\r\nHost: local\r\n{MODERATION_OPERATOR_CSRF_HEADER}: wrong\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        );
        assert_eq!(response.status, StatusCode::FORBIDDEN);
        let body = String::from_utf8(response.body).expect("error body UTF-8");
        assert!(body.contains("CSRF"));
    }
    fn moderation_operator_service_forwards_review_with_default_actor() {
        let quarantine_id = [0xAE_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let service = fixture_moderation_operator_mutation_service(
            quarantine_id,
            "review",
            StatusCode::ACCEPTED,
            norito::json!({
                "schema": "sorafs.moderation.quarantine.review.v1",
                "status": "reviewed"
            }),
        );
        let body = r#"{"reviewed_at_unix":1800000310,"notes":"reviewed through service"}"#;
        let response = handle_moderation_operator_raw_request(
            &service,
            format!(
                "POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/review HTTP/1.1\r\nHost: local\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        );
        assert_eq!(response.status, StatusCode::ACCEPTED);
        let value: Value = norito::json::from_slice(&response.body).expect("review response JSON");
        assert_eq_compact! { value.get("status").and_then(Value::as_str) => Some("reviewed") };
    }
    fn moderation_operator_service_forwards_release_with_explicit_authority() {
        let quarantine_id = [0xAF_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let service = fixture_moderation_operator_mutation_service(
            quarantine_id,
            "release",
            StatusCode::ACCEPTED,
            norito::json!({
                "schema": "sorafs.moderation.quarantine.release.v1",
                "status": "released"
            }),
        );
        let body = r#"{"release_authority":"release@moderation","released_at_unix":1800000320,"notes":"released through service"}"#;
        let response = handle_moderation_operator_raw_request(
            &service,
            format!(
                "POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/release HTTP/1.1\r\nHost: local\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        );
        assert_eq!(response.status, StatusCode::ACCEPTED);
        let value: Value = norito::json::from_slice(&response.body).expect("release response JSON");
        assert_eq_compact! { value.get("status").and_then(Value::as_str) => Some("released") };
    }
    fn moderation_operator_service_forwards_appeal_handoff_payload() {
        let quarantine_id = [0xB0_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let service = fixture_moderation_operator_mutation_service(
            quarantine_id,
            "appeal-handoff",
            StatusCode::OK,
            norito::json!({
                "schema": "sorafs.moderation.quarantine.appeal_handoff.v1",
                "status": "handoff_ready"
            }),
        );
        let body = r#"{"class":"content","backlog":3,"evidence_size_mb":8}"#;
        let response = handle_moderation_operator_raw_request(
            &service,
            format!(
                "POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/appeal-handoff HTTP/1.1\r\nHost: local\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        );
        assert_eq!(response.status, StatusCode::OK);
        let value: Value = norito::json::from_slice(&response.body).expect("handoff response JSON");
        assert_eq_compact! { value.get("status").and_then(Value::as_str) => Some("handoff_ready") };
    }
    fn moderation_operator_service_rejects_mutation_payload_bytes() {
        let quarantine_id = [0xB2_u8; 16];
        let quarantine_id_hex = encode(quarantine_id);
        let service = fixture_moderation_operator_mutation_service(
            quarantine_id,
            "appeal-handoff",
            StatusCode::OK,
            norito::json!({ "status": "must_not_be_called" }),
        );
        let body = r#"{"class":"content","payload_b64":"c2hvdWxkLW5vdC1sZWFr"}"#;
        let response = handle_moderation_operator_raw_request(
            &service,
            format!(
                "POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/appeal-handoff HTTP/1.1\r\nHost: local\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        );
        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        let body = String::from_utf8(response.body).expect("error body UTF-8");
        assert!(body.contains("payload bytes"));
    }
    fn moderation_quarantine_review_rejects_blank_notes() {
        let args = ModerationQuarantineReviewArgs {
            quarantine_id: encode([0xEF_u8; 16]),
            reviewed_by: Some("operator@moderation".to_string()),
            reviewed_at: Some("@1800000210".to_string()),
            notes: Some("   ".to_string()),
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with(&mut ctx, |_client, _, _| {
                unreachable!("submit must not run")
            })
            .expect_err("blank notes must be rejected");
        assert!(err.to_string().contains("--notes"));
        assert!(ctx.printed.is_empty());
    }
    fn repair_ticket_id_rejects_lowercase() {
        let result = parse_repair_ticket_id("rep-1", "--ticket-id");
        assert!(result.is_err(), "lowercase ticket id should fail");
    }
    }
    fn single_repair_action(transaction: &SignedTransaction) -> &ApplySorafsRepairTaskAction {
        let iroha_data_model::transaction::Executable::Instructions(instructions) =
            transaction.instructions()
        else {
            panic!("repair transaction must contain native instructions");
        };
        assert_eq!(instructions.len(), 1);
        instructions[0]
            .as_any()
            .downcast_ref::<ApplySorafsRepairTaskAction>()
            .expect("repair transaction contains ApplySorafsRepairTaskAction")
    }
    test_items! {
        fn repair_list_uses_finalized_task_cursor() {
            let args = RepairListArgs {
                ticket_id: None,
                limit: Some(25),
                expected_finalized_height: Some(7),
                expected_finalized_block_hash: Some(format!("0x{}", "AB".repeat(32))),
                after_task_id: Some("CD".repeat(32)),
            };
            let mut ctx = TestContext::new();
            args.run_with(
                &mut ctx,
                |_client, filter| {
                    assert_eq!(filter.limit, Some(25));
                    assert_eq!(filter.finalized.expected_finalized_height, Some(7));
                    assert_eq_compact! { filter.finalized.expected_finalized_block_hash_hex => Some("ab".repeat(32).as_str()) };
                    assert_eq!(filter.after_task_id_hex, Some("cd".repeat(32).as_str()));
    json_response_fixture!(StatusCode::OK, &norito::json!({
                            "tasks": [ { "ticket_id": "REP-1" } ]
                        }))
                },
                |_client, _, _| unreachable!("single-task lookup should not be called"),
            )
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("\"tasks\""));
        }
        fn repair_claim_builds_native_signed_transaction() {
            let args = RepairClaimArgs {
                ticket_id: "REP-501".to_string(),
                expected_revision: 2,
                lease_duration_ms: 60_000,
                idempotency_key: Some("claim-501".to_string()),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, transaction| {
                let apply = single_repair_action(transaction);
                assert_eq!(apply.ticket_id, "REP-501");
                assert_eq!(apply.expected_revision, 2);
                let SorafsRepairTaskActionV1::Claim(action) = &apply.action else {
                    panic!("expected claim action");
                };
                assert_eq!(action.lease_duration_ms, 60_000);
                assert_eq!(action.idempotency_key, "claim-501");
                Ok(transaction.hash())
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
            assert!(ctx.printed[0].contains("transaction_hash_hex"));
        }
        fn repair_renew_builds_native_signed_transaction() {
            let args = RepairRenewArgs {
                ticket_id: "REP-502".to_string(),
                expected_revision: 3,
                lease_generation: 2,
                lease_duration_ms: 90_000,
                idempotency_key: Some("renew-502".to_string()),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, transaction| {
                let apply = single_repair_action(transaction);
                assert_eq!(apply.ticket_id, "REP-502");
                assert_eq!(apply.expected_revision, 3);
                let SorafsRepairTaskActionV1::Renew(action) = &apply.action else {
                    panic!("expected renew action");
                };
                assert_eq!(action.lease_generation, 2);
                assert_eq!(action.lease_duration_ms, 90_000);
                assert_eq!(action.idempotency_key, "renew-502");
                Ok(transaction.hash())
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
        }
        fn repair_complete_builds_native_signed_transaction() {
            let evidence_digest = [0x33_u8; 32];
            let args = RepairCompleteArgs {
                ticket_id: "REP-503".to_string(),
                expected_revision: 4,
                lease_generation: 2,
                evidence_digest: encode(evidence_digest),
                idempotency_key: Some("complete-503".to_string()),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, transaction| {
                let apply = single_repair_action(transaction);
                assert_eq!(apply.ticket_id, "REP-503");
                assert_eq!(apply.expected_revision, 4);
                let SorafsRepairTaskActionV1::Complete(action) = &apply.action else {
                    panic!("expected complete action");
                };
                assert_eq!(action.lease_generation, 2);
                assert_eq!(action.evidence_digest, evidence_digest);
                assert_eq!(action.idempotency_key, "complete-503");
                Ok(transaction.hash())
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
        }
        fn repair_fail_builds_native_signed_transaction() {
            let failure_digest = [0x55_u8; 32];
            let args = RepairFailArgs {
                ticket_id: "REP-504".to_string(),
                expected_revision: 5,
                lease_generation: 3,
                failure_digest: encode(failure_digest),
                idempotency_key: Some("fail-504".to_string()),
            };
            let mut ctx = TestContext::new();
            args.run_with(&mut ctx, |_client, transaction| {
                let apply = single_repair_action(transaction);
                assert_eq!(apply.ticket_id, "REP-504");
                assert_eq!(apply.expected_revision, 5);
                let SorafsRepairTaskActionV1::Fail(action) = &apply.action else {
                    panic!("expected fail action");
                };
                assert_eq!(action.lease_generation, 3);
                assert_eq!(action.failure_digest, failure_digest);
                assert_eq!(action.idempotency_key, "fail-504");
                Ok(transaction.hash())
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
        }
        fn repair_escalate_builds_native_atomic_slash_transaction() {
            let manifest_digest = [0x77_u8; 32];
            let provider_id = [0x88_u8; 32];
            let args = RepairEscalateArgs {
                ticket_id: "REP-505".to_string(),
                expected_revision: 6,
                lease_generation: 4,
                manifest_digest: encode(manifest_digest),
                provider_id: encode(provider_id),
                penalty: "0.0000009".to_owned(),
                rationale: "sla_missed".to_string(),
                auditor: None,
                submitted_at: Some("@1700000504".to_string()),
                idempotency_key: Some("escalate-505".to_string()),
            };
            let mut ctx = TestContext::new();
            let expected_auditor = ctx.config().account.to_string();
            args.run_with(&mut ctx, |_client, transaction| {
                let apply = single_repair_action(transaction);
                assert_eq!(apply.ticket_id, "REP-505");
                assert_eq!(apply.expected_revision, 6);
                let SorafsRepairTaskActionV1::Escalate(action) = &apply.action else {
                    panic!("expected escalate action");
                };
                assert_eq!(action.lease_generation, 4);
                assert_eq!(action.idempotency_key, "escalate-505");
                let proposal: RepairSlashProposalV1 =
                    norito::decode_from_bytes(&action.slash_proposal_payload)
                        .expect("decode canonical slash proposal");
                assert_eq!(proposal.ticket_id.0, "REP-505");
                assert_eq!(proposal.provider_id, provider_id);
                assert_eq!(proposal.manifest_digest, manifest_digest);
                assert_eq!(proposal.auditor_account, expected_auditor);
                assert_eq_compact! { proposal.proposed_penalty => "0.0000009".parse::<XorQuantity>().expect("valid quantity") };
                assert_eq!(proposal.submitted_at_unix, 1_700_000_504);
                assert!(proposal.approval.is_none());
                Ok(transaction.hash())
            })
            .expect("run should succeed");
            assert_eq!(ctx.printed.len(), 1);
        }
        fn repair_action_rejects_zero_compare_and_set_revision() {
            let args = RepairClaimArgs {
                ticket_id: "REP-506".to_string(),
                expected_revision: 0,
                lease_duration_ms: 60_000,
                idempotency_key: Some("claim-506".to_string()),
            };
            let mut ctx = TestContext::new();
            let error = args
                .run_with(&mut ctx, |_client, _| {
                    unreachable!("zero revision must fail before submission")
                })
                .expect_err("zero compare-and-set revision must fail");
            assert!(error.to_string().contains("--expected-revision"));
            assert!(ctx.printed.is_empty());
        }
        fn gc_inspect_reports_expiry_state() {
            let dir = TempDir::new().expect("temp dir");
            write_gc_manifest(
                dir.path(),
                "alpha",
                1_000,
                ManifestStorageClass::Hot,
                100,
                10,
            );
            write_gc_manifest(
                dir.path(),
                "beta",
                2_000,
                ManifestStorageClass::Warm,
                200,
                20,
            );
            write_gc_manifest(dir.path(), "gamma", 0, ManifestStorageClass::Cold, 300, 30);
            let report = build_gc_report("inspect", Some(dir.path()), Some("@1500"), Some(100), false)
                .expect("report");
            assert_eq!(report.mode, "inspect");
            assert_eq!(report.total_manifests, 3);
            assert_eq!(report.total_payload_bytes, 600);
            assert_eq!(report.total_car_bytes, 60);
            assert_eq!(report.expired_count, 1);
            assert_eq!(report.expired_payload_bytes, 100);
            assert_eq!(report.expired_car_bytes, 10);
            assert_eq!(report.entries.len(), 3);
            assert_eq!(report.now_unix, 1_500);
            assert_eq!(report.grace_secs, 100);
            let first = &report.entries[0];
            assert_eq!(first.manifest_id, "alpha");
            assert_eq!(first.storage_class, "hot");
            assert_eq!(first.expires_at_unix, Some(1_100));
            assert!(first.expired);
            assert_eq!(first.payload_bytes, 100);
            assert_eq!(first.car_bytes, 10);
            assert_eq!(first.manifest_digest_hex.len(), 64);
            let last = &report.entries[2];
            assert_eq!(last.manifest_id, "gamma");
            assert_eq!(last.storage_class, "cold");
            assert_eq!(last.expires_at_unix, None);
            assert!(!last.expired);
        }
        fn gc_dry_run_filters_expired() {
            let dir = TempDir::new().expect("temp dir");
            write_gc_manifest(
                dir.path(),
                "alpha",
                1_000,
                ManifestStorageClass::Hot,
                100,
                10,
            );
            write_gc_manifest(
                dir.path(),
                "beta",
                2_000,
                ManifestStorageClass::Warm,
                200,
                20,
            );
            let report = build_gc_report("dry_run", Some(dir.path()), Some("@1500"), Some(100), true)
                .expect("report");
            assert_eq!(report.mode, "dry_run");
            assert_eq!(report.total_manifests, 2);
            assert_eq!(report.expired_count, 1);
            assert_eq!(report.entries.len(), 1);
            assert_eq!(report.entries[0].manifest_id, "alpha");
            assert!(report.entries[0].expired);
        }
        fn gc_inspect_command_prints_json_report() {
            let dir = TempDir::new().expect("temp dir");
            write_gc_manifest(dir.path(), "alpha", 0, ManifestStorageClass::Hot, 50, 5);
            let args = GcInspectArgs {
                data_dir: Some(dir.path().to_path_buf()),
                now: Some("@1500".to_string()),
                grace_secs: Some(100),
            };
            let mut ctx = TestContext::new();
            GcCommand::Inspect(args).run(&mut ctx).expect("inspect run");
            let output = ctx.outputs().last().expect("output");
            let json: Value = norito::json::from_str(output).expect("json");
            assert_eq!(json["mode"], Value::from("inspect"));
            assert_eq!(json["total_manifests"], Value::from(1u64));
            assert_eq!(json["entries"].as_array().map(Vec::len), Some(1));
        }
        fn gc_dry_run_command_filters_json_entries() {
            let dir = TempDir::new().expect("temp dir");
            write_gc_manifest(
                dir.path(),
                "alpha",
                1_000,
                ManifestStorageClass::Warm,
                10,
                1,
            );
            write_gc_manifest(dir.path(), "beta", 2_000, ManifestStorageClass::Cold, 20, 2);
            let args = GcDryRunArgs {
                data_dir: Some(dir.path().to_path_buf()),
                now: Some("@1500".to_string()),
                grace_secs: Some(100),
            };
            let mut ctx = TestContext::new();
            GcCommand::DryRun(args).run(&mut ctx).expect("dry run");
            let output = ctx.outputs().last().expect("output");
            let json: Value = norito::json::from_str(output).expect("json");
            assert_eq!(json["mode"], Value::from("dry_run"));
            assert_eq!(json["total_manifests"], Value::from(2u64));
            assert_eq!(json["entries"].as_array().map(Vec::len), Some(1));
        }
        fn gc_manifest_entries_require_manifest_dir() {
            let dir = TempDir::new().expect("temp dir");
            let err = load_gc_manifest_entries(dir.path()).expect_err("missing manifests");
            assert_compact! { err.to_string().contains("SoraFS manifests directory"); "unexpected error: {err}" };
        }
        fn gc_retention_deadline_respects_zero_epoch() {
            assert_eq!(retention_deadline(0, 5), None);
            assert_eq!(retention_deadline(10, 5), Some(15));
        }
        fn gc_storage_class_labels_match_expected_values() {
            assert_eq_compact! { manifest_storage_class_label(ManifestStorageClass::Hot) => "hot" };
            assert_eq_compact! { manifest_storage_class_label(ManifestStorageClass::Warm) => "warm" };
            assert_eq_compact! { manifest_storage_class_label(ManifestStorageClass::Cold) => "cold" };
        }
        fn storage_token_issue_passes_arguments_and_prints_nonce() {
            use std::cell::RefCell;
            let args = StorageTokenIssueArgs {
                manifest_id: "aa".repeat(32),
                provider_id: "bb".repeat(32),
                client_id: "gateway-alpha".into(),
                nonce: None,
                ttl_secs: Some(600),
                max_streams: Some(5),
                rate_limit_bytes: Some(256_000),
                requests_per_minute: Some(90),
            };
            let mut ctx = TestContext::new();
            let captured = RefCell::new(None);
            args.run_with(
                &mut ctx,
                |_, manifest, provider, client_id, nonce, overrides| {
                    *captured.borrow_mut() = Some((
                        manifest.to_owned(),
                        provider.to_owned(),
                        client_id.to_owned(),
                        nonce.to_owned(),
                        *overrides,
                ));
                let body = norito::json::to_vec(&norito::json!({ "token": { "body": {} } }))?;
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(body)
                    .unwrap())
            },
            )
            .expect("token issue succeeds");
            let (manifest, provider, client_id, nonce, overrides) =
                captured.borrow().clone().expect("captured arguments");
            assert_eq!(manifest, "aa".repeat(32));
            assert_eq!(provider, "bb".repeat(32));
            assert_eq!(client_id, "gateway-alpha");
            assert_eq!(overrides.ttl_secs, Some(600));
            assert_eq!(overrides.max_streams, Some(5));
            assert_eq!(overrides.rate_limit_bytes, Some(256_000));
            assert_eq!(overrides.requests_per_minute, Some(90));
            assert_eq!(ctx.printed.len(), 2);
            assert_compact! { ctx.printed[0].starts_with("nonce: "); "expected nonce println, got {}", ctx.printed[0] };
            assert_eq_compact! { ctx.printed[1] => "{\"token\":{\"body\":{}}}"; "expected JSON payload output" };
            assert_eq_compact! { nonce.len() => 24; "nonce should be 12 random bytes hex encoded" };
        }
        fn direct_mode_plan_generates_summary() {
            let manifest = ManifestBuilder::new()
                .root_cid(vec![0x01, 0x02, 0x03])
                .dag_codec(DagCodecId(0x71))
                .chunking_profile(ChunkingProfileV1 {
                    profile_id: ProfileId(7),
                    namespace: "sorafs".into(),
                    name: "sf1".into(),
                    semver: "1.0.0".into(),
                    min_size: 4096,
                    target_size: 262_144,
                    max_size: 524_288,
                    break_mask: 0,
                    multihash_code: BLAKE3_256_MULTIHASH_CODE,
                    aliases: vec!["sf1".into()],
                })
                .chunk_digest_sha3_256([0xCD; 32])
                .por_root([0xCE; 32])
                .content_length(1_048_576)
                .car_digest([0xAB; 32])
                .car_size(1_111_111)
                .pin_policy(PinPolicy {
                    min_replicas: 3,
                    storage_class: ManifestStorageClass::Hot,
                    retention_epoch: 0,
                })
                .add_metadata("manifest.requires_envelope", "true")
                .add_metadata("capability.direct_car", "true")
                .build()
                .expect("build manifest");
            let bytes = to_bytes(&manifest).expect("encode manifest");
            let mut temp_manifest = NamedTempFile::new().expect("temp manifest");
            temp_manifest
                .write_all(&bytes)
                .expect("write manifest bytes");
            let provider = [0xAA; 32];
            let args = GatewayDirectModePlanArgs {
                manifest: temp_manifest.path().to_path_buf(),
                admission_envelope: None,
                provider_id: Some(hex::encode(provider)),
                chain_id: Some("nexus".to_owned()),
                scheme: "https".to_owned(),
            };
            let mut ctx = TestContext::new();
            args.run(&mut ctx).expect("plan command runs");
            assert_eq!(ctx.outputs().len(), 1);
            let plan: DirectModePlanOutput =
                norito::json::from_str(&ctx.outputs()[0]).expect("parse plan");
            assert_eq!(plan.provider_id_hex, hex::encode(provider));
            assert_eq!(plan.chain_id, "nexus");
            assert_compact! { plan.direct_car.canonical_url.contains("/direct/v1/car/"); "direct car locator should reference the manifest digest" };
            assert!(plan.capabilities.direct_car_supported);
        }
        fn toolkit_pack_emits_manifest_and_report() {
            let temp = TempDir::new().expect("temp dir");
            let payload_path = temp.path().join("payload.bin");
            fs::write(&payload_path, b"payload-bytes").expect("write payload");
            let manifest_path = temp.path().join("manifest.to");
            let car_path = temp.path().join("payload.car");
            let json_path = temp.path().join("report.json");
            let args = ToolkitPackArgs {
                input: payload_path,
                manifest_out: Some(manifest_path.clone()),
                car_out: Some(car_path.clone()),
                json_out: Some(json_path.clone()),
                hybrid_envelope_out: None,
                hybrid_envelope_json_out: None,
                hybrid_recipient_x25519: None,
                hybrid_recipient_kyber: None,
            };
            let mut ctx = TestContext::new();
            args.run(&mut ctx).expect("pack");
            let manifest_bytes = fs::read(&manifest_path).expect("read manifest");
            let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes).expect("decode manifest");
            assert_eq!(manifest.content_length, 13);
            let car_bytes = fs::read(&car_path).expect("read CAR archive");
            assert_eq!(manifest.car_size, car_bytes.len() as u64);
            let archive_digest = *blake3::hash(&car_bytes).as_bytes();
            let archive_digest_hex = hex::encode(archive_digest);
            assert_eq_compact! { manifest.car_digest => archive_digest; "manifest must bind every byte of the canonical CARv2 archive" };
            let report_bytes = fs::read(&json_path).expect("read report");
            let report: Value = norito::json::from_slice(&report_bytes).expect("decode report");
            assert_eq_compact! { report.get("car_archive_digest_hex").and_then(Value::as_str) => Some(archive_digest_hex.as_str()) };
            assert_eq_compact! { report.get("manifest").and_then(|manifest| manifest.get("car_digest_hex")).and_then(Value::as_str) => Some(archive_digest_hex.as_str()) };
            assert_ne!(
                report.get("car_payload_digest_hex").and_then(Value::as_str),
                Some(archive_digest_hex.as_str()),
                "CARv1 payload-section digest must remain diagnostic-only"
            );
            let digest_hex = hex::encode(manifest.digest().expect("manifest digest").as_bytes());
            assert_eq_compact! { report.get("manifest_digest_hex").and_then(Value::as_str) => Some(digest_hex.as_str()) };
            let por_root_hex = hex::encode(manifest.por_root);
            assert_eq_compact! { report.get("por_root_hex").and_then(Value::as_str) => Some(por_root_hex.as_str()) };
            assert_eq_compact! { report.get("manifest").and_then(|manifest| manifest.get("por_root_hex")).and_then(Value::as_str) => Some(por_root_hex.as_str()) };
        }
        fn hybrid_manifest_aad_appends_filename() {
            let digest = blake3::hash(b"manifest");
            let chunk_digest = [0x11; 32];
            let aad = build_hybrid_manifest_aad(&digest, chunk_digest, Some("manifest.to"));
            let name_len_offset = HYBRID_MANIFEST_AAD_DOMAIN.len() + 32 + 32;
            let length_bytes: [u8; 4] = aad[name_len_offset..name_len_offset + 4]
                .try_into()
                .expect("length bytes");
            let length = u32::from_be_bytes(length_bytes);
            assert_eq!(length as usize, "manifest.to".len());
            assert_eq!(&aad[name_len_offset + 4..], b"manifest.to");
        }
        fn chunk_digest_sha3_matches_manual_hash() {
            let payload = b"hello-world".to_vec();
            let plan =
                CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
            let computed = compute_chunk_digest_sha3(&plan.chunks);
            let mut hasher = Sha3::v256();
            for chunk in &plan.chunks {
                hasher.update(&chunk.offset.to_le_bytes());
                hasher.update(&u64::from(chunk.length).to_le_bytes());
                hasher.update(&chunk.digest);
            }
            let mut expected = [0u8; 32];
            hasher.finalize(&mut expected);
            assert_eq!(computed, expected);
        }
        fn ensure_metadata_entry_dedupes_case_insensitive() {
            let mut metadata = vec![("manifest.requires_envelope".to_string(), "true".to_string())];
            ensure_metadata_entry(&mut metadata, "Manifest.Requires_Envelope", "false");
            assert_eq!(metadata.len(), 1);
            ensure_metadata_entry(&mut metadata, "manifest.hybrid_suite", "suite");
            assert_eq!(metadata.len(), 2);
        }
        }
    fn direct_mode_enable_capabilities() -> ManifestCapabilitySummary {
        ManifestCapabilitySummary {
            direct_car_supported: true,
            ..ManifestCapabilitySummary::default()
        }
    }
    fn direct_mode_enable_test_plan(
        capabilities: ManifestCapabilitySummary,
    ) -> DirectModePlanOutput {
        let chain_id = "nexus";
        let provider = [0x33; 32];
        let manifest_digest_hex = "fe".repeat(32);
        let host_input = HostMappingInput {
            chain_id,
            provider_id: &provider,
        };
        let hosts = host_input.to_summary();
        let direct_car = host_input
            .direct_car_locator("https", &manifest_digest_hex)
            .expect("direct CAR locator");
        DirectModePlanOutput::from_components(
            chain_id,
            provider,
            manifest_digest_hex,
            hosts,
            direct_car,
            capabilities,
        )
    }
    fn write_direct_mode_plan(plan: &DirectModePlanOutput) -> NamedTempFile {
        let mut plan_file = NamedTempFile::new().expect("temp plan");
        plan_file
            .write_all(&norito::json::to_vec(plan).expect("serialize plan"))
            .expect("write plan");
        plan_file
    }
    pub(super) fn assert_sorafs_config_snippet_is_schema_valid(snippet: &str) -> UserSorafsConfig {
        let mut root: toml::Table =
            toml::from_str(snippet).expect("generated snippet must parse as TOML");
        let sorafs = root
            .remove("sorafs")
            .expect("generated snippet must use the top-level `sorafs` table");
        assert_compact! { root.is_empty(); "generated snippet contains unexpected top-level keys: {root:?}" };
        let sorafs = sorafs
            .as_table()
            .expect("top-level `sorafs` value must be a table")
            .clone();
        ConfigReader::new()
            .with_toml_source(TomlSource::inline(sorafs))
            .read_and_complete::<UserSorafsConfig>()
            .expect("generated snippet must satisfy the iroha_config SoraFS schema")
    }
    test_items! {
    fn direct_mode_enable_renders_snippet() {
        let plan = direct_mode_enable_test_plan(direct_mode_enable_capabilities());
        let plan_file = write_direct_mode_plan(&plan);
        let args = GatewayDirectModeEnableArgs {
            plan: plan_file.path().to_path_buf(),
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("enable command runs");
        assert_eq!(ctx.outputs().len(), 1);
        let snippet = &ctx.outputs()[0];
        assert!(snippet.contains("require_manifest_envelope = true"));
        assert!(snippet.contains("enforce_admission = true"));
        assert!(snippet.contains("enforce_capabilities = true"));
        assert!(!snippet.contains(" = false"));
        assert!(snippet.contains("direct_car_canonical"));
        assert!(snippet.contains(&plan.provider_id_hex));
        assert!(snippet.contains("[sorafs.gateway.direct_mode]"));
        assert!(!snippet.contains("[torii.sorafs_gateway]"));
        assert_sorafs_config_snippet_is_schema_valid(snippet);
    }
    fn direct_mode_enable_rejects_missing_direct_car_capability() {
        let plan = direct_mode_enable_test_plan(ManifestCapabilitySummary::default());
        let plan_file = write_direct_mode_plan(&plan);
        let args = GatewayDirectModeEnableArgs {
            plan: plan_file.path().to_path_buf(),
        };
        let mut ctx = TestContext::new();
        let err = args
            .run(&mut ctx)
            .expect_err("missing direct-CAR capability must fail");
        assert!(format!("{err:#}").contains("capabilities.direct_car_supported=true"));
        assert!(ctx.outputs().is_empty());
    }
    fn direct_mode_enable_rejects_manifest_envelope_disabled() {
        let capabilities = ManifestCapabilitySummary {
            requires_manifest_envelope: false,
            direct_car_supported: true,
            ..ManifestCapabilitySummary::default()
        };
        let plan = direct_mode_enable_test_plan(capabilities);
        let plan_file = write_direct_mode_plan(&plan);
        let args = GatewayDirectModeEnableArgs {
            plan: plan_file.path().to_path_buf(),
        };
        let mut ctx = TestContext::new();
        let err = args
            .run(&mut ctx)
            .expect_err("disabled envelope enforcement must fail");
        assert!(format!("{err:#}").contains("requires_manifest_envelope=true"));
        assert!(ctx.outputs().is_empty());
    }
    fn direct_mode_enable_rejects_tampered_direct_car_locator() {
        let mut plan = direct_mode_enable_test_plan(direct_mode_enable_capabilities());
        plan.direct_car.canonical_url = format!(
            "https://evil.example/direct/v1/car/{}",
            plan.manifest_digest_hex
        );
        let plan_file = write_direct_mode_plan(&plan);
        let args = GatewayDirectModeEnableArgs {
            plan: plan_file.path().to_path_buf(),
        };
        let mut ctx = TestContext::new();
        let err = args
            .run(&mut ctx)
            .expect_err("tampered direct-CAR locator must fail");
        assert!(format!("{err:#}").contains("direct_car.canonical_url mismatch"));
        assert!(ctx.outputs().is_empty());
    }
    fn direct_mode_toml_string_escape_blocks_config_injection() {
        assert_eq_compact! { escape_toml_basic_string("nexus\"\nenforce_admission = false\\") => "nexus\\\"\\nenforce_admission = false\\\\" };
    }
    fn direct_mode_rollback_snippet_matches_defaults() {
        let args = GatewayDirectModeRollbackArgs;
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("rollback command runs");
        assert_eq_compact! { ctx.outputs() => &[render_direct_mode_rollback_snippet().to_owned()] };
        let snippet = &ctx.outputs()[0];
        assert!(snippet.contains("[sorafs.gateway]"));
        assert!(!snippet.contains("[torii.sorafs_gateway]"));
        assert_sorafs_config_snippet_is_schema_valid(snippet);
    }
    fn gateway_route_plan_writes_plan_and_headers() {
        use base64::engine::general_purpose::STANDARD as BASE64;
        use tempfile::TempDir;
        let tmp = TempDir::new().expect("temp dir");
        let manifest_path = tmp.path().join("manifest.json");
        fs::write(&manifest_path, r#"{"root_cid":[1,2,3]}"#).expect("write manifest");
        let output_path = tmp.path().join("route_plan.json");
        let headers_path = tmp.path().join("gateway.route.headers.txt");
        let args = GatewayRoutePlanArgs {
            manifest_json: manifest_path.clone(),
            hostname: "docs.sora.link".to_owned(),
            alias: Some("sora:docs".to_owned()),
            route_label: Some("docs@2026-03-21".to_owned()),
            proof_status: None,
            release_tag: Some("v2026.03.21".to_owned()),
            cutover_window: Some("2026-03-21T15:00Z/2026-03-21T15:30Z".to_owned()),
            output_path: output_path.clone(),
            headers_out: Some(headers_path.clone()),
            rollback_manifest_json: None,
            rollback_headers_out: None,
            rollback_route_label: None,
            rollback_release_tag: None,
            no_csp: false,
            no_permissions_policy: false,
            no_hsts: false,
            now_override: Some("2026-03-21T10:00:00Z".to_owned()),
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx)
            .expect("route plan command should succeed");
        let plan_bytes = fs::read(&output_path).expect("route plan json");
        let plan: Value =
            norito::json::from_slice(&plan_bytes).expect("route plan JSON should parse");
        assert_eq_compact! { plan["manifest_json"].as_str().expect("manifest string") => manifest_path.display().to_string() };
        assert_eq_compact! { plan["hostname"].as_str().expect("hostname string") => "docs.sora.link" };
        let headers = plan["headers"].as_object().expect("headers object missing");
        assert_eq_compact! { headers["Sora-Name"].as_str().expect("Sora-Name must exist") => "sora:docs" };
        let proof_json = BASE64
            .decode(headers["Sora-Proof"].as_str().expect("Sora-Proof base64"))
            .expect("decode proof payload");
        let proof_value: Value =
            norito::json::from_slice(&proof_json).expect("decode proof payload JSON");
        assert_eq_compact! { proof_value["alias"].as_str().expect("alias string") => "sora:docs" };
        assert_compact! { plan["headers_template"].as_str().expect("headers template string").contains("Sora-Route-Binding"); "expected rendered header template" };
        let header_file = fs::read_to_string(&headers_path).expect("header template");
        assert!(header_file.contains("Sora-Content-CID"));
        assert_compact! { ctx.outputs().iter().any(|line| line.contains(output_path.to_string_lossy().as_ref())) };
    }
    fn gateway_route_plan_supports_rollback_and_toggles() {
        use tempfile::TempDir;
        let tmp = TempDir::new().expect("temp dir");
        let manifest_path = tmp.path().join("manifest.json");
        let rollback_path = tmp.path().join("rollback.json");
        fs::write(&manifest_path, r#"{"root_cid_hex":"0102"}"#).expect("write manifest");
        fs::write(&rollback_path, r#"{"root_cid":[240,5]}"#).expect("write rollback manifest");
        let output_path = tmp.path().join("route_plan.json");
        let headers_path = tmp.path().join("gateway.route.headers.txt");
        let rollback_headers_path = tmp.path().join("gateway.route.rollback.headers.txt");
        let args = GatewayRoutePlanArgs {
            manifest_json: manifest_path.clone(),
            hostname: "nexus.sora.link".to_owned(),
            alias: None,
            route_label: None,
            proof_status: None,
            release_tag: None,
            cutover_window: None,
            output_path: output_path.clone(),
            headers_out: Some(headers_path.clone()),
            rollback_manifest_json: Some(rollback_path.clone()),
            rollback_headers_out: Some(rollback_headers_path.clone()),
            rollback_route_label: Some("docs@previous".to_owned()),
            rollback_release_tag: Some("previous".to_owned()),
            no_csp: true,
            no_permissions_policy: true,
            no_hsts: true,
            now_override: Some("2026-03-21T10:00:00Z".to_owned()),
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx)
            .expect("route plan command should succeed");
        let plan_bytes = fs::read(&output_path).expect("route plan json");
        let plan: Value =
            norito::json::from_slice(&plan_bytes).expect("route plan JSON should parse");
        assert!(plan["headers"].get("Content-Security-Policy").is_none());
        assert!(plan["headers"].get("Permissions-Policy").is_none());
        assert!(plan["headers"].get("Strict-Transport-Security").is_none());
        let rollback = plan["rollback"]
            .as_object()
            .expect("rollback object missing");
        assert_eq_compact! { rollback["manifest_json"].as_str().expect("rollback manifest") => rollback_path.display().to_string() };
        assert_eq_compact! { rollback["release_tag"].as_str().expect("release tag") => "previous" };
        assert_compact! { rollback.get("headers_path").and_then(Value::as_str).is_some_and(|value| value.contains("gateway.route.rollback.headers.txt")) };
        let header_file = fs::read_to_string(&headers_path).expect("header template");
        assert_compact! { !header_file.contains("Content-Security-Policy"); "CSP header should be omitted when --no-csp is set" };
        let rollback_headers =
            fs::read_to_string(&rollback_headers_path).expect("rollback header template");
        assert_compact! { rollback_headers.contains("Sora-Route-Binding"); "rollback template should include Sora-Route-Binding" };
        assert_compact! { ctx.outputs().iter().any(|line| line.contains("rollback headers written")); "expected rollback output message" };
    }
    fn gateway_cache_invalidate_prints_payload_and_curl() {
        let args = GatewayCacheInvalidateArgs {
            endpoint: "https://cache.example.com/purge".to_owned(),
            aliases: vec!["docs:portal".to_owned()],
            manifest_digest_hex: "AA".repeat(32),
            car_digest_hex: None,
            release_tag: Some("portal-2026.04.01".to_owned()),
            auth_env: "CACHE_TOKEN".to_owned(),
            output: None,
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("cache invalidate command runs");
        assert_eq!(ctx.outputs().len(), 2);
        let payload: Value = norito::json::from_str(&ctx.outputs()[0]).expect("json payload");
        assert_eq_compact! { payload["aliases"] => Value::Array(vec![Value::from("docs:portal")]) };
        assert_eq!(payload["manifest_digest_hex"], Value::from("aa".repeat(32)));
        assert_eq!(payload["release_tag"], Value::from("portal-2026.04.01"));
        assert_eq!(payload["car_digest_hex"], Value::Null);
        let curl = &ctx.outputs()[1];
        assert_compact! { curl.contains("https://cache.example.com/purge"); "curl snippet should reference endpoint" };
        assert_compact! { curl.contains("Authorization: Bearer $CACHE_TOKEN"); "curl snippet should reference the auth env var" };
    }
    fn gateway_cache_invalidate_writes_payload_file() {
        let temp_payload = NamedTempFile::new().expect("temp payload file");
        let path = temp_payload.into_temp_path();
        let args = GatewayCacheInvalidateArgs {
            endpoint: "https://cache.example.com/purge".to_owned(),
            aliases: vec!["docs:portal".to_owned(), "sns:preview".to_owned()],
            manifest_digest_hex: "bb".repeat(32),
            car_digest_hex: Some("cc".repeat(32)),
            release_tag: None,
            auth_env: String::new(),
            output: Some(path.to_path_buf()),
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("cache invalidate command runs");
        assert_eq!(ctx.outputs().len(), 2);
        assert_eq_compact! { ctx.outputs()[0] => format!("wrote cache invalidation payload to {}", path.display()) };
        let payload_str = std::fs::read_to_string(&path).expect("read payload");
        let payload: Value = norito::json::from_str(&payload_str).expect("json payload");
        assert_eq!(payload["release_tag"], Value::Null);
        assert_eq!(payload["car_digest_hex"], Value::from("cc".repeat(32)));
        let curl = &ctx.outputs()[1];
        assert_compact! { curl.contains("--data '{"); "curl snippet should embed the JSON payload" };
    }
    fn gateway_cache_invalidate_rejects_invalid_alias() {
        let args = GatewayCacheInvalidateArgs {
            endpoint: "https://cache.example.com/purge".to_owned(),
            aliases: vec!["invalid-alias".to_owned()],
            manifest_digest_hex: "aa".repeat(32),
            car_digest_hex: None,
            release_tag: None,
            auth_env: "CACHE_TOKEN".to_owned(),
            output: None,
        };
        let mut ctx = TestContext::new();
        let result = args.run(&mut ctx);
        assert!(result.is_err(), "invalid alias should fail");
    }
    fn incentives_compute_generates_instruction() {
        let mut config_file = NamedTempFile::new().expect("config file");
        config_file
            .write_all(
                &norito::json::to_vec(&sample_reward_config_json()).expect("serialize config"),
            )
            .expect("write config");
        let metrics = sample_metrics();
        let mut metrics_file = NamedTempFile::new().expect("metrics file");
        metrics_file
            .write_all(&to_bytes(&metrics).expect("encode metrics"))
            .expect("write metrics");
        let bond = sample_bond_entry(2_000);
        let mut bond_file = NamedTempFile::new().expect("bond file");
        bond_file
            .write_all(&to_bytes(&bond).expect("encode bond"))
            .expect("write bond");
        let instruction_file = NamedTempFile::new().expect("instruction file");
        let instruction_path = instruction_file.path().to_path_buf();
        let args = IncentivesComputeArgs {
            config: config_file.path().to_path_buf(),
            metrics: metrics_file.path().to_path_buf(),
            bond: bond_file.path().to_path_buf(),
            beneficiary: sample_account_literal("beneficiary"),
            norito_out: Some(instruction_path.clone()),
            pretty: true,
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("compute command runs");
        assert_eq!(ctx.outputs().len(), 1, "expected JSON output");
        let value: norito::json::Value =
            norito::json::from_str(&ctx.outputs()[0]).expect("parse instruction JSON");
        assert_compact! { value.get("relay_id").is_some(); "relay id missing in output" };
        let bytes = std::fs::read(&instruction_path).expect("read instruction");
        let decoded: RelayRewardInstructionV1 =
            decode_from_bytes(&bytes).expect("decode instruction");
        assert_eq!(decoded.beneficiary, sample_account_id("beneficiary"));
        assert!(decoded.payout_amount > Quantity::zero());
    }
    fn incentives_open_dispute_produces_payload() {
        let instruction = sample_reward_instruction();
        let mut instruction_file = NamedTempFile::new().expect("instruction file");
        let instruction_bytes = to_bytes(&instruction).expect("encode instruction");
        instruction_file
            .write_all(&instruction_bytes)
            .expect("write instruction");
        let dispute_file = NamedTempFile::new().expect("dispute file");
        let dispute_path = dispute_file.path().to_path_buf();
        let args = IncentivesOpenDisputeArgs {
            instruction: instruction_file.path().to_path_buf(),
            treasury_account: sample_account_literal("treasury"),
            submitted_by: sample_account_literal("operator"),
            requested_amount: "25".into(),
            reason: "calibration".into(),
            submitted_at: Some(1_234),
            norito_out: Some(dispute_path.clone()),
            pretty: false,
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("open dispute runs");
        assert_eq!(ctx.outputs().len(), 1, "expected JSON output");
        let value: norito::json::Value =
            norito::json::from_str(&ctx.outputs()[0]).expect("parse dispute JSON");
        assert_eq!(value["reason"].as_str(), Some("calibration"));
        let bytes = std::fs::read(&dispute_path).expect("read dispute");
        let dispute: RelayRewardDisputeV1 = decode_from_bytes(&bytes).expect("decode dispute");
        assert_eq!(dispute.submitted_at_unix, 1_234);
        assert_eq!(dispute.submitted_by, sample_account_id("operator"));
    }
    fn incentives_dashboard_summarises_rewards() {
        let mut inst1 = sample_reward_instruction();
        inst1.payout_amount = Quantity::from(40_u32);
        let mut inst1_file = NamedTempFile::new().expect("inst1");
        let inst1_bytes = to_bytes(&inst1).expect("encode inst1");
        inst1_file.write_all(&inst1_bytes).expect("write inst1");
        let mut inst2 = sample_reward_instruction();
        inst2.epoch = inst1.epoch + 1;
        inst2.payout_amount = Quantity::from(10_u32);
        let mut inst2_file = NamedTempFile::new().expect("inst2");
        let inst2_bytes = to_bytes(&inst2).expect("encode inst2");
        inst2_file.write_all(&inst2_bytes).expect("write inst2");
        let args = IncentivesDashboardArgs {
            instructions: vec![
                inst1_file.path().to_path_buf(),
                inst2_file.path().to_path_buf(),
            ],
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("dashboard runs");
        assert_eq!(ctx.outputs().len(), 1, "expected JSON output");
        let summary: norito::json::Value =
            norito::json::from_str(&ctx.outputs()[0]).expect("parse summary");
        assert_eq!(summary["total_relays"].as_u64(), Some(1));
        assert_eq!(summary["total_payout"].as_str(), Some("50"));
        let rows = summary["rows"].as_array().expect("rows present");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["payout_count"].as_u64(), Some(2));
        assert_eq!(rows[0]["payout_amount"].as_str(), Some("50"));
    }
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn incentives_service_shadow_run_generates_summary() {
        fn metrics_for(
            relay_id: RelayId,
            epoch: u32,
            uptime: u32,
            scheduled: u32,
            bandwidth: u128,
            compliance: RelayComplianceStatusV1,
        ) -> RelayEpochMetricsV1 {
            RelayEpochMetricsV1 {
                relay_id,
                epoch,
                uptime_seconds: u64::from(uptime),
                scheduled_uptime_seconds: u64::from(scheduled),
                verified_bandwidth_bytes: bandwidth,
                compliance,
                reward_score: 0,
                confidence_floor_per_mille: 1_000,
                measurement_ids: Vec::new(),
                metadata: Metadata::default(),
            }
        }
        let config_file = write_sample_reward_config_file();
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");
        let _init_ctx = initialize_incentives_state(config_file.path(), &state_path);
        let metrics_dir = tmp_dir.path().join("metrics");
        fs::create_dir_all(&metrics_dir).expect("create metrics dir");
        let relay_a = [0x21_u8; 32];
        let relay_b = [0x43_u8; 32];
        let relay_primary_bond = RelayBondLedgerEntryV1 {
            relay_id: relay_a,
            bonded_amount: Quantity::from(5_000_u32),
            bond_asset_id: xor_asset_id(),
            bonded_since_unix: 1,
            exit_capable: true,
        };
        let relay_secondary_bond = RelayBondLedgerEntryV1 {
            relay_id: relay_b,
            bonded_amount: Quantity::from(7_500_u32),
            bond_asset_id: xor_asset_id(),
            bonded_since_unix: 1,
            exit_capable: true,
        };
        let relay_primary_bond_file = write_bond_file(&relay_primary_bond);
        let relay_secondary_bond_file = write_bond_file(&relay_secondary_bond);
        let write_metrics_file = |relay: RelayId, epoch: u32, metrics: RelayEpochMetricsV1| {
            let relay_hex = relay_id_to_hex(relay);
            let file_path = metrics_dir.join(format!("relay-{relay_hex}-epoch-{epoch}.to"));
            fs::write(
                &file_path,
                to_bytes(&metrics).expect("encode metrics snapshot"),
            )
            .expect("write metrics snapshot");
        };
        write_metrics_file(
            relay_a,
            1,
            metrics_for(
                relay_a,
                1,
                3_600,
                3_600,
                1_000_000,
                RelayComplianceStatusV1::Clean,
            ),
        );
        write_metrics_file(
            relay_a,
            2,
            metrics_for(
                relay_a,
                2,
                3_500,
                3_600,
                950_000,
                RelayComplianceStatusV1::Warning,
            ),
        );
        write_metrics_file(
            relay_b,
            1,
            metrics_for(
                relay_b,
                1,
                3_400,
                3_600,
                1_200_000,
                RelayComplianceStatusV1::Clean,
            ),
        );
        let mut relay_entries = Vec::new();
        let mut primary_relay_entry = Map::new();
        primary_relay_entry.insert(
            "relay_id".to_string(),
            Value::String(relay_id_to_hex(relay_a)),
        );
        primary_relay_entry.insert(
            "beneficiary".to_string(),
            Value::String(sample_account_literal("relay-a")),
        );
        primary_relay_entry.insert(
            "bond_path".to_string(),
            Value::String(relay_primary_bond_file.path().display().to_string()),
        );
        relay_entries.push(Value::Object(primary_relay_entry));
        let mut secondary_relay_entry = Map::new();
        secondary_relay_entry.insert(
            "relay_id".to_string(),
            Value::String(relay_id_to_hex(relay_b)),
        );
        secondary_relay_entry.insert(
            "beneficiary".to_string(),
            Value::String(sample_account_literal("relay-b")),
        );
        secondary_relay_entry.insert(
            "bond_path".to_string(),
            Value::String(relay_secondary_bond_file.path().display().to_string()),
        );
        relay_entries.push(Value::Object(secondary_relay_entry));
        let mut root = Map::new();
        root.insert("relays".to_string(), Value::Array(relay_entries));
        let config_json = Value::Object(root);
        let config_path = tmp_dir.path().join("shadow_config.json");
        fs::write(
            &config_path,
            norito::json::to_vec_pretty(&config_json).expect("encode config"),
        )
        .expect("write config");
        let args = IncentivesServiceShadowRunArgs {
            state: state_path.clone(),
            config: config_path,
            metrics_dir: metrics_dir.clone(),
            report_out: None,
            pretty: true,
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("shadow run executes");
        assert_eq!(ctx.outputs().len(), 1, "expected JSON summary output");
        let summary: norito::json::Value =
            norito::json::from_str(&ctx.outputs()[0]).expect("parse summary json");
        assert_eq!(summary["processed_payouts"].as_u64(), Some(3));
        assert_eq!(summary["total_relays"].as_u64(), Some(2));
        let expected_budget_hex = sample_budget_id_hex();
        assert_eq_compact! { summary["expected_budget_approval"].as_str() => Some(expected_budget_hex.as_str()) };
        assert_eq!(summary["missing_budget_approval"].as_u64(), Some(0));
        assert_eq!(summary["mismatched_budget_approval"].as_u64(), Some(0));
        let relays = summary["relays"]
            .as_array()
            .expect("relay summaries present");
        assert_eq!(relays.len(), 2);
        assert_compact! { relays.iter().any(|relay| relay["warning_epochs"].as_u64() == Some(1)) };
    }
    test_items! {
    fn incentives_service_shadow_run_rejects_state_without_budget_id() {
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");
        write_state_without_budget(&state_path);
        let metrics_dir = tmp_dir.path().join("metrics");
        fs::create_dir_all(&metrics_dir).expect("create metrics dir");
        let config_path = tmp_dir.path().join("shadow_config.json");
        fs::write(&config_path, r#"{"relays": []}"#).expect("write config");
        let args = IncentivesServiceShadowRunArgs {
            state: state_path,
            config: config_path,
            metrics_dir,
            report_out: None,
            pretty: true,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run(&mut ctx)
            .expect_err("shadow run must require budget approval id");
        assert_compact! { err.to_string().contains("budget_approval_id"); "unexpected error: {err}" };
        assert!(ctx.outputs().is_empty());
    }
    fn incentives_shadow_run_summary_reports_unconvertible_payout_amount() {
        let relay_id_hex = relay_id_to_hex([0x5A; 32]);
        let summary = DaemonIterationSummary {
            processed: vec![DaemonProcessedPayoutSummary {
                relay_id_hex: relay_id_hex.clone(),
                epoch: 7,
                payout_amount: "340282366920938463463374607431768211456"
                    .parse::<Quantity>()
                    .expect("2^128 quantity"),
                budget_approval_id: Some(sample_budget_id_hex()),
                metrics: PayoutMetricsSnapshot {
                    availability_per_mille: 1_000,
                    bandwidth_per_mille: 1_000,
                    compliance_per_mille: 1_000,
                    compliance_status: "clean".to_string(),
                    score_per_mille: 900,
                    exit_bonus_applied: false,
                },
                instruction_path: None,
                transfer_path: None,
                metrics_archived_to: None,
            }],
            ..DaemonIterationSummary::default()
        };
        let shadow = build_shadow_run_summary(&summary);
        assert_eq!(shadow.processed_payouts, 1);
        assert_eq!(shadow.total_payout_nanos, 0);
        assert_eq!(shadow.payout_amount_conversion_errors.len(), 1);
        let error = &shadow.payout_amount_conversion_errors[0];
        assert_eq!(error.relay_id_hex, relay_id_hex);
        assert_eq!(error.epoch, 7);
        assert_eq!(error.amount, "340282366920938463463374607431768211456");
        assert_eq!(error.reason, "too_wide_mantissa");
        assert_eq!(shadow.relays.len(), 1);
        assert_eq!(shadow.relays[0].amount_conversion_errors, 1);
        assert_eq!(shadow.relays[0].payout_nanos, 0);
    }
    fn incentives_state_roundtrip_serializes() {
        let policy = RelayBondPolicyV1 {
            minimum_exit_bond: Quantity::from(1_000_u32),
            bond_asset_id: xor_asset_id(),
            uptime_floor_per_mille: 900,
            slash_penalty_basis_points: 250,
            activation_grace_epochs: 0,
        };
        let reward_config = RewardConfig {
            policy: policy.clone(),
            base_reward: Quantity::from(75_u32),
            uptime_weight_per_mille: 600,
            bandwidth_weight_per_mille: 400,
            compliance_penalty_basis_points: 0,
            bandwidth_target_bytes: 10_000,
            budget_approval_id: Some(sample_budget_id()),
            metrics_log_path: None,
        };
        let treasury_account = sample_account_id("treasury");
        let mut state = IncentivesState::new(&reward_config, treasury_account.clone());
        state.payouts.push(sample_reward_instruction());
        let bytes = to_bytes(&state).expect("encode incentives state");
        let decoded: IncentivesState = decode_from_bytes(&bytes).expect("decode incentives state");
        decoded.ensure_current().expect("state version matches");
        assert_eq!(decoded.treasury_account, treasury_account);
        assert_eq!(decoded.payouts.len(), state.payouts.len());
        assert_eq_compact! { decoded.reward_config.base_reward => state.reward_config.base_reward };
    }
    fn incentives_service_init_rejects_missing_budget_id() {
        let config_file = write_reward_config_with_budget(None);
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");
        let args = IncentivesServiceInitArgs {
            state: state_path.clone(),
            config: config_file.path().to_path_buf(),
            treasury_account: sample_account_literal("treasury"),
            force: false,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run(&mut ctx)
            .expect_err("init must require budget approval id");
        assert_compact! { err.to_string().contains("budget_approval_id"); "unexpected error: {err}" };
        assert_compact! { !state_path.exists(); "init must not write state without budget approval" };
    }
    fn incentives_service_process_rejects_state_without_budget_id() {
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");
        write_state_without_budget(&state_path);
        let metrics_file = write_metrics_file(&sample_metrics());
        let bond_file = write_bond_file(&sample_bond_entry(2_000));
        let args = IncentivesServiceProcessArgs {
            state: state_path,
            metrics: vec![metrics_file.path().to_path_buf()],
            bond: vec![bond_file.path().to_path_buf()],
            beneficiary: vec![sample_account_literal("beneficiary")],
            instruction_out: None,
            transfer_out: None,
            submit_transfer: false,
            pretty: false,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run(&mut ctx)
            .expect_err("budget id should be required");
        assert_compact! { err.to_string().contains("budget_approval_id"); "unexpected error: {err}" };
    }
    fn incentives_service_audit_flags_underbonded_relay() {
        let config_file = write_sample_reward_config_file();
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");
        let _init_ctx = initialize_incentives_state(config_file.path(), &state_path);
        let underbonded = sample_bond_entry(500);
        let bond_file = write_bond_file(&underbonded);
        let mut relay_entry = Map::new();
        relay_entry.insert(
            "relay_id".to_string(),
            Value::String(relay_id_to_hex(underbonded.relay_id)),
        );
        relay_entry.insert(
            "beneficiary".to_string(),
            Value::String(sample_account_literal("relay-audited")),
        );
        relay_entry.insert(
            "bond_path".to_string(),
            Value::String(bond_file.path().display().to_string()),
        );
        let mut root = Map::new();
        root.insert(
            "relays".to_string(),
            Value::Array(vec![Value::Object(relay_entry)]),
        );
        let daemon_config = tmp_dir.path().join("daemon_config.json");
        fs::write(
            &daemon_config,
            norito::json::to_vec_pretty(&root).expect("encode daemon config"),
        )
        .expect("write daemon config");
        let args = IncentivesServiceAuditArgs {
            state: state_path,
            config: daemon_config,
            scopes: vec![IncentiveAuditScope::Bond],
            pretty: true,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run(&mut ctx)
            .expect_err("audit should fail when bond minimum is not met");
        assert!(err.to_string().contains("issue"), "unexpected error: {err}");
        assert_eq!(ctx.outputs().len(), 1, "expected JSON summary output");
        let summary: Value = norito::json::from_str(&ctx.outputs()[0]).expect("parse summary");
        assert_eq_compact! { summary["bond"]["insufficient_bond"].as_u64() => Some(1); "underbonded relay should be reported" };
    }
    fn incentives_service_audit_flags_budget_mismatch_and_missing() {
        let config_file = write_sample_reward_config_file();
        let reward_config = read_reward_config(config_file.path()).expect("reward config");
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");
        let mut state =
            IncentivesState::new(&reward_config, sample_account_id("treasury-budget-audit"));
        let mut mismatched = sample_reward_instruction();
        mismatched.relay_id = [0xEE; 32];
        mismatched.budget_approval_id = Some([0xFF; 32]);
        let mut missing = sample_reward_instruction();
        missing.relay_id = [0xDD; 32];
        missing.budget_approval_id = None;
        state.payouts = vec![mismatched, missing];
        save_incentives_state(&state_path, &state).expect("write incentives state");
        let daemon_config = tmp_dir.path().join("daemon_config.json");
        fs::write(&daemon_config, r#"{"relays": []}"#).expect("write daemon config");
        let args = IncentivesServiceAuditArgs {
            state: state_path,
            config: daemon_config,
            scopes: vec![IncentiveAuditScope::Budget],
            pretty: true,
        };
        let mut ctx = TestContext::new();
        let err = args.run(&mut ctx).expect_err("budget audit should fail");
        assert!(err.to_string().contains("issue"), "unexpected error: {err}");
        assert_eq!(ctx.outputs().len(), 1, "expected JSON summary output");
        let summary: Value = norito::json::from_str(&ctx.outputs()[0]).expect("parse summary");
        let budget = summary["budget"]
            .as_object()
            .expect("budget summary present");
        let expected_budget = sample_budget_id_hex();
        assert_eq_compact! { budget.get("configured_budget_approval_id").and_then(Value::as_str) => Some(expected_budget.as_str()) };
        assert_eq_compact! { budget.get("mismatched_budget_approval").and_then(Value::as_u64) => Some(1) };
        assert_eq_compact! { budget.get("payouts_without_budget").and_then(Value::as_u64) => Some(1) };
    }
    fn incentives_service_init_writes_state() {
        let config_file = write_sample_reward_config_file();
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");
        let _ctx = initialize_incentives_state(config_file.path(), &state_path);
        assert!(state_path.exists());
        let state = read_state(&state_path);
        assert_eq!(state.version, IncentivesState::VERSION);
        assert_eq!(state.treasury_account, sample_account_id("treasury"));
        assert!(state.payouts.is_empty());
        assert!(state.disputes.is_empty());
        assert_eq_compact! { state.reward_config.policy.bond_asset_id => xor_asset_id().to_string() };
    }
    fn incentives_service_process_records_reward() {
        let config_file = write_sample_reward_config_file();
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");
        let _init_ctx = initialize_incentives_state(config_file.path(), &state_path);
        let metrics = sample_metrics();
        let metrics_file = write_metrics_file(&metrics);
        let bond_file = write_bond_file(&sample_bond_entry(2_000));
        let instruction_out = NamedTempFile::new().expect("instruction file");
        let args = IncentivesServiceProcessArgs {
            state: state_path.clone(),
            metrics: vec![metrics_file.path().to_path_buf()],
            bond: vec![bond_file.path().to_path_buf()],
            beneficiary: vec![sample_account_literal("beneficiary")],
            instruction_out: Some(instruction_out.path().to_path_buf()),
            transfer_out: None,
            submit_transfer: false,
            pretty: true,
        };
        let mut process_ctx = TestContext::new();
        args.run(&mut process_ctx).expect("process command runs");
        assert_eq!(process_ctx.outputs().len(), 1);
        let summary: norito::json::Value =
            norito::json::from_str(&process_ctx.outputs()[0]).expect("parse summary");
        assert_eq!(summary["epoch"].as_u64(), Some(u64::from(metrics.epoch)));
        assert_eq!(summary["ledger"]["total_paid"].as_str(), Some("100"));
        let state = read_state(&state_path);
        assert_eq!(state.payouts.len(), 1);
        assert_eq!(state.payouts[0].epoch, metrics.epoch);
        assert_eq_compact! { state.payouts[0].beneficiary => sample_account_id("beneficiary") };
        let instruction_bytes = fs::read(instruction_out.path()).expect("read instruction");
        let instruction: RelayRewardInstructionV1 =
            decode_from_bytes(&instruction_bytes).expect("decode instruction");
        assert_eq!(instruction.epoch, metrics.epoch);
    }
    fn incentives_daemon_rejects_state_without_budget_id() {
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");
        write_state_without_budget(&state_path);
        let metrics_dir = tmp_dir.path().join("metrics");
        fs::create_dir_all(&metrics_dir).expect("create metrics dir");
        let bond_entry = sample_bond_entry(2_000);
        let relay_hex = relay_id_to_hex(bond_entry.relay_id);
        let bond_file = write_bond_file(&bond_entry);
        let mut relay_entry = Map::new();
        relay_entry.insert("relay_id".to_string(), Value::String(relay_hex));
        relay_entry.insert(
            "beneficiary".to_string(),
            Value::String(sample_account_literal("relay-a")),
        );
        relay_entry.insert(
            "bond_path".to_string(),
            Value::String(bond_file.path().display().to_string()),
        );
        let mut root = Map::new();
        root.insert(
            "relays".to_string(),
            Value::Array(vec![Value::Object(relay_entry)]),
        );
        let config_path = tmp_dir.path().join("daemon_config.json");
        fs::write(
            &config_path,
            norito::json::to_vec_pretty(&root).expect("encode config"),
        )
        .expect("write config");
        let daemon_args = IncentivesServiceDaemonArgs {
            state: state_path,
            config: config_path,
            metrics_dir,
            instruction_out_dir: None,
            transfer_out_dir: None,
            archive_dir: None,
            poll_interval: 1,
            once: true,
            pretty: true,
        };
        let mut ctx = TestContext::new();
        let result = daemon_args.run(&mut ctx);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert_compact! { err.contains("budget_approval_id"); "unexpected error: {err}" };
    }
    fn incentives_daemon_reports_budget_hash() {
        let config_file = write_sample_reward_config_file();
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");
        let _init_ctx = initialize_incentives_state(config_file.path(), &state_path);
        let metrics_dir = tmp_dir.path().join("metrics");
        fs::create_dir_all(&metrics_dir).expect("create metrics dir");
        let metrics = sample_metrics();
        let relay_hex = relay_id_to_hex(metrics.relay_id);
        let metrics_path =
            metrics_dir.join(format!("relay-{relay_hex}-epoch-{}.to", metrics.epoch));
        fs::write(
            &metrics_path,
            to_bytes(&metrics).expect("encode metrics snapshot"),
        )
        .expect("write metrics");
        let bond_entry = sample_bond_entry(2_000);
        let bond_file = write_bond_file(&bond_entry);
        let mut relay_entry = Map::new();
        relay_entry.insert("relay_id".to_string(), Value::String(relay_hex));
        relay_entry.insert(
            "beneficiary".to_string(),
            Value::String(sample_account_literal("relay-a")),
        );
        relay_entry.insert(
            "bond_path".to_string(),
            Value::String(bond_file.path().display().to_string()),
        );
        let mut root = Map::new();
        root.insert(
            "relays".to_string(),
            Value::Array(vec![Value::Object(relay_entry)]),
        );
        let config_path = tmp_dir.path().join("daemon_config.json");
        fs::write(
            &config_path,
            norito::json::to_vec_pretty(&root).expect("encode config"),
        )
        .expect("write config");
        let daemon_args = IncentivesServiceDaemonArgs {
            state: state_path,
            config: config_path,
            metrics_dir,
            instruction_out_dir: None,
            transfer_out_dir: None,
            archive_dir: None,
            poll_interval: 1,
            once: true,
            pretty: true,
        };
        let mut ctx = TestContext::new();
        daemon_args.run(&mut ctx).expect("daemon run succeeds");
        assert_eq!(ctx.outputs().len(), 1);
        let summary: norito::json::Value =
            norito::json::from_str(&ctx.outputs()[0]).expect("parse daemon summary");
        assert_eq!(summary["processed"].as_array().map(Vec::len), Some(1));
        assert_eq!(summary["missing_budget_approval"].as_u64(), Some(0));
        assert_eq!(summary["mismatched_budget_approval"].as_u64(), Some(0));
        let expected_budget_hex = sample_budget_id_hex();
        assert_eq_compact! { summary["expected_budget_approval"].as_str() => Some(expected_budget_hex.as_str()) };
    }
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn incentives_service_dispute_flow_updates_state() {
        let config_file = write_sample_reward_config_file();
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");
        let _init_ctx = initialize_incentives_state(config_file.path(), &state_path);
        let metrics_file = write_metrics_file(&sample_metrics());
        let bond_file = write_bond_file(&sample_bond_entry(2_000));
        let process_args = IncentivesServiceProcessArgs {
            state: state_path.clone(),
            metrics: vec![metrics_file.path().to_path_buf()],
            bond: vec![bond_file.path().to_path_buf()],
            beneficiary: vec![sample_account_literal("beneficiary")],
            instruction_out: None,
            transfer_out: None,
            submit_transfer: false,
            pretty: false,
        };
        let mut process_ctx = TestContext::new();
        process_args
            .run(&mut process_ctx)
            .expect("process command runs");
        let state = read_state(&state_path);
        let instruction = state.payouts[0].clone();
        let file_args = IncentivesServiceDisputeFileArgs {
            state: state_path.clone(),
            relay_id: hex::encode(instruction.relay_id),
            epoch: instruction.epoch,
            submitted_by: sample_account_literal("operator"),
            requested_amount: "120".into(),
            reason: "missing bandwidth".into(),
            filed_at: Some(9_999),
            adjust_credit: Some("25".into()),
            adjust_debit: None,
            norito_out: None,
            pretty: true,
        };
        let mut dispute_ctx = TestContext::new();
        file_args.run(&mut dispute_ctx).expect("file dispute runs");
        assert_eq!(dispute_ctx.outputs().len(), 1);
        let state = read_state(&state_path);
        assert_eq!(state.disputes.len(), 1);
        let stored = &state.disputes[0];
        assert_eq_compact! { stored.requested_amount => Quantity::from_str("120").expect("quantity literal") };
        assert_eq_compact! { stored.requested_adjustment.as_ref().expect("adjustment present").amount => Quantity::from_str("25").expect("quantity literal") };
        let transfer_file = NamedTempFile::new().expect("transfer file");
        let resolve_args = IncentivesServiceDisputeResolveArgs {
            state: state_path.clone(),
            dispute_id: stored.id,
            resolution: IncentivesDisputeResolutionKind::Credit,
            amount: Some("25".into()),
            notes: "approved".into(),
            resolved_at: Some(10_500),
            transfer_out: Some(transfer_file.path().to_path_buf()),
            pretty: true,
        };
        let mut resolve_ctx = TestContext::new();
        resolve_args
            .run(&mut resolve_ctx)
            .expect("resolve dispute runs");
        assert_eq!(resolve_ctx.outputs().len(), 1);
        let state = read_state(&state_path);
        assert_eq!(state.disputes.len(), 1);
        match &state.disputes[0].status {
            StoredDisputeStatus::Resolved { kind, amount, .. } => {
                assert!(matches!(kind, StoredResolutionKind::Credit));
                assert_eq_compact! { amount.clone() => Some(Quantity::from_str("25").expect("quantity literal")) };
            }
            other => panic!("unexpected dispute status: {other:?}"),
        }
        let transfer_bytes = fs::read(transfer_file.path()).expect("read transfer");
        let transfer: InstructionBox = decode_from_bytes(&transfer_bytes).expect("decode transfer");
        let transfer_box = transfer
            .as_any()
            .downcast_ref::<TransferBox>()
            .expect("transfer instruction");
        let TransferBox::Asset(transfer) = transfer_box else {
            panic!("expected asset transfer, found {transfer_box:?}");
        };
        assert_eq!(transfer.object, Quantity::from(25_u32));
        assert_eq!(transfer.destination, sample_account_id("beneficiary"));
        assert_eq!(transfer.source.account, sample_account_id("treasury"));
    }
}
