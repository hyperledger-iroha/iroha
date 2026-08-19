use super::{evidence, status, telemetry, vrf};
use crate::{Run, RunContext};
use clap::ValueEnum;
use eyre::Result;
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Show consensus status snapshot (leader, `HighestQC`, `LockedQC`)
    Status(StatusArgs),
    /// Show non-authoritative pipeline, queue, election, and lane diagnostics
    Diagnostics(DiagnosticsArgs),
    /// Show leader index (and PRF context when available)
    Leader(LeaderArgs),
    /// Show on-chain Sumeragi parameters snapshot
    Params(ParamsArgs),
    /// Show HighestQC/LockedQC snapshot
    Qc(QcArgs),
    /// Show pacemaker timers/config snapshot
    Pacemaker(PacemakerArgs),
    /// Evidence audit helpers (list/count)
    #[command(subcommand)]
    Evidence(EvidenceCommand),
    /// Show VRF penalties for the given epoch
    VrfPenalties(VrfPenaltiesArgs),
    /// Show persisted VRF epoch snapshot (seed, participants, penalties)
    VrfEpoch(VrfEpochArgs),
}
#[derive(clap::Subcommand, Debug)]
pub enum EvidenceCommand {
    /// List persisted evidence entries
    List(EvidenceListArgs),
    /// Show evidence count
    Count(EvidenceCountArgs),
}
#[derive(clap::Args, Debug)]
pub struct StatusArgs {}
#[derive(clap::Args, Debug)]
pub struct DiagnosticsArgs {}
#[derive(clap::Args, Debug)]
pub struct LeaderArgs {}
#[derive(clap::Args, Debug)]
pub struct ParamsArgs {}
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
}
#[derive(clap::Args, Debug)]
pub struct EvidenceCountArgs {}
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum EvidenceKindArg {
    #[value(name = "SumeragiV2Equivocation")]
    SumeragiV2Equivocation,
}
impl EvidenceKindArg {
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceKindArg::SumeragiV2Equivocation => "SumeragiV2Equivocation",
        }
    }
}
#[derive(clap::Args, Debug)]
pub struct QcArgs {}
#[derive(clap::Args, Debug)]
pub struct PacemakerArgs {}
#[derive(clap::Args, Debug)]
pub struct VrfPenaltiesArgs {
    /// Epoch index (decimal or 0x-prefixed hex)
    #[arg(long, value_name = "EPOCH")]
    pub epoch: String,
}
#[derive(clap::Args, Debug)]
pub struct VrfEpochArgs {
    /// Epoch index (decimal or 0x-prefixed hex)
    #[arg(long, value_name = "EPOCH")]
    pub epoch: String,
}
impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Status(args) => status::status(context, args),
            Command::Diagnostics(args) => status::diagnostics(context, args),
            Command::Leader(args) => status::leader(context, args),
            Command::Params(args) => status::params(context, args),
            Command::Qc(args) => status::qc(context, args),
            Command::Pacemaker(args) => telemetry::pacemaker(context, args),
            Command::Evidence(cmd) => cmd.run(context),
            Command::VrfPenalties(args) => vrf::penalties(context, args),
            Command::VrfEpoch(args) => vrf::epoch(context, args),
        }
    }
}
impl Run for EvidenceCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            EvidenceCommand::List(args) => evidence::list(context, args),
            EvidenceCommand::Count(args) => evidence::count(context, args),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::EvidenceKindArg;
    use clap::ValueEnum as _;
    #[test]
    fn evidence_kind_filter_maps_to_the_current_wire_name() {
        let cases = [(
            EvidenceKindArg::SumeragiV2Equivocation,
            "SumeragiV2Equivocation",
        )];
        for (kind, expected) in cases {
            assert_eq!(kind.as_str(), expected);
            assert_eq!(
                kind.to_possible_value()
                    .expect("evidence kind must have a CLI value")
                    .get_name(),
                expected
            );
        }
    }
}
