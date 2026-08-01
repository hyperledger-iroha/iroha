//! Docker Compose schema.

use norito::json::{self, Map, Value};

use crate::{GenesisArtifactSettings, ImageSettings, PeerSettings, path, peer};

fn peer_env_to_value(env: &PeerEnv<'_>) -> norito::json::Value {
    let mut map = Map::new();
    map.insert(
        "CHAIN".into(),
        json::to_value(env.chain).expect("serialize chain id"),
    );
    map.insert(
        "PUBLIC_KEY".into(),
        json::to_value(env.public_key).expect("serialize public key"),
    );
    map.insert(
        "PRIVATE_KEY".into(),
        json::to_value(env.private_key).expect("serialize private key"),
    );
    map.insert(
        "P2P_PUBLIC_ADDRESS".into(),
        Value::String(env.p2p_public_address.to_string()),
    );
    map.insert(
        "P2P_ADDRESS".into(),
        Value::String(env.p2p_address.to_string()),
    );
    map.insert(
        "API_ADDRESS".into(),
        Value::String(env.api_address.to_string()),
    );
    if !env.trusted_peers.is_empty() {
        let peers: Vec<String> = env
            .trusted_peers
            .iter()
            .map(|peer| peer.id().to_string())
            .collect();
        let trusted = json::to_json(&peers).expect("serialize trusted peers list");
        map.insert("TRUSTED_PEERS".into(), Value::String(trusted));
    }
    if !env.trusted_peers_pop.is_empty() {
        let mut pops = Vec::with_capacity(env.trusted_peers_pop.len());
        for (pk, pop) in &env.trusted_peers_pop {
            let mut entry = Map::new();
            entry.insert("public_key".into(), Value::String(pk.to_string()));
            entry.insert(
                "pop_hex".into(),
                Value::String(format!("0x{}", encode_hex(pop))),
            );
            pops.push(Value::Object(entry));
        }
        let trusted = json::to_json(&pops).expect("serialize trusted peers PoP list");
        map.insert("TRUSTED_PEERS_POP".into(), Value::String(trusted));
    }

    Value::Object(map)
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        write!(&mut out, "{byte:02x}").expect("format hex byte");
    }
    out
}

fn trusted_peers_pop_map(
    network: &std::collections::BTreeMap<u16, peer::PeerInfo>,
) -> std::collections::BTreeMap<iroha_crypto::PublicKey, Vec<u8>> {
    let mut pops = std::collections::BTreeMap::new();
    for (_, _, (public_key, _), pop) in network.values() {
        pops.insert(public_key.clone(), pop.clone());
    }
    pops
}

#[cfg(test)]
mod json_value_tests {
    use norito::json::{self, Map, Value};

    use super::*;
    use crate::peer;

    type SampleTopology = (
        peer::ExposedKeyPair,
        [u16; 2],
        iroha_data_model::ChainId,
        std::collections::BTreeSet<iroha_data_model::prelude::Peer>,
        std::collections::BTreeMap<iroha_crypto::PublicKey, Vec<u8>>,
    );

    fn sample_topology() -> SampleTopology {
        let chain = peer::chain();
        let (primary_pair, primary_pop) =
            peer::generate_bls_key_pair(Some(b"swarm-json-primary"), b"node-0")
                .expect("seeded primary BLS key generation should succeed");
        let (secondary_pair, secondary_pop) =
            peer::generate_bls_key_pair(Some(b"swarm-json-secondary"), b"node-1")
                .expect("seeded secondary BLS key generation should succeed");
        let ports = [crate::BASE_PORT_P2P, crate::BASE_PORT_API];
        let other_ports = [crate::BASE_PORT_P2P + 1, crate::BASE_PORT_API + 1];
        let mut topology = std::collections::BTreeSet::new();
        topology.insert(peer::peer("irohad0", ports[0], primary_pair.0.clone()));
        topology.insert(peer::peer(
            "irohad1",
            other_ports[0],
            secondary_pair.0.clone(),
        ));
        let mut trusted_pops = std::collections::BTreeMap::new();
        trusted_pops.insert(primary_pair.0.clone(), primary_pop);
        trusted_pops.insert(secondary_pair.0.clone(), secondary_pop);
        (primary_pair, ports, chain, topology, trusted_pops)
    }

