//! Social incentive helpers (viral follow rewards and escrows).

use std::{fs, path::Path};

use clap::{Args, Subcommand};
use eyre::{Result, WrapErr, eyre};
use iroha::data_model::{
    isi::social::{CancelTwitterEscrow, ClaimTwitterFollowReward, SendToTwitter},
    oracle::KeyedHash,
};
use iroha_primitives::numeric::Numeric;
use norito::json;

use crate::{Run, RunContext};

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Claim a promotional reward for a verified Twitter follow binding.
    #[command(name = "claim-twitter-follow-reward")]
    ClaimTwitterFollowReward(ClaimArgs),
    /// Send funds to a Twitter handle; funds are escrowed until a follow binding appears.
    #[command(name = "send-to-twitter")]
    SendToTwitter(SendArgs),
    /// Cancel an existing escrow created by `send-to-twitter`.
    #[command(name = "cancel-twitter-escrow")]
    CancelTwitterEscrow(CancelArgs),
}

/// Claim a follow reward using a keyed binding hash JSON payload.
#[derive(Args, Debug)]
pub struct ClaimArgs {
    /// Path to a JSON file containing a `KeyedHash` (binding hash) payload.
    ///
    /// The JSON shape must match `iroha_data_model::oracle::KeyedHash`.
    #[arg(long, value_name = "PATH")]
    pub binding_hash_json: String,
}

/// Send funds to a Twitter handle using a keyed binding hash JSON payload.
#[derive(Args, Debug)]
pub struct SendArgs {
    /// Path to a JSON file containing a `KeyedHash` (binding hash) payload.
    ///
    /// The JSON shape must match `iroha_data_model::oracle::KeyedHash`.
    #[arg(long, value_name = "PATH")]
    pub binding_hash_json: String,
    /// Amount to escrow or deliver immediately when the binding is already active.
    ///
    /// Parsed as `Numeric` (mantissa/scale) using the standard string format.
    #[arg(long, value_name = "AMOUNT")]
    pub amount: String,
}

/// Cancel an existing Twitter escrow using a keyed binding hash JSON payload.
#[derive(Args, Debug)]
pub struct CancelArgs {
    /// Path to a JSON file containing a `KeyedHash` (binding hash) payload.
    ///
    /// The JSON shape must match `iroha_data_model::oracle::KeyedHash`.
    #[arg(long, value_name = "PATH")]
    pub binding_hash_json: String,
}

fn load_binding_hash(path: &Path) -> Result<KeyedHash> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read binding-hash JSON from {}", path.display()))?;
    let value = json::from_slice::<KeyedHash>(&bytes).wrap_err_with(|| {
        format!(
            "failed to decode binding-hash JSON at {} as KeyedHash",
            path.display()
        )
    })?;
    Ok(value)
}

fn parse_numeric(value: &str, flag: &str) -> Result<Numeric> {
    value
        .parse::<Numeric>()
        .wrap_err_with(|| format!("{flag} must be a valid Numeric"))
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::ClaimTwitterFollowReward(args) => {
                let binding_hash = load_binding_hash(Path::new(&args.binding_hash_json))?;
                let instr = ClaimTwitterFollowReward { binding_hash };
                context.finish([instr])
            }
            Command::SendToTwitter(args) => {
                let binding_hash = load_binding_hash(Path::new(&args.binding_hash_json))?;
                let amount = parse_numeric(&args.amount, "--amount")?;
                if amount.is_zero() {
                    return Err(eyre!("--amount must be non-zero"));
                }
                let instr = SendToTwitter {
                    binding_hash,
                    amount,
                };
                context.finish([instr])
            }
            Command::CancelTwitterEscrow(args) => {
                let binding_hash = load_binding_hash(Path::new(&args.binding_hash_json))?;
                let instr = CancelTwitterEscrow { binding_hash };
                context.finish([instr])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Hash;
    use iroha_data_model::oracle::KeyedHash as ModelKeyedHash;
    use norito::json;
    use tempfile::tempdir;

    #[test]
    fn load_binding_hash_roundtrips_json() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("binding.json");
        let value = ModelKeyedHash {
            pepper_id: "test-pepper".to_string(),
            digest: Hash::new(b"test-binding"),
        };
        let bytes =
            json::to_vec_pretty(&value).expect("serialize KeyedHash JSON for social helper test");
        fs::write(&path, bytes).expect("write binding.json");

        let loaded = load_binding_hash(&path).expect("load binding hash");
        assert_eq!(loaded.pepper_id, value.pepper_id);
        assert_eq!(loaded.digest, value.digest);
    }

    #[test]
    fn parse_numeric_rejects_invalid_values() {
        let err = parse_numeric("not-a-number", "--amount").unwrap_err();
        assert!(
            format!("{err:?}").contains("must be a valid Numeric"),
            "error should mention invalid Numeric"
        );
    }
}
