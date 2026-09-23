//! Read-only preparation of canonical monetary input files for staking commands.

use super::*;
use iroha::data_model::asset::AssetId;
use iroha::data_model::nexus::{
    PublicLanePreparationOperationV1, PublicLanePreparationRequestV1, PublicLanePrepareBondV1,
    PublicLanePrepareClaimV1, PublicLanePrepareRegistrationV1, PublicLanePrepareUnbondV1,
    PublicLanePreparedPlanV1,
};
use iroha_primitives::numeric::Quantity;
use std::io::Write;

/// Prepare a canonical plan file without signing or submitting a transaction.
#[derive(clap::Args, Debug)]
pub struct Args {
    /// Public lane whose retained state should be observed
    #[arg(long)]
    lane_id: u32,
    /// Explicit expiry offset from the observed height, at most one committed epoch
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    valid_for_blocks: u64,
    /// New canonical JSON plan file; existing files are never overwritten
    #[arg(long, value_name = "PATH")]
    out: PathBuf,
    #[command(subcommand)]
    operation: Operation,
}

#[derive(clap::Subcommand, Debug)]
enum Operation {
    /// Prepare an exact initial self-bond for registration or candidate admission
    Registration {
        /// Validator and self-stake account
        #[arg(long)]
        validator: String,
        /// Exact BLS consensus peer identity
        #[arg(long)]
        peer_id: PeerId,
        /// Explicit amount to self-bond
        #[arg(long)]
        amount: Quantity,
        /// Include fresh-peer activation lead for register-candidate
        #[arg(long)]
        candidate: bool,
    },
    /// Prepare additional self stake or delegation
    Bond {
        /// Registered validator
        #[arg(long)]
        validator: String,
        /// Depositing account; defaults to the configured authority
        #[arg(long)]
        staker: Option<String>,
        /// Explicit deposit quantity
        #[arg(long)]
        amount: Quantity,
    },
    /// Prepare the exact amount and original custody of a retained withdrawal
    FinalizeUnbond {
        /// Registered validator
        #[arg(long)]
        validator: String,
        /// Stake owner; defaults to the configured authority
        #[arg(long)]
        staker: Option<String>,
        /// Exact retained withdrawal request hash
        #[arg(long)]
        request_id: Hash,
    },
    /// Prepare a consecutive reward prefix and selected unpaid accrual sources
    ClaimRewards {
        /// Reward recipient; defaults to the configured authority
        #[arg(long)]
        account: Option<String>,
        /// Inclusive upper epoch cut; omission uses the latest available prefix
        #[arg(long)]
        upto_epoch: Option<u64>,
        /// Maximum consecutive records; zero selects only existing accruals
        #[arg(long, default_value_t = 64, value_parser = clap::value_parser!(u16).range(0..=64))]
        max_records: u16,
        /// Additional exact custody source to revisit; repeat for retained accruals
        #[arg(long, value_name = "ASSET_ID")]
        accrued_source: Vec<AssetId>,
    },
}

impl Run for Args {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let operation = match self.operation {
            Operation::Registration {
                validator,
                peer_id,
                amount,
                candidate,
            } => PublicLanePreparationOperationV1::Registration(PublicLanePrepareRegistrationV1 {
                validator: parse_account_id(context, &validator, "--validator")?,
                peer_id,
                amount,
                candidate,
            }),
            Operation::Bond {
                validator,
                staker,
                amount,
            } => PublicLanePreparationOperationV1::Bond(PublicLanePrepareBondV1 {
                validator: parse_account_id(context, &validator, "--validator")?,
                staker: parse_account_or_authority(context, staker.as_deref(), "--staker")?,
                amount,
            }),
            Operation::FinalizeUnbond {
                validator,
                staker,
                request_id,
            } => PublicLanePreparationOperationV1::FinalizeUnbond(PublicLanePrepareUnbondV1 {
                validator: parse_account_id(context, &validator, "--validator")?,
                staker: parse_account_or_authority(context, staker.as_deref(), "--staker")?,
                request_id,
            }),
            Operation::ClaimRewards {
                account,
                upto_epoch,
                max_records,
                mut accrued_source,
            } => {
                accrued_source.sort();
                eyre::ensure!(
                    accrued_source.len() <= 64
                        && accrued_source.windows(2).all(|pair| pair[0] < pair[1]),
                    "--accrued-source requires at most 64 distinct sources"
                );
                PublicLanePreparationOperationV1::ClaimRewards(PublicLanePrepareClaimV1 {
                    recipient: parse_account_or_authority(
                        context,
                        account.as_deref(),
                        "--account",
                    )?,
                    upto_epoch,
                    max_records,
                    accrued_sources: accrued_source,
                })
            }
        };
        let request = PublicLanePreparationRequestV1 {
            lane_id: LaneId::new(self.lane_id),
            valid_for_blocks: self.valid_for_blocks,
            operation,
        };
        let prepared = context
            .client_from_config()?
            .prepare_public_lane_plan(&request)?;
        eprintln!(
            "Observed staking preparation (server observation, not an independently verified state proof): {}",
            norito::json::to_json(&prepared)?
        );
        eprintln!(
            "Execution still checks exact legs, expiry, permissions, lifecycle and withdrawal maturity. Election calculations assume inclusion at height {}.",
            prepared.assumed_execution_height
        );
        write_plan(&self.out, &prepared.plan)?;
        context.println(format!(
            "Prepared canonical signing input: {}",
            self.out.display()
        ))
    }
}

fn write_plan(path: &std::path::Path, plan: &PublicLanePreparedPlanV1) -> Result<()> {
    let json = match plan {
        PublicLanePreparedPlanV1::Monetary(plan) => norito::json::to_json(plan)?,
        PublicLanePreparedPlanV1::Claim(plan) => norito::json::to_json(plan)?,
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .wrap_err_with(|| format!("cannot create new plan file {}", path.display()))?;
    file.write_all(json.as_bytes())
        .wrap_err("cannot write staking plan")?;
    file.write_all(b"\n")
        .wrap_err("cannot finish staking plan")?;
    file.sync_all().wrap_err("cannot persist staking plan")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        args: Args,
    }

    #[test]
    fn preparation_requires_explicit_expiry_and_bounds_claim_work() {
        let args = [
            "prepare",
            "--lane-id",
            "0",
            "--out",
            "claim.json",
            "--valid-for-blocks",
            "10",
            "claim-rewards",
            "--max-records",
            "64",
        ];
        assert!(Cli::try_parse_from(args).is_ok());
        let mut excessive = args;
        excessive[9] = "65";
        assert!(Cli::try_parse_from(excessive).is_err());
        let mut no_lifetime = args;
        no_lifetime[6] = "0";
        assert!(Cli::try_parse_from(no_lifetime).is_err());
    }

    #[test]
    fn prepared_file_is_exact_canonical_claim_and_never_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("claim.json");
        let plan = PublicLaneRewardClaimPlanV1 {
            network_scope: PublicLaneMonetaryScopeV1::Network(NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"preparation")),
            )),
            valid_until_height: 10,
            expected_state: None,
            records: vec![],
            sources: vec![],
        };
        let prepared = PublicLanePreparedPlanV1::Claim(plan.clone());
        write_plan(&path, &prepared).unwrap();
        assert_eq!(
            norito::json::from_slice::<PublicLaneRewardClaimPlanV1>(&std::fs::read(&path).unwrap())
                .unwrap(),
            plan
        );
        assert!(write_plan(&path, &prepared).is_err());
    }
}