    #[test]
    fn peer_env_to_value_matches_expected_fields() {
        let (primary_pair, ports, chain, topology, trusted_pops) = sample_topology();
        let env = PeerEnv::new(
            &primary_pair,
            ports,
            &chain,
            &topology,
            trusted_pops.clone(),
        );
        let actual = peer_env_to_value(&env);

        let mut expected = Map::new();
        expected.insert("CHAIN".into(), json::to_value(env.chain).unwrap());
        expected.insert("PUBLIC_KEY".into(), json::to_value(env.public_key).unwrap());
        expected.insert(
            "PRIVATE_KEY".into(),
            json::to_value(env.private_key).unwrap(),
        );
        expected.insert(
            "P2P_PUBLIC_ADDRESS".into(),
            Value::String(env.p2p_public_address.to_string()),
        );
        expected.insert(
            "P2P_ADDRESS".into(),
            Value::String(env.p2p_address.to_string()),
        );
        expected.insert(
            "API_ADDRESS".into(),
            Value::String(env.api_address.to_string()),
        );
        if !env.trusted_peers.is_empty() {
            let peers: Vec<String> = env
                .trusted_peers
                .iter()
                .map(|peer| peer.id().to_string())
                .collect();
            let trusted = json::to_json(&peers).unwrap();
            expected.insert("TRUSTED_PEERS".into(), Value::String(trusted.clone()));
            // Ensure the embedded JSON string remains valid Norito JSON.
            let parsed = json::parse_value(&trusted).expect("parse trusted peers JSON");
            assert!(matches!(parsed, Value::Array(_)));
        }
        if !env.trusted_peers_pop.is_empty() {
            let mut pops = Vec::new();
            for (pk, pop) in &env.trusted_peers_pop {
                let mut entry = Map::new();
                entry.insert("public_key".into(), Value::String(pk.to_string()));
                entry.insert(
                    "pop_hex".into(),
                    Value::String(format!("0x{}", encode_hex(pop))),
                );
                pops.push(Value::Object(entry));
            }
            let trusted = json::to_json(&pops).unwrap();
            expected.insert("TRUSTED_PEERS_POP".into(), Value::String(trusted.clone()));
            let parsed = json::parse_value(&trusted).expect("parse trusted peers pop JSON");
            assert!(matches!(parsed, Value::Array(_)));
        }

        assert_eq!(actual, Value::Object(expected));
    }
}

trait ComposeImageFields {
    fn into_fields(self) -> norito::json::Map;
}

/// Schema serialization error.
#[derive(displaydoc::Display, Debug)]
pub enum Error {
    /// Could not write the banner: {0}
    BannerWrite(std::io::Error),
    /// Could not serialize the schema: {0}
    Yaml(norito::yaml::Error),
}

impl std::error::Error for Error {}

/// Image identifier.
#[derive(Copy, Clone, Debug)]
struct ImageId<'a>(&'a str);

impl ImageId<'_> {
    fn as_value(self) -> norito::json::Value {
        norito::json::Value::String(self.0.to_owned())
    }
}

/// Dictates how the image provider will build the image from a Dockerfile.
#[derive(Copy, Clone, Debug)]
enum Build {
    /// Rebuild the image, ignoring the local cache.
    IgnoreCache,
    /// Only build the image when it is missing from the local cache.
    OnCacheMiss,
}

impl Build {
    fn as_str(self) -> &'static str {
        match self {
            Self::IgnoreCache => "build",
            Self::OnCacheMiss => "never",
        }
    }
}

/// Dictates that a service must use the built image.
#[derive(Copy, Clone, Debug)]
enum UseBuilt {
    UseCached,
}

impl UseBuilt {
    fn is_on_cache_miss(self) -> bool {
        match self {
            Self::UseCached => false,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::UseCached => "never",
        }
    }
}

/// Dictates how a service will pull the image from Docker Hub.
#[derive(Copy, Clone, Debug)]
enum Pull {
    /// Always pull the image, ignoring the local cache.
    IgnoreCache,
    /// Only pull the image when it is missing from the local cache.
    OnCacheMiss,
}

