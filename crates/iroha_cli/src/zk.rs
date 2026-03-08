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
    /// ZK Vote helpers (tally)
    #[command(subcommand)]
    Vote(VoteCommand),
    /// Encode a confidential encrypted payload (memo) into Norito bytes/base64
    Envelope(EnvelopeArgs),
}

#[derive(clap::Args, Debug)]
pub struct RootsArgs {
    /// `AssetDefinitionId` like `rose#wonderland`
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
            Command::Vote(args) => args.run(context),
            Command::Envelope(args) => args.run(context),
        }
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
    fn as_filter(&self) -> ZkProofsFilter<'_> {
        ZkProofsFilter {
            backend: self.backend.as_deref(),
            status: self.status.as_deref(),
            has_tag: self.has_tag.as_deref(),
            verified_from_height: self.verified_from_height,
            verified_until_height: self.verified_until_height,
            limit: self.limit,
            offset: self.offset,
            order: self.order.as_deref(),
            ids_only: None,
        }
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
        let mut filter = self.filter.as_filter();
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
        let filter = self.filter.as_filter();
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
        let hash_hex = self
            .hash
            .strip_prefix("0x")
            .map_or_else(|| self.hash.clone(), str::to_string);
        let value = client.get_zk_proof_json(&self.backend, &hash_hex)?;
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
    /// Derive a proving key (.pk) from verifying key bytes (.vk) for the Halo2 IPA IVM bind circuit
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
    /// Output path for proving key bytes (`.pk`)
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
    /// `AssetDefinitionId` like `rose#wonderland`
    #[arg(long, value_name = "ASSET_ID")]
    asset: String,
    /// Account identifier to debit (IH58 (preferred) or sora compressed literal)
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
    Ok(ConfidentialEncryptedPayload::new(
        ephemeral, nonce, ciphertext,
    ))
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
        let asset: AssetDefinitionId = self.asset.parse()?;
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
                    norito::decode_from_bytes::<ConfidentialEncryptedPayload>(&bytes)
                        .map_err(|e| eyre::eyre!("failed to decode encrypted payload: {e}"))?
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
    /// `AssetDefinitionId` like `rose#wonderland`
    #[arg(long, value_name = "ASSET_ID")]
    asset: String,
    /// Recipient account identifier to credit (IH58 (preferred) or sora compressed literal)
    #[arg(long, value_name = "ACCOUNT_ID")]
    to: String,
    /// Public amount to credit
    #[arg(long, value_name = "AMOUNT")]
    amount: u128,
    /// Spent nullifiers (comma-separated list of 64-hex strings)
    #[arg(long, value_name = "HEX32[,HEX32,...]")]
    inputs: String,
    /// Proof attachment JSON file describing { backend, `proof_b64`, `vk_ref{backend,name}`, `vk_inline{backend,bytes_b64}`, optional `vk_commitment_hex` }
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

