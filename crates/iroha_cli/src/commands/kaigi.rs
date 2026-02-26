//! Kaigi instruction helpers.
//!
//! These subcommands build Kaigi ISIs and submit them through the CLI runtime.

use crate::cli_output::print_with_optional_text;
use crate::{Run, RunContext};
use clap::{Args, Subcommand, ValueEnum};
use eyre::{Result, WrapErr};
use iroha::data_model::{
    metadata::Metadata,
    prelude::{
        DomainId, KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiPrivacyMode,
        KaigiRelayHealthStatus, KaigiRelayManifest, KaigiRoomPolicy, NewKaigi,
    },
};
use iroha_crypto::Hash;
use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a new Kaigi session.
    Create(CreateArgs),
    /// Bootstrap a Kaigi session for demos and shareable testing metadata.
    Quickstart(QuickstartArgs),
    /// Join a Kaigi session.
    Join(JoinArgs),
    /// Leave a Kaigi session.
    Leave(LeaveArgs),
    /// End an active Kaigi session.
    End(EndArgs),
    /// Record usage statistics for a Kaigi session.
    RecordUsage(RecordUsageArgs),
    /// Report the health status of a relay used by a Kaigi session.
    ReportRelayHealth(ReportRelayHealthArgs),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Create(args) => args.run(context),
            Command::Quickstart(args) => args.run(context),
            Command::Join(args) => args.run(context),
            Command::Leave(args) => args.run(context),
            Command::End(args) => args.run(context),
            Command::RecordUsage(args) => args.run(context),
            Command::ReportRelayHealth(args) => args.run(context),
        }
    }
}

#[derive(Args, Debug)]
pub struct CreateArgs {
    /// Domain identifier hosting the call (e.g. `kaigi`).
    #[arg(long, value_name = "DOMAIN-ID")]
    pub domain: String,
    /// Call name within the domain (e.g. `daily-sync`).
    #[arg(long, value_name = "NAME")]
    pub call_name: String,
    /// Host account identifier responsible for the call (IH58 (preferred)/sora (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain).
    #[arg(long, value_name = "ACCOUNT-ID")]
    pub host: String,
    /// Optional human friendly title.
    #[arg(long)]
    pub title: Option<String>,
    /// Optional description for participants.
    #[arg(long)]
    pub description: Option<String>,
    /// Maximum concurrent participants (excluding host).
    #[arg(long, value_name = "U32")]
    pub max_participants: Option<u32>,
    /// Gas rate charged per minute (defaults to 0).
    #[arg(long, value_name = "U64", default_value_t = 0)]
    pub gas_rate_per_minute: u64,
    /// Optional billing account that will cover usage (IH58 (preferred)/sora (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain).
    #[arg(long, value_name = "ACCOUNT-ID")]
    pub billing_account: Option<String>,
    /// Optional scheduled start timestamp (milliseconds since epoch).
    #[arg(long, value_name = "U64")]
    pub scheduled_start_ms: Option<u64>,
    /// Privacy mode for the session (defaults to `transparent`).
    #[arg(long, value_enum, default_value_t = PrivacyModeArg::Transparent)]
    pub privacy_mode: PrivacyModeArg,
    /// Room access policy controlling viewer authentication.
    #[arg(long, value_enum, default_value_t = RoomPolicyArg::Authenticated)]
    pub room_policy: RoomPolicyArg,
    /// Path to a JSON file describing the relay manifest (optional).
    #[arg(long, value_name = "PATH")]
    pub relay_manifest: Option<String>,
    /// Path to a JSON file providing additional metadata (object with string keys).
    #[arg(long, value_name = "PATH")]
    pub metadata_json: Option<String>,
}

impl Run for CreateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let call_id = parse_call_id(&self.domain, &self.call_name)?;
        let host = crate::resolve_account_id(context, &self.host)
            .wrap_err("failed to resolve host account")?;
        let mut template = NewKaigi::with_defaults(call_id, host.clone());
        template.title = self.title;
        template.description = self.description;
        template.max_participants = self.max_participants;
        template.gas_rate_per_minute = self.gas_rate_per_minute;
        template.billing_account = match self.billing_account {
            Some(ref id) => Some(
                crate::resolve_account_id(context, id)
                    .wrap_err("failed to resolve billing account")?,
            ),
            None => None,
        };
        template.scheduled_start_ms = self.scheduled_start_ms;
        template.privacy_mode = self.privacy_mode.into();
        template.room_policy = self.room_policy.into();
        if let Some(path) = self.relay_manifest {
            let manifest = read_manifest(&path)?;
            template.relay_manifest = Some(manifest);
        }
        if let Some(path) = self.metadata_json {
            template.metadata = read_metadata(&path)?;
        }

        context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
            Box::new(iroha::data_model::isi::kaigi::CreateKaigi { call: template }),
        )])
    }
}

