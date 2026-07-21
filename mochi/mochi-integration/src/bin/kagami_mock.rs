//! Minimal stand-in for the `kagami` binary used by the MOCHI supervisor integration tests.

use std::{env, fs, path::PathBuf, process};

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
            Some("sign") => sign(args.collect()),
            _ => Err(eyre!(
                "unsupported invocation; expected `kagami genesis generate ...` or `kagami genesis sign ...`"
            )),
        },
        Some("verify") => verify(args.collect()),
        _ => Err(eyre!(
            "unsupported invocation; expected `kagami --version`, `kagami genesis generate ...`, `kagami genesis sign ...`, or `kagami verify ...`"
        )),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SignArgs {
    manifest_path: PathBuf,
    out_file: PathBuf,
    bound_manifest_out: PathBuf,
    private_key_file: PathBuf,
    config_file: PathBuf,
}

fn sign(args: Vec<String>) -> Result<()> {
    let parsed = parse_sign_args(&args)?;
    for (label, path) in [
        ("genesis manifest", &parsed.manifest_path),
        ("private key", &parsed.private_key_file),
        ("peer config", &parsed.config_file),
    ] {
        if !path.is_file() {
            return Err(eyre!("{label} is missing: {}", path.display()));
        }
    }
    if parsed.out_file == parsed.bound_manifest_out {
        return Err(eyre!(
            "signed genesis output and bound manifest output must use different paths"
        ));
    }

    fs::write(&parsed.out_file, b"mock-signed-genesis")?;
    if parsed.bound_manifest_out != parsed.manifest_path {
        fs::copy(&parsed.manifest_path, &parsed.bound_manifest_out)?;
    }
    Ok(())
}

fn parse_sign_args(args: &[String]) -> Result<SignArgs> {
    let manifest_path = args
        .first()
        .filter(|value| !value.starts_with('-'))
        .map(PathBuf::from)
        .ok_or_else(|| eyre!("missing genesis manifest path"))?;
    let mut out_file = None;
    let mut bound_manifest_out = None;
    let mut private_key_file = None;
    let mut config_file = None;

    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--out-file" => {
                out_file = Some(PathBuf::from(next_arg_value(
                    args,
                    &mut index,
                    "--out-file",
                )?));
            }
            "--bound-manifest-out" => {
                bound_manifest_out = Some(PathBuf::from(next_arg_value(
                    args,
                    &mut index,
                    "--bound-manifest-out",
                )?));
            }
            "--private-key-file" => {
                private_key_file = Some(PathBuf::from(next_arg_value(
                    args,
                    &mut index,
                    "--private-key-file",
                )?));
            }
            "--config" => {
                config_file = Some(PathBuf::from(next_arg_value(args, &mut index, "--config")?));
            }
            "--consensus-mode" => {
                let value = next_arg_value(args, &mut index, "--consensus-mode")?;
                let _ = parse_consensus_mode(value)?;
            }
            other => return Err(eyre!("unsupported argument `{other}`")),
        }
        index += 1;
    }

    Ok(SignArgs {
        manifest_path,
        out_file: out_file.ok_or_else(|| eyre!("missing `--out-file` argument"))?,
        bound_manifest_out: bound_manifest_out
            .ok_or_else(|| eyre!("missing `--bound-manifest-out` argument"))?,
        private_key_file: private_key_file
            .ok_or_else(|| eyre!("missing `--private-key-file` argument"))?,
        config_file: config_file.ok_or_else(|| eyre!("missing `--config` argument"))?,
    })
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
    fn parse_sign_args_accepts_current_supervisor_flags() {
        let args = vec![
            "/tmp/genesis.json".to_owned(),
            "--out-file".to_owned(),
            "/tmp/genesis.signed.nrt".to_owned(),
            "--bound-manifest-out".to_owned(),
            "/tmp/genesis.bound.json".to_owned(),
            "--private-key-file".to_owned(),
            "/tmp/genesis.key".to_owned(),
            "--config".to_owned(),
            "/tmp/peer.toml".to_owned(),
            "--consensus-mode".to_owned(),
            "permissioned".to_owned(),
        ];

        assert_eq!(
            parse_sign_args(&args).expect("parse mock kagami sign args"),
            SignArgs {
                manifest_path: PathBuf::from("/tmp/genesis.json"),
                out_file: PathBuf::from("/tmp/genesis.signed.nrt"),
                bound_manifest_out: PathBuf::from("/tmp/genesis.bound.json"),
                private_key_file: PathBuf::from("/tmp/genesis.key"),
                config_file: PathBuf::from("/tmp/peer.toml"),
            }
        );
    }

    #[test]
    fn sign_publishes_block_before_bound_manifest() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = temp.path().join("genesis.json");
        let signed = temp.path().join("genesis.signed.nrt");
        let bound = temp.path().join("genesis.bound.json");
        let private_key = temp.path().join("genesis.key");
        let config = temp.path().join("peer.toml");
        fs::write(&manifest, b"manifest").expect("write manifest");
        fs::write(&private_key, b"private-key").expect("write private key");
        fs::write(&config, b"config").expect("write config");

        sign(vec![
            manifest.display().to_string(),
            "--out-file".to_owned(),
            signed.display().to_string(),
            "--bound-manifest-out".to_owned(),
            bound.display().to_string(),
            "--private-key-file".to_owned(),
            private_key.display().to_string(),
            "--config".to_owned(),
            config.display().to_string(),
            "--consensus-mode".to_owned(),
            "permissioned".to_owned(),
        ])
        .expect("sign with mock kagami");

        assert_eq!(
            fs::read(signed).expect("read signed output"),
            b"mock-signed-genesis"
        );
        assert_eq!(fs::read(bound).expect("read bound output"), b"manifest");
    }

    #[test]
    fn sign_output_failure_preserves_existing_bound_manifest() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = temp.path().join("genesis.json");
        let bound = temp.path().join("genesis.bound.json");
        let private_key = temp.path().join("genesis.key");
        let config = temp.path().join("peer.toml");
        fs::write(&manifest, b"manifest").expect("write manifest");
        fs::write(&bound, b"sentinel").expect("write bound sentinel");
        fs::write(&private_key, b"private-key").expect("write private key");
        fs::write(&config, b"config").expect("write config");

        let _ = sign(vec![
            manifest.display().to_string(),
            "--out-file".to_owned(),
            temp.path()
                .join("missing/genesis.signed.nrt")
                .display()
                .to_string(),
            "--bound-manifest-out".to_owned(),
            bound.display().to_string(),
            "--private-key-file".to_owned(),
            private_key.display().to_string(),
            "--config".to_owned(),
            config.display().to_string(),
            "--consensus-mode".to_owned(),
            "permissioned".to_owned(),
        ])
        .expect_err("missing output parent should fail");

        assert_eq!(
            fs::read(bound).expect("read bound sentinel"),
            b"sentinel",
            "bound manifest must only publish after signed output succeeds"
        );
    }

    #[test]
    fn version_probe_reports_iroha3_build_line() {
        assert!(VERSION_OUTPUT.contains("iroha3"));
    }
}
