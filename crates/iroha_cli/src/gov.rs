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
};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Deployment helpers (propose/meta/audit)
    #[command(subcommand)]
    Deploy(DeployCommand),
    /// Submit a governance ballot; auto-detects referendum mode unless overridden
    Vote(VoteArgs),
    /// Proposal helpers
    #[command(subcommand)]
    Proposal(ProposalCommand),
    /// Lock helpers
    #[command(subcommand)]
    Locks(LocksCommand),
    /// Get current sortition council or manage council VRF flows.
    Council {
        #[command(flatten)]
        args: CouncilArgs,
        #[command(subcommand)]
        action: Option<CouncilSubcommand>,
    },
    /// Unlock helpers (expired lock stats)
    #[command(subcommand)]
    Unlock(UnlockCommand),
    /// Referendum helpers
    #[command(subcommand)]
    Referendum(ReferendumCommand),
    /// Tally helpers
    #[command(subcommand)]
    Tally(TallyCommand),
    /// Build a finalize transaction for a referendum (server returns instruction skeleton)
    Finalize(FinalizeArgs),
    /// Build an enactment transaction for an approved proposal
    Enact(EnactArgs),
    /// Protected namespace helpers
    #[command(subcommand)]
    Protected(ProtectedCommand),
    /// Contract instance helpers
    #[command(subcommand)]
    Instance(InstanceCommand),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Deploy(cmd) => cmd.run(context),
            Command::Vote(args) => args.run(context),
            Command::Proposal(cmd) => cmd.run(context),
            Command::Locks(cmd) => cmd.run(context),
            Command::Council { args, action } => match action {
                Some(CouncilSubcommand::DeriveVrf(cmd)) => cmd.run(context),
                Some(CouncilSubcommand::Persist(cmd)) => cmd.run(context),
                Some(CouncilSubcommand::GenVrf(cmd)) => cmd.run(context),
                Some(CouncilSubcommand::DeriveAndPersist(cmd)) => cmd.run(context),
                Some(CouncilSubcommand::Replace(cmd)) => cmd.run(context),
                None => args.run(context),
            },
            Command::Unlock(cmd) => cmd.run(context),
            Command::Referendum(cmd) => cmd.run(context),
            Command::Tally(cmd) => cmd.run(context),
            Command::Finalize(args) => args.run(context),
            Command::Enact(args) => args.run(context),
            Command::Protected(cmd) => cmd.run(context),
            Command::Instance(cmd) => cmd.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum DeployCommand {
    /// Propose deployment of IVM bytecode by code/abi hash via governance (build-only; server returns instruction skeleton)
    Propose(ProposeDeployArgs),
    /// Build deploy metadata JSON for protected namespace admission
    Meta(DeployMetaArgs),
    /// Audit stored manifests against governance proposals and code storage
    Audit(AuditDeployArgs),
}

impl Run for DeployCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            DeployCommand::Propose(args) => args.run(context),
            DeployCommand::Meta(args) => args.run(context),
            DeployCommand::Audit(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum ProposalCommand {
    /// Get a governance proposal by id (hex)
    Get(ProposalGetArgs),
}

impl Run for ProposalCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            ProposalCommand::Get(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum LocksCommand {
    /// Get locks for a referendum id
    Get(LocksGetArgs),
}

impl Run for LocksCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            LocksCommand::Get(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum ReferendumCommand {
    /// Get a referendum by id
    Get(ReferendumGetArgs),
}

impl Run for ReferendumCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            ReferendumCommand::Get(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum TallyCommand {
    /// Get a tally snapshot by referendum id
    Get(TallyGetArgs),
}

impl Run for TallyCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            TallyCommand::Get(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum UnlockCommand {
    /// Show governance unlock sweep stats (expired locks at current height)
    Stats(UnlockStatsArgs),
}

impl Run for UnlockCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            UnlockCommand::Stats(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum ProtectedCommand {
    /// Set protected namespaces (custom parameter `gov_protected_namespaces`)
    Set(ProtectedSetArgs),
    /// Apply protected namespaces on the server (requires API token if configured)
    Apply(ProtectedApplyArgs),
    /// Get protected namespaces (custom parameter `gov_protected_namespaces`)
    Get(ProtectedGetArgs),
}

impl Run for ProtectedCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            ProtectedCommand::Set(args) => args.run(context),
            ProtectedCommand::Apply(args) => args.run(context),
            ProtectedCommand::Get(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum InstanceCommand {
    /// Activate a contract instance (namespace, `contract_id`) -> `code_hash` (admin/testing)
    Activate(ActivateInstanceArgs),
    /// List active contract instances for a namespace
    List(InstancesArgs),
}

impl Run for InstanceCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            InstanceCommand::Activate(args) => args.run(context),
            InstanceCommand::List(args) => args.run(context),
        }
    }
}
