//! Docker Compose schema.

use norito::json::{self, Map, Value};

use crate::{ImageSettings, PeerSettings, path, peer};

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
    map.insert(
        "GENESIS_PUBLIC_KEY".into(),
        json::to_value(env.genesis_public_key).expect("serialize genesis public key"),
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

    Value::Object(map)
}

fn genesis_env_to_value(env: &GenesisEnv<'_>) -> norito::json::Value {
    use norito::json::{self, Value};

    let mut value = peer_env_to_value(&env.base);
    if let Value::Object(ref mut map) = value {
        if let Some(mode) = env.consensus_mode {
            map.insert(
                "GENESIS_CONSENSUS_MODE".into(),
                Value::String(mode.to_owned()),
            );
        }
        if let Some(mode) = env.next_consensus_mode {
            map.insert(
                "GENESIS_NEXT_CONSENSUS_MODE".into(),
                Value::String(mode.to_owned()),
            );
        }
        if let Some(height) = env.mode_activation_height {
            map.insert(
                "GENESIS_MODE_ACTIVATION_HEIGHT".into(),
                Value::String(height.to_string()),
            );
        }
        if !env.peer_pops.is_empty() {
            map.insert(
                "GENESIS_PEER_POPS".into(),
                Value::String(format_peer_pop_args(&env.peer_pops)),
            );
        }
        map.insert(
            "GENESIS_PRIVATE_KEY".into(),
            json::to_value(env.genesis_private_key).expect("serialize genesis private key"),
        );
        map.insert("GENESIS".into(), Value::String(env.genesis.to_string()));
        let ids: Vec<String> = env
            .topology
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        let topology = json::to_json(&ids).expect("serialize topology list");
        map.insert("TOPOLOGY".into(), Value::String(topology));
    }
    value
}

fn format_peer_pop_args(entries: &[String]) -> String {
    let mut args = String::new();
    for (idx, entry) in entries.iter().enumerate() {
        if idx > 0 {
            args.push(' ');
        }
        args.push_str("--peer-pop ");
        args.push_str(entry);
    }
    args
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        write!(&mut out, "{byte:02x}").expect("format hex byte");
    }
    out
}

fn peer_pop_entries(network: &std::collections::BTreeMap<u16, peer::PeerInfo>) -> Vec<String> {
    network
        .values()
        .map(|(_, _, (public_key, _), pop)| format!("{public_key}=0x{}", encode_hex(pop)))
        .collect()
}

#[cfg(test)]
mod json_value_tests {
    use norito::json::{self, Map, Value};

    use super::*;
    use crate::peer;

    fn sample_topology() -> (
        peer::ExposedKeyPair,
        [u16; 2],
        peer::ExposedKeyPair,
        iroha_data_model::ChainId,
        std::collections::BTreeSet<iroha_data_model::prelude::Peer>,
    ) {
        let chain = peer::chain();
        let (primary_pair, _primary_pop) =
            peer::generate_bls_key_pair(Some(b"swarm-json-primary"), b"node-0");
        let (secondary_pair, _secondary_pop) =
            peer::generate_bls_key_pair(Some(b"swarm-json-secondary"), b"node-1");
        let ports = [crate::BASE_PORT_P2P, crate::BASE_PORT_API];
        let other_ports = [crate::BASE_PORT_P2P + 1, crate::BASE_PORT_API + 1];
        let mut topology = std::collections::BTreeSet::new();
        topology.insert(peer::peer("irohad0", ports[0], primary_pair.0.clone()));
        topology.insert(peer::peer(
            "irohad1",
            other_ports[0],
            secondary_pair.0.clone(),
        ));
        let genesis_pair = peer::generate_key_pair(Some(b"swarm-json-genesis"), b"genesis-json");
        (primary_pair, ports, genesis_pair, chain, topology)
    }

