//! Render an account literal for a public key and chain discriminant.

use std::{env, str::FromStr};

use eyre::{Result, eyre};
use iroha_crypto::PublicKey;
use iroha_data_model::account::AccountId;

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let public_key = args
        .next()
        .ok_or_else(|| eyre!("usage: account_literal <public_key> [chain_discriminant]"))?;
    let chain_discriminant = args
        .next()
        .map(|value| value.parse::<u16>())
        .transpose()?
        .unwrap_or(369);

    let public_key = PublicKey::from_str(&public_key)?;
    let account_id = AccountId::new(public_key);
    println!(
        "{}",
        account_id.to_i105_for_discriminant(chain_discriminant)?
    );
    Ok(())
}