#[derive(Args, Debug)]
pub struct QuickstartArgs {
    /// Domain identifier hosting the call.
    #[arg(long, value_name = "DOMAIN-ID", default_value = "wonderland")]
    pub domain: String,
    /// Call name within the domain (defaults to a timestamp-based identifier).
    #[arg(long, value_name = "NAME")]
    pub call_name: Option<String>,
    /// Host account identifier responsible for the call (IH58 (preferred)/sora (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain).
    #[arg(long, value_name = "ACCOUNT-ID")]
    pub host: Option<String>,
    /// Privacy mode for the session (defaults to `transparent`).
    #[arg(long, value_enum, default_value_t = PrivacyModeArg::Transparent)]
    pub privacy_mode: PrivacyModeArg,
    /// Room access policy controlling viewer authentication.
    #[arg(long, value_enum, default_value_t = RoomPolicyArg::Authenticated)]
    pub room_policy: RoomPolicyArg,
    /// Path to a JSON file describing the relay manifest (optional).
    #[arg(long, value_name = "PATH")]
    pub relay_manifest: Option<String>,
    /// Path to a JSON file providing additional metadata (object with string keys).
    #[arg(long, value_name = "PATH")]
    pub metadata_json: Option<String>,
    /// Automatically join the host account immediately after creation.
    #[arg(long)]
    pub auto_join_host: bool,
    /// File path where the JSON summary should be written (defaults to stdout only).
    #[arg(long, value_name = "PATH")]
    pub summary_out: Option<PathBuf>,
    /// Root directory where `SoraNet` spool files are expected (informational only).
    #[arg(
        long,
        value_name = "PATH",
        default_value = "storage/streaming/soranet_routes"
    )]
    pub spool_hint: String,
}

impl Run for QuickstartArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let call_label = resolve_call_label(self.call_name)?;
        let call_id = parse_call_id(&self.domain, &call_label)?;
        let host = match self.host {
            Some(id) => crate::resolve_account_id(context, &id)
                .wrap_err("failed to resolve host account")?,
            None => context.config().account.clone(),
        };
        let mut template = NewKaigi::with_defaults(call_id.clone(), host.clone());
        template.privacy_mode = self.privacy_mode.into();
        template.room_policy = self.room_policy.into();
        if let Some(path) = self.relay_manifest {
            template.relay_manifest = Some(read_manifest(&path)?);
        }
        if let Some(path) = self.metadata_json {
            template.metadata = read_metadata(&path)?;
        }

        context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
            Box::new(iroha::data_model::isi::kaigi::CreateKaigi {
                call: template.clone(),
            }),
        )])?;

        if self.auto_join_host {
            context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
                Box::new(iroha::data_model::isi::kaigi::JoinKaigi {
                    call_id: call_id.clone(),
                    participant: host.clone(),
                    commitment: None,
                    nullifier: None,
                    roster_root: None,
                    proof: None,
                }),
            )])?;
        }

        let torii_url = context.config().torii_api_url.clone();
        let join_hint = format!(
            "iroha --config <path> kaigi join --domain {} --call-name {} --participant <account-id>",
            self.domain, call_label
        );
        let summary = QuickstartSummary {
            call_id: call_id.to_string(),
            domain: self.domain,
            call_name: call_label,
            host: host.to_string(),
            torii_url: torii_url.to_string(),
            room_policy: format!("{:?}", template.room_policy),
            privacy_mode: format!("{:?}", template.privacy_mode),
            join_hint,
            spool_hint: format!("{}/exit-<relay-id>/kaigi-stream/*.norito", self.spool_hint),
        };
        let summary_out = self.summary_out;
        if let Some(path) = summary_out.as_ref() {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)
                    .wrap_err("failed to create directory for summary output")?;
            }
            let mut payload = norito::json::to_json_pretty(&summary)
                .wrap_err("failed to render quickstart summary JSON")?;
            if !payload.ends_with('\n') {
                payload.push('\n');
            }
            fs::write(path, payload)
                .wrap_err_with(|| format!("failed to write summary to {}", path.display()))?;
        }

        let output = QuickstartOutput {
            summary: summary.clone(),
            summary_out: summary_out
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
        };
        let text = render_quickstart_text(&summary, summary_out.as_deref());
        print_with_optional_text(context, Some(text), &output)
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyModeArg {
    Transparent,
    #[clap(alias = "zk", alias = "zk_roster_v1")]
    ZkRosterV1,
}