    #[test]
    fn peer_env_to_value_matches_expected_fields() {
        let (primary_pair, ports, genesis_pair, chain, topology) = sample_topology();
        let env = PeerEnv::new(&primary_pair, ports, &chain, &genesis_pair.0, &topology);
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
        expected.insert(
            "GENESIS_PUBLIC_KEY".into(),
            json::to_value(env.genesis_public_key).unwrap(),
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

        assert_eq!(actual, Value::Object(expected));
    }

    #[test]
    fn genesis_env_to_value_extends_peer_payload() {
        let (primary_pair, ports, genesis_pair, chain, topology) = sample_topology();
        let env = GenesisEnv::new(
            &primary_pair,
            ports,
            &chain,
            (&genesis_pair.0, &genesis_pair.1),
            &topology,
            None,
            None,
            None,
            Vec::new(),
        );

        let actual = genesis_env_to_value(&env);
        let Value::Object(mut expected) = peer_env_to_value(&env.base) else {
            panic!("peer env must serialize to object");
        };
        expected.insert(
            "GENESIS_PRIVATE_KEY".into(),
            json::to_value(env.genesis_private_key).unwrap(),
        );
        expected.insert("GENESIS".into(), Value::String(env.genesis.to_string()));
        let ids: Vec<String> = env
            .topology
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        let topology = json::to_json(&ids).unwrap();
        let parsed = json::parse_value(&topology).expect("parse topology JSON");
        assert!(matches!(parsed, Value::Array(_)));
        expected.insert("TOPOLOGY".into(), Value::String(topology));

        assert_eq!(actual, Value::Object(expected));
    }

    #[test]
    fn genesis_env_includes_consensus_overrides() {
        let (primary_pair, ports, genesis_pair, chain, topology) = sample_topology();
        let env = GenesisEnv::new(
            &primary_pair,
            ports,
            &chain,
            (&genesis_pair.0, &genesis_pair.1),
            &topology,
            Some("npos"),
            Some("npos"),
            Some(42),
            vec!["pk=00".to_string()],
        );

        let Value::Object(map) = genesis_env_to_value(&env) else {
            panic!("genesis env must serialize to object");
        };
        assert_eq!(
            map.get("GENESIS_CONSENSUS_MODE"),
            Some(&Value::String("npos".to_owned()))
        );
        assert_eq!(
            map.get("GENESIS_NEXT_CONSENSUS_MODE"),
            Some(&Value::String("npos".to_owned()))
        );
        assert_eq!(
            map.get("GENESIS_MODE_ACTIVATION_HEIGHT"),
            Some(&Value::String("42".to_owned()))
        );
        assert_eq!(
            map.get("GENESIS_PEER_POPS"),
            Some(&Value::String("--peer-pop pk=00".to_owned()))
        );
    }
}

trait ComposeImageFields {
    fn into_fields(self) -> norito::json::Map;
}

trait ComposeEnvironmentValue {
    fn into_value(self) -> norito::json::Value;
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

const IROHAD0: &str = "irohad0";

/// Image that has been built.
#[derive(Copy, Clone, Debug)]
struct BuiltImage<'a> {
    depends_on: [&'static str; 1],
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
            depends_on: [IROHAD0],
            image,
            pull_policy: UseBuilt::UseCached,
        }
    }
}

impl ComposeImageFields for BuiltImage<'_> {
    fn into_fields(self) -> norito::json::Map {
        let mut map = norito::json::Map::new();
        map.insert(
            "depends_on".into(),
            norito::json::Value::Array(
                self.depends_on
                    .iter()
                    .map(|dep| norito::json::Value::String((*dep).to_owned()))
                    .collect(),
            ),
        );
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
    genesis_public_key: &'a iroha_crypto::PublicKey,
    trusted_peers: std::collections::BTreeSet<&'a iroha_data_model::peer::Peer>,
}

impl<'a> PeerEnv<'a> {
    fn new(
        (public_key, private_key): &'a peer::ExposedKeyPair,
        [port_p2p, port_api]: [u16; 2],
        chain: &'a iroha_data_model::ChainId,
        genesis_public_key: &'a iroha_crypto::PublicKey,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::Peer>,
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
            genesis_public_key,
            trusted_peers: topology
                .iter()
                .filter(|&peer| peer.id().public_key() != public_key)
                .collect(),
        }
    }
}