impl Pull {
    fn as_str(self) -> &'static str {
        match self {
            Self::IgnoreCache => "always",
            Self::OnCacheMiss => "missing",
        }
    }
}

/// Path on the host.
#[derive(Copy, Clone, Debug)]
struct HostPath<'a>(&'a path::RelativePath);

/// Image build settings.
#[derive(Copy, Clone, Debug)]
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

impl ComposeImageFields for BuildImage<'_> {
    fn into_fields(self) -> norito::json::Map {
        let mut map = norito::json::Map::new();
        map.insert("image".into(), self.image.as_value());
        map.insert(
            "build".into(),
            norito::json::Value::String(self.build.0.as_ref().display().to_string()),
        );
        map.insert(
            "pull_policy".into(),
            norito::json::Value::String(self.pull_policy.as_str().into()),
        );
        map
    }
}

/// Image that has been built.
#[derive(Copy, Clone, Debug)]
struct BuiltImage<'a> {
    image: ImageId<'a>,
    pull_policy: UseBuilt,
}

/// Image that has been pulled.
#[derive(Copy, Clone, Debug)]
struct PulledImage<'a> {
    image: ImageId<'a>,
    pull_policy: Pull,
}

impl<'a> BuiltImage<'a> {
    fn new(image: ImageId<'a>) -> Self {
        Self {
            image,
            pull_policy: UseBuilt::UseCached,
        }
    }
}

impl ComposeImageFields for BuiltImage<'_> {
    fn into_fields(self) -> norito::json::Map {
        let mut map = norito::json::Map::new();
        map.insert("image".into(), self.image.as_value());
        if !self.pull_policy.is_on_cache_miss() {
            map.insert(
                "pull_policy".into(),
                norito::json::Value::String(self.pull_policy.as_str().into()),
            );
        }
        map
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

impl ComposeImageFields for PulledImage<'_> {
    fn into_fields(self) -> norito::json::Map {
        let mut map = norito::json::Map::new();
        map.insert("image".into(), self.image.as_value());
        map.insert(
            "pull_policy".into(),
            norito::json::Value::String(self.pull_policy.as_str().into()),
        );
        map
    }
}

/// Peer environment variables.
#[derive(Debug)]
struct PeerEnv<'a> {
    chain: &'a iroha_data_model::ChainId,
    public_key: &'a iroha_crypto::PublicKey,
    private_key: &'a iroha_crypto::ExposedPrivateKey,
    p2p_public_address: iroha_primitives::addr::SocketAddr,
    p2p_address: iroha_primitives::addr::SocketAddr,
    api_address: iroha_primitives::addr::SocketAddr,
    trusted_peers: std::collections::BTreeSet<&'a iroha_data_model::peer::Peer>,
    trusted_peers_pop: std::collections::BTreeMap<iroha_crypto::PublicKey, Vec<u8>>,
}

impl<'a> PeerEnv<'a> {
    fn new(
        (public_key, private_key): &'a peer::ExposedKeyPair,
        [port_p2p, port_api]: [u16; 2],
        chain: &'a iroha_data_model::ChainId,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::Peer>,
        trusted_peers_pop: std::collections::BTreeMap<iroha_crypto::PublicKey, Vec<u8>>,
    ) -> Self {
        let p2p_public_address = topology
            .iter()
            .find(|&peer| peer.id().public_key() == public_key)
            .unwrap()
            .address()
            .clone();
        Self {
            chain,
            public_key,
            private_key,
            p2p_public_address,
            p2p_address: iroha_primitives::addr::socket_addr!(0.0.0.0:port_p2p),
            api_address: iroha_primitives::addr::socket_addr!(0.0.0.0:port_api),
            trusted_peers: topology
                .iter()
                .filter(|&peer| peer.id().public_key() != public_key)
                .collect(),
            trusted_peers_pop,
        }
    }
}

/// Mapping between `host:container` ports.
#[derive(Debug)]
struct PortMapping(u16, u16);

