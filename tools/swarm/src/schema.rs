//! Docker Compose schema.

use crate::{path, peer, ImageSettings, PeerSettings};

mod serde_impls;

/// Schema serialization error.
#[derive(displaydoc::Display, Debug)]
pub enum Error {
    /// Could not write the banner: {0}
    BannerWrite(std::io::Error),
    /// Could not serialize the schema: {0}
    SerdeYaml(serde_yaml::Error),
}

impl std::error::Error for Error {}

/// Image identifier.
#[derive(serde::Serialize, Copy, Clone, Debug)]
#[serde(transparent)]
struct ImageId<'a>(&'a str);

/// Dictates how the image provider will build the image from a Dockerfile.
#[derive(serde::Serialize, Copy, Clone, Debug)]
enum Build {
    /// Rebuild the image, ignoring the local cache.
    #[serde(rename = "build")]
    IgnoreCache,
    /// Only build the image when it is missing from the local cache.
    #[serde(rename = "never")]
    OnCacheMiss,
}

/// Dictates that a service must use the built image.
#[derive(serde::Serialize, Copy, Clone, Debug)]
enum UseBuilt {
    #[serde(rename = "never")]
    UseCached,
}

/// Dictates how a service will pull the image from Docker Hub.
#[derive(serde::Serialize, Copy, Clone, Debug)]
enum Pull {
    /// Always pull the image, ignoring the local cache.
    #[serde(rename = "always")]
    IgnoreCache,
    /// Only pull the image when it is missing from the local cache.
    #[serde(rename = "missing")]
    OnCacheMiss,
}

impl Pull {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn is_on_cache_miss(&self) -> bool {
        matches!(self, Self::OnCacheMiss)
    }
}

/// Path on the host.
#[derive(serde::Serialize, Copy, Clone, Debug)]
#[serde(transparent)]
struct HostPath<'a>(&'a path::RelativePath);

/// Image build settings.
#[derive(serde::Serialize, Copy, Clone, Debug)]
struct BuildImage<'a> {
    image: ImageId<'a>,
    build: HostPath<'a>,
    pull_policy: Build,
}

impl<'a> BuildImage<'a> {
    fn new(image: ImageId<'a>, build: HostPath<'a>, ignore_cache: bool) -> Self {
        Self {
            image,
            build,
            pull_policy: if ignore_cache {
                Build::IgnoreCache
            } else {
                Build::OnCacheMiss
            },
        }
    }
}

/// Reference to the first peer.
#[derive(Copy, Clone, Debug)]
struct Irohad0Ref;

const IROHAD0: &str = "irohad0";

/// Image that has been built.
#[derive(serde::Serialize, Copy, Clone, Debug)]
struct BuiltImage<'a> {
    depends_on: [Irohad0Ref; 1],
    image: ImageId<'a>,
    pull_policy: UseBuilt,
}

/// Image that has been pulled.
#[derive(serde::Serialize, Copy, Clone, Debug)]
struct PulledImage<'a> {
    image: ImageId<'a>,
    #[serde(skip_serializing_if = "Pull::is_on_cache_miss")]
    pull_policy: Pull,
}

impl<'a> BuiltImage<'a> {
    fn new(image: ImageId<'a>) -> Self {
        Self {
            depends_on: [Irohad0Ref],
            image,
            pull_policy: UseBuilt::UseCached,
        }
    }
}

impl<'a> PulledImage<'a> {
    fn new(image: ImageId<'a>, ignore_cache: bool) -> Self {
        Self {
            image,
            pull_policy: if ignore_cache {
                Pull::IgnoreCache
            } else {
                Pull::OnCacheMiss
            },
        }
    }
}

/// Compile-time boolean literal.
#[derive(Debug)]
struct Bool<const VALUE: bool>;

/// Peer environment variables.
#[serde_with::serde_as]
#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "UPPERCASE")]
struct PeerEnv<'a> {
    chain: &'a iroha_data_model::ChainId,
    public_key: &'a iroha_crypto::PublicKey,
    private_key: &'a iroha_crypto::ExposedPrivateKey,
    p2p_address: iroha_primitives::addr::SocketAddr,
    api_address: iroha_primitives::addr::SocketAddr,
    genesis: ContainerPath<'a>,
    #[serde(skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    #[serde_as(as = "serde_with::json::JsonString")]
    trusted_peers: std::collections::BTreeSet<&'a iroha_data_model::peer::PeerId>,
    #[serde_as(as = "serde_with::json::JsonString")]
    topology: std::collections::BTreeSet<&'a iroha_data_model::peer::PeerId>,
}

impl<'a> PeerEnv<'a> {
    fn new(
        (public_key, private_key): &'a peer::ExposedKeyPair,
        [port_p2p, port_api]: [u16; 2],
        chain: &'a iroha_data_model::ChainId,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::PeerId>,
    ) -> Self {
        Self {
            chain,
            public_key,
            private_key,
            p2p_address: iroha_primitives::addr::socket_addr!(0.0.0.0:port_p2p),
            api_address: iroha_primitives::addr::socket_addr!(0.0.0.0:port_api),
            genesis: ContainerPath(GENESIS_FILE),
            trusted_peers: topology
                .iter()
                .filter(|&peer| peer.public_key() != public_key)
                .collect(),
            topology: topology.iter().collect(),
        }
    }
}