impl ComposeEnvironmentValue for PeerEnv<'_> {
    fn into_value(self) -> Value {
        peer_env_to_value(&self)
    }
}

#[derive(Debug)]
struct GenesisEnv<'a> {
    base: PeerEnv<'a>,
    genesis_private_key: &'a iroha_crypto::ExposedPrivateKey,
    genesis: ContainerFile<'a>,
    topology: std::collections::BTreeSet<&'a iroha_data_model::peer::PeerId>,
    consensus_mode: Option<&'a str>,
    next_consensus_mode: Option<&'a str>,
    mode_activation_height: Option<u64>,
    peer_pops: Vec<String>,
}

impl<'a> GenesisEnv<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        key_pair: &'a peer::ExposedKeyPair,
        ports: [u16; 2],
        chain: &'a iroha_data_model::ChainId,
        (genesis_public_key, genesis_private_key): peer::ExposedKeyRefPair<'a>,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::Peer>,
        consensus_mode: Option<&'a str>,
        next_consensus_mode: Option<&'a str>,
        mode_activation_height: Option<u64>,
        peer_pops: Vec<String>,
    ) -> Self {
        Self {
            base: PeerEnv::new(key_pair, ports, chain, genesis_public_key, topology),
            genesis_private_key,
            genesis: CONTAINER_SIGNED_GENESIS,
            topology: topology
                .iter()
                .map(iroha_data_model::prelude::Peer::id)
                .collect(),
            consensus_mode,
            next_consensus_mode,
            mode_activation_height,
            peer_pops,
        }
    }
}

impl ComposeEnvironmentValue for GenesisEnv<'_> {
    fn into_value(self) -> Value {
        genesis_env_to_value(&self)
    }
}

/// Mapping between `host:container` ports.
#[derive(Debug)]
struct PortMapping(u16, u16);

#[derive(Copy, Clone, Debug)]
struct Filename<'a>(&'a str);

impl std::fmt::Display for Filename<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Path on the host.
#[derive(Copy, Clone, Debug)]
struct HostFile<'a>(&'a path::RelativePath, Filename<'a>);

impl std::fmt::Display for HostFile<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let joined = self.0.as_ref().join((self.1).0);
        write!(f, "{}", joined.display())
    }
}

/// Path inside the container.
#[derive(Copy, Clone, Debug)]
struct ContainerPath<'a>(&'a str);

impl std::fmt::Display for ContainerPath<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Path inside the container.
#[derive(Copy, Clone, Debug)]
struct ContainerFile<'a>(ContainerPath<'a>, Filename<'a>);

impl std::fmt::Display for ContainerFile<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}{}", self.0, self.1))
    }
}

const GENESIS_FILE: Filename = Filename("genesis.json");
const CONFIG_FILE: Filename = Filename("client.toml");
const GENESIS_SIGNED_NRT: Filename = Filename("genesis.signed.nrt");

const CONTAINER_CONFIG_DIR: ContainerPath = ContainerPath("/config/");
const CONTAINER_TMP_DIR: ContainerPath = ContainerPath("/tmp/");

const CONTAINER_GENESIS_CONFIG: ContainerFile = ContainerFile(CONTAINER_CONFIG_DIR, GENESIS_FILE);
const CONTAINER_CLIENT_CONFIG: ContainerFile = ContainerFile(CONTAINER_CONFIG_DIR, CONFIG_FILE);
const CONTAINER_SIGNED_GENESIS: ContainerFile =
    ContainerFile(CONTAINER_TMP_DIR, GENESIS_SIGNED_NRT);

#[derive(Copy, Clone, Debug)]
struct ReadOnly;

/// Mapping between `host:container` paths.
#[derive(Copy, Clone, Debug)]
struct PathMapping<'a>(HostFile<'a>, ContainerFile<'a>, ReadOnly);

