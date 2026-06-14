//! ZK convenience commands for the CLI (experimental/app API).
#![allow(
    clippy::struct_excessive_bools,
    clippy::explicit_iter_loop,
    clippy::option_if_let_else,
    clippy::nonminimal_bool,
    clippy::explicit_into_iter_loop,
    clippy::needless_collect,
    clippy::ignored_unit_patterns,
    clippy::too_many_lines,
    clippy::map_unwrap_or,
    clippy::redundant_closure_for_method_calls,
    clippy::redundant_closure,
    clippy::cast_possible_truncation,
    clippy::uninlined_format_args,
    unused_imports
)]
// Current allowances cover the data-munging helpers used by the ZK CLI.
// They stay until the module is decomposed and the conversion helpers are refactored.
//!
//! Provides thin wrappers over Torii app endpoints for ZK features. These are
//! intended for operator/testing convenience and are not consensus-critical.

use eyre::{Context, Result};
// For base64 Engine trait (decode)
use base64::Engine as _;
use iroha::client::{Client, ZkProofsFilter};
use iroha::data_model::prelude::{Executable, InstructionBox};
use iroha_crypto::Hash as CryptoHash;
use iroha_zkp_halo2::OpenVerifyEnvelope as Halo2Envelope;

use crate::{CliOutputFormat, Run, RunContext, json_utils};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Get recent shielded roots for an asset (JSON). Posts to /v1/zk/roots
    Roots(RootsArgs),
    /// Verify a ZK proof by posting an `OpenVerifyEnvelope` (Norito) or a JSON DTO to /v1/zk/verify
    Verify(VerifyArgs),
    /// Submit a ZK proof envelope for later reference/inspection. Posts to /v1/zk/submit-proof
    SubmitProof(SubmitProofArgs),
    /// Verify a batch of ZK `OpenVerify` envelopes (Norito vector) via /v1/zk/verify-batch
    VerifyBatch(VerifyBatchArgs),
    /// Compute the Blake2b-32 hash required for `public_inputs_schema_hash` and print it
    SchemaHash(SchemaHashArgs),
    /// Manage ZK attachments in the app API
    #[command(subcommand)]
    Attachments(AttachmentsCommand),
    /// Register a ZK-capable asset (Hybrid mode) with policy and VK ids
    RegisterAsset(ZkRegisterAssetArgs),
    /// Shield public funds into a shielded ledger (demo flow)
    Shield(ShieldArgs),
    /// Unshield funds from shielded ledger to public (demo flow)
    Unshield(UnshieldArgs),
    /// Verifying-key registry lifecycle (register/update/deprecate/get)
    #[command(subcommand)]
    Vk(VkCommand),
    /// Inspect proof registry (list/count/get)
    #[command(subcommand)]
    Proofs(ProofCommand),
    /// Inspect background prover reports (list/get/delete)
    #[command(subcommand)]
    Prover(ProverCommand),
    /// IVM prove helpers (non-consensus, app API)
    #[command(subcommand)]
    Ivm(IvmCommand),
    /// Kagemusha offline-cash release tooling
    #[command(subcommand)]
    Kagemusha(KagemushaCommand),
    /// ZK Vote helpers (tally)
    #[command(subcommand)]
    Vote(VoteCommand),
    /// Encode a confidential encrypted payload (memo) into Norito bytes/base64
    Envelope(EnvelopeArgs),
}

#[derive(clap::Args, Debug)]
pub struct RootsArgs {
    /// Canonical unprefixed Base58 `AssetDefinitionId`
    #[arg(long, value_name = "ASSET_ID")]
    asset_id: String,
    /// Maximum number of roots to return (0 = server cap)
    #[arg(long, default_value_t = 0)]
    max: u32,
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Roots(args) => args.run(context),
            Command::Verify(args) => args.run(context),
            Command::SubmitProof(args) => args.run(context),
            Command::VerifyBatch(args) => args.run(context),
            Command::SchemaHash(args) => args.run(context),
            Command::Attachments(args) => args.run(context),
            Command::RegisterAsset(args) => args.run(context),
            Command::Shield(args) => args.run(context),
            Command::Unshield(args) => args.run(context),
            Command::Vk(args) => args.run(context),
            Command::Proofs(args) => args.run(context),
            Command::Prover(args) => args.run(context),
            Command::Ivm(args) => args.run(context),
            Command::Kagemusha(args) => args.run(context),
            Command::Vote(args) => args.run(context),
            Command::Envelope(args) => args.run(context),
        }
    }
}

impl Command {
    pub(crate) fn allows_fallback_config(&self) -> bool {
        matches!(
            self,
            Self::Kagemusha(
                KagemushaCommand::LineageKeyArtifacts(_)
                    | KagemushaCommand::RecursiveCompactKeyArtifacts(_)
                    | KagemushaCommand::LineageRecord(_)
            )
        )
    }
}

impl Run for RootsArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.get_zk_roots_json(&self.asset_id, self.max)?;
        context.print_data(&value)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct VerifyArgs {
    /// Path to Norito-encoded `OpenVerifyEnvelope` bytes (mutually exclusive with --json)
    #[arg(long, value_name = "PATH", conflicts_with = "json")]
    norito: Option<std::path::PathBuf>,
    /// Path to a JSON DTO describing the proof (backend, proof, vk) (mutually exclusive with --norito)
    #[arg(long, value_name = "PATH", conflicts_with = "norito")]
    json: Option<std::path::PathBuf>,
}

impl Run for VerifyArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        if let Some(p) = self.norito {
            let body = std::fs::read(&p)?;
            let value = client.post_zk_verify_norito(&body)?;
            context.print_data(&value)?;
            return Ok(());
        }
        if let Some(p) = self.json {
            let s = std::fs::read_to_string(&p)?;
            let v: norito::json::Value = norito::json::from_str(&s)?;
            let value = client.post_zk_verify_json(&v)?;
            context.print_data(&value)?;
            return Ok(());
        }
        eyre::bail!("provide either --norito <file> or --json <file>");
    }
}

#[derive(clap::Args, Debug)]
pub struct SubmitProofArgs {
    /// Path to Norito-encoded proof envelope bytes (mutually exclusive with --json)
    #[arg(long, value_name = "PATH", conflicts_with = "json")]
    norito: Option<std::path::PathBuf>,
    /// Path to a JSON DTO describing the proof (backend, proof, vk) (mutually exclusive with --norito)
    #[arg(long, value_name = "PATH", conflicts_with = "norito")]
    json: Option<std::path::PathBuf>,
}

impl Run for SubmitProofArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        if let Some(p) = self.norito {
            let body = std::fs::read(&p)?;
            let value = client.post_zk_submit_proof_norito(&body)?;
            context.print_data(&value)?;
            return Ok(());
        }
        if let Some(p) = self.json {
            let s = std::fs::read_to_string(&p)?;
            let v: norito::json::Value = norito::json::from_str(&s)?;
            let value = client.post_zk_submit_proof_json(&v)?;
            context.print_data(&value)?;
            return Ok(());
        }
        eyre::bail!("provide either --norito <file> or --json <file>");
    }
}

#[derive(clap::Args, Debug)]
pub struct VerifyBatchArgs {
    /// Path to a Norito-encoded Vec<OpenVerifyEnvelope> (mutually exclusive with --json)
    #[arg(long, value_name = "PATH", conflicts_with = "json")]
    norito: Option<std::path::PathBuf>,
    /// Path to a JSON array of base64-encoded Norito `OpenVerifyEnvelope` items (mutually exclusive with --norito)
    #[arg(long, value_name = "PATH", conflicts_with = "norito")]
    json: Option<std::path::PathBuf>,
}

impl Run for VerifyBatchArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        if let Some(p) = self.norito {
            let body = std::fs::read(&p)?;
            let value = client.post_zk_verify_batch_norito(&body)?;
            context.print_data(&value)?;
            return Ok(());
        }
        if let Some(p) = self.json {
            let s = std::fs::read_to_string(&p)?;
            let v: norito::json::Value = norito::json::from_str(&s)?;
            let value = client.post_zk_verify_batch_json(&v)?;
            context.print_data(&value)?;
            return Ok(());
        }
        eyre::bail!("provide either --norito <file> or --json <file>");
    }
}

#[derive(clap::Args, Debug)]
pub struct SchemaHashArgs {
    /// Path to a Norito-encoded `OpenVerifyEnvelope`
    #[arg(long, value_name = "PATH", conflicts_with = "public_inputs_hex")]
    norito: Option<std::path::PathBuf>,
    /// Hex-encoded public inputs (when not using --norito)
    #[arg(long, value_name = "HEX", conflicts_with = "norito")]
    public_inputs_hex: Option<String>,
}

impl Run for SchemaHashArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = if let Some(path) = self.norito {
            let raw = std::fs::read(&path)?;
            let env: Halo2Envelope = norito::decode_from_bytes(&raw)
                .map_err(|e| eyre::eyre!("failed to decode Norito envelope: {e}"))?;
            env.public.encode_bytes()
        } else if let Some(hex) = self.public_inputs_hex {
            parse_hex_string(&hex)?
        } else {
            eyre::bail!("provide either --norito <file> or --public-inputs-hex <hex>");
        };
        let hash: [u8; 32] = CryptoHash::new(&bytes).into();
        context.println(hex::encode(hash))?;
        Ok(())
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum ProofCommand {
    /// List proof records maintained by Torii.
    List(ProofListArgs),
    /// Count proof records matching the filters.
    Count(ProofCountArgs),
    /// Fetch a proof record by backend and proof hash (hex).
    Get(ProofGetArgs),
    /// Inspect proof retention configuration and live counters.
    Retention(ProofRetentionArgs),
    /// Submit a pruning transaction to enforce proof retention immediately.
    Prune(ProofPruneArgs),
}

impl Run for ProofCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            ProofCommand::List(args) => args.run(context),
            ProofCommand::Count(args) => args.run(context),
            ProofCommand::Get(args) => args.run(context),
            ProofCommand::Retention(args) => args.run(context),
            ProofCommand::Prune(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug, Clone, Default)]
pub struct ProofFilterArgs {
    /// Filter by backend identifier (e.g., `halo2/ipa`).
    #[arg(long, value_name = "BACKEND")]
    backend: Option<String>,
    /// Filter by verification status (`Submitted`, `Verified`, `Rejected`).
    #[arg(long, value_name = "STATUS")]
    status: Option<String>,
    /// Require a ZK1 TLV tag (4 ASCII characters, e.g., `PROF`).
    #[arg(long, value_name = "TAG")]
    has_tag: Option<String>,
    /// Minimum verification height (inclusive).
    #[arg(long, value_name = "HEIGHT")]
    verified_from_height: Option<u64>,
    /// Maximum verification height (inclusive).
    #[arg(long, value_name = "HEIGHT")]
    verified_until_height: Option<u64>,
    /// Limit result size (server caps at 1000).
    #[arg(long, value_name = "LIMIT")]
    limit: Option<u32>,
    /// Offset for server-side pagination.
    #[arg(long, value_name = "OFFSET")]
    offset: Option<u32>,
    /// Sort order (`asc` or `desc`) by verification height.
    #[arg(long, value_name = "ORDER")]
    order: Option<String>,
}

impl ProofFilterArgs {
    fn as_filter(&self) -> Result<ZkProofsFilter<'_>> {
        if let Some(backend) = self.backend.as_deref() {
            ensure_production_verify_backend_label(backend, "proof filter backend")?;
        }
        Ok(ZkProofsFilter {
            backend: self.backend.as_deref(),
            status: self.status.as_deref(),
            has_tag: self.has_tag.as_deref(),
            verified_from_height: self.verified_from_height,
            verified_until_height: self.verified_until_height,
            limit: self.limit,
            offset: self.offset,
            order: self.order.as_deref(),
            ids_only: None,
        })
    }
}

#[derive(clap::Args, Debug)]
pub struct ProofListArgs {
    #[command(flatten)]
    filter: ProofFilterArgs,
    /// Return only `{ backend, hash }` identifiers.
    #[arg(long)]
    ids_only: bool,
}

impl Run for ProofListArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let mut filter = self.filter.as_filter()?;
        if self.ids_only {
            filter.ids_only = Some(true);
        }
        let value = client.get_zk_proofs_list_filtered(&filter)?;
        context.print_data(&value)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct ProofCountArgs {
    #[command(flatten)]
    filter: ProofFilterArgs,
}

impl Run for ProofCountArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let filter = self.filter.as_filter()?;
        let count = client.get_zk_proofs_count(&filter)?;
        let value = json_utils::json_object(vec![("count", json_utils::json_value(&count)?)])?;
        context.print_data(&value)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct ProofGetArgs {
    /// Backend identifier (e.g., `halo2/ipa`).
    #[arg(long, value_name = "BACKEND")]
    backend: String,
    /// Proof hash (hex, with or without `0x` prefix).
    #[arg(long, value_name = "HASH")]
    hash: String,
}

impl Run for ProofGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let backend = ensure_production_verify_backend_label(&self.backend, "proof get backend")?;
        let hash_hex = parse_hex32_lower(&self.hash, "proof hash")?;
        let value = client.get_zk_proof_json(backend, &hash_hex)?;
        context.print_data(&value)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct ProofRetentionArgs {}

impl Run for ProofRetentionArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let status = client.get_proof_retention_status()?;
        context.print_data(&status)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct ProofPruneArgs {
    /// Restrict pruning to a single backend (e.g., `halo2/ipa`). Omit to prune all backends.
    #[arg(long, value_name = "BACKEND")]
    backend: Option<String>,
}

impl Run for ProofPruneArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if let Some(backend) = self.backend.as_deref() {
            ensure_production_verify_backend_label(backend, "proof prune backend")?;
        }
        let prune: InstructionBox =
            iroha_data_model::isi::zk::PruneProofs::new(self.backend).into();
        context.finish(Executable::Instructions(vec![prune].into()))
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum ProverCommand {
    /// Manage prover reports
    #[command(subcommand)]
    Reports(ProverReportsCommand),
}

impl Run for ProverCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            ProverCommand::Reports(cmd) => cmd.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum ProverReportsCommand {
    /// List available prover reports (JSON array)
    List(ProverReportsListArgs),
    /// Get a single prover report by id (JSON)
    Get(ProverReportsGetArgs),
    /// Delete a prover report by id
    Delete(ProverReportsDeleteArgs),
    /// Cleanup reports in bulk (apply filters, delete matches)
    Cleanup(ProverReportsCleanupArgs),
    /// Count reports matching filters (server-side)
    Count(ProverReportsCountArgs),
}