const GENESIS_PUBLIC_KEY_SECRET: &str = "iroha_genesis_public_key";
const GENESIS_EXPECTED_HASH_SECRET: &str = "iroha_genesis_expected_hash";
const CONTAINER_SIGNED_GENESIS: &str = "/genesis/genesis.signed.nrt";
const GENESIS_PUBLIC_KEY_FILE_SOURCE: &str = "${IROHA_GENESIS_PUBLIC_KEY_FILE:?set IROHA_GENESIS_PUBLIC_KEY_FILE to an owner-controlled genesis public-key file}";
const GENESIS_EXPECTED_HASH_FILE_SOURCE: &str = "${IROHA_GENESIS_EXPECTED_HASH_FILE:?set IROHA_GENESIS_EXPECTED_HASH_FILE to an owner-controlled exact genesis hash file}";
const GENESIS_SIGNED_FILE_SOURCE: &str = "${IROHA_GENESIS_SIGNED_FILE:?set IROHA_GENESIS_SIGNED_FILE to an owner-prepared signed genesis block}";

fn artifact_source<'a>(
    settings: &'a GenesisArtifactSettings,
    prepared: impl FnOnce(
        &'a path::RelativePath,
        &'a path::RelativePath,
        &'a path::RelativePath,
    ) -> &'a path::RelativePath,
    environment: &'static str,
) -> String {
    match settings {
        GenesisArtifactSettings::Environment => environment.to_owned(),
        GenesisArtifactSettings::Prepared {
            signed_block,
            public_key,
            expected_hash,
        } => prepared(signed_block, public_key, expected_hash)
            .as_ref()
            .display()
            .to_string(),
    }
}

fn signed_block_source(settings: &GenesisArtifactSettings) -> String {
    artifact_source(
        settings,
        |signed_block, _, _| signed_block,
        GENESIS_SIGNED_FILE_SOURCE,
    )
}

fn public_key_source(settings: &GenesisArtifactSettings) -> String {
    artifact_source(
        settings,
        |_, public_key, _| public_key,
        GENESIS_PUBLIC_KEY_FILE_SOURCE,
    )
}

fn expected_hash_source(settings: &GenesisArtifactSettings) -> String {
    artifact_source(
        settings,
        |_, _, expected_hash| expected_hash,
        GENESIS_EXPECTED_HASH_FILE_SOURCE,
    )
}

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

impl Healthcheck {
    fn into_value(self) -> Value {
        let mut map = norito::json::Map::new();
        map.insert(
            "test".into(),
            Value::String(format!(
                "test $$(curl -s http://127.0.0.1:{}/status/blocks) -gt 0",
                self.port
            )),
        );
        map.insert(
            "interval".into(),
            Value::String(HEALTH_CHECK_INTERVAL.into()),
        );
        map.insert("timeout".into(), Value::String(HEALTH_CHECK_TIMEOUT.into()));
        map.insert(
            "retries".into(),
            Value::Number(norito::json::Number::from(u64::from(HEALTH_CHECK_RETRIES))),
        );
        map.insert(
            "start_period".into(),
            Value::String(HEALTH_CHECK_START_PERIOD.into()),
        );
        Value::Object(map)
    }
}

fn secret_names() -> Value {
    Value::Array(vec![
        Value::String(GENESIS_PUBLIC_KEY_SECRET.into()),
        Value::String(GENESIS_EXPECTED_HASH_SECRET.into()),
    ])
}

fn compose_secrets(settings: &GenesisArtifactSettings) -> Value {
    let mut public = Map::new();
    public.insert("file".into(), Value::String(public_key_source(settings)));
    let mut expected_hash = Map::new();
    expected_hash.insert("file".into(), Value::String(expected_hash_source(settings)));
    let mut secrets = Map::new();
    secrets.insert(GENESIS_PUBLIC_KEY_SECRET.into(), Value::Object(public));
    secrets.insert(
        GENESIS_EXPECTED_HASH_SECRET.into(),
        Value::Object(expected_hash),
    );
    Value::Object(secrets)
}

