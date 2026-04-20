//! Minimal stand-in for the `kagami` binary used by the MOCHI supervisor integration tests.

use std::{env, path::PathBuf, process};

use color_eyre::{Result, eyre::eyre};
use iroha_crypto::{Algorithm, PublicKey};
use iroha_data_model::parameter::system::SumeragiConsensusMode;
use mochi_integration::kagami_default_manifest_json;

const DEFAULT_CHAIN_ID: &str = "mochi-mock-chain";
const VERSION_OUTPUT: &str = "kagami_mock iroha3 test-stub";

fn main() {
    if let Err(err) = run() {
        eprintln!("kagami_mock: {err:?}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("--version") | Some("-V") => {
            println!("{VERSION_OUTPUT}");
            Ok(())
        }
        Some("genesis") => match args.next().as_deref() {
            Some("generate") => generate(args.collect()),
            _ => Err(eyre!(
                "unsupported invocation; expected `kagami genesis generate ...`"
            )),
        },
        Some("verify") => verify(args.collect()),
        _ => Err(eyre!(
            "unsupported invocation; expected `kagami --version`, `kagami genesis generate ...`, or `kagami verify ...`"
        )),
    }
}

struct GenerateArgs {
    ivm_dir: PathBuf,
    genesis_public_key: String,
    chain_id: String,
    consensus_mode: SumeragiConsensusMode,
}

fn generate(args: Vec<String>) -> Result<()> {
    let parsed = parse_generate_args(&args)?;
    let public_key: PublicKey = match parsed.genesis_public_key.parse() {
        Ok(key) => key,
        Err(parse_err) => {
            let trimmed = parsed.genesis_public_key.trim_start_matches("0x");
            match PublicKey::from_hex(Algorithm::Ed25519, trimmed) {
                Ok(key) => key,
                Err(hex_err) => {
                    return Err(eyre!(
                        "invalid genesis public key `{}`: {parse_err}; fallback hex decode failed: {hex_err}",
                        parsed.genesis_public_key
                    ));
                }
            }
        }
    };

    let manifest = kagami_default_manifest_json(
        &public_key,
        &parsed.ivm_dir,
        parsed.chain_id,
        parsed.consensus_mode,
    )?;
    println!("{manifest}");
    Ok(())
}

fn parse_generate_args(args: &[String]) -> Result<GenerateArgs> {
    let mut ivm_dir = PathBuf::from(".");
    let mut genesis_public_key = None;
    let mut chain_id = DEFAULT_CHAIN_ID.to_owned();
    let mut consensus_mode = SumeragiConsensusMode::Permissioned;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--ivm-dir" => {
                let path = next_arg_value(args, &mut index, "--ivm-dir")?;
                ivm_dir = PathBuf::from(path);
            }
            "--genesis-public-key" => {
                genesis_public_key =
                    Some(next_arg_value(args, &mut index, "--genesis-public-key")?.to_owned());
            }
            "--chain-id" => {
                chain_id = next_arg_value(args, &mut index, "--chain-id")?.to_owned();
            }
            "--consensus-mode" => {
                consensus_mode =
                    parse_consensus_mode(next_arg_value(args, &mut index, "--consensus-mode")?)?;
            }
            "--profile" | "--vrf-seed-hex" => {
                let flag = &args[index];
                let _ = next_arg_value(args, &mut index, flag)?;
            }
            "default" => break,
            other => return Err(eyre!("unsupported argument `{other}`")),
        }
        index += 1;
    }

    let Some(genesis_public_key) = genesis_public_key else {
        return Err(eyre!("missing `--genesis-public-key` argument"));
    };
    Ok(GenerateArgs {
        ivm_dir,
        genesis_public_key,
        chain_id,
        consensus_mode,
    })
}

fn next_arg_value<'a>(args: &'a [String], index: &mut usize, flag: &str) -> Result<&'a str> {
    *index += 1;
    args.get(*index)
        .map(String::as_str)
        .ok_or_else(|| eyre!("{flag} requires a value"))
}

fn parse_consensus_mode(value: &str) -> Result<SumeragiConsensusMode> {
    match value {
        "permissioned" => Ok(SumeragiConsensusMode::Permissioned),
        "npos" => Ok(SumeragiConsensusMode::Npos),
        other => Err(eyre!("unsupported consensus mode `{other}`")),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::KeyPair;
    use norito::json::Value;

    #[test]
    fn parse_generate_args_accepts_chain_id_and_consensus_mode() {
        let key_pair = KeyPair::random();
        let args = vec![
            "--ivm-dir".to_owned(),
            "./ivm".to_owned(),
            "--genesis-public-key".to_owned(),
            key_pair.public_key().to_string(),
            "--chain-id".to_owned(),
            "local-chain".to_owned(),
            "--profile".to_owned(),
            "iroha3-dev".to_owned(),
            "--consensus-mode".to_owned(),
            "npos".to_owned(),
            "--vrf-seed-hex".to_owned(),
            "abcd".to_owned(),
            "default".to_owned(),
        ];

        let parsed = parse_generate_args(&args).expect("parse mock kagami args");

        assert_eq!(parsed.ivm_dir, PathBuf::from("./ivm"));
        assert_eq!(parsed.genesis_public_key, key_pair.public_key().to_string());
        assert_eq!(parsed.chain_id, "local-chain");
        assert_eq!(parsed.consensus_mode, SumeragiConsensusMode::Npos);
    }

    #[test]
    fn generate_emits_manifest_with_requested_chain_id() {
        let key_pair = KeyPair::random();
        let ivm_dir = tempfile::tempdir().expect("tempdir");
        let manifest = kagami_default_manifest_json(
            key_pair.public_key(),
            ivm_dir.path(),
            "custom-chain",
            SumeragiConsensusMode::Permissioned,
        )
        .expect("generate mock manifest");
        let value: Value = norito::json::from_str(&manifest).expect("parse manifest json");

        assert_eq!(
            value.get("chain").and_then(Value::as_str),
            Some("custom-chain")
        );
        assert_eq!(
            value.get("consensus_mode").and_then(Value::as_str),
            Some("Permissioned")
        );
    }

    #[test]
    fn generate_accepts_current_supervisor_flags() {
        let key_pair = KeyPair::random();
        let ivm_dir = tempfile::tempdir().expect("tempdir");
        let args = vec![
            "--ivm-dir".to_owned(),
            ivm_dir.path().display().to_string(),
            "--genesis-public-key".to_owned(),
            key_pair.public_key().to_string(),
            "--chain-id".to_owned(),
            "supervisor-chain".to_owned(),
            "--consensus-mode".to_owned(),
            "permissioned".to_owned(),
            "default".to_owned(),
        ];

        generate(args).expect("generate manifest via mock kagami");
    }

    #[test]
    fn version_probe_reports_iroha3_build_line() {
        assert!(VERSION_OUTPUT.contains("iroha3"));
    }
}
