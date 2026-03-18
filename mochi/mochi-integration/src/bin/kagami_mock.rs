//! Minimal stand-in for the `kagami` binary used by the MOCHI supervisor integration tests.

use std::{env, path::PathBuf, process};

use color_eyre::{Result, eyre::eyre};
use iroha_crypto::{Algorithm, PublicKey};
use mochi_integration::kagami_default_manifest_json;

fn main() {
    if let Err(err) = run() {
        eprintln!("kagami_mock: {err:?}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("genesis") => match args.next().as_deref() {
            Some("generate") => generate(args.collect()),
            _ => Err(eyre!(
                "unsupported invocation; expected `kagami genesis generate ...`"
            )),
        },
        Some("verify") => verify(args.collect()),
        _ => Err(eyre!(
            "unsupported invocation; expected `kagami genesis generate ...` or `kagami verify ...`"
        )),
    }
}

fn generate(args: Vec<String>) -> Result<()> {
    let mut ivm_dir = PathBuf::from(".");
    let mut genesis_key = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--ivm-dir" => {
                index += 1;
                let path = args
                    .get(index)
                    .ok_or_else(|| eyre!("--ivm-dir requires a value"))?;
                ivm_dir = PathBuf::from(path);
            }
            "--genesis-public-key" => {
                index += 1;
                genesis_key = Some(
                    args.get(index)
                        .ok_or_else(|| eyre!("--genesis-public-key requires a value"))?
                        .clone(),
                );
            }
            "default" => break,
            other => return Err(eyre!("unsupported argument `{other}`")),
        }
        index += 1;
    }

    let Some(genesis_key) = genesis_key else {
        return Err(eyre!("missing `--genesis-public-key` argument"));
    };
    let public_key: PublicKey = match genesis_key.parse() {
        Ok(key) => key,
        Err(parse_err) => {
            let trimmed = genesis_key.trim_start_matches("0x");
            match PublicKey::from_hex(Algorithm::Ed25519, trimmed) {
                Ok(key) => key,
                Err(hex_err) => {
                    return Err(eyre!(
                        "invalid genesis public key `{genesis_key}`: {parse_err}; fallback hex decode failed: {hex_err}"
                    ));
                }
            }
        }
    };

    let manifest = kagami_default_manifest_json(&public_key, &ivm_dir)?;
    println!("{manifest}");
    Ok(())
}

fn verify(args: Vec<String>) -> Result<()> {
    let mut profile = None;
    let mut genesis = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--profile" => {
                index += 1;
                profile = args.get(index).cloned();
            }
            "--genesis" => {
                index += 1;
                genesis = args.get(index).map(PathBuf::from);
            }
            "--vrf-seed-hex" => {
                index += 1;
            }
            other => return Err(eyre!("unsupported argument `{other}`")),
        }
        index += 1;
    }

    let Some(profile) = profile else {
        return Err(eyre!("missing `--profile` argument"));
    };
    if genesis.as_ref().map(|path| path.is_file()).unwrap_or(false) {
        println!(
            "verified genesis {:?} for profile {}",
            genesis.unwrap(),
            profile
        );
        return Ok(());
    }

    Err(eyre!(
        "missing or unreadable genesis path for profile {profile}"
    ))
}
