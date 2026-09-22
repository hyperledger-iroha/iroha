//! `NPoS` staking lifecycle helpers for public lanes.
use crate::{Run, RunContext};
use eyre::{Result, WrapErr, eyre};
use iroha::data_model::{
    id::NetworkId,
    isi::InstructionBox,
    isi::staking::{
        ActivatePublicLaneValidator, BondPublicLaneStake, ClaimPublicLaneRewards,
        ExitPublicLaneValidator, FinalizePublicLaneUnbond, PublicLaneCandidateAuthorization,
        PublicLanePeerBindingAuthorization, RebindPublicLaneValidatorPeer, RecordPublicLaneRewards,
        RegisterPublicLaneCandidate, RegisterPublicLaneValidator, SchedulePublicLaneUnbond,
    },
    nexus::{
        PublicLaneMonetaryPlanV1, PublicLaneMonetaryPreconditionV1, PublicLaneMonetaryScopeV1,
        PublicLaneRewardClaimPlanV1,
    },
    prelude::AccountId,
};
use iroha_crypto::{Hash, SignatureOf};
use iroha_model_base::metadata::Metadata;
use iroha_model_base::peer::PeerId;
use iroha_model_base::topology::LaneId;
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Register a stake-elected validator on a public lane
    Register(RegisterArgs),
    /// Admit a consented validator on a stake-elected lane; fresh global admission requires an epoch key transition
    RegisterCandidate(RegisterCandidateArgs),
    /// Rebind an existing validator to a replacement consensus peer
    Rebind(RebindArgs),
    /// Activate a pending validator once its scheduled activation height is reached
    Activate(ActivateArgs),
    /// Schedule or finalize a validator exit
    Exit(ExitArgs),
    /// Bond additional self stake or delegate stake to a registered validator
    Bond(BondArgs),
    /// Schedule a stake withdrawal after the configured unbonding delay
    ScheduleUnbond(ScheduleUnbondArgs),
    /// Withdraw a scheduled unbond once its time and liability bounds have passed
    FinalizeUnbond(FinalizeUnbondArgs),
    /// Process exact reward records and claim explicitly authorized custody payouts
    ClaimRewards(ClaimRewardsArgs),
    /// Record a fee-funded epoch distribution as the configured fee-sink authority
    RecordRewards(RecordRewardsArgs),
}
impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Register(args) => args.run(context),
            Command::RegisterCandidate(args) => args.run(context),
            Command::Rebind(args) => args.run(context),
            Command::Activate(args) => args.run(context),
            Command::Exit(args) => args.run(context),
            Command::Bond(args) => args.run(context),
            Command::ScheduleUnbond(args) => args.run(context),
            Command::FinalizeUnbond(args) => args.run(context),
            Command::ClaimRewards(args) => args.run(context),
            Command::RecordRewards(args) => args.run(context),
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
    /// Exact initial self-bond quantity.
    #[arg(long, value_name = "QUANTITY")]
    pub initial_stake: iroha_primitives::numeric::Quantity,
    /// Optional metadata JSON (Norito JSON object)
    #[arg(long, value_name = "PATH")]
    pub metadata: Option<PathBuf>,
    /// Canonical Norito JSON monetary plan binding network, assets, quantity, and observed state
    #[arg(long, value_name = "PATH")]
    pub monetary_plan: PathBuf,
}
impl Run for RegisterArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let instruction = self.into_instruction(context)?;
        context.finish(vec![InstructionBox::from(instruction)])
    }
}
impl RegisterArgs {
    fn into_instruction<C: RunContext>(self, context: &C) -> Result<RegisterPublicLaneValidator> {
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
        let monetary_plan = load_monetary_plan(context, &self.monetary_plan)?;
        eyre::ensure!(
            validator == stake_account,
            "initial stake must belong to --validator"
        );
        eyre::ensure!(
            monetary_plan.source_asset.account() == &stake_account
                && monetary_plan.amount == self.initial_stake,
            "--monetary-plan must bind --stake-account and --initial-stake",
        );
        eyre::ensure!(
            matches!(
                monetary_plan.precondition,
                PublicLaneMonetaryPreconditionV1::Registration(_)
            ),
            "--monetary-plan must contain a Registration precondition",
        );
        Ok(RegisterPublicLaneValidator {
            lane_id,
            validator,
            peer_id,
            stake_account,
            initial_stake: self.initial_stake,
            metadata,
            monetary_plan,
        })
    }
}
#[derive(clap::Args, Debug)]
pub struct RegisterCandidateArgs {
    /// Validator registration and initial self-bond
    #[command(flatten)]
    pub registration: RegisterArgs,
    /// Canonical network id derived from the target network's genesis header
    #[arg(long, value_name = "NETWORK_ID")]
    pub network_id: NetworkId,
    /// Exact future election boundary authorized by this candidate peer
    #[arg(long, value_name = "HEIGHT")]
    pub activation_height: u64,
    /// Absolute path to an owner-only mode-0600 BLS-normal peer private-key file
    #[arg(long, value_name = "PATH")]
    pub peer_private_key_file: PathBuf,
}
impl Run for RegisterCandidateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let registration = self.registration.into_instruction(context)?;
        eyre::ensure!(
            self.network_id == context.config().network_id,
            "--network-id must match the configured network"
        );
        eyre::ensure!(
            matches!(
                registration.monetary_plan.precondition,
                PublicLaneMonetaryPreconditionV1::Registration(ref value)
                    if value.activation_height == self.activation_height
            ),
            "--monetary-plan activation height must match --activation-height",
        );
        let key_pair = crate::operator_key::load_operator_key_pair(&self.peer_private_key_file)
            .wrap_err("failed to load --peer-private-key-file")?;
        eyre::ensure!(
            registration.peer_id.public_key() == key_pair.public_key(),
            "--peer-id does not match --peer-private-key-file"
        );
        let proof_of_possession = iroha_crypto::bls_normal_pop_prove(key_pair.private_key())
            .wrap_err("--peer-private-key-file must contain a BLS-normal consensus key")?;
        let authorization = PublicLaneCandidateAuthorization::new(
            self.network_id,
            registration.clone(),
            self.activation_height,
        );
        let peer_signature = SignatureOf::try_new(key_pair.private_key(), &authorization)
            .wrap_err("failed to sign candidate registration with the peer key")?;
        let instruction: InstructionBox = RegisterPublicLaneCandidate {
            registration,
            activation_height: self.activation_height,
            proof_of_possession,
            peer_signature,
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
    /// Genesis-derived network id for replacement-peer consent
    #[arg(long, value_name = "NETWORK_ID", requires_all = ["peer_private_key_file", "activation_height", "previous_peer_id"])]
    pub network_id: Option<NetworkId>,
    /// Absolute owner-only mode-0600 replacement peer key file; required with --network-id
    #[arg(long, value_name = "PATH", requires = "network_id")]
    pub peer_private_key_file: Option<PathBuf>,
    /// Stored pending validator activation height bound to the replacement consent
    #[arg(long, value_name = "HEIGHT", requires = "network_id")]
    pub activation_height: Option<u64>,
    /// Stored current peer binding being replaced
    #[arg(long, value_name = "PEER_ID", requires = "network_id")]
    pub previous_peer_id: Option<PeerId>,
}
impl Run for RebindArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let lane_id = LaneId::new(self.lane_id);
        let validator = parse_account_id(context, &self.validator, "--validator")?;
        let peer_id = self
            .peer_id
            .parse::<PeerId>()
            .wrap_err("--peer-id must be a valid peer id")?;
        let mut instruction =
            RebindPublicLaneValidatorPeer::new(lane_id, validator.clone(), peer_id.clone());
        match (
            self.network_id,
            self.peer_private_key_file,
            self.activation_height,
            self.previous_peer_id,
        ) {
            (Some(network_id), Some(path), Some(activation_height), Some(previous_peer_id)) => {
                let key_pair = crate::operator_key::load_operator_key_pair(&path)
                    .wrap_err("failed to load --peer-private-key-file")?;
                eyre::ensure!(
                    peer_id.public_key() == key_pair.public_key(),
                    "--peer-id does not match --peer-private-key-file"
                );
                let authorization = PublicLanePeerBindingAuthorization::new(
                    network_id,
                    lane_id,
                    validator,
                    peer_id,
                    activation_height,
                    previous_peer_id,
                );
                let signature = SignatureOf::try_new(key_pair.private_key(), &authorization)
                    .wrap_err("failed to sign validator binding with the replacement peer key")?;
                instruction = instruction.with_peer_signature(signature);
            }
            (None, None, None, None) => {}
            _ => eyre::bail!(
                "peer consent requires --network-id, --peer-private-key-file, --activation-height, and --previous-peer-id together"
            ),
        }
        context.finish(vec![InstructionBox::from(instruction)])
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
#[derive(clap::Args, Debug)]
pub struct BondArgs {
    /// Lane id containing the validator
    #[arg(long)]
    pub lane_id: u32,
    /// Target validator account identifier (canonical I105 account literal)
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub validator: String,
    /// Account supplying stake (defaults to the configured transaction authority)
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub staker: Option<String>,
    /// Exact positive stake quantity to lock
    #[arg(long, value_name = "QUANTITY")]
    pub amount: iroha_primitives::numeric::Quantity,
    /// Optional stake-share metadata JSON (Norito JSON object)
    #[arg(long, value_name = "PATH")]
    pub metadata: Option<PathBuf>,
    /// Canonical Norito JSON monetary plan binding network, assets, quantity, and observed state
    #[arg(long, value_name = "PATH")]
    pub monetary_plan: PathBuf,
}
impl Run for BondArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        eyre::ensure!(!self.amount.is_zero(), "--amount must be positive");
        let validator = parse_account_id(context, &self.validator, "--validator")?;
        let staker = parse_account_or_authority(context, self.staker.as_deref(), "--staker")?;
        let metadata = load_metadata(self.metadata.as_ref())?;
        let monetary_plan = load_monetary_plan(context, &self.monetary_plan)?;
        eyre::ensure!(
            monetary_plan.source_asset.account() == &staker && monetary_plan.amount == self.amount,
            "--monetary-plan must bind --staker and --amount",
        );
        eyre::ensure!(
            matches!(
                monetary_plan.precondition,
                PublicLaneMonetaryPreconditionV1::Bond(_)
            ),
            "--monetary-plan must contain a Bond precondition",
        );
        let instruction: InstructionBox = BondPublicLaneStake {
            lane_id: LaneId::new(self.lane_id),
            validator,
            staker,
            amount: self.amount,
            metadata,
            monetary_plan,
        }
        .into();
        context.finish(vec![instruction])
    }
}
#[derive(clap::Args, Debug)]
pub struct ScheduleUnbondArgs {
    /// Lane id containing the validator
    #[arg(long)]
    pub lane_id: u32,
    /// Validator whose stake position is being withdrawn
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub validator: String,
    /// Stake owner (defaults to the configured transaction authority)
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub staker: Option<String>,
    /// Unique withdrawal hash; retain it for finalize-unbond
    #[arg(long, value_name = "HASH")]
    pub request_id: Hash,
    /// Exact positive stake quantity to unlock
    #[arg(long, value_name = "QUANTITY")]
    pub amount: iroha_primitives::numeric::Quantity,
    /// Unix timestamp in milliseconds respecting the configured unbonding delay
    #[arg(long, value_name = "MILLIS")]
    pub release_at_ms: u64,
}
impl Run for ScheduleUnbondArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        eyre::ensure!(!self.amount.is_zero(), "--amount must be positive");
        let validator = parse_account_id(context, &self.validator, "--validator")?;
        let staker = parse_account_or_authority(context, self.staker.as_deref(), "--staker")?;
        let instruction: InstructionBox = SchedulePublicLaneUnbond {
            lane_id: LaneId::new(self.lane_id),
            validator,
            staker,
            request_id: self.request_id,
            amount: self.amount,
            release_at_ms: self.release_at_ms,
        }
        .into();
        context.finish(vec![instruction])
    }
}
#[derive(clap::Args, Debug)]
pub struct FinalizeUnbondArgs {
    /// Lane id containing the validator
    #[arg(long)]
    pub lane_id: u32,
    /// Validator whose stake position is being withdrawn
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub validator: String,
    /// Stake owner receiving the withdrawal (defaults to the configured authority)
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub staker: Option<String>,
    /// Exact withdrawal hash previously supplied to schedule-unbond
    #[arg(long, value_name = "HASH")]
    pub request_id: Hash,
    /// Canonical Norito JSON monetary plan binding network, assets, quantity, and observed state
    #[arg(long, value_name = "PATH")]
    pub monetary_plan: PathBuf,
}
impl Run for FinalizeUnbondArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let validator = parse_account_id(context, &self.validator, "--validator")?;
        let staker = parse_account_or_authority(context, self.staker.as_deref(), "--staker")?;
        let monetary_plan = load_monetary_plan(context, &self.monetary_plan)?;
        eyre::ensure!(
            monetary_plan.destination_asset.account() == &staker,
            "--monetary-plan destination must belong to --staker",
        );
        eyre::ensure!(
            matches!(
                monetary_plan.precondition,
                PublicLaneMonetaryPreconditionV1::Unbond(_)
            ),
            "--monetary-plan must contain an Unbond precondition",
        );
        let instruction: InstructionBox = FinalizePublicLaneUnbond {
            lane_id: LaneId::new(self.lane_id),
            validator,
            staker,
            request_id: self.request_id,
            monetary_plan,
        }
        .into();
        context.finish(vec![instruction])
    }
}
#[derive(clap::Args, Debug)]
pub struct ClaimRewardsArgs {
    /// Lane id whose rewards are being claimed
    #[arg(long)]
    pub lane_id: u32,
    /// Reward recipient (defaults to the configured transaction authority)
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub account: Option<String>,
    /// Canonical Norito JSON claim plan with exact retained cursor, records, and custody payouts
    #[arg(long, value_name = "PATH")]
    pub claim_plan: PathBuf,
}
impl Run for ClaimRewardsArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let account = parse_account_or_authority(context, self.account.as_deref(), "--account")?;
        let claim_plan: PublicLaneRewardClaimPlanV1 = load_plan(&self.claim_plan, "--claim-plan")?;
        eyre::ensure!(
            claim_plan.network_scope
                == PublicLaneMonetaryScopeV1::Network(context.config().network_id),
            "--claim-plan must bind the configured network",
        );
        eyre::ensure!(
            claim_plan.has_canonical_shape(&account),
            "--claim-plan must contain bounded ordered records and exact recipient payouts"
        );
        let instruction: InstructionBox = ClaimPublicLaneRewards {
            lane_id: LaneId::new(self.lane_id),
            account,
            claim_plan,
        }
        .into();
        context.finish(vec![instruction])
    }
}
#[derive(clap::Args, Debug)]
pub struct RecordRewardsArgs {
    /// Norito JSON RecordPublicLaneRewards object with exact per-account allocations
    #[arg(long, value_name = "PATH")]
    pub file: PathBuf,
}
impl Run for RecordRewardsArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let raw = fs::read_to_string(&self.file).wrap_err_with(|| {
            format!(
                "failed to read reward distribution from {}",
                self.file.display()
            )
        })?;
        let instruction: RecordPublicLaneRewards = norito::json::from_str(&raw)
            .wrap_err("--file must contain a valid Norito JSON RecordPublicLaneRewards object")?;
        context.finish(vec![InstructionBox::from(instruction)])
    }
}
fn load_plan<T: norito::json::JsonDeserialize>(path: &Path, flag: &str) -> Result<T> {
    let json = crate::read_cli_text_file_bounded(path, flag)?;
    crate::parse_json(&json)
        .wrap_err_with(|| format!("{flag} must contain a canonical Norito JSON plan"))
}
fn load_monetary_plan<C: RunContext>(context: &C, path: &Path) -> Result<PublicLaneMonetaryPlanV1> {
    let plan: PublicLaneMonetaryPlanV1 = load_plan(path, "--monetary-plan")?;
    eyre::ensure!(
        plan.network_scope == PublicLaneMonetaryScopeV1::Network(context.config().network_id),
        "--monetary-plan must bind the configured network",
    );
    eyre::ensure!(
        plan.has_canonical_shape(),
        "--monetary-plan must have positive amount and heights, matching asset scope, and exact preconditions"
    );
    Ok(plan)
}
fn parse_account_or_authority<C: RunContext>(
    context: &C,
    value: Option<&str>,
    flag: &str,
) -> Result<AccountId> {
    value.map_or_else(
        || Ok(context.config().account.clone()),
        |value| parse_account_id(context, value, flag),
    )
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
    use iroha::data_model::{
        asset::{AssetDefinitionId, AssetId},
        nexus::{
            PublicLaneMonetaryBondV1, PublicLaneMonetaryRegistrationV1, PublicLaneMonetaryUnbondV1,
            PublicLaneRewardClaimSourceV1, PublicLaneRewardClaimStateV1,
            PublicLaneRewardRecordRefV1,
        },
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_i18n::Language;
    use iroha_test_samples::{ALICE_ID, BOB_ID, CARPENTER_ID};
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
                crate::Executable::ContractCall(_)
                | crate::Executable::Ivm(_)
                | crate::Executable::IvmProved(_)
                | crate::Executable::Batch(_) => {
                    eyre::bail!("unexpected non-instruction executable in staking test context")
                }
            }
        }
    }
    fn fixture_plan(
        owner: &AccountId,
        amount: &str,
        precondition: PublicLaneMonetaryPreconditionV1,
    ) -> PublicLaneMonetaryPlanV1 {
        let definition = AssetDefinitionId::from_uuid_bytes([
            1, 2, 3, 4, 5, 6, 0x47, 8, 0x89, 10, 11, 12, 13, 14, 15, 16,
        ])
        .expect("canonical fixture definition");
        let owner_asset = AssetId::new(definition.clone(), owner.clone());
        let custody_asset = AssetId::new(definition, CARPENTER_ID.clone());
        let (source_asset, destination_asset) =
            if matches!(precondition, PublicLaneMonetaryPreconditionV1::Unbond(_)) {
                (custody_asset, owner_asset)
            } else {
                (owner_asset, custody_asset)
            };
        PublicLaneMonetaryPlanV1 {
            network_scope: PublicLaneMonetaryScopeV1::Network(TestContext::new().cfg.network_id),
            valid_until_height: 4000,
            source_asset,
            destination_asset,
            amount: amount.parse().expect("exact fixture quantity"),
            precondition,
        }
    }
    fn registration_plan(amount: &str) -> PublicLaneMonetaryPlanV1 {
        fixture_plan(
            &ALICE_ID,
            amount,
            PublicLaneMonetaryPreconditionV1::Registration(PublicLaneMonetaryRegistrationV1 {
                activation_height: 3601,
            }),
        )
    }
    fn bond_plan(owner: &AccountId, amount: &str) -> PublicLaneMonetaryPlanV1 {
        fixture_plan(
            owner,
            amount,
            PublicLaneMonetaryPreconditionV1::Bond(PublicLaneMonetaryBondV1 {
                activation_height: 1,
                peer_id: valid_peer_id_literal().parse().expect("peer"),
            }),
        )
    }
    fn unbond_plan(owner: &AccountId) -> PublicLaneMonetaryPlanV1 {
        fixture_plan(
            owner,
            "0.000000001",
            PublicLaneMonetaryPreconditionV1::Unbond(PublicLaneMonetaryUnbondV1 {
                activation_height: 1,
                request_hash: Hash::new(b"retained exact fixture withdrawal record"),
            }),
        )
    }
    fn claim_plan(owner: &AccountId, epoch: u64) -> PublicLaneRewardClaimPlanV1 {
        let transfer = unbond_plan(owner);
        PublicLaneRewardClaimPlanV1 {
            network_scope: transfer.network_scope,
            valid_until_height: transfer.valid_until_height,
            expected_state: epoch.checked_sub(1).map(|through_epoch| {
                PublicLaneRewardClaimStateV1 {
                    through_epoch: Some(through_epoch),
                }
            }),
            records: vec![PublicLaneRewardRecordRefV1 {
                epoch,
                record_hash: Hash::new(b"immutable fixture reward record"),
            }],
            sources: vec![PublicLaneRewardClaimSourceV1 {
                source_asset: transfer.source_asset,
                destination_asset: transfer.destination_asset,
                expected_accrued: Some(transfer.amount.clone()),
                payout: transfer.amount,
            }],
        }
    }
    fn write_plan<T: JsonSerialize>(plan: &T) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().expect("plan file");
        fs::write(
            file.path(),
            norito::json::to_json(plan).expect("canonical plan JSON"),
        )
        .expect("write plan");
        file
    }
    fn parse_command(args: &[&str]) -> clap::error::Result<Command> {
        Wrapper::try_parse_from(args).map(|wrapper| wrapper.command)
    }
    fn alice_literal() -> String {
        ALICE_ID.canonical_i105().expect("canonical I105")
    }
    fn checked_staking_key_fixture() -> KeyPair {
        KeyPair::try_random().expect("generate checked staking fixture key")
    }
    #[test]
    fn staking_fixture_uses_checked_default_key_generation() {
        let key_pair = checked_staking_key_fixture();
        let actual = key_pair
            .public_key()
            .try_algorithm()
            .expect("staking fixture key advertises a valid algorithm");
        assert_eq!(actual, Algorithm::default());
    }
    fn valid_peer_id_literal() -> String {
        PeerId::from(checked_staking_key_fixture().public_key().clone()).to_string()
    }
    #[cfg(unix)]
    #[test]
    fn register_candidate_signs_exact_registration_and_network() {
        let key_pair = KeyPair::try_from_seed(vec![0x93; 32], Algorithm::BlsNormal)
            .expect("candidate BLS key");
        let key_file = tempfile::NamedTempFile::new().expect("private key file");
        fs::write(
            key_file.path(),
            iroha_crypto::ExposedPrivateKey(key_pair.private_key().clone()).to_string(),
        )
        .expect("write runtime key");
        let peer_id = PeerId::new(key_pair.public_key().clone());
        let network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"staking-cli-network")),
        );
        let mut monetary_plan = registration_plan("25000.000000001");
        monetary_plan.network_scope = PublicLaneMonetaryScopeV1::Network(network_id);
        let plan_file = write_plan(&monetary_plan);
        let command = parse_command(&[
            "register-candidate",
            "--monetary-plan",
            plan_file.path().to_str().expect("plan path"),
            "--lane-id",
            "0",
            "--validator",
            &alice_literal(),
            "--peer-id",
            &peer_id.to_string(),
            "--initial-stake",
            "25000.000000001",
            "--activation-height",
            "3601",
            "--network-id",
            &network_id.to_string(),
            "--peer-private-key-file",
            key_file.path().to_str().expect("key path"),
        ])
        .expect("candidate command should parse");
        let mut context = TestContext::new();
        context.cfg.network_id = network_id;
        command.run(&mut context).expect("candidate should succeed");
        let submitted = context.submitted.expect("submitted candidate");
        assert_eq!(submitted.len(), 1);
        let instruction = submitted[0]
            .as_any()
            .downcast_ref::<RegisterPublicLaneCandidate>()
            .expect("candidate instruction");
        let expected_registration = RegisterPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: ALICE_ID.clone(),
            peer_id,
            stake_account: ALICE_ID.clone(),
            initial_stake: "25000.000000001".parse().expect("exact amount"),
            metadata: Metadata::default(),
            monetary_plan,
        };
        assert_eq!(instruction.registration, expected_registration);
        assert_eq!(instruction.activation_height, 3601);
        iroha_crypto::bls_normal_pop_verify(
            key_pair.public_key(),
            &instruction.proof_of_possession,
        )
        .expect("valid peer proof of possession");
        instruction
            .peer_signature
            .verify(
                key_pair.public_key(),
                &PublicLaneCandidateAuthorization::new(
                    network_id,
                    expected_registration.clone(),
                    3601,
                ),
            )
            .expect("signature binds the entire registration and network");
        let other_network = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"other-staking-network")),
        );
        assert!(
            instruction
                .peer_signature
                .verify(
                    key_pair.public_key(),
                    &PublicLaneCandidateAuthorization::new(
                        other_network,
                        expected_registration,
                        3601
                    ),
                )
                .is_err()
        );
    }
    #[cfg(unix)]
    #[test]
    fn register_candidate_rejects_wrong_peer_or_non_consensus_key() {
        let key_pair = checked_staking_key_fixture();
        let key_file = tempfile::NamedTempFile::new().expect("private key file");
        fs::write(
            key_file.path(),
            iroha_crypto::ExposedPrivateKey(key_pair.private_key().clone()).to_string(),
        )
        .expect("write runtime key");
        let context_network = TestContext::new().cfg.network_id;
        let monetary_plan = registration_plan("1000");
        let plan_file = write_plan(&monetary_plan);
        for (peer_id, expected_error) in [
            (
                valid_peer_id_literal(),
                "--peer-id does not match --peer-private-key-file",
            ),
            (
                PeerId::new(key_pair.public_key().clone()).to_string(),
                "--peer-private-key-file must contain a BLS-normal consensus key",
            ),
        ] {
            let args = RegisterCandidateArgs {
                registration: RegisterArgs {
                    lane_id: 0,
                    validator: alice_literal(),
                    peer_id,
                    stake_account: None,
                    initial_stake: 1_000_u64.into(),
                    metadata: None,
                    monetary_plan: plan_file.path().to_path_buf(),
                },
                network_id: context_network,
                activation_height: 3601,
                peer_private_key_file: key_file.path().to_path_buf(),
            };
            let mut context = TestContext::new();
            let error = args.run(&mut context).expect_err("wrong key must fail");
            assert!(error.to_string().contains(expected_error));
            assert!(context.submitted.is_none());
        }
    }
    #[test]
    fn register_candidate_requires_network_and_private_key_file() {
        let error = parse_command(&[
            "register-candidate",
            "--lane-id",
            "0",
            "--validator",
            &alice_literal(),
            "--peer-id",
            &valid_peer_id_literal(),
            "--initial-stake",
            "25000",
        ])
        .expect_err("candidate signing inputs are required");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
        assert!(error.to_string().contains("--network-id"));
        assert!(error.to_string().contains("--peer-private-key-file"));
        assert!(error.to_string().contains("--activation-height"));
    }
    #[test]
    fn bond_defaults_to_signer_and_preserves_exact_quantity() {
        let monetary_plan = bond_plan(
            &TestContext::new().cfg.account,
            "9007199254740993.000000001",
        );
        let plan_file = write_plan(&monetary_plan);
        let command = parse_command(&[
            "bond",
            "--monetary-plan",
            plan_file.path().to_str().expect("plan path"),
            "--lane-id",
            "0",
            "--validator",
            &alice_literal(),
            "--amount",
            "9007199254740993.000000001",
        ])
        .expect("bond command should parse");
        let mut context = TestContext::new();
        let expected: InstructionBox = BondPublicLaneStake {
            lane_id: LaneId::SINGLE,
            validator: ALICE_ID.clone(),
            staker: context.cfg.account.clone(),
            amount: "9007199254740993.000000001".parse().expect("exact amount"),
            metadata: Metadata::default(),
            monetary_plan,
        }
        .into();
        command.run(&mut context).expect("bond should succeed");
        assert_eq!(context.submitted, Some(vec![expected]));
    }
    #[test]
    fn bond_preserves_explicit_staker_and_metadata() {
        let delegator = BOB_ID.canonical_i105().expect("canonical delegator I105");
        let metadata_file = tempfile::NamedTempFile::new().expect("metadata file");
        fs::write(metadata_file.path(), r#"{"purpose":"delegation"}"#).expect("write metadata");
        let monetary_plan = bond_plan(&BOB_ID, "0.125");
        let plan_file = write_plan(&monetary_plan);
        let command = parse_command(&[
            "bond",
            "--monetary-plan",
            plan_file.path().to_str().expect("plan path"),
            "--lane-id",
            "7",
            "--validator",
            &alice_literal(),
            "--staker",
            &delegator,
            "--amount",
            "0.125",
            "--metadata",
            metadata_file.path().to_str().expect("metadata path"),
        ])
        .expect("delegation command should parse");
        let mut context = TestContext::new();
        let expected: InstructionBox = BondPublicLaneStake {
            lane_id: LaneId::new(7),
            validator: ALICE_ID.clone(),
            staker: BOB_ID.clone(),
            amount: "0.125".parse().expect("exact amount"),
            metadata: norito::json::from_str(r#"{"purpose":"delegation"}"#)
                .expect("expected metadata"),
            monetary_plan,
        }
        .into();
        command
            .run(&mut context)
            .expect("delegation should succeed");
        assert_eq!(context.submitted, Some(vec![expected]));
    }
    #[test]
    fn schedule_unbond_preserves_request_amount_and_release() {
        let request_id = Hash::new(b"staking-cli-unbond");
        let command = parse_command(&[
            "schedule-unbond",
            "--lane-id",
            "3",
            "--validator",
            &alice_literal(),
            "--staker",
            &alice_literal(),
            "--request-id",
            &request_id.to_string(),
            "--amount",
            "0.000000001",
            "--release-at-ms",
            "2000000000000",
        ])
        .expect("schedule-unbond command should parse");
        let mut context = TestContext::new();
        let expected: InstructionBox = SchedulePublicLaneUnbond {
            lane_id: LaneId::new(3),
            validator: ALICE_ID.clone(),
            staker: ALICE_ID.clone(),
            request_id,
            amount: "0.000000001".parse().expect("exact amount"),
            release_at_ms: 2_000_000_000_000,
        }
        .into();
        command.run(&mut context).expect("unbond should succeed");
        assert_eq!(context.submitted, Some(vec![expected]));
    }
    #[test]
    fn finalize_unbond_defaults_to_signer_and_preserves_request() {
        let request_id = Hash::new(b"staking-cli-unbond");
        let monetary_plan = unbond_plan(&TestContext::new().cfg.account);
        let plan_file = write_plan(&monetary_plan);
        let command = parse_command(&[
            "finalize-unbond",
            "--monetary-plan",
            plan_file.path().to_str().expect("plan path"),
            "--lane-id",
            "3",
            "--validator",
            &alice_literal(),
            "--request-id",
            &request_id.to_string(),
        ])
        .expect("finalize-unbond command should parse");
        let mut context = TestContext::new();
        let expected: InstructionBox = FinalizePublicLaneUnbond {
            lane_id: LaneId::new(3),
            validator: ALICE_ID.clone(),
            staker: context.cfg.account.clone(),
            request_id,
            monetary_plan,
        }
        .into();
        command.run(&mut context).expect("finalize should succeed");
        assert_eq!(context.submitted, Some(vec![expected]));
    }
    #[test]
    fn claim_rewards_defaults_to_signer_and_preserves_exact_records() {
        let claim_plan = claim_plan(&TestContext::new().cfg.account, 0);
        let plan_file = write_plan(&claim_plan);
        let command = parse_command(&[
            "claim-rewards",
            "--lane-id",
            "0",
            "--claim-plan",
            plan_file.path().to_str().expect("plan path"),
        ])
        .expect("claim-rewards command should parse");
        let mut context = TestContext::new();
        let expected: InstructionBox = ClaimPublicLaneRewards {
            lane_id: LaneId::SINGLE,
            account: context.cfg.account.clone(),
            claim_plan,
        }
        .into();
        command.run(&mut context).expect("claim should succeed");
        assert_eq!(context.submitted, Some(vec![expected]));
    }
    #[test]
    fn claim_rewards_preserves_account_cursor_and_exact_epoch_records() {
        let claim_plan = claim_plan(&ALICE_ID, 12);
        let plan_file = write_plan(&claim_plan);
        let command = parse_command(&[
            "claim-rewards",
            "--claim-plan",
            plan_file.path().to_str().expect("plan path"),
            "--lane-id",
            "4",
            "--account",
            &alice_literal(),
        ])
        .expect("bounded claim should parse");
        let mut context = TestContext::new();
        let expected: InstructionBox = ClaimPublicLaneRewards {
            lane_id: LaneId::new(4),
            account: ALICE_ID.clone(),
            claim_plan,
        }
        .into();
        command.run(&mut context).expect("claim should succeed");
        assert_eq!(context.submitted, Some(vec![expected]));
    }
    #[test]
    fn record_rewards_preserves_exact_distribution() {
        use iroha::data_model::{
            asset::{AssetDefinitionId, AssetId},
            nexus::{PublicLaneRewardRole, PublicLaneRewardShare},
        };

        let asset_definition = AssetDefinitionId::from_uuid_bytes([
            1, 2, 3, 4, 5, 6, 0x47, 8, 0x89, 10, 11, 12, 13, 14, 15, 16,
        ])
        .expect("fixture asset definition");
        let instruction = RecordPublicLaneRewards {
            lane_id: LaneId::SINGLE,
            epoch: 0,
            reward_asset: AssetId::new(asset_definition, ALICE_ID.clone()),
            total_reward: "0.000000001".parse().expect("exact reward"),
            shares: vec![PublicLaneRewardShare {
                account: ALICE_ID.clone(),
                role: PublicLaneRewardRole::Validator,
                amount: "0.000000001".parse().expect("exact reward"),
            }],
            metadata: norito::json::from_str(r#"{"allocation":"approved-epoch-0"}"#)
                .expect("reward metadata"),
        };
        let distribution = tempfile::NamedTempFile::new().expect("reward file");
        fs::write(
            distribution.path(),
            norito::json::to_json(&instruction).expect("encode distribution"),
        )
        .expect("write distribution");
        let command = parse_command(&[
            "record-rewards",
            "--file",
            distribution.path().to_str().expect("distribution path"),
        ])
        .expect("record-rewards command should parse");
        let mut context = TestContext::new();
        command
            .run(&mut context)
            .expect("distribution should submit");
        assert_eq!(context.submitted, Some(vec![instruction.into()]));
    }
    #[test]
    fn record_rewards_requires_valid_distribution_file() {
        let error = parse_command(&["record-rewards"]).expect_err("file is required");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
        assert!(error.to_string().contains("--file"));
        let distribution = tempfile::NamedTempFile::new().expect("reward file");
        fs::write(distribution.path(), r#"{"epoch":1}"#).expect("write incomplete distribution");
        let command = parse_command(&[
            "record-rewards",
            "--file",
            distribution.path().to_str().expect("distribution path"),
        ])
        .expect("command should parse");
        let mut context = TestContext::new();
        let error = command
            .run(&mut context)
            .expect_err("incomplete distribution must fail");
        assert!(error.to_string().contains("RecordPublicLaneRewards"));
        assert!(context.submitted.is_none());
    }
    #[test]
    fn stake_commands_reject_zero_before_submission() {
        for name in ["bond", "schedule-unbond"] {
            let account = alice_literal();
            let request = Hash::new(b"staking-cli-zero").to_string();
            let mut args = vec![
                name,
                "--lane-id",
                "0",
                "--validator",
                &account,
                "--amount",
                "0",
            ];
            if name == "schedule-unbond" {
                args.extend(["--request-id", &request, "--release-at-ms", "2000000000000"]);
            } else {
                args.extend(["--monetary-plan", "/not-opened-for-zero-amount.json"]);
            }
            let command = parse_command(&args).expect("zero quantity parses");
            let mut context = TestContext::new();
            let error = command.run(&mut context).expect_err("zero must fail");
            assert!(error.to_string().contains("--amount must be positive"));
            assert!(context.submitted.is_none());
        }
    }
    #[test]
    fn bond_rejects_invalid_quantities_during_parsing() {
        for amount in ["NaN", "-1", "1e3"] {
            let error = parse_command(&[
                "bond",
                "--monetary-plan",
                "/not-opened-for-invalid-quantity.json",
                "--lane-id",
                "0",
                "--validator",
                &alice_literal(),
                &format!("--amount={amount}"),
            ])
            .expect_err("invalid quantity must fail");
            assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
        }
    }
    #[test]
    fn unbond_commands_require_valid_request_hashes() {
        for name in ["schedule-unbond", "finalize-unbond"] {
            let account = alice_literal();
            let mut args = vec![name, "--lane-id", "0", "--validator", &account];
            if name == "schedule-unbond" {
                args.extend(["--amount", "1", "--release-at-ms", "2000000000000"]);
            } else {
                args.extend(["--monetary-plan", "/not-opened-for-invalid-request.json"]);
            }
            let error = parse_command(&args).expect_err("request id is mandatory");
            assert_eq!(
                error.kind(),
                clap::error::ErrorKind::MissingRequiredArgument
            );
            assert!(error.to_string().contains("--request-id"));
            args.extend(["--request-id", "not-a-hash"]);
            let error = parse_command(&args).expect_err("invalid request id must fail");
            assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
        }
    }
    #[test]
    fn parse_account_or_authority_rejects_invalid_explicit_account() {
        let context = TestContext::new();
        let error = parse_account_or_authority(&context, Some("not-an-account"), "--staker")
            .expect_err("invalid account must fail");
        assert!(
            error
                .to_string()
                .contains("invalid account id passed to --staker")
        );
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
    #[cfg(unix)]
    #[test]
    fn rebind_signs_replacement_peer_consent() {
        let key_pair = KeyPair::try_from_seed(vec![0x94; 32], Algorithm::BlsNormal)
            .expect("replacement peer key");
        let key_file = tempfile::NamedTempFile::new().expect("private key file");
        fs::write(
            key_file.path(),
            iroha_crypto::ExposedPrivateKey(key_pair.private_key().clone()).to_string(),
        )
        .expect("write runtime key");
        let peer_id = PeerId::new(key_pair.public_key().clone());
        let previous_peer_id: PeerId = valid_peer_id_literal().parse().expect("previous peer");
        let network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"staking-cli-network")),
        );
        let command = parse_command(&[
            "rebind",
            "--lane-id",
            "3",
            "--validator",
            &alice_literal(),
            "--peer-id",
            &peer_id.to_string(),
            "--activation-height",
            "3601",
            "--previous-peer-id",
            &previous_peer_id.to_string(),
            "--network-id",
            &network_id.to_string(),
            "--peer-private-key-file",
            key_file.path().to_str().expect("key path"),
        ])
        .expect("signed rebind should parse");
        let mut context = TestContext::new();
        command
            .run(&mut context)
            .expect("signed rebind should succeed");
        let submitted = context.submitted.expect("rebind submitted");
        assert_eq!(submitted.len(), 1);
        let instruction = submitted[0]
            .as_any()
            .downcast_ref::<RebindPublicLaneValidatorPeer>()
            .expect("rebind instruction");
        let expected = PublicLanePeerBindingAuthorization::new(
            network_id,
            LaneId::new(3),
            ALICE_ID.clone(),
            peer_id,
            3601,
            previous_peer_id,
        );
        instruction
            .peer_signature
            .as_ref()
            .expect("replacement peer consent")
            .verify(key_pair.public_key(), &expected)
            .expect("consent binds exact network, lane, validator, and peer");
    }
    #[test]
    fn rebind_signing_flags_must_be_paired() {
        let network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"staking-cli-network")),
        )
        .to_string();
        for (flag, value, required) in [
            (
                "--network-id",
                network_id.as_str(),
                "--peer-private-key-file",
            ),
            (
                "--peer-private-key-file",
                "/runtime/peer.key",
                "--network-id",
            ),
        ] {
            let error = parse_command(&[
                "rebind",
                "--lane-id",
                "0",
                "--validator",
                &alice_literal(),
                "--peer-id",
                &valid_peer_id_literal(),
                flag,
                value,
            ])
            .expect_err("partial peer consent input must fail");
            assert_eq!(
                error.kind(),
                clap::error::ErrorKind::MissingRequiredArgument
            );
            assert!(error.to_string().contains(required));
        }
    }
    #[test]
    fn register_submits_instruction_with_valid_peer_id() {
        let monetary_plan = registration_plan("10");
        let plan_file = write_plan(&monetary_plan);
        let command = parse_command(&[
            "register",
            "--monetary-plan",
            plan_file.path().to_str().expect("plan path"),
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
            initial_stake: 10_u64.into(),
            metadata: None,
            monetary_plan: PathBuf::from("/not-opened-for-invalid-peer.json"),
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
            network_id: None,
            peer_private_key_file: None,
            activation_height: None,
            previous_peer_id: None,
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
    fn monetary_commands_require_explicit_plans() {
        for args in [
            vec![
                "register",
                "--lane-id",
                "0",
                "--validator",
                "placeholder",
                "--peer-id",
                "placeholder",
                "--initial-stake",
                "1",
            ],
            vec![
                "bond",
                "--lane-id",
                "0",
                "--validator",
                "placeholder",
                "--amount",
                "1",
            ],
            vec![
                "finalize-unbond",
                "--lane-id",
                "0",
                "--validator",
                "placeholder",
            ],
        ] {
            let error = parse_command(&args).expect_err("monetary plan is required");
            assert_eq!(
                error.kind(),
                clap::error::ErrorKind::MissingRequiredArgument
            );
            assert!(error.to_string().contains("--monetary-plan"));
        }
        let error = parse_command(&["claim-rewards", "--lane-id", "0"])
            .expect_err("claim plan is required");
        assert!(error.to_string().contains("--claim-plan"));
        let error = parse_command(&[
            "claim-rewards",
            "--lane-id",
            "0",
            "--claim-plan",
            "plan.json",
            "--upto-epoch",
            "12",
        ])
        .expect_err("retired bound cannot replace exact record authorization");
        assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn bond_rejects_substituted_plan_before_submission() {
        let context = TestContext::new();
        let baseline = bond_plan(&context.cfg.account, "10");
        let mut alternatives = vec![baseline.clone(); 8];
        alternatives[0].network_scope = PublicLaneMonetaryScopeV1::Genesis;
        alternatives[1].network_scope =
            PublicLaneMonetaryScopeV1::Network(NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"other monetary network")),
            ));
        alternatives[2].source_asset = baseline.destination_asset.clone();
        alternatives[3].amount = 9_u64.into();
        alternatives[4].precondition = registration_plan("10").precondition;
        alternatives[5].valid_until_height = 0;
        alternatives[6].amount = 0_u64.into();
        alternatives[7].precondition =
            PublicLaneMonetaryPreconditionV1::Bond(PublicLaneMonetaryBondV1 {
                activation_height: 0,
                peer_id: valid_peer_id_literal().parse().expect("peer"),
            });
        for plan in alternatives {
            let file = write_plan(&plan);
            let mut context = TestContext::new();
            BondArgs {
                lane_id: 0,
                validator: alice_literal(),
                staker: None,
                amount: 10_u64.into(),
                metadata: None,
                monetary_plan: file.path().to_path_buf(),
            }
            .run(&mut context)
            .expect_err("substituted plan must fail before submission");
            assert!(context.submitted.is_none());
        }
    }

    #[test]
    fn registration_rejects_foreign_stake_and_wrong_plan_operation() {
        let mut alternatives = vec![registration_plan("10"); 4];
        alternatives[0].source_asset = alternatives[0].destination_asset.clone();
        alternatives[1].amount = 9_u64.into();
        alternatives[2].precondition = bond_plan(&ALICE_ID, "10").precondition;
        alternatives[3].source_asset = AssetId::new(
            alternatives[3].source_asset.definition().clone(),
            BOB_ID.clone(),
        );
        for (index, plan) in alternatives.into_iter().enumerate() {
            let file = write_plan(&plan);
            let mut context = TestContext::new();
            RegisterArgs {
                lane_id: 0,
                validator: alice_literal(),
                peer_id: valid_peer_id_literal(),
                stake_account: (index == 3).then(|| BOB_ID.canonical_i105().expect("staker")),
                initial_stake: 10_u64.into(),
                metadata: None,
                monetary_plan: file.path().to_path_buf(),
            }
            .run(&mut context)
            .expect_err("registration must bind validator-owned stake");
            assert!(context.submitted.is_none());
        }
    }

    #[test]
    fn candidate_rejects_network_or_activation_mismatch_before_key_access() {
        let plan = registration_plan("1000");
        let file = write_plan(&plan);
        for wrong_network in [false, true] {
            let mut context = TestContext::new();
            let network = if wrong_network {
                NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                    Hash::new(b"other candidate network"),
                ))
            } else {
                context.cfg.network_id
            };
            let error = RegisterCandidateArgs {
                registration: RegisterArgs {
                    lane_id: 0,
                    validator: alice_literal(),
                    peer_id: valid_peer_id_literal(),
                    stake_account: None,
                    initial_stake: 1000_u64.into(),
                    metadata: None,
                    monetary_plan: file.path().to_path_buf(),
                },
                network_id: network,
                activation_height: if wrong_network { 3601 } else { 3602 },
                peer_private_key_file: PathBuf::from(
                    "/not-opened-for-substituted-authorization.key",
                ),
            }
            .run(&mut context)
            .expect_err("candidate context must agree with monetary consent");
            assert!(error.to_string().contains(if wrong_network {
                "--network-id must match"
            } else {
                "activation height must match"
            }));
            assert!(context.submitted.is_none());
        }
    }

    #[test]
    fn finalize_unbond_rejects_foreign_destination_and_wrong_operation() {
        let owner = TestContext::new().cfg.account;
        let mut alternatives = vec![unbond_plan(&owner); 2];
        alternatives[0].destination_asset = alternatives[0].source_asset.clone();
        alternatives[1].precondition = registration_plan("1").precondition;
        for plan in alternatives {
            let file = write_plan(&plan);
            let mut context = TestContext::new();
            FinalizeUnbondArgs {
                lane_id: 0,
                validator: alice_literal(),
                staker: None,
                request_id: Hash::new(b"fixture withdrawal"),
                monetary_plan: file.path().to_path_buf(),
            }
            .run(&mut context)
            .expect_err("withdrawal must bind its recipient and operation");
            assert!(context.submitted.is_none());
        }
    }

    #[test]
    fn claim_rejects_wrong_network_recipient_cursor_and_unbounded_records() {
        let baseline = claim_plan(&TestContext::new().cfg.account, 12);
        let mut alternatives = vec![baseline.clone(); 6];
        alternatives[0].network_scope = PublicLaneMonetaryScopeV1::Genesis;
        alternatives[1].network_scope =
            PublicLaneMonetaryScopeV1::Network(NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"other reward network")),
            ));
        alternatives[2].sources[0].destination_asset = baseline.sources[0].source_asset.clone();
        alternatives[3].expected_state = Some(PublicLaneRewardClaimStateV1 {
            through_epoch: Some(12),
        });
        alternatives[4].records = (12..77)
            .map(|epoch| PublicLaneRewardRecordRefV1 {
                epoch,
                record_hash: Hash::new(epoch.to_le_bytes()),
            })
            .collect();
        alternatives[5].records.push(baseline.records[0]);
        for plan in alternatives {
            let file = write_plan(&plan);
            let mut context = TestContext::new();
            ClaimRewardsArgs {
                lane_id: 0,
                account: None,
                claim_plan: file.path().to_path_buf(),
            }
            .run(&mut context)
            .expect_err("invalid claim must fail before submission");
            assert!(context.submitted.is_none());
        }
    }

    #[test]
    fn monetary_plan_reader_rejects_unknown_fields_and_oversized_files() {
        let plan = registration_plan("10");
        let mut json = norito::json::to_value(&plan).expect("plan JSON");
        json.as_object_mut()
            .expect("plan object")
            .insert("retired_transfer".into(), norito::json::Value::Null);
        let file = write_plan(&json);
        let context = TestContext::new();
        load_monetary_plan(&context, file.path()).expect_err("unknown plan fields are rejected");
        let oversized = tempfile::NamedTempFile::new().expect("sparse plan file");
        oversized
            .as_file()
            .set_len(crate::MAX_CLI_STDIN_BYTES_V1 as u64 + 1)
            .expect("extend sparse file");
        let error = load_monetary_plan(&context, oversized.path())
            .expect_err("oversized plan is rejected before decoding");
        assert!(format!("{error:#}").contains("exceeds"));
    }
}
