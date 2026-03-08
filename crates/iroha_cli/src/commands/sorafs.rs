//! `SoraFS` helper commands for interacting with Torii REST endpoints.
#![allow(clippy::size_of_ref)]

use super::{
    da::{normalize_ticket_hex, persist_manifest_bundle},
    da_common::DaManifestFetcher,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    convert::TryFrom,
    fmt::{self, Write as _},
    fs,
    io::{self, Write},
    num::{NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    str::FromStr,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::cli_output::print_with_optional_text;
use crate::{CliOutputFormat, Run, RunContext};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use eyre::{Result, WrapErr, eyre};
use hex::{decode, decode_to_slice, encode};
use iroha::{
    client::{
        Client, SorafsAliasListFilter, SorafsGatewayFetchOptions, SorafsGatewayScoreboardOptions,
        SorafsPinAlias, SorafsPinListFilter, SorafsPinRegisterArgs, SorafsRepairStatusFilter,
        SorafsRepairWorkerClaimRequest, SorafsRepairWorkerCompleteRequest,
        SorafsRepairWorkerFailRequest, SorafsReplicationListFilter, SorafsTokenOverrides,
    },
    config::Config,
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
    HybridPublicKey, HybridSuite, SignatureOf,
    soranet::{
        blinding::canonical_cache_key,
        certificate::CertificateValidationPhase,
        directory::GuardDirectorySnapshotV2,
        token::{AdmissionToken, MintError as AdmissionTokenMintError, compute_issuer_fingerprint},
    },
};
use norito::{
    codec::Encode,
    json::{Map, Number, Value},
};
use rand::{CryptoRng, RngCore, SeedableRng, rngs::StdRng};
use reqwest::blocking::Client as BlockingHttpClient;
use sorafs_car::{
    CarBuildPlan, CarChunk, CarWriteStats, CarWriter, ChunkStore, FilePlan, PorMerkleTree,
    fetch_plan::{chunk_fetch_specs_from_json, chunk_fetch_specs_to_json, parse_digest_hex},
};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::chunker_registry;
use sorafs_manifest::deal::{MICRO_XOR_PER_XOR, XorAmount};
use sorafs_manifest::repair::{
    REPAIR_ESCALATION_APPROVAL_VERSION_V1, REPAIR_SLASH_PROPOSAL_VERSION_V1,
    REPAIR_WORKER_SIGNATURE_VERSION_V1, RepairEscalationApprovalV1, RepairSlashProposalV1,
    RepairTicketId, RepairWorkerActionV1, RepairWorkerSignaturePayloadV1,
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
        GatewayFetchConfig, GatewayProviderInput, GuardCacheKey, GuardRetention, GuardSelector,
        GuardSet, PayoutServiceError, RelayDirectory, RewardLedgerError,
    },
    treasury::{
        AdjustmentKind, AdjustmentRequest, DisputeId, DisputeResolution, DisputeStatus,
        EarningsDashboard, EarningsRow, LedgerReconciliationReport, LedgerTransferMismatch,
        LedgerTransferRecord, MismatchReason, PayoutInput, RelayPayoutService, ResolutionKind,
        RewardDispute, RewardLedgerSnapshot, TransferKind,
    },
};
use soranet_pq::MlDsaSuite;
use time::{Duration as TimeDelta, OffsetDateTime, format_description::well_known::Rfc3339};
use tiny_keccak::{Hasher as _, Sha3};
use tokio::runtime::Runtime;

use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    isi::{InstructionBox, Transfer},
    metadata::Metadata,
    name::Name,
    prelude::ChainId,
    sorafs::{
        gar::{GarEnforcementActionV1, GarEnforcementReceiptV1},
        pin_registry::StorageClass,
        reserve::{
            ReserveDuration, ReserveLedgerProjection, ReservePolicyV1, ReserveQuote, ReserveTier,
        },
    },
    soranet::{
        RelayId,
        incentives::{
            RelayBondLedgerEntryV1, RelayBondPolicyV1, RelayComplianceStatusV1,
            RelayEpochMetricsV1, RelayRewardInstructionV1,
        },
    },
};
use iroha_primitives::numeric::Numeric;
use norito::{NoritoSerialize, decode_from_bytes};

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
        assert_eq!(
            capture.summary.as_ref(),
            Some(&expected_dir.join("summary.json"))
        );
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
        assert_eq!(
            summary.get("provider_count").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            summary
                .get("gateway_provider_count")
                .and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            summary.get("provider_mix").and_then(Value::as_str),
            Some("gateway-only")
        );
    }

    #[test]
    fn provider_counts_report_mixed_classifications() {
        let mut summary = norito::json::Map::new();
        insert_provider_counts(&mut summary, ProviderCounts::new(2, 2));
        assert_eq!(
            summary.get("provider_mix").and_then(Value::as_str),
            Some("mixed")
        );
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
        assert_eq!(
            summary.get("transport_policy").and_then(Value::as_str),
            Some("direct-only")
        );
        assert_eq!(
            summary
                .get("transport_policy_override")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            summary
                .get("transport_policy_override_label")
                .and_then(Value::as_str),
            Some("direct-only")
        );
    }

    #[test]
    fn summary_defaults_transport_policy_without_override() {
        let mut summary = norito::json::Map::new();
        insert_transport_policy(&mut summary, None, None);
        assert_eq!(
            summary.get("transport_policy").and_then(Value::as_str),
            Some("soranet-first")
        );
        assert_eq!(
            summary
                .get("transport_policy_override")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(
            summary
                .get("transport_policy_override_label")
                .is_none_or(Value::is_null)
        );
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
        assert_eq!(
            summary.get("telemetry_source").and_then(Value::as_str),
            Some("otel::prod")
        );
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
        assert_eq!(
            summary.get("telemetry_region").and_then(Value::as_str),
            Some("iad-prod")
        );
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
    /// Storage helpers (pin, etc.).
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
    /// GAR policy evidence helpers.
    #[command(subcommand)]
    Gar(GarCommand),
    /// Repair queue helpers (list, claim, close, escalate).
    #[command(subcommand)]
    Repair(RepairCommand),
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
}

impl Run for ReserveCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Quote(args) => args.run(context),
            Self::Ledger(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum GarCommand {
    /// Render a GAR enforcement receipt artefact (JSON + optional Norito bytes).
    Receipt(GarReceiptArgs),
}

impl Run for GarCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Receipt(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum RepairCommand {
    /// List repair tickets (optionally filtered by manifest/provider/status).
    List(RepairListArgs),
    /// Claim a queued repair ticket as a repair worker.
    Claim(RepairClaimArgs),
    /// Mark a repair ticket as completed.
    Complete(RepairCompleteArgs),
    /// Mark a repair ticket as failed.
    Fail(RepairFailArgs),
    /// Escalate a repair ticket into a slash proposal.
    Escalate(RepairEscalateArgs),
}

impl Run for RepairCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            RepairCommand::List(args) => args.run(context),
            RepairCommand::Claim(args) => args.run(context),
            RepairCommand::Complete(args) => args.run(context),
            RepairCommand::Fail(args) => args.run(context),
            RepairCommand::Escalate(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct RepairListArgs {
    /// Optional manifest digest to scope the listing.
    #[arg(long = "manifest-digest", value_name = "HEX")]
    manifest_digest: Option<String>,
    /// Optional status filter (queued, verifying, in_progress, completed, failed, escalated).
    #[arg(long)]
    status: Option<String>,
    /// Optional provider identifier filter (hex-encoded).
    #[arg(long = "provider-id", value_name = "HEX")]
    provider_id: Option<String>,
}

impl Run for RepairListArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        self.run_with(
            context,
            Client::get_sorafs_repair_status_all,
            Client::get_sorafs_repair_status,
        )
    }
}

impl RepairListArgs {
    fn run_with<C, F, G>(&self, context: &mut C, list_all: F, list_manifest: G) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SorafsRepairStatusFilter<'_>) -> Result<Response<Vec<u8>>>,
        G: FnOnce(&Client, &str, &SorafsRepairStatusFilter<'_>) -> Result<Response<Vec<u8>>>,
    {
        let manifest = self
            .manifest_digest
            .as_deref()
            .map(|hex| normalize_hex_digest::<32>(hex, "--manifest-digest"))
            .transpose()?;
        let provider = self
            .provider_id
            .as_deref()
            .map(|hex| normalize_hex_digest::<32>(hex, "--provider-id"))
            .transpose()?;
        let filter = SorafsRepairStatusFilter {
            status: self.status.as_deref(),
            provider_id: provider.as_deref(),
        };
        let client = context.client_from_config();
        let response = match manifest.as_deref() {
            Some(digest) => list_manifest(&client, digest, &filter)?,
            None => list_all(&client, &filter)?,
        };
        render_json_response(context, response)
    }
}

#[derive(clap::Args, Debug)]
pub struct RepairClaimArgs {
    /// Repair ticket identifier (e.g., `REP-401`).
    #[arg(long = "ticket-id", value_name = "ID")]
    ticket_id: String,
    /// Manifest digest bound to the ticket (hex-encoded).
    #[arg(long = "manifest-digest", value_name = "HEX")]
    manifest_digest: String,
    /// Provider identifier owning the ticket (hex-encoded).
    #[arg(long = "provider-id", value_name = "HEX")]
    provider_id: String,
    /// Optional timestamp for the claim (RFC3339 or `@unix_seconds`).
    #[arg(long = "claimed-at", value_name = "RFC3339|@UNIX")]
    claimed_at: Option<String>,
    /// Optional idempotency key (auto-generated when omitted).
    #[arg(long = "idempotency-key", value_name = "KEY")]
    idempotency_key: Option<String>,
}

impl Run for RepairClaimArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        self.run_with(context, Client::post_sorafs_repair_claim)
    }
}

impl RepairClaimArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SorafsRepairWorkerClaimRequest) -> Result<Response<Vec<u8>>>,
    {
        ensure_optional_non_empty(self.idempotency_key.as_deref(), "idempotency-key")?;
        let ticket_id = parse_repair_ticket_id(&self.ticket_id, "--ticket-id")?;
        let manifest_digest = parse_hex_array::<32>(&self.manifest_digest, "--manifest-digest")?;
        let provider_id = parse_hex_array::<32>(&self.provider_id, "--provider-id")?;
        let claimed_at_unix = parse_timestamp_or_now(self.claimed_at.as_deref(), "claimed-at")?;
        let idempotency_key = self
            .idempotency_key
            .clone()
            .unwrap_or_else(|| generate_nonce_hex(12));
        let action = RepairWorkerActionV1::Claim { claimed_at_unix };
        let (worker_id, signature) = build_repair_worker_signature(
            context.config(),
            &ticket_id,
            manifest_digest,
            provider_id,
            &idempotency_key,
            action,
        )?;
        let request = SorafsRepairWorkerClaimRequest {
            ticket_id,
            manifest_digest_hex: encode(manifest_digest),
            worker_id,
            claimed_at_unix,
            idempotency_key,
            signature,
        };
        let client = context.client_from_config();
        let response = submit(&client, &request)?;
        render_json_response(context, response)
    }
}

#[derive(clap::Args, Debug)]
pub struct RepairCompleteArgs {
    /// Repair ticket identifier (e.g., `REP-401`).
    #[arg(long = "ticket-id", value_name = "ID")]
    ticket_id: String,
    /// Manifest digest bound to the ticket (hex-encoded).
    #[arg(long = "manifest-digest", value_name = "HEX")]
    manifest_digest: String,
    /// Provider identifier owning the ticket (hex-encoded).
    #[arg(long = "provider-id", value_name = "HEX")]
    provider_id: String,
    /// Optional timestamp for the completion (RFC3339 or `@unix_seconds`).
    #[arg(long = "completed-at", value_name = "RFC3339|@UNIX")]
    completed_at: Option<String>,
    /// Optional resolution notes.
    #[arg(long = "resolution-notes", value_name = "TEXT")]
    resolution_notes: Option<String>,
    /// Optional idempotency key (auto-generated when omitted).
    #[arg(long = "idempotency-key", value_name = "KEY")]
    idempotency_key: Option<String>,
}

impl Run for RepairCompleteArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        self.run_with(context, Client::post_sorafs_repair_complete)
    }
}

impl RepairCompleteArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SorafsRepairWorkerCompleteRequest) -> Result<Response<Vec<u8>>>,
    {
        ensure_optional_non_empty(self.idempotency_key.as_deref(), "idempotency-key")?;
        ensure_optional_non_empty(self.resolution_notes.as_deref(), "resolution-notes")?;
        let ticket_id = parse_repair_ticket_id(&self.ticket_id, "--ticket-id")?;
        let manifest_digest = parse_hex_array::<32>(&self.manifest_digest, "--manifest-digest")?;
        let provider_id = parse_hex_array::<32>(&self.provider_id, "--provider-id")?;
        let completed_at_unix =
            parse_timestamp_or_now(self.completed_at.as_deref(), "completed-at")?;
        let idempotency_key = self
            .idempotency_key
            .clone()
            .unwrap_or_else(|| generate_nonce_hex(12));
        let action = RepairWorkerActionV1::Complete {
            completed_at_unix,
            resolution_notes: self.resolution_notes.clone(),
        };
        let (worker_id, signature) = build_repair_worker_signature(
            context.config(),
            &ticket_id,
            manifest_digest,
            provider_id,
            &idempotency_key,
            action,
        )?;
        let request = SorafsRepairWorkerCompleteRequest {
            ticket_id,
            manifest_digest_hex: encode(manifest_digest),
            worker_id,
            completed_at_unix,
            resolution_notes: self.resolution_notes.clone(),
            idempotency_key,
            signature,
        };
        let client = context.client_from_config();
        let response = submit(&client, &request)?;
        render_json_response(context, response)
    }
}

#[derive(clap::Args, Debug)]
pub struct RepairFailArgs {
    /// Repair ticket identifier (e.g., `REP-401`).
    #[arg(long = "ticket-id", value_name = "ID")]
    ticket_id: String,
    /// Manifest digest bound to the ticket (hex-encoded).
    #[arg(long = "manifest-digest", value_name = "HEX")]
    manifest_digest: String,
    /// Provider identifier owning the ticket (hex-encoded).
    #[arg(long = "provider-id", value_name = "HEX")]
    provider_id: String,
    /// Optional timestamp for the failure (RFC3339 or `@unix_seconds`).
    #[arg(long = "failed-at", value_name = "RFC3339|@UNIX")]
    failed_at: Option<String>,
    /// Failure reason.
    #[arg(long = "reason", value_name = "TEXT")]
    reason: String,
    /// Optional idempotency key (auto-generated when omitted).
    #[arg(long = "idempotency-key", value_name = "KEY")]
    idempotency_key: Option<String>,
}

impl Run for RepairFailArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        self.run_with(context, Client::post_sorafs_repair_fail)
    }
}

impl RepairFailArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SorafsRepairWorkerFailRequest) -> Result<Response<Vec<u8>>>,
    {
        ensure_optional_non_empty(self.idempotency_key.as_deref(), "idempotency-key")?;
        if self.reason.trim().is_empty() {
            return Err(eyre!("--reason must not be empty"));
        }
        let ticket_id = parse_repair_ticket_id(&self.ticket_id, "--ticket-id")?;
        let manifest_digest = parse_hex_array::<32>(&self.manifest_digest, "--manifest-digest")?;
        let provider_id = parse_hex_array::<32>(&self.provider_id, "--provider-id")?;
        let failed_at_unix = parse_timestamp_or_now(self.failed_at.as_deref(), "failed-at")?;
        let idempotency_key = self
            .idempotency_key
            .clone()
            .unwrap_or_else(|| generate_nonce_hex(12));
        let action = RepairWorkerActionV1::Fail {
            failed_at_unix,
            reason: self.reason.clone(),
        };
        let (worker_id, signature) = build_repair_worker_signature(
            context.config(),
            &ticket_id,
            manifest_digest,
            provider_id,
            &idempotency_key,
            action,
        )?;
        let request = SorafsRepairWorkerFailRequest {
            ticket_id,
            manifest_digest_hex: encode(manifest_digest),
            worker_id,
            failed_at_unix,
            reason: self.reason.clone(),
            idempotency_key,
            signature,
        };
        let client = context.client_from_config();
        let response = submit(&client, &request)?;
        render_json_response(context, response)
    }
}

#[derive(clap::Args, Debug)]
pub struct RepairEscalateArgs {
    /// Repair ticket identifier (e.g., `REP-401`).
    #[arg(long = "ticket-id", value_name = "ID")]
    ticket_id: String,
    /// Manifest digest bound to the ticket (hex-encoded).
    #[arg(long = "manifest-digest", value_name = "HEX")]
    manifest_digest: String,
    /// Provider identifier owning the ticket (hex-encoded).
    #[arg(long = "provider-id", value_name = "HEX")]
    provider_id: String,
    /// Proposed penalty amount in nano-XOR.
    #[arg(long = "penalty-nano", value_name = "NANO")]
    penalty_nano: u128,
    /// Escalation rationale for governance review.
    #[arg(long = "rationale", value_name = "TEXT")]
    rationale: String,
    /// Optional auditor account (defaults to the CLI account).
    #[arg(long = "auditor", value_name = "ACCOUNT_ID")]
    auditor: Option<String>,
    /// Optional timestamp for the proposal (RFC3339 or `@unix_seconds`).
    #[arg(long = "submitted-at", value_name = "RFC3339|@UNIX")]
    submitted_at: Option<String>,
    /// Optional approval votes in favor of the slash decision.
    #[arg(long = "approve-votes", value_name = "COUNT")]
    approve_votes: Option<u32>,
    /// Optional approval votes against the slash decision.
    #[arg(long = "reject-votes", value_name = "COUNT")]
    reject_votes: Option<u32>,
    /// Optional approval abstain votes.
    #[arg(long = "abstain-votes", value_name = "COUNT")]
    abstain_votes: Option<u32>,
    /// Optional timestamp when approval was recorded (RFC3339 or `@unix_seconds`).
    #[arg(long = "approved-at", value_name = "RFC3339|@UNIX")]
    approved_at: Option<String>,
    /// Optional timestamp when the decision became final after appeals (RFC3339 or `@unix_seconds`).
    #[arg(long = "finalized-at", value_name = "RFC3339|@UNIX")]
    finalized_at: Option<String>,
}

impl Run for RepairEscalateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        self.run_with(context, Client::post_sorafs_repair_slash)
    }
}

impl RepairEscalateArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &RepairSlashProposalV1) -> Result<Response<Vec<u8>>>,
    {
        if self.rationale.trim().is_empty() {
            return Err(eyre!("--rationale must not be empty"));
        }
        let ticket_id = parse_repair_ticket_id(&self.ticket_id, "--ticket-id")?;
        let manifest_digest = parse_hex_array::<32>(&self.manifest_digest, "--manifest-digest")?;
        let provider_id = parse_hex_array::<32>(&self.provider_id, "--provider-id")?;
        let auditor_account = match self.auditor.as_deref() {
            Some(raw) => parse_account_id_str(context, raw, "--auditor")?.to_string(),
            None => context.config().account.to_string(),
        };
        let submitted_at_unix =
            parse_timestamp_or_now(self.submitted_at.as_deref(), "submitted-at")?;
        let wants_approval = self.approve_votes.is_some()
            || self.reject_votes.is_some()
            || self.abstain_votes.is_some()
            || self.approved_at.is_some()
            || self.finalized_at.is_some();
        let approval = if wants_approval {
            let approve_votes = self.approve_votes.ok_or_else(|| {
                eyre!("--approve-votes is required when supplying an approval summary")
            })?;
            let approved_at = self.approved_at.as_deref().ok_or_else(|| {
                eyre!("--approved-at is required when supplying an approval summary")
            })?;
            let finalized_at = self.finalized_at.as_deref().ok_or_else(|| {
                eyre!("--finalized-at is required when supplying an approval summary")
            })?;
            let approved_at_unix = parse_timestamp_value(approved_at, "approved-at")?;
            let finalized_at_unix = parse_timestamp_value(finalized_at, "finalized-at")?;
            Some(RepairEscalationApprovalV1 {
                version: REPAIR_ESCALATION_APPROVAL_VERSION_V1,
                approve_votes,
                reject_votes: self.reject_votes.unwrap_or(0),
                abstain_votes: self.abstain_votes.unwrap_or(0),
                approved_at_unix,
                finalized_at_unix,
            })
        } else {
            None
        };
        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id,
            provider_id,
            manifest_digest,
            auditor_account,
            proposed_penalty_nano: self.penalty_nano,
            submitted_at_unix,
            rationale: self.rationale.clone(),
            approval,
        };
        proposal
            .validate()
            .map_err(|err| eyre!("invalid repair slash proposal payload: {err}"))?;
        let client = context.client_from_config();
        let response = submit(&client, &proposal)?;
        render_json_response(context, response)
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum GcCommand {
    /// Inspect retained manifests and retention deadlines.
    Inspect(GcInspectArgs),
    /// Report which manifests would be evicted by GC (dry-run only).
    DryRun(GcDryRunArgs),
}

impl Run for GcCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            GcCommand::Inspect(args) => args.run(context),
            GcCommand::DryRun(args) => args.run(context),
        }
    }
}

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
    /// Reserve balance applied while computing the effective rent (XOR, up to 6 fractional digits).
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
        let reserve_balance = parse_xor_amount_decimal(&self.reserve_balance)?;
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
                reserve_balance,
            )
            .wrap_err("failed to compute reserve quote")?;
        let value = build_reserve_quote_value(
            &policy,
            storage_class,
            tier,
            duration,
            self.capacity_gib,
            reserve_balance,
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
    /// Asset definition identifier used for XOR transfers (e.g., `xor#sora`).
    #[arg(long = "asset-definition", value_name = "NAME#DOMAIN")]
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
        let asset_definition: AssetDefinitionId = self
            .asset_definition
            .parse()
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
    if let Some(hex) = receipt_id_hex {
        return parse_hex_array::<16>(hex, "--receipt-id");
    }
    let mut bytes = [0u8; 16];
    let mut rng = rand::rng();
    rng.fill_bytes(&mut bytes);
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
    /// Path to the chunk fetch plan JSON (for example, `chunk_fetch_specs` from `iroha sorafs toolkit pack --json-out`).
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
    /// Override the staged anonymity policy (default `stage-a` / `anon-guard-pq`; accepts `anon-*` or `stage-*` labels).
    #[arg(long = "anonymity-policy", value_name = "POLICY")]
    pub anonymity_policy: Option<String>,
    /// Hint that tightens PQ expectations for write paths (`read-only` or `upload-pq-only`).
    #[arg(long = "write-mode", value_name = "MODE")]
    pub write_mode: Option<String>,
    /// Force the orchestrator to stay on a specific transport stage (`soranet-first`, `soranet-strict`, or `direct-only`).
    #[arg(long = "transport-policy-override", value_name = "POLICY")]
    pub transport_policy_override: Option<String>,
    /// Force the orchestrator to stay on a specific anonymity stage (`stage-a`, `anon-guard-pq`, etc.).
    #[arg(long = "anonymity-policy-override", value_name = "POLICY")]
    pub anonymity_policy_override: Option<String>,
    /// Path to the persisted guard cache (Norito-encoded guard set).
    #[arg(long = "guard-cache", value_name = "PATH")]
    pub guard_cache: Option<PathBuf>,
    /// Optional 32-byte hex key used to tag guard caches when persisting to disk.
    #[arg(long = "guard-cache-key", value_name = "HEX")]
    pub guard_cache_key: Option<String>,
    /// Path to a guard directory JSON payload used to refresh guard selections.
    #[arg(long = "guard-directory", value_name = "PATH")]
    pub guard_directory: Option<PathBuf>,
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

        let guard_cache_key = match self.guard_cache_key.as_deref() {
            Some(hex) => Some(
                GuardCacheKey::from_hex(hex)
                    .map_err(|err| eyre!("invalid --guard-cache-key value `{hex}`: {err}"))?,
            ),
            None => None,
        };

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
        let mut chunk_specs = chunk_fetch_specs_from_json(&plan_value)
            .map_err(|err| eyre!("failed to parse chunk fetch specs: {err}"))?;
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
        let payload_digest_hex = hex::encode(manifest.car_digest);
        let payload_digest = blake3::Hash::from(manifest.car_digest);

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
                taikai_segment_hint: spec.taikai_segment_hint.clone(),
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
            let directory = load_guard_directory(directory_path).wrap_err_with(|| {
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
            let now_unix = OffsetDateTime::now_utc().unix_timestamp();
            let now_unix = u64::try_from(now_unix).unwrap_or(0);
            let policy = anonymity_policy.unwrap_or(AnonymityPolicy::GuardPq);
            let selected = selector.select(&directory, guard_set.as_ref(), now_unix, policy);
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
            moderation_token_key_b64: None,
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
            moderation_token_key_b64: gateway_config.moderation_token_key_b64.clone(),
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
            let manifest_json = norito::json::to_value(manifest)
                .expect("local proxy manifest should serialise to JSON");
            summary.insert("local_proxy_manifest".into(), manifest_json);
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
    base_url: String,
    stream_token_b64: String,
    privacy_events_url: Option<String>,
}

fn parse_gateway_provider_spec(value: &str) -> Result<ParsedGatewayProvider> {
    let mut name: Option<String> = None;
    let mut provider_id: Option<String> = None;
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
                    "unknown --gateway-provider key `{other}`; expected name, provider-id, base-url, stream-token, privacy-url"
                ));
            }
        }
    }

    let name = name.ok_or_else(|| eyre!("--gateway-provider requires name=<alias>"))?;
    let provider_id_hex =
        provider_id.ok_or_else(|| eyre!("--gateway-provider requires provider-id=<hex>"))?;
    let base_url =
        base_url.ok_or_else(|| eyre!("--gateway-provider requires base-url=<https://...>"))?;
    let stream_token_b64 =
        stream_token.ok_or_else(|| eyre!("--gateway-provider requires stream-token=<base64>"))?;

    Ok(ParsedGatewayProvider {
        name,
        provider_id_hex,
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

fn load_guard_set(path: &Path, key: Option<&GuardCacheKey>) -> Result<Option<GuardSet>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read guard cache from `{}`", path.display()))?;
    if bytes.is_empty() {
        return Ok(None);
    }
    let guard_set = key
        .map_or_else(
            || GuardSet::decode(&bytes),
            |key| GuardSet::decode_with_key(&bytes, Some(key)),
        )
        .map_err(|err| {
            eyre!(
                "failed to decode guard cache from `{}`: {err}",
                path.display()
            )
        })?;
    Ok(Some(guard_set))
}

fn persist_guard_set(path: &Path, guard_set: &GuardSet, key: Option<&GuardCacheKey>) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!(
                "failed to create guard cache directory `{}`",
                parent.display()
            )
        })?;
    }
    let payload = key
        .map_or_else(
            || guard_set.encode(),
            |key| guard_set.encode_with_key(Some(key)),
        )
        .map_err(|err| eyre!("failed to encode guard cache: {err}"))?;
    fs::write(path, payload)
        .wrap_err_with(|| format!("failed to write guard cache to `{}`", path.display()))?;
    Ok(())
}