/// Mapping between host and container paths.
type Volumes<'a> = [PathMapping<'a>; 2];

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
                "test $(curl -s http://127.0.0.1:{}/status/blocks) -gt 0",
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

/// Iroha peer service.
#[derive(Debug)]
struct Irohad<'a, Image, Environment = PeerEnv<'a>>
where
    Image: ComposeImageFields,
    Environment: ComposeEnvironmentValue,
{
    image: Image,
    environment: Environment,
    ports: [PortMapping; 2],
    volumes: Volumes<'a>,
    healthcheck: Option<Healthcheck>,
}

impl<'a, Image, Environment> Irohad<'a, Image, Environment>
where
    Image: ComposeImageFields,
    Environment: ComposeEnvironmentValue,
{
    fn new(
        image: Image,
        environment: Environment,
        [port_p2p, port_api]: [u16; 2],
        volumes: Volumes<'a>,
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
            healthcheck: healthcheck.then_some(Healthcheck { port: port_api }),
        }
    }

    fn into_map(self) -> norito::json::Map {
        let mut map = self.image.into_fields();
        map.insert("environment".into(), self.environment.into_value());
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
            norito::json::Value::Array(
                self.volumes
                    .into_iter()
                    .map(|mapping| {
                        norito::json::Value::String(format!("{}:{}:ro", mapping.0, mapping.1))
                    })
                    .collect(),
            ),
        );
        map.insert("init".into(), norito::json::Value::Bool(true));
        if let Some(healthcheck) = self.healthcheck {
            map.insert("healthcheck".into(), healthcheck.into_value());
        }
        map
    }
}