impl Run for ProverReportsCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            ProverReportsCommand::List(args) => args.run(context),
            ProverReportsCommand::Get(args) => args.run(context),
            ProverReportsCommand::Delete(args) => args.run(context),
            ProverReportsCommand::Cleanup(args) => args.run(context),
            ProverReportsCommand::Count(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct ProverReportsListArgs {
    /// Print a one-line summary per report (id, ok, `content_type`, `zk1_tags`)
    #[arg(long)]
    summary: bool,
    /// Show only successful reports
    #[arg(long, conflicts_with = "failed_only")]
    ok_only: bool,
    /// Show only failed reports
    #[arg(long, conflicts_with = "ok_only")]
    failed_only: bool,
    /// Alias for failed-only (errors have ok=false)
    #[arg(long, hide_short_help = true)]
    errors_only: bool,
    /// Filter by exact id (hex)
    #[arg(long, value_name = "ID")]
    id: Option<String>,
    /// Filter by content-type substring (e.g., application/x-norito)
    #[arg(long, value_name = "MIME")]
    content_type: Option<String>,
    /// Filter reports that contain a ZK1 tag (e.g., PROF, IPAK)
    #[arg(long, value_name = "TAG")]
    has_tag: Option<String>,
    /// Return only ids (server-side projection)
    #[arg(long)]
    ids_only: bool,
    /// Return only `{ id, error }` objects for failed reports (server-side projection)
    #[arg(long)]
    messages_only: bool,
    /// Project returned fields (client-side) from full objects, comma-separated (e.g., "`id,ok,content_type,processed_ms`"). Ignored with --summary/--ids-only/--messages-only.
    #[arg(long, value_name = "CSV")]
    fields: Option<String>,
    /// Limit number of reports returned (server-side). Max 1000.
    #[arg(long, value_name = "N")]
    limit: Option<u32>,
    /// Only reports with `processed_ms` >= this value (server-side)
    #[arg(long, value_name = "MS")]
    since_ms: Option<u64>,
    /// Only reports with `processed_ms` <= this value (server-side)
    #[arg(long, value_name = "MS")]
    before_ms: Option<u64>,
    /// Result ordering: asc (default) or desc
    #[arg(long, value_name = "ORDER", default_value = "asc")]
    order: String,
    /// Offset after ordering/filtering (server-side)
    #[arg(long, value_name = "N")]
    offset: Option<u32>,
    /// Return only the latest report after filters
    #[arg(long)]
    latest: bool,
    // (duplicate removed)
}

impl Run for ProverReportsListArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let use_server_filters = self.ok_only
            || self.failed_only
            || self.errors_only
            || self.messages_only
            || self.id.is_some()
            || self.content_type.is_some()
            || self.has_tag.is_some()
            || self.ids_only;
        let value = if use_server_filters
            || self.limit.is_some()
            || self.since_ms.is_some()
            || self.before_ms.is_some()
            || self.offset.is_some()
            || !self.order.is_empty()
            || self.latest
        {
            let filter = iroha::client::ZkProverReportsFilter {
                ok_only: Some(self.ok_only).filter(|v| *v),
                failed_only: Some(self.failed_only).filter(|v| *v),
                errors_only: Some(self.messages_only || self.errors_only).filter(|v| *v),
                id: self.id.as_deref(),
                content_type: self.content_type.as_deref(),
                has_tag: self.has_tag.as_deref(),
                limit: self.limit,
                since_ms: self.since_ms,
                before_ms: self.before_ms,
                ids_only: Some(self.ids_only).filter(|v| *v),
                order: Some(self.order.as_str()),
                offset: self.offset,
                latest: Some(self.latest).filter(|v| *v),
                messages_only: Some(self.messages_only).filter(|v| *v),
            };
            client.get_zk_prover_reports_list_filtered(&filter)?
        } else {
            client.get_zk_prover_reports_list()?
        };
        // If projection requested, print raw array and return
        if self.ids_only || self.messages_only {
            context.print_data(&value)?;
            return Ok(());
        }
        let arr = value
            .as_array()
            .cloned()
            .ok_or_else(|| eyre::eyre!("expected array from /v1/zk/prover/reports"))?;
        // Apply filters
        let mut filtered = Vec::new();
        for v in arr.iter() {
            let id_ok = match &self.id {
                Some(needle) => v.get("id").and_then(|x| x.as_str()) == Some(needle.as_str()),
                None => true,
            };
            if !id_ok {
                continue;
            }
            let ok_flag = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
            if self.ok_only && !ok_flag {
                continue;
            }
            if self.failed_only && ok_flag {
                continue;
            }
            let ct_ok = match &self.content_type {
                Some(substr) => v
                    .get("content_type")
                    .and_then(|x| x.as_str())
                    .map(|ct| ct.contains(substr))
                    .unwrap_or(false),
                None => true,
            };
            if !ct_ok {
                continue;
            }
            let tag_ok = match &self.has_tag {
                Some(tag) => v
                    .get("zk1_tags")
                    .and_then(|x| x.as_array())
                    .map(|a| a.iter().any(|t| t.as_str() == Some(tag.as_str())))
                    .unwrap_or(false),
                None => true,
            };
            if !tag_ok {
                continue;
            }
            filtered.push(v.clone());
        }

        if !self.summary {
            // Apply client-side field projection if requested
            if let Some(csv) = &self.fields {
                let want: Vec<&str> = csv
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                let mut out = Vec::with_capacity(filtered.len());
                for v in filtered {
                    if let Some(obj) = v.as_object() {
                        let mut m = norito::json::Map::new();
                        for k in &want {
                            if let Some(val) = obj.get(*k) {
                                m.insert((*k).to_string(), val.clone());
                            }
                        }
                        out.push(norito::json::Value::Object(m));
                    }
                }
                context.print_data(&norito::json::Value::from(out))?;
            } else {
                context.print_data(&norito::json::Value::from(filtered))?;
            }
            return Ok(());
        }
        for v in filtered {
            let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("");
            let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
            let ct = v.get("content_type").and_then(|x| x.as_str()).unwrap_or("");
            let tags = v
                .get("zk1_tags")
                .and_then(|x| x.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_default();
            context.println(format!("id={id} ok={ok} ct={ct} tags=[{tags}]"))?;
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct ProverReportsCountArgs {
    /// Show only successful reports
    #[arg(long, conflicts_with = "failed_only")]
    ok_only: bool,
    /// Show only failed reports
    #[arg(long, conflicts_with = "ok_only")]
    failed_only: bool,
    /// Alias for failed-only (errors have ok=false)
    #[arg(long, hide_short_help = true)]
    errors_only: bool,
    /// Filter by exact id (hex)
    #[arg(long, value_name = "ID")]
    id: Option<String>,
    /// Filter by content-type substring (e.g., application/x-norito)
    #[arg(long, value_name = "MIME")]
    content_type: Option<String>,
    /// Filter reports that contain a ZK1 tag (e.g., PROF, IPAK)
    #[arg(long, value_name = "TAG")]
    has_tag: Option<String>,
    /// Only reports with `processed_ms` >= this value (server-side)
    #[arg(long, value_name = "MS")]
    since_ms: Option<u64>,
    /// Only reports with `processed_ms` <= this value (server-side)
    #[arg(long, value_name = "MS")]
    before_ms: Option<u64>,
}

impl Run for ProverReportsCountArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let filter = iroha::client::ZkProverReportsFilter {
            ok_only: Some(self.ok_only).filter(|v| *v),
            failed_only: Some(self.failed_only || self.errors_only).filter(|v| *v),
            errors_only: None,
            id: self.id.as_deref(),
            content_type: self.content_type.as_deref(),
            has_tag: self.has_tag.as_deref(),
            limit: None,
            since_ms: self.since_ms,
            before_ms: self.before_ms,
            ids_only: None,
            order: None,
            offset: None,
            latest: None,
            messages_only: None,
        };
        let count = client.get_zk_prover_reports_count(&filter)?;
        match context.output_format() {
            CliOutputFormat::Json => context.print_data(&count)?,
            CliOutputFormat::Text => context.println(count.to_string())?,
        }
        Ok(())
    }
}

#[cfg(test)]
mod prover_list_tests {
    use super::*;

    fn sample_reports() -> norito::json::Value {
        let json = r#"[
            {"id":"a","ok":true,"content_type":"application/json"},
            {"id":"b","ok":false,"content_type":"application/x-norito","zk1_tags":["PROF"]},
            {"id":"c","ok":true,"content_type":"application/x-norito","zk1_tags":["PROF","IPAK"]}
        ]"#;
        norito::json::from_str(json).expect("sample reports")
    }

    #[test]
    fn filter_ok_only() {
        let arr = sample_reports().as_array().unwrap().clone();
        // Simulate filtering logic by reusing code paths (extract; run manual inline filter)
        let filtered: Vec<_> = arr
            .into_iter()
            .filter(|v| v.get("ok").and_then(|x| x.as_bool()) == Some(true))
            .collect();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_by_tag() {
        let arr = sample_reports().as_array().unwrap().clone();
        let tag = "IPAK";
        let filtered: Vec<_> = arr
            .into_iter()
            .filter(|v| {
                v.get("zk1_tags")
                    .and_then(|x| x.as_array())
                    .map(|a| a.iter().any(|t| t.as_str() == Some(tag)))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].get("id").and_then(|x| x.as_str()), Some("c"));
    }
}

#[derive(clap::Args, Debug)]
pub struct ProverReportsCleanupArgs {
    /// Proceed without confirmation (dangerous)
    #[arg(long)]
    yes: bool,
    /// Show only successful reports
    #[arg(long, conflicts_with = "failed_only")]
    ok_only: bool,
    /// Show only failed reports
    #[arg(long, conflicts_with = "ok_only")]
    failed_only: bool,
    /// Alias for failed-only (errors have ok=false)
    #[arg(long, hide_short_help = true)]
    errors_only: bool,
    /// Filter by exact id (hex)
    #[arg(long, value_name = "ID")]
    id: Option<String>,
    /// Filter by content-type substring (e.g., application/x-norito)
    #[arg(long, value_name = "MIME")]
    content_type: Option<String>,
    /// Filter reports that contain a ZK1 tag (e.g., PROF, IPAK)
    #[arg(long, value_name = "TAG")]
    has_tag: Option<String>,
    /// Limit number of reports returned (server-side). Max 1000.
    #[arg(long, value_name = "N")]
    limit: Option<u32>,
    /// Only reports with `processed_ms` >= this value (server-side)
    #[arg(long, value_name = "MS")]
    since_ms: Option<u64>,
    /// Only reports with `processed_ms` <= this value (server-side)
    #[arg(long, value_name = "MS")]
    before_ms: Option<u64>,
    /// Use server-side bulk deletion instead of client-side delete loop
    #[arg(long)]
    server: bool,
}

impl Run for ProverReportsCleanupArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let use_server_filters = self.ok_only
            || self.failed_only
            || self.id.is_some()
            || self.content_type.is_some()
            || self.has_tag.is_some();
        let value = if use_server_filters
            || self.limit.is_some()
            || self.since_ms.is_some()
            || self.before_ms.is_some()
        {
            let filter = iroha::client::ZkProverReportsFilter {
                ok_only: if self.ok_only { Some(true) } else { None },
                failed_only: if self.failed_only { Some(true) } else { None },
                errors_only: if self.errors_only { Some(true) } else { None },
                id: self.id.as_deref(),
                content_type: self.content_type.as_deref(),
                has_tag: self.has_tag.as_deref(),
                limit: self.limit,
                since_ms: self.since_ms,
                before_ms: self.before_ms,
                ids_only: None,
                order: None,
                offset: None,
                latest: None,
                messages_only: None,
            };
            client.get_zk_prover_reports_list_filtered(&filter)?
        } else {
            client.get_zk_prover_reports_list()?
        };
        let arr = value
            .as_array()
            .cloned()
            .ok_or_else(|| eyre::eyre!("expected array from /v1/zk/prover/reports"))?;
        // Server-side deletion path
        if self.server {
            if !self.yes {
                context.println("Pass --yes to confirm server-side deletion.")?;
                context.println(format!(
                    "Matched ~{} report(s) (server-side filter)",
                    arr.len()
                ))?;
                return Ok(());
            }
            let filter = iroha::client::ZkProverReportsFilter {
                ok_only: Some(self.ok_only).filter(|v| *v),
                failed_only: Some(self.failed_only).filter(|v| *v),
                errors_only: Some(self.errors_only).filter(|v| *v),
                id: self.id.as_deref(),
                content_type: self.content_type.as_deref(),
                has_tag: self.has_tag.as_deref(),
                limit: None,
                since_ms: self.since_ms,
                before_ms: self.before_ms,
                ids_only: None,
                order: None,
                offset: None,
                latest: None,
                messages_only: None,
            };
            let deleted = client.delete_zk_prover_reports_filtered(&filter)?;
            context.println(format!("Deleted {deleted}"))?;
            return Ok(());
        }
        let mut ids: Vec<String> = Vec::new();
        for v in arr.iter() {
            let id_ok = match &self.id {
                Some(needle) => v.get("id").and_then(|x| x.as_str()) == Some(needle.as_str()),
                None => true,
            };
            if !id_ok {
                continue;
            }
            let ok_flag = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
            if self.ok_only && !ok_flag {
                continue;
            }
            if self.failed_only && ok_flag {
                continue;
            }
            let ct_ok = match &self.content_type {
                Some(substr) => v
                    .get("content_type")
                    .and_then(|x| x.as_str())
                    .map(|ct| ct.contains(substr))
                    .unwrap_or(false),
                None => true,
            };
            if !ct_ok {
                continue;
            }
            let tag_ok = match &self.has_tag {
                Some(tag) => v
                    .get("zk1_tags")
                    .and_then(|x| x.as_array())
                    .map(|a| a.iter().any(|t| t.as_str() == Some(tag.as_str())))
                    .unwrap_or(false),
                None => true,
            };
            if !tag_ok {
                continue;
            }
            if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                ids.push(id.to_string());
            }
        }

        // Sort ids for deterministic deletion order
        ids.sort();
        context.println(format!("Matched {} report(s)", ids.len()))?;
        if ids.is_empty() {
            return Ok(());
        }
        if !self.yes {
            context.println("Pass --yes to confirm deletion.")?;
            for id in &ids {
                context.println(format!("  would delete: {id}"))?;
            }
            return Ok(());
        }
        for id in ids {
            client.delete_zk_prover_report(&id)?;
            context.println(format!("Deleted {id}"))?;
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct ProverReportsGetArgs {
    /// Report id (attachment id)
    #[arg(long, value_name = "ID")]
    id: String,
}

impl Run for ProverReportsGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.get_zk_prover_report_json(&self.id)?;
        context.print_data(&value)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct ProverReportsDeleteArgs {
    /// Report id (attachment id)
    #[arg(long, value_name = "ID")]
    id: String,
}

impl Run for ProverReportsDeleteArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        client.delete_zk_prover_report(&self.id)?;
        context.println("Deleted")?;
        Ok(())
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum IvmCommand {
    /// Derive an `IvmProved` payload via `/v1/zk/ivm/derive`
    Derive(IvmDeriveArgs),
    /// Submit a prove job for an `IvmProved` payload via `/v1/zk/ivm/prove`
    Prove(IvmProveArgs),
    /// Get a prove job status via `/v1/zk/ivm/prove/{job_id}`
    Get(IvmProveGetArgs),
    /// Delete a prove job via `/v1/zk/ivm/prove/{job_id}`
    Delete(IvmProveDeleteArgs),
    /// Derive a circuit/vk-bound proving key archive (.pk) from verifying key bytes (.vk) for the Halo2 IPA IVM bind circuit
    DerivePk(IvmDerivePkArgs),
}

impl Run for IvmCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            IvmCommand::Derive(args) => args.run(context),
            IvmCommand::Prove(args) => args.run(context),
            IvmCommand::Get(args) => args.run(context),
            IvmCommand::Delete(args) => args.run(context),
            IvmCommand::DerivePk(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct IvmDeriveArgs {
    /// Path to a JSON request DTO `{ vk_ref, authority, metadata, bytecode }`
    #[arg(long, value_name = "PATH")]
    json: std::path::PathBuf,
}

impl Run for IvmDeriveArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let s = std::fs::read_to_string(&self.json)?;
        let req: norito::json::Value = norito::json::from_str(&s)?;
        let value = client.post_zk_ivm_derive_json(&req)?;
        context.print_data(&value)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct IvmProveArgs {
    /// Path to a JSON request DTO `{ vk_ref, authority, metadata, bytecode, proved? }`
    #[arg(long, value_name = "PATH")]
    json: std::path::PathBuf,
    /// Poll the job until it reaches `done` or `error`
    #[arg(long)]
    wait: bool,
    /// Poll interval (milliseconds) when using --wait
    #[arg(long, default_value_t = 250)]
    poll_interval_ms: u64,
    /// Optional timeout (seconds) when using --wait (0 = no timeout)
    #[arg(long, default_value_t = 0)]
    timeout_secs: u64,
}

impl Run for IvmProveArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let s = std::fs::read_to_string(&self.json)?;
        let req: norito::json::Value = norito::json::from_str(&s)?;
        let created = client.post_zk_ivm_prove_json(&req)?;
        if !self.wait {
            context.print_data(&created)?;
            return Ok(());
        }
        let job_id = created
            .get("job_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("response missing job_id"))?
            .to_string();

        let started = std::time::Instant::now();
        let poll = std::time::Duration::from_millis(self.poll_interval_ms.max(10));
        let timeout =
            (self.timeout_secs > 0).then(|| std::time::Duration::from_secs(self.timeout_secs));

        loop {
            if let Some(timeout) = timeout
                && started.elapsed() >= timeout
            {
                eyre::bail!("timed out waiting for ivm prove job {job_id}");
            }
            let status = client.get_zk_ivm_prove_job_json(&job_id)?;
            let label = status
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            match label {
                "pending" | "running" => std::thread::sleep(poll),
                "done" | "error" => {
                    context.print_data(&status)?;
                    return Ok(());
                }
                other => eyre::bail!("unexpected job status `{other}` for job {job_id}"),
            }
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct IvmProveGetArgs {
    /// Prove job id returned by `iroha zk ivm prove`
    #[arg(long, value_name = "JOB_ID")]
    job_id: String,
}

impl Run for IvmProveGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.get_zk_ivm_prove_job_json(&self.job_id)?;
        context.print_data(&value)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct IvmProveDeleteArgs {
    /// Prove job id returned by `iroha zk ivm prove`
    #[arg(long, value_name = "JOB_ID")]
    job_id: String,
}

impl Run for IvmProveDeleteArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.delete_zk_ivm_prove_job_json(&self.job_id)?;
        context.print_data(&value)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct IvmDerivePkArgs {
    /// Backend label for the verifying key bytes (must match Torii `vk_ref.backend`), e.g. `halo2/ipa`
    #[arg(long, default_value = "halo2/ipa", value_name = "BACKEND")]
    backend: String,
    /// Path to verifying key bytes (`.vk`) in Halo2 "processed" format
    #[arg(long, value_name = "PATH")]
    vk: std::path::PathBuf,
    /// Output path for circuit/vk-bound Norito proving key archive (`.pk`)
    #[arg(long, value_name = "PATH")]
    out: std::path::PathBuf,
}

impl Run for IvmDerivePkArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let vk_bytes = std::fs::read(&self.vk)?;
        let vk_box = iroha::data_model::proof::VerifyingKeyBox::new(self.backend, vk_bytes);
        let pk = iroha_core::zk::derive_halo2_ipa_ivm_execution_proving_key_bytes(&vk_box)
            .map_err(|err| {
                eyre::eyre!("failed to derive proving key bytes from verifying key bytes: {err}")
            })?;
        std::fs::write(&self.out, &pk)?;
        context.println(format!(
            "Wrote {} bytes to {}",
            pk.len(),
            self.out.display()
        ))?;
        Ok(())
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum KagemushaCommand {
    /// Generate portable Reserved-lineage verifier/proving key artifacts
    LineageKeyArtifacts(KagemushaLineageKeyArtifactsArgs),
    /// Generate ABI-7 recursive compact verifier/proving key artifacts
    RecursiveCompactKeyArtifacts(KagemushaRecursiveCompactKeyArtifactsArgs),
    /// Build a Reserved-lineage verifier record from an existing verifier key file
    LineageRecord(KagemushaLineageRecordArgs),
}

impl Run for KagemushaCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            KagemushaCommand::LineageKeyArtifacts(args) => args.run(context),
            KagemushaCommand::RecursiveCompactKeyArtifacts(args) => args.run(context),
            KagemushaCommand::LineageRecord(args) => args.run(context),
        }
    }
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum KagemushaLineageKeyProfile {
    /// First-hop Reserved-lineage init proof profile
    Init,
    /// Multi-hop Reserved-lineage append proof profile
    Append,
}

impl KagemushaLineageKeyProfile {
    fn circuit_id(self) -> &'static str {
        match self {
            Self::Init => iroha::data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            Self::Append => iroha::data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Append => "append",
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct KagemushaLineageKeyArtifactsArgs {
    /// Reserved-lineage proof profile to generate
    #[arg(long, value_enum)]
    profile: KagemushaLineageKeyProfile,
    /// Supported Pallas IPA opening vector length for the key pair
    #[arg(long, value_name = "LEN")]
    opening_len: u32,
    /// Output path for Norito `KagemushaRecursiveSpendLineageKeyArtifactsV1`
    #[arg(long, value_name = "PATH")]
    out: std::path::PathBuf,
    /// Optional output path for the verifier key envelope bytes
    #[arg(long, value_name = "PATH")]
    vk_out: Option<std::path::PathBuf>,
    /// Optional output path for the proving key archive bytes
    #[arg(long, value_name = "PATH")]
    pk_out: Option<std::path::PathBuf>,
    /// Optional output path for a Norito `VerifyingKeyRecord`
    #[arg(long, value_name = "PATH")]
    record_out: Option<std::path::PathBuf>,
    /// Namespace to embed in `--record-out`
    #[arg(long, default_value = "offline_kagemusha")]
    record_namespace: String,
    /// Governance version to embed in `--record-out`
    #[arg(long, default_value_t = 1)]
    record_version: u32,
}

#[derive(clap::Args, Debug)]
pub struct KagemushaRecursiveCompactKeyArtifactsArgs {
    /// Output path for ABI-7 recursive compact verifier key envelope bytes
    #[arg(long, value_name = "PATH")]
    vk_out: std::path::PathBuf,
    /// Output path for ABI-7 recursive compact proving key archive bytes
    #[arg(long, value_name = "PATH")]
    pk_out: std::path::PathBuf,
    /// Output path for Norito `KagemushaRecursiveCompactKeyArtifactsV1`
    #[arg(long, value_name = "PATH", required = true)]
    key_artifacts_out: Option<std::path::PathBuf>,
    /// Output path for Norito `KagemushaRecursiveCompactVerifierKeysV1`
    #[arg(long, value_name = "PATH", required = true)]
    verifier_keys_out: Option<std::path::PathBuf>,
    /// Optional output path for a Norito `VerifyingKeyRecord`
    #[arg(long, value_name = "PATH")]
    record_out: Option<std::path::PathBuf>,
    /// Namespace to embed in `--record-out`
    #[arg(long, default_value = "offline_kagemusha")]
    record_namespace: String,
    /// Governance version to embed in `--record-out`
    #[arg(long, default_value_t = 1)]
    record_version: u32,
}

#[derive(clap::Args, Debug)]
pub struct KagemushaLineageRecordArgs {
    /// Reserved-lineage proof profile for the verifier key
    #[arg(long, value_enum)]
    profile: KagemushaLineageKeyProfile,
    /// Supported Pallas IPA opening vector length for the verifier key
    #[arg(long, value_name = "LEN")]
    opening_len: u32,
    /// Path to the verifier key envelope bytes
    #[arg(long, value_name = "PATH")]
    vk: std::path::PathBuf,
    /// Output path for a Norito `VerifyingKeyRecord`
    #[arg(long, value_name = "PATH")]
    out: std::path::PathBuf,
    /// Namespace to embed in the record
    #[arg(long, default_value = "offline_kagemusha")]
    record_namespace: String,
    /// Governance version to embed in the record
    #[arg(long, default_value_t = 1)]
    record_version: u32,
}

fn kagemusha_lineage_vk_record_from_bytes(
    profile: KagemushaLineageKeyProfile,
    namespace: String,
    version: u32,
    opening_len: u32,
    vk_bytes: Vec<u8>,
) -> Result<iroha::data_model::proof::VerifyingKeyRecord> {
    let vk_box = iroha::data_model::proof::VerifyingKeyBox::new(
        iroha_core::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
        vk_bytes,
    );
    match profile {
        KagemushaLineageKeyProfile::Init => {
            iroha_core::zk::kagemusha_recursive_spend_lineage_vk_record_from_box(
                namespace,
                version,
                opening_len,
                vk_box,
            )
        }
        KagemushaLineageKeyProfile::Append => {
            iroha_core::zk::kagemusha_recursive_spend_lineage_append_vk_record_from_box(
                namespace,
                version,
                opening_len,
                vk_box,
            )
        }
    }
    .map_err(|err| {
        eyre::eyre!(
            "failed to build {} Reserved-lineage verifier record for opening length {}: {err}",
            profile.label(),
            opening_len
        )
    })
}

#[cfg(test)]
fn kagemusha_recursive_compact_vk_record_from_bytes(
    namespace: String,
    version: u32,
    vk_bytes: Vec<u8>,
) -> Result<iroha::data_model::proof::VerifyingKeyRecord> {
    let vk_box = iroha::data_model::proof::VerifyingKeyBox::new(
        iroha_core::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
        vk_bytes,
    );
    iroha_core::zk::kagemusha_recursive_compact_payment_token_vk_record_from_box(
        namespace, version, vk_box,
    )
    .map_err(|err| eyre::eyre!("failed to build ABI-7 recursive compact verifier record: {err}"))
}

impl Run for KagemushaLineageRecordArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let vk_bytes = std::fs::read(&self.vk)
            .wrap_err_with(|| format!("failed to read {}", self.vk.display()))?;
        let vk_len = vk_bytes.len();
        let record = kagemusha_lineage_vk_record_from_bytes(
            self.profile,
            self.record_namespace,
            self.record_version,
            self.opening_len,
            vk_bytes,
        )?;
        let record_bytes = norito::to_bytes(&record).map_err(|err| {
            eyre::eyre!("failed to encode Reserved-lineage verifier record: {err}")
        })?;
        write_kagemusha_lineage_key_artifact_file(&self.out, &record_bytes)
            .wrap_err_with(|| format!("failed to write {}", self.out.display()))?;
        context.println(format!(
            "Wrote {} Reserved-lineage verifier record for `{}` opening_len={} from {} to {} (vk={} bytes, record={} bytes)",
            self.profile.label(),
            self.profile.circuit_id(),
            self.opening_len,
            self.vk.display(),
            self.out.display(),
            vk_len,
            record_bytes.len(),
        ))?;
        Ok(())
    }
}

impl Run for KagemushaRecursiveCompactKeyArtifactsArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match (&self.key_artifacts_out, &self.verifier_keys_out) {
            (Some(_), Some(_)) => {}
            _ => {
                return Err(eyre::eyre!(
                    "--key-artifacts-out and --verifier-keys-out must both be provided for ABI-7 recursive compact production key packages"
                ));
            }
        }

        eprintln!(
            "Generating ABI-7 recursive compact verifier key for `{}` opening_len={}",
            iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID,
            iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_OPENING_LEN
        );
        let vk_box =
            iroha_core::zk::kagemusha_recursive_compact_payment_token_vk_box().map_err(|err| {
                eyre::eyre!("failed to generate ABI-7 recursive compact verifier key: {err}")
            })?;

        eprintln!(
            "Writing ABI-7 recursive compact verifier key to {}",
            self.vk_out.display()
        );
        let vk_summary = compact_key_output_summary(&vk_box.bytes);
        write_kagemusha_lineage_key_artifact_file(&self.vk_out, &vk_box.bytes)
            .wrap_err_with(|| format!("failed to write {}", self.vk_out.display()))?;

        let record_summary = if let Some(path) = &self.record_out {
            eprintln!(
                "Writing ABI-7 recursive compact verifier record to {}",
                path.display()
            );
            let record =
                iroha_core::zk::kagemusha_recursive_compact_payment_token_vk_record_from_box(
                    self.record_namespace.clone(),
                    self.record_version,
                    vk_box.clone(),
                )
                .map_err(|err| {
                    eyre::eyre!("failed to build ABI-7 recursive compact verifier record: {err}")
                })?;
            let record_bytes = norito::to_bytes(&record).map_err(|err| {
                eyre::eyre!("failed to encode ABI-7 recursive compact verifier record: {err}")
            })?;
            let record_summary = compact_key_output_summary(&record_bytes);
            write_kagemusha_lineage_key_artifact_file(path, &record_bytes)
                .wrap_err_with(|| format!("failed to write {}", path.display()))?;
            Some(record_summary)
        } else {
            None
        };

        eprintln!(
            "Deriving ABI-7 recursive compact proving key archive for `{}` opening_len={}",
            iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID,
            iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_OPENING_LEN
        );
        let proving_key =
            iroha_core::zk::derive_halo2_ipa_kagemusha_recursive_compact_payment_token_proving_key_bytes(
                &vk_box,
            )
            .map_err(|err| {
                eyre::eyre!(
                    "failed to derive ABI-7 recursive compact proving key archive: {err}"
                )
            })?;

        eprintln!(
            "Writing ABI-7 recursive compact proving key archive to {}",
            self.pk_out.display()
        );
        let pk_summary = compact_key_output_summary(&proving_key);
        write_kagemusha_lineage_key_artifact_file(&self.pk_out, &proving_key)
            .wrap_err_with(|| format!("failed to write {}", self.pk_out.display()))?;

        let package_summaries = if let (Some(key_artifacts_path), Some(verifier_keys_path)) =
            (&self.key_artifacts_out, &self.verifier_keys_out)
        {
            eprintln!(
                "Generating ABI-7 recursive compact key package for `{}` opening_len={}",
                iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID,
                iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_OPENING_LEN
            );
            let append_vk_box =
                iroha_core::zk::kagemusha_recursive_compact_payment_token_append_vk_box(
                    iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_OPENING_LEN,
                )
                .map_err(|err| {
                    eyre::eyre!(
                        "failed to generate ABI-7 recursive compact append verifier key: {err}"
                    )
                })?;
            let append_proving_key =
                iroha_core::zk::derive_halo2_ipa_kagemusha_recursive_compact_payment_token_append_proving_key_bytes(
                    &append_vk_box,
                    iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_OPENING_LEN,
                )
                .map_err(|err| {
                    eyre::eyre!(
                        "failed to derive ABI-7 recursive compact append proving key archive: {err}"
                    )
                })?;
            let key_artifacts =
                iroha::data_model::offline::KagemushaRecursiveCompactKeyArtifactsV1::new(vec![
                    iroha::data_model::offline::KagemushaRecursiveCompactKeyArtifactEntryV1::new(
                        iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_OPENING_LEN,
                        vk_box.clone(),
                        proving_key.clone(),
                        append_vk_box,
                        append_proving_key,
                    )
                    .map_err(|err| {
                        eyre::eyre!(
                            "failed to build ABI-7 recursive compact key package entry: {err}"
                        )
                    })?,
                ])
                .map_err(|err| {
                    eyre::eyre!("failed to build ABI-7 recursive compact key package: {err}")
                })?;
            let verifier_keys = key_artifacts.verifier_keys().map_err(|err| {
                eyre::eyre!("failed to derive ABI-7 recursive compact verifier-key package: {err}")
            })?;
            let key_artifacts_bytes = norito::to_bytes(&key_artifacts).map_err(|err| {
                eyre::eyre!("failed to encode ABI-7 recursive compact key package: {err}")
            })?;
            let verifier_keys_bytes = norito::to_bytes(&verifier_keys).map_err(|err| {
                eyre::eyre!("failed to encode ABI-7 recursive compact verifier-key package: {err}")
            })?;
            let key_artifacts_summary = compact_key_output_summary(&key_artifacts_bytes);
            let verifier_keys_summary = compact_key_output_summary(&verifier_keys_bytes);

            eprintln!(
                "Writing ABI-7 recursive compact key package to {}",
                key_artifacts_path.display()
            );
            write_kagemusha_lineage_key_artifact_file(key_artifacts_path, &key_artifacts_bytes)
                .wrap_err_with(|| format!("failed to write {}", key_artifacts_path.display()))?;
            eprintln!(
                "Writing ABI-7 recursive compact verifier-key package to {}",
                verifier_keys_path.display()
            );
            write_kagemusha_lineage_key_artifact_file(verifier_keys_path, &verifier_keys_bytes)
                .wrap_err_with(|| format!("failed to write {}", verifier_keys_path.display()))?;
            Some((key_artifacts_summary, verifier_keys_summary))
        } else {
            None
        };

        context.println(kagemusha_recursive_compact_key_artifacts_summary(
            &self.vk_out,
            &self.pk_out,
            &vk_summary,
            &pk_summary,
            record_summary.as_ref(),
            package_summaries
                .as_ref()
                .map(|(key_artifacts, verifier_keys)| (key_artifacts, verifier_keys)),
        ))?;
        Ok(())
    }
}

