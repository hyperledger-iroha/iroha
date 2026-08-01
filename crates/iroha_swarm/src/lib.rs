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
    /// Deterministic development mode requires a non-empty seed.
    EmptyDevelopmentSeed,
    /// A prepared validator bundle must contain at least one validator.
    EmptyPreparedValidators,
    /// Prepared validator count ({actual}) exceeds the supported u16 range.
    PreparedValidatorCountOverflow {
        /// Number of prepared validators supplied by the caller.
        actual: usize,
    },
    /// Prepared validator {index} has an invalid BLS proof of possession: {reason}
    InvalidPreparedValidatorPop {
        /// Zero-based validator index.
        index: usize,
        /// Cryptographic verification error.
        reason: String,
    },
    /// Peer {index} has invalid Compose service name `{name}`.
    InvalidPeerServiceName {
        /// Zero-based peer index.
        index: u16,
        /// Invalid service name.
        name: String,
    },
    /// Compose service name `{name}` is used by more than one peer.
    DuplicatePeerServiceName {
        /// Duplicated service name.
        name: String,
    },
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
    /// Runtime genesis artifact sources.
    genesis: GenesisArtifactSettings,
    /// Absolute target path.
    target_path: path::AbsolutePath,
}

/// One validator identity from an already prepared and signed deployment bundle.
#[derive(Debug)]
pub struct PreparedValidator {
    /// Human-readable Compose service name.
    pub name: String,
    /// P2P port exposed by the validator service.
    pub p2p_port: u16,
    /// Torii API port exposed by the validator service.
    pub api_port: u16,
    /// Validator signing identity.
    pub key_pair: iroha_crypto::KeyPair,
    /// BLS proof of possession committed by the signed genesis topology.
    pub pop: Vec<u8>,
}

/// Host-prepared genesis artifacts consumed read-only by validators.
#[derive(Copy, Clone, Debug)]
pub struct PreparedGenesisArtifacts<'a> {
    /// Canonical signed genesis wire body.
    pub signed_block: &'a std::path::Path,
    /// Canonical one-line genesis verifier key.
    pub public_key: &'a std::path::Path,
    /// Canonical one-line exact genesis header hash.
    pub expected_hash: &'a std::path::Path,
}

/// Runtime genesis artifact paths, normalized relative to the Compose file.
#[derive(Debug)]
enum GenesisArtifactSettings {
    Environment,
    Prepared {
        signed_block: path::RelativePath,
        public_key: path::RelativePath,
        expected_hash: path::RelativePath,
    },
}

impl GenesisArtifactSettings {
    fn prepared(
        artifacts: PreparedGenesisArtifacts<'_>,
        target_dir: &path::AbsolutePath,
    ) -> Result<Self, Error> {
        let relative =
            |artifact: &std::path::Path| path::AbsolutePath::new(artifact)?.relative_to(target_dir);
        Ok(Self::Prepared {
            signed_block: relative(artifacts.signed_block)?,
            public_key: relative(artifacts.public_key)?,
            expected_hash: relative(artifacts.expected_hash)?,
        })
    }
}

/// Iroha peer settings.
struct PeerSettings {
    /// If `true`, include a healthcheck for every service in the configuration.
    healthcheck: bool,
    chain: iroha_data_model::ChainId,
    network: std::collections::BTreeMap<u16, peer::PeerInfo>,
    topology: std::collections::BTreeSet<iroha_data_model::peer::Peer>,
}

impl PeerSettings {
    fn validate_service_names(
        network: &std::collections::BTreeMap<u16, peer::PeerInfo>,
    ) -> Result<(), Error> {
        let mut names = std::collections::BTreeSet::new();
        for (index, (name, ..)) in network {
            let valid = name
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
            if !valid {
                return Err(Error::InvalidPeerServiceName {
                    index: *index,
                    name: name.clone(),
                });
            }
            if !names.insert(name.clone()) {
                return Err(Error::DuplicatePeerServiceName { name: name.clone() });
            }
        }
        Ok(())
    }

