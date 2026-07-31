//! Docker Compose configuration generator for Iroha.

mod path;
mod peer;
mod schema;

pub use crate::peer::PeerOverride;

const CHAIN_ID: &str = "00000000-0000-0000-0000-000000000000";
const BASE_PORT_P2P: u16 = 1337;
const BASE_PORT_API: u16 = 8080;

/// Swarm error.
#[derive(displaydoc::Display, Debug)]
pub enum Error {
    /// Target file path points to a directory.
    TargetFileIsADirectory,
    /// Target directory not found.
    NoTargetDirectory,
    /// Peer overrides count ({actual}) does not match the requested peer count ({expected}).
    InvalidPeerOverrideCount {
        /// Number of peers requested for the swarm manifest.
        expected: u16,
        /// Number of override entries supplied by the caller.
        actual: usize,
    },
    /// Failed to generate cryptographic swarm material: {0}
    KeyGeneration(String),
    /// {0}
    PathConversion(path::Error),
}

impl std::error::Error for Error {}

/// Swarm settings.
pub struct Swarm<'a> {
    /// Peer settings.
    peer: PeerSettings,
    /// Docker image settings.
    image: ImageSettings<'a>,
    /// Absolute target path.
    target_path: path::AbsolutePath,
}

/// Iroha peer settings.
struct PeerSettings {
    /// If `true`, include a healthcheck for every service in the configuration.
    healthcheck: bool,
    /// Path to a directory with peer configuration relative to the target path.
    config_dir: path::RelativePath,
    chain: iroha_data_model::ChainId,
    network: std::collections::BTreeMap<u16, peer::PeerInfo>,
    topology: std::collections::BTreeSet<iroha_data_model::peer::Peer>,
    consensus_mode: Option<String>,
    next_consensus_mode: Option<String>,
    mode_activation_height: Option<u64>,
}

impl PeerSettings {
    #[allow(clippy::too_many_arguments)]
    fn new(
        count: std::num::NonZeroU16,
        overrides: Option<Vec<peer::PeerOverride>>,
        seed: Option<&[u8]>,
        healthcheck: bool,
        config_dir: &std::path::Path,
        target_dir: &path::AbsolutePath,
        consensus_mode: Option<String>,
        next_consensus_mode: Option<String>,
        mode_activation_height: Option<u64>,
    ) -> Result<Self, Error> {
        let network = if let Some(overrides) = overrides {
            let expected = count.get();
            if overrides.len() != expected as usize {
                return Err(Error::InvalidPeerOverrideCount {
                    expected,
                    actual: overrides.len(),
                });
            }
            overrides
                .into_iter()
                .enumerate()
                .map(|(idx, override_)| {
                    let nth = u16::try_from(idx).expect("peer override index must fit into u16");
                    let extra_seed = nth.to_be_bytes();
                    let name = override_.name;
                    let (key_pair, pop) =
                        peer::generate_bls_key_pair(seed, &extra_seed).map_err(|error| {
                            Error::KeyGeneration(format!(
                                "failed to generate BLS key pair for peer {name}: {error}"
                            ))
                        })?;
                    Ok((
                        nth,
                        (
                            name,
                            [override_.p2p_port, override_.api_port],
                            key_pair,
                            pop,
                        ),
                    ))
                })
                .collect::<Result<_, Error>>()?
        } else {
            peer::network(count.get(), seed).map_err(|error| {
                Error::KeyGeneration(format!("failed to generate peer network keys: {error}"))
            })?
        };
        let topology = peer::topology(network.values());
        Ok(Self {
            healthcheck,
            config_dir: path::AbsolutePath::new(config_dir)?.relative_to(target_dir)?,
            chain: peer::chain(),
            network,
            topology,
            consensus_mode,
            next_consensus_mode,
            mode_activation_height,
        })
    }
}

/// Docker image settings.
struct ImageSettings<'a> {
    /// Image identifier.
    name: &'a str,
    /// Path to the Dockerfile directory relative to the target path.
    build_dir: Option<path::RelativePath>,
    /// If `true`, image will be pulled or built even if cached.
    ignore_cache: bool,
}

impl<'a, 'temp> ImageSettings<'a> {
    fn new(
        name: &'a str,
        build_dir: Option<&std::path::Path>,
        ignore_cache: bool,
        target_dir: &'temp path::AbsolutePath,
    ) -> Result<Self, Error> {
        Ok(Self {
            name,
            build_dir: build_dir
                .map(path::AbsolutePath::new)
                .transpose()?
                .map(|dir| dir.relative_to(target_dir))
                .transpose()?,
            ignore_cache,
        })
    }
}

