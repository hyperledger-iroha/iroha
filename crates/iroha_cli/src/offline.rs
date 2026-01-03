use super::*;

use clap::ValueEnum;
use core::convert::TryFrom;
use iroha::data_model::{
    offline::{
        AggregateProofEnvelope, AndroidIntegrityPolicy, OfflineAllowanceRecord,
        OfflineCounterSummary, OfflineProofRequestCounter, OfflineProofRequestKind,
        OfflineProofRequestReplay, OfflineProofRequestSum, OfflineSpendReceipt,
        OfflineToOnlineTransfer, OfflineTransferRecord, OfflineTransferStatus,
        OfflineVerdictRevocation, OfflineVerdictSnapshot, compute_receipts_root,
    },
    prelude::AccountId,
    query::{
        builder::{QueryBuilder, QueryBuilderExt},
        offline::prelude::{
            FindOfflineAllowanceByCertificateId, FindOfflineAllowances,
            FindOfflineToOnlineTransferById, FindOfflineToOnlineTransfers,
            FindOfflineToOnlineTransfersByController, FindOfflineToOnlineTransfersByPolicy,
            FindOfflineToOnlineTransfersByReceiver, FindOfflineToOnlineTransfersByStatus,
            FindOfflineVerdictRevocations,
        },
        parameters::{FetchSize, Pagination, Sorting},
    },
};
use iroha_crypto::Hash;
use iroha_primitives::numeric::Numeric;
use norito::{decode_from_bytes, json::JsonSerialize};
use reqwest::blocking::Client as BlockingHttpClient;
use std::{
    fs,
    io::Write,
    num::NonZeroU64,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Offline allowance and bundle inspection commands.
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Inspect offline allowances registered on-ledger
    #[command(subcommand)]
    Allowance(AllowanceCommand),
    /// Inspect pending offline-to-online transfer bundles
    #[command(subcommand)]
    Transfer(TransferCommand),
    /// Inspect offline bundle fixtures and aggregate proofs
    #[command(subcommand)]
    Bundle(BundleCommand),
    /// Inspect derived counter summaries per offline certificate
    #[command(subcommand)]
    Summary(SummaryCommand),
    /// Inspect recorded verdict revocations
    #[command(subcommand)]
    Revocation(RevocationCommand),
    /// Fetch offline rejection telemetry snapshots
    #[command(subcommand)]
    Rejection(RejectionCommand),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Allowance(cmd) => cmd.run(context),
            Command::Transfer(cmd) => cmd.run(context),
            Command::Bundle(cmd) => cmd.run(context),
            Command::Summary(cmd) => cmd.run(context),
            Command::Revocation(cmd) => cmd.run(context),
            Command::Rejection(cmd) => cmd.run(context),
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(clap::Subcommand, Debug)]
pub enum AllowanceCommand {
    /// List all registered offline allowances
    List(AllowanceListArgs),
    /// Fetch a specific allowance by certificate id
    Get(AllowanceId),
}