fn load_guard_directory(path: &Path) -> Result<RelayDirectory> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read guard directory from `{}`", path.display()))?;
    RelayDirectory::from_guard_directory_bytes(&bytes).map_err(|err| {
        eyre!(
            "failed to parse guard directory from `{}`: {err} (expected SRCv2 Norito snapshot)",
            path.display(),
        )
    })
}

#[derive(Debug, Clone, norito::json::JsonSerialize)]
struct GuardDirectorySummary {
    version: u8,
    directory_hash_hex: Option<String>,
    published_at_unix: Option<i64>,
    valid_after_unix: Option<i64>,
    valid_until_unix: Option<i64>,
    validation_phase: Option<&'static str>,
    issuer_count: usize,
    relay_count: usize,
    entry_guards: usize,
    entry_guards_pq: usize,
    entry_guard_pq_ratio: f64,
    exit_relays: usize,
    dual_signed_relays: usize,
    pq_certificate_relays: usize,
    snapshot_size_bytes: usize,
}

impl GuardDirectorySummary {
    fn from_components(
        snapshot: &GuardDirectorySnapshotV2,
        directory: &RelayDirectory,
        snapshot_size_bytes: usize,
    ) -> Self {
        let mut entry_guards = 0usize;
        let mut pq_entry_guards = 0usize;
        let mut exit_relays = 0usize;
        let mut dual_signed = 0usize;
        let mut pq_cert_relays = 0usize;

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
            if let Some(metadata) = descriptor.certificate_metadata() {
                if metadata.has_dual_signatures {
                    dual_signed += 1;
                }
                if metadata.has_pq_key {
                    pq_cert_relays += 1;
                }
            } else if descriptor.is_pq_capable() {
                pq_cert_relays += 1;
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
            directory_hash_hex: directory.directory_hash().map(hex::encode),
            published_at_unix: directory.published_at(),
            valid_after_unix: directory.valid_after(),
            valid_until_unix: directory.valid_until(),
            validation_phase: directory.validation_phase().map(validation_phase_label),
            issuer_count: snapshot.issuers.len(),
            relay_count: directory.entries().len(),
            entry_guards,
            entry_guards_pq: pq_entry_guards,
            entry_guard_pq_ratio: pq_ratio,
            exit_relays,
            dual_signed_relays: dual_signed,
            pq_certificate_relays: pq_cert_relays,
            snapshot_size_bytes,
        }
    }
}

fn inspect_guard_directory_bytes(bytes: &[u8]) -> Result<GuardDirectorySummary> {
    let snapshot = GuardDirectorySnapshotV2::from_bytes(bytes)
        .wrap_err("failed to decode guard directory snapshot")?;
    let directory = RelayDirectory::from_guard_directory_bytes(bytes)
        .wrap_err("guard directory verification failed")?;
    Ok(GuardDirectorySummary::from_components(
        &snapshot,
        &directory,
        bytes.len(),
    ))
}

fn ensure_expected_directory_hash(
    summary: &GuardDirectorySummary,
    expected_hex: Option<&str>,
) -> Result<()> {
    if let Some(expected) = expected_hex {
        let expected_normalised = normalise_directory_hash_hex(expected)?;
        let actual = summary
            .directory_hash_hex
            .as_deref()
            .ok_or_else(|| eyre!("guard directory snapshot did not advertise `directory_hash`"))?;
        if actual != expected_normalised {
            return Err(eyre!(
                "guard directory hash mismatch (expected {expected_normalised}, got {actual})"
            ));
        }
    }
    Ok(())
}

fn normalise_directory_hash_hex(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.len() != 64 {
        return Err(eyre!(
            "directory hash must contain 64 hex characters (got length {})",
            trimmed.len()
        ));
    }
    if !trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(eyre!(
            "directory hash `{trimmed}` must only contain hexadecimal characters"
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn validation_phase_label(phase: CertificateValidationPhase) -> &'static str {
    match phase {
        CertificateValidationPhase::Phase1AllowSingle => "phase1_allow_single",
        CertificateValidationPhase::Phase2PreferDual => "phase2_prefer_dual",
        CertificateValidationPhase::Phase3RequireDual => "phase3_require_dual",
    }
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
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(eyre!("{flag} must not be empty"));
        }
        TransportPolicy::parse(trimmed)
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
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(eyre!("{flag} must not be empty"));
        }
        AnonymityPolicy::parse(trimmed)
            .ok_or_else(|| {
                eyre!(
                    "{flag} must be one of `anon-guard-pq`, `stage-a`, `anon-majority-pq`, \
                 `stage-b`, `anon-strict-pq`, or `stage-c`"
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
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(eyre!("{flag} must not be empty"));
        }
        WriteModeHint::parse(trimmed).ok_or_else(|| {
            eyre!("{flag} must be one of `read-only`, `read_only`, `upload-pq-only`, or `upload_pq_only`")
        }).map(Some)
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

impl Run for Command {
    #[allow(clippy::too_many_lines)]
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Pin(cmd) => cmd.run(context),
            Command::Alias(cmd) => cmd.run(context),
            Command::Replication(cmd) => cmd.run(context),
            Command::Storage(cmd) => cmd.run(context),
            Command::Gateway(cmd) => cmd.run(context),
            Command::Incentives(cmd) => cmd.run(context),
            Command::Handshake(cmd) => cmd.run(context),
            Command::Toolkit(cmd) => cmd.run(context),
            Command::GuardDirectory(cmd) => cmd.run(context),
            Command::Reserve(cmd) => cmd.run(context),
            Command::Gar(cmd) => cmd.run(context),
            Command::Repair(cmd) => cmd.run(context),
            Command::Gc(cmd) => cmd.run(context),
            Command::Fetch(args) => args.run(context),
        }
    }
}

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

impl Run for IncentivesCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            IncentivesCommand::Compute(args) => args.run(context),
            IncentivesCommand::OpenDispute(args) => args.run(context),
            IncentivesCommand::Dashboard(args) => args.run(context),
            IncentivesCommand::Service(cmd) => cmd.run(context),
        }
    }
}

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
    /// Requested adjustment amount (Numeric).
    #[arg(long = "requested-amount", value_name = "NUMERIC")]
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
        let requested_amount = parse_numeric_str(&self.requested_amount, "--requested-amount")?;
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
            accumulator.record(&instruction);
        }

        let mut rows: Vec<_> = accumulator
            .entries()
            .iter()
            .map(|(relay_id, entry)| {
                let nanos = u64::try_from(entry.payout_amount_nanos).unwrap_or(u64::MAX);
                IncentivesDashboardRow {
                    relay: hex::encode(relay_id),
                    payout_count: entry.payout_count,
                    payout_nanos: nanos,
                }
            })
            .collect();
        rows.sort_by(|a, b| a.relay.cmp(&b.relay));
        let total_nanos = rows
            .iter()
            .fold(0_u64, |acc, row| acc.saturating_add(row.payout_nanos));

        let summary = IncentivesDashboardSummary {
            total_relays: rows.len(),
            total_payout_nanos: total_nanos,
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

impl Run for IncentivesServiceCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            IncentivesServiceCommand::Init(args) => args.run(context),
            IncentivesServiceCommand::Process(args) => args.run(context),
            IncentivesServiceCommand::Record(args) => args.run(context),
            IncentivesServiceCommand::Dispute(cmd) => cmd.run(context),
            IncentivesServiceCommand::Dashboard(args) => args.run(context),
            IncentivesServiceCommand::Audit(args) => args.run(context),
            IncentivesServiceCommand::ShadowRun(args) => args.run(context),
            IncentivesServiceCommand::Reconcile(args) => args.run(context),
            IncentivesServiceCommand::Daemon(args) => args.run(context),
        }
    }
}

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
    /// Allow missing `budget_approval_id` in the reward configuration (for lab/staging replays).
    #[arg(long = "allow-missing-budget-approval", default_value_t = false)]
    pub allow_missing_budget_approval: bool,
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
        if config.budget_approval_id.is_none() && !self.allow_missing_budget_approval {
            return Err(eyre!(
                "reward_config.budget_approval_id is required for incentives"
            ));
        }

        let treasury_account =
            parse_account_id_str(context, &self.treasury_account, "--treasury-account")?;
        let state = IncentivesState::new(&config, treasury_account);
        save_incentives_state(&self.state, &state)?;
        if config.budget_approval_id.is_none() {
            context.println(format_args!(
                "warning: proceeding without budget_approval_id in reward config"
            ))?;
        }
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

impl Run for IncentivesServiceDisputeCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            IncentivesServiceDisputeCommand::File(args) => args.run(context),
            IncentivesServiceDisputeCommand::Resolve(args) => args.run(context),
            IncentivesServiceDisputeCommand::Reject(args) => args.run(context),
        }
    }
}

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
    /// Requested payout amount (Numeric).
    #[arg(long = "requested-amount", value_name = "NUMERIC")]
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
        value_name = "NUMERIC",
        conflicts_with = "adjust_debit"
    )]
    pub adjust_credit: Option<String>,
    /// Debit adjustment requested by the operator.
    #[arg(
        long = "adjust-debit",
        value_name = "NUMERIC",
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
        let requested_amount = parse_numeric_str(&self.requested_amount, "--requested-amount")?;
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
    #[arg(long = "amount", value_name = "NUMERIC")]
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
                    amount: parse_numeric_str(amount, "--amount")?,
                    notes: self.notes.clone(),
                }
            }
            IncentivesDisputeResolutionKind::Debit => {
                let amount = self
                    .amount
                    .as_ref()
                    .ok_or_else(|| eyre!("--amount is required for debit resolutions"))?;
                DisputeResolution::Debit {
                    amount: parse_numeric_str(amount, "--amount")?,
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
    /// Allow payouts without `budget_approval_id` (for local testing only).
    #[arg(long = "allow-missing-budget-approval", default_value_t = false)]
    pub allow_missing_budget_approval: bool,
}

impl Run for IncentivesServiceShadowRunArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let state = load_incentives_state(&self.state)?;
        let mut state_for_run = state.clone();
        let mut service = build_clean_payout_service_with_budget_policy(
            &state_for_run,
            self.allow_missing_budget_approval,
        )?;
        let config = load_daemon_config(&self.config, &|literal| {
            crate::resolve_account_id(context, literal)
        })?;
        let expected_budget =
            match require_budget_approval_id(state.reward_config.budget_approval_id.as_ref()) {
                Ok(id) => Some(id),
                Err(err) => {
                    if self.allow_missing_budget_approval {
                        context.println(format_args!(
                            "warning: proceeding without budget_approval_id enforcement: {err}"
                        ))?;
                        None
                    } else {
                        return Err(err);
                    }
                }
            };

        let iteration_summary = process_daemon_iteration(
            &mut state_for_run,
            &mut service,
            &config,
            &self.metrics_dir,
            None,
            None,
            None,
            expected_budget.as_ref(),
        )?;

        if expected_budget.is_some()
            && (iteration_summary.missing_budget_approval > 0
                || iteration_summary.mismatched_budget_approval > 0)
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
    /// Allow payouts without `budget_approval_id` (for local testing only).
    #[arg(long = "allow-missing-budget-approval", default_value_t = false)]
    pub allow_missing_budget_approval: bool,
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
        let (mut state, mut service) =
            load_state_service_with_budget_policy(&self.state, self.allow_missing_budget_approval)?;
        let expected_budget =
            match require_budget_approval_id(state.reward_config.budget_approval_id.as_ref()) {
                Ok(id) => Some(id),
                Err(err) => {
                    if self.allow_missing_budget_approval {
                        context.println(format_args!(
                            "warning: proceeding without budget_approval_id enforcement: {err}"
                        ))?;
                        None
                    } else {
                        return Err(err);
                    }
                }
            };

        loop {
            let summary = process_daemon_iteration(
                &mut state,
                &mut service,
                &config,
                &self.metrics_dir,
                self.instruction_out_dir.as_deref(),
                self.transfer_out_dir.as_deref(),
                self.archive_dir.as_deref(),
                expected_budget.as_ref(),
            )?;

            if !summary.processed.is_empty() {
                save_incentives_state(&self.state, &state)?;
            }

            log_daemon_summary(context, &summary, self.pretty)?;
            if expected_budget.is_some()
                && (summary.missing_budget_approval > 0 || summary.mismatched_budget_approval > 0)
            {
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
    let bundle = fetcher.fetch(&normalized_ticket, None)?;
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
        assert!(
            err.to_string().contains("`--storage-ticket` provides one"),
            "error message should mention storage ticket fallback"
        );
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
        assert!(
            err.to_string().contains("must not be empty"),
            "error should mention empty envelope"
        );
    }

    #[test]
    fn load_manifest_envelope_encodes_valid_envelope() {
        let mut rng = StdRng::seed_from_u64(7);
        let key_pair = HybridKeyPair::generate(&mut rng);
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
        assert!(
            err.to_string()
                .contains("manifest envelope is missing required KEM or ciphertext fields"),
            "error should call out missing fields"
        );
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
        assert_eq!(
            object
                .get("transport_policy")
                .and_then(Value::as_str)
                .expect("transport_policy string"),
            "direct-only"
        );
        assert_eq!(
            object
                .get("transport_policy_override")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            object
                .get("transport_policy_override_label")
                .and_then(Value::as_str),
            Some("direct-only")
        );
        assert_eq!(
            object
                .get("anonymity_policy")
                .and_then(Value::as_str)
                .expect("anonymity label"),
            "anon-strict-pq"
        );
        assert_eq!(
            object
                .get("anonymity_policy_override")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            object
                .get("anonymity_policy_override_label")
                .and_then(Value::as_str),
            Some("anon-strict-pq")
        );
        assert_eq!(
            object.get("gateway_manifest_id").and_then(Value::as_str),
            Some("deadbeef")
        );
        assert_eq!(
            object.get("gateway_manifest_cid").and_then(Value::as_str),
            Some("c0ffee")
        );
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
        assert_eq!(
            object.get("assume_now").and_then(Value::as_u64),
            Some(1_700_000_000)
        );
        assert_eq!(
            object.get("telemetry_source").and_then(Value::as_str),
            Some("otel::prod")
        );
        assert_eq!(
            object.get("telemetry_region").and_then(Value::as_str),
            Some("iad-prod")
        );
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
        assert_eq!(
            object
                .get("transport_policy")
                .and_then(Value::as_str)
                .expect("transport_policy string"),
            "soranet-first"
        );
        assert_eq!(
            object
                .get("transport_policy_override")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            object.get("provider_count").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            object.get("gateway_provider_count").and_then(Value::as_u64),
            Some(2)
        );
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
        assert_eq!(
            object.get("provider_count").and_then(Value::as_u64),
            Some(5)
        );
        assert_eq!(
            object.get("gateway_provider_count").and_then(Value::as_u64),
            Some(7)
        );
        assert_eq!(
            object.get("transport_policy").and_then(Value::as_str),
            Some("soranet-first")
        );
        assert_eq!(
            object.get("anonymity_policy").and_then(Value::as_str),
            Some("anon-majority-pq")
        );
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
        assert_eq!(
            object.get("provider_mix").and_then(Value::as_str),
            Some("gateway-only")
        );
    }
}

#[derive(Debug, norito::json::JsonSerialize)]
struct IncentivesDashboardRow {
    relay: String,
    payout_count: u64,
    payout_nanos: u64,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct IncentivesDashboardSummary {
    total_relays: usize,
    total_payout_nanos: u64,
    rows: Vec<IncentivesDashboardRow>,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct DaemonProcessedPayoutSummary {
    relay_id_hex: String,
    epoch: u32,
    payout_amount: Numeric,
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
            Numeric::from_str(&base_reward).map_err(|err| eyre!("invalid base_reward: {err}"))?;
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
        let minimum_exit_bond = Numeric::from_str(&value.minimum_exit_bond)
            .map_err(|err| eyre!("invalid minimum_exit_bond: {err}"))?;
        let bond_asset_id = AssetDefinitionId::from_str(&value.bond_asset_id)
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
    requested_amount: Numeric,
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
    amount: Numeric,
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
        amount: Option<Numeric>,
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
    amount: Option<Numeric>,
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

fn build_clean_payout_service_with_budget_policy(
    state: &IncentivesState,
    allow_missing_budget_approval: bool,
) -> Result<RelayPayoutService> {
    let config = RewardConfig::try_from(state.reward_config.clone())
        .map_err(|err| eyre!("invalid reward configuration in state: {err}"))?;
    let engine = if allow_missing_budget_approval {
        RelayRewardEngine::new_allowing_missing_budget(config, true)
    } else {
        RelayRewardEngine::new(config)
    }
    .map_err(|err| eyre!("invalid reward configuration in state: {err}"))?;
    Ok(RelayPayoutService::new(
        engine,
        RelayPayoutLedger::new(state.treasury_account.clone()),
    ))
}

#[allow(dead_code)]
fn build_clean_payout_service(state: &IncentivesState) -> Result<RelayPayoutService> {
    build_clean_payout_service_with_budget_policy(state, false)
}

#[allow(dead_code)]
fn build_payout_service_with_budget_policy(
    state: &IncentivesState,
    allow_missing_budget_approval: bool,
) -> Result<RelayPayoutService> {
    state.ensure_current()?;
    let mut service =
        build_clean_payout_service_with_budget_policy(state, allow_missing_budget_approval)?;

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

#[allow(dead_code)]
fn build_payout_service(state: &IncentivesState) -> Result<RelayPayoutService> {
    build_payout_service_with_budget_policy(state, false)
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

fn numeric_to_nanos(amount: &Numeric) -> Option<u128> {
    let scale = amount.scale();
    let mantissa = amount.try_mantissa_u128()?;
    if scale >= 9 {
        let divisor = 10u128.checked_pow(scale.saturating_sub(9))?;
        mantissa.checked_div(divisor)
    } else {
        let multiplier = 10u128.checked_pow(9 - scale)?;
        mantissa.checked_mul(multiplier)
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
    load_state_service_with_budget_policy(path, false)
}

fn load_state_service_with_budget_policy(
    path: &Path,
    allow_missing_budget_approval: bool,
) -> Result<(IncentivesState, RelayPayoutService)> {
    let state = load_incentives_state(path)?;
    let service = build_payout_service_with_budget_policy(&state, allow_missing_budget_approval)?;
    Ok((state, service))
}

fn parse_adjustment_flags(
    credit: Option<&String>,
    debit: Option<&String>,
) -> Result<Option<AdjustmentRequest>> {
    if let Some(value) = credit {
        let amount = parse_numeric_str(value, "--adjust-credit")?;
        return Ok(Some(AdjustmentRequest {
            kind: AdjustmentKind::Credit,
            amount,
        }));
    }
    if let Some(value) = debit {
        let amount = parse_numeric_str(value, "--adjust-debit")?;
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
    payout_amount: Numeric,
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
    total_paid: Numeric,
    total_rebated: Numeric,
    total_withheld: Numeric,
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
    amount_nanos: u128,
    source_asset: String,
    destination: String,
}

impl ReconciliationTransferSummary {
    fn from_record(record: &LedgerTransferRecord) -> Self {
        Self {
            relay_id: relay_id_to_hex(record.relay_id),
            epoch: record.epoch,
            kind: transfer_kind_label(record.kind).to_string(),
            dispute_id: record.dispute_id,
            amount: record.amount.to_string(),
            amount_nanos: numeric_to_nanos(&record.amount).unwrap_or(0),
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
struct ReconciliationReportSummary {
    clean: bool,
    matched_transfers: usize,
    total_expected_transfers: usize,
    expected_amount_nanos: String,
    exported_amount_nanos: String,
    missing_transfers: Vec<ReconciliationTransferSummary>,
    unexpected_transfers: Vec<ReconciliationTransferSummary>,
    mismatched_transfers: Vec<ReconciliationMismatchSummary>,
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

        Self {
            clean: report.is_clean(),
            matched_transfers: report.matched_transfers,
            total_expected_transfers: report.total_expected_transfers,
            expected_amount_nanos: report.expected_amount_nanos.to_string(),
            exported_amount_nanos: report.exported_amount_nanos.to_string(),
            missing_transfers,
            unexpected_transfers,
            mismatched_transfers,
        }
    }
}

#[derive(Debug, norito::json::JsonSerialize)]
struct ShadowRunRelaySummary {
    relay_id_hex: String,
    epochs: usize,
    payout_nanos: u128,
    average_payout_nanos: f64,
    average_score_per_mille: f64,
    average_availability_per_mille: f64,
    average_bandwidth_per_mille: f64,
    warning_epochs: usize,
    suspended_epochs: usize,
    zero_score_epochs: usize,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct ShadowRunSummary {
    processed_payouts: usize,
    total_relays: usize,
    total_payout_nanos: u128,
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
    }

    let mut accumulators: BTreeMap<&str, RelayAccumulator> = BTreeMap::new();
    let mut payout_totals: Vec<u128> = Vec::new();
    let mut sum_availability = 0_u64;
    let mut sum_bandwidth = 0_u64;
    let mut total_epochs = 0_usize;
    let mut warning_epochs_total = 0_usize;
    let mut suspended_epochs_total = 0_usize;
    let mut zero_score_epochs_total = 0_usize;
    let mut max_relay_payout = 0_u128;

    for payout in &summary.processed {
        let payout_nanos = numeric_to_nanos(&payout.payout_amount).unwrap_or(0);
        let relay_entry = accumulators.entry(&payout.relay_id_hex).or_default();
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
    total_paid: Numeric,
    total_rebated: Numeric,
    total_withheld: Numeric,
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

fn build_repair_worker_signature(
    config: &Config,
    ticket_id: &RepairTicketId,
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    idempotency_key: &str,
    action: RepairWorkerActionV1,
) -> Result<(String, SignatureOf<RepairWorkerSignaturePayloadV1>)> {
    let worker_id = config.account.to_string();
    let payload = RepairWorkerSignaturePayloadV1 {
        version: REPAIR_WORKER_SIGNATURE_VERSION_V1,
        ticket_id: ticket_id.clone(),
        manifest_digest,
        provider_id,
        worker_id: worker_id.clone(),
        idempotency_key: idempotency_key.to_string(),
        action,
    };
    payload
        .validate()
        .map_err(|err| eyre!("invalid repair worker signature payload: {err}"))?;
    let signature = SignatureOf::new(config.key_pair.private_key(), &payload);
    Ok((worker_id, signature))
}

fn parse_numeric_str(value: &str, flag: &str) -> Result<Numeric> {
    Numeric::from_str(value).map_err(|err| eyre!("{flag} must be a valid Numeric: {err}"))
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
    /// Require proof-of-work tickets for admission (`--pow-optional` disables).
    #[arg(long = "pow-required", action = clap::ArgAction::SetTrue, conflicts_with = "pow_optional")]
    pow_required: bool,
    /// Disable mandatory proof-of-work tickets.
    #[arg(long = "pow-optional", action = clap::ArgAction::SetTrue, conflicts_with = "pow_required")]
    pow_optional: bool,
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
    /// Enable the Argon2 puzzle gate for handshake admission (`--pow-puzzle-disable` clears).
    #[arg(long = "pow-puzzle-enable", action = clap::ArgAction::SetTrue, conflicts_with = "pow_puzzle_disable")]
    pow_puzzle_enable: bool,
    /// Disable the Argon2 puzzle gate.
    #[arg(long = "pow-puzzle-disable", action = clap::ArgAction::SetTrue, conflicts_with = "pow_puzzle_enable")]
    pow_puzzle_disable: bool,
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
        conflicts_with = "allow_sm_handshake_mismatch"
    )]
    require_sm_handshake_match: bool,
    /// Allow mismatched SM helper availability.
    #[arg(
        long = "allow-sm-handshake-mismatch",
        action = clap::ArgAction::SetTrue,
        conflicts_with = "require_sm_handshake_match"
    )]
    allow_sm_handshake_mismatch: bool,
    /// Require peers to match the OpenSSL preview flag.
    #[arg(
        long = "require-sm-openssl-preview-match",
        action = clap::ArgAction::SetTrue,
        conflicts_with = "allow_sm_openssl_preview_mismatch"
    )]
    require_sm_openssl_preview_match: bool,
    /// Allow mismatched OpenSSL preview flags.
    #[arg(
        long = "allow-sm-openssl-preview-mismatch",
        action = clap::ArgAction::SetTrue,
        conflicts_with = "require_sm_openssl_preview_match"
    )]
    allow_sm_openssl_preview_mismatch: bool,
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
            confidential_gas: None,
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
        if self.pow_required {
            pow_update.required = Some(true);
        }
        if self.pow_optional {
            pow_update.required = Some(false);
        }
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
        let mut pow_touched = self.pow_required
            || self.pow_optional
            || self.pow_difficulty.is_some()
            || self.pow_max_future_skew.is_some()
            || self.pow_min_ttl.is_some()
            || self.pow_ttl.is_some();
        let mut puzzle_update = SoranetHandshakePuzzleUpdate::default();
        if self.pow_puzzle_enable {
            puzzle_update.enabled = Some(true);
        }
        if self.pow_puzzle_disable {
            puzzle_update.enabled = Some(false);
        }
        if let Some(value) = self.pow_puzzle_memory {
            puzzle_update.memory_kib = Some(value);
        }
        if let Some(value) = self.pow_puzzle_time {
            puzzle_update.time_cost = Some(value);
        }
        if let Some(value) = self.pow_puzzle_lanes {
            puzzle_update.lanes = Some(value);
        }
        let puzzle_touched = self.pow_puzzle_enable
            || self.pow_puzzle_disable
            || self.pow_puzzle_memory.is_some()
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
        if self.allow_sm_handshake_mismatch {
            network_update.require_sm_handshake_match = Some(false);
        }
        if self.require_sm_openssl_preview_match {
            network_update.require_sm_openssl_preview_match = Some(true);
        }
        if self.allow_sm_openssl_preview_mismatch {
            network_update.require_sm_openssl_preview_match = Some(false);
        }
        let network_touched = self.require_sm_handshake_match
            || self.allow_sm_handshake_mismatch
            || self.require_sm_openssl_preview_match
            || self.allow_sm_openssl_preview_mismatch;
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

impl Run for HandshakeTokenCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            HandshakeTokenCommand::Issue(args) => args.run(context),
            HandshakeTokenCommand::Id(args) => args.run(context),
            HandshakeTokenCommand::Fingerprint(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct HandshakeTokenIssueArgs {
    /// ML-DSA suite used to sign the token (mldsa44, mldsa65, mldsa87).
    #[arg(long = "suite", value_enum, default_value_t = MlDsaSuiteArg::default())]
    suite: MlDsaSuiteArg,
    /// Path to the issuer ML-DSA secret key (raw bytes).
    #[arg(
        long = "issuer-secret-key",
        value_name = "PATH",
        conflicts_with = "issuer_secret_hex"
    )]
    issuer_secret_key: Option<PathBuf>,
    /// Hex-encoded issuer ML-DSA secret key.
    #[arg(
        long = "issuer-secret-hex",
        value_name = "HEX",
        conflicts_with = "issuer_secret_key"
    )]
    issuer_secret_hex: Option<String>,
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
    /// Optional path to write the encoded token.
    #[arg(long = "output", value_name = "PATH")]
    output: Option<PathBuf>,
    /// Encoding used when writing the token to --output (base64, hex, binary).
    #[arg(long = "token-encoding", value_enum, default_value_t = TokenOutputFormat::Base64)]
    token_encoding: TokenOutputFormat,
}

#[derive(Debug)]
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

impl HandshakeTokenIssueArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let mut thread_rng = rand::rng();
        let mut rng = StdRng::from_rng(&mut thread_rng);
        let now = SystemTime::now();
        let artifacts = self.issue_with_rng(context, &mut rng, now)?;
        Self::emit(
            context,
            &artifacts,
            self.output.as_deref(),
            self.token_encoding,
        )?;
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
        let secret_key = materialise_key_bytes(
            self.issuer_secret_key.as_ref(),
            self.issuer_secret_hex.as_deref(),
            "--issuer-secret-key",
            "--issuer-secret-hex",
        )?;
        let public_key = materialise_key_bytes(
            self.issuer_public_key.as_ref(),
            self.issuer_public_hex.as_deref(),
            "--issuer-public-key",
            "--issuer-public-hex",
        )?;
        let relay_id = parse_hex_array::<32>(&self.relay_id, "--relay-id")?;
        let transcript_hash = parse_hex_array::<32>(&self.transcript_hash, "--transcript-hash")?;

        let issued_dt = parse_timestamp(self.issued_at.as_deref(), "--issued-at")?
            .unwrap_or_else(|| OffsetDateTime::from(default_now));
        let issued_secs = issued_dt.unix_timestamp();

        let expires_dt =
            if let Some(explicit) = parse_timestamp(self.expires_at.as_deref(), "--expires-at")? {
                explicit
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
        output: Option<&Path>,
        format: TokenOutputFormat,
    ) -> Result<()> {
        if let Some(path) = output {
            write_token_to_file(path, format, &artifacts.token_bytes)?;
        }

        let token_base64 = URL_SAFE_NO_PAD.encode(&artifacts.token_bytes);
        let token_hex = hex::encode(&artifacts.token_bytes);
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
        obj.insert("token_base64url".into(), Value::from(token_base64));
        obj.insert("token_hex".into(), Value::from(token_hex));
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
            output.map_or(Value::Null, |path| {
                Value::from(path.to_string_lossy().into_owned())
            }),
        );

        let text = render_token_issue_text(artifacts, &obj, output, format.describe());
        print_with_optional_text(context, Some(text), &Value::Object(obj))
    }
}