/// Mapping between `host:container` ports.
#[derive(Debug)]
struct PortMapping(u16, u16);

/// Path inside the container.
#[derive(serde::Serialize, Copy, Clone, Debug)]
#[serde(transparent)]
struct ContainerPath<'a>(&'a str);

const CONTAINER_CONFIG_DIR: &str = "/config";
const GENESIS_FILE: &str = "/tmp/genesis.scale";

/// Mapping between `host:container` paths.
#[derive(Copy, Clone, Debug)]
struct PathMapping<'a>(HostPath<'a>, ContainerPath<'a>);

/// Healthcheck parameters.
#[derive(Debug)]
struct Healthcheck {
    port: u16,
}

// half of default pipeline time
const HEALTH_CHECK_INTERVAL: &str = "2s";
// status request usually resolves immediately
const HEALTH_CHECK_TIMEOUT: &str = "1s";
// try within one minute given the interval
const HEALTH_CHECK_RETRIES: u8 = 30u8;
// default pipeline time
const HEALTH_CHECK_START_PERIOD: &str = "4s";

/// Iroha peer service.
#[derive(serde::Serialize, Debug)]
struct Irohad<'a, Image, Environment = PeerEnv<'a>>
where
    Image: serde::Serialize,
    Environment: serde::Serialize,
{
    #[serde(flatten)]
    image: Image,
    environment: Environment,
    ports: [PortMapping; 2],
    volumes: [PathMapping<'a>; 1],
    init: Bool<true>,
    #[serde(skip_serializing_if = "Option::is_none")]
    healthcheck: Option<Healthcheck>,
    command: PrepareGenesisAndStartIroha,
}

impl<'a, Image, Environment> Irohad<'a, Image, Environment>
where
    Image: serde::Serialize,
    Environment: serde::Serialize,
{
    fn new(
        image: Image,
        environment: Environment,
        [port_p2p, port_api]: [u16; 2],
        volumes: [PathMapping<'a>; 1],
        healthcheck: bool,
    ) -> Self {
        Self {
            image,
            environment,
            ports: [
                PortMapping(port_p2p, port_p2p),
                PortMapping(port_api, port_api),
            ],
            volumes,
            init: Bool,
            healthcheck: healthcheck.then_some(Healthcheck { port: port_api }),
            command: PrepareGenesisAndStartIroha,
        }
    }
}

/// Command used by the genesis service to sign and submit genesis.
#[derive(Debug)]
struct PrepareGenesisAndStartIroha;

const PREPARE_GENESIS_AND_START_IROHA: &str = r#"/bin/sh -c "
    EXECUTOR_RELATIVE_PATH=$(jq -r '.executor' /config/genesis.json) && \\
    EXECUTOR_ABSOLUTE_PATH=$(realpath \"/config/$$EXECUTOR_RELATIVE_PATH\") && \\
    jq \\
        --arg executor \"$$EXECUTOR_ABSOLUTE_PATH\" \\
        --argjson topology \"$$TOPOLOGY\" \\
        '.executor = $$executor | .topology = $$topology' /config/genesis.json \\
        >/tmp/genesis.json && \\
    export GENESIS_HASH=$(kagami genesis prepare /tmp/genesis.json --out-file $$GENESIS) && \\
    irohad
""#;

/// Reference to an `irohad` service.
#[derive(Debug, PartialOrd, PartialEq, Ord, Eq)]
struct IrohadRef(u16);

#[derive(serde::Serialize, Debug)]
#[serde(untagged)]
enum BuildOrPull<'a> {
    Build {
        irohad0: Irohad<'a, BuildImage<'a>>,
        #[serde(flatten, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
        irohads: std::collections::BTreeMap<IrohadRef, Irohad<'a, BuiltImage<'a>>>,
    },
    Pull {
        irohad0: Irohad<'a, PulledImage<'a>>,
        #[serde(flatten, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
        irohads: std::collections::BTreeMap<IrohadRef, Irohad<'a, PulledImage<'a>>>,
    },
}

impl<'a> BuildOrPull<'a> {
    fn pull(
        image: PulledImage<'a>,
        volumes: [PathMapping<'a>; 1],
        healthcheck: bool,
        chain: &'a iroha_data_model::ChainId,
        network: &'a std::collections::BTreeMap<u16, peer::PeerInfo>,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::PeerId>,
    ) -> Self {
        Self::Pull {
            irohad0: Self::irohad0(image, volumes, healthcheck, chain, network, topology),
            irohads: Self::irohads(image, volumes, healthcheck, chain, network, topology),
        }
    }

    fn build(
        image: BuildImage<'a>,
        volumes: [PathMapping<'a>; 1],
        healthcheck: bool,
        chain: &'a iroha_data_model::ChainId,
        network: &'a std::collections::BTreeMap<u16, peer::PeerInfo>,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::PeerId>,
    ) -> Self {
        Self::Build {
            irohad0: Self::irohad0(image, volumes, healthcheck, chain, network, topology),
            irohads: Self::irohads(
                BuiltImage::new(image.image),
                volumes,
                healthcheck,
                chain,
                network,
                topology,
            ),
        }
    }

    fn irohad0<Image: serde::Serialize>(
        image: Image,
        volumes: [PathMapping<'a>; 1],
        healthcheck: bool,
        chain: &'a iroha_data_model::ChainId,
        network: &'a std::collections::BTreeMap<u16, peer::PeerInfo>,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::PeerId>,
    ) -> Irohad<'a, Image> {
        let (_, ports, key_pair) = network.get(&0).expect("irohad0 must be present");
        Irohad::new(
            image,
            PeerEnv::new(key_pair, *ports, chain, topology),
            *ports,
            volumes,
            healthcheck,
        )
    }

    fn irohads<Image: serde::Serialize + Copy>(
        image: Image,
        volumes: [PathMapping<'a>; 1],
        healthcheck: bool,
        chain: &'a iroha_data_model::ChainId,
        network: &'a std::collections::BTreeMap<u16, peer::PeerInfo>,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::PeerId>,
    ) -> std::collections::BTreeMap<IrohadRef, Irohad<'a, Image>> {
        network
            .iter()
            .skip(1)
            .map(|(id, (_, ports, key_pair))| {
                (
                    IrohadRef(*id),
                    Irohad::new(
                        image,
                        PeerEnv::new(key_pair, *ports, chain, topology),
                        *ports,
                        volumes,
                        healthcheck,
                    ),
                )
            })
            .collect()
    }
}

/// Docker Compose configuration.
#[derive(serde::Serialize, Debug)]
pub struct DockerCompose<'a> {
    services: BuildOrPull<'a>,
}

impl<'a> DockerCompose<'a> {
    /// Constructs a Compose configuration.
    pub(super) fn new(
        ImageSettings {
            name,
            build_dir,
            ignore_cache,
        }: &'a ImageSettings,
        PeerSettings {
            healthcheck,
            config_dir,
            chain,
            network,
            topology,
        }: &'a PeerSettings,
    ) -> Self {
        let image = ImageId(name);
        let volumes = [PathMapping(
            HostPath(config_dir),
            ContainerPath(CONTAINER_CONFIG_DIR),
        )];
        Self {
            services: build_dir.as_ref().map_or_else(
                || {
                    BuildOrPull::pull(
                        PulledImage::new(image, *ignore_cache),
                        volumes,
                        *healthcheck,
                        chain,
                        network,
                        topology,
                    )
                },
                |build| {
                    BuildOrPull::build(
                        BuildImage::new(image, HostPath(build), *ignore_cache),
                        volumes,
                        *healthcheck,
                        chain,
                        network,
                        topology,
                    )
                },
            ),
        }
    }

    /// Serializes the schema into a writer as YAML, with optional `banner` comment lines.
    pub fn write<W>(self, mut writer: W, banner: Option<&[&str]>) -> Result<(), Error>
    where
        W: std::io::Write,
    {
        if let Some(banner) = banner {
            for line in banner {
                writeln!(writer, "# {line}").map_err(Error::BannerWrite)?;
            }
            writeln!(writer).map_err(Error::BannerWrite)?;
        }
        serde_yaml::to_writer(writer, &self).map_err(Error::SerdeYaml)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use iroha_crypto::Hash;

    use super::*;
    use crate::{BASE_PORT_API, BASE_PORT_P2P};

    impl<'a> From<PeerEnv<'a>> for iroha_config::base::env::MockEnv {
        fn from(env: PeerEnv<'a>) -> Self {
            let json = serde_json::to_string(&env).expect("should be serializable");
            let mut map: HashMap<String, String> =
                serde_json::from_str(&json).expect("should be deserializable into a map");
            // GENESIS_HASH will be set via `export ...` in command
            map.insert("GENESIS_HASH".to_owned(), Hash::new([]).to_string());
            Self::with_map(map)
        }
    }

    #[test]
    fn peer_env_produces_exhaustive_config() {
        let key_pair = peer::generate_key_pair(None, &[]);
        let ports = [BASE_PORT_P2P, BASE_PORT_API];
        let chain = peer::chain();
        let topology = [peer::peer_id("dummy", BASE_PORT_API, key_pair.0.clone())].into();
        let env = PeerEnv::new(&key_pair, ports, &chain, &topology);
        let mock_env = iroha_config::base::env::MockEnv::from(env);
        let _ = iroha_config::base::read::ConfigReader::new()
            .with_env(mock_env.clone())
            .read_and_complete::<iroha_config::parameters::user::Root>()
            .expect("config in env should be exhaustive");
        assert_eq!(
            mock_env.unvisited(),
            ["TOPOLOGY"].into_iter().map(ToOwned::to_owned).collect()
        );
    }
}
