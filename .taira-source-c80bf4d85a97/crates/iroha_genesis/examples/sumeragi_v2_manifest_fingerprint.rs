//! Print canonical Sumeragi v2 metadata for raw genesis manifests.

use std::{env, path::PathBuf, str::FromStr as _};

use iroha_crypto::PublicKey;
use iroha_data_model::account::AccountId;
use iroha_genesis::RawGenesisTransaction;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut raw_args = env::args_os().skip(1);
    if raw_args.next().as_deref() == Some(std::ffi::OsStr::new("--account")) {
        let public_key = raw_args
            .next()
            .ok_or("--account requires a public key")?
            .into_string()
            .map_err(|_| "public key must be UTF-8")?;
        let discriminant = raw_args
            .next()
            .ok_or("--account requires a chain discriminant")?
            .into_string()
            .map_err(|_| "chain discriminant must be UTF-8")?
            .parse::<u16>()?;
        if raw_args.next().is_some() {
            return Err("usage: sumeragi_v2_manifest_fingerprint --account <public-key> <chain-discriminant>".into());
        }
        let account = AccountId::new(PublicKey::from_str(&public_key)?);
        println!("{}", account.to_i105_for_discriminant(discriminant)?);
        return Ok(());
    }

    let paths = env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return Err("usage: sumeragi_v2_manifest_fingerprint <genesis.json> [...]".into());
    }

    for path in paths {
        let manifest = RawGenesisTransaction::from_path(&path)?;
        let refreshed = manifest.with_consensus_meta();
        let context = refreshed.sumeragi_v2_context_parameters();
        let fingerprint = refreshed
            .consensus_fingerprint()
            .ok_or("manifest omitted consensus fingerprint")?;
        println!(
            "{} {} {}",
            path.display(),
            hex::encode(context.nexus_amx_context_hash),
            fingerprint
        );
    }
    Ok(())
}
