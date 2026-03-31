//! `NPoS` staking lifecycle helpers for public lanes.

use crate::{Run, RunContext};
use eyre::{Result, WrapErr, eyre};
use iroha::data_model::{
    isi::InstructionBox,
    isi::staking::{
        ActivatePublicLaneValidator, ExitPublicLaneValidator, RebindPublicLaneValidatorPeer,
        RegisterPublicLaneValidator,
    },
    metadata::Metadata,
    nexus::LaneId,
    peer::PeerId,
    prelude::AccountId,
};
use iroha_primitives::numeric::Numeric;
use std::{fs, path::PathBuf};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Register a stake-elected validator on a public lane
    Register(RegisterArgs),
    /// Rebind an existing validator to a replacement consensus peer
    Rebind(RebindArgs),
    /// Activate a pending validator once its activation epoch is reached
    Activate(ActivateArgs),
    /// Schedule or finalize a validator exit
    Exit(ExitArgs),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Register(args) => args.run(context),
            Command::Rebind(args) => args.run(context),
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
    /// Validator account identifier (canonical I105 account literal)
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub validator: String,
    /// Peer identity that will participate in consensus for this validator
    #[arg(long, value_name = "PEER_ID")]
    pub peer_id: String,
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
        let peer_id = self
            .peer_id
            .parse::<PeerId>()
            .wrap_err("--peer-id must be a valid peer id")?;
        let stake_account = match self.stake_account {
            Some(value) => parse_account_id(context, &value, "--stake-account")?,
            None => validator.clone(),
        };
        let metadata = load_metadata(self.metadata.as_ref())?;
        let instruction: InstructionBox = RegisterPublicLaneValidator {
            lane_id,
            validator,
            peer_id,
            stake_account,
            initial_stake: Numeric::new(self.initial_stake, 0),
            metadata,
        }
        .into();
        context.finish(vec![instruction])
    }
}

#[derive(clap::Args, Debug)]
pub struct RebindArgs {
    /// Lane id containing the validator
    #[arg(long)]
    pub lane_id: u32,
    /// Validator account identifier (canonical I105 account literal)
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub validator: String,
    /// Replacement peer identity that will participate in consensus for this validator
    #[arg(long, value_name = "PEER_ID")]
    pub peer_id: String,
}

impl Run for RebindArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let lane_id = LaneId::new(self.lane_id);
        let validator = parse_account_id(context, &self.validator, "--validator")?;
        let peer_id = self
            .peer_id
            .parse::<PeerId>()
            .wrap_err("--peer-id must be a valid peer id")?;
        let instruction: InstructionBox =
            RebindPublicLaneValidatorPeer::new(lane_id, validator, peer_id).into();
        context.finish(vec![instruction])
    }
}

#[derive(clap::Args, Debug)]
pub struct ActivateArgs {
    /// Lane id containing the pending validator
    #[arg(long)]
    pub lane_id: u32,
    /// Validator account identifier (canonical I105 account literal)
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
    /// Validator account identifier (canonical I105 account literal)
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use eyre::Result;
    use iroha_i18n::Language;
    use iroha_test_samples::ALICE_ID;
    use norito::json::JsonSerialize;
    use std::fmt::Display;

    #[derive(Parser, Debug)]
    #[command(no_binary_name = true)]
    struct Wrapper {
        #[command(subcommand)]
        command: Command,
    }

    struct TestContext {
        cfg: crate::Config,
        submitted: Option<Vec<InstructionBox>>,
        i18n: crate::Localizer,
    }

    impl TestContext {
        fn new() -> Self {
            Self {
                cfg: crate::fallback_config(),
                submitted: None,
                i18n: crate::Localizer::new(crate::Bundle::Cli, Language::English),
            }
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &crate::Config {
            &self.cfg
        }

        fn transaction_metadata(&self) -> Option<&Metadata> {
            None
        }

        fn input_instructions(&self) -> bool {
            false
        }

        fn output_instructions(&self) -> bool {
            false
        }

        fn i18n(&self) -> &crate::Localizer {
            &self.i18n
        }

        fn print_data<T>(&mut self, _data: &T) -> Result<()>
        where
            T: JsonSerialize + ?Sized,
        {
            Ok(())
        }

        fn println(&mut self, _data: impl Display) -> Result<()> {
            Ok(())
        }

        fn submit_with_metadata(
            &mut self,
            instructions: impl Into<crate::Executable>,
            _metadata: Metadata,
            _wait_for_confirmation: bool,
        ) -> Result<()> {
            self.submit(instructions)
        }

        fn submit(&mut self, instructions: impl Into<crate::Executable>) -> Result<()> {
            match instructions.into() {
                crate::Executable::Instructions(list) => {
                    self.submitted = Some(list.into_vec());
                    Ok(())
                }
                crate::Executable::Ivm(_) | crate::Executable::IvmProved(_) => {
                    eyre::bail!("unexpected non-instruction executable in staking test context")
                }
            }
        }
    }

