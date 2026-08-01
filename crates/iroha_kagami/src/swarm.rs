use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use clap::Args as ClapArgs;
use color_eyre::eyre::{WrapErr as _, ensure, eyre};
use iroha_config::{base::toml::TomlSource, parameters::actual};
use iroha_crypto::{HashOf, PublicKey};
use iroha_data_model::{
    ChainId,
    account::address::ChainDiscriminantGuard,
    block::{BlockHeader, decode_framed_signed_block},
    isi::register::RegisterPeerWithPop,
    parameter::system::SumeragiConsensusMode,
    prelude::AccountId,
    transaction::Executable,
};
use iroha_genesis::RawGenesisTransaction;
use iroha_swarm::{PeerOverride, PreparedGenesisArtifacts, PreparedValidator};

use crate::{
    Outcome, RunArgs,
    genesis::{
        ConsensusPolicy, build_line_from_env, ensure_npos_parameters,
        validate_consensus_mode_for_line,
    },
    tui,
};

/// Docker Compose configuration generator for Iroha.
#[allow(clippy::struct_excessive_bools)]
#[derive(ClapArgs, Debug, Clone)]
pub struct Args {
    /// Number of peer services in the configuration.
    #[arg(long, short, value_name = "COUNT")]
    peers: std::num::NonZeroU16,
    /// Enable deterministic development mode with this UTF-8 validator seed.
    ///
    /// When omitted, `--config-dir` must be an authoritative prepared bundle containing
    /// `peerN.toml`, signed genesis, verifier-key, and exact-hash files. Production workflows
    /// should omit this option so Compose cannot generate identities that diverge from genesis.
    #[arg(long, short)]
    seed: Option<String>,
    /// Includes a healthcheck for every service in the configuration.
    ///
    /// Healthchecks use predefined settings.
    ///
    /// For more details on healthcheck configuration in Docker Compose files, see:
    /// <https://docs.docker.com/compose/compose-file/compose-file-v3/#healthcheck>
    #[arg(long, short = 'H')]
    healthcheck: bool,
    /// Authoritative prepared validator/genesis bundle, or development manifest directory.
    ///
    /// Normal mode requires `genesis.json`, `peer0.toml` through `peerN.toml`,
    /// `genesis.signed.nrt`, `genesis.public_key`, and `genesis.expected_hash`. Kagami validates
    /// their signer, exact hash, validator roster, and PoPs together. With `--seed`, only
    /// `genesis.json` is read and runtime artifact paths are supplied explicitly through the
    /// generated manifest's `IROHA_GENESIS_*_FILE` variables.
    #[arg(long, short, value_name = "DIR")]
    config_dir: PathBuf,
    /// Optional TOML file describing peer names and port mappings.
    /// Only available with deterministic development `--seed` mode.
    ///
    /// The file must contain an array named `peers`, for example:
    ///
    /// ```toml
    /// [[peers]]
    /// name = "alpha"
    /// p2p_port = 2000
    /// api_port = 9000
    /// [[peers]]
    /// name = "beta"
    /// p2p_port = 2001
    /// api_port = 9001
    /// ```
    #[arg(long, value_name = "FILE", requires = "seed")]
    peer_config: Option<PathBuf>,
    /// Docker image used by the peer services.
    ///
    /// By default, the image is pulled from Docker Hub if not cached.
    /// Pass the `--build` option to build the image from a Dockerfile instead.
    ///
    /// **Note**: Swarm only guarantees that the Docker Compose configuration it generates
    /// is compatible with the same Git revision it is built from itself. Therefore, if the
    /// specified image is not compatible with the version of Swarm you are running,
    /// the generated configuration might not work.
    #[arg(long, short, value_name = "NAME")]
    image: String,
    /// Build the image from the Dockerfile in the specified directory.
    /// Do not rebuild if the image has been cached.
    ///
    /// The provided path is resolved relative to the current working directory.
    #[arg(long, short, value_name = "DIR")]
    build: Option<PathBuf>,
    /// Always pull or rebuild the image even if it is cached locally.
    #[arg(long)]
    no_cache: bool,
    /// Path to the target Compose configuration file.
    ///
    /// If the file exists, the app will prompt its overwriting. If the TTY is not
    /// interactive, the app will stop execution with a non-zero exit code.
    /// To overwrite the file anyway, pass the `--force` flag.
    #[arg(long, short, value_name = "FILE")]
    out_file: PathBuf,
    /// Print the generated configuration to stdout
    /// instead of writing it to the target file.
    ///
    /// Note that the target path still needs to be provided, as it is used to resolve paths.
    #[arg(long, short = 'P', conflicts_with = "force")]
    print: bool,
    /// Overwrite the target file if it already exists.
    #[arg(long, short = 'F')]
    force: bool,
    /// Do not include the banner with the generation notice in the file.
    ///
    /// The banner includes the seed to help with reproducibility.
    #[arg(long)]
    no_banner: bool,
}