impl From<PrivacyModeArg> for KaigiPrivacyMode {
    fn from(arg: PrivacyModeArg) -> Self {
        match arg {
            PrivacyModeArg::Transparent => KaigiPrivacyMode::Transparent,
            PrivacyModeArg::ZkRosterV1 => KaigiPrivacyMode::ZkRosterV1,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomPolicyArg {
    Public,
    #[clap(alias = "auth", alias = "authenticated")]
    Authenticated,
}

impl From<RoomPolicyArg> for KaigiRoomPolicy {
    fn from(arg: RoomPolicyArg) -> Self {
        match arg {
            RoomPolicyArg::Public => KaigiRoomPolicy::Public,
            RoomPolicyArg::Authenticated => KaigiRoomPolicy::Authenticated,
        }
    }
}

#[derive(Debug, Clone, norito::json::JsonSerialize)]
struct QuickstartSummary {
    call_id: String,
    domain: String,
    call_name: String,
    host: String,
    torii_url: String,
    room_policy: String,
    privacy_mode: String,
    join_hint: String,
    spool_hint: String,
}

#[derive(Debug, Clone, norito::json::JsonSerialize)]
struct QuickstartOutput {
    summary: QuickstartSummary,
    summary_out: Option<String>,
}

fn render_quickstart_text(summary: &QuickstartSummary, summary_out: Option<&Path>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Kaigi demo call created. Share the summary below:");
    let _ = writeln!(out, "call_id: {}", summary.call_id);
    let _ = writeln!(out, "domain: {}", summary.domain);
    let _ = writeln!(out, "call_name: {}", summary.call_name);
    let _ = writeln!(out, "host: {}", summary.host);
    let _ = writeln!(out, "torii_url: {}", summary.torii_url);
    let _ = writeln!(out, "room_policy: {}", summary.room_policy);
    let _ = writeln!(out, "privacy_mode: {}", summary.privacy_mode);
    let _ = writeln!(out, "join_hint: {}", summary.join_hint);
    let _ = writeln!(out, "spool_hint: {}", summary.spool_hint);
    if let Some(path) = summary_out {
        let _ = writeln!(out, "summary_out: {}", path.display());
    }
    out
}

fn resolve_call_label(value: Option<String>) -> Result<String> {
    if let Some(label) = value {
        return Ok(label);
    }
    let uptime = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock is before UNIX_EPOCH")?
        .as_secs();
    Ok(format!("kaigi_demo_{uptime:x}"))
}

#[derive(Args, Debug)]
pub struct JoinArgs {
    /// Domain identifier hosting the call.
    #[arg(long, value_name = "DOMAIN-ID")]
    pub domain: String,
    /// Call name within the domain.
    #[arg(long, value_name = "NAME")]
    pub call_name: String,
    /// Participant account joining the call (IH58 (preferred)/sora (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain).
    #[arg(long, value_name = "ACCOUNT-ID")]
    pub participant: String,
    /// Commitment hash (hex) for privacy mode joins.
    #[arg(long, value_name = "HEX")]
    pub commitment_hex: Option<String>,
    /// Alias tag describing the commitment (privacy mode).
    #[arg(long)]
    pub commitment_alias: Option<String>,
    /// Nullifier hash (hex) preventing duplicate joins (privacy mode).
    #[arg(long, value_name = "HEX")]
    pub nullifier_hex: Option<String>,
    /// Nullifier issuance timestamp (milliseconds since epoch).
    #[arg(long, value_name = "U64")]
    pub nullifier_issued_at_ms: Option<u64>,
    /// Roster Merkle root bound into the proof transcript (privacy mode).
    #[arg(long, value_name = "HEX")]
    pub roster_root_hex: Option<String>,
    /// Proof bytes attesting ownership (hex encoding of raw bytes).
    #[arg(long, value_name = "HEX")]
    pub proof_hex: Option<String>,
}

impl Run for JoinArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let call_id = parse_call_id(&self.domain, &self.call_name)?;
        let participant = crate::resolve_account_id(context, &self.participant)
            .wrap_err("failed to resolve participant account")?;
        let nullifier_issued_at_ms = self.nullifier_issued_at_ms;

        let commitment = match self.commitment_hex {
            Some(hex) => {
                let builder = KaigiCommitmentBuilder::new(&hex)?;
                Some(builder.with_alias(self.commitment_alias.clone()))
            }
            None => None,
        };

        let nullifier = self
            .nullifier_hex
            .as_ref()
            .map(|hex| build_nullifier(hex, nullifier_issued_at_ms))
            .transpose()?;

        let proof = self
            .proof_hex
            .map(|hex| decode_hex_vec(&hex).wrap_err("invalid proof hex"))
            .transpose()?;
        let roster_root = self
            .roster_root_hex
            .map(|hex| parse_hash(&hex).wrap_err("invalid roster root hex"))
            .transpose()?;

        context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
            Box::new(iroha::data_model::isi::kaigi::JoinKaigi {
                call_id,
                participant,
                commitment,
                nullifier,
                roster_root,
                proof,
            }),
        )])
    }
}

