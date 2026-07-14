use std::{
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use clap::Args as ClapArgs;
use color_eyre::eyre::{WrapErr as _, ensure, eyre};
use iroha_data_model::parameter::system::SumeragiConsensusMode;
use iroha_genesis::RawGenesisTransaction;
use iroha_swarm::PeerOverride;

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
    /// UTF-8 seed for deterministic key-generation.
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
    /// Directory with Iroha configuration.
    /// It will be mapped to a volume for each container.
    ///
    /// The directory should contain `genesis.json`. If you plan to upgrade the executor
    /// at genesis, include the executor bytecode file and reference it from `genesis.json`.
    #[arg(long, short, value_name = "DIR")]
    config_dir: PathBuf,
    /// Optional TOML file describing peer names and port mappings.
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
    #[arg(long, value_name = "FILE")]
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
        let swarm = iroha_swarm::Swarm::new(
            args.peers,
            args.seed.as_deref().map(str::as_bytes),
            args.healthcheck,
            &args.config_dir,
            &args.image,
            args.build.as_deref(),
            args.no_cache,
            &args.out_file,
            peer_overrides,
            None,
            None,
            None,
        )?;
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

    use iroha_data_model::{
        ChainId,
        parameter::{Parameter, system::SumeragiNposParameters},
    };
    use iroha_genesis::GenesisBuilder;

    use super::{Args, load_peer_overrides, parse_peer_override_toml};
    use crate::RunArgs;

    #[test]
    fn run_succeeds_without_banner() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let config_dir = temp_dir.path().join("cfg");
        fs::create_dir_all(&config_dir).expect("create config dir");
        write_minimal_genesis(&config_dir.join("genesis.json"));
        let args = Args {
            peers: NonZeroU16::new(1).expect("non-zero"),
            seed: None,
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
            seed: None,
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