fn signed_genesis_mount(settings: &GenesisArtifactSettings) -> Value {
    let mut mount = Map::new();
    mount.insert("type".into(), Value::String("bind".into()));
    mount.insert(
        "source".into(),
        Value::String(signed_block_source(settings)),
    );
    mount.insert(
        "target".into(),
        Value::String(CONTAINER_SIGNED_GENESIS.into()),
    );
    mount.insert("read_only".into(), Value::Bool(true));
    Value::Object(mount)
}

const LOAD_SIGNED_GENESIS_AND_RUN: &str = r#"/bin/sh -eu -c "
    GENESIS_PUBLIC_KEY_FILE=/run/secrets/iroha_genesis_public_key && \\
    GENESIS=/genesis/genesis.signed.nrt && \\
    GENESIS_EXPECTED_HASH_FILE=/run/secrets/iroha_genesis_expected_hash && \\
    test -s \"$$GENESIS\" && \\
    test -s \"$$GENESIS_EXPECTED_HASH_FILE\" && \\
    test -s \"$$GENESIS_PUBLIC_KEY_FILE\" && \\
    test \"$$(wc -l < \"$$GENESIS_PUBLIC_KEY_FILE\")\" -eq 1 && \\
    test -z \"$$(tail -c 1 < \"$$GENESIS_PUBLIC_KEY_FILE\")\" && \\
    IFS= read -r GENESIS_PUBLIC_KEY < \"$$GENESIS_PUBLIC_KEY_FILE\" && \\
    test -n \"$$GENESIS_PUBLIC_KEY\" && \\
    printf '%s\n' \"$$GENESIS_PUBLIC_KEY\" | grep -Eq '^[^[:space:]]+$$' && \\
    test \"$$(wc -l < \"$$GENESIS_EXPECTED_HASH_FILE\")\" -eq 1 && \\
    test -z \"$$(tail -c 1 < \"$$GENESIS_EXPECTED_HASH_FILE\")\" && \\
    IFS= read -r GENESIS_EXPECTED_HASH < \"$$GENESIS_EXPECTED_HASH_FILE\" && \\
    printf '%s\n' \"$$GENESIS_EXPECTED_HASH\" | grep -Eq '^[0-9a-f]{63}[13579bdf]$$' && \\
    export GENESIS_PUBLIC_KEY GENESIS GENESIS_EXPECTED_HASH && \\
    exec irohad
""#;

/// Iroha peer service.
#[derive(Debug)]
struct Irohad<'a, Image>
where
    Image: ComposeImageFields,
{
    image: Image,
    environment: PeerEnv<'a>,
    ports: [PortMapping; 2],
    healthcheck: Option<Healthcheck>,
    genesis: &'a GenesisArtifactSettings,
}

impl<'a, Image> Irohad<'a, Image>
where
    Image: ComposeImageFields,
{
    fn new(
        image: Image,
        environment: PeerEnv<'a>,
        [port_p2p, port_api]: [u16; 2],
        healthcheck: bool,
        genesis: &'a GenesisArtifactSettings,
    ) -> Self {
        Self {
            image,
            environment,
            ports: [
                PortMapping(port_p2p, port_p2p),
                PortMapping(port_api, port_api),
            ],
            healthcheck: healthcheck.then_some(Healthcheck { port: port_api }),
            genesis,
        }
    }

    fn into_map(self) -> norito::json::Map {
        let mut map = self.image.into_fields();
        map.insert("environment".into(), peer_env_to_value(&self.environment));
        map.insert(
            "ports".into(),
            norito::json::Value::Array(
                self.ports
                    .into_iter()
                    .map(|mapping| {
                        norito::json::Value::String(format!("{}:{}", mapping.0, mapping.1))
                    })
                    .collect(),
            ),
        );
        map.insert(
            "volumes".into(),
            Value::Array(vec![signed_genesis_mount(self.genesis)]),
        );
        map.insert(
            "command".into(),
            Value::String(LOAD_SIGNED_GENESIS_AND_RUN.into()),
        );
        map.insert("secrets".into(), secret_names());
        map.insert("init".into(), norito::json::Value::Bool(true));
        if let Some(healthcheck) = self.healthcheck {
            map.insert("healthcheck".into(), healthcheck.into_value());
        }
        map
    }
}