struct KaigiCommitmentBuilder {
    commitment: KaigiParticipantCommitment,
}

impl KaigiCommitmentBuilder {
    fn new(hex: &str) -> Result<Self> {
        let hash = parse_hash(hex)?;
        Ok(Self {
            commitment: KaigiParticipantCommitment {
                commitment: hash,
                alias_tag: None,
            },
        })
    }

    fn with_alias(mut self, alias: Option<String>) -> KaigiParticipantCommitment {
        self.commitment.alias_tag = alias;
        self.commitment
    }
}

fn build_nullifier(hex: &str, issued_at_ms: Option<u64>) -> Result<KaigiParticipantNullifier> {
    let digest = parse_hash(hex)?;
    let issued_at_ms = issued_at_ms.unwrap_or_default();
    Ok(KaigiParticipantNullifier {
        digest,
        issued_at_ms,
    })
}

#[derive(Args, Debug)]
pub struct LeaveArgs {
    /// Domain identifier hosting the call.
    #[arg(long, value_name = "DOMAIN-ID")]
    pub domain: String,
    /// Call name within the domain.
    #[arg(long, value_name = "NAME")]
    pub call_name: String,
    /// Participant account leaving the call (IH58 (preferred)/sora (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain).
    #[arg(long, value_name = "ACCOUNT-ID")]
    pub participant: String,
    /// Commitment hash (hex) identifying the participant in privacy mode.
    #[arg(long, value_name = "HEX")]
    pub commitment_hex: Option<String>,
    /// Nullifier hash (hex) preventing duplicate leaves (privacy mode).
    #[arg(long, value_name = "HEX")]
    pub nullifier_hex: Option<String>,
    /// Nullifier issuance timestamp (milliseconds since epoch).
    #[arg(long, value_name = "U64")]
    pub nullifier_issued_at_ms: Option<u64>,
    /// Roster Merkle root bound into the proof transcript (privacy mode).
    #[arg(long, value_name = "HEX")]
    pub roster_root_hex: Option<String>,
    /// Proof bytes attesting ownership (hex encoding of raw bytes).
    #[arg(long, value_name = "HEX")]
    pub proof_hex: Option<String>,
}

impl Run for LeaveArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let call_id = parse_call_id(&self.domain, &self.call_name)?;
        let participant = crate::resolve_account_id(context, &self.participant)
            .wrap_err("failed to resolve participant account")?;
        let nullifier_issued_at_ms = self.nullifier_issued_at_ms;

        let commitment = self
            .commitment_hex
            .map(|hex| {
                parse_hash(&hex).map(|hash| KaigiParticipantCommitment {
                    commitment: hash,
                    alias_tag: None,
                })
            })
            .transpose()?;

        let nullifier = self
            .nullifier_hex
            .as_ref()
            .map(|hex| build_nullifier(hex, nullifier_issued_at_ms))
            .transpose()?;

        let proof = self
            .proof_hex
            .map(|hex| decode_hex_vec(&hex).wrap_err("invalid proof hex"))
            .transpose()?;
        let roster_root = self
            .roster_root_hex
            .map(|hex| parse_hash(&hex).wrap_err("invalid roster root hex"))
            .transpose()?;

        context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
            Box::new(iroha::data_model::isi::kaigi::LeaveKaigi {
                call_id,
                participant,
                commitment,
                nullifier,
                roster_root,
                proof,
            }),
        )])
    }
}