fn render_token_issue_text(
    artifacts: &TokenIssueArtifacts,
    payload: &Map,
    output: Option<&Path>,
    encoding_label: &str,
) -> String {
    let token_base64 = payload
        .get("token_base64url")
        .and_then(Value::as_str)
        .unwrap_or_default();
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
    let _ = writeln!(out, "token_base64url: {token_base64}");
    let _ = writeln!(out, "token_id_hex: {token_id_hex}");
    let _ = writeln!(out, "issuer_fingerprint_hex: {fingerprint_hex}");
    let _ = writeln!(out, "relay_id_hex: {relay_id_hex}");
    let _ = writeln!(out, "transcript_hash_hex: {transcript_hash_hex}");
    let _ = writeln!(out, "issued_at: {issued_at}");
    let _ = writeln!(out, "expires_at: {expires_at}");
    let _ = writeln!(out, "ttl_secs: {ttl_secs}");
    if let Some(path) = output {
        let _ = writeln!(out, "output: {} ({encoding_label})", path.display());
    }
    out
}

#[derive(clap::Args, Debug)]
pub struct HandshakeTokenIdArgs {
    /// Path to the admission token frame (binary).
    #[arg(
        long = "token",
        value_name = "PATH",
        id = "token",
        conflicts_with_all = ["token_hex", "token_base64"]
    )]
    path: Option<PathBuf>,
    /// Hex-encoded admission token frame.
    #[arg(
        long = "token-hex",
        value_name = "HEX",
        id = "token_hex",
        conflicts_with_all = ["token", "token_base64"]
    )]
    hex_input: Option<String>,
    /// Base64url-encoded admission token frame.
    #[arg(
        long = "token-base64",
        value_name = "BASE64",
        id = "token_base64",
        conflicts_with_all = ["token", "token_hex"]
    )]
    base64_input: Option<String>,
}

impl HandshakeTokenIdArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = materialise_token_bytes(
            self.path.as_deref(),
            self.hex_input.as_deref(),
            self.base64_input.as_deref(),
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
) -> Result<Vec<u8>> {
    match (path, hex) {
        (Some(path), None) => {
            let bytes = fs::read(path)
                .wrap_err_with(|| format!("failed to read {path_flag} {}", path.display()))?;
            if bytes.is_empty() {
                return Err(eyre!("{path_flag} must not be empty"));
            }
            Ok(bytes)
        }
        (None, Some(hex)) => decode_hex_string(hex, hex_flag),
        (Some(_), Some(_)) => Err(eyre!(
            "exactly one of {path_flag} or {hex_flag} must be provided"
        )),
        (None, None) => Err(eyre!("either {path_flag} or {hex_flag} must be provided")),
    }
}

fn materialise_token_bytes(
    path: Option<&Path>,
    hex: Option<&str>,
    base64: Option<&str>,
) -> Result<Vec<u8>> {
    match (path, hex, base64) {
        (Some(path), None, None) => {
            let bytes = fs::read(path)
                .wrap_err_with(|| format!("failed to read --token {}", path.display()))?;
            if bytes.is_empty() {
                return Err(eyre!("--token must not be empty"));
            }
            Ok(bytes)
        }
        (None, Some(hex), None) => decode_hex_string(hex, "--token-hex"),
        (None, None, Some(b64)) => decode_base64_string(b64, "--token-base64"),
        _ => Err(eyre!(
            "provide exactly one of --token, --token-hex, or --token-base64"
        )),
    }
}

fn write_token_to_file(path: &Path, format: TokenOutputFormat, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create {}", parent.display()))?;
    }
    match format {
        TokenOutputFormat::Base64 => {
            let encoded = URL_SAFE_NO_PAD.encode(bytes);
            fs::write(path, format!("{encoded}\n"))
                .wrap_err_with(|| format!("failed to write {}", path.display()))?;
        }
        TokenOutputFormat::Hex => {
            let encoded = hex::encode(bytes);
            fs::write(path, format!("{encoded}\n"))
                .wrap_err_with(|| format!("failed to write {}", path.display()))?;
        }
        TokenOutputFormat::Binary => {
            fs::write(path, bytes)
                .wrap_err_with(|| format!("failed to write {}", path.display()))?;
        }
    }
    Ok(())
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

fn decode_base64_string(value: &str, flag: &str) -> Result<Vec<u8>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(eyre!("{flag} must not be empty"));
    }
    URL_SAFE_NO_PAD
        .decode(trimmed.as_bytes())
        .or_else(|_| STANDARD.decode(trimmed.as_bytes()))
        .map_err(|err| eyre!("failed to decode {flag}: {err}"))
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
    context.println(format_args!("pow.required: {}", summary.pow.required))?;
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
    if let Some(puzzle) = summary.pow.puzzle {
        context.println(format_args!("pow.puzzle.enabled: true"))?;
        context.println(format_args!("pow.puzzle.memory_kib: {}", puzzle.memory_kib))?;
        context.println(format_args!("pow.puzzle.time_cost: {}", puzzle.time_cost))?;
        context.println(format_args!("pow.puzzle.lanes: {}", puzzle.lanes))?;
    } else {
        context.println(format_args!("pow.puzzle.enabled: false"))?;
    }
    Ok(())
}

#[derive(clap::Subcommand, Debug)]
pub enum GatewayCommand {
    /// Validate a denylist file against gateway policy rules.
    LintDenylist(GatewayLintDenylistArgs),
    /// Apply additions/removals to a denylist bundle with deterministic ordering.
    UpdateDenylist(GatewayUpdateDenylistArgs),
    /// Emit a TOML snippet with gateway configuration defaults.
    TemplateConfig(GatewayTemplateConfigArgs),
    /// Derive canonical/vanity hostnames for a provider.
    GenerateHosts(GatewayGenerateHostsArgs),
    /// Render the headers + route binding plan for a manifest rollout.
    RoutePlan(GatewayRoutePlanArgs),
    /// Generate a cache invalidation payload and curl snippet for GAR/SoraFS gateways.
    CacheInvalidate(GatewayCacheInvalidateArgs),
    /// Emit an evidence summary for a denylist bundle.
    Evidence(GatewayEvidenceArgs),
    /// Direct-mode planning and configuration helpers.
    #[command(subcommand)]
    DirectMode(GatewayDirectModeCommand),
    /// Merkle snapshot/proof tooling for denylist bundles.
    #[command(subcommand)]
    Merkle(GatewayMerkleCommand),
}

impl Run for GatewayCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            GatewayCommand::LintDenylist(args) => args.run(context),
            GatewayCommand::UpdateDenylist(args) => args.run(context),
            GatewayCommand::TemplateConfig(args) => args.run(context),
            GatewayCommand::GenerateHosts(args) => args.run(context),
            GatewayCommand::RoutePlan(args) => args.run(context),
            GatewayCommand::CacheInvalidate(args) => args.run(context),
            GatewayCommand::Evidence(args) => args.run(context),
            GatewayCommand::DirectMode(cmd) => cmd.run(context),
            GatewayCommand::Merkle(cmd) => cmd.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum ToolkitCommand {
    /// Package a payload into a CAR + manifest bundle using the canonical tooling.
    Pack(ToolkitPackArgs),
}

impl Run for ToolkitCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            ToolkitCommand::Pack(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum GuardDirectoryCommand {
    /// Fetch a guard directory snapshot over HTTPS, verify it, and emit a summary.
    Fetch(GuardDirectoryFetchArgs),
    /// Verify a guard directory snapshot stored on disk.
    Verify(GuardDirectoryVerifyArgs),
}

impl Run for GuardDirectoryCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            GuardDirectoryCommand::Fetch(args) => args.run(context),
            GuardDirectoryCommand::Verify(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct GuardDirectoryFetchArgs {
    /// URLs publishing the guard directory snapshot (first success wins).
    #[arg(long = "url", value_name = "URL", required = true)]
    pub url: Vec<String>,
    /// Path where the verified snapshot will be stored (optional).
    #[arg(long = "output", value_name = "PATH")]
    pub output: Option<PathBuf>,
    /// Expected directory hash (hex). Command fails when the snapshot hash differs.
    #[arg(long = "expected-directory-hash", value_name = "HEX")]
    pub expected_directory_hash: Option<String>,
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
                    Ok(success) => match success.bytes() {
                        Ok(bytes) => {
                            snapshot = Some(bytes.to_vec());
                            break;
                        }
                        Err(err) => {
                            errors.push(format!("{url}: failed to read body: {err}"));
                        }
                    },
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

        let summary = inspect_guard_directory_bytes(&bytes)?;
        ensure_expected_directory_hash(&summary, self.expected_directory_hash.as_deref())?;

        if let Some(path) = &self.output {
            write_guard_directory_snapshot(path, &bytes, self.overwrite)?;
        }

        context.print_data(&summary)
    }
}

#[derive(clap::Args, Debug)]
pub struct GuardDirectoryVerifyArgs {
    /// Path to the guard directory snapshot to verify.
    #[arg(long = "path", value_name = "PATH")]
    pub path: PathBuf,
    /// Expected directory hash (hex). Command fails when the snapshot hash differs.
    #[arg(long = "expected-directory-hash", value_name = "HEX")]
    pub expected_directory_hash: Option<String>,
}

impl Run for GuardDirectoryVerifyArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = fs::read(&self.path).wrap_err_with(|| {
            format!(
                "failed to read guard directory snapshot from `{}`",
                self.path.display()
            )
        })?;
        let summary = inspect_guard_directory_bytes(&bytes)?;
        ensure_expected_directory_hash(&summary, self.expected_directory_hash.as_deref())?;
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

impl Run for GatewayDirectModeCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            GatewayDirectModeCommand::Plan(args) => args.run(context),
            GatewayDirectModeCommand::Enable(args) => args.run(context),
            GatewayDirectModeCommand::Rollback(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum GatewayMerkleCommand {
    /// Compute the Merkle root summary for a denylist bundle.
    Snapshot(GatewayMerkleSnapshotArgs),
    /// Emit a membership proof for a single denylist entry.
    Proof(GatewayMerkleProofArgs),
}

impl Run for GatewayMerkleCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            GatewayMerkleCommand::Snapshot(args) => args.run(context),
            GatewayMerkleCommand::Proof(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct GatewayMerkleSnapshotArgs {
    /// Path to the denylist JSON bundle.
    #[arg(
        long = "denylist",
        value_name = "PATH",
        default_value = "docs/source/sorafs_gateway_denylist_sample.json"
    )]
    pub denylist_path: PathBuf,
    /// Optional path to persist the JSON summary.
    #[arg(
        long = "json-out",
        value_name = "PATH",
        default_value = "artifacts/sorafs_gateway/denylist_merkle_snapshot.json"
    )]
    pub json_out: PathBuf,
    /// Optional path to persist the Norito-encoded snapshot artefact.
    #[arg(long = "norito-out", value_name = "PATH")]
    pub norito_out: Option<PathBuf>,
}

impl Run for GatewayMerkleSnapshotArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let entries = read_gateway_denylist(&self.denylist_path)?;
        let resolve = |literal: &str| crate::resolve_account_id(context, literal);
        let summary = build_gateway_merkle_snapshot(&entries, &resolve)?;

        write_gateway_merkle_snapshot(&summary, &self.json_out, self.norito_out.as_deref())?;

        context.println(format!(
            "denylist Merkle root: {} (leaves={})",
            summary.root_hex, summary.leaf_count
        ))?;
        for entry in &summary.entries {
            context.println(format!(
                "#{:04} {:<18} {:<40} {}",
                entry.index, entry.kind, entry.descriptor, entry.hash_hex
            ))?;
        }
        context.println(format!(
            "snapshot JSON written to {}",
            self.json_out.display()
        ))?;
        if let Some(path) = &self.norito_out {
            context.println(format!(
                "snapshot Norito payload written to {}",
                path.display()
            ))?;
        }

        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct GatewayMerkleProofArgs {
    /// Path to the denylist JSON bundle.
    #[arg(
        long = "denylist",
        value_name = "PATH",
        default_value = "docs/source/sorafs_gateway_denylist_sample.json"
    )]
    pub denylist_path: PathBuf,
    /// Zero-based index of the entry to prove (see the snapshot listing).
    #[arg(
        long = "index",
        value_name = "INDEX",
        required_unless_present = "descriptor",
        conflicts_with = "descriptor"
    )]
    pub index: Option<usize>,
    /// Descriptor of the entry to prove (`kind:value` from the snapshot output).
    #[arg(
        long = "descriptor",
        value_name = "KIND:VALUE",
        required_unless_present = "index",
        conflicts_with = "index"
    )]
    pub descriptor: Option<String>,
    /// Optional path to persist the JSON proof artefact.
    #[arg(
        long = "json-out",
        value_name = "PATH",
        default_value = "artifacts/sorafs_gateway/denylist_merkle_proof.json"
    )]
    pub json_out: PathBuf,
    /// Optional path to persist the Norito-encoded proof artefact.
    #[arg(long = "norito-out", value_name = "PATH")]
    pub norito_out: Option<PathBuf>,
}

impl Run for GatewayMerkleProofArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let entries = read_gateway_denylist(&self.denylist_path)?;
        let resolve = |literal: &str| crate::resolve_account_id(context, literal);
        let leaves = build_gateway_merkle_leaves(&entries, &resolve)?;
        let index = self.resolve_entry_index(&leaves)?;
        let hashes: Vec<[u8; 32]> = leaves.iter().map(|leaf| leaf.hash).collect();
        let levels = build_merkle_levels(&hashes)?;
        let root = levels
            .last()
            .and_then(|level| level.first())
            .ok_or_else(|| eyre!("failed to compute Merkle root"))?;
        let root_hex = format_digest_hex(root);
        let entry = make_snapshot_entry(index, &leaves[index]);
        let proof_nodes = build_merkle_proof(&levels, index)?;
        let proof = GatewayMerkleProofOutput {
            schema_version: GATEWAY_MERKLE_SCHEMA_VERSION,
            root_hex: root_hex.clone(),
            entry: entry.clone(),
            proof: proof_nodes,
        };
        if let Some(parent) = self.json_out.parent() {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!(
                    "failed to create parent directory for `{}`",
                    self.json_out.display()
                )
            })?;
        }
        let rendered =
            norito::json::to_json_pretty(&proof).wrap_err("failed to render Merkle proof JSON")?;
        fs::write(&self.json_out, rendered.as_bytes()).wrap_err_with(|| {
            format!(
                "failed to write Merkle proof to `{}`",
                self.json_out.display()
            )
        })?;

        if let Some(path) = &self.norito_out {
            write_norito_payload(path, &proof)?;
            context.println(format!(
                "proof Norito payload written to {}",
                path.display()
            ))?;
        }

        context.println(format!(
            "entry #{:04} ({}) => {}",
            entry.index, entry.kind, entry.descriptor
        ))?;
        context.println(format!("leaf hash: {}", entry.hash_hex))?;
        context.println(format!("Merkle root: {root_hex}"))?;
        if proof.proof.is_empty() {
            context.println("single-leaf tree: proof is empty")?;
        } else {
            for (level, step) in proof.proof.iter().enumerate() {
                context.println(format!(
                    "level {level}: sibling={} direction={} duplicate={}",
                    step.sibling_hex, step.direction, step.duplicate
                ))?;
            }
        }
        context.println(format!("proof JSON written to {}", self.json_out.display()))?;

        Ok(())
    }
}

impl GatewayMerkleProofArgs {
    fn resolve_entry_index(&self, leaves: &[GatewayMerkleLeaf]) -> Result<usize> {
        if let Some(index) = self.index {
            if index >= leaves.len() {
                return Err(eyre!(
                    "index {index} exceeds denylist entry count {}",
                    leaves.len()
                ));
            }
            return Ok(index);
        }
        if let Some(raw_descriptor) = self.descriptor.as_deref() {
            let descriptor = raw_descriptor.trim();
            if descriptor.is_empty() {
                return Err(eyre!("`--descriptor` must not be empty"));
            }
            if let Some((index, _entry)) = find_merkle_entry_by_descriptor(leaves, descriptor) {
                return Ok(index);
            }
            return Err(eyre!(
                "descriptor `{descriptor}` not found in denylist; run `iroha sorafs gateway merkle snapshot --denylist {}` to inspect valid descriptors",
                self.denylist_path.display()
            ));
        }
        Err(eyre!(
            "either `--index` or `--descriptor` must be supplied for the proof command"
        ))
    }
}
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
        chunk_store.ingest_plan(&payload, &plan);
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
        let mut car_payload_digest = [0u8; 32];
        car_payload_digest.copy_from_slice(car_stats.car_payload_digest.as_bytes());

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
        let mut builder = ManifestBuilder::new()
            .root_cid(root_cid.clone())
            .dag_codec(DagCodecId(car_stats.dag_codec))
            .chunking_profile(chunk_profile.clone())
            .content_length(plan.content_length)
            .car_digest(car_payload_digest)
            .car_size(car_stats.car_size)
            .pin_policy(PinPolicy {
                min_replicas: 3,
                storage_class: ManifestStorageClass::Hot,
                retention_epoch: 0,
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

        let chunk_digest_sha3 = compute_chunk_digest_sha3(&plan.chunks);
        let hybrid_output = if let Some(recipient) = hybrid_recipient {
            let aad = build_hybrid_manifest_aad(
                &manifest_digest,
                chunk_digest_sha3,
                manifest_filename.as_deref(),
            );
            let mut rng = rand::rng();
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
        });
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
fn build_pack_report(ctx: &PackReportContext<'_>) -> Value {
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

    let chunk_fetch_specs = chunk_fetch_specs_to_json(ctx.plan);

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

    Value::Object(report_obj)
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
        CapabilityType::VendorReserved => "vendor_reserved",
    }
}

fn render_direct_mode_enable_snippet(plan: &DirectModePlanOutput) -> String {
    format!(
        r#"# Direct-mode configuration snippet (generated)
[torii.sorafs_gateway]
require_manifest_envelope = false
enforce_admission = false
enforce_capabilities = false

[torii.sorafs_gateway.direct_mode]
provider_id_hex = "{provider}"
chain_id = "{chain}"
canonical_host = "{canonical}"
vanity_host = "{vanity}"
direct_car_canonical = "{direct_canonical}"
direct_car_vanity = "{direct_vanity}"
manifest_digest_hex = "{digest}"
"#,
        provider = plan.provider_id_hex,
        chain = plan.chain_id,
        canonical = plan.hosts.canonical,
        vanity = plan.hosts.vanity,
        direct_canonical = plan.direct_car.canonical_url,
        direct_vanity = plan.direct_car.vanity_url,
        digest = plan.manifest_digest_hex,
    )
}

fn render_direct_mode_rollback_snippet() -> &'static str {
    r"# Direct-mode rollback snippet
[torii.sorafs_gateway]
require_manifest_envelope = true
enforce_admission = true
enforce_capabilities = true

# Remove the `torii.sorafs_gateway.direct_mode` table to disable overrides.
"
}

#[derive(clap::Args, Debug)]
pub struct GatewayLintDenylistArgs {
    /// Path to the JSON denylist file to validate.
    #[arg(long, value_name = "PATH")]
    pub path: PathBuf,
}

#[derive(clap::Args, Debug)]
pub struct GatewayUpdateDenylistArgs {
    /// Base denylist JSON bundle to update.
    #[arg(long = "base", value_name = "PATH")]
    pub base_path: PathBuf,
    /// Additional denylist fragments to merge (JSON array of entries).
    #[arg(long = "add", value_name = "PATH")]
    pub add_paths: Vec<PathBuf>,
    /// Descriptors to remove (use output from the Merkle snapshot for accuracy).
    #[arg(long = "remove-descriptor", value_name = "KIND:VALUE")]
    pub remove_descriptors: Vec<String>,
    /// Destination path for the updated denylist (defaults to in-place).
    #[arg(long = "out", value_name = "PATH")]
    pub output_path: Option<PathBuf>,
    /// Optional Merkle snapshot JSON artefact path.
    #[arg(long = "snapshot-out", value_name = "PATH")]
    pub snapshot_out: Option<PathBuf>,
    /// Optional Merkle snapshot Norito artefact path.
    #[arg(long = "snapshot-norito-out", value_name = "PATH")]
    pub snapshot_norito_out: Option<PathBuf>,
    /// Optional evidence summary output path.
    #[arg(long = "evidence-out", value_name = "PATH")]
    pub evidence_out: Option<PathBuf>,
    /// Optional label stored in evidence output.
    #[arg(long = "label", value_name = "STRING")]
    pub label: Option<String>,
    /// Allow overwriting the destination file.
    #[arg(long = "force")]
    pub force: bool,
    /// Permit replacing existing descriptors when merging additions.
    #[arg(long = "allow-replacement")]
    pub allow_replacement: bool,
    /// Do not error if a requested removal is missing from the base.
    #[arg(long = "allow-missing-removals")]
    pub allow_missing_removals: bool,
}

#[derive(clap::Args, Debug)]
pub struct GatewayTemplateConfigArgs {
    /// Hostname to include in the ACME / gateway sample (repeatable).
    #[arg(long = "host", value_name = "HOSTNAME")]
    pub hosts: Vec<String>,
    /// Optional denylist path to include in the template.
    #[arg(
        long,
        value_name = "PATH",
        default_value = "docs/source/sorafs_gateway_denylist_sample.json"
    )]
    pub denylist_path: String,
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