impl Run for AllowanceCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        match self {
            AllowanceCommand::List(args) => {
                let builder = client.query(FindOfflineAllowances);
                let builder = apply_allowance_common_args(builder, &args.common.common)?;
                let mut entries = builder.execute_all()?;
                apply_allowance_filters(&mut entries, &args)?;
                if args.summary {
                    let now_ms = current_timestamp_ms()?;
                    let rows: Vec<_> = entries
                        .iter()
                        .map(|record| AllowanceSummaryRow::from_record(record, now_ms))
                        .collect();
                    context.print_data(&rows)
                } else if args.common.verbose {
                    context.print_data(&entries)
                } else {
                    let ids: Vec<_> = entries
                        .into_iter()
                        .map(|record| record.certificate_id())
                        .collect();
                    context.print_data(&ids)
                }
            }
            AllowanceCommand::Get(args) => {
                let mut results = client
                    .query(FindOfflineAllowanceByCertificateId::new(
                        args.certificate_id,
                    ))
                    .execute_all()?;
                let record = results
                    .pop()
                    .ok_or_else(|| eyre!("Offline allowance not found"))?;
                context.print_data(&record)
            }
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct AllowanceId {
    /// Deterministic certificate identifier (hex)
    #[arg(long)]
    certificate_id: Hash,
}

#[derive(clap::Args, Debug)]
pub struct AllowanceListArgs {
    /// Common list/query controls (filters, pagination, selectors)
    #[command(flatten)]
    pub common: crate::list_support::AllArgs,
    /// Optional controller filter (account id)
    #[arg(long, value_name = "ACCOUNT@DOMAIN")]
    pub controller: Option<AccountId>,
    /// Optional verdict identifier filter (hex)
    #[arg(long, value_name = "HEX")]
    pub verdict_id: Option<Hash>,
    /// Optional attestation nonce filter (hex)
    #[arg(long, value_name = "HEX")]
    pub attestation_nonce: Option<Hash>,
    /// Only show allowances whose certificate expiry is at or before this value
    #[arg(long)]
    pub certificate_expires_before_ms: Option<u64>,
    /// Only show allowances whose certificate expiry is at or after this value
    #[arg(long)]
    pub certificate_expires_after_ms: Option<u64>,
    /// Only show allowances whose policy expiry is at or before this value
    #[arg(long)]
    pub policy_expires_before_ms: Option<u64>,
    /// Only show allowances whose policy expiry is at or after this value
    #[arg(long)]
    pub policy_expires_after_ms: Option<u64>,
    /// Only show allowances whose attestation refresh-by timestamp is at or before this value
    #[arg(long)]
    pub refresh_before_ms: Option<u64>,
    /// Only show allowances whose attestation refresh-by timestamp is at or after this value
    #[arg(long)]
    pub refresh_after_ms: Option<u64>,
    /// Emit summary rows with expiry/verdict metadata instead of bare certificate ids
    #[arg(long)]
    pub summary: bool,
    /// Include certificates that have already expired (default skips them)
    #[arg(long)]
    pub include_expired: bool,
}

#[allow(clippy::large_enum_variant)]
#[derive(clap::Subcommand, Debug)]
pub enum TransferCommand {
    /// List all pending offline-to-online transfer bundles
    List(TransferListArgs),
    /// Fetch a specific transfer bundle by id
    Get(BundleId),
    /// Generate a FASTPQ witness request for a bundle
    Proof(TransferProofArgs),
}

impl Run for TransferCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        match self {
            TransferCommand::List(args) => {
                let mut applied_filters = TransferFilterFlags::default();
                let mut entries = if let Some(controller) = args.controller.clone() {
                    applied_filters.controller = true;
                    let builder =
                        client.query(FindOfflineToOnlineTransfersByController { controller });
                    let builder = apply_transfer_common_args(builder, &args.common.common)?;
                    builder.execute_all()?
                } else if let Some(receiver) = args.receiver.clone() {
                    applied_filters.receiver = true;
                    let builder = client.query(FindOfflineToOnlineTransfersByReceiver { receiver });
                    let builder = apply_transfer_common_args(builder, &args.common.common)?;
                    builder.execute_all()?
                } else if let Some(status) = args.status.clone() {
                    applied_filters.status = true;
                    let builder = client.query(FindOfflineToOnlineTransfersByStatus {
                        status: status.as_status(),
                    });
                    let builder = apply_transfer_common_args(builder, &args.common.common)?;
                    builder.execute_all()?
                } else if let Some(policy) = args.platform_policy {
                    applied_filters.platform_policy = true;
                    let builder = client.query(FindOfflineToOnlineTransfersByPolicy {
                        policy: policy.as_policy(),
                    });
                    let builder = apply_transfer_common_args(builder, &args.common.common)?;
                    builder.execute_all()?
                } else {
                    let builder = client.query(FindOfflineToOnlineTransfers);
                    let builder = apply_transfer_common_args(builder, &args.common.common)?;
                    builder.execute_all()?
                };
                apply_transfer_filters(&mut entries, &args, applied_filters)?;
                if let Some(path) = args.audit_log.as_ref() {
                    let audit_entries: Vec<_> =
                        entries.iter().map(OfflineAuditLogEntry::from).collect();
                    write_audit_log(path, &audit_entries)?;
                }
                if args.summary {
                    let rows: Vec<_> = entries
                        .iter()
                        .map(TransferSummaryRow::from_record)
                        .collect();
                    context.print_data(&rows)
                } else if args.common.verbose {
                    context.print_data(&entries)
                } else {
                    let ids: Vec<_> = entries
                        .into_iter()
                        .map(|bundle| bundle.transfer.bundle_id)
                        .collect();
                    context.print_data(&ids)
                }
            }
            TransferCommand::Get(args) => {
                let mut results = client
                    .query(FindOfflineToOnlineTransferById::new(args.bundle_id))
                    .execute_all()?;
                let transfer = results
                    .pop()
                    .ok_or_else(|| eyre!("Offline-to-online transfer not found"))?;
                context.print_data(&transfer)
            }
            TransferCommand::Proof(args) => {
                let mut results = client
                    .query(FindOfflineToOnlineTransferById::new(args.bundle_id))
                    .execute_all()?;
                let record = results
                    .pop()
                    .ok_or_else(|| eyre!("Offline-to-online transfer not found"))?;
                match build_proof_request(&record, &args)? {
                    ProofRequestVariant::Sum(req) => context.print_data(&req),
                    ProofRequestVariant::Counter(req) => context.print_data(&req),
                    ProofRequestVariant::Replay(req) => context.print_data(&req),
                }
            }
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct BundleId {
    /// Deterministic bundle identifier (hex)
    #[arg(long)]
    bundle_id: Hash,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum ProofRequestKindArg {
    Sum,
    Counter,
    Replay,
}

impl From<ProofRequestKindArg> for OfflineProofRequestKind {
    fn from(kind: ProofRequestKindArg) -> Self {
        match kind {
            ProofRequestKindArg::Sum => Self::Sum,
            ProofRequestKindArg::Counter => Self::Counter,
            ProofRequestKindArg::Replay => Self::Replay,
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct TransferProofArgs {
    /// Bundle identifier used to fetch the transfer record
    #[arg(long)]
    bundle_id: Hash,
    /// Witness type to build
    #[arg(long, value_enum)]
    kind: ProofRequestKindArg,
    /// Optional counter checkpoint (defaults to first counter - 1)
    #[arg(long)]
    counter_checkpoint: Option<u64>,
    /// Replay log head hash (required for replay proofs)
    #[arg(long)]
    replay_log_head: Option<Hash>,
    /// Replay log tail hash (required for replay proofs)
    #[arg(long)]
    replay_log_tail: Option<Hash>,
}

enum ProofRequestVariant {
    Sum(OfflineProofRequestSum),
    Counter(OfflineProofRequestCounter),
    Replay(OfflineProofRequestReplay),
}

fn build_proof_request(
    record: &OfflineTransferRecord,
    args: &TransferProofArgs,
) -> Result<ProofRequestVariant> {
    let kind: OfflineProofRequestKind = args.kind.into();
    match kind {
        OfflineProofRequestKind::Sum => record
            .to_proof_request_sum()
            .map(ProofRequestVariant::Sum)
            .map_err(|err| eyre!("failed to build sum request: {err}")),
        OfflineProofRequestKind::Counter => {
            let checkpoint = if let Some(value) = args.counter_checkpoint {
                value
            } else {
                infer_counter_checkpoint(record)?
            };
            record
                .to_proof_request_counter(checkpoint)
                .map(ProofRequestVariant::Counter)
                .map_err(|err| eyre!("failed to build counter request: {err}"))
        }
        OfflineProofRequestKind::Replay => {
            let head = args
                .replay_log_head
                .ok_or_else(|| eyre!("--replay-log-head is required for replay proofs"))?;
            let tail = args
                .replay_log_tail
                .ok_or_else(|| eyre!("--replay-log-tail is required for replay proofs"))?;
            record
                .to_proof_request_replay(head, tail)
                .map(ProofRequestVariant::Replay)
                .map_err(|err| eyre!("failed to build replay request: {err}"))
        }
    }
}

fn infer_counter_checkpoint(record: &OfflineTransferRecord) -> Result<u64> {
    record
        .counter_checkpoint_hint()
        .map_err(|err| eyre!("failed to infer counter checkpoint: {err}"))
}

#[derive(clap::Args, Debug)]
pub struct TransferListArgs {
    /// Common list/query controls (filters, pagination, selectors)
    #[command(flatten)]
    pub common: crate::list_support::AllArgs,
    /// Optional controller filter (account id)
    #[arg(long, value_name = "ACCOUNT@DOMAIN")]
    pub controller: Option<AccountId>,
    /// Optional receiver filter (account id)
    #[arg(long, value_name = "ACCOUNT@DOMAIN")]
    pub receiver: Option<AccountId>,
    /// Optional lifecycle status filter
    #[arg(long, value_enum)]
    pub status: Option<TransferStatusArg>,
    /// Only show bundles whose certificate id matches the provided hex value
    #[arg(long, value_name = "HEX")]
    pub certificate_id: Option<Hash>,
    /// Only show bundles whose certificate expiry is at or before this value
    #[arg(long)]
    pub certificate_expires_before_ms: Option<u64>,
    /// Only show bundles whose certificate expiry is at or after this value
    #[arg(long)]
    pub certificate_expires_after_ms: Option<u64>,
    /// Only show bundles whose policy expiry is at or before this value
    #[arg(long)]
    pub policy_expires_before_ms: Option<u64>,
    /// Only show bundles whose policy expiry is at or after this value
    #[arg(long)]
    pub policy_expires_after_ms: Option<u64>,
    /// Only show bundles whose attestation refresh deadline is at or before this value
    #[arg(long)]
    pub refresh_before_ms: Option<u64>,
    /// Only show bundles whose attestation refresh deadline is at or after this value
    #[arg(long)]
    pub refresh_after_ms: Option<u64>,
    /// Optional verdict identifier filter (hex)
    #[arg(long, value_name = "HEX")]
    pub verdict_id: Option<Hash>,
    /// Optional attestation nonce filter (hex)
    #[arg(long, value_name = "HEX")]
    pub attestation_nonce: Option<Hash>,
    /// Restrict settled bundles to a specific Android integrity policy (requires Play Integrity or HMS tokens)
    #[arg(long, value_enum)]
    pub platform_policy: Option<PlatformPolicyArg>,
    /// Include only bundles that already carry verdict metadata
    #[arg(long)]
    pub require_verdict: bool,
    /// Include only bundles that are missing verdict metadata
    #[arg(long)]
    pub only_missing_verdict: bool,
    /// Write a canonical audit log JSON file containing `{tx_id,sender_id,receiver_id,asset_id,amount,timestamp_ms}` entries
    #[arg(long, value_name = "PATH")]
    pub audit_log: Option<PathBuf>,
    /// Emit summary rows with certificate/verdict metadata instead of bare bundle ids
    #[arg(long)]
    pub summary: bool,
}

#[derive(clap::Subcommand, Debug)]
pub enum BundleCommand {
    /// Inspect offline bundle fixtures and compute Poseidon receipts roots
    Inspect(BundleInspectArgs),
}

impl Run for BundleCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            BundleCommand::Inspect(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct BundleInspectArgs {
    /// Paths to offline bundle fixtures (JSON or Norito)
    #[arg(value_name = "PATH", required = true)]
    pub bundles: Vec<PathBuf>,
    /// Override the bundle encoding detection
    #[arg(long, value_enum, default_value_t = BundleEncoding::Auto)]
    pub encoding: BundleEncoding,
    /// Include aggregate proof byte counts and metadata keys
    #[arg(long)]
    pub proofs: bool,
}

impl BundleInspectArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let mut rows = Vec::with_capacity(self.bundles.len());
        for bundle_path in &self.bundles {
            let encoding = self.encoding.resolve_for_path(bundle_path);
            let transfer = load_bundle_from_path(bundle_path, encoding)?;
            let row = build_bundle_row(bundle_path, &transfer, self.proofs)?;
            rows.push(row);
        }
        context.print_data(&rows)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum BundleEncoding {
    Auto,
    Json,
    Norito,
}

impl BundleEncoding {
    fn resolve_for_path(self, path: &Path) -> Self {
        if self != Self::Auto {
            return self;
        }
        Self::guess_from_extension(path).unwrap_or(Self::Auto)
    }

    fn guess_from_extension(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
        match ext.as_str() {
            "json" => Some(Self::Json),
            "norito" | "to" => Some(Self::Norito),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, ValueEnum, PartialEq, Eq)]
pub enum TransferStatusArg {
    Settled,
    Archived,
}

impl TransferStatusArg {
    const fn as_status(&self) -> OfflineTransferStatus {
        match self {
            Self::Settled => OfflineTransferStatus::Settled,
            Self::Archived => OfflineTransferStatus::Archived,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum PlatformPolicyArg {
    PlayIntegrity,
    HmsSafetyDetect,
}

impl PlatformPolicyArg {
    const fn as_policy(self) -> AndroidIntegrityPolicy {
        match self {
            Self::PlayIntegrity => AndroidIntegrityPolicy::PlayIntegrity,
            Self::HmsSafetyDetect => AndroidIntegrityPolicy::HmsSafetyDetect,
        }
    }
}

#[derive(Default, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
struct TransferFilterFlags {
    controller: bool,
    receiver: bool,
    status: bool,
    platform_policy: bool,
}

#[derive(clap::Subcommand, Debug)]
pub enum SummaryCommand {
    /// List counter summaries derived from wallet allowances
    List(crate::list_support::AllArgs),
    /// Export counter summaries to a JSON digest for receiver sharing
    Export(SummaryExportArgs),
}

impl Run for SummaryCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        match self {
            SummaryCommand::List(args) => {
                let entries = fetch_counter_summaries(&client, &args.common)?;
                if args.verbose {
                    context.print_data(&entries)
                } else {
                    let ids: Vec<_> = entries.into_iter().map(|summary| summary.id()).collect();
                    context.print_data(&ids)
                }
            }
            SummaryCommand::Export(args) => {
                let entries = fetch_counter_summaries(&client, &args.common)?;
                let exported = write_summary_export(&args.output, &entries, args.pretty)?;
                context.println(format!(
                    "Wrote {exported} counter summaries to {}",
                    args.output.display()
                ))?;
                Ok(())
            }
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum RevocationCommand {
    /// List recorded verdict revocations
    List(crate::list_support::AllArgs),
}

impl Run for RevocationCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        match self {
            RevocationCommand::List(args) => {
                let builder = client.query(FindOfflineVerdictRevocations);
                let builder = apply_revocation_common_args(builder, &args.common)?;
                let entries = builder.execute_all()?;
                if args.verbose {
                    context.print_data(&entries)
                } else {
                    let verdict_ids: Vec<_> =
                        entries.iter().map(|record| record.verdict_id).collect();
                    context.print_data(&verdict_ids)
                }
            }
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum RejectionCommand {
    /// Fetch aggregated offline rejection counters
    Stats(RejectionStatsArgs),
}

impl Run for RejectionCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            RejectionCommand::Stats(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct RejectionStatsArgs {
    /// Optional telemetry profile header used when fetching stats
    #[arg(long, value_name = "PROFILE")]
    pub telemetry_profile: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct SummaryExportArgs {
    /// Common list/query controls (filters, pagination, selectors)
    #[command(flatten)]
    pub common: crate::list_support::CommonArgs,
    /// Destination file for the digest (JSON)
    #[arg(long, value_name = "PATH")]
    pub output: PathBuf,
    /// Pretty-print the JSON export instead of emitting a compact document
    #[arg(long, default_value_t = false)]
    pub pretty: bool,
}

fn apply_allowance_common_args<'a>(
    builder: QueryBuilder<'a, Client, FindOfflineAllowances, OfflineAllowanceRecord>,
    common: &'a crate::list_support::CommonArgs,
) -> Result<QueryBuilder<'a, Client, FindOfflineAllowances, OfflineAllowanceRecord>> {
    apply_common_args(builder, common)
}

fn apply_revocation_common_args<'a>(
    builder: QueryBuilder<'a, Client, FindOfflineVerdictRevocations, OfflineVerdictRevocation>,
    common: &'a crate::list_support::CommonArgs,
) -> Result<QueryBuilder<'a, Client, FindOfflineVerdictRevocations, OfflineVerdictRevocation>> {
    apply_common_args(builder, common)
}

fn apply_allowance_filters(
    entries: &mut Vec<OfflineAllowanceRecord>,
    args: &AllowanceListArgs,
) -> Result<()> {
    if let Some(controller) = &args.controller {
        entries.retain(|record| &record.certificate.controller == controller);
    }
    if let Some(verdict) = args.verdict_id.as_ref() {
        entries.retain(|record| record_verdict_id(record) == Some(verdict));
    }
    if let Some(nonce) = args.attestation_nonce.as_ref() {
        entries.retain(|record| record_attestation_nonce(record) == Some(nonce));
    }
    if let Some(threshold) = args.certificate_expires_before_ms {
        entries.retain(|record| record.certificate.expires_at_ms <= threshold);
    }
    if let Some(threshold) = args.certificate_expires_after_ms {
        entries.retain(|record| record.certificate.expires_at_ms >= threshold);
    }
    if let Some(threshold) = args.policy_expires_before_ms {
        entries.retain(|record| record.certificate.policy.expires_at_ms <= threshold);
    }
    if let Some(threshold) = args.policy_expires_after_ms {
        entries.retain(|record| record.certificate.policy.expires_at_ms >= threshold);
    }
    if let Some(threshold) = args.refresh_before_ms {
        entries.retain(|record| record_refresh_at(record).is_some_and(|value| value <= threshold));
    }
    if let Some(threshold) = args.refresh_after_ms {
        entries.retain(|record| record_refresh_at(record).is_some_and(|value| value >= threshold));
    }
    if !args.include_expired {
        let now = current_timestamp_ms()?;
        entries.retain(|record| {
            record.certificate.expires_at_ms > now
                && record.certificate.policy.expires_at_ms > now
                && record_refresh_at(record).is_none_or(|value| value > now)
        });
    }
    Ok(())
}

fn record_verdict_id(record: &OfflineAllowanceRecord) -> Option<&Hash> {
    record
        .verdict_id
        .as_ref()
        .or(record.certificate.verdict_id.as_ref())
}

fn record_attestation_nonce(record: &OfflineAllowanceRecord) -> Option<&Hash> {
    record
        .attestation_nonce
        .as_ref()
        .or(record.certificate.attestation_nonce.as_ref())
}

fn record_refresh_at(record: &OfflineAllowanceRecord) -> Option<u64> {
    record.refresh_at_ms.or(record.certificate.refresh_at_ms)
}

fn transfer_verdict_id(record: &OfflineTransferRecord) -> Option<&Hash> {
    record
        .verdict_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.verdict_id.as_ref())
        .or_else(|| {
            record
                .primary_certificate()
                .and_then(|cert| cert.verdict_id.as_ref())
        })
}

fn transfer_attestation_nonce(record: &OfflineTransferRecord) -> Option<&Hash> {
    record
        .verdict_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.attestation_nonce.as_ref())
        .or_else(|| {
            record
                .primary_certificate()
                .and_then(|cert| cert.attestation_nonce.as_ref())
        })
}

fn transfer_refresh_at(record: &OfflineTransferRecord) -> Option<u64> {
    record
        .verdict_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.refresh_at_ms)
        .or_else(|| {
            record
                .primary_certificate()
                .and_then(|cert| cert.refresh_at_ms)
        })
}

fn transfer_certificate_expires(record: &OfflineTransferRecord) -> Option<u64> {
    record
        .verdict_snapshot
        .as_ref()
        .map(|snapshot| snapshot.certificate_expires_at_ms)
        .or_else(|| record.primary_certificate().map(|cert| cert.expires_at_ms))
}

fn transfer_policy_expires(record: &OfflineTransferRecord) -> Option<u64> {
    record
        .verdict_snapshot
        .as_ref()
        .map(|snapshot| snapshot.policy_expires_at_ms)
        .or_else(|| {
            record
                .primary_certificate()
                .map(|cert| cert.policy.expires_at_ms)
        })
}

fn map_verdict_snapshot(snapshot: &OfflineVerdictSnapshot) -> TransferVerdictSnapshot {
    TransferVerdictSnapshot {
        certificate_id_hex: hex::encode(snapshot.certificate_id.as_ref()),
        verdict_id_hex: snapshot.verdict_id.map(|hash| hex::encode(hash.as_ref())),
        attestation_nonce_hex: snapshot
            .attestation_nonce
            .map(|hash| hex::encode(hash.as_ref())),
        refresh_at_ms: snapshot.refresh_at_ms,
        certificate_expires_at_ms: snapshot.certificate_expires_at_ms,
        policy_expires_at_ms: snapshot.policy_expires_at_ms,
    }
}

const ALLOWANCE_WARNING_THRESHOLD_MS: u64 = 86_400_000;

struct AllowanceDeadlineInfo {
    kind: &'static str,
    state: &'static str,
    deadline_ms: u64,
    millis_remaining: i64,
}

fn allowance_deadline(
    record: &OfflineAllowanceRecord,
    now_ms: u64,
) -> Option<AllowanceDeadlineInfo> {
    let deadlines = [
        ("refresh", record_refresh_at(record)),
        ("policy", Some(record.certificate.policy.expires_at_ms)),
        ("certificate", Some(record.certificate.expires_at_ms)),
    ];

    for (kind, maybe_deadline) in deadlines {
        let Some(deadline_ms) = maybe_deadline else {
            continue;
        };
        if deadline_ms == 0 {
            continue;
        }
        let deadline = i128::from(deadline_ms);
        let now = i128::from(now_ms);
        let remaining = deadline - now;
        let (state, remaining_ms) = if remaining <= 0 {
            ("expired", clamp_i128_to_i64(remaining))
        } else if deadline_ms.saturating_sub(now_ms) <= ALLOWANCE_WARNING_THRESHOLD_MS {
            ("warning", clamp_i128_to_i64(remaining))
        } else {
            ("ok", clamp_i128_to_i64(remaining))
        };
        return Some(AllowanceDeadlineInfo {
            kind,
            state,
            deadline_ms,
            millis_remaining: remaining_ms,
        });
    }
    None
}

fn clamp_i128_to_i64(value: i128) -> i64 {
    let min = i128::from(i64::MIN);
    let max = i128::from(i64::MAX);
    let clamped = value.clamp(min, max);
    i64::try_from(clamped).expect("value clamped into i64 range")
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct AllowanceSummaryRow {
    certificate_id_hex: String,
    controller_id: String,
    expires_at_ms: u64,
    policy_expires_at_ms: u64,
    refresh_at_ms: Option<u64>,
    verdict_id_hex: Option<String>,
    attestation_nonce_hex: Option<String>,
    remaining_amount: String,
    deadline_kind: Option<String>,
    deadline_state: Option<String>,
    deadline_ms: Option<u64>,
    deadline_ms_remaining: Option<i64>,
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct TransferSummaryRow {
    bundle_id_hex: String,
    controller_id: String,
    receiver_id: String,
    deposit_account_id: String,
    status: String,
    recorded_at_ms: u64,
    total_amount: String,
    certificate_id_hex: Option<String>,
    certificate_expires_at_ms: Option<u64>,
    policy_expires_at_ms: Option<u64>,
    refresh_at_ms: Option<u64>,
    verdict_id_hex: Option<String>,
    attestation_nonce_hex: Option<String>,
    status_transitions: Vec<TransferStatusTransitionRow>,
    verdict_snapshot: Option<TransferVerdictSnapshot>,
    platform_token_snapshot: Option<TransferPlatformTokenSnapshot>,
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct TransferStatusTransitionRow {
    status: String,
    transitioned_at_ms: u64,
    verdict_snapshot: Option<TransferVerdictSnapshot>,
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct TransferVerdictSnapshot {
    certificate_id_hex: String,
    verdict_id_hex: Option<String>,
    attestation_nonce_hex: Option<String>,
    refresh_at_ms: Option<u64>,
    certificate_expires_at_ms: u64,
    policy_expires_at_ms: u64,
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct TransferPlatformTokenSnapshot {
    policy: String,
    attestation_jws_b64: String,
}

impl TransferSummaryRow {
    fn from_record(record: &OfflineTransferRecord) -> Self {
        let certificate_id_hex = record
            .certificate_id()
            .map(|hash| hex::encode(hash.as_ref()));
        let certificate_expires_at_ms = transfer_certificate_expires(record);
        let policy_expires_at_ms = transfer_policy_expires(record);
        let refresh_at_ms = transfer_refresh_at(record);
        let verdict_id_hex = transfer_verdict_id(record).map(|hash| hex::encode(hash.as_ref()));
        let attestation_nonce_hex =
            transfer_attestation_nonce(record).map(|hash| hex::encode(hash.as_ref()));
        let total_amount = record
            .transfer
            .receipts
            .iter()
            .fold(Numeric::zero(), |acc, receipt| {
                let fallback = acc.clone();
                acc.checked_add(receipt.amount().clone())
                    .unwrap_or(fallback)
            })
            .to_string();
        let status_transitions = record
            .history
            .iter()
            .map(|entry| TransferStatusTransitionRow {
                status: entry.status.as_label().into(),
                transitioned_at_ms: entry.transitioned_at_ms,
                verdict_snapshot: entry.verdict_snapshot.as_ref().map(map_verdict_snapshot),
            })
            .collect();
        let verdict_snapshot = record.verdict_snapshot.as_ref().map(map_verdict_snapshot);
        let platform_token_snapshot =
            record
                .platform_snapshot
                .as_ref()
                .map(|snapshot| TransferPlatformTokenSnapshot {
                    policy: snapshot.policy_label().into(),
                    attestation_jws_b64: snapshot.attestation_jws_b64().into(),
                });
        Self {
            bundle_id_hex: hex::encode(record.transfer.bundle_id.as_ref()),
            controller_id: record.controller.to_string(),
            receiver_id: record.transfer.receiver.to_string(),
            deposit_account_id: record.transfer.deposit_account.to_string(),
            status: record.status.as_label().into(),
            recorded_at_ms: record.recorded_at_ms,
            total_amount,
            certificate_id_hex,
            certificate_expires_at_ms,
            policy_expires_at_ms,
            refresh_at_ms,
            verdict_id_hex,
            attestation_nonce_hex,
            status_transitions,
            verdict_snapshot,
            platform_token_snapshot,
        }
    }
}

impl AllowanceSummaryRow {
    fn from_record(record: &OfflineAllowanceRecord, now_ms: u64) -> Self {
        let certificate_id_hex = hex::encode(record.certificate_id().as_ref());
        let controller_id = record.certificate.controller.to_string();
        let expires_at_ms = record.certificate.expires_at_ms;
        let policy_expires_at_ms = record.certificate.policy.expires_at_ms;
        let refresh_at_ms = record_refresh_at(record);
        let verdict_id_hex = record_verdict_id(record).map(|hash| hex::encode(hash.as_ref()));
        let attestation_nonce_hex =
            record_attestation_nonce(record).map(|hash| hex::encode(hash.as_ref()));
        let remaining_amount = record.remaining_amount.to_string();
        let deadline = allowance_deadline(record, now_ms);
        let (deadline_kind, deadline_state, deadline_ms, deadline_ms_remaining) =
            deadline.map_or((None, None, None, None), |info| {
                (
                    Some(info.kind.to_string()),
                    Some(info.state.to_string()),
                    Some(info.deadline_ms),
                    Some(info.millis_remaining),
                )
            });
        Self {
            certificate_id_hex,
            controller_id,
            expires_at_ms,
            policy_expires_at_ms,
            refresh_at_ms,
            verdict_id_hex,
            attestation_nonce_hex,
            remaining_amount,
            deadline_kind,
            deadline_state,
            deadline_ms,
            deadline_ms_remaining,
        }
    }
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct BundleInspectionRow {
    bundle_path: String,
    bundle_id_hex: String,
    receiver_id: String,
    deposit_account_id: String,
    receipt_count: u64,
    total_receipt_amount: String,
    receipts_root_hex: String,
    aggregate_proof_root_hex: Option<String>,
    receipts_root_matches: Option<bool>,
    proof_status: String,
    proof_summary: Option<BundleProofSummary>,
    attachments_present: bool,
    platform_snapshot_policy: Option<String>,
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct BundleProofSummary {
    version: u16,
    proof_sum_bytes: Option<usize>,
    proof_counter_bytes: Option<usize>,
    proof_replay_bytes: Option<usize>,
    metadata_keys: Vec<String>,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
struct OfflineAuditLogEntry {
    tx_id: String,
    sender_id: String,
    receiver_id: String,
    asset_id: String,
    amount: String,
    timestamp_ms: u64,
}

impl From<&OfflineTransferRecord> for OfflineAuditLogEntry {
    fn from(record: &OfflineTransferRecord) -> Self {
        let fallback_sender = record.transfer.receiver.to_string();
        let fallback_receiver = record.transfer.deposit_account.to_string();
        let fallback_asset = record
            .transfer
            .balance_proof
            .initial_commitment
            .asset
            .to_string();
        let fallback_amount = record.transfer.balance_proof.claimed_delta.to_string();
        let (sender_id, receiver_id, asset_id, amount) = record.transfer.receipts.first().map_or(
            (
                fallback_sender,
                fallback_receiver,
                fallback_asset,
                fallback_amount,
            ),
            |receipt| {
                (
                    receipt.from.to_string(),
                    receipt.to.to_string(),
                    receipt.asset.to_string(),
                    receipt.amount.to_string(),
                )
            },
        );
        Self {
            tx_id: hex::encode(record.transfer.bundle_id.as_ref()),
            sender_id,
            receiver_id,
            asset_id,
            amount,
            timestamp_ms: record.recorded_at_ms,
        }
    }
}

fn apply_transfer_common_args<'a, Q>(
    builder: QueryBuilder<'a, Client, Q, OfflineTransferRecord>,
    common: &'a crate::list_support::CommonArgs,
) -> Result<QueryBuilder<'a, Client, Q, OfflineTransferRecord>>
where
    Q: iroha::data_model::query::Query<Item = OfflineTransferRecord> + 'static,
{
    apply_common_args(builder, common)
}

fn apply_transfer_filters(
    entries: &mut Vec<OfflineTransferRecord>,
    args: &TransferListArgs,
    applied: TransferFilterFlags,
) -> Result<()> {
    if args.require_verdict && args.only_missing_verdict {
        return Err(eyre!(
            "`--require-verdict` cannot be combined with `--only-missing-verdict`"
        ));
    }
    if args.only_missing_verdict && args.verdict_id.is_some() {
        return Err(eyre!(
            "`--verdict-id` cannot be combined with `--only-missing-verdict`"
        ));
    }

    if !applied.controller
        && let Some(controller) = &args.controller
    {
        entries.retain(|record| &record.controller == controller);
    }
    if !applied.receiver
        && let Some(receiver) = &args.receiver
    {
        entries.retain(|record| &record.transfer.receiver == receiver);
    }
    if !applied.status
        && let Some(status) = args.status.as_ref()
    {
        let expected = status.as_status();
        entries.retain(|record| record.status == expected);
    }
    if let Some(cert_id) = &args.certificate_id {
        entries.retain(|record| record.certificate_id().as_ref() == Some(cert_id));
    }
    if let Some(verdict) = args.verdict_id.as_ref() {
        entries.retain(|record| transfer_verdict_id(record) == Some(verdict));
    }
    if let Some(nonce) = args.attestation_nonce.as_ref() {
        entries.retain(|record| transfer_attestation_nonce(record) == Some(nonce));
    }
    if let Some(max) = args.certificate_expires_before_ms {
        entries.retain(|record| {
            transfer_certificate_expires(record).is_some_and(|value| value <= max)
        });
    }
    if let Some(min) = args.certificate_expires_after_ms {
        entries.retain(|record| {
            transfer_certificate_expires(record).is_some_and(|value| value >= min)
        });
    }
    if let Some(max) = args.policy_expires_before_ms {
        entries.retain(|record| transfer_policy_expires(record).is_some_and(|value| value <= max));
    }
    if let Some(min) = args.policy_expires_after_ms {
        entries.retain(|record| transfer_policy_expires(record).is_some_and(|value| value >= min));
    }
    if let Some(max) = args.refresh_before_ms {
        entries.retain(|record| transfer_refresh_at(record).is_some_and(|value| value <= max));
    }
    if let Some(min) = args.refresh_after_ms {
        entries.retain(|record| transfer_refresh_at(record).is_some_and(|value| value >= min));
    }
    if !applied.platform_policy
        && let Some(policy_arg) = args.platform_policy
    {
        let policy = policy_arg.as_policy();
        entries.retain(|record| {
            record
                .platform_snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.policy() == Some(policy))
        });
    }
    if args.require_verdict {
        entries.retain(|record| transfer_verdict_id(record).is_some());
    }
    if args.only_missing_verdict {
        entries.retain(|record| transfer_verdict_id(record).is_none());
    }
    Ok(())
}

fn apply_common_args<'a, Q, Item>(
    builder: QueryBuilder<'a, Client, Q, Item>,
    common: &'a crate::list_support::CommonArgs,
) -> Result<QueryBuilder<'a, Client, Q, Item>>
where
    Q: iroha::data_model::query::Query<Item = Item> + 'static,
    Item: 'static,
{
    let mut builder = builder;
    if let Some(key) = common.sort_by_metadata_key.clone() {
        let sorting = Sorting::new(Some(key), common.order.map(Into::into));
        builder = builder.with_sorting(sorting);
    }
    if common.limit.is_some() || common.offset > 0 {
        let pagination = Pagination::new(common.limit.and_then(NonZeroU64::new), common.offset);
        builder = builder.with_pagination(pagination);
    }
    if let Some(n) = common.fetch_size.and_then(NonZeroU64::new) {
        let fs = FetchSize::new(Some(n));
        builder = builder.with_fetch_size(fs);
    }
    if let Some(sel) = common.select.as_deref() {
        let tuple = crate::list_support::parse_selector_tuple::<Item>(sel)?;
        builder = builder.with_selector_tuple(tuple);
    }
    Ok(builder)
}

fn build_bundle_row(
    path: &Path,
    transfer: &OfflineToOnlineTransfer,
    include_proofs: bool,
) -> Result<BundleInspectionRow> {
    let receipts_root = compute_receipts_root(&transfer.receipts).map_err(|err| {
        eyre!(
            "failed to compute Poseidon receipts root for `{}`: {err}",
            path.display()
        )
    })?;
    let bundle_id_hex = hex::encode(transfer.bundle_id.as_ref());
    let receiver_id = transfer.receiver.to_string();
    let deposit_account_id = transfer.deposit_account.to_string();
    let receipt_count = u64::try_from(transfer.receipts.len()).unwrap_or(u64::MAX);
    let total_receipt_amount = bundle_total_amount(&transfer.receipts);
    let receipts_root_hex = receipts_root.to_hex_upper();
    let attachments_present = transfer
        .attachments
        .as_ref()
        .is_some_and(|list| !list.0.is_empty());
    let (aggregate_proof_root_hex, receipts_root_matches, proof_status, proof_summary) =
        transfer.aggregate_proof.as_ref().map_or_else(
            || (None, None, "missing".to_string(), None),
            |envelope| {
                let stored_hex = envelope.receipts_root.to_hex_upper();
                let matches = envelope.receipts_root == receipts_root;
                let status = if matches { "match" } else { "mismatch" }.to_string();
                let summary = if include_proofs {
                    Some(BundleProofSummary::from_envelope(envelope))
                } else {
                    None
                };
                (Some(stored_hex), Some(matches), status, summary)
            },
        );
    let platform_snapshot_policy = transfer
        .platform_snapshot
        .as_ref()
        .map(|snapshot| snapshot.policy_label().to_string());
    Ok(BundleInspectionRow {
        bundle_path: path.display().to_string(),
        bundle_id_hex,
        receiver_id,
        deposit_account_id,
        receipt_count,
        total_receipt_amount,
        receipts_root_hex,
        aggregate_proof_root_hex,
        receipts_root_matches,
        proof_status,
        proof_summary,
        attachments_present,
        platform_snapshot_policy,
    })
}

fn bundle_total_amount(receipts: &[OfflineSpendReceipt]) -> String {
    receipts
        .iter()
        .fold(Numeric::zero(), |acc, receipt| {
            let fallback = acc.clone();
            acc.checked_add(receipt.amount().clone())
                .unwrap_or(fallback)
        })
        .to_string()
}

fn write_audit_log<T: JsonSerialize>(path: &Path, entries: &T) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|err| {
            eyre!(
                "failed to create audit log directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let mut file = fs::File::create(path)
        .map_err(|err| eyre!("failed to create audit log file {}: {err}", path.display()))?;
    norito::json::to_writer_pretty(&mut file, entries)
        .map_err(|err| eyre!("failed to encode audit log JSON: {err}"))?;
    file.write_all(b"\n").map_err(|err| {
        eyre!(
            "failed to finalize audit log file {}: {err}",
            path.display()
        )
    })?;
    Ok(())
}

impl BundleProofSummary {
    fn from_envelope(envelope: &AggregateProofEnvelope) -> Self {
        let metadata_keys = envelope
            .metadata
            .iter()
            .map(|(name, _)| name.to_string())
            .collect();
        Self {
            version: envelope.version,
            proof_sum_bytes: envelope.proof_sum.as_ref().map(Vec::len),
            proof_counter_bytes: envelope.proof_counter.as_ref().map(Vec::len),
            proof_replay_bytes: envelope.proof_replay.as_ref().map(Vec::len),
            metadata_keys,
        }
    }
}

fn load_bundle_from_path(path: &Path, encoding: BundleEncoding) -> Result<OfflineToOnlineTransfer> {
    let bytes =
        fs::read(path).map_err(|err| eyre!("failed to read bundle `{}`: {err}", path.display()))?;
    match encoding {
        BundleEncoding::Json => parse_bundle_json(&bytes, path),
        BundleEncoding::Norito => parse_bundle_norito(&bytes, path),
        BundleEncoding::Auto => {
            if let Some(guessed) = BundleEncoding::guess_from_extension(path) {
                return match guessed {
                    BundleEncoding::Json => parse_bundle_json(&bytes, path),
                    BundleEncoding::Norito => parse_bundle_norito(&bytes, path),
                    BundleEncoding::Auto => unreachable!(),
                };
            }
            match parse_bundle_json(&bytes, path) {
                Ok(transfer) => Ok(transfer),
                Err(json_err) => match parse_bundle_norito(&bytes, path) {
                    Ok(transfer) => Ok(transfer),
                    Err(norito_err) => Err(eyre!(
                        "failed to decode bundle `{}` as JSON ({json_err}) or Norito ({norito_err})",
                        path.display()
                    )),
                },
            }
        }
    }
}

fn parse_bundle_json(bytes: &[u8], path: &Path) -> Result<OfflineToOnlineTransfer> {
    norito::json::from_slice(bytes)
        .map_err(|err| eyre!("failed to parse bundle `{}` as JSON: {err}", path.display()))
}

fn parse_bundle_norito(bytes: &[u8], path: &Path) -> Result<OfflineToOnlineTransfer> {
    decode_from_bytes(bytes).map_err(|err| {
        eyre!(
            "failed to decode bundle `{}` as Norito bytes: {err}",
            path.display()
        )
    })
}

fn fetch_counter_summaries<'a>(
    client: &'a Client,
    common: &'a crate::list_support::CommonArgs,
) -> Result<Vec<OfflineCounterSummary>> {
    let builder = client.query(FindOfflineAllowances);
    let builder = apply_allowance_common_args(builder, common)?;
    let allowances = builder.execute_all()?;
    Ok(allowances
        .iter()
        .map(OfflineCounterSummary::from_allowance)
        .collect())
}

fn write_summary_export(
    output: &Path,
    summaries: &[OfflineCounterSummary],
    pretty: bool,
) -> Result<usize> {
    let generated_at_ms = current_timestamp_ms()?;
    let envelope = summary_export_envelope(generated_at_ms, summaries);
    let bytes = if pretty {
        norito::json::to_vec_pretty(&envelope)
    } else {
        norito::json::to_vec(&envelope)
    }
    .map_err(|err| eyre!("failed to serialize counter summary export: {err}"))?;
    fs::write(output, &bytes).map_err(|err| {
        eyre!(
            "failed to write counter summary export to {}: {err}",
            output.display()
        )
    })?;
    Ok(summaries.len())
}

fn summary_export_envelope(
    generated_at_ms: u64,
    summaries: &[OfflineCounterSummary],
) -> norito::json::Value {
    use norito::json::{Map, Number, Value};

    fn number(value: u64) -> Value {
        Value::Number(Number::from(value))
    }

    let entries = summaries
        .iter()
        .map(summary_entry_value)
        .collect::<Vec<_>>();
    let mut obj = Map::new();
    obj.insert("generated_at_ms".into(), number(generated_at_ms));
    obj.insert(
        "summary_count".into(),
        number(u64::try_from(entries.len()).unwrap_or(u64::MAX)),
    );
    obj.insert("summaries".into(), Value::Array(entries));
    Value::Object(obj)
}

#[cfg(test)]
mod bundle_inspect_tests {
    use super::*;
    use iroha::data_model::{
        account::AccountId,
        asset::AssetId,
        metadata::Metadata,
        offline::{
            AGGREGATE_PROOF_VERSION_V1, AppleAppAttestProof, OfflineAllowanceCommitment,
            OfflineBalanceProof, OfflinePlatformProof, OfflineWalletPolicy,
        },
    };
    use iroha_crypto::{Hash, KeyPair, Signature};
    use std::{path::Path, str::FromStr};

    #[test]
    fn bundle_loader_handles_json_and_norito_payloads() {
        let transfer = sample_transfer();
        let json_bytes = norito::json::to_vec(&transfer).expect("encode bundle json");
        let decoded =
            parse_bundle_json(&json_bytes, Path::new("bundle.json")).expect("decode bundle json");
        assert_eq!(decoded, transfer);

        let norito_bytes = norito::to_bytes(&transfer).expect("encode bundle norito");
        let decoded = parse_bundle_norito(&norito_bytes, Path::new("bundle.norito"))
            .expect("decode bundle norito");
        assert_eq!(decoded, transfer);
    }

    #[test]
    fn bundle_row_reports_root_match_status() {
        let transfer = sample_transfer();
        let row = build_bundle_row(Path::new("bundle.json"), &transfer, true)
            .expect("build inspection row");
        assert_eq!(row.proof_status, "match");
        assert_eq!(row.receipts_root_matches, Some(true));
        assert!(row.platform_snapshot_policy.is_none());
        let summary = row.proof_summary.expect("proof summary");
        assert_eq!(summary.version, AGGREGATE_PROOF_VERSION_V1);
    }

    #[allow(clippy::too_many_lines)]
    fn sample_transfer() -> OfflineToOnlineTransfer {
        use iroha::data_model::offline::{
            AggregateProofEnvelope, OfflineSpendReceipt, OfflineToOnlineTransfer,
            OfflineWalletCertificate,
        };
        use iroha_core::smartcontracts::isi::offline::{build_balance_proof, compute_commitment};
        use iroha_primitives::numeric::Numeric;

        let controller: AccountId =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                .parse()
                .unwrap();
        let receiver: AccountId =
            "ed0120A98BAFB0663CE08D75EBD506FEC38A84E576A7C9B0897693ED4B04FD9EF2D18D@wonderland"
                .parse()
                .unwrap();
        let deposit = receiver.clone();
        let asset: AssetId = "xor#wonderland#ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
            .parse()
            .unwrap();

        let allowance_amount = Numeric::from_str("500").unwrap();
        let expected_scale = allowance_amount.scale();
        let initial_blinding = scalar_bytes(1);
        let resulting_blinding = scalar_bytes(2);
        let initial_commitment =
            compute_commitment(&allowance_amount, expected_scale, &initial_blinding)
            .expect("initial commitment");
        let allowance_commitment = OfflineAllowanceCommitment {
            asset: asset.clone(),
            amount: allowance_amount.clone(),
            commitment: initial_commitment.clone(),
        };
        let policy = OfflineWalletPolicy {
            max_balance: allowance_amount.clone(),
            max_tx_value: Numeric::from_str("125").unwrap(),
            expires_at_ms: 1_800_000_000_000,
        };

        let operator_keys = KeyPair::random();
        let spend_keys = KeyPair::random();
        let operator_signature =
            Signature::new(operator_keys.private_key(), b"certificate payload");
        let certificate = OfflineWalletCertificate {
            controller: controller.clone(),
            allowance: allowance_commitment.clone(),
            spend_public_key: spend_keys.public_key().clone(),
            attestation_report: vec![1, 2, 3],
            issued_at_ms: 1_600_000_000_000,
            expires_at_ms: 1_900_000_000_000,
            policy,
            operator_signature,
            metadata: Metadata::default(),
            verdict_id: None,
            attestation_nonce: None,
            refresh_at_ms: None,
        };

        let receipt_amount = Numeric::from_str("42").unwrap();
        let platform_proof = OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
            key_id: "shared-key".into(),
            counter: 1,
            assertion: vec![9, 9, 9],
            challenge_hash: Hash::new(b"challenge"),
        });
        let receipt = OfflineSpendReceipt {
            tx_id: Hash::new(b"tx-1"),
            from: controller.clone(),
            to: receiver.clone(),
            asset: asset.clone(),
            amount: receipt_amount.clone(),
            issued_at_ms: 1_600_000_100_000,
            invoice_id: "inv-1".into(),
            platform_proof,
            platform_snapshot: None,
            sender_certificate: certificate.clone(),
            sender_signature: Signature::new(spend_keys.private_key(), b"receipt payload"),
        };

        let receipts = vec![receipt];
        let receipts_root = compute_receipts_root(&receipts).expect("receipts root");
        let aggregate_proof = Some(AggregateProofEnvelope {
            version: AGGREGATE_PROOF_VERSION_V1,
            receipts_root,
            proof_sum: Some(vec![1, 2, 3]),
            proof_counter: None,
            proof_replay: None,
            metadata: Metadata::default(),
        });
        let resulting_value = allowance_amount
            .clone()
            .checked_add(receipt_amount.clone())
            .expect("resulting value");
        let resulting_commitment =
            compute_commitment(&resulting_value, expected_scale, &resulting_blinding)
                .expect("resulting commitment");
        let zk_proof = build_balance_proof(
            &ChainId::from("cli-sample"),
            expected_scale,
            &receipt_amount,
            &resulting_value,
            &initial_commitment,
            &resulting_commitment,
            &initial_blinding,
            &resulting_blinding,
        )
        .expect("balance proof");
        let balance_proof = OfflineBalanceProof {
            initial_commitment: allowance_commitment,
            resulting_commitment,
            claimed_delta: receipt_amount,
            zk_proof: Some(zk_proof),
        };

        OfflineToOnlineTransfer {
            bundle_id: Hash::new(b"bundle"),
            receiver,
            deposit_account: deposit,
            receipts,
            balance_proof,
            aggregate_proof,
            attachments: None,
            platform_snapshot: None,
        }
    }

    fn scalar_bytes(value: u8) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[0] = value;
        bytes
    }
}

fn summary_entry_value(summary: &OfflineCounterSummary) -> norito::json::Value {
    use norito::json::{Map, Number, Value};

    fn number(value: u64) -> Value {
        Value::Number(Number::from(value))
    }

    let mut obj = Map::new();
    obj.insert(
        "certificate_id_hex".into(),
        Value::String(hex::encode(summary.certificate_id.as_ref())),
    );
    obj.insert(
        "controller_id".into(),
        Value::String(summary.controller.to_string()),
    );
    obj.insert(
        "summary_hash_hex".into(),
        Value::String(hex::encode(summary.summary_hash.as_ref())),
    );
    let apple = summary
        .apple_key_counters
        .iter()
        .map(|(key, value)| (key.clone(), number(*value)))
        .collect::<Map>();
    obj.insert("apple_key_counters".into(), Value::Object(apple));
    let android = summary
        .android_series_counters
        .iter()
        .map(|(key, value)| (key.clone(), number(*value)))
        .collect::<Map>();
    obj.insert("android_series_counters".into(), Value::Object(android));
    Value::Object(obj)
}

fn current_timestamp_ms() -> Result<u64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| eyre!("system clock before unix epoch: {err}"))?;
    u64::try_from(now.as_millis()).map_err(|_| eyre!("timestamp exceeded u64::MAX"))
}

impl RejectionStatsArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if let Some(stats) = self.fetch(context)? {
            context.print_data(&stats)
        } else {
            context.println(
                "Offline rejection telemetry is unavailable for the selected Torii profile.",
            )?;
            Ok(())
        }
    }

    fn fetch<C: RunContext>(&self, context: &C) -> Result<Option<OfflineRejectionStatsResponse>> {
        let endpoint = context
            .config()
            .torii_api_url
            .join("v1/offline/rejections")
            .map_err(|err| eyre!("failed to derive /v1/offline/rejections URL: {err}"))?;
        let client = BlockingHttpClient::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|err| eyre!("failed to build HTTP client: {err}"))?;
        let mut request = client
            .get(endpoint.clone())
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(profile) = &self.telemetry_profile {
            let trimmed = profile.trim();
            if trimmed.is_empty() {
                return Err(eyre!("telemetry profile must not be empty"));
            }
            request = request.header("X-Torii-Telemetry-Profile", trimmed);
        }
        let response = request
            .send()
            .map_err(|err| eyre!("failed to fetch {endpoint}: {err}"))?;
        let response_status = response.status();
        if matches!(response_status.as_u16(), 403 | 404 | 503) {
            return Ok(None);
        }
        if !response_status.is_success() {
            return Err(eyre!(
                "Torii returned unexpected status {response_status} for {endpoint}"
            ));
        }
        let bytes = response
            .bytes()
            .map_err(|err| eyre!("failed to read {endpoint} body: {err}"))?;
        if bytes.is_empty() {
            return Err(eyre!("Torii returned an empty payload for {endpoint}"));
        }
        let stats: OfflineRejectionStatsResponse = norito::json::from_slice(&bytes)
            .map_err(|err| eyre!("failed to parse /v1/offline/rejections response: {err}"))?;
        Ok(Some(stats))
    }
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
pub struct OfflineRejectionStatsResponse {
    pub total: u64,
    pub items: Vec<OfflineRejectionStatsItem>,
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
pub struct OfflineRejectionStatsItem {
    pub platform: String,
    pub reason: String,
    pub count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::data_model::{
        asset::AssetDefinitionId,
        domain::DomainId,
        offline::{
            AppleAppAttestProof, OfflineAllowanceCommitment, OfflineBalanceProof,
            OfflineCounterState, OfflinePlatformProof, OfflinePlatformTokenSnapshot,
            OfflineSpendReceipt, OfflineToOnlineTransfer, OfflineTransferRecord,
            OfflineTransferStatus, OfflineWalletCertificate, OfflineWalletPolicy,
        },
        prelude::AccountId,
    };
    use iroha_crypto::{Hash, PublicKey, Signature};
    use norito::json::Value;
    use std::{collections::BTreeMap, str::FromStr};
    use tempfile::tempdir;

    fn default_common_args() -> crate::list_support::CommonArgs {
        crate::list_support::CommonArgs {
            sort_by_metadata_key: None,
            order: None,
            limit: None,
            offset: 0,
            fetch_size: None,
            select: None,
        }
    }

    fn default_all_args() -> crate::list_support::AllArgs {
        crate::list_support::AllArgs {
            verbose: false,
            common: default_common_args(),
        }
    }

    fn default_allowance_list_args() -> AllowanceListArgs {
        AllowanceListArgs {
            common: default_all_args(),
            controller: None,
            verdict_id: None,
            attestation_nonce: None,
            certificate_expires_before_ms: None,
            certificate_expires_after_ms: None,
            policy_expires_before_ms: None,
            policy_expires_after_ms: None,
            refresh_before_ms: None,
            refresh_after_ms: None,
            summary: false,
            include_expired: true,
        }
    }

    fn default_transfer_list_args() -> TransferListArgs {
        TransferListArgs {
            common: default_all_args(),
            controller: None,
            receiver: None,
            status: None,
            certificate_id: None,
            certificate_expires_before_ms: None,
            certificate_expires_after_ms: None,
            policy_expires_before_ms: None,
            policy_expires_after_ms: None,
            refresh_before_ms: None,
            refresh_after_ms: None,
            verdict_id: None,
            attestation_nonce: None,
            platform_policy: None,
            require_verdict: false,
            only_missing_verdict: false,
            audit_log: None,
            summary: false,
        }
    }

    fn sample_allowance_record() -> OfflineAllowanceRecord {
        let controller = sample_account(0xA1, "wonderland");
        let definition = AssetDefinitionId::from_str("xor#wonderland").expect("asset definition");
        let asset = AssetId::new(definition, controller.clone());
        OfflineAllowanceRecord {
            certificate: OfflineWalletCertificate {
                controller,
                allowance: OfflineAllowanceCommitment {
                    asset,
                    amount: Numeric::new(1_000, 0),
                    commitment: vec![0xAA; 32],
                },
                spend_public_key: PublicKey::from_str(
                    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
                )
                .expect("public key"),
                attestation_report: Vec::new(),
                issued_at_ms: 1_700_000_000_000,
                expires_at_ms: 1_900_000_000_000,
                policy: OfflineWalletPolicy {
                    max_balance: Numeric::new(5_000, 0),
                    max_tx_value: Numeric::new(1_000, 0),
                    expires_at_ms: 1_900_000_000_000,
                },
                operator_signature: Signature::from_bytes(&[0; 64]),
                metadata: Metadata::default(),
                verdict_id: Some(Hash::new(b"verdict")),
                attestation_nonce: Some(Hash::new(b"nonce")),
                refresh_at_ms: Some(1_850_000_000_000),
            },
            current_commitment: vec![0xAA; 32],
            registered_at_ms: 1_700_000_500_000,
            remaining_amount: Numeric::new(1_000, 0),
            counter_state: OfflineCounterState::default(),
            verdict_id: Some(Hash::new(b"verdict")),
            attestation_nonce: Some(Hash::new(b"nonce")),
            refresh_at_ms: Some(1_850_000_000_000),
        }
    }

    fn sample_certificate() -> OfflineWalletCertificate {
        sample_allowance_record().certificate
    }

    fn sample_transfer_record(policy: Option<AndroidIntegrityPolicy>) -> OfflineTransferRecord {
        let certificate = sample_certificate();
        let receiver = sample_account(0xB2, "wonderland");
        let deposit_account = sample_account(0xC3, "wonderland");
        let receipt = OfflineSpendReceipt {
            tx_id: Hash::new(b"offline-tx"),
            from: certificate.controller.clone(),
            to: receiver.clone(),
            asset: certificate.allowance.asset.clone(),
            amount: Numeric::new(50, 0),
            issued_at_ms: 1_700_000_600_000,
            invoice_id: "INV-001".into(),
            platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                key_id: "apple-key".into(),
                counter: 1,
                assertion: vec![],
                challenge_hash: Hash::new(b"challenge"),
            }),
            platform_snapshot: None,
            sender_certificate: certificate.clone(),
            sender_signature: Signature::from_bytes(&[0; 64]),
        };
        let transfer = OfflineToOnlineTransfer {
            bundle_id: Hash::new(b"bundle"),
            receiver: receiver.clone(),
            deposit_account,
            receipts: vec![receipt],
            balance_proof: OfflineBalanceProof {
                initial_commitment: certificate.allowance.clone(),
                resulting_commitment: vec![0; 32],
                claimed_delta: Numeric::new(50, 0),
                zk_proof: None,
            },
            aggregate_proof: None,
            attachments: None,
            platform_snapshot: None,
        };
        OfflineTransferRecord {
            transfer,
            controller: certificate.controller.clone(),
            status: OfflineTransferStatus::Settled,
            recorded_at_ms: 1,
            recorded_at_height: 1,
            archived_at_height: None,
            history: Vec::new(),
            pos_verdict_snapshots: vec![OfflineVerdictSnapshot::from_certificate(&certificate)],
            verdict_snapshot: None,
            platform_snapshot: policy.map(|policy| OfflinePlatformTokenSnapshot {
                policy: policy.to_string(),
                attestation_jws_b64: "token".into(),
            }),
        }
    }

    fn sample_summary() -> OfflineCounterSummary {
        let mut apple = BTreeMap::new();
        apple.insert("app.attest:k1".into(), 7);
        let mut android = BTreeMap::new();
        android.insert("series:a".into(), 13);
        OfflineCounterSummary {
            certificate_id: Hash::new(b"certificate"),
            controller: sample_account(0xA1, "wonderland"),
            apple_key_counters: apple,
            android_series_counters: android,
            summary_hash: Hash::new(b"summary"),
        }
    }

    fn sample_account(tag: u8, domain: &str) -> AccountId {
        let domain_id = DomainId::from_str(domain).expect("domain id");
        let mut bytes = [0u8; 32];
        bytes.fill(tag);
        let encoded = format!("ed0120{}", hex::encode(bytes));
        let public_key = PublicKey::from_str(&encoded).expect("public key");
        AccountId::new(domain_id, public_key)
    }

    #[test]
    fn summary_entry_value_contains_expected_fields() {
        let summary = sample_summary();
        let value = summary_entry_value(&summary);
        let object = value.as_object().expect("json object");
        assert_eq!(
            object
                .get("certificate_id_hex")
                .and_then(|v| v.as_str())
                .unwrap(),
            hex::encode(summary.certificate_id.as_ref())
        );
        assert_eq!(
            object
                .get("controller_id")
                .and_then(|v| v.as_str())
                .unwrap(),
            summary.controller.to_string()
        );
        assert_eq!(
            object
                .get("summary_hash_hex")
                .and_then(|v| v.as_str())
                .unwrap(),
            hex::encode(summary.summary_hash.as_ref())
        );
        let apple = object
            .get("apple_key_counters")
            .and_then(|v| v.as_object())
            .expect("apple counters");
        assert_eq!(
            apple.get("app.attest:k1").and_then(Value::as_u64).unwrap(),
            7
        );
        let android = object
            .get("android_series_counters")
            .and_then(|v| v.as_object())
            .expect("android counters");
        assert_eq!(android.get("series:a").and_then(Value::as_u64).unwrap(), 13);
    }

    #[test]
    fn summary_export_envelope_counts_entries() {
        let summary = sample_summary();
        let value = summary_export_envelope(123, std::slice::from_ref(&summary));
        let object = value.as_object().expect("json object");
        assert_eq!(
            object.get("generated_at_ms").unwrap().as_u64().unwrap(),
            123
        );
        assert_eq!(object.get("summary_count").unwrap().as_u64().unwrap(), 1);
        let summaries = object
            .get("summaries")
            .and_then(|v| v.as_array())
            .expect("summaries array");
        assert_eq!(summaries.len(), 1);
    }

    #[test]
    fn transfer_status_arg_converts() {
        assert_eq!(
            TransferStatusArg::Settled.as_status(),
            OfflineTransferStatus::Settled
        );
        assert_eq!(
            TransferStatusArg::Archived.as_status(),
            OfflineTransferStatus::Archived
        );
    }

    #[test]
    fn record_verdict_helper_prefers_record_field() {
        let mut record = sample_allowance_record();
        let extra = Hash::new(b"other");
        record.certificate.verdict_id = Some(extra);
        let verdict = record_verdict_id(&record).expect("verdict id");
        assert_eq!(verdict, record.verdict_id.as_ref().expect("record verdict"));
    }

    #[test]
    fn allowance_filter_by_verdict() {
        let mut entries = vec![sample_allowance_record()];
        let mut args = default_allowance_list_args();
        args.verdict_id = Some(Hash::new(b"verdict"));
        apply_allowance_filters(&mut entries, &args).expect("filters");
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn allowance_filter_drops_expired_when_requested() {
        let mut record = sample_allowance_record();
        record.certificate.expires_at_ms = 1;
        let mut entries = vec![record];
        let mut args = default_allowance_list_args();
        args.include_expired = false;
        apply_allowance_filters(&mut entries, &args).expect("filters");
        assert!(entries.is_empty());
    }

    #[test]
    fn allowance_filter_certificate_expiry_bounds() {
        let record = sample_allowance_record();

        let mut entries = vec![record.clone()];
        let mut args = default_allowance_list_args();
        args.certificate_expires_before_ms = Some(record.certificate.expires_at_ms - 1);
        apply_allowance_filters(&mut entries, &args).expect("filters");
        assert!(entries.is_empty());

        let mut entries = vec![record.clone()];
        let mut args = default_allowance_list_args();
        args.certificate_expires_after_ms = Some(record.certificate.expires_at_ms + 1);
        apply_allowance_filters(&mut entries, &args).expect("filters");
        assert!(entries.is_empty());

        let mut entries = vec![record];
        let mut args = default_allowance_list_args();
        args.certificate_expires_before_ms = Some(1_900_000_000_000);
        args.certificate_expires_after_ms = Some(1_700_000_000_000);
        apply_allowance_filters(&mut entries, &args).expect("filters");
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn allowance_filter_policy_expiry_bounds() {
        let mut record = sample_allowance_record();
        record.certificate.policy.expires_at_ms = 2_000_000_000_000;

        let mut entries = vec![record.clone()];
        let mut args = default_allowance_list_args();
        args.policy_expires_before_ms = Some(1_900_000_000_000);
        apply_allowance_filters(&mut entries, &args).expect("filters");
        assert!(entries.is_empty());

        let mut entries = vec![record.clone()];
        let mut args = default_allowance_list_args();
        args.policy_expires_after_ms = Some(2_100_000_000_000);
        apply_allowance_filters(&mut entries, &args).expect("filters");
        assert!(entries.is_empty());

        let mut entries = vec![record];
        let mut args = default_allowance_list_args();
        args.policy_expires_before_ms = Some(2_000_000_000_000);
        args.policy_expires_after_ms = Some(1_800_000_000_000);
        apply_allowance_filters(&mut entries, &args).expect("filters");
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn allowance_summary_row_contains_metadata() {
        let record = sample_allowance_record();
        let refresh_at = record_refresh_at(&record).unwrap();
        let summary = AllowanceSummaryRow::from_record(&record, refresh_at - 500);
        assert_eq!(
            summary.certificate_id_hex,
            hex::encode(record.certificate_id().as_ref())
        );
        assert_eq!(
            summary.controller_id,
            record.certificate.controller.to_string()
        );
        assert_eq!(summary.refresh_at_ms, record_refresh_at(&record));
        assert_eq!(
            summary.verdict_id_hex.as_deref(),
            record_verdict_id(&record)
                .map(|hash| hex::encode(hash.as_ref()))
                .as_deref()
        );
        assert_eq!(summary.deadline_kind.as_deref(), Some("refresh"));
        assert_eq!(summary.deadline_state.as_deref(), Some("warning"));
        assert_eq!(summary.deadline_ms, Some(refresh_at));
        assert_eq!(summary.deadline_ms_remaining, Some(500));
    }

    #[test]
    fn rejection_stats_response_parses() {
        let payload = br#"{
            "total": 3,
            "items": [
                {"platform": "general", "reason": "allowanceNotRegistered", "count": 2},
                {"platform": "android", "reason": "platformProofInvalid", "count": 1}
            ]
        }"#;
        let stats: OfflineRejectionStatsResponse =
            norito::json::from_slice(payload).expect("stats");
        assert_eq!(stats.total, 3);
        assert_eq!(stats.items.len(), 2);
        assert_eq!(stats.items[0].platform, "general");
        assert_eq!(stats.items[0].reason, "allowanceNotRegistered");
        assert_eq!(stats.items[0].count, 2);
    }

    #[test]
    fn rejection_stats_response_serializes() {
        let stats = OfflineRejectionStatsResponse {
            total: 1,
            items: vec![OfflineRejectionStatsItem {
                platform: "general".into(),
                reason: "deltaMismatch".into(),
                count: 1,
            }],
        };
        let rendered = norito::json::to_json_pretty(&stats).expect("serialize rejection stats");
        assert!(
            rendered.contains("\"total\": 1"),
            "rendered payload should include total"
        );
        assert!(
            rendered.contains("deltaMismatch"),
            "rendered payload should include reason"
        );
    }

    #[test]
    fn transfer_filter_platform_policy_matches_only_snapshot() {
        let mut entries = vec![
            sample_transfer_record(Some(AndroidIntegrityPolicy::PlayIntegrity)),
            sample_transfer_record(Some(AndroidIntegrityPolicy::HmsSafetyDetect)),
            sample_transfer_record(None),
        ];
        let mut args = default_transfer_list_args();
        args.platform_policy = Some(PlatformPolicyArg::PlayIntegrity);
        apply_transfer_filters(&mut entries, &args, TransferFilterFlags::default()).unwrap();
        assert_eq!(entries.len(), 1);
        let snapshot = entries[0].platform_snapshot.as_ref().expect("snapshot");
        assert_eq!(
            snapshot.policy(),
            Some(AndroidIntegrityPolicy::PlayIntegrity)
        );
    }

    #[test]
    fn audit_entry_prefers_receipt_summary() {
        let record = sample_transfer_record(None);
        let entry = OfflineAuditLogEntry::from(&record);
        let receipt = record.transfer.receipts.first().expect("receipt");
        assert_eq!(entry.sender_id, receipt.from.to_string());
        assert_eq!(entry.receiver_id, receipt.to.to_string());
        assert_eq!(entry.asset_id, receipt.asset.to_string());
        assert_eq!(entry.amount, receipt.amount.to_string());
        assert_eq!(entry.timestamp_ms, record.recorded_at_ms);
        assert_eq!(entry.tx_id, hex::encode(record.transfer.bundle_id.as_ref()));
    }

    #[test]
    fn audit_entry_falls_back_when_receipts_missing() {
        let mut record = sample_transfer_record(None);
        record.transfer.receipts.clear();
        record.transfer.balance_proof.claimed_delta = Numeric::new(75, 0);
        let entry = OfflineAuditLogEntry::from(&record);
        assert_eq!(entry.sender_id, record.transfer.receiver.to_string());
        assert_eq!(
            entry.receiver_id,
            record.transfer.deposit_account.to_string()
        );
        assert_eq!(
            entry.asset_id,
            record
                .transfer
                .balance_proof
                .initial_commitment
                .asset
                .to_string()
        );
        assert_eq!(entry.amount, "75");
    }

    #[test]
    fn write_audit_log_writes_expected_json() {
        let record = sample_transfer_record(None);
        let entries = vec![OfflineAuditLogEntry::from(&record)];
        let temp = tempdir().expect("temp dir");
        let path = temp.path().join("audit/log.json");
        write_audit_log(&path, &entries).expect("write audit log");
        let bytes = fs::read(&path).expect("audit log contents");
        let decoded: Vec<OfflineAuditLogEntry> =
            norito::json::from_slice(&bytes).expect("decode audit log JSON");
        assert_eq!(decoded, entries);
    }
}