#[derive(Args, Debug)]
pub struct EndArgs {
    /// Domain identifier hosting the call.
    #[arg(long, value_name = "DOMAIN-ID")]
    pub domain: String,
    /// Call name within the domain.
    #[arg(long, value_name = "NAME")]
    pub call_name: String,
    /// Optional timestamp in milliseconds when the call ended.
    #[arg(long, value_name = "U64")]
    pub ended_at_ms: Option<u64>,
}

impl Run for EndArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let call_id = parse_call_id(&self.domain, &self.call_name)?;
        context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
            Box::new(iroha::data_model::isi::kaigi::EndKaigi {
                call_id,
                ended_at_ms: self.ended_at_ms,
            }),
        )])
    }
}

#[derive(Args, Debug)]
pub struct RecordUsageArgs {
    /// Domain identifier hosting the call.
    #[arg(long, value_name = "DOMAIN-ID")]
    pub domain: String,
    /// Call name within the domain.
    #[arg(long, value_name = "NAME")]
    pub call_name: String,
    /// Duration in milliseconds for this usage segment.
    #[arg(long, value_name = "U64")]
    pub duration_ms: u64,
    /// Gas billed for this segment.
    #[arg(long, value_name = "U64", default_value_t = 0)]
    pub billed_gas: u64,
    /// Optional usage commitment hash (privacy mode).
    #[arg(long, value_name = "HEX")]
    pub usage_commitment_hex: Option<String>,
    /// Optional proof bytes attesting the usage delta (privacy mode).
    #[arg(long, value_name = "HEX")]
    pub proof_hex: Option<String>,
}

impl Run for RecordUsageArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let call_id = parse_call_id(&self.domain, &self.call_name)?;
        let usage_commitment = self
            .usage_commitment_hex
            .map(|hex| parse_hash(&hex).wrap_err("invalid usage commitment hex"))
            .transpose()?;
        let proof = self
            .proof_hex
            .map(|hex| decode_hex_vec(&hex).wrap_err("invalid proof hex"))
            .transpose()?;
        context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
            Box::new(iroha::data_model::isi::kaigi::RecordKaigiUsage {
                call_id,
                duration_ms: self.duration_ms,
                billed_gas: self.billed_gas,
                usage_commitment,
                proof,
            }),
        )])
    }
}

#[derive(Args, Debug)]
pub struct ReportRelayHealthArgs {
    /// Domain identifier hosting the call.
    #[arg(long, value_name = "DOMAIN-ID")]
    pub domain: String,
    /// Call name within the domain.
    #[arg(long, value_name = "NAME")]
    pub call_name: String,
    /// Relay account identifier being reported (IH58 (preferred)/sora (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain).
    #[arg(long, value_name = "ACCOUNT-ID")]
    pub relay: String,
    /// Observed health status for the relay.
    #[arg(long, value_enum)]
    pub status: RelayHealthStatusArg,
    /// Timestamp in milliseconds when the status was observed.
    #[arg(long, value_name = "U64")]
    pub reported_at_ms: u64,
    /// Optional notes capturing failure or recovery context.
    #[arg(long)]
    pub notes: Option<String>,
}

impl Run for ReportRelayHealthArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let call_id = parse_call_id(&self.domain, &self.call_name)?;
        let relay_id = crate::resolve_account_id(context, &self.relay)
            .wrap_err("failed to resolve relay account")?;
        let status: KaigiRelayHealthStatus = self.status.into();

        context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
            Box::new(iroha::data_model::isi::kaigi::ReportKaigiRelayHealth {
                call_id,
                relay_id,
                status,
                reported_at_ms: self.reported_at_ms,
                notes: self.notes.clone(),
            }),
        )])
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayHealthStatusArg {
    Healthy,
    Degraded,
    Unavailable,
}