impl Run for GatewayLintDenylistArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = fs::read(&self.path)
            .wrap_err_with(|| format!("failed to read denylist from `{}`", self.path.display()))?;
        let entries: Vec<GatewayDenylistRecord> =
            norito::json::from_slice(&bytes).wrap_err("failed to parse denylist JSON")?;

        if entries.is_empty() {
            context.println(format_args!(
                "denylist `{}` contains no entries",
                self.path.display()
            ))?;
            return Ok(());
        }

        let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        {
            let resolve = |literal: &str| crate::resolve_account_id(context, literal);
            for (index, entry) in entries.iter().enumerate() {
                let validation = entry
                    .validate(&resolve)
                    .wrap_err_with(|| format!("validation failed for entry #{index}"))?;
                *counts.entry(validation.kind.as_str()).or_default() += 1;
            }
        }

        context.println(format_args!(
            "validated {} denylist entries in `{}`",
            entries.len(),
            self.path.display()
        ))?;
        for (kind, count) in counts {
            context.println(format_args!("  {kind}: {count}"))?;
        }

        Ok(())
    }
}

impl Run for GatewayUpdateDenylistArgs {
    #[allow(clippy::too_many_lines)]
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if self.add_paths.is_empty() && self.remove_descriptors.is_empty() {
            return Err(eyre!(
                "at least one --add or --remove-descriptor option must be provided"
            ));
        }

        let resolve = |literal: &str| crate::resolve_account_id(context, literal);
        let mut entries = load_canonical_denylist(&self.base_path, &resolve)?;
        let mut added = 0_usize;
        let mut replaced = 0_usize;

        for path in &self.add_paths {
            let records = read_gateway_denylist(path)?;
            for (index, record) in records.into_iter().enumerate() {
                let descriptor = descriptor_for_record(&record, index, path, &resolve)
                    .wrap_err("failed to build descriptor")?;
                if let Some(existing) = entries.get_mut(&descriptor) {
                    if self.allow_replacement {
                        *existing = record;
                        replaced += 1;
                    } else {
                        return Err(eyre!(
                            "descriptor `{descriptor}` already present in `{}`; pass --allow-replacement to override",
                            path.display()
                        ));
                    }
                } else {
                    entries.insert(descriptor, record);
                    added += 1;
                }
            }
        }

        let mut removed = 0_usize;
        if !self.remove_descriptors.is_empty() {
            let mut unique = BTreeSet::new();
            for raw in &self.remove_descriptors {
                unique.insert(normalise_descriptor_input(raw)?);
            }
            for descriptor in unique {
                if entries.remove(&descriptor).is_some() {
                    removed += 1;
                } else if !self.allow_missing_removals {
                    return Err(eyre!(
                        "descriptor `{descriptor}` not found in base denylist; pass --allow-missing-removals to continue"
                    ));
                }
            }
        }

        let mut ordered: Vec<_> = entries.into_iter().collect();
        ordered.sort_by(|a, b| a.0.cmp(&b.0));
        let updated: Vec<_> = ordered.into_iter().map(|(_, record)| record).collect();

        if updated.is_empty() {
            return Err(eyre!(
                "updated denylist is empty; add at least one entry before writing"
            ));
        }

        let summary = build_gateway_merkle_snapshot(&updated, &resolve)?;
        let rendered = norito::json::to_json_pretty(&updated)
            .wrap_err("failed to render updated denylist JSON")?;
        let output_path = self.output_path.unwrap_or_else(|| self.base_path.clone());
        if output_path.exists() && !self.force {
            return Err(eyre!(
                "output `{}` already exists; pass --force to overwrite",
                output_path.display()
            ));
        }
        if let Some(parent) = output_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .wrap_err_with(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&output_path, rendered.as_bytes()).wrap_err_with(|| {
            format!(
                "failed to write updated denylist to `{}`",
                output_path.display()
            )
        })?;

        let digest_hex = blake3::hash(rendered.as_bytes()).to_hex().to_string();
        if self.snapshot_out.is_some() || self.snapshot_norito_out.is_some() {
            if let Some(path) = &self.snapshot_out {
                write_gateway_merkle_snapshot(&summary, path, self.snapshot_norito_out.as_deref())?;
            } else if let Some(path) = &self.snapshot_norito_out {
                ensure_parent_dir(path)?;
                write_norito_payload(path, &summary)?;
            }
        }

        if let Some(path) = &self.evidence_out {
            let generated_at = OffsetDateTime::now_utc();
            let summary = build_denylist_evidence(
                &updated,
                &digest_hex,
                &output_path,
                self.label.as_deref(),
                generated_at,
                &resolve,
            )?;
            ensure_parent_dir(path)?;
            let rendered = norito::json::to_json_pretty(&summary)
                .wrap_err("failed to render denylist evidence JSON")?;
            fs::write(path, rendered.as_bytes()).wrap_err_with(|| {
                format!("failed to write denylist evidence to `{}`", path.display())
            })?;
        }

        context.println(format_args!(
            "updated denylist wrote {} entries (added {added}, replaced {replaced}, removed {removed}) to {}",
            updated.len(),
            output_path.display()
        ))?;
        context.println(format_args!("denylist digest (blake3): {digest_hex}"))?;
        context.println(format_args!("Merkle root: {}", summary.root_hex))?;
        if let Some(path) = &self.snapshot_out {
            context.println(format_args!("snapshot JSON written to {}", path.display()))?;
        }
        if let Some(path) = &self.snapshot_norito_out {
            context.println(format_args!(
                "snapshot Norito payload written to {}",
                path.display()
            ))?;
        }
        if let Some(path) = &self.evidence_out {
            context.println(format_args!(
                "evidence summary written to {}",
                path.display()
            ))?;
        }

        Ok(())
    }
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
[torii.sorafs_gateway]
require_manifest_envelope = true
enforce_admission = true

[torii.sorafs_gateway.rate_limit]
max_requests = 120
window = "60s"
ban = "30s"

[torii.sorafs_gateway.denylist]
path = "{denylist}"

[torii.sorafs_gateway.acme]
enabled = true
account_email = "ops@example.com"
directory_url = "https://acme-v02.api.letsencrypt.org/directory"
hostnames = [{hosts}]
dns_provider_id = "cloudflare-prod"
renewal_window = "30d"
retry_backoff = "30m"
retry_jitter = "5m"

[torii.sorafs_gateway.acme.challenges]
dns01 = true
tls_alpn_01 = true
"#,
            denylist = self.denylist_path,
            hosts = host_list,
        );

        context.println(template)
    }
}

impl Run for GatewayGenerateHostsArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let chain_id = ChainId::from(self.chain_id.as_str());
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

#[derive(clap::Args, Debug)]
pub struct GatewayEvidenceArgs {
    /// Path to the JSON denylist file to summarise.
    #[arg(
        long = "denylist",
        value_name = "PATH",
        default_value = "docs/source/sorafs_gateway_denylist_sample.json"
    )]
    pub denylist_path: PathBuf,
    /// Output path for the evidence JSON bundle.
    #[arg(
        long = "out",
        value_name = "PATH",
        default_value = "artifacts/sorafs_gateway/denylist_evidence.json"
    )]
    pub output_path: PathBuf,
    /// Optional evidence label embedded in the output.
    #[arg(long = "label", value_name = "STRING")]
    pub label: Option<String>,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct GatewayDenylistEvidenceReport {
    generated_at: String,
    label: Option<String>,
    source: GatewayEvidenceSource,
    totals: GatewayEvidenceTotals,
    emergency_reviews: Vec<GatewayEmergencyEvidence>,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct GatewayEvidenceSource {
    path: String,
    digest_blake3_hex: String,
    entry_count: usize,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct GatewayEvidenceTotals {
    by_kind: Vec<EvidenceCount>,
    by_policy_tier: Vec<PolicyTierSummary>,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct EvidenceCount {
    label: String,
    count: usize,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct PolicyTierSummary {
    tier: String,
    count: usize,
    earliest_issued_at: Option<String>,
    latest_expires_at: Option<String>,
    latest_review_deadline: Option<String>,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct GatewayEmergencyEvidence {
    index: usize,
    kind: String,
    descriptor: Option<String>,
    issued_at: String,
    expires_at: String,
    max_allowed_expires_at: String,
    review_due_by: String,
    emergency_canon: Option<String>,
    governance_reference: Option<String>,
}

#[derive(Default)]
struct TierAccumulator {
    count: usize,
    earliest_issued_at: Option<OffsetDateTime>,
    latest_expires_at: Option<OffsetDateTime>,
    latest_review_deadline: Option<OffsetDateTime>,
}

impl TierAccumulator {
    fn observe(&mut self, window: &GatewayPolicyWindow) {
        self.count += 1;
        if let Some(issued) = window.issued_at {
            self.earliest_issued_at = match self.earliest_issued_at {
                Some(existing) if existing <= issued => Some(existing),
                _ => Some(issued),
            };
        }
        if let Some(expires) = window.expires_at.or(window.max_expires_at) {
            self.latest_expires_at = match self.latest_expires_at {
                Some(existing) if existing >= expires => Some(existing),
                _ => Some(expires),
            };
        }
        if let Some(review) = window.review_due_at {
            self.latest_review_deadline = match self.latest_review_deadline {
                Some(existing) if existing >= review => Some(existing),
                _ => Some(review),
            };
        }
    }

    fn into_summary(self, tier: &'static str) -> PolicyTierSummary {
        PolicyTierSummary {
            tier: tier.to_owned(),
            count: self.count,
            earliest_issued_at: self.earliest_issued_at.map(format_datetime),
            latest_expires_at: self.latest_expires_at.map(format_datetime),
            latest_review_deadline: self.latest_review_deadline.map(format_datetime),
        }
    }
}

impl GatewayEmergencyEvidence {
    fn from_record(
        index: usize,
        record: &GatewayDenylistRecord,
        validation: &GatewayRecordValidation,
    ) -> Result<Self> {
        let policy = validation.policy;
        let issued = policy
            .issued_at
            .ok_or_else(|| eyre!("`issued_at` is required for emergency entries"))?;
        let review_due = policy
            .review_due_at
            .ok_or_else(|| eyre!("emergency entries must include a review deadline"))?;
        let effective_expiry = policy
            .expires_at
            .or(policy.max_expires_at)
            .ok_or_else(|| eyre!("emergency entries must include `expires_at`"))?;

        Ok(Self {
            index,
            kind: validation.kind.as_str().to_owned(),
            descriptor: select_descriptor(record),
            issued_at: format_datetime(issued),
            expires_at: format_datetime(effective_expiry),
            max_allowed_expires_at: format_datetime(
                policy.max_expires_at.unwrap_or(effective_expiry),
            ),
            review_due_by: format_datetime(review_due),
            emergency_canon: record.emergency_canon.clone(),
            governance_reference: record.governance_reference.clone(),
        })
    }
}

fn select_descriptor(record: &GatewayDenylistRecord) -> Option<String> {
    [
        record.alias.as_deref(),
        record.account_alias.as_deref(),
        record.account_id.as_deref(),
        record.provider_id_hex.as_deref(),
        record.manifest_digest_hex.as_deref(),
        record.cid_hex.as_deref(),
        record.cid_utf8.as_deref(),
        record.url.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .find(|value| !value.is_empty())
    .map(str::to_owned)
}

fn read_gateway_denylist(path: &Path) -> Result<Vec<GatewayDenylistRecord>> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read denylist bundle from `{}`", path.display()))?;
    norito::json::from_slice(&bytes)
        .wrap_err_with(|| format!("failed to parse denylist JSON `{}`", path.display()))
}

fn load_canonical_denylist(
    path: &Path,
    resolve: &dyn Fn(&str) -> Result<AccountId>,
) -> Result<BTreeMap<String, GatewayDenylistRecord>> {
    let records = read_gateway_denylist(path)?;
    let mut map = BTreeMap::new();
    for (index, record) in records.into_iter().enumerate() {
        let descriptor = descriptor_for_record(&record, index, path, resolve)?;
        if map.insert(descriptor.clone(), record).is_some() {
            return Err(eyre!(
                "duplicate denylist descriptor `{descriptor}` in `{}` (entry #{index})",
                path.display()
            ));
        }
    }
    Ok(map)
}

fn descriptor_for_record(
    record: &GatewayDenylistRecord,
    index: usize,
    source: &Path,
    resolve: &dyn Fn(&str) -> Result<AccountId>,
) -> Result<String> {
    let validation = record.validate(resolve).wrap_err_with(|| {
        format!(
            "validation failed for entry #{index} in `{}`",
            source.display()
        )
    })?;
    merkle_descriptor(validation.kind, record).wrap_err_with(|| {
        format!(
            "failed to compute descriptor for entry #{index} in `{}`",
            source.display()
        )
    })
}

fn normalise_descriptor_input(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(eyre!("descriptor must not be empty"));
    }
    if !trimmed.contains(':') {
        return Err(eyre!(
            "descriptor `{trimmed}` must include a kind prefix (e.g. provider:..., manifest_digest:...)"
        ));
    }
    let mut segments = trimmed.splitn(2, ':');
    let kind = segments.next().unwrap_or_default();
    let rest = segments
        .next()
        .ok_or_else(|| eyre!("descriptor `{trimmed}` must include a payload after `:`"))?;
    let rest = rest.trim();
    let payload = match kind {
        "provider" | "manifest_digest" => rest.to_ascii_uppercase(),
        "perceptual_family" => {
            if let Some((family, variant)) = rest.split_once(':') {
                format!("{}:{}", family.trim(), variant.trim())
            } else {
                rest.to_owned()
            }
        }
        _ => rest.to_owned(),
    };
    Ok(format!("{kind}:{payload}"))
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

fn build_gateway_merkle_leaves(
    entries: &[GatewayDenylistRecord],
    resolve: &dyn Fn(&str) -> Result<AccountId>,
) -> Result<Vec<GatewayMerkleLeaf>> {
    if entries.is_empty() {
        return Err(eyre!("denylist contains no entries"));
    }

    let mut leaves = Vec::with_capacity(entries.len());
    for (index, record) in entries.iter().enumerate() {
        let validation = record
            .validate(resolve)
            .wrap_err_with(|| format!("validation failed for entry #{index}"))?;
        let leaf = gateway_leaf_from_record(record, &validation)
            .wrap_err_with(|| format!("failed to canonicalise entry #{index}"))?;
        leaves.push(leaf);
    }

    Ok(leaves)
}

#[allow(clippy::too_many_lines)]
fn gateway_leaf_from_record(
    record: &GatewayDenylistRecord,
    validation: &GatewayRecordValidation,
) -> Result<GatewayMerkleLeaf> {
    let descriptor = merkle_descriptor(validation.kind, record)?;
    let key =
        match validation.kind {
            GatewayRecordKind::Provider => {
                let provider_id = parse_hex_array::<32>(
                    record.provider_id_hex.as_deref().ok_or_else(|| {
                        eyre!("`provider_id_hex` is required for provider entries")
                    })?,
                    "provider_id_hex",
                )?;
                GatewayMerkleLeafKey::Provider { provider_id }
            }
            GatewayRecordKind::ManifestDigest => {
                let digest = parse_hex_array::<32>(
                    record.manifest_digest_hex.as_deref().ok_or_else(|| {
                        eyre!("`manifest_digest_hex` is required for manifest entries")
                    })?,
                    "manifest_digest_hex",
                )?;
                GatewayMerkleLeafKey::ManifestDigest { digest }
            }
            GatewayRecordKind::Cid => {
                let (encoding, payload, _) = canonicalize_cid(record)?;
                GatewayMerkleLeafKey::Cid { encoding, payload }
            }
            GatewayRecordKind::Url => GatewayMerkleLeafKey::Url {
                url: record
                    .url
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| eyre!("`url` is required for url entries"))?
                    .to_owned(),
            },
            GatewayRecordKind::AccountId => GatewayMerkleLeafKey::AccountId {
                account: record
                    .account_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| eyre!("`account_id` is required for account_id entries"))?
                    .to_owned(),
            },
            GatewayRecordKind::AccountAlias => GatewayMerkleLeafKey::AccountAlias {
                alias: record
                    .account_alias
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| eyre!("`account_alias` is required for account_alias entries"))?
                    .to_owned(),
            },
            GatewayRecordKind::PerceptualFamily => {
                let family_id = parse_hex_array::<16>(
                    record.family_id_hex.as_deref().ok_or_else(|| {
                        eyre!("`family_id_hex` is required for perceptual entries")
                    })?,
                    "family_id_hex",
                )?;
                let variant_id = parse_hex_array::<16>(
                    record.variant_id_hex.as_deref().ok_or_else(|| {
                        eyre!("`variant_id_hex` is required for perceptual entries")
                    })?,
                    "variant_id_hex",
                )?;
                let perceptual_hash = parse_hex_array::<32>(
                    record.perceptual_hash_hex.as_deref().ok_or_else(|| {
                        eyre!("`perceptual_hash_hex` is required for perceptual entries")
                    })?,
                    "perceptual_hash_hex",
                )?;
                let hamming_radius = record.perceptual_hamming_radius.ok_or_else(|| {
                    eyre!("`perceptual_hamming_radius` is required for perceptual entries")
                })?;
                GatewayMerkleLeafKey::PerceptualFamily {
                    family_id,
                    variant_id,
                    perceptual_hash,
                    hamming_radius,
                    attack_vector: record
                        .attack_vector
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned),
                }
            }
        };

    let policy = validation.policy;
    let leaf = GatewayMerkleLeafV1 {
        schema_version: GATEWAY_MERKLE_SCHEMA_VERSION,
        key,
        policy_tier: encode_policy_tier(policy.tier()),
        issued_at_unix: policy.issued_at.map(OffsetDateTime::unix_timestamp),
        expires_at_unix: policy.expires_at.map(OffsetDateTime::unix_timestamp),
        review_due_at_unix: policy.review_due_at.map(OffsetDateTime::unix_timestamp),
        max_expires_at_unix: policy.max_expires_at.map(OffsetDateTime::unix_timestamp),
        jurisdiction: trimmed_option(record.jurisdiction.as_deref()),
        reason: trimmed_option(record.reason.as_deref()),
        alias: record
            .alias
            .as_deref()
            .or(record.account_alias.as_deref())
            .and_then(trimmed_option_str),
        emergency_canon: trimmed_option(record.emergency_canon.as_deref()),
        governance_reference: trimmed_option(record.governance_reference.as_deref()),
    };
    let hash = hash_leaf(&leaf.encode());

    Ok(GatewayMerkleLeaf {
        kind: validation.kind,
        descriptor,
        policy_tier: policy.tier(),
        hash,
    })
}

fn encode_policy_tier(tier: GatewayRecordPolicyTier) -> u8 {
    match tier {
        GatewayRecordPolicyTier::Standard => 0,
        GatewayRecordPolicyTier::Emergency => 1,
        GatewayRecordPolicyTier::Permanent => 2,
    }
}

fn trimmed_option(value: Option<&str>) -> Option<String> {
    value.and_then(trimmed_option_str)
}

fn trimmed_option_str(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn canonicalize_cid(
    record: &GatewayDenylistRecord,
) -> Result<(GatewayMerkleCidEncoding, Vec<u8>, String)> {
    if let Some(b64) = record.cid_b64.as_deref() {
        let trimmed = b64.trim();
        let payload = decode_base64_string(trimmed, "cid_b64")?;
        return Ok((
            GatewayMerkleCidEncoding::Base64,
            payload,
            trimmed.to_owned(),
        ));
    }
    if let Some(hex_value) = record.cid_hex.as_deref() {
        let trimmed = hex_value.trim();
        let payload =
            hex::decode(trimmed).map_err(|err| eyre!("failed to decode cid hex payload: {err}"))?;
        return Ok((GatewayMerkleCidEncoding::Hex, payload, trimmed.to_owned()));
    }
    if let Some(text) = record.cid_utf8.as_deref() {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(eyre!("`cid_utf8` must not be empty"));
        }
        return Ok((
            GatewayMerkleCidEncoding::Utf8,
            trimmed.as_bytes().to_vec(),
            trimmed.to_owned(),
        ));
    }
    Err(eyre!(
        "cid entries require `cid_b64`, `cid_hex`, or `cid_utf8` field"
    ))
}

fn merkle_descriptor(kind: GatewayRecordKind, record: &GatewayDenylistRecord) -> Result<String> {
    match kind {
        GatewayRecordKind::Provider => {
            let bytes = parse_hex_array::<32>(
                record
                    .provider_id_hex
                    .as_deref()
                    .ok_or_else(|| eyre!("`provider_id_hex` missing for provider entry"))?,
                "provider_id_hex",
            )?;
            Ok(format!("provider:{}", hex::encode_upper(bytes)))
        }
        GatewayRecordKind::ManifestDigest => {
            let bytes = parse_hex_array::<32>(
                record
                    .manifest_digest_hex
                    .as_deref()
                    .ok_or_else(|| eyre!("`manifest_digest_hex` missing for manifest entry"))?,
                "manifest_digest_hex",
            )?;
            Ok(format!("manifest_digest:{}", hex::encode_upper(bytes)))
        }
        GatewayRecordKind::Cid => {
            let (encoding, _, literal) = canonicalize_cid(record)?;
            let label = match encoding {
                GatewayMerkleCidEncoding::Base64 => "b64",
                GatewayMerkleCidEncoding::Hex => "hex",
                GatewayMerkleCidEncoding::Utf8 => "utf8",
            };
            Ok(format!("cid:{label}:{literal}"))
        }
        GatewayRecordKind::Url => record
            .url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("url:{value}"))
            .ok_or_else(|| eyre!("`url` is required for url entries")),
        GatewayRecordKind::AccountId => record
            .account_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("account_id:{value}"))
            .ok_or_else(|| eyre!("`account_id` is required for account_id entries")),
        GatewayRecordKind::AccountAlias => record
            .account_alias
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("account_alias:{value}"))
            .ok_or_else(|| eyre!("`account_alias` is required for account_alias entries")),
        GatewayRecordKind::PerceptualFamily => {
            let family = record
                .family_id_hex
                .as_deref()
                .ok_or_else(|| eyre!("`family_id_hex` is required for perceptual entries"))?;
            let variant = record
                .variant_id_hex
                .as_deref()
                .ok_or_else(|| eyre!("`variant_id_hex` is required for perceptual entries"))?;
            Ok(format!(
                "perceptual_family:{}:{}",
                family.trim(),
                variant.trim()
            ))
        }
    }
}

fn hash_leaf(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

fn build_merkle_levels(leaves: &[[u8; 32]]) -> Result<Vec<Vec<[u8; 32]>>> {
    if leaves.is_empty() {
        return Err(eyre!("cannot compute Merkle levels for zero leaves"));
    }
    let mut current = leaves.to_vec();
    let mut levels = vec![current.clone()];

    while current.len() > 1 {
        let mut next = Vec::with_capacity(current.len().div_ceil(2));
        for chunk in current.chunks(2) {
            let parent = if chunk.len() == 2 {
                hash_pair(&chunk[0], &chunk[1])
            } else {
                hash_pair(&chunk[0], &chunk[0])
            };
            next.push(parent);
        }
        levels.push(next.clone());
        current = next;
    }

    Ok(levels)
}

fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(left);
    hasher.update(right);
    *hasher.finalize().as_bytes()
}

fn build_merkle_proof(
    levels: &[Vec<[u8; 32]>],
    mut index: usize,
) -> Result<Vec<GatewayMerkleProofNode>> {
    if levels.is_empty() || levels[0].is_empty() {
        return Err(eyre!("cannot build Merkle proof without leaves"));
    }
    if index >= levels[0].len() {
        return Err(eyre!(
            "requested proof index {index} exceeds leaf count {}",
            levels[0].len()
        ));
    }
    if levels.len() == 1 {
        return Ok(Vec::new());
    }

    let mut proof = Vec::with_capacity(levels.len() - 1);
    for nodes in levels.iter().take(levels.len() - 1) {
        let sibling_index = if index.is_multiple_of(2) {
            index + 1
        } else {
            index.saturating_sub(1)
        };
        if sibling_index < nodes.len() {
            let direction = if index.is_multiple_of(2) {
                "right"
            } else {
                "left"
            };
            proof.push(GatewayMerkleProofNode {
                direction: direction.to_owned(),
                sibling_hex: format_digest_hex(&nodes[sibling_index]),
                duplicate: false,
            });
        } else {
            proof.push(GatewayMerkleProofNode {
                direction: "right".to_owned(),
                sibling_hex: format_digest_hex(&nodes[index]),
                duplicate: true,
            });
        }
        index /= 2;
    }

    Ok(proof)
}

fn format_digest_hex(digest: &[u8; 32]) -> String {
    hex::encode_upper(digest)
}

fn make_snapshot_entry(index: usize, leaf: &GatewayMerkleLeaf) -> GatewayMerkleSnapshotEntry {
    GatewayMerkleSnapshotEntry {
        index,
        kind: leaf.kind.as_str().to_owned(),
        descriptor: leaf.descriptor.clone(),
        hash_hex: format_digest_hex(&leaf.hash),
        policy_tier: leaf.policy_tier.as_str().to_owned(),
    }
}

fn find_merkle_entry_by_descriptor(
    leaves: &[GatewayMerkleLeaf],
    descriptor: &str,
) -> Option<(usize, GatewayMerkleSnapshotEntry)> {
    let needle = descriptor.trim();
    if needle.is_empty() {
        return None;
    }
    leaves.iter().enumerate().find_map(|(index, leaf)| {
        let entry = make_snapshot_entry(index, leaf);
        if entry.descriptor == needle {
            Some((index, entry))
        } else {
            None
        }
    })
}

fn build_gateway_merkle_snapshot(
    entries: &[GatewayDenylistRecord],
    resolve: &dyn Fn(&str) -> Result<AccountId>,
) -> Result<GatewayMerkleSnapshotSummary> {
    let leaves = build_gateway_merkle_leaves(entries, resolve)?;
    let hashes: Vec<[u8; 32]> = leaves.iter().map(|leaf| leaf.hash).collect();
    let levels = build_merkle_levels(&hashes)?;
    let root = levels
        .last()
        .and_then(|level| level.first())
        .ok_or_else(|| eyre!("failed to compute Merkle root"))?;
    let summary_entries: Vec<_> = leaves
        .iter()
        .enumerate()
        .map(|(index, leaf)| make_snapshot_entry(index, leaf))
        .collect();
    Ok(GatewayMerkleSnapshotSummary {
        schema_version: GATEWAY_MERKLE_SCHEMA_VERSION,
        root_hex: format_digest_hex(root),
        leaf_count: summary_entries.len(),
        entries: summary_entries,
    })
}