/// Reference to an `irohad` service.
#[derive(Debug, PartialOrd, PartialEq, Ord, Eq)]
struct IrohadRef(String);

impl IrohadRef {
    fn service_name(&self) -> String {
        self.0.clone()
    }
}

#[derive(Debug)]
enum BuildOrPull<'a> {
    Build {
        primary: (IrohadRef, Irohad<'a, BuildImage<'a>>),
        irohads: std::collections::BTreeMap<IrohadRef, Irohad<'a, BuiltImage<'a>>>,
    },
    Pull {
        irohads: std::collections::BTreeMap<IrohadRef, Irohad<'a, PulledImage<'a>>>,
    },
}

impl<'a> BuildOrPull<'a> {
    fn pull(
        image: PulledImage<'a>,
        healthcheck: bool,
        genesis: &'a GenesisArtifactSettings,
        chain: &'a iroha_data_model::ChainId,
        network: &'a std::collections::BTreeMap<u16, peer::PeerInfo>,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::Peer>,
    ) -> Self {
        let trusted_peers_pop = trusted_peers_pop_map(network);
        Self::Pull {
            irohads: Self::irohads(
                image,
                healthcheck,
                genesis,
                chain,
                network,
                topology,
                &trusted_peers_pop,
            ),
        }
    }

    fn build(
        image: BuildImage<'a>,
        healthcheck: bool,
        genesis: &'a GenesisArtifactSettings,
        chain: &'a iroha_data_model::ChainId,
        network: &'a std::collections::BTreeMap<u16, peer::PeerInfo>,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::Peer>,
    ) -> Self {
        let trusted_peers_pop = trusted_peers_pop_map(network);
        let mut peers = network.iter();
        let (_, primary_info) = peers
            .next()
            .expect("a swarm always contains at least one validator");
        Self::Build {
            primary: (
                IrohadRef(primary_info.0.clone()),
                Self::irohad(
                    image,
                    healthcheck,
                    genesis,
                    chain,
                    topology,
                    &trusted_peers_pop,
                    primary_info,
                ),
            ),
            irohads: peers
                .map(|(_, info)| {
                    (
                        IrohadRef(info.0.clone()),
                        Self::irohad(
                            BuiltImage::new(image.image),
                            healthcheck,
                            genesis,
                            chain,
                            topology,
                            &trusted_peers_pop,
                            info,
                        ),
                    )
                })
                .collect(),
        }
    }

    fn irohad<Image: ComposeImageFields>(
        image: Image,
        healthcheck: bool,
        genesis: &'a GenesisArtifactSettings,
        chain: &'a iroha_data_model::ChainId,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::Peer>,
        trusted_peers_pop: &std::collections::BTreeMap<iroha_crypto::PublicKey, Vec<u8>>,
        (_, ports, key_pair, _): &'a peer::PeerInfo,
    ) -> Irohad<'a, Image> {
        Irohad::new(
            image,
            PeerEnv::new(key_pair, *ports, chain, topology, trusted_peers_pop.clone()),
            *ports,
            healthcheck,
            genesis,
        )
    }

    fn irohads<Image: ComposeImageFields + Copy>(
        image: Image,
        healthcheck: bool,
        genesis: &'a GenesisArtifactSettings,
        chain: &'a iroha_data_model::ChainId,
        network: &'a std::collections::BTreeMap<u16, peer::PeerInfo>,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::Peer>,
        trusted_peers_pop: &std::collections::BTreeMap<iroha_crypto::PublicKey, Vec<u8>>,
    ) -> std::collections::BTreeMap<IrohadRef, Irohad<'a, Image>> {
        network
            .iter()
            .map(|(_, info)| {
                (
                    IrohadRef(info.0.clone()),
                    Self::irohad(
                        image,
                        healthcheck,
                        genesis,
                        chain,
                        topology,
                        trusted_peers_pop,
                        info,
                    ),
                )
            })
            .collect()
    }

    fn into_services_map(self) -> norito::json::Map {
        let mut services = norito::json::Map::new();
        match self {
            BuildOrPull::Build { primary, irohads } => {
                let (service_ref, service) = primary;
                services.insert(
                    service_ref.service_name(),
                    norito::json::Value::Object(service.into_map()),
                );
                for (service_ref, service) in irohads {
                    services.insert(
                        service_ref.service_name(),
                        norito::json::Value::Object(service.into_map()),
                    );
                }
            }
            BuildOrPull::Pull { irohads } => {
                for (service_ref, service) in irohads {
                    services.insert(
                        service_ref.service_name(),
                        norito::json::Value::Object(service.into_map()),
                    );
                }
            }
        }
        services
    }
}