impl From<RelayHealthStatusArg> for KaigiRelayHealthStatus {
    fn from(arg: RelayHealthStatusArg) -> Self {
        match arg {
            RelayHealthStatusArg::Healthy => KaigiRelayHealthStatus::Healthy,
            RelayHealthStatusArg::Degraded => KaigiRelayHealthStatus::Degraded,
            RelayHealthStatusArg::Unavailable => KaigiRelayHealthStatus::Unavailable,
        }
    }
}

fn parse_call_id(domain: &str, call_name: &str) -> Result<KaigiId> {
    let domain_id = DomainId::from_str(domain).wrap_err("invalid domain id")?;
    let call = iroha::data_model::name::Name::from_str(call_name).wrap_err("invalid call name")?;
    Ok(KaigiId::new(domain_id, call))
}

fn parse_hash(hex: &str) -> Result<Hash> {
    let trimmed = hex.trim_start_matches("0x");
    Hash::from_str(trimmed).wrap_err("invalid hash literal")
}

fn decode_hex_vec(hex: &str) -> Result<Vec<u8>> {
    let trimmed = hex.trim_start_matches("0x");
    hex::decode(trimmed).wrap_err("invalid hex encoding")
}

fn read_manifest(path: &str) -> Result<KaigiRelayManifest> {
    let contents = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read relay manifest from `{path}`"))?;
    norito::json::from_str(&contents).wrap_err("invalid relay manifest JSON")
}