fn write_gateway_merkle_snapshot(
    summary: &GatewayMerkleSnapshotSummary,
    json_out: &Path,
    norito_out: Option<&Path>,
) -> Result<()> {
    ensure_parent_dir(json_out)?;
    let rendered =
        norito::json::to_json_pretty(summary).wrap_err("failed to render Merkle snapshot JSON")?;
    fs::write(json_out, rendered.as_bytes()).wrap_err_with(|| {
        format!(
            "failed to write Merkle snapshot to `{}`",
            json_out.display()
        )
    })?;

    if let Some(path) = norito_out {
        ensure_parent_dir(path)?;
        write_norito_payload(path, summary)?;
    }

    Ok(())
}

fn build_denylist_evidence(
    entries: &[GatewayDenylistRecord],
    digest_hex: &str,
    source_path: &Path,
    label: Option<&str>,
    generated_at: OffsetDateTime,
    resolve: &dyn Fn(&str) -> Result<AccountId>,
) -> Result<GatewayDenylistEvidenceReport> {
    let mut kind_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut tier_counts: BTreeMap<&'static str, TierAccumulator> = BTreeMap::new();
    let mut emergency_reviews = Vec::new();

    for (index, record) in entries.iter().enumerate() {
        let validation = record
            .validate(resolve)
            .wrap_err_with(|| format!("validation failed for entry #{index}"))?;
        *kind_counts.entry(validation.kind.as_str()).or_default() += 1;
        tier_counts
            .entry(validation.policy.tier().as_str())
            .or_default()
            .observe(&validation.policy);
        if validation.policy.tier() == GatewayRecordPolicyTier::Emergency {
            emergency_reviews.push(GatewayEmergencyEvidence::from_record(
                index,
                record,
                &validation,
            )?);
        }
    }

    let totals = GatewayEvidenceTotals {
        by_kind: kind_counts
            .into_iter()
            .map(|(label, count)| EvidenceCount {
                label: label.to_owned(),
                count,
            })
            .collect(),
        by_policy_tier: tier_counts
            .into_iter()
            .map(|(tier, acc)| acc.into_summary(tier))
            .collect(),
    };

    Ok(GatewayDenylistEvidenceReport {
        generated_at: format_datetime(generated_at),
        label: label.map(str::to_owned),
        source: GatewayEvidenceSource {
            path: source_path.display().to_string(),
            digest_blake3_hex: digest_hex.to_owned(),
            entry_count: entries.len(),
        },
        totals,
        emergency_reviews,
    })
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

impl Run for GatewayEvidenceArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = fs::read(&self.denylist_path).wrap_err_with(|| {
            format!(
                "failed to read denylist from `{}`",
                self.denylist_path.display()
            )
        })?;
        let entries: Vec<GatewayDenylistRecord> =
            norito::json::from_slice(&bytes).wrap_err("failed to parse denylist JSON")?;
        let digest_hex = blake3::hash(&bytes).to_hex().to_string();
        let generated_at = OffsetDateTime::now_utc();
        let resolve = |literal: &str| crate::resolve_account_id(context, literal);
        let report = build_denylist_evidence(
            &entries,
            &digest_hex,
            &self.denylist_path,
            self.label.as_deref(),
            generated_at,
            &resolve,
        )?;

        if let Some(parent) = self.output_path.parent() {
            fs::create_dir_all(parent)
                .wrap_err_with(|| format!("failed to create `{}`", parent.display()))?;
        }
        let payload = norito::json::to_vec_pretty(&report)?;
        fs::write(&self.output_path, &payload).wrap_err_with(|| {
            format!(
                "failed to write evidence bundle to `{}`",
                self.output_path.display()
            )
        })?;
        context.println(format_args!(
            "wrote denylist evidence for {} entries to {} (digest {})",
            report.source.entry_count,
            self.output_path.display(),
            report.source.digest_blake3_hex,
        ))?;
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

#[derive(Debug, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct GatewayDenylistRecord {
    kind: String,
    #[norito(default)]
    provider_id_hex: Option<String>,
    #[norito(default)]
    manifest_digest_hex: Option<String>,
    #[norito(default)]
    cid_b64: Option<String>,
    #[norito(default)]
    cid_hex: Option<String>,
    #[norito(default)]
    cid_utf8: Option<String>,
    #[norito(default)]
    url: Option<String>,
    #[norito(default)]
    account_id: Option<String>,
    #[norito(default)]
    account_alias: Option<String>,
    #[norito(default)]
    jurisdiction: Option<String>,
    #[norito(default)]
    reason: Option<String>,
    #[norito(default)]
    alias: Option<String>,
    #[norito(default)]
    issued_at: Option<String>,
    #[norito(default)]
    expires_at: Option<String>,
    #[norito(default)]
    family_id_hex: Option<String>,
    #[norito(default)]
    variant_id_hex: Option<String>,
    #[norito(default)]
    attack_vector: Option<String>,
    #[norito(default)]
    perceptual_hash_hex: Option<String>,
    #[norito(default)]
    perceptual_hamming_radius: Option<u8>,
    #[norito(default)]
    policy_tier: Option<String>,
    #[norito(default)]
    emergency_canon: Option<String>,
    #[norito(default)]
    governance_reference: Option<String>,
}

const GATEWAY_MERKLE_SCHEMA_VERSION: u16 = 1;

#[allow(clippy::size_of_ref)]
#[derive(Debug, Encode)]
struct GatewayMerkleLeafV1 {
    schema_version: u16,
    key: GatewayMerkleLeafKey,
    policy_tier: u8,
    issued_at_unix: Option<i64>,
    expires_at_unix: Option<i64>,
    review_due_at_unix: Option<i64>,
    max_expires_at_unix: Option<i64>,
    jurisdiction: Option<String>,
    reason: Option<String>,
    alias: Option<String>,
    emergency_canon: Option<String>,
    governance_reference: Option<String>,
}

#[allow(clippy::size_of_ref)]
#[derive(Debug, Encode)]
enum GatewayMerkleLeafKey {
    Provider {
        provider_id: [u8; 32],
    },
    ManifestDigest {
        digest: [u8; 32],
    },
    Cid {
        encoding: GatewayMerkleCidEncoding,
        payload: Vec<u8>,
    },
    Url {
        url: String,
    },
    AccountId {
        account: String,
    },
    AccountAlias {
        alias: String,
    },
    PerceptualFamily {
        family_id: [u8; 16],
        variant_id: [u8; 16],
        perceptual_hash: [u8; 32],
        hamming_radius: u8,
        attack_vector: Option<String>,
    },
}

#[allow(clippy::size_of_ref)]
#[derive(Debug, Encode, Copy, Clone)]
enum GatewayMerkleCidEncoding {
    Base64,
    Hex,
    Utf8,
}

#[derive(Clone, Debug)]
struct GatewayMerkleLeaf {
    kind: GatewayRecordKind,
    descriptor: String,
    policy_tier: GatewayRecordPolicyTier,
    hash: [u8; 32],
}

#[derive(
    Debug,
    Clone,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
struct GatewayMerkleSnapshotSummary {
    schema_version: u16,
    root_hex: String,
    leaf_count: usize,
    entries: Vec<GatewayMerkleSnapshotEntry>,
}

#[derive(
    Clone,
    Debug,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
struct GatewayMerkleSnapshotEntry {
    index: usize,
    kind: String,
    descriptor: String,
    hash_hex: String,
    policy_tier: String,
}

#[derive(
    Debug,
    Clone,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
struct GatewayMerkleProofOutput {
    schema_version: u16,
    root_hex: String,
    entry: GatewayMerkleSnapshotEntry,
    proof: Vec<GatewayMerkleProofNode>,
}

#[derive(
    Debug,
    Clone,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
struct GatewayMerkleProofNode {
    direction: String,
    sibling_hex: String,
    duplicate: bool,
}

const STANDARD_POLICY_TTL: Duration = defaults::sorafs::gateway::denylist::STANDARD_TTL;
const EMERGENCY_POLICY_TTL: Duration = defaults::sorafs::gateway::denylist::EMERGENCY_TTL;
const EMERGENCY_REVIEW_WINDOW: Duration =
    defaults::sorafs::gateway::denylist::EMERGENCY_REVIEW_WINDOW;
const PERMANENT_NEEDS_REFERENCE: bool =
    defaults::sorafs::gateway::denylist::REQUIRE_GOVERNANCE_REFERENCE;

impl GatewayDenylistRecord {
    fn validate(
        &self,
        resolve: &dyn Fn(&str) -> Result<AccountId>,
    ) -> Result<GatewayRecordValidation> {
        ensure_optional_non_empty(self.jurisdiction.as_deref(), "jurisdiction")?;
        ensure_optional_non_empty(self.reason.as_deref(), "reason")?;
        ensure_optional_non_empty(self.alias.as_deref(), "alias")?;

        let policy = self.policy_window()?;

        let kind = match self.kind.as_str() {
            "provider" => {
                let hex = self
                    .provider_id_hex
                    .as_deref()
                    .ok_or_else(|| eyre!("`provider_id_hex` is required for provider entries"))?;
                let _: [u8; 32] = parse_hex_array::<32>(hex, "provider_id_hex")?;
                Ok(GatewayRecordKind::Provider)
            }
            "manifest_digest" => {
                let hex = self.manifest_digest_hex.as_deref().ok_or_else(|| {
                    eyre!("`manifest_digest_hex` is required for manifest_digest entries")
                })?;
                let _: [u8; 32] = parse_hex_array::<32>(hex, "manifest_digest_hex")?;
                Ok(GatewayRecordKind::ManifestDigest)
            }
            "cid" => {
                self.validate_cid_fields()?;
                Ok(GatewayRecordKind::Cid)
            }
            "url" => {
                let value = self
                    .url
                    .as_deref()
                    .ok_or_else(|| eyre!("`url` is required for url entries"))?;
                if value.trim().is_empty() {
                    return Err(eyre!("`url` must not be empty"));
                }
                Ok(GatewayRecordKind::Url)
            }
            "account_id" => {
                let value = self
                    .account_id
                    .as_deref()
                    .ok_or_else(|| eyre!("`account_id` is required for account_id entries"))?;
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err(eyre!("`account_id` must not be empty"));
                }
                // Validate as a strict encoded account literal (IH58/compressed only).
                resolve(trimmed).wrap_err("failed to resolve `account_id`")?;
                if let Some(alias) = self.account_alias.as_deref()
                    && alias.trim().is_empty()
                {
                    return Err(eyre!("`account_alias` must not be empty when supplied"));
                }
                Ok(GatewayRecordKind::AccountId)
            }
            "account_alias" => {
                let alias = self.account_alias.as_deref().ok_or_else(|| {
                    eyre!("`account_alias` is required for account_alias entries")
                })?;
                let trimmed = alias.trim();
                if trimmed.is_empty() {
                    return Err(eyre!("`account_alias` must not be empty"));
                }
                if !trimmed.contains('@') {
                    return Err(eyre!(
                        "`account_alias` must include an `@` separator (got `{trimmed}`)"
                    ));
                }
                Ok(GatewayRecordKind::AccountAlias)
            }
            "perceptual_family" => {
                let family_hex = self.family_id_hex.as_deref().ok_or_else(|| {
                    eyre!("`family_id_hex` is required for perceptual_family entries")
                })?;
                let _: [u8; 16] = parse_hex_array::<16>(family_hex, "family_id_hex")?;

                let variant_hex = self.variant_id_hex.as_deref().ok_or_else(|| {
                    eyre!("`variant_id_hex` is required for perceptual_family entries")
                })?;
                let _: [u8; 16] = parse_hex_array::<16>(variant_hex, "variant_id_hex")?;

                let hash_hex = self.perceptual_hash_hex.as_deref().ok_or_else(|| {
                    eyre!("`perceptual_hash_hex` is required for perceptual_family entries")
                })?;
                let _: [u8; 32] = parse_hex_array::<32>(hash_hex, "perceptual_hash_hex")?;

                let radius = self.perceptual_hamming_radius.ok_or_else(|| {
                    eyre!("`perceptual_hamming_radius` is required for perceptual_family entries")
                })?;
                if radius == 0 {
                    return Err(eyre!(
                        "`perceptual_hamming_radius` must be greater than zero"
                    ));
                }
                ensure_optional_non_empty(self.attack_vector.as_deref(), "attack_vector")?;
                Ok(GatewayRecordKind::PerceptualFamily)
            }
            other => Err(eyre!("unsupported denylist kind `{other}`")),
        }?;

        Ok(GatewayRecordValidation { kind, policy })
    }

    fn validate_cid_fields(&self) -> Result<()> {
        if let Some(b64) = self.cid_b64.as_deref() {
            let decoded = STANDARD
                .decode(b64.trim())
                .wrap_err("invalid base64 payload in `cid_b64`")?;
            if decoded.is_empty() {
                return Err(eyre!("`cid_b64` must not decode to an empty payload"));
            }
            return Ok(());
        }

        if let Some(hex_value) = self.cid_hex.as_deref() {
            let decoded =
                hex::decode(hex_value.trim()).wrap_err("invalid hex payload in `cid_hex`")?;
            if decoded.is_empty() {
                return Err(eyre!("`cid_hex` must not decode to an empty payload"));
            }
            return Ok(());
        }

        if let Some(text) = self.cid_utf8.as_deref() {
            if text.trim().is_empty() {
                return Err(eyre!("`cid_utf8` must not be empty"));
            }
            return Ok(());
        }

        Err(eyre!(
            "cid entries require `cid_b64`, `cid_hex`, or `cid_utf8` field"
        ))
    }

    fn policy_window(&self) -> Result<GatewayPolicyWindow> {
        let issued_at = parse_timestamp(self.issued_at.as_deref(), "issued_at")?;
        let expires_at = parse_timestamp(self.expires_at.as_deref(), "expires_at")?;
        if let (Some(issued), Some(expires)) = (issued_at, expires_at)
            && expires <= issued
        {
            return Err(eyre!(
                "`expires_at` must be later than `issued_at` ({} <= {})",
                expires,
                issued
            ));
        }

        let tier = GatewayRecordPolicyTier::parse(self.policy_tier.as_deref())?;
        let mut max_expires_at = None;
        let mut review_due_at = None;

        match tier {
            GatewayRecordPolicyTier::Standard => {
                let issued = issued_at
                    .ok_or_else(|| eyre!("`issued_at` is required for standard entries"))?;
                let dead_line = add_datetime_duration(issued, STANDARD_POLICY_TTL, "standard TTL")?;
                let expires = expires_at.unwrap_or(dead_line);
                if expires > dead_line {
                    return Err(eyre!(
                        "`expires_at` exceeds the {}-second standard TTL",
                        STANDARD_POLICY_TTL.as_secs()
                    ));
                }
                max_expires_at = Some(dead_line);
            }
            GatewayRecordPolicyTier::Emergency => {
                let issued = issued_at
                    .ok_or_else(|| eyre!("`issued_at` is required for emergency entries"))?;
                if tier.requires_canon()
                    && self
                        .emergency_canon
                        .as_deref()
                        .is_none_or(|value| value.trim().is_empty())
                {
                    return Err(eyre!("`emergency_canon` is required for emergency entries"));
                }
                let max_expiry =
                    add_datetime_duration(issued, EMERGENCY_POLICY_TTL, "emergency TTL")?;
                let expires = expires_at.unwrap_or(max_expiry);
                if expires > max_expiry {
                    return Err(eyre!(
                        "`expires_at` exceeds the {}-second emergency TTL",
                        EMERGENCY_POLICY_TTL.as_secs()
                    ));
                }
                let review_deadline = add_datetime_duration(
                    issued,
                    EMERGENCY_REVIEW_WINDOW,
                    "emergency review window",
                )?;
                review_due_at = Some(review_deadline);
                max_expires_at = Some(max_expiry);
            }
            GatewayRecordPolicyTier::Permanent => {
                if expires_at.is_some() {
                    return Err(eyre!("permanent entries must omit `expires_at`"));
                }
                if PERMANENT_NEEDS_REFERENCE
                    && tier.requires_governance_reference()
                    && self
                        .governance_reference
                        .as_deref()
                        .is_none_or(|value| value.trim().is_empty())
                {
                    return Err(eyre!(
                        "`governance_reference` is required for permanent entries"
                    ));
                }
            }
        }

        Ok(GatewayPolicyWindow {
            tier,
            issued_at,
            expires_at,
            max_expires_at,
            review_due_at,
        })
    }
}

#[derive(Copy, Clone, Debug)]
enum GatewayRecordKind {
    Provider,
    ManifestDigest,
    Cid,
    Url,
    AccountId,
    AccountAlias,
    PerceptualFamily,
}