impl Args {
    /// If this returns `Ok(true)`, then Swarm is allowed to proceed.
    fn user_allows_overwrite(&self) -> Result<bool, inquire::InquireError> {
        if self.out_file.exists() && !self.force {
            use owo_colors::OwoColorize;
            return inquire::Confirm::new(&format!(
                "File {} already exists. Overwrite it?",
                self.out_file.display().blue().bold()
            ))
            .with_help_message("Pass the `--force` flag to overwrite the file anyway.")
            .with_default(false)
            .prompt();
        }
        Ok(true)
    }
}

#[derive(Debug)]
struct PreparedBundle {
    chain: ChainId,
    validators: Vec<PreparedValidator>,
    signed_block: PathBuf,
    public_key: PathBuf,
    expected_hash: PathBuf,
}

struct ValidatedGenesis {
    block: iroha_data_model::block::SignedBlock,
    public_key: PublicKey,
    expected_hash: HashOf<BlockHeader>,
    validator_pops: BTreeMap<PublicKey, Vec<u8>>,
}

fn read_exact_record(path: &Path, label: &str) -> color_eyre::Result<String> {
    let record = fs::read_to_string(path)
        .wrap_err_with(|| format!("read {label} record {}", path.display()))?;
    let payload = record.strip_suffix('\n').ok_or_else(|| {
        eyre!(
            "{label} record {} must end in exactly one newline",
            path.display()
        )
    })?;
    ensure!(
        !payload.is_empty()
            && payload.trim() == payload
            && !payload.chars().any(char::is_whitespace),
        "{label} record {} must contain exactly one non-empty canonical line",
        path.display()
    );
    Ok(payload.to_owned())
}

fn validate_prepared_genesis(
    signed_block: &Path,
    public_key_path: &Path,
    expected_hash_path: &Path,
) -> color_eyre::Result<ValidatedGenesis> {
    let public_record = read_exact_record(public_key_path, "genesis public-key")?;
    let public_key = public_record
        .parse::<PublicKey>()
        .wrap_err("parse prepared genesis public key")?;
    ensure!(
        public_key.to_string() == public_record,
        "prepared genesis public-key record is not canonical"
    );

    let expected_record = read_exact_record(expected_hash_path, "genesis expected-hash")?;
    let expected_hash = expected_record
        .parse::<HashOf<BlockHeader>>()
        .wrap_err("parse prepared exact genesis hash")?;
    ensure!(
        expected_hash.to_string() == expected_record,
        "prepared genesis expected-hash record is not canonical lowercase marked hex"
    );

    let signed = fs::read(signed_block)
        .wrap_err_with(|| format!("read signed genesis body {}", signed_block.display()))?;
    ensure!(
        !signed.is_empty(),
        "signed genesis body {} is empty",
        signed_block.display()
    );
    iroha_genesis::init_instruction_registry();
    let block = decode_framed_signed_block(&signed)
        .wrap_err("decode prepared canonical signed genesis body")?;
    ensure!(
        block.hash() == expected_hash,
        "prepared signed genesis body hashes to {}, expected {}",
        block.hash(),
        expected_hash
    );

    let first = block
        .external_transactions()
        .next()
        .ok_or_else(|| eyre!("prepared signed genesis contains no external transactions"))?;
    let embedded_signer = first.authority().try_signatory().ok_or_else(|| {
        eyre!("prepared genesis authority must be one canonical single-key account")
    })?;
    ensure!(
        embedded_signer == &public_key,
        "prepared genesis signer {embedded_signer} differs from verifier key {public_key}"
    );

    let mut signatures = block.signatures();
    let signature = signatures
        .next()
        .ok_or_else(|| eyre!("prepared signed genesis has no block signature"))?;
    ensure!(
        signature.index() == 0 && signatures.next().is_none(),
        "prepared signed genesis must have exactly one block signature at index 0"
    );
    signature
        .signature()
        .verify_hash(&public_key, block.hash())
        .wrap_err("verify prepared genesis block signature")?;
    for transaction in block.external_transactions() {
        transaction
            .verify_signature()
            .wrap_err("verify prepared genesis transaction signature")?;
    }

    let mut validator_pops = BTreeMap::new();
    for transaction in block.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            continue;
        };
        for instruction in instructions {
            let Some(register) = instruction.as_any().downcast_ref::<RegisterPeerWithPop>() else {
                continue;
            };
            let public_key = register.peer.public_key().clone();
            ensure!(
                validator_pops
                    .insert(public_key.clone(), register.pop.clone())
                    .is_none(),
                "prepared genesis registers validator {public_key} more than once"
            );
        }
    }
    ensure!(
        !validator_pops.is_empty(),
        "prepared genesis contains no RegisterPeerWithPop validator roster"
    );

    Ok(ValidatedGenesis {
        block,
        public_key,
        expected_hash,
        validator_pops,
    })
}