struct CompactKeyOutputSummary {
    len: usize,
    sha256: String,
}

fn compact_key_output_summary(bytes: &[u8]) -> CompactKeyOutputSummary {
    use sha2::{Digest as _, Sha256};

    CompactKeyOutputSummary {
        len: bytes.len(),
        sha256: hex::encode(Sha256::digest(bytes)),
    }
}

fn kagemusha_recursive_compact_key_artifacts_summary(
    vk_out: &std::path::Path,
    pk_out: &std::path::Path,
    vk: &CompactKeyOutputSummary,
    pk: &CompactKeyOutputSummary,
    record: Option<&CompactKeyOutputSummary>,
    packages: Option<(&CompactKeyOutputSummary, &CompactKeyOutputSummary)>,
) -> String {
    let record_summary = record
        .map(|artifact| format!(", record={} bytes sha256={}", artifact.len, artifact.sha256))
        .unwrap_or_default();
    let package_summary = packages
        .map(|(key_artifacts, verifier_keys)| {
            format!(
                ", key_artifacts={} bytes sha256={}, verifier_keys={} bytes sha256={}",
                key_artifacts.len, key_artifacts.sha256, verifier_keys.len, verifier_keys.sha256
            )
        })
        .unwrap_or_default();
    format!(
        "Wrote ABI-7 recursive compact key artifacts for `{}` opening_len={} to {} and {} (vk={} bytes sha256={}, pk={} bytes sha256={}{}{})",
        iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID,
        iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_OPENING_LEN,
        vk_out.display(),
        pk_out.display(),
        vk.len,
        vk.sha256,
        pk.len,
        pk.sha256,
        record_summary,
        package_summary
    )
}

impl Run for KagemushaLineageKeyArtifactsArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        use iroha::data_model::offline::KagemushaRecursiveSpendLineageKeyArtifactsV1;

        eprintln!(
            "Generating {} Reserved-lineage verifier key for `{}` opening_len={}",
            self.profile.label(),
            self.profile.circuit_id(),
            self.opening_len
        );
        let vk_box = match self.profile {
            KagemushaLineageKeyProfile::Init => {
                iroha_core::zk::kagemusha_recursive_spend_lineage_vk_box(self.opening_len)
            }
            KagemushaLineageKeyProfile::Append => {
                iroha_core::zk::kagemusha_recursive_spend_lineage_append_vk_box(self.opening_len)
            }
        }
        .map_err(|err| {
            eyre::eyre!(
                "failed to generate {} Reserved-lineage verifier key for opening length {}: {err}",
                self.profile.label(),
                self.opening_len
            )
        })?;

        if let Some(path) = &self.vk_out {
            eprintln!(
                "Writing {} Reserved-lineage verifier key to {}",
                self.profile.label(),
                path.display()
            );
            write_kagemusha_lineage_key_artifact_file(path, &vk_box.bytes)
                .wrap_err_with(|| format!("failed to write {}", path.display()))?;
        }

        let mut record_summary = String::new();
        if let Some(path) = &self.record_out {
            eprintln!(
                "Writing {} Reserved-lineage verifier record to {}",
                self.profile.label(),
                path.display()
            );
            let record = match self.profile {
                KagemushaLineageKeyProfile::Init => {
                    iroha_core::zk::kagemusha_recursive_spend_lineage_vk_record_from_box(
                        self.record_namespace.clone(),
                        self.record_version,
                        self.opening_len,
                        vk_box.clone(),
                    )
                }
                KagemushaLineageKeyProfile::Append => {
                    iroha_core::zk::kagemusha_recursive_spend_lineage_append_vk_record_from_box(
                        self.record_namespace.clone(),
                        self.record_version,
                        self.opening_len,
                        vk_box.clone(),
                    )
                }
            }
            .map_err(|err| {
                eyre::eyre!(
                    "failed to build {} Reserved-lineage verifier record for opening length {}: {err}",
                    self.profile.label(),
                    self.opening_len
                )
            })?;
            let record_bytes = norito::to_bytes(&record).map_err(|err| {
                eyre::eyre!("failed to encode Reserved-lineage verifier record: {err}")
            })?;
            record_summary = format!(", record={} bytes", record_bytes.len());
            write_kagemusha_lineage_key_artifact_file(path, &record_bytes)
                .wrap_err_with(|| format!("failed to write {}", path.display()))?;
        }

        eprintln!(
            "Deriving {} Reserved-lineage proving key archive for `{}` opening_len={}",
            self.profile.label(),
            self.profile.circuit_id(),
            self.opening_len
        );
        let proving_key = match self.profile {
            KagemushaLineageKeyProfile::Init => {
                iroha_core::zk::derive_halo2_ipa_kagemusha_recursive_spend_lineage_one_hop_proving_key_bytes(
                    &vk_box,
                    self.opening_len,
                )
            }
            KagemushaLineageKeyProfile::Append => {
                iroha_core::zk::derive_halo2_ipa_kagemusha_recursive_spend_lineage_append_proving_key_bytes(
                    &vk_box,
                    self.opening_len,
                )
            }
        }
        .map_err(|err| {
            eyre::eyre!(
                "failed to derive {} Reserved-lineage proving key archive for opening length {}: {err}",
                self.profile.label(),
                self.opening_len
            )
        })?;

        if let Some(path) = &self.pk_out {
            eprintln!(
                "Writing {} Reserved-lineage proving key archive to {}",
                self.profile.label(),
                path.display()
            );
            write_kagemusha_lineage_key_artifact_file(path, &proving_key)
                .wrap_err_with(|| format!("failed to write {}", path.display()))?;
        }

        eprintln!(
            "Encoding {} Reserved-lineage key package for `{}` opening_len={}",
            self.profile.label(),
            self.profile.circuit_id(),
            self.opening_len
        );
        let artifacts = match self.profile {
            KagemushaLineageKeyProfile::Init => {
                KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_init(
                    self.opening_len,
                    vk_box.clone(),
                    proving_key.clone(),
                )
            }
            KagemushaLineageKeyProfile::Append => {
                KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_append(
                    self.opening_len,
                    vk_box.clone(),
                    proving_key.clone(),
                )
            }
        }
        .map_err(|err| {
            eyre::eyre!(
                "generated {} Reserved-lineage key package failed validation: {err}",
                self.profile.label()
            )
        })?;
        let artifact_bytes = norito::to_bytes(&artifacts)
            .map_err(|err| eyre::eyre!("failed to encode Reserved-lineage key package: {err}"))?;

        eprintln!(
            "Writing {} Reserved-lineage key package to {}",
            self.profile.label(),
            self.out.display()
        );
        write_kagemusha_lineage_key_artifact_file(&self.out, &artifact_bytes)
            .wrap_err_with(|| format!("failed to write {}", self.out.display()))?;

        context.println(format!(
            "Wrote {} Reserved-lineage key package for `{}` opening_len={} to {} (package={} bytes, vk={} bytes, pk={} bytes{})",
            self.profile.label(),
            self.profile.circuit_id(),
            self.opening_len,
            self.out.display(),
            artifact_bytes.len(),
            vk_box.bytes.len(),
            proving_key.len(),
            record_summary
        ))?;
        Ok(())
    }
}

fn write_kagemusha_lineage_key_artifact_file(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write as _;

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    if parent != std::path::Path::new(".") {
        std::fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| eyre::eyre!("artifact output path must include a file name"))?
        .to_string_lossy();
    let mut temp_path = None;
    for attempt in 0..1024_u16 {
        let candidate = parent.join(format!(".{file_name}.tmp-{}-{attempt}", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                if let Err(err) = file.write_all(bytes).and_then(|()| file.sync_all()) {
                    let _ = std::fs::remove_file(&candidate);
                    return Err(err.into());
                }
                temp_path = Some(candidate);
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err.into()),
        }
    }
    let temp_path = temp_path
        .ok_or_else(|| eyre::eyre!("failed to allocate temporary artifact output path"))?;
    if let Err(err) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(err.into());
    }
    if let Ok(parent_dir) = std::fs::File::open(parent) {
        parent_dir.sync_all()?;
    }
    Ok(())
}

#[derive(clap::Subcommand, Debug)]
pub enum VoteCommand {
    /// Get election tally (JSON)
    Tally(VoteTallyArgs),
}

impl Run for VoteCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            VoteCommand::Tally(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct VoteTallyArgs {
    /// Election identifier
    #[arg(long, value_name = "ELECTION_ID")]
    election_id: String,
}

impl Run for VoteTallyArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.post_zk_vote_tally_json(&self.election_id)?;
        context.print_data(&value)?;
        Ok(())
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum AttachmentsCommand {
    /// Upload a file as an attachment. Returns JSON metadata.
    Upload(AttachmentUploadArgs),
    /// List stored attachments (JSON array of metadata).
    List(AttachmentListArgs),
    /// Download an attachment by id to a file.
    Get(AttachmentGetArgs),
    /// Delete an attachment by id.
    Delete(AttachmentDeleteArgs),
    /// Cleanup attachments by filters (age/content-type/ids). Deletes individually via API.
    Cleanup(AttachmentCleanupArgs),
}

impl Run for AttachmentsCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            AttachmentsCommand::Upload(args) => args.run(context),
            AttachmentsCommand::List(args) => args.run(context),
            AttachmentsCommand::Get(args) => args.run(context),
            AttachmentsCommand::Delete(args) => args.run(context),
            AttachmentsCommand::Cleanup(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct AttachmentUploadArgs {
    /// Path to the file to upload
    #[arg(long, value_name = "PATH")]
    file: std::path::PathBuf,
    /// Content-Type to send with the file
    #[arg(long, value_name = "MIME", default_value = "application/octet-stream")]
    content_type: String,
}

impl Run for AttachmentUploadArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let body = std::fs::read(&self.file)?;
        let value = client.post_zk_attachment(&body, &self.content_type)?;
        context.print_data(&value)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct AttachmentListArgs {}

impl Run for AttachmentListArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.get_zk_attachments_list()?;
        context.print_data(&value)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct AttachmentGetArgs {
    /// Attachment id (hex)
    #[arg(long, value_name = "ID")]
    id: String,
    /// Output path to write the downloaded bytes
    #[arg(long, value_name = "PATH")]
    out: std::path::PathBuf,
}

impl Run for AttachmentGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let (bytes, _ct) = client.get_zk_attachment_raw(&self.id)?;
        std::fs::write(&self.out, &bytes)?;
        context.println(format!(
            "Wrote {} bytes to {}",
            bytes.len(),
            self.out.display()
        ))?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct AttachmentDeleteArgs {
    /// Attachment id (hex)
    #[arg(long, value_name = "ID")]
    id: String,
}

impl Run for AttachmentDeleteArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        client.delete_zk_attachment(&self.id)?;
        context.println("Deleted")?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct AttachmentCleanupArgs {
    /// Proceed without confirmation
    #[arg(long)]
    yes: bool,
    /// Delete all attachments (dangerous). Requires --yes.
    #[arg(long, conflicts_with_all = ["content_type", "older_than_secs", "before_ms", "id"])]
    all: bool,
    /// Filter by content-type substring (e.g., application/x-norito)
    #[arg(long, value_name = "MIME")]
    content_type: Option<String>,
    /// Filter attachments created strictly before this UNIX epoch in milliseconds
    #[arg(long, value_name = "MS", conflicts_with = "older_than_secs")]
    before_ms: Option<u64>,
    /// Filter attachments older than N seconds (relative to now)
    #[arg(long, value_name = "SECS")]
    older_than_secs: Option<u64>,
    /// Filter by specific id(s); may be repeated
    #[arg(long, value_name = "ID")]
    id: Vec<String>,
    /// Maximum number of attachments to delete (applied after filtering)
    #[arg(long, value_name = "N")]
    limit: Option<usize>,
    /// Preview only: list matching ids instead of full metadata
    #[arg(long)]
    ids_only: bool,
    /// Preview only: print a summary table (id, `content_type`, size, `created_ms`)
    #[arg(long)]
    summary: bool,
}

fn now_ms_u64() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn select_attachment_ids(
    list: &norito::json::Value,
    content_type_sub: Option<&str>,
    before_ms: Option<u64>,
    ids: &[String],
    older_than_secs: Option<u64>,
    now_ms: u64,
) -> Vec<(String, String, u64, u64)> {
    let mut out = Vec::new();
    let arr = list.as_array().cloned().unwrap_or_default();
    for v in arr.into_iter() {
        let id = v
            .get("id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            continue;
        }
        if !ids.is_empty() && !ids.iter().any(|x| x == &id) {
            continue;
        }
        let ct = v
            .get("content_type")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(sub) = content_type_sub
            && !ct.contains(sub)
        {
            continue;
        }
        let created_ms = v.get("created_ms").and_then(|x| x.as_u64()).unwrap_or(0);
        let size = v.get("size").and_then(|x| x.as_u64()).unwrap_or(0);
        if let Some(ms) = before_ms
            && !(created_ms < ms)
        {
            continue;
        }
        if let Some(secs) = older_than_secs {
            let threshold = now_ms.saturating_sub(secs.saturating_mul(1000));
            if !(created_ms < threshold) {
                continue;
            }
        }
        out.push((id, ct, size, created_ms));
    }
    out
}

impl Run for AttachmentCleanupArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let list = client.get_zk_attachments_list()?;
        let now_ms = now_ms_u64();
        let before_ms = if self.older_than_secs.is_some() {
            None
        } else {
            self.before_ms
        };

        if !(self.all
            || self.content_type.is_some()
            || before_ms.is_some()
            || self.older_than_secs.is_some()
            || !self.id.is_empty())
        {
            eyre::bail!(
                "no filters provided; use --all (with --yes) or one of --content-type/--before-ms/--older-than-secs/--id"
            );
        }
        if self.all && !self.yes {
            eyre::bail!("--all requires --yes confirmation");
        }

        let mut matches = if self.all {
            // select everything
            select_attachment_ids(&list, None, None, &[], None, now_ms)
        } else {
            select_attachment_ids(
                &list,
                self.content_type.as_deref(),
                before_ms,
                &self.id,
                self.older_than_secs,
                now_ms,
            )
        };
        // Sort by created time ascending
        matches.sort_by_key(|(_, _, _, created)| *created);
        if let Some(cap) = self.limit
            && matches.len() > cap
        {
            matches.truncate(cap);
        }

        if !self.yes {
            if self.ids_only {
                let ids = json_utils::json_array(matches.iter().map(|(id, _, _, _)| id.as_str()))?;
                context.print_data(&ids)?;
            } else if self.summary {
                // print concise lines
                for (id, ct, size, created) in &matches {
                    context.println(format!("{id}  {ct}  size={size}  created_ms={created}"))?;
                }
                context.println(format!("{} match(es). Use --yes to delete.", matches.len()))?;
            } else {
                // default: print the full JSON array back
                let arr: Vec<norito::json::Value> = matches
                    .iter()
                    .map(|(id, ct, size, created)| {
                        json_utils::json_object(vec![
                            ("id", json_utils::json_value(id.as_str())?),
                            ("content_type", json_utils::json_value(ct.as_str())?),
                            ("size", json_utils::json_value(size)?),
                            ("created_ms", json_utils::json_value(created)?),
                        ])
                    })
                    .collect::<Result<_, _>>()?;
                let arr_value = json_utils::json_array(arr)?;
                context.print_data(&arr_value)?;
                context.println("Preview only. Use --yes to delete.")?;
            }
            return Ok(());
        }

        // Proceed with deletion
        let mut ok = 0usize;
        let mut failed = 0usize;
        for (id, _ct, _size, _created) in &matches {
            match client.delete_zk_attachment(id) {
                Ok(_) => ok += 1,
                Err(_) => failed += 1,
            }
        }
        context.println(format!("Deleted {} attachment(s), failed {}.", ok, failed))?;
        Ok(())
    }
}