impl GatewayRecordKind {
    fn as_str(self) -> &'static str {
        match self {
            GatewayRecordKind::Provider => "provider",
            GatewayRecordKind::ManifestDigest => "manifest_digest",
            GatewayRecordKind::Cid => "cid",
            GatewayRecordKind::Url => "url",
            GatewayRecordKind::AccountId => "account_id",
            GatewayRecordKind::AccountAlias => "account_alias",
            GatewayRecordKind::PerceptualFamily => "perceptual_family",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct GatewayPolicyWindow {
    tier: GatewayRecordPolicyTier,
    issued_at: Option<OffsetDateTime>,
    expires_at: Option<OffsetDateTime>,
    max_expires_at: Option<OffsetDateTime>,
    review_due_at: Option<OffsetDateTime>,
}

impl GatewayPolicyWindow {
    fn tier(self) -> GatewayRecordPolicyTier {
        self.tier
    }
}

#[derive(Clone, Copy, Debug)]
struct GatewayRecordValidation {
    kind: GatewayRecordKind,
    policy: GatewayPolicyWindow,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum GatewayRecordPolicyTier {
    Standard,
    Emergency,
    Permanent,
}

impl GatewayRecordPolicyTier {
    fn parse(value: Option<&str>) -> Result<Self> {
        let Some(label) = value.map(|raw| raw.trim().to_ascii_lowercase()) else {
            return Ok(Self::Standard);
        };
        match label.as_str() {
            "standard" | "default" => Ok(Self::Standard),
            "emergency" => Ok(Self::Emergency),
            "permanent" => Ok(Self::Permanent),
            other => Err(eyre!(
                "unsupported policy tier `{other}`; expected standard|emergency|permanent"
            )),
        }
    }

    fn requires_canon(self) -> bool {
        matches!(self, Self::Emergency)
    }

    fn requires_governance_reference(self) -> bool {
        matches!(self, Self::Permanent)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Emergency => "emergency",
            Self::Permanent => "permanent",
        }
    }
}

#[cfg(test)]
mod gateway_tests {
    use super::tests::TestContext;
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn resolve_account_literal(literal: &str) -> Result<AccountId> {
        AccountId::parse_encoded(literal)
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .wrap_err_with(|| format!("invalid test account literal `{literal}`"))
    }

    #[test]
    fn sample_denylist_is_valid() {
        let json = include_str!("../../../../docs/source/sorafs_gateway_denylist_sample.json");
        let entries: Vec<GatewayDenylistRecord> =
            norito::json::from_str(json).expect("parse sample denylist");
        assert!(!entries.is_empty(), "sample denylist should not be empty");
        for entry in entries {
            entry
                .validate(&resolve_account_literal)
                .expect("sample entry validation");
        }
    }

    #[test]
    fn template_config_uses_host_override() {
        let args = GatewayTemplateConfigArgs {
            hosts: vec![
                "gateway-a.example.com".to_owned(),
                "gateway-b.example.com".to_owned(),
            ],
            denylist_path: "denylist.json".to_owned(),
        };

        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("template command runs");
        let rendered = ctx.outputs().join("\n");
        assert!(rendered.contains("gateway-a.example.com"));
        assert!(rendered.contains("gateway-b.example.com"));
        assert!(rendered.contains("denylist.json"));
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
        assert!(
            !ctx.outputs().is_empty(),
            "expected at least one daemon output entry"
        );
        let output = &ctx.outputs()[0];
        assert!(output.contains("canonical"));
        assert!(output.contains("vanity"));
        assert!(output.contains("aaaaaaaa.nexus.sorafs"));
        assert!(output.contains("aaaa.nexus.direct.sorafs"));
    }

    #[test]
    fn rejects_missing_provider_hex() {
        let record = GatewayDenylistRecord {
            kind: "provider".to_owned(),
            provider_id_hex: None,
            manifest_digest_hex: None,
            cid_b64: None,
            cid_hex: None,
            cid_utf8: None,
            url: None,
            account_id: None,
            account_alias: None,
            jurisdiction: None,
            reason: None,
            alias: None,
            issued_at: None,
            expires_at: None,
            family_id_hex: None,
            variant_id_hex: None,
            attack_vector: None,
            perceptual_hash_hex: None,
            perceptual_hamming_radius: None,
            policy_tier: None,
            emergency_canon: None,
            governance_reference: None,
        };
        assert!(record.validate(&resolve_account_literal).is_err());
    }

    #[test]
    fn evidence_builder_reports_emergency_entries() {
        let json = r#"
        [
            {
                "kind": "account_id",
                "account_id": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
                "policy_tier": "standard",
                "issued_at": "2026-01-01T00:00:00Z",
                "expires_at": "2026-06-30T00:00:00Z"
            },
            {
                "kind": "account_alias",
                "account_alias": "hotline@hbl",
                "policy_tier": "emergency",
                "issued_at": "2026-01-15T00:00:00Z",
                "expires_at": "2026-01-20T00:00:00Z",
                "emergency_canon": "csam-hotline"
            }
        ]
        "#;
        let entries: Vec<GatewayDenylistRecord> =
            norito::json::from_str(json).expect("parse fixture");
        let timestamp = OffsetDateTime::from_unix_timestamp(1_735_488_000).expect("timestamp");
        let report = build_denylist_evidence(
            &entries,
            "deadbeef",
            Path::new("denylist.json"),
            Some("test-bundle"),
            timestamp,
            &resolve_account_literal,
        )
        .expect("evidence");
        assert_eq!(report.source.entry_count, 2);
        assert_eq!(report.label.as_deref(), Some("test-bundle"));
        assert_eq!(report.totals.by_kind.len(), 2);
        assert_eq!(report.totals.by_policy_tier.len(), 2);
        assert_eq!(report.emergency_reviews.len(), 1);
        assert_eq!(
            report.emergency_reviews[0].descriptor.as_deref(),
            Some("hotline@hbl")
        );
    }

    #[test]
    fn merkle_snapshot_summary_matches_expected_descriptors() {
        let json = r#"
        [
            {
                "kind": "account_id",
                "account_id": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
                "policy_tier": "standard",
                "issued_at": "2026-01-01T00:00:00Z",
                "expires_at": "2026-06-15T00:00:00Z"
            },
            {
                "kind": "provider",
                "provider_id_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "policy_tier": "emergency",
                "issued_at": "2026-02-01T00:00:00Z",
                "expires_at": "2026-02-15T00:00:00Z",
                "emergency_canon": "csam-hotline"
            }
        ]
        "#;
        let entries: Vec<GatewayDenylistRecord> =
            norito::json::from_str(json).expect("parse fixture");
        let leaves = build_gateway_merkle_leaves(&entries, &resolve_account_literal)
            .expect("canonical leaves");
        let hashes: Vec<[u8; 32]> = leaves.iter().map(|leaf| leaf.hash).collect();
        let levels = build_merkle_levels(&hashes).expect("build levels");
        let root_hex = levels
            .last()
            .and_then(|level| level.first())
            .map(format_digest_hex)
            .expect("root hex");
        assert_eq!(leaves.len(), 2);
        assert_eq!(root_hex.len(), 64);
        let summary_entries: Vec<_> = leaves
            .iter()
            .enumerate()
            .map(|(index, leaf)| make_snapshot_entry(index, leaf))
            .collect();
        assert_eq!(summary_entries[0].kind, "account_id");
        assert_eq!(
            summary_entries[0].descriptor,
            "account_id:6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"
        );
        assert_eq!(summary_entries[1].kind, "provider");
        assert_eq!(
            summary_entries[1].descriptor,
            "provider:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
        );
        assert_eq!(summary_entries[0].hash_hex.len(), 64);
        assert_eq!(summary_entries[1].hash_hex.len(), 64);
    }

    #[test]
    fn merkle_proof_round_trips() {
        let json = r#"
        [
            {
                "kind": "account_id",
                "account_id": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
                "policy_tier": "standard",
                "issued_at": "2026-01-01T00:00:00Z",
                "expires_at": "2026-06-15T00:00:00Z"
            },
            {
                "kind": "provider",
                "provider_id_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "policy_tier": "emergency",
                "issued_at": "2026-02-01T00:00:00Z",
                "expires_at": "2026-02-15T00:00:00Z",
                "emergency_canon": "csam-hotline"
            }
        ]
        "#;
        let entries: Vec<GatewayDenylistRecord> =
            norito::json::from_str(json).expect("parse fixture");
        let leaves = build_gateway_merkle_leaves(&entries, &resolve_account_literal)
            .expect("canonical leaves");
        let hashes: Vec<[u8; 32]> = leaves.iter().map(|leaf| leaf.hash).collect();
        let levels = build_merkle_levels(&hashes).expect("build levels");
        let root = levels
            .last()
            .and_then(|level| level.first())
            .copied()
            .expect("root");
        let proof = build_merkle_proof(&levels, 1).expect("proof");
        let recomputed = verify_hex_proof(leaves[1].hash, &proof);
        assert_eq!(format_digest_hex(&recomputed), format_digest_hex(&root));
    }

    #[test]
    fn merkle_snapshot_emits_norito_payload() {
        let sample = r#"
        [
            {
                "kind": "account_id",
                "account_id": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
                "account_alias": "routing@sora",
                "policy_tier": "standard",
                "issued_at": "2025-01-01T00:00:00Z",
                "expires_at": "2025-02-01T00:00:00Z"
            },
            {
                "kind": "provider",
                "provider_id_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "policy_tier": "emergency",
                "emergency_canon": "csam-hotline",
                "issued_at": "2025-02-01T00:00:00Z",
                "expires_at": "2025-02-10T00:00:00Z"
            }
        ]
        "#;
        let temp = TempDir::new().expect("tempdir");
        let denylist_path = temp.path().join("denylist.json");
        fs::write(&denylist_path, sample).expect("write denylist");
        let json_out = temp.path().join("snapshot.json");
        let norito_out = temp.path().join("snapshot.to");
        let args = GatewayMerkleSnapshotArgs {
            denylist_path,
            json_out,
            norito_out: Some(norito_out.clone()),
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("snapshot command");
        let bytes = fs::read(&norito_out).expect("read norito");
        let summary: GatewayMerkleSnapshotSummary =
            decode_from_bytes(&bytes).expect("decode snapshot");
        assert!(!summary.entries.is_empty());
        assert_eq!(summary.root_hex.len(), 64);
    }

    #[test]
    fn merkle_proof_emits_norito_payload() {
        let sample = r#"
        [
            {
                "kind": "account_id",
                "account_id": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
                "account_alias": "routing@sora",
                "policy_tier": "standard",
                "issued_at": "2025-01-01T00:00:00Z",
                "expires_at": "2025-02-01T00:00:00Z"
            },
            {
                "kind": "provider",
                "provider_id_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "policy_tier": "emergency",
                "emergency_canon": "csam-hotline",
                "issued_at": "2025-02-01T00:00:00Z",
                "expires_at": "2025-02-10T00:00:00Z"
            }
        ]
        "#;
        let temp = TempDir::new().expect("tempdir");
        let denylist_path = temp.path().join("denylist.json");
        fs::write(&denylist_path, sample).expect("write denylist");
        let json_out = temp.path().join("proof.json");
        let norito_out = temp.path().join("proof.to");
        let args = GatewayMerkleProofArgs {
            denylist_path,
            index: Some(1),
            descriptor: None,
            json_out,
            norito_out: Some(norito_out.clone()),
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("proof command");
        let bytes = fs::read(&norito_out).expect("read norito proof");
        let proof: GatewayMerkleProofOutput = decode_from_bytes(&bytes).expect("decode proof");
        assert_eq!(proof.entry.index, 1);
        assert!(!proof.proof.is_empty());
        assert_eq!(proof.root_hex.len(), 64);
    }

    #[test]
    fn merkle_proof_descriptor_selection() {
        let sample = r#"
        [
            {
                "kind": "account_id",
                "account_id": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
                "account_alias": "routing@sora",
                "policy_tier": "standard",
                "issued_at": "2025-01-01T00:00:00Z",
                "expires_at": "2025-02-01T00:00:00Z"
            },
            {
                "kind": "provider",
                "provider_id_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "policy_tier": "emergency",
                "emergency_canon": "csam-hotline",
                "issued_at": "2025-02-01T00:00:00Z",
                "expires_at": "2025-02-10T00:00:00Z"
            }
        ]
        "#;
        let temp = TempDir::new().expect("tempdir");
        let denylist_path = temp.path().join("denylist.json");
        fs::write(&denylist_path, sample).expect("write denylist");
        let json_out = temp.path().join("proof_by_descriptor.json");
        let args = GatewayMerkleProofArgs {
            denylist_path,
            index: None,
            descriptor: Some(
                "provider:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                    .to_owned(),
            ),
            json_out,
            norito_out: None,
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("proof command");
        assert!(
            ctx.outputs()
                .iter()
                .any(|line| line.contains("entry #0001")),
            "stdout should mention the resolved descriptor"
        );
    }

    #[test]
    fn denylist_update_merges_entries_and_emits_artifacts() {
        let temp = TempDir::new().expect("tempdir");
        let base_path = temp.path().join("base.json");
        let add_path = temp.path().join("add.json");
        let output_path = temp.path().join("updated.json");
        let snapshot_json = temp.path().join("snapshot.json");
        let snapshot_norito = temp.path().join("snapshot.to");
        let evidence_path = temp.path().join("evidence.json");

        let base = r#"
        [
            {
                "kind": "account_alias",
                "account_alias": "alias@sora",
                "policy_tier": "standard",
                "issued_at": "2026-01-01T00:00:00Z",
                "expires_at": "2026-06-01T00:00:00Z"
            },
            {
                "kind": "provider",
                "provider_id_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "policy_tier": "standard",
                "issued_at": "2026-02-01T00:00:00Z",
                "expires_at": "2026-03-01T00:00:00Z"
            }
        ]
        "#;
        fs::write(&base_path, base).expect("write base denylist");

        let addition = r#"
        [
            {
                "kind": "manifest_digest",
                "manifest_digest_hex": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "policy_tier": "standard",
                "issued_at": "2026-02-10T00:00:00Z",
                "expires_at": "2026-03-10T00:00:00Z"
            }
        ]
        "#;
        fs::write(&add_path, addition).expect("write addition denylist");

        let args = GatewayUpdateDenylistArgs {
            base_path: base_path.clone(),
            add_paths: vec![add_path],
            remove_descriptors: vec!["account_alias:alias@sora".to_owned()],
            output_path: Some(output_path.clone()),
            snapshot_out: Some(snapshot_json.clone()),
            snapshot_norito_out: Some(snapshot_norito.clone()),
            evidence_out: Some(evidence_path.clone()),
            label: Some("test-run".to_owned()),
            force: false,
            allow_replacement: false,
            allow_missing_removals: false,
        };

        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("update command runs");

        let rendered = fs::read(&output_path).expect("read updated denylist");
        let entries: Vec<GatewayDenylistRecord> =
            norito::json::from_slice(&rendered).expect("parse updated denylist");
        assert_eq!(entries.len(), 2);
        assert!(
            ctx.outputs()
                .iter()
                .any(|line| line.contains("Merkle root")),
            "stdout should print merkle root"
        );

        let snapshot_bytes = fs::read(&snapshot_norito).expect("read norito snapshot");
        let summary: GatewayMerkleSnapshotSummary =
            decode_from_bytes(&snapshot_bytes).expect("decode norito snapshot");
        assert_eq!(summary.leaf_count, 2);
        assert_eq!(summary.root_hex.len(), 64);

        let evidence_bytes = fs::read(&evidence_path).expect("read evidence");
        let evidence: norito::json::Value =
            norito::json::from_slice(&evidence_bytes).expect("parse evidence json");
        assert_eq!(evidence["label"].as_str(), Some("test-run"));
        assert_eq!(evidence["source"]["entry_count"].as_u64(), Some(2));
    }

    #[test]
    fn denylist_update_rejects_duplicate_without_replacement() {
        let temp = TempDir::new().expect("tempdir");
        let base_path = temp.path().join("base.json");
        let add_path = temp.path().join("add.json");
        let output_path = temp.path().join("updated.json");

        let base = r#"
        [
            {
                "kind": "provider",
                "provider_id_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "policy_tier": "standard",
                "issued_at": "2026-02-01T00:00:00Z",
                "expires_at": "2026-03-01T00:00:00Z"
            }
        ]
        "#;
        fs::write(&base_path, base).expect("write base denylist");

        let addition = r#"
        [
            {
                "kind": "provider",
                "provider_id_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "policy_tier": "standard",
                "issued_at": "2026-04-01T00:00:00Z",
                "expires_at": "2026-05-01T00:00:00Z"
            }
        ]
        "#;
        fs::write(&add_path, addition).expect("write addition denylist");

        let args = GatewayUpdateDenylistArgs {
            base_path,
            add_paths: vec![add_path],
            remove_descriptors: Vec::new(),
            output_path: Some(output_path),
            snapshot_out: None,
            snapshot_norito_out: None,
            evidence_out: None,
            label: None,
            force: true,
            allow_replacement: false,
            allow_missing_removals: false,
        };

        let mut ctx = TestContext::new();
        let result = args.run(&mut ctx);
        assert!(
            result.is_err(),
            "duplicate descriptor should be rejected without allow-replacement"
        );
    }

    #[test]
    fn descriptor_normalisation_uppercases_provider_hex() {
        let descriptor =
            normalise_descriptor_input("provider:aabbccdd").expect("normalises descriptor");
        assert_eq!(descriptor, "provider:AABBCCDD");
    }

    fn verify_hex_proof(mut acc: [u8; 32], proof: &[GatewayMerkleProofNode]) -> [u8; 32] {
        for node in proof {
            let sibling = parse_hex_array::<32>(&node.sibling_hex, "sibling").expect("hex");
            let next = if node.duplicate {
                hash_pair(&acc, &acc)
            } else if node.direction == "right" {
                hash_pair(&acc, &sibling)
            } else {
                hash_pair(&sibling, &acc)
            };
            acc = next;
        }
        acc
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

impl Run for PinCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            PinCommand::List(args) => args.run(context),
            PinCommand::Show(args) => args.run(context),
            PinCommand::Register(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct PinListArgs {
    /// Optional status filter (pending, approved, retired).
    #[arg(long)]
    pub status: Option<String>,
    /// Maximum number of manifests to return.
    #[arg(long)]
    pub limit: Option<u32>,
    /// Offset for pagination.
    #[arg(long)]
    pub offset: Option<u32>,
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
        let client = context.client_from_config();
        let filter = SorafsPinListFilter {
            status: self.status.as_deref(),
            limit: self.limit,
            offset: self.offset,
        };
        let response = fetch(&client, &filter)?;
        render_json_response(context, response)
    }
}

#[derive(clap::Args, Debug)]
pub struct PinShowArgs {
    /// Hex-encoded manifest digest.
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
        let client = context.client_from_config();
        let response = fetch(&client, &self.digest)?;
        let status = response.status();
        let body = response.into_body();
        match status {
            StatusCode::OK => render_json_body(context, &body),
            StatusCode::NOT_FOUND => {
                context.println(format_args!("manifest `{}` not found", self.digest))
            }
            status => Err(make_http_error(status, &body)),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct PinRegisterArgs {
    /// Path to the Norito-encoded manifest (`.to`) file.
    #[arg(long, value_name = "PATH")]
    pub manifest: PathBuf,
    /// Hex-encoded SHA3-256 digest of the chunk metadata plan.
    #[arg(long, value_name = "HEX")]
    pub chunk_digest: String,
    /// Epoch recorded when submitting the manifest.
    #[arg(long)]
    pub submitted_epoch: u64,
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
        let manifest: ManifestV1 = norito::decode_from_bytes(&manifest_bytes)
            .wrap_err("failed to decode manifest payload")?;

        let chunk_digest = parse_hex_array::<32>(&self.chunk_digest, "chunk_digest")?;
        let alias_inputs = self.load_alias_inputs()?;
        let successor = self
            .successor_of
            .as_ref()
            .map(|hex| parse_hex_array::<32>(hex, "successor_of"))
            .transpose()?;

        let config = context.config();
        let client = context.client_from_config();

        let alias_ref = alias_inputs.as_ref().map(|alias| SorafsPinAlias {
            namespace: alias.namespace.as_str(),
            name: alias.name.as_str(),
            proof: alias.proof.as_slice(),
        });

        let response = client
            .post_sorafs_pin_register(SorafsPinRegisterArgs {
                authority: &config.account,
                private_key: config.key_pair.private_key(),
                manifest: &manifest,
                chunk_digest_sha3_256: chunk_digest,
                submitted_epoch: self.submitted_epoch,
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

impl Run for AliasCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            AliasCommand::List(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct AliasListArgs {
    /// Maximum number of aliases to return.
    #[arg(long)]
    pub limit: Option<u32>,
    /// Offset for pagination.
    #[arg(long)]
    pub offset: Option<u32>,
    /// Restrict aliases to a namespace (case-insensitive).
    #[arg(long)]
    pub namespace: Option<String>,
    /// Restrict aliases bound to a manifest digest (hex-encoded).
    #[arg(long, value_name = "HEX")]
    pub manifest_digest: Option<String>,
}

impl Run for AliasListArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        self.run_with(context, Client::get_sorafs_aliases)
    }
}

impl AliasListArgs {
    fn run_with<C, F>(&self, context: &mut C, fetch: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &SorafsAliasListFilter<'_>) -> Result<Response<Vec<u8>>>,
    {
        let client = context.client_from_config();
        let filter = SorafsAliasListFilter {
            limit: self.limit,
            offset: self.offset,
            namespace: self.namespace.as_deref(),
            manifest_digest: self.manifest_digest.as_deref(),
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

impl Run for ReplicationCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            ReplicationCommand::List(args) => args.run(context),
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
    /// Optional status filter (pending, completed, expired).
    #[arg(long)]
    pub status: Option<String>,
    /// Restrict to orders for a manifest digest (hex-encoded).
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
        let client = context.client_from_config();
        let filter = SorafsReplicationListFilter {
            limit: self.limit,
            offset: self.offset,
            status: self.status.as_deref(),
            manifest_digest: self.manifest_digest.as_deref(),
        };
        let response = fetch(&client, &filter)?;
        render_json_response(context, response)
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum StorageCommand {
    /// Submit a manifest + payload to local storage for pinning.
    Pin(StoragePinArgs),
    /// Issue and inspect stream tokens for chunk-range gateways.
    #[command(subcommand)]
    Token(StorageTokenCommand),
}

impl Run for StorageCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            StorageCommand::Pin(args) => args.run(context),
            StorageCommand::Token(cmd) => cmd.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct StoragePinArgs {
    /// Path to the Norito-encoded manifest (`.to` file).
    #[arg(long, value_name = "PATH")]
    pub manifest: PathBuf,
    /// Path to the raw payload bytes referenced by the manifest.
    #[arg(long, value_name = "PATH")]
    pub payload: PathBuf,
}

impl Run for StoragePinArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        self.run_with(context, |client, manifest, payload| {
            client.post_sorafs_storage_pin(manifest, payload)
        })
    }
}

impl StoragePinArgs {
    fn run_with<C, F>(&self, context: &mut C, submit: F) -> Result<()>
    where
        C: RunContext,
        F: FnOnce(&Client, &[u8], &[u8]) -> Result<Response<Vec<u8>>>,
    {
        let manifest_bytes = fs::read(&self.manifest).wrap_err("failed to read manifest file")?;
        let payload_bytes = fs::read(&self.payload).wrap_err("failed to read payload file")?;

        let client = context.client_from_config();
        let response = submit(&client, &manifest_bytes, &payload_bytes)?;
        let status = response.status();
        let body = response.into_body();
        match status {
            StatusCode::OK => render_json_body(context, &body),
            status => Err(make_http_error(status, &body)),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum StorageTokenCommand {
    /// Issue a stream token for a manifest/provider pair.
    Issue(StorageTokenIssueArgs),
}

impl Run for StorageTokenCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            StorageTokenCommand::Issue(args) => args.run(context),
        }
    }
}

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
        let nonce = self.nonce.clone().unwrap_or_else(|| generate_nonce_hex(12));
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

fn duration_to_time_delta(delta: Duration, label: &str) -> Result<TimeDelta> {
    let seconds = i64::try_from(delta.as_secs())
        .map_err(|_| eyre!("{label} exceeds supported duration range"))?;
    let nanos = delta.subsec_nanos();
    TimeDelta::seconds(seconds)
        .checked_add(TimeDelta::nanoseconds(i64::from(nanos)))
        .ok_or_else(|| eyre!("{label} overflowed while converting duration"))
}

fn add_datetime_duration(
    base: OffsetDateTime,
    delta: Duration,
    label: &str,
) -> Result<OffsetDateTime> {
    let step = duration_to_time_delta(delta, label)?;
    base.checked_add(step)
        .ok_or_else(|| eyre!("{label} overflowed while validating gateway denylist policy"))
}

fn format_datetime(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("RFC3339 formatting should be infallible")
}

fn generate_nonce_hex(bytes: usize) -> String {
    let mut rng = rand::rng();
    let mut data = vec![0u8; bytes];
    rng.fill_bytes(&mut data);
    hex::encode(data)
}

fn render_json_response<C: RunContext>(context: &mut C, response: Response<Vec<u8>>) -> Result<()> {
    let status = response.status();
    let body = response.into_body();
    match status {
        StatusCode::OK => render_json_body(context, &body),
        status => Err(make_http_error(status, &body)),
    }
}

fn render_json_body<C: RunContext>(context: &mut C, body: &[u8]) -> Result<()> {
    let value: norito::json::Value = norito::json::from_slice(body)?;
    context.print_data(&value)
}

fn make_http_error(status: StatusCode, body: &[u8]) -> eyre::Report {
    let message = String::from_utf8_lossy(body);
    eyre!("request failed with status {status}: {message}")
}

fn parse_xor_amount_decimal(input: &str) -> Result<XorAmount> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(eyre!("reserve balance must not be empty"));
    }
    if trimmed.starts_with('-') {
        return Err(eyre!("reserve balance must be non-negative"));
    }
    let mut parts = trimmed.split('.');
    let whole_part = parts.next().unwrap_or("");
    let fractional_part = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return Err(eyre!(
            "reserve balance may contain at most one decimal separator"
        ));
    }
    if whole_part.is_empty() && fractional_part.is_empty() {
        return Err(eyre!("reserve balance must contain digits"));
    }
    if !whole_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(eyre!("reserve balance contains invalid characters"));
    }
    if !fractional_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(eyre!("reserve balance fractional part is invalid"));
    }
    if fractional_part.len() > 6 {
        return Err(eyre!(
            "reserve balance supports up to six fractional digits (micro XOR precision)"
        ));
    }
    let whole_value = if whole_part.is_empty() {
        0
    } else {
        whole_part
            .parse::<u128>()
            .wrap_err("failed to parse whole-number component")?
    };
    let mut fractional_value = 0u128;
    let mut digits = 0;
    for ch in fractional_part.chars() {
        digits += 1;
        fractional_value = fractional_value * 10 + u128::from(ch as u8 - b'0');
    }
    if digits > 0 {
        for _ in digits..6 {
            fractional_value *= 10;
        }
    }
    let base = whole_value
        .checked_mul(MICRO_XOR_PER_XOR)
        .ok_or_else(|| eyre!("reserve balance exceeds supported range"))?;
    let total = base
        .checked_add(fractional_value)
        .ok_or_else(|| eyre!("reserve balance exceeds supported range"))?;
    Ok(XorAmount::from_micro(total))
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
    reserve_balance: XorAmount,
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
        norito::json::to_value(&reserve_balance).wrap_err("serialize reserve balance to JSON")?;
    inputs.insert("reserve_balance".into(), reserve_value);
    root.insert("inputs".into(), Value::Object(inputs));

    let policy_value =
        norito::json::to_value(policy).wrap_err("serialize reserve policy to JSON")?;
    root.insert("policy".into(), policy_value);
    let quote_value = norito::json::to_value(quote).wrap_err("serialize reserve quote to JSON")?;
    root.insert("quote".into(), quote_value);
    let projection_value = norito::json::to_value(&quote.ledger_projection())
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

#[derive(Clone, Copy)]
struct LedgerProjectionAmounts {
    rent_due: XorAmount,
    reserve_shortfall: XorAmount,
    top_up_shortfall: XorAmount,
}

fn extract_ledger_projection(value: &Value) -> Result<LedgerProjectionAmounts> {
    let root = value
        .as_object()
        .ok_or_else(|| eyre!("reserve quote must be a JSON object"))?;
    let ledger_value = root
        .get("ledger_projection")
        .ok_or_else(|| eyre!("reserve quote missing `ledger_projection` block"))?;

    if let Some(ledger) = ledger_value.as_object() {
        let rent_due = parse_optional_micro_amount(ledger, "rent_due_micro_xor")?;
        let reserve_shortfall = parse_optional_micro_amount(ledger, "reserve_shortfall_micro_xor")?;
        let top_up_shortfall = parse_optional_micro_amount(ledger, "top_up_shortfall_micro_xor")?
            .unwrap_or_else(XorAmount::zero);

        if let (Some(rent_due), Some(reserve_shortfall)) = (rent_due, reserve_shortfall) {
            return Ok(LedgerProjectionAmounts {
                rent_due,
                reserve_shortfall,
                top_up_shortfall,
            });
        }
    }

    let projection: ReserveLedgerProjection = norito::json::from_value(ledger_value.clone())
        .wrap_err("failed to parse reserve ledger projection from quote")?;
    Ok(LedgerProjectionAmounts {
        rent_due: projection.rent_due,
        reserve_shortfall: projection.reserve_shortfall,
        top_up_shortfall: projection.top_up_shortfall,
    })
}

fn parse_optional_micro_amount(map: &Map, key: &str) -> Result<Option<XorAmount>> {
    map.get(key).map_or_else(
        || Ok(None),
        |value| {
            value_to_micro(value, key)
                .map(XorAmount::from_micro)
                .map(Some)
        },
    )
}

fn value_to_micro(value: &Value, key: &str) -> Result<u128> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .map(u128::from)
            .ok_or_else(|| eyre!("`{key}` must fit into u64 micro XOR range")),
        Value::String(text) => text
            .parse::<u128>()
            .wrap_err_with(|| format!("failed to parse `{key}` as an unsigned integer")),
        _ => Err(eyre!(
            "`{key}` must be encoded as a JSON number or string (micro XOR)"
        )),
    }
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
        projection.rent_due,
        asset_definition,
    )?;
    append_transfer_instruction(
        &mut instructions,
        provider,
        reserve,
        projection.reserve_shortfall,
        asset_definition,
    )?;

    let mut root = Map::new();
    root.insert(
        "quote_path".into(),
        Value::from(quote_path.display().to_string()),
    );
    root.insert(
        "rent_due_micro_xor".into(),
        ledger_micro_value(projection.rent_due),
    );
    root.insert(
        "reserve_shortfall_micro_xor".into(),
        ledger_micro_value(projection.reserve_shortfall),
    );
    root.insert(
        "top_up_shortfall_micro_xor".into(),
        ledger_micro_value(projection.top_up_shortfall),
    );
    root.insert("instructions".into(), Value::Array(instructions));
    Ok(Value::Object(root))
}

fn append_transfer_instruction(
    instructions: &mut Vec<Value>,
    source_account: &AccountId,
    destination_account: &AccountId,
    amount: XorAmount,
    asset_definition: &AssetDefinitionId,
) -> Result<()> {
    if amount.is_zero() {
        return Ok(());
    }
    let numeric_amount = xor_amount_to_numeric(amount)?;
    let asset_id = AssetId::new(asset_definition.clone(), source_account.clone());
    let transfer = InstructionBox::from(Transfer::asset_numeric(
        asset_id,
        numeric_amount,
        destination_account.clone(),
    ));
    let value = norito::json::to_value(&transfer)
        .wrap_err("failed to serialize reserve ledger transfer instruction")?;
    instructions.push(value);
    Ok(())
}

fn xor_amount_to_numeric(amount: XorAmount) -> Result<Numeric> {
    Numeric::try_new(amount.as_micro(), 6)
        .map_err(|err| eyre!("failed to convert XOR amount to Numeric: {err}"))
}

fn ledger_micro_value(amount: XorAmount) -> Value {
    let micro = amount.as_micro();
    u64::try_from(micro).map_or_else(
        |_| Value::String(micro.to_string()),
        |value| Value::Number(Number::from(value)),
    )
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
    use iroha_crypto::{
        Algorithm, PublicKey,
        soranet::{
            certificate::{
                CapabilityToggle, KemRotationModeV1, KemRotationPolicyV1, RelayCapabilityFlagsV1,
                RelayCertificateV2, RelayEndpointV2, RelayRolesV2,
            },
            directory::{
                GuardDirectoryIssuerV1, GuardDirectoryRelayEntryV2, GuardDirectorySnapshotV2,
                compute_issuer_fingerprint,
            },
            handshake::HandshakeSuite,
            token::{self, AdmissionTokenVerifier},
        },
    };
    use iroha_data_model::account::{AccountAddress, address};
    use iroha_data_model::{
        asset::{AssetDefinitionId, AssetId},
        domain::DomainId,
        isi::{InstructionBox, TransferBox},
        soranet::incentives::{
            RelayBondLedgerEntryV1, RelayBondPolicyV1, RelayComplianceStatusV1,
            RelayEpochMetricsV1, RelayRewardDisputeV1, RelayRewardInstructionV1,
        },
    };
    use iroha_i18n::{Bundle, Language, Localizer};
    use iroha_primitives::numeric::Numeric;
    use norito::json::{Map, Value};
    use norito::{decode_from_bytes, json::JsonSerialize, to_bytes};
    use rand::{RngCore, SeedableRng, rngs::StdRng};
    use sorafs_manifest::{
        BLAKE3_256_MULTIHASH_CODE, ChunkingProfileV1, DagCodecId, ManifestBuilder, PinPolicy,
        ProfileId, StorageClass as ManifestStorageClass,
    };
    use sorafs_orchestrator::soranet::EndpointTag;
    use sorafs_orchestrator::{incentives::RewardConfig, treasury::ExpectedLedgerTransfer};
    use soranet_pq::{MlDsaSuite, generate_mldsa_keypair};
    use std::{
        fmt::Display,
        fs,
        io::Write,
        path::Path,
        str::FromStr,
        time::{Duration, SystemTime},
    };
    use tempfile::{NamedTempFile, TempDir};
    use url::Url;

    fn sample_account_literals() -> (String, String, String) {
        let domain: DomainId = "default".parse().expect("domain parses");
        let public_key: PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("public key parses");
        let account = AccountId::new(domain, public_key);
        let address = AccountAddress::from_account_id(&account).expect("address from account");
        let canonical = address.canonical_hex().expect("canonical hex");
        let ih58 = address
            .to_ih58(address::chain_discriminant())
            .expect("ih58 encode");
        let compressed = address.to_compressed_sora().expect("compressed encode");
        (canonical, ih58, compressed)
    }

    fn resolve_account_literal(literal: &str) -> Result<AccountId> {
        AccountId::parse_encoded(literal)
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .wrap_err_with(|| format!("invalid test account literal `{literal}`"))
    }

    #[test]
    fn parse_xor_amount_decimal_handles_fractional_inputs() {
        let amount = parse_xor_amount_decimal("12.3456").expect("parse succeeds");
        assert_eq!(amount.as_micro(), 12_345_600);
    }

    #[test]
    fn parse_xor_amount_decimal_rejects_excess_precision() {
        let err = parse_xor_amount_decimal("0.1234567").expect_err("expected failure");
        assert!(
            err.to_string()
                .contains("supports up to six fractional digits"),
            "{err:?}"
        );
    }

    #[test]
    fn reserve_quote_builder_renders_inputs() {
        let policy = ReservePolicyV1::default();
        let quote = policy
            .quote(
                super::StorageClass::Hot,
                4,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                XorAmount::zero(),
            )
            .expect("quote");
        let value = build_reserve_quote_value(
            &policy,
            super::StorageClass::Hot,
            ReserveTier::TierA,
            ReserveDuration::Monthly,
            4,
            XorAmount::zero(),
            &quote,
            "test policy",
        )
        .expect("build");
        let root = value
            .as_object()
            .expect("quote payload should be a JSON object");
        assert_eq!(
            root.get("policy_source").and_then(Value::as_str),
            Some("test policy")
        );
        let inputs = root
            .get("inputs")
            .and_then(Value::as_object)
            .expect("inputs object");
        assert_eq!(
            inputs.get("storage_class").and_then(Value::as_str),
            Some("hot")
        );
        assert_eq!(inputs.get("capacity_gib").and_then(Value::as_u64), Some(4));
        let quote_value = root.get("quote").expect("quote field exists");
        assert!(
            quote_value.get("monthly_rent").is_some(),
            "quote field should carry rent breakdown: {quote_value:?}"
        );
        let ledger_projection = root
            .get("ledger_projection")
            .and_then(Value::as_object)
            .expect("ledger projection should be serialized");
        assert!(
            ledger_projection.contains_key("rent_due"),
            "ledger projection exposes rent_due amount: {ledger_projection:?}"
        );
    }

    fn denylist_record_for_account(value: &str) -> GatewayDenylistRecord {
        GatewayDenylistRecord {
            kind: "account_id".to_owned(),
            provider_id_hex: None,
            manifest_digest_hex: None,
            cid_b64: None,
            cid_hex: None,
            cid_utf8: None,
            url: None,
            account_id: Some(value.to_owned()),
            account_alias: None,
            jurisdiction: None,
            reason: None,
            alias: None,
            issued_at: Some("2026-01-01T00:00:00Z".to_owned()),
            expires_at: Some("2026-04-01T00:00:00Z".to_owned()),
            family_id_hex: None,
            variant_id_hex: None,
            attack_vector: None,
            perceptual_hash_hex: None,
            perceptual_hamming_radius: None,
            policy_tier: Some("standard".to_owned()),
            emergency_canon: None,
            governance_reference: None,
        }
    }

    fn sample_guard_directory_snapshot_bytes() -> Vec<u8> {
        let mut rng = StdRng::seed_from_u64(0x5EED);
        let mut ed_seed = [0u8; 32];
        rng.fill_bytes(&mut ed_seed);
        let signing_key = SigningKey::from_bytes(&ed_seed);
        let ed_public = Ed25519VerifyingKey::from(&signing_key).to_bytes();

        let mldsa_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");
        let mldsa_public = mldsa_keys.public_key().to_vec();
        let fingerprint = compute_issuer_fingerprint(&ed_public, &mldsa_public);

        let directory_hash = [0xAB; 32];
        let certificate = RelayCertificateV2 {
            relay_id: [0x11; 32],
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
                url: "soranet://pq.guard".to_string(),
                priority: 0,
                tags: vec![EndpointTag::NoritoStream.as_label().to_string()],
            }],
            capability_flags: RelayCapabilityFlagsV1::new(
                CapabilityToggle::Enabled,
                CapabilityToggle::Disabled,
                CapabilityToggle::Enabled,
                CapabilityToggle::Disabled,
            ),
            kem_policy: KemRotationPolicyV1 {
                mode: KemRotationModeV1::Static,
                preferred_suite: 2,
                fallback_suite: None,
                rotation_interval_hours: 0,
                grace_period_hours: 0,
            },
            handshake_suites: vec![
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            published_at: 1_734_000_000,
            valid_after: 1_734_000_000,
            valid_until: 1_734_086_400,
            directory_hash,
            issuer_fingerprint: fingerprint,
            pq_kem_public: vec![0x55; ML_KEM_768_PUBLIC_LEN],
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
            validation_phase: 3,
            issuers: vec![GuardDirectoryIssuerV1 {
                fingerprint,
                ed25519_public: ed_public,
                mldsa65_public: mldsa_public,
            }],
            relays: vec![GuardDirectoryRelayEntryV2 {
                certificate: bundle.to_cbor(),
            }],
        };

        to_bytes(&snapshot).expect("encode snapshot")
    }

    #[test]
    fn gateway_denylist_record_accepts_ih58_literals() {
        let (_, ih58, _) = sample_account_literals();
        let record = denylist_record_for_account(&ih58);
        record
            .validate(&resolve_account_literal)
            .expect("ih58 literal accepted");
    }

    #[test]
    fn gateway_denylist_record_accepts_compressed_literals() {
        let (_, _, compressed) = sample_account_literals();
        let record = denylist_record_for_account(&compressed);
        record
            .validate(&resolve_account_literal)
            .expect("compressed literal accepted");
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
            let kp = KeyPair::random();
            let domain: DomainId = "wonderland".parse().expect("domain");
            let account = AccountId::new(domain, kp.public_key().clone());
            let cfg = Config {
                chain: ChainId::from("test-chain"),
                account,
                key_pair: kp,
                basic_auth: None,
                torii_api_url: Url::parse("http://localhost/").unwrap(),
                torii_api_version: config::default_torii_api_version(),
                torii_api_min_proof_version: config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                    .to_string(),
                torii_request_timeout: config::DEFAULT_TORII_REQUEST_TIMEOUT,
                transaction_ttl: config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
                transaction_status_timeout: config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
                transaction_add_nonce: config::DEFAULT_TRANSACTION_NONCE,
                connect_queue_root: config::default_connect_queue_root(),
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

    #[test]
    fn output_summary_prefers_json_in_json_mode() {
        let mut ctx = OutputModeContext::new(CliOutputFormat::Json);
        let summary = DaemonIterationSummary::default();
        output_summary(&mut ctx, &summary, false).expect("summary output");
        assert_eq!(ctx.printed, vec!["json"]);
    }

    #[test]
    fn output_summary_uses_text_in_text_mode() {
        let mut ctx = OutputModeContext::new(CliOutputFormat::Text);
        let summary = DaemonIterationSummary::default();
        output_summary(&mut ctx, &summary, false).expect("summary output");
        assert_eq!(ctx.printed, vec!["text"]);
    }

    #[test]
    fn log_daemon_summary_emits_json_in_json_mode() {
        let mut ctx = OutputModeContext::new(CliOutputFormat::Json);
        let summary = DaemonIterationSummary::default();
        log_daemon_summary(&mut ctx, &summary, false).expect("daemon summary");
        assert_eq!(ctx.printed, vec!["json"]);
    }

    #[test]
    fn handshake_update_requires_flags() {
        let result = HandshakeUpdateArgs::default().into_update();
        assert!(result.is_err(), "expected at least one override");
    }

    #[test]
    fn handshake_update_accepts_pow_overrides() {
        let args = HandshakeUpdateArgs {
            descriptor_commit: Some("aa".into()),
            pow_optional: true,
            pow_difficulty: Some(7),
            pow_max_future_skew: Some(120),
            ..Default::default()
        };
        let update = args.into_update().expect("update should succeed");
        assert_eq!(update.descriptor_commit_hex.as_deref(), Some("aa"));
        let pow = update.pow.expect("pow overrides present");
        assert_eq!(pow.required, Some(false));
        assert_eq!(pow.difficulty, Some(7));
        assert_eq!(pow.max_future_skew_secs, Some(120));
        assert!(pow.min_ticket_ttl_secs.is_none());
        assert!(pow.ticket_ttl_secs.is_none());
    }

    #[test]
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

    fn sample_reward_config_json() -> norito::json::Value {
        let mut policy = norito::json::Map::new();
        policy.insert(
            "minimum_exit_bond".to_string(),
            norito::json::Value::String("1000".to_string()),
        );
        policy.insert(
            "bond_asset_id".to_string(),
            norito::json::Value::String("xor#sora".to_string()),
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
            bonded_amount: Numeric::from(amount),
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
            payout_amount: Numeric::from(42_u32),
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
            amount: Numeric::from(amount),
            source_asset: AssetId::new(xor_asset_id(), sample_account_id("treasury")),
            destination: sample_account_id("relay"),
        }
    }

    #[test]
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
        assert!(
            combined.contains("schema mismatch"),
            "expected schema mismatch in error chain: {combined}"
        );
        assert!(
            combined.contains("expected"),
            "expected schema hash detail in error chain: {combined}"
        );
        assert!(
            combined.contains("got"),
            "expected actual schema hash detail in error chain: {combined}"
        );
    }

    #[test]
    fn reconciliation_summary_builds_expected_counts() {
        let missing_record = sample_transfer_record(TransferKind::Payout, 100);
        let unexpected_record = sample_transfer_record(TransferKind::Credit, 25);
        let mismatch_expected = sample_transfer_record(TransferKind::Debit, 40);
        let mut mismatch_actual = sample_transfer_record(TransferKind::Debit, 35);
        mismatch_actual.destination = sample_account_id("alt-treasury");

        let report = LedgerReconciliationReport {
            total_expected_transfers: 3,
            matched_transfers: 1,
            expected_amount_nanos: 150_000_000_000,
            exported_amount_nanos: 70_000_000_000,
            missing_transfers: vec![ExpectedLedgerTransfer {
                record: missing_record.clone(),
            }],
            unexpected_transfers: vec![unexpected_record.clone()],
            mismatched_transfers: vec![LedgerTransferMismatch {
                expected: mismatch_expected.clone(),
                actual: mismatch_actual.clone(),
                reasons: vec![MismatchReason::Amount, MismatchReason::Destination],
            }],
        };

        let summary = ReconciliationReportSummary::from_report(&report);
        assert!(!summary.clean);
        assert_eq!(summary.matched_transfers, 1);
        assert_eq!(summary.total_expected_transfers, 3);
        assert_eq!(summary.missing_transfers.len(), 1);
        assert_eq!(summary.unexpected_transfers.len(), 1);
        assert_eq!(summary.mismatched_transfers.len(), 1);
        assert_eq!(
            summary.missing_transfers[0].relay_id,
            relay_id_to_hex(missing_record.relay_id)
        );
        assert_eq!(
            summary.unexpected_transfers[0].kind,
            transfer_kind_label(unexpected_record.kind)
        );
        assert!(
            summary.mismatched_transfers[0]
                .reasons
                .iter()
                .any(|reason| reason == "amount")
        );
    }

    fn sample_account_id(name: &str) -> AccountId {
        let domain = DomainId::from_str("default").expect("domain id");
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
        AccountId::new(domain, public_key)
    }

    fn sample_account_literal(name: &str) -> String {
        let account = sample_account_id(name);
        account.to_string()
    }

    fn xor_asset_id() -> AssetDefinitionId {
        AssetDefinitionId::from_str("xor#sora").expect("asset id")
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

    #[test]
    fn handshake_token_issue_generates_verifiable_token() {
        let mut ctx = TestContext::new();
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keypair");
        let secret_hex = hex::encode(keypair.secret_key());
        let public_hex = hex::encode(keypair.public_key());

        let args = HandshakeTokenIssueArgs {
            suite: MlDsaSuiteArg::MlDsa44,
            issuer_secret_key: None,
            issuer_secret_hex: Some(secret_hex),
            issuer_public_key: None,
            issuer_public_hex: Some(public_hex.clone()),
            relay_id: "11".repeat(32),
            transcript_hash: "22".repeat(32),
            issued_at: Some("2026-01-01T00:00:00Z".to_string()),
            expires_at: None,
            ttl_secs: Some(900),
            flags: Some(0),
            output: None,
            token_encoding: TokenOutputFormat::Base64,
        };

        let mut rng = StdRng::seed_from_u64(0x5eed);
        let default_now = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let artifacts = args
            .issue_with_rng(&mut ctx, &mut rng, default_now)
            .expect("issue token");

        let verifier = AdmissionTokenVerifier::new(
            MlDsaSuite::MlDsa44,
            keypair.public_key().to_vec(),
            Duration::from_secs(900),
            Duration::from_secs(5),
        );
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

        HandshakeTokenIssueArgs::emit(&mut ctx, &artifacts, None, TokenOutputFormat::Base64)
            .expect("emit output");
        let output = ctx.outputs().last().expect("json output present");
        let json: Value = norito::json::from_str(output).expect("valid json");
        assert_eq!(json["flags"], Value::from(0u64));
        assert_eq!(
            json["token_id_hex"],
            Value::from(hex::encode(artifacts.token.token_id()))
        );
    }

    #[test]
    fn handshake_token_id_reports_expected_digest() {
        let mut ctx = TestContext::new();
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keypair");
        let secret_hex = hex::encode(keypair.secret_key());
        let public_hex = hex::encode(keypair.public_key());

        let args = HandshakeTokenIssueArgs {
            suite: MlDsaSuiteArg::MlDsa44,
            issuer_secret_key: None,
            issuer_secret_hex: Some(secret_hex),
            issuer_public_key: None,
            issuer_public_hex: Some(public_hex),
            relay_id: "33".repeat(32),
            transcript_hash: "44".repeat(32),
            issued_at: Some("2026-02-01T00:00:00Z".to_string()),
            expires_at: None,
            ttl_secs: Some(600),
            flags: None,
            output: None,
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
        let token_hex = hex::encode(&artifacts.token_bytes);

        let id_args = HandshakeTokenIdArgs {
            path: None,
            hex_input: Some(token_hex),
            base64_input: None,
        };
        id_args.run(&mut ctx).expect("compute id");

        let output = ctx.outputs().last().expect("json output");
        let json: Value = norito::json::from_str(output).expect("valid json");
        assert_eq!(
            json["token_id_hex"],
            Value::from(hex::encode(artifacts.token.token_id()))
        );
    }

    #[test]
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
        assert_eq!(
            json["issuer_fingerprint_hex"],
            Value::from(hex::encode(expected))
        );
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

    #[test]
    fn gateway_provider_spec_parses_expected_keys() {
        let id_hex = "11".repeat(32);
        let spec = format!(
            "name=alpha, provider-id={id_hex}, base-url=https://example.com, stream-token=YWJj"
        );
        let parsed = parse_gateway_provider_spec(&spec).expect("parse spec");
        assert_eq!(parsed.name, "alpha");
        assert_eq!(parsed.provider_id_hex, id_hex);
        assert_eq!(parsed.base_url, "https://example.com");
        assert_eq!(parsed.stream_token_b64, "YWJj");
    }

    #[test]
    fn gateway_provider_spec_rejects_missing_fields() {
        let err = parse_gateway_provider_spec("name=alpha, base-url=https://example.com")
            .expect_err("missing provider-id should fail");
        assert!(
            err.to_string().contains("provider-id"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_hex_digest_enforces_format() {
        let valid = validate_hex_digest(&"ab".repeat(32), "--flag").expect("valid digest");
        assert_eq!(valid, "ab".repeat(32));

        let err = validate_hex_digest("zz", "--flag").expect_err("invalid digest");
        assert!(err.to_string().contains("--flag"));
    }

    #[test]
    fn parse_transport_policy_flag_accepts_valid_value() {
        let value = "soranet-strict".to_string();
        let parsed = parse_transport_policy_flag(Some(&value), "--transport-policy-override")
            .expect("parse transport policy");
        assert_eq!(parsed, Some(TransportPolicy::SoranetStrict));
    }

    #[test]
    fn parse_transport_policy_flag_rejects_blank_input() {
        let value = "   ".to_string();
        let err =
            parse_transport_policy_flag(Some(&value), "--transport-policy").expect_err("blank");
        assert!(
            err.to_string().contains("must not be empty"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn parse_anonymity_policy_flag_accepts_stage_alias() {
        let value = "stage-b".to_string();
        let parsed = parse_anonymity_policy_flag(Some(&value), "--anonymity-policy-override")
            .expect("parse anonymity policy");
        assert_eq!(parsed, Some(AnonymityPolicy::MajorityPq));
    }

    #[test]
    fn parse_anonymity_policy_flag_rejects_unknown() {
        let value = "anon-unknown".to_string();
        let err = parse_anonymity_policy_flag(Some(&value), "--anonymity-policy")
            .expect_err("unsupported anonymity policy must be rejected");
        assert!(
            err.to_string().contains("anon-guard-pq"),
            "error should point to the canonical staged policies: {err}"
        );
    }

    #[test]
    fn anonymity_policy_label_matches_expected_values() {
        assert_eq!(
            anonymity_policy_label(AnonymityPolicy::GuardPq),
            "anon-guard-pq"
        );
        assert_eq!(
            anonymity_policy_label(AnonymityPolicy::MajorityPq),
            "anon-majority-pq"
        );
        assert_eq!(
            anonymity_policy_label(AnonymityPolicy::StrictPq),
            "anon-strict-pq"
        );
    }

    #[test]
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
        let err = load_guard_directory(file.path()).expect_err("json format must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("failed to parse guard directory"),
            "unexpected error message: {msg}"
        );
        assert!(
            msg.contains("SRCv2"),
            "error should mention the canonical SRCv2 Norito format: {msg}"
        );
    }

    #[test]
    fn load_guard_directory_decodes_srcv2_bundle() {
        let bytes = sample_guard_directory_snapshot_bytes();
        let mut file = NamedTempFile::new().expect("temp file");
        file.write_all(&bytes).expect("write snapshot");

        let directory = load_guard_directory(file.path()).expect("load directory");
        let entries = directory.entries();
        assert_eq!(entries.len(), 1);
        let descriptor = &entries[0];
        assert_eq!(descriptor.relay_id, [0x11; 32]);
        assert!(descriptor.is_pq_capable());
        assert!(descriptor.certificate().is_some());
        assert_eq!(
            descriptor.certificate_validity(),
            directory.valid_after().zip(directory.valid_until())
        );
        assert_eq!(directory.valid_after(), Some(1_734_000_000));
        assert_eq!(directory.valid_until(), Some(1_734_086_400));
    }

    #[test]
    fn guard_directory_summary_reports_expected_counts() {
        let bytes = sample_guard_directory_snapshot_bytes();
        let summary = inspect_guard_directory_bytes(&bytes).expect("inspect directory");
        assert_eq!(summary.version, 2);
        assert_eq!(summary.issuer_count, 1);
        assert_eq!(summary.relay_count, 1);
        assert_eq!(summary.entry_guards, 1);
        assert_eq!(summary.entry_guards_pq, 1);
        assert_eq!(summary.exit_relays, 0);
        assert_eq!(summary.dual_signed_relays, 1);
        let expected_hash = "ab".repeat(32);
        assert_eq!(
            summary.directory_hash_hex.as_deref(),
            Some(expected_hash.as_str())
        );
        assert!(summary.entry_guard_pq_ratio > 0.99);
    }

    #[test]
    fn ensure_expected_directory_hash_accepts_matching_hex() {
        let bytes = sample_guard_directory_snapshot_bytes();
        let summary = inspect_guard_directory_bytes(&bytes).expect("inspect directory");
        let expected = summary.directory_hash_hex.as_deref().unwrap();
        ensure_expected_directory_hash(&summary, Some(expected)).expect("hash should match");
    }

    #[test]
    fn ensure_expected_directory_hash_rejects_mismatch() {
        let bytes = sample_guard_directory_snapshot_bytes();
        let summary = inspect_guard_directory_bytes(&bytes).expect("inspect directory");
        let result = ensure_expected_directory_hash(&summary, Some("deadbeef"));
        assert!(result.is_err(), "hash mismatch should fail");
    }

    #[test]
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

    #[test]
    fn pin_list_with_prints_payload() {
        let args = PinListArgs {
            status: Some("approved".to_string()),
            limit: Some(5),
            offset: Some(10),
        };
        let mut ctx = TestContext::new();

        args.run_with(&mut ctx, |_client, filter| {
            assert_eq!(filter.status, Some("approved"));
            assert_eq!(filter.limit, Some(5));
            assert_eq!(filter.offset, Some(10));
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(norito::json::to_vec(&norito::json!({
                    "manifests": [ { "digest": "aa" } ]
                }))?)
                .unwrap())
        })
        .expect("run should succeed");

        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"manifests\""));
    }

    #[test]
    fn pin_list_with_propagates_error_status() {
        let args = PinListArgs {
            status: None,
            limit: None,
            offset: None,
        };
        let mut ctx = TestContext::new();

        let result = args.run_with(&mut ctx, |_client, _| {
            Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "application/json")
                .body(norito::json::to_vec(
                    &norito::json!({ "error": "bad request" }),
                )?)
                .unwrap())
        });

        assert!(result.is_err());
        assert!(ctx.printed.is_empty());
    }

    #[test]
    fn pin_show_with_handles_not_found() {
        let args = PinShowArgs {
            digest: "deadbeef".to_string(),
        };
        let mut ctx = TestContext::new();

        args.run_with(&mut ctx, |_client, digest| {
            assert_eq!(digest, "deadbeef");
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "application/json")
                .body(norito::json::to_vec(
                    &norito::json!({ "error": "missing" }),
                )?)
                .unwrap())
        })
        .expect("run should succeed for 404");

        assert_eq!(
            ctx.printed,
            vec!["manifest `deadbeef` not found".to_string()]
        );
    }

    #[test]
    fn alias_list_with_prints_payload() {
        let args = AliasListArgs {
            limit: Some(3),
            offset: Some(0),
            namespace: Some("docs".to_string()),
            manifest_digest: Some("aa".to_string()),
        };
        let mut ctx = TestContext::new();

        args.run_with(&mut ctx, |_client, filter| {
            assert_eq!(filter.limit, Some(3));
            assert_eq!(filter.namespace, Some("docs"));
            assert_eq!(filter.manifest_digest, Some("aa"));
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(norito::json::to_vec(&norito::json!({
                    "aliases": [
                        { "alias": "docs/latest", "digest": "aa" }
                    ]
                }))?)
                .unwrap())
        })
        .expect("run should succeed");

        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"aliases\""));
    }

    #[test]
    fn replication_list_with_prints_payload() {
        let args = ReplicationListArgs {
            limit: Some(2),
            offset: None,
            status: Some("completed".to_string()),
            manifest_digest: None,
        };
        let mut ctx = TestContext::new();

        args.run_with(&mut ctx, |_client, filter| {
            assert_eq!(filter.status, Some("completed"));
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(norito::json::to_vec(&norito::json!({
                    "orders": [
                        { "id": "order1", "status": "completed" }
                    ]
                }))?)
                .unwrap())
        })
        .expect("run should succeed");

        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"orders\""));
    }