fn prepared_peer_config_paths(
    config_dir: &Path,
    count: std::num::NonZeroU16,
) -> color_eyre::Result<Vec<PathBuf>> {
    let mut discovered = BTreeSet::new();
    for entry in fs::read_dir(config_dir)
        .wrap_err_with(|| format!("read prepared bundle directory {}", config_dir.display()))?
    {
        let entry = entry.wrap_err("read prepared bundle directory entry")?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(index) = name
            .strip_prefix("peer")
            .and_then(|suffix| suffix.strip_suffix(".toml"))
        else {
            continue;
        };
        if index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let index = index
            .parse::<u16>()
            .wrap_err_with(|| format!("prepared peer filename `{name}` has an invalid index"))?;
        ensure!(
            name == format!("peer{index}.toml"),
            "prepared peer filename `{name}` is not canonical"
        );
        ensure!(
            discovered.insert(index),
            "prepared bundle contains duplicate peer index {index}"
        );
    }
    let expected = (0..count.get()).collect::<BTreeSet<_>>();
    ensure!(
        discovered == expected,
        "prepared validator roster files are {:?}, expected {:?}",
        discovered,
        expected
    );
    Ok(expected
        .into_iter()
        .map(|index| config_dir.join(format!("peer{index}.toml")))
        .collect())
}

fn parse_prepared_peer_config(path: &Path) -> color_eyre::Result<actual::Root> {
    let raw = fs::read_to_string(path)
        .wrap_err_with(|| format!("read prepared validator config {}", path.display()))?;
    let table = raw
        .parse::<toml::Table>()
        .wrap_err_with(|| format!("parse prepared validator config {}", path.display()))?;
    let chain_discriminant = table
        .get("chain_discriminant")
        .and_then(toml::Value::as_integer)
        .and_then(|value| u16::try_from(value).ok());
    let _chain_discriminant = chain_discriminant.map(ChainDiscriminantGuard::enter);
    let source = TomlSource::from_file(path)
        .wrap_err_with(|| format!("load prepared validator config {}", path.display()))?;
    actual::Root::from_toml_source(source).map_err(|error| {
        eyre!(
            "prepared validator config {} is invalid: {error:?}",
            path.display()
        )
    })
}

fn load_prepared_bundle(
    config_dir: &Path,
    count: std::num::NonZeroU16,
) -> color_eyre::Result<PreparedBundle> {
    let signed_block = config_dir.join("genesis.signed.nrt");
    let public_key_path = config_dir.join(crate::localnet::GENESIS_PUBLIC_KEY_FILE);
    let expected_hash_path = config_dir.join(crate::localnet::GENESIS_EXPECTED_HASH_FILE);
    let validated =
        validate_prepared_genesis(&signed_block, &public_key_path, &expected_hash_path)?;
    let config_paths = prepared_peer_config_paths(config_dir, count)?;

    let mut chain = None;
    let mut validators = Vec::with_capacity(config_paths.len());
    let mut validator_keys = BTreeSet::new();
    for (index, path) in config_paths.iter().enumerate() {
        let config = parse_prepared_peer_config(path)?;
        if let Some(expected_chain) = chain.as_ref() {
            ensure!(
                &config.common.chain == expected_chain,
                "prepared validator config {} uses chain {}, expected {}",
                path.display(),
                config.common.chain,
                expected_chain
            );
        } else {
            chain = Some(config.common.chain.clone());
        }
        ensure!(
            config.genesis.public_key == validated.public_key,
            "prepared validator config {} has a different genesis verifier key",
            path.display()
        );
        ensure!(
            config.genesis.expected_hash == validated.expected_hash,
            "prepared validator config {} has genesis hash {}, expected {}",
            path.display(),
            config.genesis.expected_hash,
            validated.expected_hash
        );
        ensure!(
            config.genesis.file.is_some(),
            "prepared validator config {} does not select a signed genesis body",
            path.display()
        );

        let trusted = config.common.trusted_peers.value();
        ensure!(
            trusted.pops == validated.validator_pops,
            "prepared validator config {} PoP roster differs from signed genesis",
            path.display()
        );
        let trusted_keys = std::iter::once(&trusted.myself)
            .chain(trusted.others.iter())
            .map(|peer| peer.id().public_key().clone())
            .collect::<BTreeSet<_>>();
        ensure!(
            trusted_keys
                == validated
                    .validator_pops
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>(),
            "prepared validator config {} trusted roster differs from signed genesis",
            path.display()
        );

        let key_pair = config.common.key_pair.clone();
        let validator_key = key_pair.public_key().clone();
        ensure!(
            trusted.myself.id().public_key() == &validator_key,
            "prepared validator config {} local identity differs from its trusted-roster self",
            path.display()
        );
        ensure!(
            validator_keys.insert(validator_key.clone()),
            "prepared validator identity {validator_key} is duplicated"
        );
        let pop = validated
            .validator_pops
            .get(&validator_key)
            .cloned()
            .ok_or_else(|| {
                eyre!(
                    "prepared validator config {} identity {validator_key} is absent from signed genesis",
                    path.display()
                )
            })?;
        iroha_crypto::bls_normal_pop_verify(&validator_key, &pop)
            .wrap_err_with(|| format!("verify prepared validator {index} PoP"))?;
        validators.push(PreparedValidator {
            name: format!("irohad{index}"),
            p2p_port: config.network.address.value().port(),
            api_port: config.torii.address.value().port(),
            key_pair,
            pop,
        });
    }
    ensure!(
        validator_keys
            == validated
                .validator_pops
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
        "prepared validator config identities differ from the signed genesis roster"
    );
    let chain = chain.expect("non-empty prepared bundle has a chain id");
    iroha_core::validate_genesis_block(
        &validated.block,
        &AccountId::new(validated.public_key.clone()),
        &chain,
    )
    .map_err(|error| eyre!("prepared signed genesis failed full validation: {error}"))?;

    Ok(PreparedBundle {
        chain,
        validators,
        signed_block,
        public_key: public_key_path,
        expected_hash: expected_hash_path,
    })
}