    fn deterministic_dev(
        count: std::num::NonZeroU16,
        overrides: Option<Vec<peer::PeerOverride>>,
        seed: &[u8],
        healthcheck: bool,
    ) -> Result<Self, Error> {
        if seed.is_empty() {
            return Err(Error::EmptyDevelopmentSeed);
        }
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
                    let (key_pair, pop) = peer::generate_bls_key_pair(Some(seed), &extra_seed)
                        .map_err(|error| {
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
            peer::network(count.get(), Some(seed)).map_err(|error| {
                Error::KeyGeneration(format!("failed to generate peer network keys: {error}"))
            })?
        };
        Self::validate_service_names(&network)?;
        let topology = peer::topology(network.values());
        Ok(Self {
            healthcheck,
            chain: peer::chain(),
            network,
            topology,
        })
    }

    fn prepared(
        chain: iroha_data_model::ChainId,
        validators: Vec<PreparedValidator>,
        healthcheck: bool,
    ) -> Result<Self, Error> {
        if validators.is_empty() {
            return Err(Error::EmptyPreparedValidators);
        }
        if validators.len() > usize::from(u16::MAX) {
            return Err(Error::PreparedValidatorCountOverflow {
                actual: validators.len(),
            });
        }
        let network = validators
            .into_iter()
            .enumerate()
            .map(|(index, validator)| {
                iroha_crypto::bls_normal_pop_verify(
                    validator.key_pair.public_key(),
                    &validator.pop,
                )
                .map_err(|error| Error::InvalidPreparedValidatorPop {
                    index,
                    reason: error.to_string(),
                })?;
                let id = u16::try_from(index).expect("prepared validator index fits u16");
                let (public_key, private_key) = validator.key_pair.into_parts();
                Ok((
                    id,
                    (
                        validator.name,
                        [validator.p2p_port, validator.api_port],
                        (public_key, iroha_crypto::ExposedPrivateKey(private_key)),
                        validator.pop,
                    ),
                ))
            })
            .collect::<Result<_, Error>>()?;
        Self::validate_service_names(&network)?;
        let topology = peer::topology(network.values());
        Ok(Self {
            healthcheck,
            chain,
            network,
            topology,
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
    /// Creates a deterministic development-only Swarm generator.
    ///
    /// The generated manifest requires the signed genesis body, verifier key, and exact hash
    /// through explicit host-file environment variables. Production callers should use
    /// [`Self::from_prepared`] so the validator roster and signed artifacts come from one
    /// authoritative prepared bundle.
    #[allow(clippy::missing_errors_doc)]
    pub fn deterministic_dev(
        count: std::num::NonZeroU16,
        seed: &[u8],
        healthcheck: bool,
        image: &'a str,
        build_dir: Option<&std::path::Path>,
        ignore_cache: bool,
        target_path: &std::path::Path,
        peer_overrides: Option<Vec<peer::PeerOverride>>,
    ) -> Result<Self, Error> {
        let peer = PeerSettings::deterministic_dev(count, peer_overrides, seed, healthcheck)?;
        Self::with_settings(
            peer,
            GenesisArtifactSettings::Environment,
            image,
            build_dir,
            ignore_cache,
            target_path,
        )
    }

    /// Creates a Swarm from one authoritative prepared validator/genesis bundle.
    #[allow(clippy::missing_errors_doc, clippy::too_many_arguments)]
    pub fn from_prepared(
        chain: iroha_data_model::ChainId,
        validators: Vec<PreparedValidator>,
        artifacts: PreparedGenesisArtifacts<'_>,
        healthcheck: bool,
        image: &'a str,
        build_dir: Option<&std::path::Path>,
        ignore_cache: bool,
        target_path: &std::path::Path,
    ) -> Result<Self, Error> {
        let peer = PeerSettings::prepared(chain, validators, healthcheck)?;
        if target_path.is_dir() {
            return Err(Error::TargetFileIsADirectory);
        }
        let target_path = path::AbsolutePath::new(target_path)?;
        let target_dir = target_path.parent().ok_or(Error::NoTargetDirectory)?;
        let genesis = GenesisArtifactSettings::prepared(artifacts, &target_dir)?;
        Ok(Self {
            peer,
            image: ImageSettings::new(image, build_dir, ignore_cache, &target_dir)?,
            genesis,
            target_path,
        })
    }

    fn with_settings(
        peer: PeerSettings,
        genesis: GenesisArtifactSettings,
        image: &'a str,
        build_dir: Option<&std::path::Path>,
        ignore_cache: bool,
        target_path: &std::path::Path,
    ) -> Result<Self, Error> {
        if target_path.is_dir() {
            return Err(Error::TargetFileIsADirectory);
        }
        let target_path = path::AbsolutePath::new(target_path)?;
        let target_dir = target_path.parent().ok_or(Error::NoTargetDirectory)?;
        Ok(Self {
            peer,
            image: ImageSettings::new(image, build_dir, ignore_cache, &target_dir)?,
            genesis,
            target_path,
        })
    }

    /// Builds the schema.
    #[allow(clippy::missing_errors_doc)]
    pub fn build(&self) -> schema::DockerCompose<'_> {
        schema::DockerCompose::new(&self.image, &self.peer, &self.genesis)
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

    use crate::{
        PreparedGenesisArtifacts, PreparedValidator, Swarm,
        peer::{self, PeerOverride},
    };

    const IMAGE: &str = "hyperledger/iroha:dev";
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
        target_path: &'a std::path::Path,
        peer_overrides: Option<Vec<PeerOverride>>,
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
            target_path,
            peer_overrides,
        } = paths;
        let mut buffer = Vec::new();
        Swarm::deterministic_dev(
            count,
            b"iroha-swarm-tests",
            healthcheck,
            IMAGE,
            build_dir,
            ignore_cache,
            target_path,
            peer_overrides,
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
                target_path: TARGET_PATH.as_ref(),
                peer_overrides: None,
            },
        )
    }

    fn assert_runtime_genesis_artifact_contract(output: &str, peer_count: usize) {
        for required in [
            "IROHA_GENESIS_PUBLIC_KEY_FILE:?set",
            "IROHA_GENESIS_EXPECTED_HASH_FILE:?set",
            "IROHA_GENESIS_SIGNED_FILE:?set",
        ] {
            assert!(
                output.contains(required),
                "Compose must require {required}: {output}"
            );
        }
        assert_eq!(
            output.matches("exec irohad").count(),
            peer_count,
            "the manifest must contain validators only: {output}"
        );
        assert_eq!(
            output.matches("tail -c 1").count(),
            2 * peer_count,
            "every validator must reject public-key/hash files without a final newline"
        );
        assert_eq!(
            output.matches("^[0-9a-f]{63}[13579bdf]$$").count(),
            peer_count,
            "every validator must enforce canonical exact-hash syntax"
        );
        for secret in [
            "- iroha_genesis_public_key",
            "- iroha_genesis_expected_hash",
        ] {
            assert_eq!(
                output.lines().filter(|line| line.trim() == secret).count(),
                peer_count,
                "every validator must receive {secret}: {output}"
            );
        }
        assert_eq!(
            output
                .lines()
                .filter(|line| line.trim() == "type: bind")
                .count(),
            peer_count,
            "every validator must bind the signed genesis body"
        );
        assert_eq!(
            output
                .lines()
                .filter(|line| line.trim() == "read_only: true")
                .count(),
            peer_count,
            "every signed genesis bind must be read-only"
        );
        assert!(
            output.contains("export GENESIS_PUBLIC_KEY GENESIS GENESIS_EXPECTED_HASH"),
            "validators must pass all approved artifacts into configuration"
        );
        for forbidden in [
            "IROHA_GENESIS_PRIVATE_KEY_FILE",
            "iroha_genesis_private_key",
            "kagami genesis sign",
            "service_completed_successfully",
            "depends_on:",
            "iroha_genesis:/genesis",
            "/config/client.toml",
            "/config/genesis.json",
        ] {
            assert!(
                !output.contains(forbidden),
                "validator-only Compose must not contain {forbidden}: {output}"
            );
        }

        let shell_content = output
            .lines()
            .filter(|line| !line.contains("${IROHA_GENESIS_"))
            .collect::<String>();
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

    fn prepared_validator(index: u16) -> PreparedValidator {
        let (key_pair, pop) = peer::generate_bls_key_pair(
            Some(b"iroha-swarm-prepared-validator"),
            &index.to_be_bytes(),
        )
        .expect("derive prepared validator");
        PreparedValidator {
            name: format!("irohad{index}"),
            p2p_port: crate::BASE_PORT_P2P + index,
            api_port: crate::BASE_PORT_API + index,
            key_pair: iroha_crypto::KeyPair::new(key_pair.0, key_pair.1.0)
                .expect("rebuild prepared validator key pair"),
            pop,
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
        assert_runtime_genesis_artifact_contract(&output, 1);
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
        assert_runtime_genesis_artifact_contract(&output, 1);
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

        assert_eq!(output.matches("pull_policy: build").count(), 1);
        assert_eq!(output.matches("pull_policy: never").count(), 3);
        assert!(output.contains("irohad0:"));
        assert_runtime_genesis_artifact_contract(&output, 4);
    }

    #[test]
    fn single_pull_healthcheck() {
        let output = build_as_string(nonzero_ext::nonzero!(1u16), true, None, false, None);

        assert!(output.contains("pull_policy: missing"));
        assert!(output.contains("start_period: 4s"));
        assert_runtime_genesis_artifact_contract(&output, 1);
    }

    #[test]
    fn multiple_pull_healthcheck_nocache() {
        let output = build_as_string(nonzero_ext::nonzero!(4u16), true, None, true, None);

        assert_eq!(output.matches("pull_policy: always").count(), 4);
        assert_eq!(output.matches("start_period: 4s").count(), 4);
        assert_runtime_genesis_artifact_contract(&output, 4);
    }

    #[test]
    fn runtime_does_not_mount_source_manifests_or_client_credentials() {
        let temp = TempDir::new("runtime_inputs");
        let target_path = temp.path().join("deployment/docker-compose.yml");

        let output = build_with_paths(
            nonzero_ext::nonzero!(1u16),
            false,
            None,
            false,
            None,
            ComposePaths {
                target_path: &target_path,
                peer_overrides: None,
            },
        );

        assert!(
            !output.contains("genesis.json"),
            "unexpected manifest mount: {output}"
        );
        assert!(
            !output.contains("client.toml"),
            "unexpected client credential mount: {output}"
        );
    }

    #[test]
    fn peer_overrides_replace_default_names_and_ports() {
        let temp = TempDir::new("peer_overrides");
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
                target_path: &target_path,
                peer_overrides: Some(overrides),
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

        let result = Swarm::deterministic_dev(
            nonzero_ext::nonzero!(1u16),
            b"iroha-swarm-tests",
            false,
            IMAGE,
            None,
            false,
            &target_dir,
            None,
        );

        assert!(matches!(result, Err(crate::Error::TargetFileIsADirectory)));
    }

    #[test]
    fn rejects_override_count_mismatch() {
        let temp = TempDir::new("override_mismatch");
        let target_path = temp.path().join("compose.yml");

        let overrides = vec![PeerOverride {
            name: "solo".into(),
            p2p_port: 2100,
            api_port: 9100,
        }];

        let result = Swarm::deterministic_dev(
            nonzero_ext::nonzero!(2u16),
            b"iroha-swarm-tests",
            false,
            IMAGE,
            None,
            false,
            &target_path,
            Some(overrides),
        );

        assert!(matches!(
            result,
            Err(crate::Error::InvalidPeerOverrideCount {
                expected: 2,
                actual: 1
            })
        ));
    }

    #[test]
    fn deterministic_development_mode_rejects_empty_seed() {
        let result = Swarm::deterministic_dev(
            nonzero_ext::nonzero!(1u16),
            b"",
            false,
            IMAGE,
            None,
            false,
            std::path::Path::new(TARGET_PATH),
            None,
        );
        assert!(matches!(result, Err(crate::Error::EmptyDevelopmentSeed)));
    }

    #[test]
    fn prepared_mode_uses_concrete_read_only_genesis_artifacts() {
        let temp = TempDir::new("prepared_artifacts");
        let bundle = temp.path().join("bundle");
        let deployment = temp.path().join("deployment");
        std::fs::create_dir_all(&bundle).expect("create prepared bundle directory");
        std::fs::create_dir_all(&deployment).expect("create deployment directory");
        let signed_block = bundle.join("genesis.signed.nrt");
        let public_key = bundle.join("genesis.public_key");
        let expected_hash = bundle.join("genesis.expected_hash");
        std::fs::write(&signed_block, b"signed").expect("write signed fixture");
        std::fs::write(&public_key, b"public\n").expect("write public fixture");
        std::fs::write(
            &expected_hash,
            format!("{}\n", iroha_crypto::Hash::new(b"genesis")),
        )
        .expect("write hash fixture");
        let target = deployment.join("docker-compose.yml");
        let swarm = Swarm::from_prepared(
            peer::chain(),
            vec![prepared_validator(0), prepared_validator(1)],
            PreparedGenesisArtifacts {
                signed_block: &signed_block,
                public_key: &public_key,
                expected_hash: &expected_hash,
            },
            false,
            IMAGE,
            None,
            false,
            &target,
        )
        .expect("build prepared swarm");
        let mut output = Vec::new();
        swarm
            .build()
            .write(&mut output, None)
            .expect("render prepared swarm");
        let output = String::from_utf8(output).expect("Compose output is UTF-8");
        for artifact in [
            "../bundle/genesis.signed.nrt",
            "../bundle/genesis.public_key",
            "../bundle/genesis.expected_hash",
        ] {
            assert!(output.contains(artifact), "missing {artifact}: {output}");
        }
        assert!(!output.contains("${IROHA_GENESIS_"));
        assert_eq!(output.matches("read_only: true").count(), 2);
        assert_eq!(output.matches("exec irohad").count(), 2);
    }
}