    #[test]
    fn repair_ticket_id_rejects_lowercase() {
        let result = parse_repair_ticket_id("rep-1", "--ticket-id");
        assert!(result.is_err(), "lowercase ticket id should fail");
    }

    #[test]
    fn repair_list_scopes_manifest_and_filters() {
        let args = RepairListArgs {
            manifest_digest: Some(format!("0x{}", "AB".repeat(32))),
            status: Some("queued".to_string()),
            provider_id: Some("FF".repeat(32)),
        };
        let mut ctx = TestContext::new();
        let expected_provider = "ff".repeat(32);

        args.run_with(
            &mut ctx,
            |_client, _| unreachable!("list_all should not be called"),
            |_client, digest, filter| {
                assert_eq!(digest, &"ab".repeat(32));
                assert_eq!(filter.status, Some("queued"));
                assert_eq!(filter.provider_id, Some(expected_provider.as_str()));
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(norito::json::to_vec(&norito::json!({
                        "tasks": [ { "ticket_id": "REP-1" } ]
                    }))?)
                    .unwrap())
            },
        )
        .expect("run should succeed");

        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"tasks\""));
    }

    #[test]
    fn repair_claim_builds_signed_request() {
        let manifest_digest = [0x11_u8; 32];
        let provider_id = [0x22_u8; 32];
        let args = RepairClaimArgs {
            ticket_id: "REP-501".to_string(),
            manifest_digest: encode(manifest_digest),
            provider_id: encode(provider_id),
            claimed_at: Some("@1700000501".to_string()),
            idempotency_key: Some("claim-501".to_string()),
        };
        let mut ctx = TestContext::new();
        let expected_worker_id = ctx.config().account.to_string();
        let public_key = ctx.config().key_pair.public_key().clone();

        args.run_with(&mut ctx, |_client, request| {
            assert_eq!(request.ticket_id.0, "REP-501");
            assert_eq!(request.manifest_digest_hex, encode(manifest_digest));
            assert_eq!(request.worker_id, expected_worker_id);
            assert_eq!(request.idempotency_key, "claim-501");
            assert_eq!(request.claimed_at_unix, 1_700_000_501);
            let payload = RepairWorkerSignaturePayloadV1 {
                version: REPAIR_WORKER_SIGNATURE_VERSION_V1,
                ticket_id: request.ticket_id.clone(),
                manifest_digest,
                provider_id,
                worker_id: request.worker_id.clone(),
                idempotency_key: request.idempotency_key.clone(),
                action: RepairWorkerActionV1::Claim {
                    claimed_at_unix: request.claimed_at_unix,
                },
            };
            request
                .signature
                .verify(&public_key, &payload)
                .expect("signature should verify");
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(norito::json::to_vec(&norito::json!({ "ok": true }))?)
                .unwrap())
        })
        .expect("run should succeed");

        assert_eq!(ctx.printed.len(), 1);
    }

    #[test]
    fn repair_complete_builds_signed_request() {
        let manifest_digest = [0x33_u8; 32];
        let provider_id = [0x44_u8; 32];
        let args = RepairCompleteArgs {
            ticket_id: "REP-502".to_string(),
            manifest_digest: encode(manifest_digest),
            provider_id: encode(provider_id),
            completed_at: Some("@1700000502".to_string()),
            resolution_notes: Some("patched".to_string()),
            idempotency_key: Some("complete-502".to_string()),
        };
        let mut ctx = TestContext::new();
        let expected_worker_id = ctx.config().account.to_string();
        let public_key = ctx.config().key_pair.public_key().clone();

        args.run_with(&mut ctx, |_client, request| {
            assert_eq!(request.ticket_id.0, "REP-502");
            assert_eq!(request.manifest_digest_hex, encode(manifest_digest));
            assert_eq!(request.worker_id, expected_worker_id);
            assert_eq!(request.idempotency_key, "complete-502");
            assert_eq!(request.completed_at_unix, 1_700_000_502);
            let payload = RepairWorkerSignaturePayloadV1 {
                version: REPAIR_WORKER_SIGNATURE_VERSION_V1,
                ticket_id: request.ticket_id.clone(),
                manifest_digest,
                provider_id,
                worker_id: request.worker_id.clone(),
                idempotency_key: request.idempotency_key.clone(),
                action: RepairWorkerActionV1::Complete {
                    completed_at_unix: request.completed_at_unix,
                    resolution_notes: request.resolution_notes.clone(),
                },
            };
            request
                .signature
                .verify(&public_key, &payload)
                .expect("signature should verify");
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(norito::json::to_vec(&norito::json!({ "ok": true }))?)
                .unwrap())
        })
        .expect("run should succeed");

        assert_eq!(ctx.printed.len(), 1);
    }

    #[test]
    fn repair_fail_builds_signed_request() {
        let manifest_digest = [0x55_u8; 32];
        let provider_id = [0x66_u8; 32];
        let args = RepairFailArgs {
            ticket_id: "REP-503".to_string(),
            manifest_digest: encode(manifest_digest),
            provider_id: encode(provider_id),
            failed_at: Some("@1700000503".to_string()),
            reason: "checksum_mismatch".to_string(),
            idempotency_key: Some("fail-503".to_string()),
        };
        let mut ctx = TestContext::new();
        let expected_worker_id = ctx.config().account.to_string();
        let public_key = ctx.config().key_pair.public_key().clone();

        args.run_with(&mut ctx, |_client, request| {
            assert_eq!(request.ticket_id.0, "REP-503");
            assert_eq!(request.manifest_digest_hex, encode(manifest_digest));
            assert_eq!(request.worker_id, expected_worker_id);
            assert_eq!(request.idempotency_key, "fail-503");
            assert_eq!(request.failed_at_unix, 1_700_000_503);
            let payload = RepairWorkerSignaturePayloadV1 {
                version: REPAIR_WORKER_SIGNATURE_VERSION_V1,
                ticket_id: request.ticket_id.clone(),
                manifest_digest,
                provider_id,
                worker_id: request.worker_id.clone(),
                idempotency_key: request.idempotency_key.clone(),
                action: RepairWorkerActionV1::Fail {
                    failed_at_unix: request.failed_at_unix,
                    reason: request.reason.clone(),
                },
            };
            request
                .signature
                .verify(&public_key, &payload)
                .expect("signature should verify");
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(norito::json::to_vec(&norito::json!({ "ok": true }))?)
                .unwrap())
        })
        .expect("run should succeed");

        assert_eq!(ctx.printed.len(), 1);
    }

    #[test]
    fn repair_escalate_builds_slash_proposal() {
        let manifest_digest = [0x77_u8; 32];
        let provider_id = [0x88_u8; 32];
        let args = RepairEscalateArgs {
            ticket_id: "REP-504".to_string(),
            manifest_digest: encode(manifest_digest),
            provider_id: encode(provider_id),
            penalty_nano: 900,
            rationale: "sla_missed".to_string(),
            auditor: None,
            submitted_at: Some("@1700000504".to_string()),
            approve_votes: Some(2),
            reject_votes: Some(1),
            abstain_votes: Some(0),
            approved_at: Some("@1700000600".to_string()),
            finalized_at: Some("@1700000700".to_string()),
        };
        let mut ctx = TestContext::new();
        let expected_auditor = ctx.config().account.to_string();

        args.run_with(&mut ctx, |_client, proposal| {
            assert_eq!(proposal.ticket_id.0, "REP-504");
            assert_eq!(proposal.provider_id, provider_id);
            assert_eq!(proposal.manifest_digest, manifest_digest);
            assert_eq!(proposal.auditor_account, expected_auditor);
            assert_eq!(proposal.proposed_penalty_nano, 900);
            assert_eq!(proposal.submitted_at_unix, 1_700_000_504);
            let approval = proposal.approval.as_ref().expect("approval");
            assert_eq!(approval.approve_votes, 2);
            assert_eq!(approval.reject_votes, 1);
            assert_eq!(approval.abstain_votes, 0);
            assert_eq!(approval.approved_at_unix, 1_700_000_600);
            assert_eq!(approval.finalized_at_unix, 1_700_000_700);
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(norito::json::to_vec(&norito::json!({ "ok": true }))?)
                .unwrap())
        })
        .expect("run should succeed");

        assert_eq!(ctx.printed.len(), 1);
    }

    #[test]
    fn repair_escalate_allows_missing_approval_summary() {
        let manifest_digest = [0x79_u8; 32];
        let provider_id = [0x81_u8; 32];
        let args = RepairEscalateArgs {
            ticket_id: "REP-505".to_string(),
            manifest_digest: encode(manifest_digest),
            provider_id: encode(provider_id),
            penalty_nano: 1_200,
            rationale: "missing-approval".to_string(),
            auditor: None,
            submitted_at: Some("@1700000605".to_string()),
            approve_votes: None,
            reject_votes: None,
            abstain_votes: None,
            approved_at: None,
            finalized_at: None,
        };
        let mut ctx = TestContext::new();
        let expected_auditor = ctx.config().account.to_string();

        args.run_with(&mut ctx, |_client, proposal| {
            assert_eq!(proposal.ticket_id.0, "REP-505");
            assert_eq!(proposal.provider_id, provider_id);
            assert_eq!(proposal.manifest_digest, manifest_digest);
            assert_eq!(proposal.auditor_account, expected_auditor);
            assert_eq!(proposal.proposed_penalty_nano, 1_200);
            assert_eq!(proposal.submitted_at_unix, 1_700_000_605);
            assert!(proposal.approval.is_none());
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(norito::json::to_vec(&norito::json!({ "ok": true }))?)
                .unwrap())
        })
        .expect("run should succeed");

        assert_eq!(ctx.printed.len(), 1);
    }

    #[test]
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

    #[test]
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

    #[test]
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

    #[test]
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

    #[test]
    fn gc_manifest_entries_require_manifest_dir() {
        let dir = TempDir::new().expect("temp dir");
        let err = load_gc_manifest_entries(dir.path()).expect_err("missing manifests");
        assert!(
            err.to_string().contains("SoraFS manifests directory"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn gc_retention_deadline_respects_zero_epoch() {
        assert_eq!(retention_deadline(0, 5), None);
        assert_eq!(retention_deadline(10, 5), Some(15));
    }

    #[test]
    fn gc_storage_class_labels_match_expected_values() {
        assert_eq!(
            manifest_storage_class_label(ManifestStorageClass::Hot),
            "hot"
        );
        assert_eq!(
            manifest_storage_class_label(ManifestStorageClass::Warm),
            "warm"
        );
        assert_eq!(
            manifest_storage_class_label(ManifestStorageClass::Cold),
            "cold"
        );
    }

    #[test]
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
        assert!(
            ctx.printed[0].starts_with("nonce: "),
            "expected nonce println, got {}",
            ctx.printed[0]
        );
        assert_eq!(
            ctx.printed[1], "{\"token\":{\"body\":{}}}",
            "expected JSON payload output"
        );
        assert_eq!(
            nonce.len(),
            24,
            "nonce should be 12 random bytes hex encoded"
        );
    }

    #[test]
    fn storage_pin_with_reads_files_and_prints_payload() {
        let manifest = NamedTempFile::new().expect("temp manifest");
        let payload = NamedTempFile::new().expect("temp payload");
        std::fs::write(manifest.path(), b"manifest-bytes").unwrap();
        std::fs::write(payload.path(), b"payload-bytes").unwrap();

        let args = StoragePinArgs {
            manifest: manifest.path().to_path_buf(),
            payload: payload.path().to_path_buf(),
        };
        let mut ctx = TestContext::new();

        args.run_with(&mut ctx, |_client, manifest_bytes, payload_bytes| {
            assert_eq!(manifest_bytes, b"manifest-bytes");
            assert_eq!(payload_bytes, b"payload-bytes");
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(norito::json::to_vec(&norito::json!({ "ok": true }))?)
                .unwrap())
        })
        .expect("run should succeed");

        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"ok\":true"));
    }

    #[test]
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
        assert!(
            plan.direct_car.canonical_url.contains("/direct/v1/car/"),
            "direct car locator should reference the manifest digest"
        );
        assert!(plan.capabilities.direct_car_supported);
    }

    #[test]
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
        let car_size = fs::metadata(&car_path).expect("car metadata").len();
        assert_eq!(manifest.car_size, car_size);

        let report_bytes = fs::read(&json_path).expect("read report");
        let report: Value = norito::json::from_slice(&report_bytes).expect("decode report");
        let digest_hex = hex::encode(manifest.digest().expect("manifest digest").as_bytes());
        assert_eq!(
            report.get("manifest_digest_hex").and_then(Value::as_str),
            Some(digest_hex.as_str())
        );
    }

    #[test]
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

    #[test]
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

    #[test]
    fn ensure_metadata_entry_dedupes_case_insensitive() {
        let mut metadata = vec![("manifest.requires_envelope".to_string(), "true".to_string())];
        ensure_metadata_entry(&mut metadata, "Manifest.Requires_Envelope", "false");
        assert_eq!(metadata.len(), 1);
        ensure_metadata_entry(&mut metadata, "manifest.hybrid_suite", "suite");
        assert_eq!(metadata.len(), 2);
    }

    #[test]
    fn direct_mode_enable_renders_snippet() {
        let plan = DirectModePlanOutput::from_components(
            "nexus",
            [0x33; 32],
            "feedface".to_owned(),
            HostMappingSummary {
                canonical: "33333333.nexus.sorafs".to_owned(),
                vanity: "3333.nexus.direct.sorafs".to_owned(),
            },
            DirectCarLocator {
                canonical_url: "https://33333333.nexus.sorafs/direct/v1/car/feedface".to_owned(),
                vanity_url: "https://3333.nexus.direct.sorafs/direct/v1/car/feedface".to_owned(),
            },
            ManifestCapabilitySummary::default(),
        );
        let mut plan_file = NamedTempFile::new().expect("temp plan");
        plan_file
            .write_all(&norito::json::to_vec(&plan).expect("serialize plan"))
            .expect("write plan");

        let args = GatewayDirectModeEnableArgs {
            plan: plan_file.path().to_path_buf(),
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("enable command runs");
        assert_eq!(ctx.outputs().len(), 1);
        let snippet = &ctx.outputs()[0];
        assert!(snippet.contains("require_manifest_envelope = false"));
        assert!(snippet.contains("direct_car_canonical"));
        assert!(snippet.contains(&plan.provider_id_hex));
    }

    #[test]
    fn direct_mode_rollback_snippet_matches_defaults() {
        let args = GatewayDirectModeRollbackArgs;
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("rollback command runs");
        assert_eq!(
            ctx.outputs(),
            &[render_direct_mode_rollback_snippet().to_owned()]
        );
    }

    #[test]
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
        assert_eq!(
            plan["manifest_json"].as_str().expect("manifest string"),
            manifest_path.display().to_string()
        );
        assert_eq!(
            plan["hostname"].as_str().expect("hostname string"),
            "docs.sora.link"
        );
        let headers = plan["headers"].as_object().expect("headers object missing");
        assert_eq!(
            headers["Sora-Name"].as_str().expect("Sora-Name must exist"),
            "sora:docs"
        );
        let proof_json = BASE64
            .decode(headers["Sora-Proof"].as_str().expect("Sora-Proof base64"))
            .expect("decode proof payload");
        let proof_value: Value =
            norito::json::from_slice(&proof_json).expect("decode proof payload JSON");
        assert_eq!(
            proof_value["alias"].as_str().expect("alias string"),
            "sora:docs"
        );
        assert!(
            plan["headers_template"]
                .as_str()
                .expect("headers template string")
                .contains("Sora-Route-Binding"),
            "expected rendered header template"
        );
        let header_file = fs::read_to_string(&headers_path).expect("header template");
        assert!(header_file.contains("Sora-Content-CID"));
        assert!(
            ctx.outputs()
                .iter()
                .any(|line| line.contains(output_path.to_string_lossy().as_ref()))
        );
    }

    #[test]
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
        assert_eq!(
            rollback["manifest_json"]
                .as_str()
                .expect("rollback manifest"),
            rollback_path.display().to_string()
        );
        assert_eq!(
            rollback["release_tag"].as_str().expect("release tag"),
            "previous"
        );
        assert!(
            rollback
                .get("headers_path")
                .and_then(Value::as_str)
                .is_some_and(|value| value.contains("gateway.route.rollback.headers.txt"))
        );
        let header_file = fs::read_to_string(&headers_path).expect("header template");
        assert!(
            !header_file.contains("Content-Security-Policy"),
            "CSP header should be omitted when --no-csp is set"
        );
        let rollback_headers =
            fs::read_to_string(&rollback_headers_path).expect("rollback header template");
        assert!(
            rollback_headers.contains("Sora-Route-Binding"),
            "rollback template should include Sora-Route-Binding"
        );
        assert!(
            ctx.outputs()
                .iter()
                .any(|line| line.contains("rollback headers written")),
            "expected rollback output message"
        );
    }

    #[test]
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
        assert_eq!(
            payload["aliases"],
            Value::Array(vec![Value::from("docs:portal")])
        );
        assert_eq!(payload["manifest_digest_hex"], Value::from("aa".repeat(32)));
        assert_eq!(payload["release_tag"], Value::from("portal-2026.04.01"));
        assert_eq!(payload["car_digest_hex"], Value::Null);
        let curl = &ctx.outputs()[1];
        assert!(
            curl.contains("https://cache.example.com/purge"),
            "curl snippet should reference endpoint"
        );
        assert!(
            curl.contains("Authorization: Bearer $CACHE_TOKEN"),
            "curl snippet should reference the auth env var"
        );
    }

    #[test]
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
        assert_eq!(
            ctx.outputs()[0],
            format!("wrote cache invalidation payload to {}", path.display())
        );
        let payload_str = std::fs::read_to_string(&path).expect("read payload");
        let payload: Value = norito::json::from_str(&payload_str).expect("json payload");
        assert_eq!(payload["release_tag"], Value::Null);
        assert_eq!(payload["car_digest_hex"], Value::from("cc".repeat(32)));
        let curl = &ctx.outputs()[1];
        assert!(
            curl.contains("--data '{"),
            "curl snippet should embed the JSON payload"
        );
    }

    #[test]
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

    #[test]
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
        assert!(
            value.get("relay_id").is_some(),
            "relay id missing in output"
        );

        let bytes = std::fs::read(&instruction_path).expect("read instruction");
        let decoded: RelayRewardInstructionV1 =
            decode_from_bytes(&bytes).expect("decode instruction");
        assert_eq!(decoded.beneficiary, sample_account_id("beneficiary"));
        assert!(decoded.payout_amount > Numeric::zero());
    }

    #[test]
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

    #[test]
    fn incentives_dashboard_summarises_rewards() {
        let mut inst1 = sample_reward_instruction();
        inst1.payout_amount = Numeric::from(40_u32);
        let mut inst1_file = NamedTempFile::new().expect("inst1");
        let inst1_bytes = to_bytes(&inst1).expect("encode inst1");
        inst1_file.write_all(&inst1_bytes).expect("write inst1");

        let mut inst2 = sample_reward_instruction();
        inst2.epoch = inst1.epoch + 1;
        inst2.payout_amount = Numeric::from(10_u32);
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
        assert_eq!(
            summary["total_payout_nanos"].as_u64(),
            Some(50_000_000_000),
            "reward nanos should sum"
        );
        let rows = summary["rows"].as_array().expect("rows present");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["payout_count"].as_u64(), Some(2));
        assert_eq!(rows[0]["payout_nanos"].as_u64(), Some(50_000_000_000));
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

        let init_args = IncentivesServiceInitArgs {
            state: state_path.clone(),
            config: config_file.path().to_path_buf(),
            treasury_account: sample_account_literal("treasury"),
            force: false,
            allow_missing_budget_approval: true,
        };
        let mut init_ctx = TestContext::new();
        init_args.run(&mut init_ctx).expect("init command runs");

        let metrics_dir = tmp_dir.path().join("metrics");
        fs::create_dir_all(&metrics_dir).expect("create metrics dir");

        let relay_a = [0x21_u8; 32];
        let relay_b = [0x43_u8; 32];
        let relay_primary_bond = RelayBondLedgerEntryV1 {
            relay_id: relay_a,
            bonded_amount: Numeric::from(5_000_u32),
            bond_asset_id: xor_asset_id(),
            bonded_since_unix: 1,
            exit_capable: true,
        };
        let relay_secondary_bond = RelayBondLedgerEntryV1 {
            relay_id: relay_b,
            bonded_amount: Numeric::from(7_500_u32),
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
            allow_missing_budget_approval: false,
        };

        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("shadow run executes");
        assert_eq!(ctx.outputs().len(), 1, "expected JSON summary output");

        let summary: norito::json::Value =
            norito::json::from_str(&ctx.outputs()[0]).expect("parse summary json");
        assert_eq!(summary["processed_payouts"].as_u64(), Some(3));
        assert_eq!(summary["total_relays"].as_u64(), Some(2));
        let expected_budget_hex = sample_budget_id_hex();
        assert_eq!(
            summary["expected_budget_approval"].as_str(),
            Some(expected_budget_hex.as_str())
        );
        assert_eq!(summary["missing_budget_approval"].as_u64(), Some(0));
        assert_eq!(summary["mismatched_budget_approval"].as_u64(), Some(0));
        let relays = summary["relays"]
            .as_array()
            .expect("relay summaries present");
        assert_eq!(relays.len(), 2);
        assert!(
            relays
                .iter()
                .any(|relay| relay["warning_epochs"].as_u64() == Some(1))
        );
    }

    #[test]
    fn incentives_state_roundtrip_serializes() {
        let policy = RelayBondPolicyV1 {
            minimum_exit_bond: Numeric::from(1_000_u32),
            bond_asset_id: xor_asset_id(),
            uptime_floor_per_mille: 900,
            slash_penalty_basis_points: 250,
            activation_grace_epochs: 0,
        };
        let reward_config = RewardConfig {
            policy: policy.clone(),
            base_reward: Numeric::from(75_u32),
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
        assert_eq!(
            decoded.reward_config.base_reward,
            state.reward_config.base_reward
        );
    }

    #[test]
    fn incentives_service_process_requires_budget_id() {
        let mut config = sample_reward_config_json();
        if let Some(object) = config.as_object_mut() {
            object.insert("budget_approval_id".to_string(), Value::Null);
        }

        let config_file = NamedTempFile::new().expect("config file");
        fs::write(
            config_file.path(),
            norito::json::to_vec_pretty(&config).expect("encode config"),
        )
        .expect("write config");

        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");

        let init_args = IncentivesServiceInitArgs {
            state: state_path.clone(),
            config: config_file.path().to_path_buf(),
            treasury_account: sample_account_literal("treasury"),
            force: false,
            allow_missing_budget_approval: true,
        };
        let mut init_ctx = TestContext::new();
        init_args.run(&mut init_ctx).expect("init command runs");

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
        assert!(
            err.to_string().contains("budget_approval_id"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn incentives_service_audit_flags_underbonded_relay() {
        let config_file = write_sample_reward_config_file();
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");

        let init_args = IncentivesServiceInitArgs {
            state: state_path.clone(),
            config: config_file.path().to_path_buf(),
            treasury_account: sample_account_literal("treasury"),
            force: false,
            allow_missing_budget_approval: true,
        };
        let mut init_ctx = TestContext::new();
        init_args.run(&mut init_ctx).expect("init command runs");

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
        assert_eq!(
            summary["bond"]["insufficient_bond"].as_u64(),
            Some(1),
            "underbonded relay should be reported"
        );
    }

    #[test]
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
        assert_eq!(
            budget
                .get("configured_budget_approval_id")
                .and_then(Value::as_str),
            Some(expected_budget.as_str())
        );
        assert_eq!(
            budget
                .get("mismatched_budget_approval")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            budget.get("payouts_without_budget").and_then(Value::as_u64),
            Some(1)
        );
    }

    #[test]
    fn incentives_service_init_writes_state() {
        let config_file = write_sample_reward_config_file();
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");

        let args = IncentivesServiceInitArgs {
            state: state_path.clone(),
            config: config_file.path().to_path_buf(),
            treasury_account: sample_account_literal("treasury"),
            force: false,
            allow_missing_budget_approval: false,
        };

        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("init command runs");
        assert!(state_path.exists());

        let state = read_state(&state_path);
        assert_eq!(state.version, IncentivesState::VERSION);
        assert_eq!(state.treasury_account, sample_account_id("treasury"));
        assert!(state.payouts.is_empty());
        assert!(state.disputes.is_empty());
        assert_eq!(state.reward_config.policy.bond_asset_id, "xor#sora");
    }

    #[test]
    fn incentives_service_process_records_reward() {
        let config_file = write_sample_reward_config_file();
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");

        let init_args = IncentivesServiceInitArgs {
            state: state_path.clone(),
            config: config_file.path().to_path_buf(),
            treasury_account: sample_account_literal("treasury"),
            force: false,
            allow_missing_budget_approval: true,
        };
        let mut init_ctx = TestContext::new();
        init_args.run(&mut init_ctx).expect("init command runs");

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
        assert_eq!(
            state.payouts[0].beneficiary,
            sample_account_id("beneficiary")
        );

        let instruction_bytes = fs::read(instruction_out.path()).expect("read instruction");
        let instruction: RelayRewardInstructionV1 =
            decode_from_bytes(&instruction_bytes).expect("decode instruction");
        assert_eq!(instruction.epoch, metrics.epoch);
    }

    #[test]
    fn incentives_daemon_rejects_missing_budget_without_override() {
        let config_file = write_reward_config_with_budget(None);
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");

        let init_args = IncentivesServiceInitArgs {
            state: state_path.clone(),
            config: config_file.path().to_path_buf(),
            treasury_account: sample_account_literal("treasury"),
            force: false,
            allow_missing_budget_approval: true,
        };
        let mut init_ctx = TestContext::new();
        init_args.run(&mut init_ctx).expect("init command runs");

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
            allow_missing_budget_approval: false,
        };

        let mut ctx = TestContext::new();
        let result = daemon_args.run(&mut ctx);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("budget_approval_id"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn incentives_daemon_allows_missing_budget_with_override() {
        let config_file = write_reward_config_with_budget(None);
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");

        let init_args = IncentivesServiceInitArgs {
            state: state_path.clone(),
            config: config_file.path().to_path_buf(),
            treasury_account: sample_account_literal("treasury"),
            force: false,
            allow_missing_budget_approval: true,
        };
        let mut init_ctx = TestContext::new();
        init_args.run(&mut init_ctx).expect("init command runs");

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
            allow_missing_budget_approval: true,
        };

        let mut ctx = TestContext::new();
        daemon_args
            .run(&mut ctx)
            .expect("daemon run succeeds when enforcement disabled");
        assert!(
            !ctx.outputs().is_empty(),
            "expected at least one daemon output entry"
        );

        let summary: norito::json::Value = ctx
            .outputs()
            .iter()
            .find_map(|line| norito::json::from_str(line).ok())
            .expect("parse daemon summary");
        let processed_len = summary["processed"].as_array().map_or(0, Vec::len);
        assert!(
            processed_len >= 1,
            "expected at least one processed payout, got {processed_len}"
        );
        assert!(
            summary["missing_budget_approval"]
                .as_u64()
                .is_some_and(|count| count >= 1),
            "expected at least one missing budget approval, got {:?}",
            summary["missing_budget_approval"]
        );
        assert_eq!(summary["mismatched_budget_approval"].as_u64(), Some(0));
        assert!(summary["expected_budget_approval"].as_str().is_none());
        let processed = summary["processed"]
            .as_array()
            .expect("processed payouts present");
        assert!(
            processed
                .first()
                .and_then(|payout| payout.get("budget_approval_id"))
                .is_some_and(Value::is_null)
        );
    }

    #[test]
    fn incentives_daemon_reports_budget_hash_with_allow_flag() {
        let config_file = write_sample_reward_config_file();
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");

        let init_args = IncentivesServiceInitArgs {
            state: state_path.clone(),
            config: config_file.path().to_path_buf(),
            treasury_account: sample_account_literal("treasury"),
            force: false,
            allow_missing_budget_approval: true,
        };
        let mut init_ctx = TestContext::new();
        init_args.run(&mut init_ctx).expect("init command runs");

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
            allow_missing_budget_approval: true,
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
        assert_eq!(
            summary["expected_budget_approval"].as_str(),
            Some(expected_budget_hex.as_str())
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn incentives_service_dispute_flow_updates_state() {
        let config_file = write_sample_reward_config_file();
        let tmp_dir = tempfile::tempdir().expect("temp dir");
        let state_path = tmp_dir.path().join("payout_state.json");

        let init_args = IncentivesServiceInitArgs {
            state: state_path.clone(),
            config: config_file.path().to_path_buf(),
            treasury_account: sample_account_literal("treasury"),
            force: false,
            allow_missing_budget_approval: false,
        };
        let mut init_ctx = TestContext::new();
        init_args.run(&mut init_ctx).expect("init command runs");

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
        assert_eq!(
            stored.requested_amount,
            Numeric::from_str("120").expect("numeric literal")
        );
        assert_eq!(
            stored
                .requested_adjustment
                .as_ref()
                .expect("adjustment present")
                .amount,
            Numeric::from_str("25").expect("numeric literal")
        );

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
                assert_eq!(
                    amount.clone(),
                    Some(Numeric::from_str("25").expect("numeric literal"))
                );
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
        assert_eq!(transfer.object, Numeric::from(25_u32));
        assert_eq!(transfer.destination, sample_account_id("beneficiary"));
        assert_eq!(transfer.source.account, sample_account_id("treasury"));
    }
}