impl<'a> Swarm<'a> {
    /// Creates a new Swarm generator.
    #[allow(clippy::too_many_arguments, clippy::missing_errors_doc)]
    pub fn new(
        count: std::num::NonZeroU16,
        seed: Option<&'a [u8]>,
        healthcheck: bool,
        config_dir: &'a std::path::Path,
        image: &'a str,
        build_dir: Option<&'a std::path::Path>,
        ignore_cache: bool,
        target_path: &'a std::path::Path,
        peer_overrides: Option<Vec<peer::PeerOverride>>,
        consensus_mode: Option<String>,
        next_consensus_mode: Option<String>,
        mode_activation_height: Option<u64>,
    ) -> Result<Self, Error> {
        if target_path.is_dir() {
            return Err(Error::TargetFileIsADirectory);
        }
        let target_path = path::AbsolutePath::new(target_path)?;
        let target_dir = target_path.parent().ok_or(Error::NoTargetDirectory)?;
        Ok(Self {
            peer: PeerSettings::new(
                count,
                peer_overrides,
                seed,
                healthcheck,
                config_dir,
                &target_dir,
                consensus_mode,
                next_consensus_mode,
                mode_activation_height,
            )?,
            image: ImageSettings::new(image, build_dir, ignore_cache, &target_dir)?,
            target_path,
        })
    }

    /// Builds the schema.
    #[allow(clippy::missing_errors_doc)]
    pub fn build(&self) -> schema::DockerCompose<'_> {
        schema::DockerCompose::new(&self.image, &self.peer)
    }

    /// Returns the absolute target file path.
    pub fn absolute_target_path(&self) -> &std::path::Path {
        self.target_path.as_ref()
    }
}