/// Docker Compose configuration.
#[derive(Debug)]
pub struct DockerCompose<'a> {
    services: BuildOrPull<'a>,
    genesis: &'a GenesisArtifactSettings,
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
            chain,
            network,
            topology,
        }: &'a PeerSettings,
        genesis: &'a GenesisArtifactSettings,
    ) -> Self {
        let image = ImageId(name);
        Self {
            services: build_dir.as_ref().map_or_else(
                || {
                    BuildOrPull::pull(
                        PulledImage::new(image, *ignore_cache),
                        *healthcheck,
                        genesis,
                        chain,
                        network,
                        topology,
                    )
                },
                |build| {
                    BuildOrPull::build(
                        BuildImage::new(image, HostPath(build), *ignore_cache),
                        *healthcheck,
                        genesis,
                        chain,
                        network,
                        topology,
                    )
                },
            ),
            genesis,
        }
    }

    fn into_value(self) -> norito::json::Value {
        let mut root = norito::json::Map::new();
        root.insert("secrets".into(), compose_secrets(self.genesis));
        root.insert(
            "services".into(),
            norito::json::Value::Object(self.services.into_services_map()),
        );
        norito::json::Value::Object(root)
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
        let value = self.into_value();
        norito::yaml::to_writer_from_value(&mut writer, &value).map_err(Error::Yaml)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use super::*;
    use crate::{BASE_PORT_API, BASE_PORT_P2P};

    impl<'a> From<PeerEnv<'a>> for iroha_config::base::env::MockEnv {
        fn from(env: PeerEnv<'a>) -> Self {
            mock_env_from_value(peer_env_to_value(&env))
        }
    }

    fn mock_env_from_value(value: Value) -> iroha_config::base::env::MockEnv {
        let Value::Object(map) = value else {
            panic!("environment payload must be an object");
        };
        let mut vars = HashMap::new();
        for (key, value) in map {
            let value = match value {
                Value::String(s) => s,
                other => json::to_json(&other).expect("serialize environment value"),
            };
            vars.insert(key, value);
        }
        iroha_config::base::env::MockEnv::with_map(vars)
    }

    #[test]
    fn peer_env_produces_exhaustive_config() {
        let (key_pair, pop) = peer::generate_bls_key_pair(None, &[])
            .expect("random BLS key generation should succeed");
        let mut trusted_pops = BTreeMap::new();
        trusted_pops.insert(key_pair.0.clone(), pop);
        let genesis_public_key = peer::generate_key_pair(None, &[])
            .expect("random genesis key generation should succeed")
            .0;
        let ports = [BASE_PORT_P2P, BASE_PORT_API];
        let chain = peer::chain();
        let topology = [peer::peer("dummy", BASE_PORT_API, key_pair.0.clone())].into();
        let env = PeerEnv::new(&key_pair, ports, &chain, &topology, trusted_pops);
        let mut value = peer_env_to_value(&env);
        let Value::Object(ref mut map) = value else {
            unreachable!("peer environment is an object");
        };
        map.insert(
            "GENESIS_PUBLIC_KEY".into(),
            Value::String(genesis_public_key.to_string()),
        );
        map.insert(
            "GENESIS_EXPECTED_HASH".into(),
            Value::String(
                "0000000000000000000000000000000000000000000000000000000000000001".to_owned(),
            ),
        );
        let mock_env = mock_env_from_value(value);
        let reader = iroha_config::base::read::ConfigReader::new().with_env(mock_env.clone());
        let _ = iroha_config::parameters::user::Root::read_and_complete(reader)
            .expect("config in env should be exhaustive");
        assert!(mock_env.unvisited().is_empty());
    }
}