#[cfg(test)]
mod attachments_cleanup_tests {
    use super::select_attachment_ids;

    #[test]
    fn selects_by_ct_and_age() {
        let now_ms = 1_000_000u64;
        let json = r#"[
            {"id":"a","content_type":"application/json","size":1,"created_ms":900000},
            {"id":"b","content_type":"application/x-norito","size":2,"created_ms":800000},
            {"id":"c","content_type":"application/x-norito","size":3,"created_ms":999000}
        ]"#;
        let list = norito::json::from_str(json).expect("parse attachment list");
        // older than 100 seconds => threshold 900_000; norito only
        let v = select_attachment_ids(
            &list,
            Some("application/x-norito"),
            None,
            &[],
            Some(100),
            now_ms,
        );
        let ids: Vec<String> = v.into_iter().map(|t| t.0).collect();
        assert_eq!(ids, vec!["b"]);
    }
}

// ---------------- Shield / Unshield demo flows ----------------

#[derive(clap::Args, Debug)]
pub struct ShieldArgs {
    /// Canonical unprefixed Base58 `AssetDefinitionId`
    #[arg(long, value_name = "ASSET_ID")]
    asset: String,
    /// Account identifier to debit (canonical I105 account literal)
    #[arg(long, value_name = "ACCOUNT_ID")]
    from: String,
    /// Public amount to debit
    #[arg(long, value_name = "AMOUNT")]
    amount: u128,
    /// Output note commitment (hex, 64 chars)
    #[arg(long, value_name = "HEX32")]
    note_commitment: String,
    /// Encrypted recipient payload envelope (Norito bytes). Optional; empty if not provided.
    #[arg(long, value_name = "PATH")]
    enc_payload: Option<std::path::PathBuf>,
    /// Ephemeral public key for encrypted payload (hex, 64 chars).
    #[arg(
        long,
        value_name = "HEX32",
        requires_all = ["nonce_hex", "ciphertext_b64"]
    )]
    ephemeral_pubkey: Option<String>,
    /// XChaCha20-Poly1305 nonce for encrypted payload (hex, 48 chars).
    #[arg(
        long,
        value_name = "HEX24",
        requires_all = ["ephemeral_pubkey", "ciphertext_b64"]
    )]
    nonce_hex: Option<String>,
    /// Ciphertext payload (base64). Includes Poly1305 authentication tag.
    #[arg(
        long,
        value_name = "BASE64",
        requires_all = ["ephemeral_pubkey", "nonce_hex"]
    )]
    ciphertext_b64: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct EnvelopeArgs {
    /// Ephemeral public key (hex, 64 chars).
    #[arg(long, value_name = "HEX32")]
    ephemeral_pubkey: String,
    /// XChaCha20-Poly1305 nonce (hex, 48 chars).
    #[arg(long, value_name = "HEX24")]
    nonce_hex: String,
    /// Ciphertext payload (base64) including Poly1305 tag.
    #[arg(long, value_name = "BASE64")]
    ciphertext_b64: String,
    /// Optional output path for Norito bytes.
    #[arg(long, value_name = "PATH")]
    output: Option<std::path::PathBuf>,
    /// Print base64 of the encoded envelope (default when no output file is provided).
    #[arg(long, default_value_t = false)]
    print_base64: bool,
    /// Print hexadecimal representation of the encoded envelope.
    #[arg(long, default_value_t = false)]
    print_hex: bool,
    /// Print JSON representation of the envelope.
    #[arg(long, default_value_t = false)]
    print_json: bool,
}