impl<T: Write> RunArgs<T> for Args {
    #[allow(clippy::too_many_lines)]
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        // let args: Args = <Args as clap::Parser>::parse();
        let args = self;

        if !args.print && !args.user_allows_overwrite()? {
            return Ok(());
        }

        let build_line = build_line_from_env();
        let genesis_path = args.config_dir.join("genesis.json");
        let manifest = RawGenesisTransaction::from_path(&genesis_path).wrap_err_with(|| {
            eyre!(
                "failed to parse genesis manifest at {}",
                genesis_path.display()
            )
        })?;
        let manifest_mode = manifest.consensus_mode();
        validate_consensus_mode_for_line(build_line, manifest_mode, ConsensusPolicy::Any)?;
        if matches!(manifest_mode, SumeragiConsensusMode::Npos) {
            ensure_npos_parameters(&manifest)?;
        }

        let peer_overrides = match &args.peer_config {
            Some(path) => Some(load_peer_overrides(path)?),
            None => None,
        };

        tui::status("Composing Docker deployment manifest");
        let prepared_artifacts;
        let swarm = if let Some(seed) = args.seed.as_deref() {
            prepared_artifacts = None;
            iroha_swarm::Swarm::deterministic_dev(
                args.peers,
                seed.as_bytes(),
                args.healthcheck,
                &args.image,
                args.build.as_deref(),
                args.no_cache,
                &args.out_file,
                peer_overrides,
            )?
        } else {
            let PreparedBundle {
                chain,
                validators,
                signed_block,
                public_key,
                expected_hash,
            } = load_prepared_bundle(&args.config_dir, args.peers)?;
            let artifacts = PreparedGenesisArtifacts {
                signed_block: &signed_block,
                public_key: &public_key,
                expected_hash: &expected_hash,
            };
            let swarm = iroha_swarm::Swarm::from_prepared(
                chain,
                validators,
                artifacts,
                args.healthcheck,
                &args.image,
                args.build.as_deref(),
                args.no_cache,
                &args.out_file,
            )?;
            prepared_artifacts = Some((signed_block, public_key, expected_hash));
            swarm
        };
        let schema = swarm.build();

        let mut file;

        let manifest_writer: &mut dyn Write = if args.print {
            writer
        } else {
            use color_eyre::eyre::Context;
            file = std::fs::File::create(&args.out_file)
                .wrap_err("Could not open the target file.")?;
            &mut file
        };

        let banner = if args.no_banner {
            None
        } else {
            let mut lines = vec![
                "Generated by `kagami docker`.".to_owned(),
                "You should not edit this manually.".to_owned(),
            ];
            if let Some(seed) = args.seed.as_ref() {
                lines.push(format!("Seed: {seed}"));
            }
            Some(lines)
        };
        let banner_refs = banner
            .as_ref()
            .map(|lines| lines.iter().map(String::as_str).collect::<Vec<_>>());

