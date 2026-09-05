//! Minimal stand-in for the `kagami` binary used by the MOCHI supervisor integration tests.
use color_eyre::{Result, eyre::eyre};
use iroha_crypto::{ExposedPrivateKey, KeyPair, PublicKey};
use iroha_data_model::{
    NetworkId, isi::kagemusha_v1::KagemushaMintFinalityGenesisParametersV1,
    parameter::system::SumeragiConsensusMode, prelude::ChainId,
};
use mochi_core::{GenesisProfile, sign_kagami_stub_genesis_from_config};
use mochi_integration::kagami_default_manifest_json;
use std::{env, fs, path::PathBuf, process};
const KAGEMUSHA_MINT_FINALITY_PARAMETERS_MAX_BYTES: u64 = 1024 * 1024;
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
            Some("sign") => sign(args.collect()),
            _ => Err(eyre!(
                "unsupported invocation; expected `kagami genesis generate ...` or `kagami genesis sign ...`"
            )),
        },
        Some("verify") => verify(args.collect()),
        _ => Err(eyre!(
            "unsupported invocation; expected `kagami genesis generate ...`, `kagami genesis sign ...`, or `kagami verify ...`"
        )),
    }
}
#[derive(Debug, PartialEq, Eq)]
struct SignArgs {
    manifest_path: PathBuf,
    out_file: PathBuf,
    bound_manifest_out: PathBuf,
    expected_hash_out: PathBuf,
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
    let private_record = fs::read_to_string(&parsed.private_key_file)?;
    let canonical_private = private_record
        .strip_suffix('\n')
        .ok_or_else(|| eyre!("private key record must end in one newline"))?;
    if canonical_private.is_empty() || canonical_private.chars().any(char::is_whitespace) {
        return Err(eyre!("private key record is not canonical"));
    }
    let private_key = canonical_private.parse::<ExposedPrivateKey>()?;
    if private_key.to_string() != canonical_private {
        return Err(eyre!("private key record is not canonical"));
    }
    let key_pair = KeyPair::from_private_key(private_key.0)?;
    let block = sign_kagami_stub_genesis_from_config(
        &parsed.manifest_path,
        &parsed.config_file,
        &key_pair,
        None,
    )?;
    fs::write(&parsed.out_file, block.encode_wire()?)?;
    if parsed.bound_manifest_out != parsed.manifest_path {
        fs::copy(&parsed.manifest_path, &parsed.bound_manifest_out)?;
    }
    fs::write(
        &parsed.expected_hash_out,
        format!("{}\n", NetworkId::from_genesis_hash(block.hash())),
    )?;
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
    let mut expected_hash_out = None;
    let mut private_key_file = None;
    let mut config_file = None;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--out-file" => {
                let value = PathBuf::from(next_arg_value(args, &mut index, "--out-file")?);
                set_once(&mut out_file, "--out-file", value)?;
            }
            "--bound-manifest-out" => {
                let value =
                    PathBuf::from(next_arg_value(args, &mut index, "--bound-manifest-out")?);
                set_once(&mut bound_manifest_out, "--bound-manifest-out", value)?;
            }
            "--expected-hash-out" => {
                let value = PathBuf::from(next_arg_value(args, &mut index, "--expected-hash-out")?);
                set_once(&mut expected_hash_out, "--expected-hash-out", value)?;
            }
            "--private-key-file" => {
                let value = PathBuf::from(next_arg_value(args, &mut index, "--private-key-file")?);
                set_once(&mut private_key_file, "--private-key-file", value)?;
            }
            "--config" => {
                let value = PathBuf::from(next_arg_value(args, &mut index, "--config")?);
                set_once(&mut config_file, "--config", value)?;
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
        expected_hash_out: expected_hash_out
            .ok_or_else(|| eyre!("missing `--expected-hash-out` argument"))?,
        private_key_file: private_key_file
            .ok_or_else(|| eyre!("missing `--private-key-file` argument"))?,
        config_file: config_file.ok_or_else(|| eyre!("missing `--config` argument"))?,
    })
}
#[derive(Debug)]
struct GenerateArgs {
    ivm_dir: PathBuf,
    genesis_public_key: String,
    chain_id: String,
    consensus_mode: SumeragiConsensusMode,
    kagemusha_mint_finality_parameters: PathBuf,
}
fn generate(args: Vec<String>) -> Result<()> {
    let parsed = parse_generate_args(&args)?;
    let public_key = parse_genesis_public_key(&parsed.genesis_public_key)?;
    let parameter_metadata = fs::symlink_metadata(&parsed.kagemusha_mint_finality_parameters)?;
    if parameter_metadata.file_type().is_symlink()
        || !parameter_metadata.is_file()
        || parameter_metadata.len() > KAGEMUSHA_MINT_FINALITY_PARAMETERS_MAX_BYTES
    {
        return Err(eyre!(
            "KAGEMUSHA mint-finality parameters must be a regular file no larger than {} bytes",
            KAGEMUSHA_MINT_FINALITY_PARAMETERS_MAX_BYTES
        ));
    }
    let parameter_bytes = fs::read(&parsed.kagemusha_mint_finality_parameters)?;
    let kagemusha_mint_finality: KagemushaMintFinalityGenesisParametersV1 =
        norito::json::from_slice(&parameter_bytes)?;
    kagemusha_mint_finality
        .validate()
        .map_err(|error| eyre!("invalid KAGEMUSHA mint-finality parameters: {error}"))?;
    let manifest = kagami_default_manifest_json(
        &public_key,
        &parsed.ivm_dir,
        parsed.chain_id,
        parsed.consensus_mode,
        &kagemusha_mint_finality,
    )?;
    println!("{manifest}");
    Ok(())
}
fn parse_genesis_public_key(value: &str) -> Result<PublicKey> {
    value
        .parse::<PublicKey>()
        .map_err(|error| eyre!("invalid canonical genesis public key `{value}`: {error}"))
}
fn parse_generate_args(args: &[String]) -> Result<GenerateArgs> {
    let mut ivm_dir = None;
    let mut genesis_public_key = None;
    let mut chain_id = None;
    let mut consensus_mode = None;
    let mut kagemusha_mint_finality_parameters = None;
    let mut profile = None;
    let mut vrf_seed_hex = None;
    let mut saw_default = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--ivm-dir" => {
                let value = PathBuf::from(next_arg_value(args, &mut index, "--ivm-dir")?);
                set_once(&mut ivm_dir, "--ivm-dir", value)?;
            }
            "--genesis-public-key" => {
                let value = next_arg_value(args, &mut index, "--genesis-public-key")?.to_owned();
                set_once(&mut genesis_public_key, "--genesis-public-key", value)?;
            }
            "--chain-id" => {
                let raw = next_arg_value(args, &mut index, "--chain-id")?;
                let parsed = raw
                    .parse::<ChainId>()
                    .map_err(|error| eyre!("invalid chain id `{raw}`: {error}"))?;
                if parsed.to_string() != raw {
                    return Err(eyre!("chain id must use its canonical spelling"));
                }
                set_once(&mut chain_id, "--chain-id", raw.to_owned())?;
            }
            "--consensus-mode" => {
                let value =
                    parse_consensus_mode(next_arg_value(args, &mut index, "--consensus-mode")?)?;
                set_once(&mut consensus_mode, "--consensus-mode", value)?;
            }
            "--kagemusha-mint-finality-parameters" => {
                let value = PathBuf::from(next_arg_value(
                    args,
                    &mut index,
                    "--kagemusha-mint-finality-parameters",
                )?);
                set_once(
                    &mut kagemusha_mint_finality_parameters,
                    "--kagemusha-mint-finality-parameters",
                    value,
                )?;
            }
            "--profile" => {
                let raw = next_arg_value(args, &mut index, "--profile")?;
                let value = raw
                    .parse::<GenesisProfile>()
                    .map_err(|error| eyre!(error))?;
                set_once(&mut profile, "--profile", value)?;
            }
            "--vrf-seed-hex" => {
                let value = next_arg_value(args, &mut index, "--vrf-seed-hex")?.to_owned();
                validate_vrf_seed_hex(&value)?;
                set_once(&mut vrf_seed_hex, "--vrf-seed-hex", value)?;
            }
            "default" => {
                if index + 1 != args.len() {
                    return Err(eyre!("`default` must be the terminal generate argument"));
                }
                saw_default = true;
                break;
            }
            other => return Err(eyre!("unsupported argument `{other}`")),
        }
        index += 1;
    }
    if !saw_default {
        return Err(eyre!("missing terminal `default` argument"));
    }
    validate_profile_seed(profile, vrf_seed_hex.as_deref())?;
    Ok(GenerateArgs {
        ivm_dir: ivm_dir.ok_or_else(|| eyre!("missing `--ivm-dir` argument"))?,
        genesis_public_key: genesis_public_key
            .ok_or_else(|| eyre!("missing `--genesis-public-key` argument"))?,
        chain_id: chain_id.ok_or_else(|| eyre!("missing `--chain-id` argument"))?,
        consensus_mode: consensus_mode
            .ok_or_else(|| eyre!("missing `--consensus-mode` argument"))?,
        kagemusha_mint_finality_parameters: kagemusha_mint_finality_parameters
            .ok_or_else(|| eyre!("missing `--kagemusha-mint-finality-parameters` argument"))?,
    })
}
fn set_once<T>(slot: &mut Option<T>, flag: &str, value: T) -> Result<()> {
    if slot.is_some() {
        return Err(eyre!("duplicate `{flag}` argument"));
    }
    *slot = Some(value);
    Ok(())
}
fn next_arg_value<'a>(args: &'a [String], index: &mut usize, flag: &str) -> Result<&'a str> {
    *index += 1;
    let value = args
        .get(*index)
        .map(String::as_str)
        .ok_or_else(|| eyre!("{flag} requires a value"))?;
    if value.starts_with("--") || value == "default" {
        return Err(eyre!("{flag} requires a value"));
    }
    Ok(value)
}
fn parse_consensus_mode(value: &str) -> Result<SumeragiConsensusMode> {
    match value {
        "permissioned" => Ok(SumeragiConsensusMode::Permissioned),
        "npos" => Ok(SumeragiConsensusMode::Npos),
        other => Err(eyre!("unsupported consensus mode `{other}`")),
    }
}
#[derive(Debug, PartialEq, Eq)]
struct VerifyArgs {
    profile: GenesisProfile,
    genesis: PathBuf,
    vrf_seed_hex: Option<String>,
}
fn parse_verify_args(args: &[String]) -> Result<VerifyArgs> {
    let mut profile = None;
    let mut genesis = None;
    let mut vrf_seed_hex = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--profile" => {
                let raw = next_arg_value(args, &mut index, "--profile")?;
                let value = raw
                    .parse::<GenesisProfile>()
                    .map_err(|error| eyre!(error))?;
                set_once(&mut profile, "--profile", value)?;
            }
            "--genesis" => {
                let value = PathBuf::from(next_arg_value(args, &mut index, "--genesis")?);
                set_once(&mut genesis, "--genesis", value)?;
            }
            "--vrf-seed-hex" => {
                let value = next_arg_value(args, &mut index, "--vrf-seed-hex")?.to_owned();
                validate_vrf_seed_hex(&value)?;
                set_once(&mut vrf_seed_hex, "--vrf-seed-hex", value)?;
            }
            other => return Err(eyre!("unsupported argument `{other}`")),
        }
        index += 1;
    }
    let profile = profile.ok_or_else(|| eyre!("missing `--profile` argument"))?;
    validate_profile_seed(Some(profile), vrf_seed_hex.as_deref())?;
    Ok(VerifyArgs {
        profile,
        genesis: genesis.ok_or_else(|| eyre!("missing `--genesis` argument"))?,
        vrf_seed_hex,
    })
}
fn validate_vrf_seed_hex(seed: &str) -> Result<()> {
    if seed.len() != 64 || !seed.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(eyre!("VRF seed must be exactly 32 hexadecimal bytes"));
    }
    Ok(())
}
fn validate_profile_seed(profile: Option<GenesisProfile>, seed: Option<&str>) -> Result<()> {
    if profile.is_none() && seed.is_some() {
        return Err(eyre!("`--vrf-seed-hex` requires `--profile`"));
    }
    if profile.is_some_and(GenesisProfile::requires_seed) && seed.is_none() {
        return Err(eyre!("selected profile requires `--vrf-seed-hex`"));
    }
    Ok(())
}
fn verify(args: Vec<String>) -> Result<()> {
    let parsed = parse_verify_args(&args)?;
    if parsed.genesis.is_file() {
        println!(
            "verified genesis {:?} for profile {}",
            parsed.genesis, parsed.profile
        );
        return Ok(());
    }
    Err(eyre!(
        "missing or unreadable genesis path for profile {}",
        parsed.profile
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, bls_normal_pop_prove};
    use iroha_data_model::{
        block::decode_framed_signed_block,
        isi::kagemusha_v1::{
            KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterTemplateV1,
        },
        peer::PeerId,
    };
    use iroha_genesis::{GenesisTopologyEntry, RawGenesisTransaction};
    use mochi_core::kagami_stub_genesis_policies_from_config;
    use norito::json::Value;
    const GENESIS_EXPECTED_HASH_PLACEHOLDER: &str = "REPLACE_WITH_GENESIS_EXPECTED_HASH";

    fn test_topology() -> Vec<GenesisTopologyEntry> {
        (0..4)
            .map(|_| {
                let validator = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
                    .expect("generate validator key");
                let pop = bls_normal_pop_prove(validator.private_key())
                    .expect("generate validator proof of possession");
                GenesisTopologyEntry::new(PeerId::new(validator.public_key().clone()), pop)
            })
            .collect()
    }

    fn test_kagemusha_mint_finality_parameters(
        topology: &[GenesisTopologyEntry],
    ) -> KagemushaMintFinalityGenesisParametersV1 {
        let mut validators = topology
            .iter()
            .map(|entry| entry.peer.clone())
            .collect::<Vec<_>>();
        validators.sort();
        let validators = validators
            .into_iter()
            .enumerate()
            .map(|(index, validator)| {
                iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                    &[0xA0_u8.wrapping_add(u8::try_from(index).expect("test index fits u8")); 32],
                    0,
                    validator,
                )
                .expect("derive mock mint-finality validator keys")
            })
            .collect();
        let parameters = KagemushaMintFinalityGenesisParametersV1 {
            epoch_roster: KagemushaMintFinalityEpochRosterTemplateV1 {
                version: KAGEMUSHA_CHAIN_VERSION_V1,
                epoch: 0,
                validators,
            },
            next_epoch_roster: None,
        };
        parameters
            .validate()
            .expect("mock mint-finality parameters are valid");
        parameters
    }

    fn with_valid_topology(manifest_json: String) -> String {
        let manifest: RawGenesisTransaction =
            norito::json::from_str(&manifest_json).expect("decode mock manifest");
        let topology = test_topology();
        let kagemusha_mint_finality = test_kagemusha_mint_finality_parameters(&topology);
        let manifest = manifest
            .into_builder()
            .with_kagemusha_mint_finality_genesis_parameters(kagemusha_mint_finality)
            .next_transaction()
            .set_topology(topology)
            .build_raw()
            .expect("rebuild complete mock manifest");
        norito::json::to_json_pretty(&manifest).expect("encode mock manifest")
    }

    fn peer_config(
        chain_id: &str,
        genesis_public_key: &PublicKey,
        manifest: &std::path::Path,
        signed: &std::path::Path,
    ) -> String {
        format!(
            r#"chain = "{chain_id}"
chain_discriminant = {chain_discriminant}
public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
private_key = "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"
soranet_transport_public_key = "ed0120D9F6AEF1813164294D1D9C0662FEB9C7F7861B4DFFE385680331093DA4ABD10B"
soranet_transport_private_key = "802620134C4527B3852AE2218A8F079B301C651EAD8C7567B96BD7A9BE8DB366E46B89"
trusted_peers_pop = [
  {{ public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2", pop_hex = "8515da750f81182aaba5c22fc9f03a01e81ed85e4495a2ca6b29a71c0c8549537e31e79cddf6ff285b9e22d0d9dc17ce0f46e7d0cf78b2ef9feab50c849a1ea8e1e4f07e966f6113faa8a999317545d9f111b8e08a7273913710b43a20b19c08" }}
]

[network]
address = "addr:127.0.0.1:1337#8F78"
public_address = "addr:127.0.0.1:1337#8F78"

[torii]
address = "addr:127.0.0.1:8080#8942"

[genesis]
public_key = "{genesis_public_key}"
file = "{signed}"
manifest_json = "{manifest}"
expected_hash = "{GENESIS_EXPECTED_HASH_PLACEHOLDER}"

[streaming]
identity_public_key = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
identity_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"
"#,
            chain_discriminant = iroha_data_model::account::address::chain_discriminant(),
            manifest = manifest.display(),
            signed = signed.display(),
        )
    }
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
            "--kagemusha-mint-finality-parameters".to_owned(),
            "/tmp/kagemusha.json".to_owned(),
            "--profile".to_owned(),
            "iroha3-dev".to_owned(),
            "--consensus-mode".to_owned(),
            "npos".to_owned(),
            "--vrf-seed-hex".to_owned(),
            "ab".repeat(32),
            "default".to_owned(),
        ];
        let parsed = parse_generate_args(&args).expect("parse mock kagami args");
        assert_eq!(parsed.ivm_dir, PathBuf::from("./ivm"));
        assert_eq!(parsed.genesis_public_key, key_pair.public_key().to_string());
        assert_eq!(parsed.chain_id, "local-chain");
        assert_eq!(parsed.consensus_mode, SumeragiConsensusMode::Npos);
    }
    #[test]
    fn generate_parser_rejects_missing_duplicate_and_trailing_arguments() {
        let key = KeyPair::random().public_key().to_string();
        let base = vec![
            "--ivm-dir".to_owned(),
            ".".to_owned(),
            "--genesis-public-key".to_owned(),
            key,
            "--chain-id".to_owned(),
            "strict-chain".to_owned(),
            "--consensus-mode".to_owned(),
            "permissioned".to_owned(),
            "--kagemusha-mint-finality-parameters".to_owned(),
            "/tmp/kagemusha.json".to_owned(),
            "default".to_owned(),
        ];
        for flag in [
            "--ivm-dir",
            "--genesis-public-key",
            "--chain-id",
            "--consensus-mode",
            "--kagemusha-mint-finality-parameters",
        ] {
            let mut missing = base.clone();
            let index = missing.iter().position(|arg| arg == flag).expect("flag");
            missing.drain(index..=index + 1);
            assert!(
                parse_generate_args(&missing).is_err(),
                "required generate flag must not default"
            );
        }
        let mut missing_default = base.clone();
        missing_default.pop();
        assert!(
            parse_generate_args(&missing_default).is_err(),
            "terminal default is required"
        );

        let mut duplicate = base.clone();
        duplicate.splice(
            duplicate.len() - 1..duplicate.len() - 1,
            ["--chain-id".to_owned(), "strict-chain".to_owned()],
        );
        assert!(
            parse_generate_args(&duplicate).is_err(),
            "duplicate flags must fail"
        );

        let mut trailing = base;
        trailing.push("ignored".to_owned());
        assert!(
            parse_generate_args(&trailing).is_err(),
            "default must be terminal"
        );
    }
    #[test]
    fn generate_and_verify_parsers_validate_profiles_and_vrf_seeds() {
        let key = KeyPair::random().public_key().to_string();
        let generate = |profile: Option<&str>, seed: Option<&str>| {
            let mut args = vec![
                "--ivm-dir".to_owned(),
                ".".to_owned(),
                "--genesis-public-key".to_owned(),
                key.clone(),
                "--chain-id".to_owned(),
                "strict-chain".to_owned(),
                "--consensus-mode".to_owned(),
                "npos".to_owned(),
                "--kagemusha-mint-finality-parameters".to_owned(),
                "/tmp/kagemusha.json".to_owned(),
            ];
            if let Some(profile) = profile {
                args.extend(["--profile".to_owned(), profile.to_owned()]);
            }
            if let Some(seed) = seed {
                args.extend(["--vrf-seed-hex".to_owned(), seed.to_owned()]);
            }
            args.push("default".to_owned());
            args
        };
        let valid_seed = "01".repeat(32);
        parse_generate_args(&generate(Some("iroha3-taira"), Some(&valid_seed)))
            .expect("Taira profile with canonical seed");
        assert!(
            parse_generate_args(&generate(Some("unknown"), None)).is_err(),
            "unknown profile must fail"
        );
        assert!(
            parse_generate_args(&generate(None, Some(&valid_seed))).is_err(),
            "seed without profile must fail"
        );
        assert!(
            parse_generate_args(&generate(Some("iroha3-taira"), None)).is_err(),
            "Taira profile requires seed"
        );
        for invalid in ["01".repeat(31), format!("{}gg", "01".repeat(31))] {
            assert!(
                parse_generate_args(&generate(Some("iroha3-dev"), Some(&invalid))).is_err(),
                "invalid seed must fail"
            );
        }

        let verify_args = vec![
            "--profile".to_owned(),
            "iroha3-taira".to_owned(),
            "--genesis".to_owned(),
            "/tmp/genesis.json".to_owned(),
            "--vrf-seed-hex".to_owned(),
            valid_seed,
        ];
        parse_verify_args(&verify_args).expect("strict verify arguments");
        let mut missing_seed_value = verify_args;
        missing_seed_value.pop();
        assert!(
            parse_verify_args(&missing_seed_value).is_err(),
            "verify seed flag must include a value"
        );
    }
    #[test]
    fn generate_emits_manifest_with_requested_chain_id() {
        let key_pair = KeyPair::random();
        let ivm_dir = tempfile::tempdir().expect("tempdir");
        let topology = test_topology();
        let kagemusha_mint_finality = test_kagemusha_mint_finality_parameters(&topology);
        let manifest = kagami_default_manifest_json(
            key_pair.public_key(),
            ivm_dir.path(),
            "custom-chain",
            SumeragiConsensusMode::Permissioned,
            &kagemusha_mint_finality,
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
        let parameters_path = ivm_dir.path().join("kagemusha.json");
        let topology = test_topology();
        let kagemusha_mint_finality = test_kagemusha_mint_finality_parameters(&topology);
        fs::write(
            &parameters_path,
            norito::json::to_vec_pretty(&kagemusha_mint_finality)
                .expect("encode mint-finality parameters"),
        )
        .expect("write mint-finality parameters");
        let args = vec![
            "--ivm-dir".to_owned(),
            ivm_dir.path().display().to_string(),
            "--genesis-public-key".to_owned(),
            key_pair.public_key().to_string(),
            "--chain-id".to_owned(),
            "supervisor-chain".to_owned(),
            "--consensus-mode".to_owned(),
            "permissioned".to_owned(),
            "--kagemusha-mint-finality-parameters".to_owned(),
            parameters_path.display().to_string(),
            "default".to_owned(),
        ];
        generate(args).expect("generate manifest via mock kagami");
    }
    #[test]
    fn genesis_public_key_requires_canonical_encoding() {
        let canonical = KeyPair::random().public_key().to_string();
        assert_eq!(
            parse_genesis_public_key(&canonical)
                .expect("canonical public key")
                .to_string(),
            canonical
        );

        let raw_hex = canonical
            .strip_prefix("ed0120")
            .expect("test key uses canonical Ed25519 prefix");
        for invalid in [raw_hex.to_owned(), format!("0x{raw_hex}")] {
            assert!(
                parse_genesis_public_key(&invalid).is_err(),
                "raw compatibility encodings must be rejected"
            );
        }
    }
    #[test]
    fn parse_sign_args_accepts_current_supervisor_flags() {
        let args = vec![
            "/tmp/genesis.json".to_owned(),
            "--out-file".to_owned(),
            "/tmp/genesis.signed.nrt".to_owned(),
            "--bound-manifest-out".to_owned(),
            "/tmp/genesis.bound.json".to_owned(),
            "--expected-hash-out".to_owned(),
            "/tmp/genesis.expected_hash".to_owned(),
            "--private-key-file".to_owned(),
            "/tmp/genesis.key".to_owned(),
            "--config".to_owned(),
            "/tmp/peer.toml".to_owned(),
        ];
        assert_eq!(
            parse_sign_args(&args).expect("parse mock kagami sign args"),
            SignArgs {
                manifest_path: PathBuf::from("/tmp/genesis.json"),
                out_file: PathBuf::from("/tmp/genesis.signed.nrt"),
                bound_manifest_out: PathBuf::from("/tmp/genesis.bound.json"),
                expected_hash_out: PathBuf::from("/tmp/genesis.expected_hash"),
                private_key_file: PathBuf::from("/tmp/genesis.key"),
                config_file: PathBuf::from("/tmp/peer.toml"),
            }
        );
        let mut duplicate = args.clone();
        duplicate.extend([
            "--out-file".to_owned(),
            "/tmp/duplicate.signed.nrt".to_owned(),
        ]);
        let error = parse_sign_args(&duplicate).expect_err("duplicate sign flag must be rejected");
        assert!(error.to_string().contains("duplicate"));

        let mut retired = args;
        retired.extend(["--consensus-mode".to_owned(), "permissioned".to_owned()]);
        let error = parse_sign_args(&retired)
            .expect_err("retired sign-time consensus override must be rejected");
        assert!(error.to_string().contains("unsupported argument"));
    }
    #[test]
    fn sign_publishes_block_before_bound_manifest() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = temp.path().join("genesis.json");
        let signed = temp.path().join("genesis.signed.nrt");
        let bound = temp.path().join("genesis.bound.json");
        let expected_hash = temp.path().join("genesis.expected_hash");
        let private_key = temp.path().join("genesis.key");
        let config = temp.path().join("peer.toml");
        let key_pair = KeyPair::random();
        let initial_topology = test_topology();
        let initial_kagemusha_mint_finality =
            test_kagemusha_mint_finality_parameters(&initial_topology);
        let manifest_json = with_valid_topology(
            kagami_default_manifest_json(
                key_pair.public_key(),
                temp.path(),
                "mock-sign-chain",
                SumeragiConsensusMode::Permissioned,
                &initial_kagemusha_mint_finality,
            )
            .expect("build manifest"),
        );
        fs::write(&manifest, manifest_json.as_bytes()).expect("write manifest");
        fs::write(
            &private_key,
            format!("{}\n", ExposedPrivateKey(key_pair.private_key().clone())),
        )
        .expect("write private key");
        fs::write(
            &config,
            peer_config("mock-sign-chain", key_pair.public_key(), &manifest, &signed),
        )
        .expect("write config");
        sign(vec![
            manifest.display().to_string(),
            "--out-file".to_owned(),
            signed.display().to_string(),
            "--bound-manifest-out".to_owned(),
            bound.display().to_string(),
            "--expected-hash-out".to_owned(),
            expected_hash.display().to_string(),
            "--private-key-file".to_owned(),
            private_key.display().to_string(),
            "--config".to_owned(),
            config.display().to_string(),
        ])
        .expect("sign with mock kagami");
        let wire = fs::read(&signed).expect("read signed output");
        let block = decode_framed_signed_block(&wire).expect("decode signed output");
        assert_eq!(
            fs::read(bound).expect("read bound output"),
            manifest_json.as_bytes()
        );
        assert_eq!(
            fs::read_to_string(&expected_hash).expect("read exact hash"),
            format!("{}\n", NetworkId::from_genesis_hash(block.hash()))
        );
        let identity_path = expected_hash.to_string_lossy().replace('\\', "\\\\");
        let exact_config =
            peer_config("mock-sign-chain", key_pair.public_key(), &manifest, &signed).replace(
                &format!("expected_hash = \"{GENESIS_EXPECTED_HASH_PLACEHOLDER}\""),
                &format!("expected_hash_file = \"{identity_path}\""),
            );
        fs::write(&config, exact_config).expect("bind genesis identity file in config");
        let (expected_da_policies, expected_confidential_policy) =
            kagami_stub_genesis_policies_from_config(&config)
                .expect("derive exact signing policies from config");
        assert_eq!(
            block.da_proof_policies(),
            Some(&expected_da_policies),
            "decoded signed output must carry the config-derived DA policy"
        );
        assert_eq!(
            block
                .header()
                .confidential_features()
                .and_then(|digest| digest.zk_policy_hash),
            Some(expected_confidential_policy),
            "decoded signed output must carry the config-derived confidential policy"
        );
    }
    #[test]
    fn sign_output_failure_preserves_existing_bound_manifest() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = temp.path().join("genesis.json");
        let bound = temp.path().join("genesis.bound.json");
        let expected_hash = temp.path().join("genesis.expected_hash");
        let private_key = temp.path().join("genesis.key");
        let config = temp.path().join("peer.toml");
        let key_pair = KeyPair::random();
        let initial_topology = test_topology();
        let initial_kagemusha_mint_finality =
            test_kagemusha_mint_finality_parameters(&initial_topology);
        let manifest_json = with_valid_topology(
            kagami_default_manifest_json(
                key_pair.public_key(),
                temp.path(),
                "mock-sign-failure-chain",
                SumeragiConsensusMode::Permissioned,
                &initial_kagemusha_mint_finality,
            )
            .expect("build manifest"),
        );
        fs::write(&manifest, manifest_json).expect("write manifest");
        fs::write(&bound, b"sentinel").expect("write bound sentinel");
        fs::write(
            &private_key,
            format!("{}\n", ExposedPrivateKey(key_pair.private_key().clone())),
        )
        .expect("write private key");
        fs::write(
            &config,
            peer_config(
                "mock-sign-failure-chain",
                key_pair.public_key(),
                &manifest,
                &temp.path().join("missing/genesis.signed.nrt"),
            ),
        )
        .expect("write config");
        let _ = sign(vec![
            manifest.display().to_string(),
            "--out-file".to_owned(),
            temp.path()
                .join("missing/genesis.signed.nrt")
                .display()
                .to_string(),
            "--bound-manifest-out".to_owned(),
            bound.display().to_string(),
            "--expected-hash-out".to_owned(),
            expected_hash.display().to_string(),
            "--private-key-file".to_owned(),
            private_key.display().to_string(),
            "--config".to_owned(),
            config.display().to_string(),
        ])
        .expect_err("missing output parent should fail");
        assert_eq!(
            fs::read(bound).expect("read bound sentinel"),
            b"sentinel",
            "bound manifest must only publish after signed output succeeds"
        );
        assert!(
            !expected_hash.exists(),
            "expected hash must only publish after the block and bound manifest"
        );
    }
}
