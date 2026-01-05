//! Governance helpers (app API convenience). Build/submit governance transactions.

mod audit;
mod council;
mod deploy;
mod shared;
mod vote;

use self::council::{CouncilArgs, CouncilSubcommand};
use crate::{Run, RunContext};
use eyre::Result;

pub use audit::AuditDeployArgs;
pub use deploy::{
    ActivateInstanceArgs, DeployMetaArgs, EnactArgs, FinalizeArgs, InstancesArgs,
    ProposeDeployArgs, ProtectedApplyArgs, ProtectedGetArgs, ProtectedSetArgs,
};
pub use vote::{
    LocksGetArgs, ProposalGetArgs, ReferendumGetArgs, TallyGetArgs, UnlockStatsArgs, VoteArgs,
    VotePlainArgs, VoteZkArgs,
};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Propose deployment of IVM bytecode by code/abi hash via governance (build-only; server returns instruction skeleton)
    ProposeDeploy(ProposeDeployArgs),
    /// Submit a governance ballot; auto-detects referendum mode unless overridden
    Vote(VoteArgs),
    /// Submit a ZK ballot (server returns instruction skeleton)
    VoteZk(VoteZkArgs),
    /// Submit a non-ZK quadratic ballot (server returns instruction skeleton)
    VotePlain(VotePlainArgs),
    /// Get a governance proposal by id (hex)
    ProposalGet(ProposalGetArgs),
    /// Get locks for a referendum id
    LocksGet(LocksGetArgs),
    /// Get current sortition council or manage council VRF flows.
    Council {
        #[command(flatten)]
        args: CouncilArgs,
        #[command(subcommand)]
        action: Option<CouncilSubcommand>,
    },
    /// Show governance unlock sweep stats (expired locks at current height)
    UnlockStats(UnlockStatsArgs),
    /// Get a referendum by id
    ReferendumGet(ReferendumGetArgs),
    /// Get a tally snapshot by referendum id
    TallyGet(TallyGetArgs),
    /// Build a finalize transaction for a referendum (server returns instruction skeleton)
    Finalize(FinalizeArgs),
    /// Build an enactment transaction for an approved proposal
    Enact(EnactArgs),
    /// Set protected namespaces (custom parameter `gov_protected_namespaces`)
    ProtectedSet(ProtectedSetArgs),
    /// Apply protected namespaces on the server (requires API token if configured)
    ProtectedApply(ProtectedApplyArgs),
    /// Get protected namespaces (custom parameter `gov_protected_namespaces`)
    ProtectedGet(ProtectedGetArgs),
    /// Activate a contract instance (namespace, `contract_id`) -> `code_hash` (admin/testing)
    ActivateInstance(ActivateInstanceArgs),
    /// List active contract instances for a namespace
    Instances(InstancesArgs),
    /// Build deploy metadata JSON for protected namespace admission
    DeployMeta(DeployMetaArgs),
    /// Audit stored manifests against governance proposals and code storage
    AuditDeploy(AuditDeployArgs),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::ProposeDeploy(args) => args.run(context),
            Command::Vote(args) => args.run(context),
            Command::VoteZk(args) => args.run(context),
            Command::VotePlain(args) => args.run(context),
            Command::ProposalGet(args) => args.run(context),
            Command::LocksGet(args) => args.run(context),
            Command::Council { args, action } => match action {
                Some(CouncilSubcommand::DeriveVrf(cmd)) => cmd.run(context),
                Some(CouncilSubcommand::Persist(cmd)) => cmd.run(context),
                Some(CouncilSubcommand::GenVrf(cmd)) => cmd.run(context),
                Some(CouncilSubcommand::DeriveAndPersist(cmd)) => cmd.run(context),
                Some(CouncilSubcommand::Replace(cmd)) => cmd.run(context),
                None => args.run(context),
            },
            Command::UnlockStats(args) => args.run(context),
            Command::ReferendumGet(args) => args.run(context),
            Command::TallyGet(args) => args.run(context),
            Command::Finalize(args) => args.run(context),
            Command::Enact(args) => args.run(context),
            Command::ProtectedSet(args) => args.run(context),
            Command::ProtectedApply(args) => args.run(context),
            Command::ProtectedGet(args) => args.run(context),
            Command::DeployMeta(args) => args.run(context),
            Command::ActivateInstance(args) => args.run(context),
            Command::Instances(args) => args.run(context),
            Command::AuditDeploy(args) => args.run(context),
        }
    }
}