fn parse_hex32(s: &str) -> eyre::Result<[u8; 32]> {
    let bytes = hex::decode(s).map_err(|e| eyre::eyre!("invalid hex: {e}"))?;
    if bytes.len() != 32 {
        return Err(eyre::eyre!("expected 32 bytes, got {}", bytes.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn parse_hex_array<const N: usize>(s: &str) -> eyre::Result<[u8; N]> {
    let bytes = hex::decode(s).map_err(|e| eyre::eyre!("invalid hex: {e}"))?;
    if bytes.len() != N {
        return Err(eyre::eyre!("expected {N} bytes, got {}", bytes.len()));
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn build_encrypted_payload(
    ephemeral_hex: &str,
    nonce_hex: &str,
    ciphertext_b64: &str,
) -> eyre::Result<iroha::data_model::confidential::ConfidentialEncryptedPayload> {
    use iroha::data_model::confidential::ConfidentialEncryptedPayload;

    let ephemeral = parse_hex_array::<32>(ephemeral_hex)?;
    let nonce = parse_hex_array::<24>(nonce_hex)?;
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(ciphertext_b64)
        .map_err(|e| eyre::eyre!("invalid ciphertext base64: {e}"))?;
    validate_encrypted_payload(ConfidentialEncryptedPayload::new(
        ephemeral, nonce, ciphertext,
    ))
}

fn validate_encrypted_payload(
    payload: iroha::data_model::confidential::ConfidentialEncryptedPayload,
) -> eyre::Result<iroha::data_model::confidential::ConfidentialEncryptedPayload> {
    payload
        .validate()
        .map_err(|e| eyre::eyre!("invalid encrypted payload: {e}"))?;
    Ok(payload)
}

fn encode_encrypted_payload(
    ephemeral_hex: &str,
    nonce_hex: &str,
    ciphertext_b64: &str,
) -> eyre::Result<(
    iroha::data_model::confidential::ConfidentialEncryptedPayload,
    Vec<u8>,
)> {
    let payload = build_encrypted_payload(ephemeral_hex, nonce_hex, ciphertext_b64)?;
    let bytes = norito::codec::encode_adaptive(&payload);
    Ok((payload, bytes))
}

impl Run for ShieldArgs {
    fn run<C: RunContext>(self, context: &mut C) -> eyre::Result<()> {
        use iroha::data_model::{
            confidential::ConfidentialEncryptedPayload,
            prelude::{AccountId, AssetDefinitionId, InstructionBox},
        };
        let asset = AssetDefinitionId::parse_address_literal(&self.asset)?;
        let from =
            crate::resolve_account_id(context, &self.from).wrap_err("failed to resolve --from")?;
        let note_commitment = parse_hex32(&self.note_commitment)?;
        let enc_payload = if let (Some(ephemeral_hex), Some(nonce_hex), Some(ciphertext_b64)) = (
            &self.ephemeral_pubkey,
            &self.nonce_hex,
            &self.ciphertext_b64,
        ) {
            build_encrypted_payload(ephemeral_hex, nonce_hex, ciphertext_b64)?
        } else {
            match &self.enc_payload {
                Some(p) => {
                    let bytes = std::fs::read(p)?;
                    let payload = norito::decode_from_bytes::<ConfidentialEncryptedPayload>(&bytes)
                        .map_err(|e| eyre::eyre!("failed to decode encrypted payload: {e}"))?;
                    validate_encrypted_payload(payload)?
                }
                None => {
                    return Err(eyre::eyre!(
                        "encrypted payload requires ephemeral_pubkey, nonce_hex, and ciphertext_b64 or an encoded envelope file"
                    ));
                }
            }
        };
        let ib: InstructionBox = iroha::data_model::isi::zk::Shield::new(
            asset,
            from,
            self.amount,
            note_commitment,
            enc_payload,
        )
        .into();
        context.finish(vec![ib])
    }
}

impl Run for EnvelopeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> eyre::Result<()> {
        let (payload, bytes) = encode_encrypted_payload(
            &self.ephemeral_pubkey,
            &self.nonce_hex,
            &self.ciphertext_b64,
        )?;

        if let Some(path) = &self.output {
            std::fs::write(path, &bytes)
                .with_context(|| format!("failed to write envelope to {}", path.display()))?;
            context.println(format!("Wrote {} bytes to {}", bytes.len(), path.display()))?;
        }

        if self.output.is_none() || self.print_base64 {
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            context.println(encoded)?;
        }

        if self.print_hex {
            context.println(hex::encode(&bytes))?;
        }

        if self.print_json {
            context.print_data(&payload)?;
        }

        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct UnshieldArgs {
    /// Canonical unprefixed Base58 `AssetDefinitionId`
    #[arg(long, value_name = "ASSET_ID")]
    asset: String,
    /// Recipient account identifier to credit (canonical I105 account literal)
    #[arg(long, value_name = "ACCOUNT_ID")]
    to: String,
    /// Public amount to credit
    #[arg(long, value_name = "AMOUNT")]
    amount: u128,
    /// Spent nullifiers (comma-separated list of 64-hex strings)
    #[arg(long, value_name = "HEX32[,HEX32,...]")]
    inputs: String,
    /// Proof attachment JSON file describing { backend, `proof_b64`, `vk_ref{backend,name}`, optional `vk_commitment_hex` }
    #[arg(long, value_name = "PATH")]
    proof_json: std::path::PathBuf,
    /// Optional Merkle root hint (hex, 64 chars)
    #[arg(long, value_name = "HEX32")]
    root_hint: Option<String>,
}

fn parse_inputs_csv(s: &str) -> eyre::Result<Vec<[u8; 32]>> {
    s.split(',')
        .filter(|x| !x.is_empty())
        .map(|h| parse_hex32(h.trim()))
        .collect()
}

fn parse_hex_string(hex_str: &str) -> eyre::Result<Vec<u8>> {
    let trimmed = hex_str.trim();
    let without_prefix = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    let bytes = hex::decode(without_prefix).map_err(|e| eyre::eyre!("invalid hex string: {e}"))?;
    Ok(bytes)
}

fn ensure_production_verify_backend_label<'a>(backend: &'a str, field: &str) -> Result<&'a str> {
    if backend.is_empty() {
        eyre::bail!("{field} must be non-empty");
    }
    if !iroha_core::zk::is_production_verify_backend_label(backend) {
        eyre::bail!("{field} uses unsupported production verifier backend `{backend}`");
    }
    Ok(backend)
}

fn parse_hex32_lower(value: &str, field: &str) -> Result<String> {
    let bytes = parse_hex32_str(value, field)?;
    Ok(hex::encode(bytes))
}

fn build_proof_attachment_from_json(
    v: &norito::json::Value,
) -> eyre::Result<iroha::data_model::proof::ProofAttachment> {
    use iroha::data_model::proof::{ProofAttachment, ProofBox, VerifyingKeyId};
    let object = v
        .as_object()
        .ok_or_else(|| eyre::eyre!("proof attachment JSON must be an object"))?;
    for field in object.keys() {
        match field.as_str() {
            "backend" | "proof_b64" | "vk_ref" | "vk_commitment_hex" => {}
            "vk_inline" | "vkInline" | "verifyingKeyInline" | "verifying_key_inline" => {
                return Err(eyre::eyre!(
                    "legacy inline verifying-key field `{field}` is not supported; use vk_ref"
                ));
            }
            other => return Err(eyre::eyre!("unknown proof attachment field `{other}`")),
        }
    }
    let backend = v
        .get("backend")
        .and_then(|x| x.as_str())
        .ok_or_else(|| eyre::eyre!("missing backend"))?;
    let backend = ensure_production_verify_backend_label(backend, "backend")?;
    let proof_b64 = v
        .get("proof_b64")
        .and_then(|x| x.as_str())
        .ok_or_else(|| eyre::eyre!("missing proof_b64"))?;
    let proof_bytes = base64::engine::general_purpose::STANDARD
        .decode(proof_b64)
        .map_err(|e| eyre::eyre!("invalid proof_b64: {e}"))?;
    let proof = ProofBox::new(backend.into(), proof_bytes);
    let vk_ref = v.get("vk_ref").and_then(|x| x.as_object());
    let mut att = if let Some(obj) = vk_ref {
        for field in obj.keys() {
            match field.as_str() {
                "backend" | "name" => {}
                other => return Err(eyre::eyre!("unknown vk_ref field `{other}`")),
            }
        }
        let b = obj
            .get("backend")
            .and_then(|x| x.as_str())
            .ok_or_else(|| eyre::eyre!("vk_ref.backend missing"))?;
        ensure_production_verify_backend_label(b, "vk_ref.backend")?;
        if b != backend {
            return Err(eyre::eyre!("vk_ref.backend must match backend"));
        }
        let name = obj
            .get("name")
            .and_then(|x| x.as_str())
            .ok_or_else(|| eyre::eyre!("vk_ref.name missing"))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(eyre::eyre!("vk_ref.name must be non-empty"));
        }
        let id = VerifyingKeyId::new(b, name);
        ProofAttachment::new_ref(backend.into(), proof, id)
    } else {
        return Err(eyre::eyre!("vk_ref must be provided"));
    };
    if let Some(hex) = v.get("vk_commitment_hex").and_then(|x| x.as_str()) {
        let bytes = hex::decode(hex).map_err(|e| eyre::eyre!("invalid vk_commitment_hex: {e}"))?;
        if bytes.len() != 32 {
            return Err(eyre::eyre!(
                "vk_commitment_hex must decode to 32 bytes, got {}",
                bytes.len()
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        att.vk_commitment = Some(arr);
    }
    Ok(att)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestContext {
        cfg: iroha::config::Config,
        json_outputs: Vec<String>,
        lines: Vec<String>,
        i18n: iroha_i18n::Localizer,
    }

    impl TestContext {
        fn new() -> Self {
            let key_pair =
                iroha_crypto::KeyPair::random_with_algorithm(iroha_crypto::Algorithm::Ed25519);
            let account_id =
                iroha::data_model::account::AccountId::new(key_pair.public_key().clone());
            let cfg = iroha::config::Config {
                chain: iroha::data_model::prelude::ChainId::from(
                    "00000000-0000-0000-0000-000000000000",
                ),
                account: account_id,
                account_chain_discriminant:
                    iroha_config::parameters::defaults::common::chain_discriminant(),
                key_pair,
                basic_auth: None,
                torii_api_url: url::Url::parse("http://127.0.0.1/").unwrap(),
                torii_api_version: iroha::config::default_torii_api_version(),
                torii_api_min_proof_version: iroha::config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                    .to_string(),
                torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
                transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
                transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
                transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
                connect_queue_root: iroha::config::default_connect_queue_root(),
                soracloud_http_witness_file: None,
                sorafs_alias_cache: crate::config_utils::default_alias_cache_policy(),
                sorafs_anonymity_policy: crate::config_utils::default_anonymity_policy(),
                sorafs_rollout_phase: crate::config_utils::default_rollout_phase(),
            };
            Self {
                cfg,
                json_outputs: Vec::new(),
                lines: Vec::new(),
                i18n: iroha_i18n::Localizer::new(
                    iroha_i18n::Bundle::Cli,
                    iroha_i18n::Language::English,
                ),
            }
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &iroha::config::Config {
            &self.cfg
        }

        fn transaction_metadata(&self) -> Option<&iroha::data_model::prelude::Metadata> {
            None
        }

        fn input_instructions(&self) -> bool {
            false
        }

        fn output_instructions(&self) -> bool {
            false
        }

        fn i18n(&self) -> &iroha_i18n::Localizer {
            &self.i18n
        }

        fn print_data<T>(&mut self, data: &T) -> Result<()>
        where
            T: norito::json::JsonSerialize + ?Sized,
        {
            let json =
                norito::json::to_json_pretty(data).map_err(|err| eyre::eyre!(err.to_string()))?;
            self.json_outputs.push(json);
            Ok(())
        }

        fn println(&mut self, data: impl std::fmt::Display) -> Result<()> {
            self.lines.push(data.to_string());
            Ok(())
        }
    }

    #[test]
    fn fallback_config_is_limited_to_offline_kagemusha_artifact_commands() {
        let lineage_artifacts = Command::Kagemusha(KagemushaCommand::LineageKeyArtifacts(
            KagemushaLineageKeyArtifactsArgs {
                profile: KagemushaLineageKeyProfile::Init,
                opening_len: 4,
                out: "lineage.to".into(),
                vk_out: None,
                pk_out: None,
                record_out: None,
                record_namespace: "offline_kagemusha".to_owned(),
                record_version: 1,
            },
        ));
        assert!(lineage_artifacts.allows_fallback_config());

        let compact_artifacts = Command::Kagemusha(KagemushaCommand::RecursiveCompactKeyArtifacts(
            KagemushaRecursiveCompactKeyArtifactsArgs {
                vk_out: "recursive-compact.vk".into(),
                pk_out: "recursive-compact.pk".into(),
                key_artifacts_out: Some("recursive-compact-key-artifacts.norito".into()),
                verifier_keys_out: Some("recursive-compact-verifier-keys.norito".into()),
                record_out: Some("recursive-compact.record.norito".into()),
                record_namespace: "offline_kagemusha".to_owned(),
                record_version: 1,
            },
        ));
        assert!(compact_artifacts.allows_fallback_config());

        let lineage_record = Command::Kagemusha(KagemushaCommand::LineageRecord(
            KagemushaLineageRecordArgs {
                profile: KagemushaLineageKeyProfile::Append,
                opening_len: 4,
                vk: "lineage.vk".into(),
                out: "lineage.record.norito".into(),
                record_namespace: "offline_kagemusha".to_owned(),
                record_version: 1,
            },
        ));
        assert!(lineage_record.allows_fallback_config());

        let runtime_roots = Command::Roots(RootsArgs {
            asset_id: "asset".to_owned(),
            max: 1,
        });
        assert!(!runtime_roots.allows_fallback_config());
    }

    #[test]
    fn recursive_compact_key_artifacts_summary_matches_readiness_evidence_gate() {
        let summary = kagemusha_recursive_compact_key_artifacts_summary(
            std::path::Path::new("artifacts/kagemusha/recursive-compact-len4.vk"),
            std::path::Path::new("artifacts/kagemusha/recursive-compact-len4.pk"),
            &CompactKeyOutputSummary {
                len: 123,
                sha256: "1".repeat(64),
            },
            &CompactKeyOutputSummary {
                len: 456,
                sha256: "2".repeat(64),
            },
            Some(&CompactKeyOutputSummary {
                len: 789,
                sha256: "3".repeat(64),
            }),
            Some((
                &CompactKeyOutputSummary {
                    len: 321,
                    sha256: "4".repeat(64),
                },
                &CompactKeyOutputSummary {
                    len: 654,
                    sha256: "5".repeat(64),
                },
            )),
        );

        assert_eq!(
            summary,
            "Wrote ABI-7 recursive compact key artifacts for \
             `kagemusha-recursive-compact-v1` opening_len=4 to \
             artifacts/kagemusha/recursive-compact-len4.vk and \
             artifacts/kagemusha/recursive-compact-len4.pk \
             (vk=123 bytes sha256=1111111111111111111111111111111111111111111111111111111111111111, \
            pk=456 bytes sha256=2222222222222222222222222222222222222222222222222222222222222222, \
            record=789 bytes sha256=3333333333333333333333333333333333333333333333333333333333333333, \
            key_artifacts=321 bytes sha256=4444444444444444444444444444444444444444444444444444444444444444, \
            verifier_keys=654 bytes sha256=5555555555555555555555555555555555555555555555555555555555555555)"
        );
    }

    #[test]
    fn recursive_compact_key_artifacts_rejects_one_sided_package_outputs_before_keygen() {
        for (key_artifacts_out, verifier_keys_out) in [
            (
                Some(std::path::PathBuf::from(
                    "recursive-compact-key-artifacts.norito",
                )),
                None,
            ),
            (
                None,
                Some(std::path::PathBuf::from(
                    "recursive-compact-verifier-keys.norito",
                )),
            ),
        ] {
            let mut context = TestContext::new();
            let err = KagemushaRecursiveCompactKeyArtifactsArgs {
                vk_out: "recursive-compact-len4.vk".into(),
                pk_out: "recursive-compact-len4.pk".into(),
                key_artifacts_out,
                verifier_keys_out,
                record_out: None,
                record_namespace: "offline_kagemusha".to_owned(),
                record_version: 1,
            }
            .run(&mut context)
            .expect_err("one-sided package output flags must fail before keygen");

            assert_eq!(
                err.to_string(),
                "--key-artifacts-out and --verifier-keys-out must both be provided for ABI-7 recursive compact production key packages"
            );
            assert!(context.lines.is_empty());
            assert!(context.json_outputs.is_empty());
        }
    }

    #[test]
    fn recursive_compact_key_artifacts_rejects_missing_package_outputs_before_keygen() {
        let mut context = TestContext::new();
        let err = KagemushaRecursiveCompactKeyArtifactsArgs {
            vk_out: "recursive-compact-len4.vk".into(),
            pk_out: "recursive-compact-len4.pk".into(),
            key_artifacts_out: None,
            verifier_keys_out: None,
            record_out: Some("recursive-compact-len4.record.norito".into()),
            record_namespace: "offline_kagemusha".to_owned(),
            record_version: 1,
        }
        .run(&mut context)
        .expect_err("missing package output flags must fail before keygen");

        assert_eq!(
            err.to_string(),
            "--key-artifacts-out and --verifier-keys-out must both be provided for ABI-7 recursive compact production key packages"
        );
        assert!(context.lines.is_empty());
        assert!(context.json_outputs.is_empty());
    }

    fn append_test_tlv(buf: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
        buf.extend_from_slice(tag);
        buf.extend_from_slice(
            &u32::try_from(payload.len())
                .expect("test TLV payload length fits u32")
                .to_le_bytes(),
        );
        buf.extend_from_slice(payload);
    }

    fn h2vk_header(k: u32, selector_compression: u8, fixed_columns: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0x02);
        bytes.extend_from_slice(&k.to_le_bytes());
        bytes.push(selector_compression);
        bytes.extend_from_slice(&fixed_columns.to_le_bytes());
        bytes.extend(vec![
            0x42;
            usize::try_from(fixed_columns)
                .expect("test fixed-column count fits usize")
                * 32
        ]);
        bytes.extend_from_slice(b"test-h2vk-body");
        bytes
    }

    fn lineage_vk_bytes(circuit_id: &str) -> Vec<u8> {
        let ipa_k = iroha_core::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_IPA_MIN_K;
        let mut bytes = b"ZK1\0".to_vec();
        append_test_tlv(&mut bytes, b"CID1", circuit_id.as_bytes());
        append_test_tlv(&mut bytes, b"IPAK", &ipa_k.to_le_bytes());
        append_test_tlv(&mut bytes, b"H2VK", &h2vk_header(ipa_k, 1, 3));
        bytes
    }

    #[test]
    fn kagemusha_lineage_record_from_existing_vk_bytes_canonicalizes_without_keygen() {
        let init_vk =
            lineage_vk_bytes(iroha_core::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_CIRCUIT_ID);
        let init_record = kagemusha_lineage_vk_record_from_bytes(
            KagemushaLineageKeyProfile::Init,
            "test_kagemusha".to_owned(),
            7,
            2,
            init_vk.clone(),
        )
        .expect("init record from existing vk");
        assert_eq!(init_record.version, 7);
        assert_eq!(init_record.namespace, "test_kagemusha");
        assert_eq!(
            init_record.circuit_id,
            iroha_core::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_CIRCUIT_ID
        );
        assert_eq!(init_record.vk_len as usize, init_vk.len());
        assert_eq!(
            init_record
                .key
                .as_ref()
                .expect("embedded init vk")
                .bytes
                .as_slice(),
            init_vk.as_slice()
        );

        let append_vk =
            lineage_vk_bytes(iroha_core::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_CIRCUIT_ID);
        let append_record = kagemusha_lineage_vk_record_from_bytes(
            KagemushaLineageKeyProfile::Append,
            "test_kagemusha".to_owned(),
            8,
            2,
            append_vk.clone(),
        )
        .expect("append record from existing vk");
        assert_eq!(append_record.version, 8);
        assert_eq!(
            append_record.circuit_id,
            iroha_core::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_CIRCUIT_ID
        );
        assert_eq!(append_record.vk_len as usize, append_vk.len());
        assert_eq!(
            append_record
                .key
                .as_ref()
                .expect("embedded append vk")
                .bytes
                .as_slice(),
            append_vk.as_slice()
        );
    }

    #[test]
    fn kagemusha_lineage_record_run_writes_norito_record_from_existing_vk_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let vk_path = temp.path().join("keys/init.vk");
        let out_path = temp.path().join("records/init.record.norito");
        let init_vk =
            lineage_vk_bytes(iroha_core::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_CIRCUIT_ID);
        std::fs::create_dir_all(vk_path.parent().expect("vk parent")).expect("vk dir");
        std::fs::write(&vk_path, &init_vk).expect("write vk");

        let mut context = TestContext::new();
        KagemushaLineageRecordArgs {
            profile: KagemushaLineageKeyProfile::Init,
            opening_len: 2,
            vk: vk_path.clone(),
            out: out_path.clone(),
            record_namespace: "run_kagemusha".to_owned(),
            record_version: 9,
        }
        .run(&mut context)
        .expect("lineage-record run");

        let record_bytes = std::fs::read(&out_path).expect("read record");
        let expected_record = kagemusha_lineage_vk_record_from_bytes(
            KagemushaLineageKeyProfile::Init,
            "run_kagemusha".to_owned(),
            9,
            2,
            init_vk,
        )
        .expect("expected record");
        let expected_bytes = norito::to_bytes(&expected_record).expect("expected record bytes");
        assert_eq!(record_bytes, expected_bytes);
        assert!(
            context
                .lines
                .iter()
                .any(|line| line.contains("Wrote init Reserved-lineage verifier record")),
            "missing lineage-record summary: {:?}",
            context.lines
        );
    }

    #[test]
    fn kagemusha_key_artifact_writer_creates_nested_parent_and_replaces_target() {
        let temp = tempfile::tempdir().expect("tempdir");
        let out_path = temp.path().join("nested/lineage-init-len128.norito");

        write_kagemusha_lineage_key_artifact_file(&out_path, b"old")
            .expect("initial artifact write");
        write_kagemusha_lineage_key_artifact_file(&out_path, b"new-key-material")
            .expect("replacement artifact write");

        assert_eq!(
            std::fs::read(&out_path).expect("read replaced artifact"),
            b"new-key-material"
        );
        let leftovers = std::fs::read_dir(out_path.parent().expect("output parent"))
            .expect("read output parent")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".lineage-init-len128.norito.tmp-")
            })
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn kagemusha_key_artifact_writer_rejects_directory_output_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let err = write_kagemusha_lineage_key_artifact_file(temp.path(), b"key-material")
            .expect_err("directory output must not be accepted as a key artifact file");

        assert!(
            format!("{err}").contains("Is a directory")
                || format!("{err}").contains("is a directory"),
            "unexpected directory write error: {err}"
        );
    }

    #[test]
    fn kagemusha_lineage_record_from_existing_vk_bytes_rejects_adversarial_inputs() {
        let init_vk =
            lineage_vk_bytes(iroha_core::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_CIRCUIT_ID);
        let err = kagemusha_lineage_vk_record_from_bytes(
            KagemushaLineageKeyProfile::Append,
            "test_kagemusha".to_owned(),
            1,
            2,
            init_vk,
        )
        .expect_err("init vk must not be accepted as append");
        assert!(
            format!("{err}").contains("is not `kagemusha-recursive-spend-lineage-append-v1`"),
            "unexpected profile mismatch error: {err}"
        );

        let err = kagemusha_lineage_vk_record_from_bytes(
            KagemushaLineageKeyProfile::Init,
            "test_kagemusha".to_owned(),
            1,
            2,
            Vec::new(),
        )
        .expect_err("empty vk bytes must reject");
        assert!(
            format!("{err}").contains("must be non-empty"),
            "unexpected empty-key error: {err}"
        );

        let append_vk =
            lineage_vk_bytes(iroha_core::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_CIRCUIT_ID);
        let err = kagemusha_lineage_vk_record_from_bytes(
            KagemushaLineageKeyProfile::Append,
            "test_kagemusha".to_owned(),
            1,
            3,
            append_vk,
        )
        .expect_err("unsupported opening length must reject");
        assert!(
            format!("{err}").contains("opening length `3` is unsupported"),
            "unexpected opening-length error: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_compact_record_from_existing_vk_bytes_rejects_adversarial_inputs() {
        let lineage_vk =
            lineage_vk_bytes(iroha_core::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_CIRCUIT_ID);
        let err = kagemusha_recursive_compact_vk_record_from_bytes(
            "test_kagemusha".to_owned(),
            1,
            lineage_vk,
        )
        .expect_err("lineage vk must not be accepted as compact-token vk");
        assert!(
            format!("{err}").contains(iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID),
            "unexpected compact circuit-id error: {err}"
        );

        let err = kagemusha_recursive_compact_vk_record_from_bytes(
            "test_kagemusha".to_owned(),
            1,
            Vec::new(),
        )
        .expect_err("empty compact vk bytes must reject");
        assert!(
            format!("{err}").contains("must be non-empty"),
            "unexpected empty compact-key error: {err}"
        );
    }

    #[test]
    fn build_proof_attachment_from_json_vk_ref() {
        // proof_b64 = "Hello" in base64
        let proof_b64 = "SGVsbG8=";
        let json = format!(
            r#"{{
                "backend": "halo2/ipa",
                "proof_b64": "{proof_b64}",
                "vk_ref": {{ "backend": "halo2/ipa", "name": "vk_transfer" }},
                "vk_commitment_hex": "0000000000000000000000000000000000000000000000000000000000000000"
            }}"#
        );
        let v = norito::json::from_str(&json).expect("vk_ref json");
        let att = build_proof_attachment_from_json(&v).expect("ok");
        assert_eq!(att.backend.as_str(), "halo2/ipa");
        assert_eq!(att.proof.backend.as_str(), "halo2/ipa");
        assert_eq!(att.proof.bytes, b"Hello");
        assert_eq!(att.vk_ref.name.as_str(), "vk_transfer");
        assert_eq!(att.vk_commitment.unwrap(), [0u8; 32]);
    }

    #[test]
    fn build_proof_attachment_from_json_rejects_legacy_inline_vk_field() {
        for field in [
            "vk_inline",
            "vkInline",
            "verifyingKeyInline",
            "verifying_key_inline",
        ] {
            let json = format!(
                r#"{{
                    "backend": "halo2/ipa",
                    "proof_b64": "AA==",
                    "vk_ref": {{ "backend": "halo2/ipa", "name": "vk_transfer" }},
                    "{field}": {{ "backend": "halo2/ipa", "bytes_b64": "AQID" }}
                }}"#
            );
            let v = norito::json::from_str(&json).expect("legacy inline json");
            let err = build_proof_attachment_from_json(&v).expect_err("legacy inline vk rejected");
            assert!(format!("{err}").contains("legacy inline verifying-key field"));
        }
    }

    #[test]
    fn build_proof_attachment_from_json_rejects_short_vk_commitment() {
        let json = r#"{
            "backend": "halo2/ipa",
            "proof_b64": "AA==",
            "vk_ref": { "backend": "halo2/ipa", "name": "vk_transfer" },
            "vk_commitment_hex": "abcd"
        }"#;
        let v = norito::json::from_str(json).expect("short commitment json");
        let err = build_proof_attachment_from_json(&v).expect_err("short commitment rejected");
        assert!(format!("{err}").contains("32 bytes"));
    }

    #[test]
    fn build_proof_attachment_from_json_rejects_non_object_vk_ref() {
        let json = r#"{
            "backend": "halo2/ipa",
            "proof_b64": "AA==",
            "vk_ref": "halo2/ipa:vk_transfer"
        }"#;
        let v = norito::json::from_str(json).expect("string vk_ref json");
        let err = build_proof_attachment_from_json(&v).expect_err("string vk_ref rejected");
        assert!(format!("{err}").contains("vk_ref must be provided"));
    }

    #[test]
    fn build_proof_attachment_from_json_rejects_vk_ref_backend_mismatch() {
        let json = r#"{
            "backend": "halo2/ipa",
            "proof_b64": "AA==",
            "vk_ref": { "backend": "stark/fri", "name": "vk_transfer" }
        }"#;
        let v = norito::json::from_str(json).expect("vk backend mismatch json");
        let err = build_proof_attachment_from_json(&v).expect_err("vk backend mismatch rejected");
        assert!(format!("{err}").contains("vk_ref.backend must match backend"));
    }

    #[test]
    fn build_proof_attachment_from_json_rejects_vk_reference_shadow_field() {
        let json = r#"{
            "backend": "halo2/ipa",
            "proof_b64": "AA==",
            "vk_ref": { "backend": "halo2/ipa", "name": "vk_transfer" },
            "vk_reference": { "backend": "halo2/ipa", "name": "vk_shadow" }
        }"#;
        let v = norito::json::from_str(json).expect("vk_reference shadow json");
        let err = build_proof_attachment_from_json(&v).expect_err("shadow alias rejected");
        assert!(format!("{err}").contains("unknown proof attachment field `vk_reference`"));
    }

    #[test]
    fn build_proof_attachment_from_json_rejects_bridge_only_proof_backend_shadow() {
        let json = r#"{
            "backend": "halo2/ipa",
            "proof_backend": "stark/fri",
            "proof_b64": "AA==",
            "vk_ref": { "backend": "halo2/ipa", "name": "vk_transfer" }
        }"#;
        let v = norito::json::from_str(json).expect("proof_backend shadow json");
        let err = build_proof_attachment_from_json(&v).expect_err("proof_backend shadow rejected");
        assert!(format!("{err}").contains("unknown proof attachment field `proof_backend`"));
    }

    #[test]
    fn build_proof_attachment_from_json_rejects_nested_vk_ref_shadow_field() {
        let json = r#"{
            "backend": "halo2/ipa",
            "proof_b64": "AA==",
            "vk_ref": {
                "backend": "halo2/ipa",
                "name": "vk_transfer",
                "vk_reference": "shadow"
            }
        }"#;
        let v = norito::json::from_str(json).expect("nested vk_ref shadow json");
        let err = build_proof_attachment_from_json(&v).expect_err("nested shadow rejected");
        assert!(format!("{err}").contains("unknown vk_ref field `vk_reference`"));
    }

    #[test]
    fn build_proof_attachment_from_json_rejects_blank_vk_ref_name() {
        let json = r#"{
            "backend": "halo2/ipa",
            "proof_b64": "AA==",
            "vk_ref": { "backend": "halo2/ipa", "name": "   " }
        }"#;
        let v = norito::json::from_str(json).expect("blank vk_ref name json");
        let err = build_proof_attachment_from_json(&v).expect_err("blank vk_ref name rejected");
        assert!(format!("{err}").contains("vk_ref.name must be non-empty"));
    }

    #[test]
    fn build_proof_attachment_from_json_rejects_blank_backend_fields() {
        for (json, expected) in [
            (
                r#"{
                    "backend": "   ",
                    "proof_b64": "AA==",
                    "vk_ref": { "backend": "halo2/ipa", "name": "vk_transfer" }
                }"#,
                "unsupported production verifier backend",
            ),
            (
                r#"{
                    "backend": "halo2/ipa",
                    "proof_b64": "AA==",
                    "vk_ref": { "backend": "   ", "name": "vk_transfer" }
                }"#,
                "unsupported production verifier backend",
            ),
        ] {
            let v = norito::json::from_str(json).expect("blank backend json");
            let err =
                build_proof_attachment_from_json(&v).expect_err("blank backend field rejected");
            assert!(
                format!("{err}").contains(expected),
                "expected error to mention {expected}, got {err}"
            );
        }
    }

    #[test]
    fn build_proof_attachment_from_json_rejects_unsupported_production_backends() {
        for backend in [
            " halo2/ipa",
            "halo2/ipa ",
            "HALO2/IPA",
            "stark/FRI",
            "halo2/ipa::ivm-execution-v1",
            "halo2//ipa",
            "halo2/ipa:",
            "halo2/ipa.",
            "halo2/ipa/.ivm-execution-v1",
            "halo2/ipa:ivm..execution-v1",
            "halo2/ipa/orchard",
            "halo2/kzg",
            "groth16/bls12-377",
            "mock/dev",
            "stark/fri/miden",
            "stark/fri/latest",
            "stark/fri/random-profile",
            "stark/fri/sha512-goldilocks",
            "stark/fri/boi-audited",
            "stark/fri/external-security-review",
            "stark/fri/s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
            "halo2/ipa:release-ready",
            "halo2/ipa:certified-mainnet",
            "halo2/ipa:third-party-audited",
            "halo2/pasta/tiny-add",
            "halo2/ipa:tiny-add",
            "halo2/pasta/anon-transfer-2x2",
            "halo2/ipa:anon-transfer-2x2",
            "halo2/pasta/vote-bool-commit",
            "halo2/ipa:vote-bool-commit",
            "halo2/pasta/asset-hidden-transfer-public-test",
        ] {
            let json = format!(
                r#"{{
                    "backend": "{backend}",
                    "proof_b64": "AA==",
                    "vk_ref": {{ "backend": "{backend}", "name": "vk_transfer" }}
                }}"#
            );
            let v = norito::json::from_str(&json).expect("unsupported backend json");
            let err = build_proof_attachment_from_json(&v)
                .expect_err("unsupported production backend rejected");
            assert!(
                format!("{err}").contains("unsupported production verifier backend"),
                "expected unsupported backend error for {backend:?}, got {err}"
            );
        }
    }

    #[test]
    fn proof_filter_args_reject_unsupported_backend_labels() {
        for backend in [
            " halo2/ipa",
            "halo2/ipa ",
            "\thalo2/ipa",
            "halo2/ipa\n",
            "HALO2/IPA",
            "stark/FRI",
            "halo2/ipa::ivm-execution-v1",
            "halo2//ipa",
            "halo2/ipa:",
            "halo2/ipa.",
            "halo2/ipa/.ivm-execution-v1",
            "halo2/ipa:ivm..execution-v1",
            "halo2/ipa/orchard",
            "halo2/kzg",
            "mock/dev",
            "stark/fri/latest",
            "stark/fri/random-profile",
            "stark/fri/boi-audited",
            "halo2/ipa:release-ready",
            "halo2/ipa:tiny-add",
            "halo2/pasta/asset-hidden-transfer-public-test",
        ] {
            let args = ProofFilterArgs {
                backend: Some(backend.to_string()),
                ..ProofFilterArgs::default()
            };
            let err = args
                .as_filter()
                .expect_err("unsupported proof filter backend rejected");
            assert!(format!("{err}").contains("unsupported production verifier backend"));
        }
    }

    #[test]
    fn parse_vk_id_pair_rejects_unsupported_backend_labels_and_preserves_colon_aliases() {
        for literal in [
            " halo2/ipa:vk_transfer",
            "halo2/ipa :vk_transfer",
            "HALO2/IPA:vk_transfer",
            "stark/FRI:vk_transfer",
            "halo2/ipa::ivm-execution-v1:vk_transfer",
            "halo2//ipa:vk_transfer",
            "halo2/ipa.:vk_transfer",
            "halo2/ipa:ivm..execution-v1:vk_transfer",
            "halo2/ipa/orchard:vk_transfer",
            "stark/fri/latest:vk_transfer",
            "stark/fri/random-profile:vk_transfer",
            "stark/fri/boi-audited:vk_transfer",
            "halo2/ipa:release-ready:vk_transfer",
            "halo2/ipa:tiny-add:vk_transfer",
            "halo2/pasta/asset-hidden-transfer-public-test:vk_transfer",
            "mock/dev:vk_transfer",
            "halo2/ipa:",
            "halo2/ipa:vk:shadow",
        ] {
            assert!(
                parse_vk_id_pair(literal).is_err(),
                "{literal:?} must reject before building a verifying-key id"
            );
        }

        let parsed =
            parse_vk_id_pair("halo2/ipa:ivm-execution-v1:vk_ivm").expect("colon alias vk id");
        assert_eq!(parsed.backend.as_str(), "halo2/ipa:ivm-execution-v1");
        assert_eq!(parsed.name.as_str(), "vk_ivm");

        let parsed =
            parse_vk_id_pair("stark/fri/poseidon2-goldilocks:vk_stark").expect("stark vk id");
        assert_eq!(parsed.backend.as_str(), "stark/fri/poseidon2-goldilocks");
        assert_eq!(parsed.name.as_str(), "vk_stark");

        let parsed = parse_vk_id_pair("halo2/pasta/kagemusha-recursive-compact-v1:vk_compact")
            .expect("compact Kagemusha vk id");
        assert_eq!(
            parsed.backend.as_str(),
            "halo2/pasta/kagemusha-recursive-compact-v1"
        );
        assert_eq!(parsed.name.as_str(), "vk_compact");
    }

    #[test]
    fn parse_hex32_lower_canonicalizes_and_rejects_malformed_hashes() {
        assert_eq!(
            parse_hex32_lower(&format!("0x{}", "AA".repeat(32)), "proof hash").expect("hash"),
            "aa".repeat(32)
        );
        for value in [
            String::new(),
            "abc".to_string(),
            "z".repeat(64),
            "a".repeat(63),
            format!("0x0x{}", "aa".repeat(32)),
        ] {
            assert!(
                parse_hex32_lower(&value, "proof hash").is_err(),
                "{value:?} must reject as malformed proof hash"
            );
        }
    }

    #[test]
    fn parse_hex_array_exact_length() {
        let value = "01".repeat(32);
        let arr = parse_hex_array::<32>(&value).expect("parse hex array");
        assert_eq!(arr, [1u8; 32]);
    }

    #[test]
    fn parse_hex_array_rejects_wrong_length() {
        let err = parse_hex_array::<24>("00").expect_err("should fail");
        assert!(format!("{err}").contains("expected 24 bytes"));
    }

    #[test]
    fn encode_encrypted_payload_returns_expected_bytes() {
        use base64::Engine as _;
        let epk = "07".repeat(32);
        let nonce = "22".repeat(24);
        let (payload, bytes) =
            encode_encrypted_payload(&epk, &nonce, "AQIDBA==").expect("encode envelope");
        let expected_b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let decoded: iroha::data_model::confidential::ConfidentialEncryptedPayload =
            norito::codec::decode_adaptive(&bytes).expect("decode envelope");
        assert_eq!(payload, decoded);
        assert!(!expected_b64.is_empty());
    }

    #[test]
    fn vk_submission_backend_parser_preserves_pending_protocol_tags() {
        use iroha::data_model::zk::BackendTag;

        for (label, expected) in [
            ("halo2/ipa/orchard", BackendTag::Halo2IpaOrchard),
            ("groth16/bls12-377", BackendTag::Groth16Bls12377),
            ("penumbra-masp", BackendTag::Groth16Bls12377),
            ("monero-fcmp++", BackendTag::FcmpPlusPlusCurveTree),
            ("fcmp++", BackendTag::FcmpPlusPlusCurveTree),
            ("sis-with-hints", BackendTag::SisWithHints),
            ("post-quantum-masp", BackendTag::PqMaspStarkFri),
        ] {
            assert_eq!(
                vk_backend_tag_from_label(label),
                expected,
                "{label} must not collapse into a generic supported backend",
            );
        }
    }

    fn sample_prepared_vk_submission(
        key_pair: &iroha_crypto::KeyPair,
        name: &str,
    ) -> PreparedVkSubmission {
        use iroha::data_model::{account::AccountId, proof::VerifyingKeyRecord, zk::BackendTag};

        let mut record = VerifyingKeyRecord::new_with_owner(
            1,
            "test-zk-vk-circuit-v1",
            None,
            "core",
            BackendTag::Halo2IpaPasta,
            "pasta",
            [7_u8; 32],
            [9_u8; 32],
        );
        record.vk_len = 32;
        record.max_proof_bytes = 4096;

        PreparedVkSubmission {
            authority: AccountId::new(key_pair.public_key().clone()),
            private_key: iroha_crypto::ExposedPrivateKey(key_pair.private_key().clone()),
            id: iroha::data_model::proof::VerifyingKeyId::new("halo2/ipa", name),
            record,
        }
    }

    #[test]
    fn vk_register_update_checked_transaction_helpers_verify() {
        let key_pair = iroha_crypto::KeyPair::try_from_seed(
            vec![71, 72, 73, 74],
            iroha_crypto::Algorithm::Ed25519,
        )
        .expect("nonzero deterministic test key");
        let chain =
            iroha::data_model::prelude::ChainId::from("00000000-0000-0000-0000-000000000000");

        let register_tx = signed_vk_register_transaction(
            chain.clone(),
            iroha::data_model::prelude::Metadata::default(),
            sample_prepared_vk_submission(&key_pair, "vk_register_checked"),
        )
        .expect("register transaction signs through checked helper");
        register_tx
            .verify_signature()
            .expect("register transaction signature verifies");

        let update_tx = signed_vk_update_transaction(
            chain,
            iroha::data_model::prelude::Metadata::default(),
            sample_prepared_vk_submission(&key_pair, "vk_update_checked"),
        )
        .expect("update transaction signs through checked helper");
        update_tx
            .verify_signature()
            .expect("update transaction signature verifies");
    }

    #[test]
    fn encode_encrypted_payload_rejects_empty_ciphertext() {
        let epk = "07".repeat(32);
        let nonce = "22".repeat(24);
        let err = encode_encrypted_payload(&epk, &nonce, "")
            .expect_err("empty encrypted payload ciphertext must fail");
        assert!(
            format!("{err}").contains("ciphertext must not be empty"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn encode_encrypted_payload_rejects_low_order_ephemeral_key() {
        let epk = "00".repeat(32);
        let nonce = "22".repeat(24);
        let err = encode_encrypted_payload(&epk, &nonce, "AQIDBA==")
            .expect_err("low-order X25519 ephemeral key must fail");
        assert!(
            format!("{err}").contains("low-order"),
            "unexpected error: {err}"
        );
    }
}

impl Run for UnshieldArgs {
    fn run<C: RunContext>(self, context: &mut C) -> eyre::Result<()> {
        use iroha::data_model::prelude::{AccountId, AssetDefinitionId, InstructionBox};
        let asset = AssetDefinitionId::parse_address_literal(&self.asset)?;
        let to = crate::resolve_account_id(context, &self.to).wrap_err("failed to resolve --to")?;
        let inputs = parse_inputs_csv(&self.inputs)?;
        let proof_json_str = std::fs::read_to_string(&self.proof_json)?;
        let v: norito::json::Value = norito::json::from_str(&proof_json_str)?;
        let mut proof_att = build_proof_attachment_from_json(&v)?;
        let root_hint = match self.root_hint {
            Some(h) => Some(parse_hex32(&h)?),
            None => None,
        };
        // Optional: derive envelope hash from proof_b64 for audit binding (demo).
        // We use blake2b-32 of proof bytes as a placeholder.
        if proof_att.envelope_hash.is_none() {
            use iroha::crypto::Hash;
            let h = Hash::new(&proof_att.proof.bytes);
            proof_att.envelope_hash = Some(h.into());
        }
        let ib: InstructionBox = iroha::data_model::isi::zk::Unshield::new(
            asset,
            to,
            self.amount,
            inputs,
            proof_att,
            root_hint,
        )
        .into();
        context.finish(vec![ib])
    }
}

// --------------- Register ZK Asset (Hybrid) ---------------

#[derive(clap::Args, Debug)]
pub struct ZkRegisterAssetArgs {
    /// Canonical unprefixed Base58 `AssetDefinitionId`
    #[arg(long, value_name = "ASSET_ID")]
    asset: String,
    /// Allow shielding from public to shielded (default: true)
    #[arg(long, default_value_t = true)]
    allow_shield: bool,
    /// Allow unshielding from shielded to public (default: true)
    #[arg(long, default_value_t = true)]
    allow_unshield: bool,
    /// Verifying key id for private transfers (format: `<backend>:<name>`, e.g., `halo2/ipa:vk_transfer`)
    #[arg(long, value_name = "BACKEND:NAME")]
    vk_transfer: Option<String>,
    /// Verifying key id for unshield proofs (format: `<backend>:<name>`)
    #[arg(long, value_name = "BACKEND:NAME")]
    vk_unshield: Option<String>,
    /// Verifying key id for shield proofs (optional; format: `<backend>:<name>`)
    #[arg(long, value_name = "BACKEND:NAME")]
    vk_shield: Option<String>,
}

fn parse_vk_id_pair(s: &str) -> eyre::Result<iroha::data_model::proof::VerifyingKeyId> {
    use iroha::data_model::proof::VerifyingKeyId;
    let (backend, name) = s
        .rsplit_once(':')
        .ok_or_else(|| eyre::eyre!("expected BACKEND:NAME for verifying key id"))?;
    let backend = ensure_production_verify_backend_label(backend, "verifying key backend")?;
    let name = name.trim();
    if name.is_empty() {
        eyre::bail!("verifying key name must be non-empty");
    }
    if name.contains(':') {
        eyre::bail!("verifying key name must not contain ':'");
    }
    Ok(VerifyingKeyId::new(backend, name))
}

impl Run for ZkRegisterAssetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> eyre::Result<()> {
        use iroha::data_model::isi::zk::{RegisterZkAsset, ZkAssetMode};
        use iroha::data_model::prelude::{AssetDefinitionId, InstructionBox};

        let asset = AssetDefinitionId::parse_address_literal(&self.asset)?;
        let vk_transfer = match self.vk_transfer {
            Some(s) => Some(parse_vk_id_pair(&s)?),
            None => None,
        };
        let vk_unshield = match self.vk_unshield {
            Some(s) => Some(parse_vk_id_pair(&s)?),
            None => None,
        };
        let vk_shield = match self.vk_shield {
            Some(s) => Some(parse_vk_id_pair(&s)?),
            None => None,
        };
        let ib: InstructionBox = RegisterZkAsset::new(
            asset,
            ZkAssetMode::Hybrid,
            self.allow_shield,
            self.allow_unshield,
            vk_transfer,
            vk_unshield,
            vk_shield,
        )
        .into();
        context.finish(vec![ib])
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum VkCommand {
    /// Register a verifying key record (signed transaction via Torii app API)
    Register(VkRegisterArgs),
    /// Update an existing verifying key record (version must increase)
    Update(VkUpdateArgs),
    /// Get a verifying key record by backend and name
    Get(VkGetArgs),
}

impl Run for VkCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            VkCommand::Register(args) => args.run(context),
            VkCommand::Update(args) => args.run(context),
            VkCommand::Get(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct VkRegisterArgs {
    /// Path to a JSON DTO file for register (authority, `private_key`, backend, name, version, optional `vk_bytes` (base64) or `commitment_hex`)
    #[arg(long, value_name = "PATH")]
    json: std::path::PathBuf,
}

#[derive(Debug, Clone, norito::json::JsonDeserialize)]
struct VkSubmissionJson {
    authority: iroha::data_model::account::AccountId,
    private_key: iroha::data_model::prelude::ExposedPrivateKey,
    backend: String,
    name: String,
    version: u32,
    circuit_id: String,
    public_inputs_schema_hash_hex: String,
    #[norito(default)]
    curve: Option<String>,
    #[norito(default)]
    gas_schedule_id: Option<String>,
    #[norito(default)]
    vk_len: Option<u32>,
    #[norito(default)]
    max_proof_bytes: Option<u32>,
    #[norito(default)]
    metadata_uri_cid: Option<String>,
    #[norito(default)]
    vk_bytes_cid: Option<String>,
    #[norito(default)]
    activation_height: Option<u64>,
    #[norito(default)]
    withdraw_height: Option<u64>,
    #[norito(default)]
    commitment_hex: Option<String>,
    #[norito(default)]
    vk_bytes: Option<String>,
    #[norito(default)]
    status: Option<iroha::data_model::confidential::ConfidentialStatus>,
}

struct PreparedVkSubmission {
    authority: iroha::data_model::account::AccountId,
    private_key: iroha::data_model::prelude::ExposedPrivateKey,
    id: iroha::data_model::proof::VerifyingKeyId,
    record: iroha::data_model::proof::VerifyingKeyRecord,
}

fn signed_vk_register_transaction(
    chain: iroha::data_model::prelude::ChainId,
    metadata: iroha::data_model::prelude::Metadata,
    prepared: PreparedVkSubmission,
) -> Result<iroha::data_model::prelude::SignedTransaction> {
    use iroha::data_model::{isi::verifying_keys, prelude::TransactionBuilder};

    TransactionBuilder::new(chain, prepared.authority.into())
        .with_metadata(metadata)
        .with_instructions(core::iter::once(InstructionBox::from(
            verifying_keys::RegisterVerifyingKey {
                id: prepared.id,
                record: prepared.record,
            },
        )))
        .try_sign(&prepared.private_key.0)
        .wrap_err("failed to sign VK register transaction")
}

fn signed_vk_update_transaction(
    chain: iroha::data_model::prelude::ChainId,
    metadata: iroha::data_model::prelude::Metadata,
    prepared: PreparedVkSubmission,
) -> Result<iroha::data_model::prelude::SignedTransaction> {
    use iroha::data_model::{isi::verifying_keys, prelude::TransactionBuilder};

    TransactionBuilder::new(chain, prepared.authority.into())
        .with_metadata(metadata)
        .with_instructions(core::iter::once(InstructionBox::from(
            verifying_keys::UpdateVerifyingKey {
                id: prepared.id,
                record: prepared.record,
            },
        )))
        .try_sign(&prepared.private_key.0)
        .wrap_err("failed to sign VK update transaction")
}

fn parse_hex32_str(value: &str, field: &str) -> Result<[u8; 32]> {
    let trimmed = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(trimmed).wrap_err_with(|| format!("invalid {field}"))?;
    if bytes.len() != 32 {
        eyre::bail!("{field} must be 32 bytes");
    }
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn parse_commitment_hex(value: &str) -> Result<[u8; 32]> {
    parse_hex32_str(value, "commitment_hex")
}

fn vk_backend_tag_from_label(label: &str) -> iroha::data_model::zk::BackendTag {
    iroha::data_model::zk::BackendTag::from_catalog_label(label)
}

fn build_vk_record(
    payload: &VkSubmissionJson,
) -> Result<iroha::data_model::proof::VerifyingKeyRecord> {
    use iroha::data_model::{
        confidential::ConfidentialStatus,
        proof::{VerifyingKeyBox, VerifyingKeyRecord},
    };
    use iroha_core::zk::hash_vk;

    let backend =
        ensure_production_verify_backend_label(&payload.backend, "verifying key backend")?;

    let vk_bytes = match payload.vk_bytes.as_deref() {
        Some(value) => Some(
            base64::engine::general_purpose::STANDARD
                .decode(value.as_bytes())
                .wrap_err("failed to decode vk_bytes base64")?,
        ),
        None => None,
    };

    let mut key_opt = None;
    let commitment;
    let vk_len_value;
    if let Some(bytes) = vk_bytes {
        let vk = VerifyingKeyBox::new(backend.into(), bytes);
        let actual_commitment = hash_vk(&vk);
        if let Some(hex) = payload.commitment_hex.as_deref() {
            let parsed = parse_commitment_hex(hex)?;
            if parsed != actual_commitment {
                eyre::bail!("commitment mismatch with provided vk_bytes");
            }
        }
        commitment = actual_commitment;
        let actual_len = vk.bytes.len() as u32;
        if let Some(explicit_len) = payload.vk_len {
            if explicit_len != actual_len {
                eyre::bail!("vk_len mismatch with provided vk_bytes");
            }
        }
        vk_len_value = actual_len;
        key_opt = Some(vk);
    } else if let Some(hex) = payload.commitment_hex.as_deref() {
        commitment = parse_commitment_hex(hex)?;
        let explicit_len = payload
            .vk_len
            .ok_or_else(|| eyre::eyre!("vk_len required when vk_bytes omitted"))?;
        if explicit_len == 0 {
            eyre::bail!("vk_len must be > 0");
        }
        vk_len_value = explicit_len;
    } else {
        eyre::bail!("provide either vk_bytes or commitment_hex");
    }

    let backend_tag = vk_backend_tag_from_label(backend);
    let schema_hash = parse_hex32_str(
        &payload.public_inputs_schema_hash_hex,
        "public_inputs_schema_hash_hex",
    )?;
    if payload
        .gas_schedule_id
        .as_ref()
        .is_some_and(|gas_schedule_id| gas_schedule_id.trim().is_empty())
    {
        eyre::bail!("gas_schedule_id must not be empty");
    }
    if let (Some(activation_height), Some(withdraw_height)) =
        (payload.activation_height, payload.withdraw_height)
    {
        if activation_height > withdraw_height {
            eyre::bail!("withdraw_height must be >= activation_height");
        }
    }

    let mut record = VerifyingKeyRecord::new_with_owner(
        payload.version,
        payload.circuit_id.clone(),
        None,
        "core",
        backend_tag,
        payload.curve.clone().unwrap_or_else(|| "unknown".into()),
        schema_hash,
        commitment,
    );
    record.vk_len = vk_len_value;
    record.max_proof_bytes = payload.max_proof_bytes.unwrap_or(0);
    record.status = payload.status.unwrap_or(ConfidentialStatus::Active);
    record.metadata_uri_cid = payload.metadata_uri_cid.clone();
    record.vk_bytes_cid = payload.vk_bytes_cid.clone();
    record.activation_height = payload.activation_height;
    record.withdraw_height = payload.withdraw_height;
    record.key = key_opt;
    record.gas_schedule_id = payload.gas_schedule_id.clone();
    Ok(record)
}

fn load_vk_submission(path: &std::path::Path) -> Result<PreparedVkSubmission> {
    let raw = std::fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read {}", path.display()))?;
    let payload: VkSubmissionJson =
        norito::json::from_str(&raw).wrap_err("failed to parse VK submission JSON")?;
    let backend =
        ensure_production_verify_backend_label(&payload.backend, "verifying key backend")?;
    let id =
        iroha::data_model::proof::VerifyingKeyId::new(backend.to_string(), payload.name.clone());
    let record = build_vk_record(&payload)?;
    Ok(PreparedVkSubmission {
        authority: payload.authority,
        private_key: payload.private_key,
        id,
        record,
    })
}

impl Run for VkRegisterArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let prepared = load_vk_submission(&self.json)?;
        let metadata = context.transaction_metadata().cloned().unwrap_or_default();
        let tx =
            signed_vk_register_transaction(context.config().chain.clone(), metadata, prepared)?;
        let hash = tx.hash();
        client
            .submit_transaction(&tx)
            .wrap_err("failed to submit VK register transaction")?;
        context.println(format!("VK register submitted: {hash}"))?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct VkUpdateArgs {
    /// Path to a JSON DTO file for update (authority, `private_key`, backend, name, version, optional `vk_bytes` or `commitment_hex`)
    #[arg(long, value_name = "PATH")]
    json: std::path::PathBuf,
}

impl Run for VkUpdateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let prepared = load_vk_submission(&self.json)?;
        let metadata = context.transaction_metadata().cloned().unwrap_or_default();
        let tx = signed_vk_update_transaction(context.config().chain.clone(), metadata, prepared)?;
        let hash = tx.hash();
        client
            .submit_transaction(&tx)
            .wrap_err("failed to submit VK update transaction")?;
        context.println(format!("VK update submitted: {hash}"))?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct VkGetArgs {
    /// Backend identifier (e.g., "halo2/ipa")
    #[arg(long, value_name = "BACKEND")]
    backend: String,
    /// Verifying key name
    #[arg(long, value_name = "NAME")]
    name: String,
}

impl Run for VkGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let backend =
            ensure_production_verify_backend_label(&self.backend, "verifying key get backend")?;
        let name = self.name.trim();
        if name.is_empty() {
            eyre::bail!("verifying key get name must be non-empty");
        }
        let client: Client = context.client_from_config();
        let v = client.get_zk_vk_json(backend, name)?;
        context.print_data(&v)?;
        Ok(())
    }
}