        schema.write(
            &mut std::io::BufWriter::new(manifest_writer),
            banner_refs.as_deref(),
        )?;

        if !args.print {
            writeln!(
                writer,
                "compose_path: {}",
                swarm.absolute_target_path().display()
            )?;
            writeln!(writer, "config_dir: {}", args.config_dir.display())?;
            writeln!(writer, "image: {}", args.image)?;
            writeln!(writer, "peers: {}", args.peers)?;
            writeln!(
                writer,
                "consensus_mode: {}",
                crate::localnet::consensus_mode_label(manifest_mode)
            )?;
            if let Some((signed_block, public_key, expected_hash)) = prepared_artifacts.as_ref() {
                writeln!(writer, "genesis_signed: {}", signed_block.display())?;
                writeln!(writer, "genesis_public_key: {}", public_key.display())?;
                writeln!(writer, "genesis_expected_hash: {}", expected_hash.display())?;
            } else {
                writeln!(
                    writer,
                    "genesis_public_key_file_env: IROHA_GENESIS_PUBLIC_KEY_FILE"
                )?;
                writeln!(writer, "genesis_signed_file_env: IROHA_GENESIS_SIGNED_FILE")?;
                writeln!(
                    writer,
                    "genesis_expected_hash_file_env: IROHA_GENESIS_EXPECTED_HASH_FILE"
                )?;
            }
            writeln!(
                writer,
                "next: docker compose -f {} up",
                args.out_file.display()
            )?;
        }
        tui::success("Compose manifest ready");

        Ok(())
    }
}

fn load_peer_overrides(path: &Path) -> color_eyre::Result<Vec<PeerOverride>> {
    ensure!(
        path.exists(),
        "peer configuration {} does not exist",
        path.display()
    );
    ensure!(
        path.is_file(),
        "peer configuration {} is not a file",
        path.display()
    );
    let contents = fs::read_to_string(path)
        .wrap_err_with(|| eyre!("failed to read peer configuration at {}", path.display()))?;
    parse_peer_override_toml(&contents)
        .wrap_err_with(|| eyre!("failed to parse peer configuration at {}", path.display()))
}

fn parse_peer_override_toml(input: &str) -> color_eyre::Result<Vec<PeerOverride>> {
    let value: toml::Value =
        toml::from_str(input).wrap_err("peer configuration is not valid TOML")?;
    let peers = value
        .get("peers")
        .ok_or_else(|| eyre!("peer configuration must define [[peers]] entries"))?
        .as_array()
        .ok_or_else(|| eyre!("`peers` must be an array of tables"))?;

    ensure!(
        !peers.is_empty(),
        "peer configuration must list at least one peer"
    );

    peers
        .iter()
        .map(|entry| -> color_eyre::Result<PeerOverride> {
            let table = entry
                .as_table()
                .ok_or_else(|| eyre!("each [[peers]] entry must be a table"))?;
            let name = table
                .get("name")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| eyre!("peer entry is missing `name`"))?;
            let p2p_port = parse_port(table, "p2p_port")?;
            let api_port = parse_port(table, "api_port")?;
            Ok(PeerOverride {
                name: name.to_owned(),
                p2p_port,
                api_port,
            })
        })
        .collect()
}

