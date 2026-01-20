//! `NPoS` staking lifecycle helpers for public lanes.

use crate::{Run, RunContext};
use eyre::{Result, WrapErr, eyre};
use iroha::data_model::{
    isi::InstructionBox,
    isi::staking::{
        ActivatePublicLaneValidator, ExitPublicLaneValidator, RegisterPublicLaneValidator,
    },
    metadata::Metadata,
    nexus::LaneId,
    prelude::AccountId,
};
use iroha_primitives::numeric::Numeric;
use std::{fs, path::PathBuf};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Register a stake-elected validator on a public lane
    Register(RegisterArgs),
    /// Activate a pending validator once its activation epoch is reached
    Activate(ActivateArgs),
    /// Schedule or finalize a validator exit
    Exit(ExitArgs),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Register(args) => args.run(context),
            Command::Activate(args) => args.run(context),
            Command::Exit(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct RegisterArgs {
    /// Lane id to register against
    #[arg(long)]
    pub lane_id: u32,
    /// Validator account identifier (IH58/compressed/0x, uaid:, opaque:, or <alias|public_key>@domain)
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub validator: String,
    /// Optional staking account (defaults to validator)
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub stake_account: Option<String>,
    /// Initial self-bond (integer, uses the staking asset scale)
    #[arg(long, value_name = "AMOUNT")]
    pub initial_stake: u64,
    /// Optional metadata JSON (Norito JSON object)
    #[arg(long, value_name = "PATH")]
    pub metadata: Option<PathBuf>,
}

impl Run for RegisterArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let lane_id = LaneId::new(self.lane_id);
        let validator = parse_account_id(context, &self.validator, "--validator")?;
        let stake_account = match self.stake_account {
            Some(value) => parse_account_id(context, &value, "--stake-account")?,
            None => validator.clone(),
        };
        let metadata = load_metadata(self.metadata.as_ref())?;
        let instruction: InstructionBox = RegisterPublicLaneValidator {
            lane_id,
            validator,
            stake_account,
            initial_stake: Numeric::new(self.initial_stake, 0),
            metadata,
        }
        .into();
        context.finish(vec![instruction])
    }
}

#[derive(clap::Args, Debug)]
pub struct ActivateArgs {
    /// Lane id containing the pending validator
    #[arg(long)]
    pub lane_id: u32,
    /// Validator account identifier (IH58/compressed/0x, uaid:, opaque:, or <alias|public_key>@domain)
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub validator: String,
}

impl Run for ActivateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let lane_id = LaneId::new(self.lane_id);
        let validator = parse_account_id(context, &self.validator, "--validator")?;
        let instruction: InstructionBox = ActivatePublicLaneValidator { lane_id, validator }.into();
        context.finish(vec![instruction])
    }
}

#[derive(clap::Args, Debug)]
pub struct ExitArgs {
    /// Lane id containing the validator
    #[arg(long)]
    pub lane_id: u32,
    /// Validator account identifier (IH58/compressed/0x, uaid:, opaque:, or <alias|public_key>@domain)
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub validator: String,
    /// Release timestamp in milliseconds (must not precede current block timestamp)
    #[arg(long, value_name = "MILLIS")]
    pub release_at_ms: u64,
}

impl Run for ExitArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let lane_id = LaneId::new(self.lane_id);
        let validator = parse_account_id(context, &self.validator, "--validator")?;
        let instruction: InstructionBox = ExitPublicLaneValidator {
            lane_id,
            validator,
            release_at_ms: self.release_at_ms,
        }
        .into();
        context.finish(vec![instruction])
    }
}

fn parse_account_id<C: RunContext>(context: &C, value: &str, flag: &str) -> Result<AccountId> {
    crate::resolve_account_id(context, value)
        .map_err(|err| eyre!("invalid account id passed to {flag}: {err}"))
}

fn load_metadata(path: Option<&PathBuf>) -> Result<Metadata> {
    let Some(path) = path else {
        return Ok(Metadata::default());
    };
    let raw = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read metadata from {}", path.display()))?;
    norito::json::from_str(&raw).wrap_err("metadata file is not valid Norito JSON")
}