impl From<path::Error> for Error {
    fn from(error: path::Error) -> Self {
        Self::PathConversion(error)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::too_many_lines, clippy::needless_raw_string_hashes)]

    use crate::{Swarm, peer::PeerOverride};

    const IMAGE: &str = "hyperledger/iroha:dev";
    const PEER_CONFIG_PATH: &str = "./defaults";
    const TARGET_PATH: &str = "./defaults/docker-compose.yml";

    struct TempDir {
        path: std::path::PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> Self {
            let unique_suffix = format!(
                "iroha_swarm_{label}_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be monotonic")
                    .as_nanos()
            );
            Self {
                path: std::env::temp_dir().join(unique_suffix),
            }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    struct ComposePaths<'a> {
        config_dir: &'a std::path::Path,
        target_path: &'a std::path::Path,
        peer_overrides: Option<Vec<PeerOverride>>,
        consensus_mode: Option<&'a str>,
        next_consensus_mode: Option<&'a str>,
        mode_activation_height: Option<u64>,
    }

    fn build_with_paths(
        count: std::num::NonZeroU16,
        healthcheck: bool,
        build_dir: Option<&std::path::Path>,
        ignore_cache: bool,
        banner: Option<&[&str]>,
        paths: ComposePaths<'_>,
    ) -> String {
        let ComposePaths {
            config_dir,
            target_path,
            peer_overrides,
            consensus_mode,
            next_consensus_mode,
            mode_activation_height,
        } = paths;
        let mut buffer = Vec::new();
        Swarm::new(
            count,
            Some(&[]),
            healthcheck,
            config_dir,
            IMAGE,
            build_dir,
            ignore_cache,
            target_path,
            peer_overrides,
            consensus_mode.map(str::to_owned),
            next_consensus_mode.map(str::to_owned),
            mode_activation_height,
        )
        .unwrap()
        .build()
        .write(&mut buffer, banner)
        .unwrap();
        String::from_utf8(buffer).unwrap()
    }

    fn build_as_string(
        count: std::num::NonZeroU16,
        healthcheck: bool,
        build_dir: Option<&str>,
        ignore_cache: bool,
        banner: Option<&[&str]>,
    ) -> String {
        build_with_paths(
            count,
            healthcheck,
            build_dir.map(std::path::Path::new),
            ignore_cache,
            banner,
            ComposePaths {
                config_dir: PEER_CONFIG_PATH.as_ref(),
                target_path: TARGET_PATH.as_ref(),
                peer_overrides: None,
                consensus_mode: None,
                next_consensus_mode: None,
                mode_activation_height: None,
            },
        )
    }

    fn assert_runtime_genesis_secret_contract(output: &str, peer_count: usize) {
        assert!(
            output.contains("IROHA_GENESIS_PUBLIC_KEY_FILE:?set"),
            "Compose must require an explicit genesis public-key file: {output}"
        );
        assert!(
            output.contains("IROHA_GENESIS_PRIVATE_KEY_FILE:?set"),
            "Compose must require an explicit genesis private-key file: {output}"
        );
        assert!(
            output.contains("--private-key-file \\\"$$GENESIS_PRIVATE_KEY_FILE\\\""),
            "genesis signer must consume a file, not a command-line secret: {output}"
        );
        assert!(
            output.contains("--expected-public-key \\\"$$GENESIS_PUBLIC_KEY\\\""),
            "genesis signer must enforce public/private consistency: {output}"
        );
        assert_eq!(
            output.matches("tail -c 1").count(),
            peer_count,
            "every peer must reject a public-key file without a final newline"
        );
        let cleanup = "rm -f \\\"$$GENESIS_PRIVATE_KEY_FILE\\\"";
        assert_eq!(
            output.matches(cleanup).count(),
            2,
            "the signer must clean its copied key on both failure and success"
        );
        assert!(
            output.contains("trap 'exit 143' TERM"),
            "startup signals must terminate through the cleanup trap"
        );
        assert!(
            output.find(cleanup) < output.rfind("exec irohad"),
            "the successful signing path must remove the copied key before starting irohad"
        );
        assert!(
            !output.contains("GENESIS_PRIVATE_KEY:"),
            "generated environments must never contain genesis private material: {output}"
        );
        assert!(
            !output.contains("GENESIS_PUBLIC_KEY:"),
            "generated environments must load the verifier key from a secret file: {output}"
        );
        assert_eq!(
            output.matches("- iroha_genesis_public_key").count(),
            peer_count,
            "every peer must receive only the public genesis secret by default"
        );
        assert_eq!(
            output.matches("- iroha_genesis_private_key").count(),
            1,
            "only the genesis-submitting peer may receive the signing secret"
        );

        let required_public_file = "${IROHA_GENESIS_PUBLIC_KEY_FILE:?set IROHA_GENESIS_PUBLIC_KEY_FILE to an owner-controlled genesis public-key file}";
        let required_private_file = "${IROHA_GENESIS_PRIVATE_KEY_FILE:?set IROHA_GENESIS_PRIVATE_KEY_FILE to an owner-held mode-0600 genesis private-key file}";
        let shell_content = output
            .replace(required_public_file, "")
            .replace(required_private_file, "");
        let mut chars = shell_content.chars();
        while let Some(character) = chars.next() {
            if character == '$' {
                assert_eq!(
                    chars.next(),
                    Some('$'),
                    "shell dollars must be escaped from Compose interpolation: {output}"
                );
            }
        }
    }

    #[test]
    fn single_build_banner() {
        let output = build_as_string(
            nonzero_ext::nonzero!(1u16),
            false,
            Some("."),
            false,
            Some(&["Single-line banner"]),
        );

        assert!(output.starts_with("# Single-line banner\n\n"));
        assert!(output.contains("build: .."));
        assert!(output.contains("pull_policy: never"));
        assert_runtime_genesis_secret_contract(&output, 1);
    }

    #[test]
    fn single_build_banner_nocache() {
        let output = build_as_string(
            nonzero_ext::nonzero!(1u16),
            false,
            Some("."),
            true,
            Some(&["Multi-line banner 1", "Multi-line banner 2"]),
        );

        assert!(output.starts_with("# Multi-line banner 1\n# Multi-line banner 2\n\n"));
        assert!(output.contains("pull_policy: build"));
        assert_runtime_genesis_secret_contract(&output, 1);
    }

    #[test]
    fn multiple_build_banner_nocache() {
        let output = build_as_string(
            nonzero_ext::nonzero!(4u16),
            false,
            Some("."),
            true,
            Some(&["Single-line banner"]),
        );

        assert_eq!(output.matches("pull_policy: build").count(), 4);
        assert!(output.contains("- irohad0"));
        assert_runtime_genesis_secret_contract(&output, 4);
    }

    #[test]
    fn single_pull_healthcheck() {
        let output = build_as_string(nonzero_ext::nonzero!(1u16), true, None, false, None);

        assert!(output.contains("pull_policy: missing"));
        assert!(output.contains("start_period: 4s"));
        assert_runtime_genesis_secret_contract(&output, 1);
    }

    #[test]
    fn multiple_pull_healthcheck_nocache() {
        let output = build_as_string(nonzero_ext::nonzero!(4u16), true, None, true, None);

        assert_eq!(output.matches("pull_policy: always").count(), 4);
        assert_eq!(output.matches("start_period: 4s").count(), 4);
        assert_runtime_genesis_secret_contract(&output, 4);
    }

    #[test]
    fn npos_consensus_mode_propagates_into_genesis_env() {
        let temp = TempDir::new("npos_env");
        let config_dir = temp.path().join("configs");
        let target_path = temp.path().join("docker-compose.yml");

        let output = build_with_paths(
            nonzero_ext::nonzero!(1u16),
            false,
            None,
            false,
            None,
            ComposePaths {
                config_dir: &config_dir,
                target_path: &target_path,
                peer_overrides: None,
                consensus_mode: Some("npos"),
                next_consensus_mode: Some("npos"),
                mode_activation_height: Some(7),
            },
        );

        assert!(
            output.contains("GENESIS_CONSENSUS_MODE: npos"),
            "genesis signing environment should carry consensus mode"
        );
        assert!(
            output.contains("GENESIS_NEXT_CONSENSUS_MODE: npos"),
            "genesis signing environment should carry next consensus mode"
        );
        assert!(
            output.contains("GENESIS_MODE_ACTIVATION_HEIGHT: 7"),
            "genesis signing environment should carry activation height"
        );
        assert!(
            output.contains("--consensus-mode $$GENESIS_CONSENSUS_MODE"),
            "signing command must forward consensus mode override"
        );
        assert!(
            output.contains("--next-consensus-mode $$GENESIS_NEXT_CONSENSUS_MODE"),
            "signing command must forward next consensus mode override"
        );
        assert!(
            output.contains("--mode-activation-height $$GENESIS_MODE_ACTIVATION_HEIGHT"),
            "signing command must forward activation height override"
        );
    }

    #[test]
    fn nested_config_dir_volume_paths_are_normalized() {
        let temp = TempDir::new("nested_config");
        let config_dir = temp.path().join("configs/peer");
        let target_path = temp.path().join("deployment/docker-compose.yml");

        let output = build_with_paths(
            nonzero_ext::nonzero!(1u16),
            false,
            None,
            false,
            None,
            ComposePaths {
                config_dir: &config_dir,
                target_path: &target_path,
                peer_overrides: None,
                consensus_mode: None,
                next_consensus_mode: None,
                mode_activation_height: None,
            },
        );

        assert!(
            output.contains("- ../configs/peer/genesis.json:/config/genesis.json:ro"),
            "generated YAML did not include normalized genesis volume: {output}"
        );
        assert!(
            output.contains("- ../configs/peer/client.toml:/config/client.toml:ro"),
            "generated YAML did not include normalized client volume: {output}"
        );
    }

    #[test]
    fn peer_overrides_replace_default_names_and_ports() {
        let temp = TempDir::new("peer_overrides");
        let config_dir = temp.path().join("configs");
        let target_path = temp.path().join("docker-compose.yml");

        let overrides = vec![
            PeerOverride {
                name: "alpha".into(),
                p2p_port: 2000,
                api_port: 9000,
            },
            PeerOverride {
                name: "beta".into(),
                p2p_port: 2001,
                api_port: 9001,
            },
        ];

        let output = build_with_paths(
            nonzero_ext::nonzero!(2u16),
            false,
            None,
            false,
            None,
            ComposePaths {
                config_dir: &config_dir,
                target_path: &target_path,
                peer_overrides: Some(overrides),
                consensus_mode: None,
                next_consensus_mode: None,
                mode_activation_height: None,
            },
        );

        assert!(
            output.contains("alpha"),
            "custom peer name missing: {output}"
        );
        assert!(
            output.contains("beta"),
            "custom peer name missing: {output}"
        );
        assert!(output.contains("2000"), "custom P2P port missing: {output}");
        assert!(output.contains("9000"), "custom API port missing: {output}");
    }

    #[test]
    fn rejects_directory_target_path() {
        let temp = TempDir::new("target_directory");
        let target_dir = temp.path().join("deployment");
        std::fs::create_dir_all(&target_dir).expect("should create target directory");
        let config_dir = temp.path().join("configs");

        let result = Swarm::new(
            nonzero_ext::nonzero!(1u16),
            Some(&[]),
            false,
            &config_dir,
            IMAGE,
            None,
            false,
            &target_dir,
            None,
            None,
            None,
            None,
        );

        assert!(matches!(result, Err(crate::Error::TargetFileIsADirectory)));
    }

    #[test]
    fn rejects_override_count_mismatch() {
        let temp = TempDir::new("override_mismatch");
        let config_dir = temp.path().join("configs");
        let target_path = temp.path().join("compose.yml");

        let overrides = vec![PeerOverride {
            name: "solo".into(),
            p2p_port: 2100,
            api_port: 9100,
        }];

        let result = Swarm::new(
            nonzero_ext::nonzero!(2u16),
            Some(&[]),
            false,
            &config_dir,
            IMAGE,
            None,
            false,
            &target_path,
            Some(overrides),
            None,
            None,
            None,
        );

        assert!(matches!(
            result,
            Err(crate::Error::InvalidPeerOverrideCount {
                expected: 2,
                actual: 1
            })
        ));
    }
}
