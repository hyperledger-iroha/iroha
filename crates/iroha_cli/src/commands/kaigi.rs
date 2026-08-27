//! Kaigi instruction helpers.
//!
//! These subcommands build Kaigi ISIs and submit them through the CLI runtime.
use crate::cli_output::print_with_optional_text;
use crate::{Run, RunContext};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use clap::{Args, Subcommand, ValueEnum};
use eyre::{Result, WrapErr};
use iroha::data_model::{
    kaigi::{
        KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1, KAIGI_RELAY_MANIFEST_MAX_HOPS_V1,
        KAIGI_RELAY_MANIFEST_MIN_HOPS_V1,
    },
    metadata::Metadata,
    prelude::{
        AccountId, DomainId, KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier,
        KaigiPrivacyMode, KaigiRelayHealthStatus, KaigiRelayManifest, KaigiRelayRegistration,
        KaigiRoomPolicy, NewKaigi,
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
    /// Register or update a Kaigi relay descriptor.
    RegisterRelay(RegisterRelayArgs),
    /// Retire a Kaigi relay descriptor and its retained health feedback.
    UnregisterRelay(UnregisterRelayArgs),
    /// Replace or clear the relay manifest for an existing Kaigi session.
    SetRelayManifest(SetRelayManifestArgs),
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
            Command::RegisterRelay(args) => args.run(context),
            Command::UnregisterRelay(args) => args.run(context),
            Command::SetRelayManifest(args) => args.run(context),
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
    /// Domain identifier hosting the call (e.g. `kaigi.universal`).
    #[arg(long, value_name = "DOMAIN-ID")]
    pub domain: String,
    /// Call name within the domain (e.g. `daily-sync`).
    #[arg(long, value_name = "NAME")]
    pub call_name: String,
    /// Host account identifier responsible for the call (canonical I105 account literal).
    #[arg(long, value_name = "ACCOUNT-ID")]
    pub host: String,
    /// Optional human friendly title.
    #[arg(long)]
    pub title: Option<String>,
    /// Optional description for participants.
    #[arg(long)]
    pub description: Option<String>,
    /// Maximum concurrent participants (excluding host); zero is invalid.
    #[arg(long, value_name = "U32")]
    pub max_participants: Option<u32>,
    /// Gas rate charged per minute (defaults to 0).
    #[arg(long, value_name = "U64", default_value_t = 0)]
    pub gas_rate_per_minute: u64,
    /// Optional host billing account that will cover usage (canonical I105 account literal).
    /// Third-party delegated billing is not supported in the first release.
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
    /// Commitment hash (hex) for privacy mode creation.
    #[arg(long, value_name = "HEX")]
    pub commitment_hex: Option<String>,
    /// Reserved on-chain alias tag; must be omitted to avoid ledger disclosure.
    #[arg(long)]
    pub commitment_alias: Option<String>,
    /// Nullifier hash (hex) preventing proof replay (privacy mode).
    #[arg(long, value_name = "HEX")]
    pub nullifier_hex: Option<String>,
    /// Reserved on-chain timing field; must be omitted or zero.
    #[arg(long, value_name = "U64")]
    pub nullifier_issued_at_ms: Option<u64>,
    /// Roster Merkle root bound into the proof transcript (privacy mode).
    #[arg(long, value_name = "HEX")]
    pub roster_root_hex: Option<String>,
    /// Proof bytes attesting ownership (hex encoding of raw bytes).
    #[arg(long, value_name = "HEX")]
    pub proof_hex: Option<String>,
}
impl Run for CreateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        validate_max_participants(self.max_participants)?;
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
        validate_billing_account(&host, template.billing_account.as_ref())?;
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
        let privacy = parse_optional_privacy_artifacts(
            self.commitment_hex.as_deref(),
            self.commitment_alias.as_deref(),
            self.nullifier_hex.as_deref(),
            self.nullifier_issued_at_ms,
            self.roster_root_hex.as_deref(),
            self.proof_hex.as_deref(),
        )?;
        validate_create_privacy_artifacts(template.privacy_mode, &privacy)?;
        context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
            Box::new(iroha::data_model::isi::kaigi::CreateKaigi {
                call: template,
                commitment: privacy.commitment,
                nullifier: privacy.nullifier,
                roster_root: privacy.roster_root,
                proof: privacy.proof,
            }),
        )])
    }
}
#[derive(Args, Debug)]
pub struct QuickstartArgs {
    /// Domain identifier hosting the call.
    #[arg(long, value_name = "DOMAIN-ID", default_value = "wonderland.universal")]
    pub domain: String,
    /// Call name within the domain (defaults to a timestamp-based identifier).
    #[arg(long, value_name = "NAME")]
    pub call_name: Option<String>,
    /// Host account identifier responsible for the call (canonical I105 account literal).
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
    /// File path where the JSON summary should be written (defaults to stdout only).
    #[arg(long, value_name = "PATH")]
    pub summary_out: Option<PathBuf>,
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
        context.finish(quickstart_instructions(template.clone()))?;
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
fn quickstart_instructions(template: NewKaigi) -> [iroha::data_model::isi::InstructionBox; 1] {
    [iroha::data_model::isi::Instruction::into_instruction_box(
        Box::new(iroha::data_model::isi::kaigi::CreateKaigi {
            call: template,
            commitment: None,
            nullifier: None,
            roster_root: None,
            proof: None,
        }),
    )]
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
    if let Some(path) = summary_out {
        let _ = writeln!(out, "summary_out: {}", path.display());
    }
    out
}
fn resolve_call_label(value: Option<String>) -> Result<String> {
    if let Some(label) = value {
        return Ok(label);
    }
    generated_call_label(SystemTime::now(), std::process::id())
}
fn generated_call_label(now: SystemTime, process_id: u32) -> Result<String> {
    let unix_nanos = now
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock is before UNIX_EPOCH")?
        .as_nanos();
    Ok(format!("kaigi_demo_{unix_nanos:x}_{process_id:x}"))
}
#[derive(Args, Debug)]
pub struct RegisterRelayArgs {
    /// Relay account identifier advertising relay capabilities (canonical I105 account literal).
    /// The account must have a live domain-qualified primary alias, which selects
    /// the governance domain where the descriptor is stored.
    #[arg(long, value_name = "ACCOUNT-ID")]
    pub relay: String,
    /// HPKE public key bytes advertised by the relay (base64-encoded raw bytes).
    #[arg(long, value_name = "BASE64")]
    pub hpke_public_key_b64: String,
    /// Relative bandwidth class advertised by the relay.
    #[arg(long, value_name = "U8")]
    pub bandwidth_class: u8,
}
impl Run for RegisterRelayArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if self.bandwidth_class == 0 {
            eyre::bail!("relay bandwidth class must be non-zero");
        }
        let relay_id = crate::resolve_account_id(context, &self.relay)
            .wrap_err("failed to resolve relay account")?;
        let hpke_public_key = BASE64_STANDARD
            .decode(self.hpke_public_key_b64.trim())
            .wrap_err("invalid relay HPKE public key base64")?;
        validate_relay_hpke_public_key(&hpke_public_key)?;
        let relay = KaigiRelayRegistration {
            relay_id,
            hpke_public_key,
            bandwidth_class: self.bandwidth_class,
        };
        context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
            Box::new(iroha::data_model::isi::kaigi::RegisterKaigiRelay { relay }),
        )])
    }
}
#[derive(Args, Debug)]
pub struct UnregisterRelayArgs {
    /// Relay account identifier whose descriptor should be retired.
    #[arg(long, value_name = "ACCOUNT-ID")]
    pub relay: String,
}
impl Run for UnregisterRelayArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let relay_id = crate::resolve_account_id(context, &self.relay)
            .wrap_err("failed to resolve relay account")?;
        context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
            Box::new(iroha::data_model::isi::kaigi::UnregisterKaigiRelay { relay_id }),
        )])
    }
}
#[derive(Args, Debug)]
pub struct SetRelayManifestArgs {
    /// Domain identifier hosting the call.
    #[arg(long, value_name = "DOMAIN-ID")]
    pub domain: String,
    /// Call name within the domain.
    #[arg(long, value_name = "NAME")]
    pub call_name: String,
    /// Path to a JSON file describing the relay manifest.
    #[arg(
        long,
        value_name = "PATH",
        conflicts_with = "clear",
        required_unless_present = "clear"
    )]
    pub relay_manifest: Option<String>,
    /// Clear the stored relay manifest entirely.
    #[arg(long, conflicts_with = "relay_manifest")]
    pub clear: bool,
}
impl Run for SetRelayManifestArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let call_id = parse_call_id(&self.domain, &self.call_name)?;
        let relay_manifest = match (self.clear, self.relay_manifest) {
            (true, None) => None,
            (false, Some(path)) => Some(read_manifest(&path)?),
            _ => eyre::bail!("provide either --relay-manifest <PATH> or --clear"),
        };
        context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
            Box::new(iroha::data_model::isi::kaigi::SetKaigiRelayManifest {
                call_id,
                relay_manifest,
            }),
        )])
    }
}
#[derive(Args, Debug)]
pub struct JoinArgs {
    /// Domain identifier hosting the call.
    #[arg(long, value_name = "DOMAIN-ID")]
    pub domain: String,
    /// Call name within the domain.
    #[arg(long, value_name = "NAME")]
    pub call_name: String,
    /// Participant account joining the call (canonical I105 account literal).
    #[arg(long, value_name = "ACCOUNT-ID")]
    pub participant: String,
    /// Commitment hash (hex) for privacy mode joins.
    #[arg(long, value_name = "HEX")]
    pub commitment_hex: Option<String>,
    /// Reserved on-chain alias tag; must be omitted to avoid ledger disclosure.
    #[arg(long)]
    pub commitment_alias: Option<String>,
    /// Nullifier hash (hex) preventing duplicate joins (privacy mode).
    #[arg(long, value_name = "HEX")]
    pub nullifier_hex: Option<String>,
    /// Reserved on-chain timing field; must be omitted or zero.
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
        let privacy = parse_optional_privacy_artifacts(
            self.commitment_hex.as_deref(),
            self.commitment_alias.as_deref(),
            self.nullifier_hex.as_deref(),
            self.nullifier_issued_at_ms,
            self.roster_root_hex.as_deref(),
            self.proof_hex.as_deref(),
        )?;
        context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
            Box::new(iroha::data_model::isi::kaigi::JoinKaigi {
                call_id,
                participant,
                commitment: privacy.commitment,
                nullifier: privacy.nullifier,
                roster_root: privacy.roster_root,
                proof: privacy.proof,
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
}
#[derive(Debug)]
struct ParsedKaigiPrivacyArtifacts {
    commitment: Option<KaigiParticipantCommitment>,
    nullifier: Option<KaigiParticipantNullifier>,
    roster_root: Option<Hash>,
    proof: Option<Vec<u8>>,
}
fn parse_optional_privacy_artifacts(
    commitment_hex: Option<&str>,
    commitment_alias: Option<&str>,
    nullifier_hex: Option<&str>,
    nullifier_issued_at_ms: Option<u64>,
    roster_root_hex: Option<&str>,
    proof_hex: Option<&str>,
) -> Result<ParsedKaigiPrivacyArtifacts> {
    if commitment_alias.is_some() {
        eyre::bail!(
            "commitment aliases are off-chain only and must be omitted from Kaigi privacy instructions"
        );
    }
    if nullifier_issued_at_ms.is_some_and(|issued_at_ms| issued_at_ms != 0) {
        eyre::bail!("nullifier issuance timestamps are off-chain only and must be omitted or zero");
    }
    let commitment = commitment_hex
        .map(KaigiCommitmentBuilder::new)
        .transpose()?
        .map(|builder| builder.commitment);
    let nullifier = nullifier_hex
        .map(|hex| build_nullifier(hex, nullifier_issued_at_ms))
        .transpose()?;
    let roster_root = roster_root_hex
        .map(|hex| parse_hash(hex).wrap_err("invalid roster root hex"))
        .transpose()?;
    let proof = proof_hex
        .map(|hex| decode_hex_vec(hex).wrap_err("invalid proof hex"))
        .transpose()?;
    if proof.as_ref().is_some_and(|proof| proof.is_empty()) {
        eyre::bail!("privacy proof payload must be non-empty");
    }
    let supplied_artifact_count = [
        commitment.is_some(),
        nullifier.is_some(),
        roster_root.is_some(),
        proof.is_some(),
    ]
    .into_iter()
    .filter(|supplied| *supplied)
    .count();
    if supplied_artifact_count != 0 && supplied_artifact_count != 4 {
        eyre::bail!(
            "Kaigi privacy artifacts must either all be omitted or include commitment, nullifier, roster root, and proof"
        );
    }
    Ok(ParsedKaigiPrivacyArtifacts {
        commitment,
        nullifier,
        roster_root,
        proof,
    })
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
    /// Participant account leaving the call (canonical I105 account literal).
    #[arg(long, value_name = "ACCOUNT-ID")]
    pub participant: String,
    /// Reserved privacy-leave commitment; must be omitted because privacy-mode leave is off-chain.
    #[arg(long, value_name = "HEX")]
    pub commitment_hex: Option<String>,
    /// Reserved privacy-leave nullifier; must be omitted because privacy-mode leave is off-chain.
    #[arg(long, value_name = "HEX")]
    pub nullifier_hex: Option<String>,
    /// Reserved privacy-leave timing field; must be omitted.
    #[arg(long, value_name = "U64")]
    pub nullifier_issued_at_ms: Option<u64>,
    /// Reserved privacy-leave roster root; must be omitted.
    #[arg(long, value_name = "HEX")]
    pub roster_root_hex: Option<String>,
    /// Reserved privacy-leave proof; must be omitted.
    #[arg(long, value_name = "HEX")]
    pub proof_hex: Option<String>,
}
impl Run for LeaveArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        validate_leave_privacy_arguments(
            self.commitment_hex.as_deref(),
            self.nullifier_hex.as_deref(),
            self.nullifier_issued_at_ms,
            self.roster_root_hex.as_deref(),
            self.proof_hex.as_deref(),
        )?;
        let call_id = parse_call_id(&self.domain, &self.call_name)?;
        let participant = crate::resolve_account_id(context, &self.participant)
            .wrap_err("failed to resolve participant account")?;
        context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
            Box::new(iroha::data_model::isi::kaigi::LeaveKaigi {
                call_id,
                participant,
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
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
    /// Optional end timestamp between call creation and the current block time.
    #[arg(long, value_name = "U64")]
    pub ended_at_ms: Option<u64>,
    /// Commitment hash (hex) for privacy mode end requests.
    #[arg(long, value_name = "HEX")]
    pub commitment_hex: Option<String>,
    /// Reserved on-chain alias tag; must be omitted to avoid ledger disclosure.
    #[arg(long)]
    pub commitment_alias: Option<String>,
    /// Nullifier hash (hex) preventing proof replay (privacy mode).
    #[arg(long, value_name = "HEX")]
    pub nullifier_hex: Option<String>,
    /// Reserved on-chain timing field; must be omitted or zero.
    #[arg(long, value_name = "U64")]
    pub nullifier_issued_at_ms: Option<u64>,
    /// Roster Merkle root bound into the proof transcript (privacy mode).
    #[arg(long, value_name = "HEX")]
    pub roster_root_hex: Option<String>,
    /// Proof bytes attesting ownership (hex encoding of raw bytes).
    #[arg(long, value_name = "HEX")]
    pub proof_hex: Option<String>,
}
impl Run for EndArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let call_id = parse_call_id(&self.domain, &self.call_name)?;
        let privacy = parse_optional_privacy_artifacts(
            self.commitment_hex.as_deref(),
            self.commitment_alias.as_deref(),
            self.nullifier_hex.as_deref(),
            self.nullifier_issued_at_ms,
            self.roster_root_hex.as_deref(),
            self.proof_hex.as_deref(),
        )?;
        context.finish([iroha::data_model::isi::Instruction::into_instruction_box(
            Box::new(iroha::data_model::isi::kaigi::EndKaigi {
                call_id,
                ended_at_ms: self.ended_at_ms,
                commitment: privacy.commitment,
                nullifier: privacy.nullifier,
                roster_root: privacy.roster_root,
                proof: privacy.proof,
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
        validate_usage_duration(self.duration_ms)?;
        let call_id = parse_call_id(&self.domain, &self.call_name)?;
        let usage_commitment = self
            .usage_commitment_hex
            .map(|hex| parse_hash(&hex).wrap_err("invalid usage commitment hex"))
            .transpose()?;
        let proof = self
            .proof_hex
            .map(|hex| decode_hex_vec(&hex).wrap_err("invalid proof hex"))
            .transpose()?;
        validate_usage_privacy_artifacts(usage_commitment.as_ref(), proof.as_deref())?;
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
    /// Relay account identifier being reported (canonical I105 account literal).
    #[arg(long, value_name = "ACCOUNT-ID")]
    pub relay: String,
    /// Observed health status for the relay.
    #[arg(long, value_enum)]
    pub status: RelayHealthStatusArg,
    /// Observation timestamp in milliseconds, no later than the current block time.
    #[arg(long, value_name = "U64")]
    pub reported_at_ms: u64,
    /// Optional notes capturing failure or recovery context.
    #[arg(long)]
    pub notes: Option<String>,
}
impl Run for ReportRelayHealthArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        validate_relay_health_notes(self.notes.as_deref())?;
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
    let domain_id = DomainId::parse_fully_qualified(domain).wrap_err("invalid domain id")?;
    let call = iroha::data_model::name::Name::from_str(call_name).wrap_err("invalid call name")?;
    Ok(KaigiId::new(domain_id, call))
}
fn validate_max_participants(max_participants: Option<u32>) -> Result<()> {
    if max_participants == Some(0) {
        eyre::bail!("Kaigi max participants must be greater than zero when provided");
    }
    Ok(())
}
fn validate_relay_hpke_public_key(hpke_public_key: &[u8]) -> Result<()> {
    if hpke_public_key.is_empty() {
        eyre::bail!("relay HPKE public key must be non-empty");
    }
    if hpke_public_key.len() > KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1 {
        eyre::bail!(
            "relay HPKE public key must not exceed {KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1} bytes"
        );
    }
    Ok(())
}
fn validate_relay_manifest_limits(manifest: &KaigiRelayManifest) -> Result<()> {
    if manifest.hops.len() < KAIGI_RELAY_MANIFEST_MIN_HOPS_V1 {
        eyre::bail!("relay manifest must include at least {KAIGI_RELAY_MANIFEST_MIN_HOPS_V1} hops");
    }
    if manifest.hops.len() > KAIGI_RELAY_MANIFEST_MAX_HOPS_V1 {
        eyre::bail!(
            "relay manifest must not include more than {KAIGI_RELAY_MANIFEST_MAX_HOPS_V1} hops"
        );
    }
    for hop in &manifest.hops {
        validate_relay_hpke_public_key(&hop.hpke_public_key)?;
    }
    Ok(())
}
fn validate_relay_health_notes(notes: Option<&str>) -> Result<()> {
    if notes.is_some_and(|notes| notes.chars().count() > 512) {
        eyre::bail!("relay health notes must not exceed 512 characters");
    }
    Ok(())
}
fn validate_usage_duration(duration_ms: u64) -> Result<()> {
    if duration_ms == 0 {
        eyre::bail!("usage duration must be positive");
    }
    Ok(())
}
fn validate_billing_account(host: &AccountId, billing_account: Option<&AccountId>) -> Result<()> {
    if billing_account.is_some_and(|billing_account| billing_account != host) {
        eyre::bail!(
            "Kaigi billing account must resolve to the host until delegated billing is supported"
        );
    }
    Ok(())
}
fn validate_create_privacy_artifacts(
    privacy_mode: KaigiPrivacyMode,
    artifacts: &ParsedKaigiPrivacyArtifacts,
) -> Result<()> {
    if privacy_mode == KaigiPrivacyMode::Transparent
        && (artifacts.commitment.is_some()
            || artifacts.nullifier.is_some()
            || artifacts.roster_root.is_some()
            || artifacts.proof.is_some())
    {
        eyre::bail!("transparent Kaigi sessions must not include privacy artifacts");
    }
    Ok(())
}
fn validate_leave_privacy_arguments(
    commitment_hex: Option<&str>,
    nullifier_hex: Option<&str>,
    nullifier_issued_at_ms: Option<u64>,
    roster_root_hex: Option<&str>,
    proof_hex: Option<&str>,
) -> Result<()> {
    if commitment_hex.is_some()
        || nullifier_hex.is_some()
        || nullifier_issued_at_ms.is_some()
        || roster_root_hex.is_some()
        || proof_hex.is_some()
    {
        eyre::bail!(
            "Kaigi leave does not accept privacy artifacts; privacy-mode leave is off-chain only"
        );
    }
    Ok(())
}
fn validate_usage_privacy_artifacts(
    usage_commitment: Option<&Hash>,
    proof: Option<&[u8]>,
) -> Result<()> {
    if proof.is_some_and(|proof| proof.is_empty()) {
        eyre::bail!("privacy proof payload must be non-empty");
    }
    if usage_commitment.is_some() != proof.is_some() {
        eyre::bail!("usage commitment and privacy proof must be supplied together");
    }
    Ok(())
}
fn parse_hash(hex: &str) -> Result<Hash> {
    let trimmed = hex.strip_prefix("0x").unwrap_or(hex);
    Hash::from_str(trimmed).wrap_err("invalid hash literal")
}
fn decode_hex_vec(hex: &str) -> Result<Vec<u8>> {
    let trimmed = hex.strip_prefix("0x").unwrap_or(hex);
    hex::decode(trimmed).wrap_err("invalid hex encoding")
}
fn read_manifest(path: &str) -> Result<KaigiRelayManifest> {
    let contents = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read relay manifest from `{path}`"))?;
    let manifest = norito::json::from_str(&contents).wrap_err("invalid relay manifest JSON")?;
    validate_relay_manifest_limits(&manifest)?;
    Ok(manifest)
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
    const HOST_ACCOUNT: &str = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
    const PARTICIPANT_ACCOUNT: &str = "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D";
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
            HOST_ACCOUNT,
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
    fn clap_parses_create_with_optional_privacy_fields() {
        match parse_command(&[
            "create",
            "--domain",
            "kaigi",
            "--call-name",
            "daily",
            "--host",
            HOST_ACCOUNT,
            "--commitment-hex",
            "0xdeadbeef",
            "--commitment-alias",
            "host",
            "--nullifier-hex",
            "cafebabe",
            "--nullifier-issued-at-ms",
            "123",
            "--roster-root-hex",
            "feedface",
            "--proof-hex",
            "aa55",
        ]) {
            Command::Create(args) => {
                assert_eq!(args.commitment_hex.as_deref(), Some("0xdeadbeef"));
                assert_eq!(args.commitment_alias.as_deref(), Some("host"));
                assert_eq!(args.nullifier_hex.as_deref(), Some("cafebabe"));
                assert_eq!(args.nullifier_issued_at_ms, Some(123));
                assert_eq!(args.roster_root_hex.as_deref(), Some("feedface"));
                assert_eq!(args.proof_hex.as_deref(), Some("aa55"));
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
            PARTICIPANT_ACCOUNT,
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
                assert_eq!(args.participant, PARTICIPANT_ACCOUNT);
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
    fn clap_parses_register_relay() {
        match parse_command(&[
            "register-relay",
            "--relay",
            PARTICIPANT_ACCOUNT,
            "--hpke-public-key-b64",
            "qrvM3e7/AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBk=",
            "--bandwidth-class",
            "7",
        ]) {
            Command::RegisterRelay(args) => {
                assert_eq!(args.relay, PARTICIPANT_ACCOUNT);
                assert_eq!(
                    args.hpke_public_key_b64,
                    "qrvM3e7/AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBk="
                );
                assert_eq!(args.bandwidth_class, 7);
            }
            other => panic!("expected register-relay command, got {other:?}"),
        }
    }
    #[test]
    fn clap_parses_unregister_relay() {
        match parse_command(&["unregister-relay", "--relay", PARTICIPANT_ACCOUNT]) {
            Command::UnregisterRelay(args) => {
                assert_eq!(args.relay, PARTICIPANT_ACCOUNT);
            }
            other => panic!("expected unregister-relay command, got {other:?}"),
        }
    }
    #[test]
    fn clap_parses_set_relay_manifest_with_clear() {
        match parse_command(&[
            "set-relay-manifest",
            "--domain",
            "kaigi",
            "--call-name",
            "daily",
            "--clear",
        ]) {
            Command::SetRelayManifest(args) => {
                assert_eq!(args.domain, "kaigi");
                assert_eq!(args.call_name, "daily");
                assert!(args.clear);
                assert!(args.relay_manifest.is_none());
            }
            other => panic!("expected set-relay-manifest command, got {other:?}"),
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
            PARTICIPANT_ACCOUNT,
        ]) {
            Command::Leave(args) => {
                assert_eq!(args.domain, "kaigi");
                assert_eq!(args.call_name, "daily");
                assert_eq!(args.participant, PARTICIPANT_ACCOUNT);
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
    fn clap_parses_end_with_optional_privacy_fields() {
        match parse_command(&[
            "end",
            "--domain",
            "kaigi",
            "--call-name",
            "daily",
            "--commitment-hex",
            "0xdeadbeef",
            "--commitment-alias",
            "host",
            "--nullifier-hex",
            "cafebabe",
            "--nullifier-issued-at-ms",
            "456",
            "--roster-root-hex",
            "feedface",
            "--proof-hex",
            "aa55",
        ]) {
            Command::End(args) => {
                assert_eq!(args.commitment_hex.as_deref(), Some("0xdeadbeef"));
                assert_eq!(args.commitment_alias.as_deref(), Some("host"));
                assert_eq!(args.nullifier_hex.as_deref(), Some("cafebabe"));
                assert_eq!(args.nullifier_issued_at_ms, Some(456));
                assert_eq!(args.roster_root_hex.as_deref(), Some("feedface"));
                assert_eq!(args.proof_hex.as_deref(), Some("aa55"));
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
                assert_eq!(args.domain, "wonderland.universal");
                assert!(parse_call_id(&args.domain, "demo").is_ok());
                assert!(args.call_name.is_none());
                assert!(args.host.is_none());
            }
            other => panic!("expected quickstart command, got {other:?}"),
        }
    }
    #[test]
    fn generated_quickstart_call_labels_are_process_and_subsecond_scoped() {
        use std::time::Duration;

        let instant = UNIX_EPOCH + Duration::new(1_700_000_000, 123);
        let first = generated_call_label(instant, 7).expect("generated label");
        let other_process = generated_call_label(instant, 8).expect("generated label");
        let next_tick =
            generated_call_label(instant + Duration::from_nanos(1), 7).expect("generated label");

        assert!(first.starts_with("kaigi_demo_"));
        assert_ne!(first, other_process);
        assert_ne!(first, next_tick);
        assert!(
            iroha::data_model::name::Name::from_str(&first).is_ok(),
            "generated label must be a valid Name"
        );
    }
    #[test]
    fn quickstart_rejects_retired_auto_join_host_flag() {
        let result = TestCli::try_parse_from(["test", "quickstart", "--auto-join-host"]);
        assert!(
            result.is_err(),
            "retired compatibility flags must not parse"
        );
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
    fn local_scalar_validation_rejects_requests_core_would_refuse() {
        assert!(validate_max_participants(None).is_ok());
        assert!(validate_max_participants(Some(1)).is_ok());
        assert!(validate_max_participants(Some(0)).is_err());
        assert!(validate_usage_duration(1).is_ok());
        assert!(validate_usage_duration(0).is_err());
        assert!(validate_relay_hpke_public_key(&[1]).is_ok());
        assert!(validate_relay_hpke_public_key(&[]).is_err());
        assert!(
            validate_relay_hpke_public_key(&vec![0xA5; KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1])
                .is_ok()
        );
        assert!(
            validate_relay_hpke_public_key(&vec![
                0xA5;
                KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1 + 1
            ])
            .is_err()
        );
        let max_notes = "界".repeat(512);
        let oversized_notes = "界".repeat(513);
        assert!(validate_relay_health_notes(None).is_ok());
        assert!(validate_relay_health_notes(Some(&max_notes)).is_ok());
        assert!(validate_relay_health_notes(Some(&oversized_notes)).is_err());

        let hash_hex = "ab".repeat(32);
        assert!(parse_hash(&format!("0x{hash_hex}")).is_ok());
        assert!(parse_hash(&format!("0x0x{hash_hex}")).is_err());
        assert_eq!(
            decode_hex_vec("0xaa55").expect("single prefix"),
            [0xaa, 0x55]
        );
        assert!(decode_hex_vec("0x0xaa55").is_err());
    }
    #[test]
    fn relay_manifest_local_limits_match_core_v1_boundaries() {
        fn hop(key_len: usize) -> iroha::data_model::kaigi::KaigiRelayHop {
            let key_pair =
                iroha_crypto::KeyPair::try_random().expect("generate checked relay fixture key");
            iroha::data_model::kaigi::KaigiRelayHop {
                relay_id: AccountId::new(key_pair.public_key().clone()),
                hpke_public_key: vec![0xA5; key_len],
                weight: 1,
            }
        }

        let exact_minimum = KaigiRelayManifest {
            hops: (0..KAIGI_RELAY_MANIFEST_MIN_HOPS_V1)
                .map(|_| hop(KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1))
                .collect(),
            expiry_ms: 1,
        };
        assert!(validate_relay_manifest_limits(&exact_minimum).is_ok());

        let too_few = KaigiRelayManifest {
            hops: (0..KAIGI_RELAY_MANIFEST_MIN_HOPS_V1 - 1)
                .map(|_| hop(1))
                .collect(),
            expiry_ms: 1,
        };
        assert!(validate_relay_manifest_limits(&too_few).is_err());

        let too_many = KaigiRelayManifest {
            hops: (0..KAIGI_RELAY_MANIFEST_MAX_HOPS_V1 + 1)
                .map(|_| hop(1))
                .collect(),
            expiry_ms: 1,
        };
        assert!(validate_relay_manifest_limits(&too_many).is_err());

        let oversized_key = KaigiRelayManifest {
            hops: (0..KAIGI_RELAY_MANIFEST_MIN_HOPS_V1)
                .map(|_| hop(KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1 + 1))
                .collect(),
            expiry_ms: 1,
        };
        assert!(validate_relay_manifest_limits(&oversized_key).is_err());
    }
    #[test]
    fn local_billing_validation_requires_the_resolved_host() {
        let host = AccountId::parse_encoded(HOST_ACCOUNT).expect("host account");
        let participant =
            AccountId::parse_encoded(PARTICIPANT_ACCOUNT).expect("participant account");

        assert!(validate_billing_account(&host, None).is_ok());
        assert!(validate_billing_account(&host, Some(&host)).is_ok());
        assert!(validate_billing_account(&host, Some(&participant)).is_err());
    }
    #[test]
    fn parse_optional_privacy_artifacts_builds_ledger_safe_fields() {
        let commitment_hex = format!("0x{}", "ab".repeat(32));
        let nullifier_hex = format!("0x{}", "cd".repeat(32));
        let roster_root_hex = format!("0x{}", "ef".repeat(32));
        let artifacts = parse_optional_privacy_artifacts(
            Some(&commitment_hex),
            None,
            Some(&nullifier_hex),
            Some(0),
            Some(&roster_root_hex),
            Some("aa55"),
        )
        .expect("valid privacy artifacts");
        assert_eq!(
            artifacts
                .commitment
                .as_ref()
                .and_then(|commitment| commitment.alias_tag.as_deref()),
            None
        );
        assert_eq!(
            artifacts
                .nullifier
                .as_ref()
                .map(|nullifier| nullifier.issued_at_ms),
            Some(0)
        );
        assert_eq!(artifacts.proof, Some(vec![0xaa, 0x55]));
        assert_eq!(
            artifacts.roster_root,
            Some(parse_hash(&roster_root_hex).unwrap())
        );
    }
    #[test]
    fn parse_optional_privacy_artifacts_rejects_clear_identity_hints() {
        let commitment_hex = format!("0x{}", "ab".repeat(32));
        let nullifier_hex = format!("0x{}", "cd".repeat(32));
        let alias_error = parse_optional_privacy_artifacts(
            Some(&commitment_hex),
            Some("host"),
            Some(&nullifier_hex),
            None,
            None,
            None,
        )
        .expect_err("clear aliases must not enter ledger privacy artifacts");
        assert!(alias_error.to_string().contains("off-chain only"));
        let timestamp_error = parse_optional_privacy_artifacts(
            Some(&commitment_hex),
            None,
            Some(&nullifier_hex),
            Some(1),
            None,
            None,
        )
        .expect_err("clear issuance timestamps must not enter ledger privacy artifacts");
        assert!(timestamp_error.to_string().contains("off-chain only"));
    }
    #[test]
    fn privacy_artifacts_must_be_complete_and_match_create_mode() {
        let commitment_hex = format!("0x{}", "ab".repeat(32));
        let nullifier_hex = format!("0x{}", "cd".repeat(32));
        let roster_root_hex = format!("0x{}", "ef".repeat(32));
        let partial_error =
            parse_optional_privacy_artifacts(Some(&commitment_hex), None, None, None, None, None)
                .expect_err("partial privacy artifacts must fail locally");
        assert!(partial_error.to_string().contains("all be omitted"));

        let empty_proof_error = parse_optional_privacy_artifacts(
            Some(&commitment_hex),
            None,
            Some(&nullifier_hex),
            Some(0),
            Some(&roster_root_hex),
            Some(""),
        )
        .expect_err("empty privacy proofs must fail locally");
        assert!(empty_proof_error.to_string().contains("non-empty"));

        let complete = parse_optional_privacy_artifacts(
            Some(&commitment_hex),
            None,
            Some(&nullifier_hex),
            Some(0),
            Some(&roster_root_hex),
            Some("aa55"),
        )
        .expect("complete privacy artifacts");
        assert!(
            validate_create_privacy_artifacts(KaigiPrivacyMode::Transparent, &complete).is_err()
        );
        assert!(validate_create_privacy_artifacts(KaigiPrivacyMode::ZkRosterV1, &complete).is_ok());

        assert!(validate_leave_privacy_arguments(None, None, None, None, None).is_ok());
        let leave_error = validate_leave_privacy_arguments(
            Some(&commitment_hex),
            Some(&nullifier_hex),
            Some(0),
            Some(&roster_root_hex),
            Some("aa55"),
        )
        .expect_err("on-chain leave must reject even complete privacy artifacts");
        assert!(leave_error.to_string().contains("off-chain only"));
        assert!(validate_leave_privacy_arguments(None, None, Some(0), None, None).is_err());
    }
    #[test]
    fn usage_privacy_artifacts_must_be_supplied_together() {
        let commitment = Hash::new(b"usage");
        assert!(validate_usage_privacy_artifacts(None, None).is_ok());
        assert!(validate_usage_privacy_artifacts(Some(&commitment), Some(&[1])).is_ok());
        assert!(validate_usage_privacy_artifacts(Some(&commitment), None).is_err());
        assert!(validate_usage_privacy_artifacts(None, Some(&[1])).is_err());
        assert!(validate_usage_privacy_artifacts(Some(&commitment), Some(&[])).is_err());
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
            host: HOST_ACCOUNT.to_string(),
            torii_url: "http://localhost:8080".to_string(),
            room_policy: "Public".to_string(),
            privacy_mode: "Transparent".to_string(),
            join_hint: "iroha kaigi join ...".to_string(),
        };
        let text = render_quickstart_text(&summary, Some(Path::new("/tmp/summary.json")));
        assert!(text.contains("call_id: call-1"));
        assert!(text.contains("summary_out: /tmp/summary.json"));
    }
}