const SIGN_AND_SUBMIT_GENESIS: &str = r#"/bin/sh -c "
    EXECUTOR_RELATIVE_PATH=$(jq -r '.executor // empty' /config/genesis.json) && \\
    if [ -n \"$$EXECUTOR_RELATIVE_PATH\" ]; then EXECUTOR_ABSOLUTE_PATH=$(realpath \"/config/$$EXECUTOR_RELATIVE_PATH\"); else EXECUTOR_ABSOLUTE_PATH=; fi && \\
    IVM_DIR_RELATIVE_PATH=$(jq -r '.ivm_dir // empty' /config/genesis.json) && \\
    if [ -n \"$$IVM_DIR_RELATIVE_PATH\" ]; then IVM_DIR_ABSOLUTE_PATH=$(realpath \"/config/$$IVM_DIR_RELATIVE_PATH\"); else IVM_DIR_ABSOLUTE_PATH=; fi && \\
    jq \\
        --arg executor \"$$EXECUTOR_ABSOLUTE_PATH\" \\
        --arg ivm_dir \"$$IVM_DIR_ABSOLUTE_PATH\" \\
        'if ($executor|length)>0 then .executor = $$executor else del(.executor) end | if ($ivm_dir|length)>0 then .ivm_dir = $$ivm_dir else del(.ivm_dir) end' /config/genesis.json \\
        >/tmp/genesis.json && \\
    kagami genesis sign /tmp/genesis.json \\
        --public-key $$GENESIS_PUBLIC_KEY \\
        --private-key $$GENESIS_PRIVATE_KEY \\
        ${GENESIS_CONSENSUS_MODE:+--consensus-mode $$GENESIS_CONSENSUS_MODE} \\
        ${GENESIS_NEXT_CONSENSUS_MODE:+--next-consensus-mode $$GENESIS_NEXT_CONSENSUS_MODE} \\
        ${GENESIS_MODE_ACTIVATION_HEIGHT:+--mode-activation-height $$GENESIS_MODE_ACTIVATION_HEIGHT} \\
        --topology \"$$TOPOLOGY\" \\
        ${GENESIS_PEER_POPS:+$$GENESIS_PEER_POPS} \\
        --out-file $$GENESIS \\
    && \\
    exec irohad
""#;

/// Configuration of the `irohad` service that submits genesis.
#[derive(Debug)]
struct Irohad0<'a, Image>
where
    Image: ComposeImageFields,
{
    base: Irohad<'a, Image, GenesisEnv<'a>>,
}

impl<'a, Image> Irohad0<'a, Image>
where
    Image: ComposeImageFields,
{
    #[allow(clippy::too_many_arguments)]
    fn new(
        image: Image,
        environment: GenesisEnv<'a>,
        ports: [u16; 2],
        volumes: Volumes<'a>,
        healthcheck: bool,
    ) -> Self {
        Self {
            base: Irohad::new(image, environment, ports, volumes, healthcheck),
        }
    }

    fn into_map(self) -> norito::json::Map {
        let mut map = self.base.into_map();
        map.insert(
            "command".into(),
            norito::json::Value::String(SIGN_AND_SUBMIT_GENESIS.into()),
        );
        map
    }
}

/// Reference to an `irohad` service.
#[derive(Debug, PartialOrd, PartialEq, Ord, Eq)]
struct IrohadRef(u16);

impl IrohadRef {
    fn service_name(&self) -> String {
        format!("{}{}", crate::peer::SERVICE_NAME, self.0)
    }
}

#[derive(Debug)]
enum BuildOrPull<'a> {
    Build {
        irohad0: Irohad0<'a, BuildImage<'a>>,
        irohads: std::collections::BTreeMap<IrohadRef, Irohad<'a, BuiltImage<'a>>>,
    },
    Pull {
        irohad0: Irohad0<'a, PulledImage<'a>>,
        irohads: std::collections::BTreeMap<IrohadRef, Irohad<'a, PulledImage<'a>>>,
    },
}

impl<'a> BuildOrPull<'a> {
    #[allow(clippy::too_many_arguments)]
    fn pull(
        image: PulledImage<'a>,
        volumes: Volumes<'a>,
        healthcheck: bool,
        chain: &'a iroha_data_model::ChainId,
        (genesis_public_key, genesis_private_key): &'a peer::ExposedKeyPair,
        network: &'a std::collections::BTreeMap<u16, peer::PeerInfo>,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::Peer>,
        consensus_mode: Option<&'a str>,
        next_consensus_mode: Option<&'a str>,
        mode_activation_height: Option<u64>,
        peer_pops: Vec<String>,
    ) -> Self {
        Self::Pull {
            irohad0: Self::irohad0(
                image,
                volumes,
                healthcheck,
                chain,
                (genesis_public_key, genesis_private_key),
                network,
                topology,
                consensus_mode,
                next_consensus_mode,
                mode_activation_height,
                peer_pops,
            ),
            irohads: Self::irohads(
                image,
                volumes,
                healthcheck,
                chain,
                genesis_public_key,
                network,
                topology,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        image: BuildImage<'a>,
        volumes: Volumes<'a>,
        healthcheck: bool,
        chain: &'a iroha_data_model::ChainId,
        (genesis_public_key, genesis_private_key): &'a peer::ExposedKeyPair,
        network: &'a std::collections::BTreeMap<u16, peer::PeerInfo>,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::Peer>,
        consensus_mode: Option<&'a str>,
        next_consensus_mode: Option<&'a str>,
        mode_activation_height: Option<u64>,
        peer_pops: Vec<String>,
    ) -> Self {
        Self::Build {
            irohad0: Self::irohad0(
                image,
                volumes,
                healthcheck,
                chain,
                (genesis_public_key, genesis_private_key),
                network,
                topology,
                consensus_mode,
                next_consensus_mode,
                mode_activation_height,
                peer_pops,
            ),
            irohads: Self::irohads(
                BuiltImage::new(image.image),
                volumes,
                healthcheck,
                chain,
                genesis_public_key,
                network,
                topology,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn irohad0<Image: ComposeImageFields + Copy>(
        image: Image,
        volumes: Volumes<'a>,
        healthcheck: bool,
        chain: &'a iroha_data_model::ChainId,
        (genesis_public_key, genesis_private_key): peer::ExposedKeyRefPair<'a>,
        network: &'a std::collections::BTreeMap<u16, peer::PeerInfo>,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::Peer>,
        consensus_mode: Option<&'a str>,
        next_consensus_mode: Option<&'a str>,
        mode_activation_height: Option<u64>,
        peer_pops: Vec<String>,
    ) -> Irohad0<'a, Image> {
        let (_, ports, key_pair, _) = network.get(&0).expect("irohad0 must be present");
        Irohad0::new(
            image,
            GenesisEnv::new(
                key_pair,
                *ports,
                chain,
                (genesis_public_key, genesis_private_key),
                topology,
                consensus_mode,
                next_consensus_mode,
                mode_activation_height,
                peer_pops,
            ),
            *ports,
            volumes,
            healthcheck,
        )
    }

    fn irohads<Image: ComposeImageFields + Copy>(
        image: Image,
        volumes: Volumes<'a>,
        healthcheck: bool,
        chain: &'a iroha_data_model::ChainId,
        genesis_public_key: &'a iroha_crypto::PublicKey,
        network: &'a std::collections::BTreeMap<u16, peer::PeerInfo>,
        topology: &'a std::collections::BTreeSet<iroha_data_model::peer::Peer>,
    ) -> std::collections::BTreeMap<IrohadRef, Irohad<'a, Image>> {
        network
            .iter()
            .skip(1)
            .map(|(id, (_, ports, key_pair, _))| {
                (
                    IrohadRef(*id),
                    Irohad::new(
                        image,
                        PeerEnv::new(key_pair, *ports, chain, genesis_public_key, topology),
                        *ports,
                        volumes,
                        healthcheck,
                    ),
                )
            })
            .collect()
    }

    fn into_services_map(self) -> norito::json::Map {
        let mut services = norito::json::Map::new();
        match self {
            BuildOrPull::Build { irohad0, irohads } => {
                services.insert(
                    "irohad0".into(),
                    norito::json::Value::Object(irohad0.into_map()),
                );
                for (service_ref, service) in irohads {
                    services.insert(
                        service_ref.service_name(),
                        norito::json::Value::Object(service.into_map()),
                    );
                }
            }
            BuildOrPull::Pull { irohad0, irohads } => {
                services.insert(
                    "irohad0".into(),
                    norito::json::Value::Object(irohad0.into_map()),
                );
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
            genesis_key_pair,
            network,
            topology,
            consensus_mode,
            next_consensus_mode,
            mode_activation_height,
        }: &'a PeerSettings,
    ) -> Self {
        let image = ImageId(name);
        let peer_pops = peer_pop_entries(network);
        let peer_pops_for_build = peer_pops.clone();
        let volumes = [
            PathMapping(
                HostFile(config_dir, GENESIS_FILE),
                CONTAINER_GENESIS_CONFIG,
                ReadOnly,
            ),
            PathMapping(
                HostFile(config_dir, CONFIG_FILE),
                CONTAINER_CLIENT_CONFIG,
                ReadOnly,
            ),
        ];
        Self {
            services: build_dir.as_ref().map_or_else(
                || {
                    BuildOrPull::pull(
                        PulledImage::new(image, *ignore_cache),
                        volumes,
                        *healthcheck,
                        chain,
                        genesis_key_pair,
                        network,
                        topology,
                        consensus_mode.as_deref(),
                        next_consensus_mode.as_deref(),
                        *mode_activation_height,
                        peer_pops,
                    )
                },
                |build| {
                    BuildOrPull::build(
                        BuildImage::new(image, HostPath(build), *ignore_cache),
                        volumes,
                        *healthcheck,
                        chain,
                        genesis_key_pair,
                        network,
                        topology,
                        consensus_mode.as_deref(),
                        next_consensus_mode.as_deref(),
                        *mode_activation_height,
                        peer_pops_for_build,
                    )
                },
            ),
        }
    }

    fn into_value(self) -> norito::json::Value {
        let mut root = norito::json::Map::new();
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
    use std::collections::HashMap;

    use super::*;
    use crate::{BASE_PORT_API, BASE_PORT_P2P};

    impl<'a> From<PeerEnv<'a>> for iroha_config::base::env::MockEnv {
        fn from(env: PeerEnv<'a>) -> Self {
            mock_env_from_value(peer_env_to_value(&env))
        }
    }

    impl<'a> From<GenesisEnv<'a>> for iroha_config::base::env::MockEnv {
        fn from(env: GenesisEnv<'a>) -> Self {
            mock_env_from_value(genesis_env_to_value(&env))
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
        let (key_pair, _pop) = peer::generate_bls_key_pair(None, &[]);
        let genesis_key_pair = peer::generate_key_pair(None, &[]);
        let ports = [BASE_PORT_P2P, BASE_PORT_API];
        let chain = peer::chain();
        let topology = [peer::peer("dummy", BASE_PORT_API, key_pair.0.clone())].into();
        let env = PeerEnv::new(&key_pair, ports, &chain, &genesis_key_pair.0, &topology);
        let mock_env = iroha_config::base::env::MockEnv::from(env);
        let _ = iroha_config::base::read::ConfigReader::new()
            .with_env(mock_env.clone())
            .read_and_complete::<iroha_config::parameters::user::Root>()
            .expect("config in env should be exhaustive");
        assert!(mock_env.unvisited().is_empty());
    }

    #[test]
    fn genesis_env_produces_exhaustive_config_sans_genesis_private_key_and_topology() {
        let (key_pair, _pop) = peer::generate_bls_key_pair(None, &[]);
        let (genesis_public_key, genesis_private_key) = &peer::generate_key_pair(None, &[]);
        let ports = [BASE_PORT_P2P, BASE_PORT_API];
        let chain = peer::chain();
        let topology = [peer::peer("dummy", BASE_PORT_API, key_pair.0.clone())].into();
        let env = GenesisEnv::new(
            &key_pair,
            ports,
            &chain,
            (genesis_public_key, genesis_private_key),
            &topology,
            None,
            None,
            None,
            Vec::new(),
        );
        let mock_env = iroha_config::base::env::MockEnv::from(env);
        let _ = iroha_config::base::read::ConfigReader::new()
            .with_env(mock_env.clone())
            .read_and_complete::<iroha_config::parameters::user::Root>()
            .expect("config in env should be exhaustive");
        assert_eq!(
            mock_env.unvisited(),
            ["GENESIS_PRIVATE_KEY", "TOPOLOGY"]
                .into_iter()
                .map(ToOwned::to_owned)
                .collect()
        );
    }

    #[test]
    fn genesis_env_with_consensus_overrides_is_exhaustive_plus_overrides() {
        let (key_pair, _pop) = peer::generate_bls_key_pair(None, &[]);
        let (genesis_public_key, genesis_private_key) = &peer::generate_key_pair(None, &[]);
        let ports = [BASE_PORT_P2P, BASE_PORT_API];
        let chain = peer::chain();
        let topology = [peer::peer("dummy", BASE_PORT_API, key_pair.0.clone())].into();
        let env = GenesisEnv::new(
            &key_pair,
            ports,
            &chain,
            (genesis_public_key, genesis_private_key),
            &topology,
            Some("npos"),
            Some("npos"),
            Some(7),
            Vec::new(),
        );
        let mock_env = iroha_config::base::env::MockEnv::from(env);
        let _ = iroha_config::base::read::ConfigReader::new()
            .with_env(mock_env.clone())
            .read_and_complete::<iroha_config::parameters::user::Root>()
            .expect("config in env should be exhaustive");
        let mut unvisited: Vec<_> = mock_env.unvisited().into_iter().collect();
        unvisited.sort();
        assert_eq!(
            unvisited,
            [
                "GENESIS_CONSENSUS_MODE",
                "GENESIS_MODE_ACTIVATION_HEIGHT",
                "GENESIS_NEXT_CONSENSUS_MODE",
                "GENESIS_PRIVATE_KEY",
                "TOPOLOGY"
            ]
            .into_iter()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
        );
    }
}