    fn parse_command(args: &[&str]) -> clap::error::Result<Command> {
        Wrapper::try_parse_from(args).map(|wrapper| wrapper.command)
    }

    fn alice_literal() -> String {
        ALICE_ID.canonical_i105().expect("canonical I105")
    }

    fn valid_peer_id_literal() -> String {
        PeerId::from(iroha_crypto::KeyPair::random().public_key().clone()).to_string()
    }

    #[test]
    fn register_requires_peer_id_flag() {
        let err = parse_command(&[
            "register",
            "--lane-id",
            "1",
            "--validator",
            &alice_literal(),
            "--initial-stake",
            "10",
        ])
        .expect_err("register without --peer-id should fail");

        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
        assert!(err.to_string().contains("--peer-id"));
    }

    #[test]
    fn rebind_requires_peer_id_flag() {
        let err = parse_command(&["rebind", "--lane-id", "1", "--validator", &alice_literal()])
            .expect_err("rebind without --peer-id should fail");

        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
        assert!(err.to_string().contains("--peer-id"));
    }

    #[test]
    fn rebind_requires_validator_flag() {
        let err = parse_command(&[
            "rebind",
            "--lane-id",
            "1",
            "--peer-id",
            &valid_peer_id_literal(),
        ])
        .expect_err("rebind without --validator should fail");

        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
        assert!(err.to_string().contains("--validator"));
    }

    #[test]
    fn register_submits_instruction_with_valid_peer_id() {
        let command = parse_command(&[
            "register",
            "--lane-id",
            "1",
            "--validator",
            &alice_literal(),
            "--peer-id",
            &valid_peer_id_literal(),
            "--initial-stake",
            "10",
        ])
        .expect("register command should parse");
        let mut context = TestContext::new();

        command.run(&mut context).expect("register should succeed");

        assert_eq!(
            context.submitted.as_ref().map(Vec::len),
            Some(1),
            "register should submit exactly one instruction"
        );
    }

    #[test]
    fn rebind_submits_instruction_with_valid_peer_id() {
        let command = parse_command(&[
            "rebind",
            "--lane-id",
            "1",
            "--validator",
            &alice_literal(),
            "--peer-id",
            &valid_peer_id_literal(),
        ])
        .expect("rebind command should parse");
        let mut context = TestContext::new();

        command.run(&mut context).expect("rebind should succeed");

        assert_eq!(
            context.submitted.as_ref().map(Vec::len),
            Some(1),
            "rebind should submit exactly one instruction"
        );
    }

    #[test]
    fn register_rejects_invalid_peer_id_during_run() {
        let args = RegisterArgs {
            lane_id: 1,
            validator: alice_literal(),
            peer_id: "not-a-peer-id".to_owned(),
            stake_account: None,
            initial_stake: 10,
            metadata: None,
        };
        let mut context = TestContext::new();
        let err = args
            .run(&mut context)
            .expect_err("invalid peer id should fail");

        assert!(
            err.to_string()
                .contains("--peer-id must be a valid peer id")
        );
        assert!(context.submitted.is_none());
    }

    #[test]
    fn rebind_rejects_invalid_peer_id_during_run() {
        let args = RebindArgs {
            lane_id: 1,
            validator: alice_literal(),
            peer_id: "not-a-peer-id".to_owned(),
        };
        let mut context = TestContext::new();
        let err = args
            .run(&mut context)
            .expect_err("invalid peer id should fail");

        assert!(
            err.to_string()
                .contains("--peer-id must be a valid peer id")
        );
        assert!(context.submitted.is_none());
    }
}