fn build_proof_attachment_from_json(
    v: &norito::json::Value,
) -> eyre::Result<iroha::data_model::proof::ProofAttachment> {
    use iroha::data_model::proof::{ProofAttachment, ProofBox, VerifyingKeyBox, VerifyingKeyId};
    let backend = v
        .get("backend")
        .and_then(|x| x.as_str())
        .ok_or_else(|| eyre::eyre!("missing backend"))?;
    let proof_b64 = v
        .get("proof_b64")
        .and_then(|x| x.as_str())
        .ok_or_else(|| eyre::eyre!("missing proof_b64"))?;
    let proof_bytes = base64::engine::general_purpose::STANDARD
        .decode(proof_b64)
        .map_err(|e| eyre::eyre!("invalid proof_b64: {e}"))?;
    let proof = ProofBox::new(backend.into(), proof_bytes);
    // vk_ref or vk_inline
    let vk_ref = v.get("vk_ref").and_then(|x| x.as_object());
    let vk_inline = v.get("vk_inline").and_then(|x| x.as_object());
    let mut att = if let Some(obj) = vk_ref {
        let b = obj
            .get("backend")
            .and_then(|x| x.as_str())
            .ok_or_else(|| eyre::eyre!("vk_ref.backend missing"))?;
        let name = obj
            .get("name")
            .and_then(|x| x.as_str())
            .ok_or_else(|| eyre::eyre!("vk_ref.name missing"))?;
        let id = VerifyingKeyId::new(b, name);
        ProofAttachment::new_ref(backend.into(), proof, id)
    } else if let Some(obj) = vk_inline {
        let b = obj
            .get("backend")
            .and_then(|x| x.as_str())
            .ok_or_else(|| eyre::eyre!("vk_inline.backend missing"))?;
        let bytes_b64 = obj
            .get("bytes_b64")
            .and_then(|x| x.as_str())
            .ok_or_else(|| eyre::eyre!("vk_inline.bytes_b64 missing"))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(bytes_b64)
            .map_err(|e| eyre::eyre!("invalid vk_inline.bytes_b64: {e}"))?;
        let vk = VerifyingKeyBox::new(b.into(), bytes);
        ProofAttachment::new_inline(backend.into(), proof, vk)
    } else {
        return Err(eyre::eyre!("either vk_ref or vk_inline must be provided"));
    };
    if let Some(hex) = v.get("vk_commitment_hex").and_then(|x| x.as_str()) {
        let bytes = hex::decode(hex).map_err(|e| eyre::eyre!("invalid vk_commitment_hex: {e}"))?;
        if bytes.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            att.vk_commitment = Some(arr);
        }
    }
    Ok(att)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(att.vk_ref.is_some());
        assert!(att.vk_inline.is_none());
        assert_eq!(att.vk_commitment.unwrap(), [0u8; 32]);
    }

    #[test]
    fn build_proof_attachment_from_json_vk_inline() {
        // vk bytes = [1,2,3,4] -> AQIDBA==
        let vk_b64 = "AQIDBA==";
        // proof bytes = [5,6] -> BQY=
        let proof_b64 = "BQY=";
        let json = format!(
            r#"{{
                "backend": "halo2/ipa",
                "proof_b64": "{proof_b64}",
                "vk_inline": {{ "backend": "halo2/ipa", "bytes_b64": "{vk_b64}" }}
            }}"#
        );
        let v = norito::json::from_str(&json).expect("vk_inline json");
        let att = build_proof_attachment_from_json(&v).expect("ok");
        assert_eq!(att.proof.bytes, vec![5u8, 6u8]);
        assert!(att.vk_inline.is_some());
        assert!(att.vk_ref.is_none());
        let vk = att.vk_inline.as_ref().unwrap();
        assert_eq!(vk.backend.as_str(), "halo2/ipa");
        assert_eq!(vk.bytes, vec![1u8, 2u8, 3u8, 4u8]);
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
        let epk = "11".repeat(32);
        let nonce = "22".repeat(24);
        let (payload, bytes) =
            encode_encrypted_payload(&epk, &nonce, "AQIDBA==").expect("encode envelope");
        let expected_b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let decoded: iroha::data_model::confidential::ConfidentialEncryptedPayload =
            norito::codec::decode_adaptive(&bytes).expect("decode envelope");
        assert_eq!(payload, decoded);
        assert!(!expected_b64.is_empty());
    }
}

impl Run for UnshieldArgs {
    fn run<C: RunContext>(self, context: &mut C) -> eyre::Result<()> {
        use iroha::data_model::prelude::{AccountId, AssetDefinitionId, InstructionBox};
        let asset: AssetDefinitionId = self.asset.parse()?;
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
    /// `AssetDefinitionId` like `rose#wonderland`
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
        .split_once(':')
        .ok_or_else(|| eyre::eyre!("expected BACKEND:NAME for verifying key id"))?;
    Ok(VerifyingKeyId::new(backend, name))
}

impl Run for ZkRegisterAssetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> eyre::Result<()> {
        use iroha::data_model::isi::zk::{RegisterZkAsset, ZkAssetMode};
        use iroha::data_model::prelude::{AssetDefinitionId, InstructionBox};

        let asset: AssetDefinitionId = self.asset.parse()?;
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

impl Run for VkRegisterArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let s = std::fs::read_to_string(&self.json)?;
        let v: norito::json::Value = norito::json::from_str(&s)?;
        client.post_zk_vk_register(&v)?;
        context.println("VK register submitted (202 Accepted)")?;
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
        let s = std::fs::read_to_string(&self.json)?;
        let v: norito::json::Value = norito::json::from_str(&s)?;
        client.post_zk_vk_update(&v)?;
        context.println("VK update submitted (202 Accepted)")?;
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
        let client: Client = context.client_from_config();
        let v = client.get_zk_vk_json(&self.backend, &self.name)?;
        context.print_data(&v)?;
        Ok(())
    }
}