fn parse_port(table: &toml::Table, field: &str) -> color_eyre::Result<u16> {
    let raw = table
        .get(field)
        .ok_or_else(|| eyre!("peer entry is missing `{field}`"))?;
    let value = raw
        .as_integer()
        .ok_or_else(|| eyre!("`{field}` must be an integer"))?;
    let port = u16::try_from(value).map_err(|_| eyre!("`{field}` must fit into a u16"))?;
    Ok(port)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{BufWriter, Write},
        num::NonZeroU16,
        path::{Path, PathBuf},
    };

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        ChainId,
        parameter::{
            Parameter,
            system::{SumeragiConsensusMode, SumeragiNposParameters},
        },
    };
    use iroha_genesis::GenesisBuilder;
    use iroha_version::BuildLine;

    use super::{Args, load_peer_overrides, load_prepared_bundle, parse_peer_override_toml};
    use crate::{RunArgs, localnet::LocalnetOptions};

    fn generate_prepared_bundle(root: &Path) -> PathBuf {
        let bundle = root.join("prepared-bundle");
        let options = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("kagami-compose-prepared-bundle".to_owned()),
            bind_host: "127.0.0.1".to_owned(),
            public_host: "127.0.0.1".to_owned(),
            base_api_port: 19_080,
            base_p2p_port: 23_337,
            out_dir: bundle.clone(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        crate::localnet::generate_localnet(&options, &mut BufWriter::new(Vec::new()))
            .expect("generate authoritative prepared localnet bundle");
        bundle
    }

    #[test]
    fn run_succeeds_without_banner() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let config_dir = temp_dir.path().join("cfg");
        fs::create_dir_all(&config_dir).expect("create config dir");
        write_minimal_genesis(&config_dir.join("genesis.json"));
        let args = Args {
            peers: NonZeroU16::new(1).expect("non-zero"),
            seed: Some("swarm-no-banner-dev".to_owned()),
            healthcheck: false,
            config_dir,
            peer_config: None,
            image: "hyperledger/iroha:dev".to_owned(),
            build: None,
            no_cache: false,
            out_file: temp_dir.path().join("docker-compose.yml"),
            print: true,
            force: false,
            no_banner: true,
        };

        let mut buffer = Vec::new();
        let mut writer = BufWriter::new(&mut buffer);
        args.run(&mut writer)
            .expect("`Args::run` should succeed without banner");
        writer.flush().expect("flush buffer");
        drop(writer);

        let output = String::from_utf8(buffer).expect("output should be UTF-8");
        assert!(!output.contains("Generated by `kagami docker`."));
        assert!(!output.contains("Seed:"));
    }

    #[test]
    fn file_output_reports_required_runtime_genesis_artifacts() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let config_dir = temp_dir.path().join("cfg");
        fs::create_dir_all(&config_dir).expect("create config dir");
        write_minimal_genesis(&config_dir.join("genesis.json"));
        let compose_path = temp_dir.path().join("docker-compose.yml");
        let args = Args {
            peers: NonZeroU16::new(1).expect("non-zero"),
            seed: Some("swarm-artifact-summary-dev".to_owned()),
            healthcheck: false,
            config_dir,
            peer_config: None,
            image: "hyperledger/iroha:dev".to_owned(),
            build: None,
            no_cache: false,
            out_file: compose_path.clone(),
            print: false,
            force: false,
            no_banner: true,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("write Compose manifest");
        let output = String::from_utf8(writer.into_inner().expect("flush summary"))
            .expect("summary is UTF-8");

        assert!(compose_path.is_file());
        assert!(output.contains("genesis_public_key_file_env: IROHA_GENESIS_PUBLIC_KEY_FILE"));
        assert!(output.contains("genesis_signed_file_env: IROHA_GENESIS_SIGNED_FILE"));
        assert!(
            output.contains("genesis_expected_hash_file_env: IROHA_GENESIS_EXPECTED_HASH_FILE")
        );
        assert!(!output.contains("IROHA_GENESIS_PRIVATE_KEY_FILE"));
        assert!(output.contains("next: docker compose"));
    }

    #[test]
    fn prepared_bundle_renders_exact_read_only_runtime_inputs() {
        let temp_dir = tempfile::tempdir().expect("prepared bundle temp dir");
        let config_dir = generate_prepared_bundle(temp_dir.path());
        let args = Args {
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: None,
            healthcheck: false,
            config_dir,
            peer_config: None,
            image: "hyperledger/iroha:dev".to_owned(),
            build: None,
            no_cache: false,
            out_file: temp_dir.path().join("deployment/docker-compose.yml"),
            print: true,
            force: false,
            no_banner: true,
        };

        let mut output = Vec::new();
        let mut writer = BufWriter::new(&mut output);
        args.run(&mut writer)
            .expect("render prepared validator bundle");
        writer.flush().expect("flush prepared Compose output");
        drop(writer);
        let output = String::from_utf8(output).expect("Compose output is UTF-8");
        assert_eq!(output.matches("exec irohad").count(), 4);
        assert_eq!(output.matches("read_only: true").count(), 4);
        for artifact in [
            "prepared-bundle/genesis.signed.nrt",
            "prepared-bundle/genesis.public_key",
            "prepared-bundle/genesis.expected_hash",
        ] {
            assert!(output.contains(artifact), "missing {artifact}: {output}");
        }
        for forbidden in [
            "${IROHA_GENESIS_",
            "genesis.private_key",
            "IROHA_GENESIS_PRIVATE_KEY_FILE",
            "kagami genesis sign",
            "depends_on:",
        ] {
            assert!(
                !output.contains(forbidden),
                "unexpected {forbidden}: {output}"
            );
        }
    }

    #[test]
    fn prepared_bundle_rejects_signer_hash_roster_and_pop_mismatches() {
        let temp_dir = tempfile::tempdir().expect("prepared mismatch temp dir");
        let config_dir = generate_prepared_bundle(temp_dir.path());
        let count = NonZeroU16::new(4).expect("non-zero");
        load_prepared_bundle(&config_dir, count).expect("baseline prepared bundle validates");

        let public_path = config_dir.join(crate::localnet::GENESIS_PUBLIC_KEY_FILE);
        let original_public = fs::read(&public_path).expect("read public-key fixture");
        let other_signer = KeyPair::try_from_seed(
            b"different-prepared-genesis-signer".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("derive alternate signer");
        fs::write(&public_path, format!("{}\n", other_signer.public_key()))
            .expect("write mismatched signer");
        let signer_error = load_prepared_bundle(&config_dir, count)
            .expect_err("mismatched prepared signer must fail");
        assert!(
            signer_error.to_string().contains("signer"),
            "unexpected signer mismatch: {signer_error:#}"
        );
        fs::write(&public_path, original_public).expect("restore public-key fixture");

        let hash_path = config_dir.join(crate::localnet::GENESIS_EXPECTED_HASH_FILE);
        let original_hash = fs::read(&hash_path).expect("read hash fixture");
        fs::write(
            &hash_path,
            format!("{}\n", Hash::new(b"different prepared genesis body")),
        )
        .expect("write mismatched hash");
        let hash_error = load_prepared_bundle(&config_dir, count)
            .expect_err("mismatched prepared hash must fail");
        assert!(
            hash_error.to_string().contains("body hashes"),
            "unexpected hash mismatch: {hash_error:#}"
        );
        fs::write(&hash_path, original_hash).expect("restore hash fixture");

        let peer3_path = config_dir.join("peer3.toml");
        let peer3 = fs::read(&peer3_path).expect("read peer3 fixture");
        fs::remove_file(&peer3_path).expect("remove peer3 fixture");
        let roster_error = load_prepared_bundle(&config_dir, count)
            .expect_err("incomplete prepared roster must fail");
        assert!(
            roster_error.to_string().contains("roster files"),
            "unexpected roster mismatch: {roster_error:#}"
        );
        fs::write(&peer3_path, peer3).expect("restore peer3 fixture");

        let peer0_path = config_dir.join("peer0.toml");
        let original_peer0 = fs::read_to_string(&peer0_path).expect("read peer0 fixture");
        let marker = "pop_hex = \"";
        let pop_start = original_peer0.find(marker).expect("peer0 has PoP") + marker.len();
        let pop_end = original_peer0[pop_start..]
            .find('"')
            .map(|offset| pop_start + offset)
            .expect("peer0 PoP is quoted");
        let mut invalid_peer0 = original_peer0.clone();
        let last = pop_end.checked_sub(1).expect("PoP is non-empty");
        let replacement = if invalid_peer0.as_bytes()[last] == b'0' {
            "1"
        } else {
            "0"
        };
        invalid_peer0.replace_range(last..pop_end, replacement);
        fs::write(&peer0_path, invalid_peer0).expect("write mismatched PoP");
        let pop_error = load_prepared_bundle(&config_dir, count)
            .expect_err("mismatched prepared PoP must fail");
        assert!(
            format!("{pop_error:#}")
                .to_ascii_lowercase()
                .contains("pop"),
            "unexpected PoP mismatch: {pop_error:#}"
        );
        fs::write(&peer0_path, original_peer0).expect("restore peer0 fixture");
    }

    #[test]
    fn load_peer_overrides_reads_valid_file() -> color_eyre::Result<()> {
        let file = tempfile::NamedTempFile::new()?;
        fs::write(
            file.path(),
            r#"
[[peers]]
name = "alpha"
p2p_port = 2000
api_port = 9000

[[peers]]
name = "beta"
p2p_port = 2001
api_port = 9001
"#,
        )?;

        let overrides = load_peer_overrides(file.path())?;
        assert_eq!(overrides.len(), 2);
        assert_eq!(overrides[0].name, "alpha");
        assert_eq!(overrides[0].p2p_port, 2000);
        assert_eq!(overrides[0].api_port, 9000);
        assert_eq!(overrides[1].name, "beta");
        assert_eq!(overrides[1].p2p_port, 2001);
        assert_eq!(overrides[1].api_port, 9001);
        Ok(())
    }

    #[test]
    fn parse_peer_override_toml_rejects_empty_peer_list() {
        let err = parse_peer_override_toml("peers = []").expect_err("should fail on empty peers");
        assert!(
            err.to_string().contains("must list at least one peer"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn parse_peer_override_toml_rejects_out_of_range_ports() {
        let err = parse_peer_override_toml(
            r#"
[[peers]]
name = "alpha"
p2p_port = 70000
api_port = 9000
"#,
        )
        .expect_err("port 70000 should be rejected");
        assert!(
            err.to_string().contains("must fit into a u16"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn swarm_uses_manifest_consensus_without_environment_overrides() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let config_dir = temp_dir.path().join("cfg");
        fs::create_dir_all(&config_dir).expect("create config dir");
        write_npos_genesis(&config_dir.join("genesis.json"));
        let args = Args {
            peers: NonZeroU16::new(2).expect("non-zero"),
            seed: Some("swarm-npos-overrides".to_owned()),
            healthcheck: false,
            config_dir: config_dir.clone(),
            peer_config: None,
            image: "hyperledger/iroha:dev".to_owned(),
            build: None,
            no_cache: false,
            out_file: temp_dir.path().join("docker-compose.yml"),
            print: true,
            force: false,
            no_banner: true,
        };

        let mut buffer = Vec::new();
        let mut writer = BufWriter::new(&mut buffer);
        args.run(&mut writer)
            .expect("`Args::run` should render compose yaml");
        writer.flush().expect("flush buffer");
        drop(writer);

        let output = String::from_utf8(buffer).expect("output should be UTF-8");
        for retired_override in [
            "GENESIS_CONSENSUS_MODE:",
            "GENESIS_NEXT_CONSENSUS_MODE:",
            "GENESIS_MODE_ACTIVATION_HEIGHT:",
        ] {
            assert!(
                !output.contains(retired_override),
                "compose output must derive consensus from the manifest, not {retired_override}: {output}"
            );
        }
    }

    #[test]
    fn npos_swarm_requires_genesis_with_npos_parameters() {
        let temp_dir = tempfile::tempdir().expect("tmp dir");
        let config_dir = temp_dir.path().join("cfg");
        fs::create_dir_all(&config_dir).expect("create config dir");
        write_npos_genesis_without_parameters(&config_dir.join("genesis.json"));

        let args = Args {
            peers: NonZeroU16::new(1).expect("non-zero"),
            seed: Some("swarm-invalid-npos-dev".to_owned()),
            healthcheck: false,
            config_dir: config_dir.clone(),
            peer_config: None,
            image: "hyperledger/iroha:dev".to_owned(),
            build: None,
            no_cache: false,
            out_file: temp_dir.path().join("docker-compose.yml"),
            print: true,
            force: true,
            no_banner: true,
        };

        let mut writer = BufWriter::new(Vec::new());
        let err = args
            .run(&mut writer)
            .expect_err("missing NPoS parameters should fail compose generation");
        assert!(
            err.to_string().contains("sumeragi_npos_parameters"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn npos_swarm_succeeds_with_npos_genesis() {
        let temp_dir = tempfile::tempdir().expect("tmp dir");
        let config_dir = temp_dir.path().join("cfg");
        fs::create_dir_all(&config_dir).expect("create config dir");
        write_npos_genesis(&config_dir.join("genesis.json"));

        let args = Args {
            peers: NonZeroU16::new(3).expect("non-zero"),
            seed: Some("npos-ok".to_owned()),
            healthcheck: false,
            config_dir: config_dir.clone(),
            peer_config: None,
            image: "hyperledger/iroha:dev".to_owned(),
            build: None,
            no_cache: false,
            out_file: temp_dir.path().join("docker-compose.yml"),
            print: true,
            force: true,
            no_banner: true,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer)
            .expect("npos genesis with parameters should pass");
    }

    fn write_minimal_genesis(path: &Path) {
        let manifest =
            GenesisBuilder::new_without_executor(ChainId::from("test-chain"), PathBuf::from("."))
                .build_raw()
                .with_consensus_mode(
                    iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned,
                );
        let genesis_json = norito::json::to_json_pretty(&manifest).expect("serialize genesis");
        fs::write(path, genesis_json).expect("write minimal genesis");
    }

    fn write_npos_genesis_without_parameters(path: &Path) {
        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from("npos-without-parameters"),
            PathBuf::from("."),
        )
        .build_raw()
        .with_consensus_mode(iroha_data_model::parameter::system::SumeragiConsensusMode::Npos);
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis");
        fs::write(path, json).expect("write NPoS genesis without parameters");
    }

    fn write_npos_genesis(path: &Path) {
        let chain = ChainId::from("npos-swarm");
        let manifest = GenesisBuilder::new_without_executor(chain, PathBuf::from("."))
            .append_parameter(Parameter::Custom(
                SumeragiNposParameters::default().into_custom_parameter(),
            ))
            .build_raw()
            .with_consensus_mode(iroha_data_model::parameter::system::SumeragiConsensusMode::Npos);
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis");
        fs::write(path, json).expect("write npos genesis");
    }
}