fn read_metadata(path: &str) -> Result<Metadata> {
    let contents = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read metadata JSON from `{path}`"))?;
    let value: norito::json::Value =
        norito::json::from_str(&contents).wrap_err("invalid metadata JSON")?;
    let obj = value
        .as_object()
        .ok_or_else(|| eyre::eyre!("metadata JSON must be an object"))?;
    let mut metadata = Metadata::default();
    for (key, value) in obj {
        let name = iroha::data_model::name::Name::from_str(key)
            .wrap_err_with(|| format!("invalid metadata key `{key}`"))?;
        metadata.insert(name, value.clone());
    }
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::path::Path;

    #[derive(Parser, Debug)]
    struct TestCli {
        #[command(subcommand)]
        command: Command,
    }

    fn parse_command(args: &[&str]) -> Command {
        let mut cli_argv = vec!["test"];
        cli_argv.extend_from_slice(args);
        TestCli::parse_from(cli_argv).command
    }

    #[test]
    fn clap_parses_create_with_privacy_alias() {
        match parse_command(&[
            "create",
            "--domain",
            "kaigi",
            "--call-name",
            "daily",
            "--host",
            "alice@kaigi",
            "--privacy-mode",
            "zk",
            "--gas-rate-per-minute",
            "42",
        ]) {
            Command::Create(args) => {
                assert_eq!(args.call_name, "daily");
                assert_eq!(args.domain, "kaigi");
                assert_eq!(args.privacy_mode, PrivacyModeArg::ZkRosterV1);
                assert_eq!(args.gas_rate_per_minute, 42);
            }
            other => panic!("expected create command, got {other:?}"),
        }
    }

    #[test]
    fn clap_parses_join_with_optional_fields() {
        match parse_command(&[
            "join",
            "--domain",
            "kaigi",
            "--call-name",
            "daily",
            "--participant",
            "bob@kaigi",
            "--commitment-hex",
            "0xdeadbeef",
            "--commitment-alias",
            "bob",
            "--nullifier-hex",
            "cafebabe",
            "--nullifier-issued-at-ms",
            "123",
            "--roster-root-hex",
            "feedface",
            "--proof-hex",
            "aa55",
        ]) {
            Command::Join(args) => {
                assert_eq!(args.domain, "kaigi");
                assert_eq!(args.call_name, "daily");
                assert_eq!(args.participant, "bob@kaigi");
                assert_eq!(args.commitment_hex.as_deref(), Some("0xdeadbeef"));
                assert_eq!(args.commitment_alias.as_deref(), Some("bob"));
                assert_eq!(args.nullifier_hex.as_deref(), Some("cafebabe"));
                assert_eq!(args.nullifier_issued_at_ms, Some(123));
                assert_eq!(args.roster_root_hex.as_deref(), Some("feedface"));
                assert_eq!(args.proof_hex.as_deref(), Some("aa55"));
            }
            other => panic!("expected create command, got {other:?}"),
        }
    }

    #[test]
    fn clap_parses_leave_without_optional_fields() {
        match parse_command(&[
            "leave",
            "--domain",
            "kaigi",
            "--call-name",
            "daily",
            "--participant",
            "bob@kaigi",
        ]) {
            Command::Leave(args) => {
                assert_eq!(args.domain, "kaigi");
                assert_eq!(args.call_name, "daily");
                assert_eq!(args.participant, "bob@kaigi");
                assert!(args.commitment_hex.is_none());
                assert!(args.nullifier_hex.is_none());
                assert!(args.proof_hex.is_none());
                assert!(args.roster_root_hex.is_none());
            }
            other => panic!("expected leave command, got {other:?}"),
        }
    }

    #[test]
    fn clap_parses_end_with_timestamp() {
        match parse_command(&[
            "end",
            "--domain",
            "kaigi",
            "--call-name",
            "daily",
            "--ended-at-ms",
            "456",
        ]) {
            Command::End(args) => {
                assert_eq!(args.ended_at_ms, Some(456));
            }
            other => panic!("expected end command, got {other:?}"),
        }
    }

    #[test]
    fn clap_parses_record_usage_defaults() {
        match parse_command(&[
            "record-usage",
            "--domain",
            "kaigi",
            "--call-name",
            "daily",
            "--duration-ms",
            "789",
        ]) {
            Command::RecordUsage(args) => {
                assert_eq!(args.duration_ms, 789);
                assert_eq!(args.billed_gas, 0);
                assert_eq!(args.domain, "kaigi");
            }
            other => panic!("expected record-usage command, got {other:?}"),
        }
    }

    #[test]
    fn clap_parses_quickstart_defaults() {
        match parse_command(&["quickstart"]) {
            Command::Quickstart(args) => {
                assert_eq!(args.domain, "wonderland");
                assert!(args.call_name.is_none());
                assert!(args.host.is_none());
                assert!(!args.auto_join_host);
                assert_eq!(args.spool_hint, "storage/streaming/soranet_routes");
            }
            other => panic!("expected quickstart command, got {other:?}"),
        }
    }

    #[test]
    fn clap_parses_record_usage_with_privacy_fields() {
        match parse_command(&[
            "record-usage",
            "--domain",
            "kaigi",
            "--call-name",
            "daily",
            "--duration-ms",
            "120",
            "--billed-gas",
            "450",
            "--usage-commitment-hex",
            "b16b00b5",
            "--proof-hex",
            "ff01",
        ]) {
            Command::RecordUsage(args) => {
                assert_eq!(args.usage_commitment_hex.as_deref(), Some("b16b00b5"));
                assert_eq!(args.proof_hex.as_deref(), Some("ff01"));
            }
            other => panic!("expected record-usage command, got {other:?}"),
        }
    }

    #[test]
    fn build_nullifier_defaults_to_zero_when_timestamp_missing() {
        let hex = "ab".repeat(32);
        let payload = format!("0x{hex}");
        let nullifier = build_nullifier(&payload, None).expect("valid nullifier");
        assert_eq!(nullifier.issued_at_ms, 0);
    }

    #[test]
    fn read_metadata_rejects_non_object_json() {
        use std::fs;

        let mut path = std::env::temp_dir();
        path.push(format!(
            "kaigi_metadata_invalid_{}.json",
            std::process::id()
        ));
        fs::write(&path, "\"not-an-object\"").expect("write temp metadata");

        let result = read_metadata(path.to_str().expect("path to str"));

        fs::remove_file(&path).expect("cleanup temp metadata");
        assert!(
            result.is_err(),
            "metadata reader should reject non-object JSON"
        );
    }

    #[test]
    fn render_quickstart_text_includes_summary_out() {
        let summary = QuickstartSummary {
            call_id: "call-1".to_string(),
            domain: "kaigi".to_string(),
            call_name: "daily".to_string(),
            host: "alice@kaigi".to_string(),
            torii_url: "http://localhost:8080".to_string(),
            room_policy: "Public".to_string(),
            privacy_mode: "Transparent".to_string(),
            join_hint: "iroha kaigi join ...".to_string(),
            spool_hint: "/tmp/spool".to_string(),
        };
        let text = render_quickstart_text(&summary, Some(Path::new("/tmp/summary.json")));
        assert!(text.contains("call_id: call-1"));
        assert!(text.contains("summary_out: /tmp/summary.json"));
    }
}
