use std::path::PathBuf;

use clap::ValueEnum;
use eyre::Result;

use crate::{Run, RunContext};

use super::{evidence, exec, rbc, status, telemetry, vrf};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Show consensus status snapshot (leader, `HighestQC`, `LockedQC`)
    Status(StatusArgs),
    /// Show leader index (and PRF context when available)
    Leader(LeaderArgs),
    /// Show on-chain Sumeragi parameters snapshot
    Params(ParamsArgs),
    /// Show current collector indices and peers
    Collectors(CollectorsArgs),
    /// Show HighestQC/LockedQC snapshot
    Qc(QcArgs),
    /// Show pacemaker timers/config snapshot
    Pacemaker(PacemakerArgs),
    /// Show latest per-phase latencies (ms)
    Phases(PhasesArgs),
    /// Show aggregated telemetry snapshot (availability, QC, RBC, VRF)
    Telemetry(TelemetryArgs),
    /// Evidence helpers (list/count/submit)
    #[command(subcommand)]
    Evidence(EvidenceCommand),
    /// RBC helpers (status/sessions)
    #[command(subcommand)]
    Rbc(RbcCommand),
    /// Show VRF penalties for the given epoch
    VrfPenalties(VrfPenaltiesArgs),
    /// Show persisted VRF epoch snapshot (seed, participants, penalties)
    VrfEpoch(VrfEpochArgs),
    /// Fetch full `ExecutionQC` record (if present) for a parent block hash
    ExecQcGet(ExecQcGetArgs),
    /// Fetch execution root (if present) for a parent block hash
    ExecRootGet(ExecRootGetArgs),
}

#[derive(clap::Subcommand, Debug)]
pub enum EvidenceCommand {
    /// List persisted evidence entries
    List(EvidenceListArgs),
    /// Show evidence count
    Count(EvidenceCountArgs),
    /// Submit hex-encoded evidence payload
    Submit(EvidenceSubmitArgs),
}

#[derive(clap::Subcommand, Debug)]
pub enum RbcCommand {
    /// Show RBC session/throughput counters
    Status(RbcStatusArgs),
    /// Show RBC sessions snapshot
    Sessions(RbcSessionsArgs),
}

#[derive(clap::Args, Debug)]
pub struct StatusArgs {
    /// Print a single compact line instead of JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
}

#[derive(clap::Args, Debug)]
pub struct LeaderArgs {
    /// Print a single compact line instead of JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
}

#[derive(clap::Args, Debug)]
pub struct ParamsArgs {
    /// Print a compact summary instead of JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
}

#[derive(clap::Args, Debug)]
pub struct EvidenceListArgs {
    /// Maximum number of entries to return
    #[arg(long)]
    pub limit: Option<u32>,
    /// Offset into the evidence list
    #[arg(long)]
    pub offset: Option<u32>,
    /// Filter by evidence kind
    #[arg(long, value_enum)]
    pub kind: Option<EvidenceKindArg>,
    /// Print human-readable summaries before JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
    /// Print summaries only (omit JSON)
    #[arg(long, default_value_t = false)]
    pub summary_only: bool,
}

#[derive(clap::Args, Debug)]
pub struct EvidenceCountArgs {
    /// Print human-readable summary before JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
    /// Print summary only (omit JSON)
    #[arg(long, default_value_t = false)]
    pub summary_only: bool,
}

#[derive(clap::Args, Debug)]
pub struct EvidenceSubmitArgs {
    /// Hex-encoded Norito evidence payload (0x optional)
    #[arg(long, conflicts_with = "evidence_hex_file")]
    pub evidence_hex: Option<String>,
    /// Path to file containing hex-encoded proof (whitespace ignored)
    #[arg(long, value_name = "PATH", conflicts_with = "evidence_hex")]
    pub evidence_hex_file: Option<PathBuf>,
    /// Print human-readable summary before JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
    /// Print summary only (omit JSON)
    #[arg(long, default_value_t = false)]
    pub summary_only: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum EvidenceKindArg {
    DoublePrepare,
    DoubleCommit,
    DoubleExecVote,
    #[value(alias = "invalid-qc")]
    InvalidCommitCertificate,
    InvalidProposal,
}

impl EvidenceKindArg {
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceKindArg::DoublePrepare => "DoublePrepare",
            EvidenceKindArg::DoubleCommit => "DoubleCommit",
            EvidenceKindArg::DoubleExecVote => "DoubleExecVote",
            EvidenceKindArg::InvalidCommitCertificate => "InvalidCommitCertificate",
            EvidenceKindArg::InvalidProposal => "InvalidProposal",
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct CollectorsArgs {
    /// Print a compact summary instead of JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
}

#[derive(clap::Args, Debug)]
pub struct QcArgs {
    /// Print a compact summary instead of JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
}

#[derive(clap::Args, Debug)]
pub struct PacemakerArgs {
    /// Print a compact summary instead of JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
}

#[derive(clap::Args, Debug)]
pub struct PhasesArgs {
    /// Print a compact summary instead of JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
}

#[derive(clap::Args, Debug)]
pub struct TelemetryArgs {
    /// Print a compact summary instead of JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
}

#[derive(clap::Args, Debug)]
pub struct RbcStatusArgs {
    /// Print a compact summary instead of JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
}

#[derive(clap::Args, Debug)]
pub struct RbcSessionsArgs {
    /// Print a compact summary instead of JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
}

#[derive(clap::Args, Debug)]
pub struct VrfPenaltiesArgs {
    /// Epoch index (decimal or 0x-prefixed hex)
    #[arg(long, value_name = "EPOCH")]
    pub epoch: String,
    /// Print a compact summary instead of JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
}

#[derive(clap::Args, Debug)]
pub struct VrfEpochArgs {
    /// Epoch index (decimal or 0x-prefixed hex)
    #[arg(long, value_name = "EPOCH")]
    pub epoch: String,
    /// Print a compact summary instead of JSON
    #[arg(long, default_value_t = false)]
    pub summary: bool,
}

#[derive(clap::Args, Debug)]
pub struct ExecQcGetArgs {
    /// Block hash for which the `ExecutionQC` should be fetched
    #[arg(long)]
    pub hash: String,
}

#[derive(clap::Args, Debug)]
pub struct ExecRootGetArgs {
    /// Block hash for which the execution root should be fetched
    #[arg(long)]
    pub hash: String,
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Status(args) => status::status(context, args),
            Command::Leader(args) => status::leader(context, args),
            Command::Params(args) => status::params(context, args),
            Command::Collectors(args) => status::collectors(context, args),
            Command::Qc(args) => status::qc(context, args),
            Command::Pacemaker(args) => telemetry::pacemaker(context, args),
            Command::Phases(args) => telemetry::phases(context, args),
            Command::Telemetry(args) => telemetry::telemetry(context, args),
            Command::Evidence(cmd) => cmd.run(context),
            Command::Rbc(cmd) => rbc::run(context, cmd),
            Command::VrfPenalties(args) => vrf::penalties(context, args),
            Command::VrfEpoch(args) => vrf::epoch(context, args),
            Command::ExecQcGet(args) => exec::qc(context, args),
            Command::ExecRootGet(args) => exec::root(context, args),
        }
    }
}

impl Run for EvidenceCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            EvidenceCommand::List(args) => evidence::list(context, args),
            EvidenceCommand::Count(args) => evidence::count(context, args),
            EvidenceCommand::Submit(args) => evidence::submit(context, args),
        }
    }
}
