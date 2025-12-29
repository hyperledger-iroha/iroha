//! Docker Compose configuration generator for Iroha.

mod path;
mod peer;
mod schema;

pub use crate::peer::PeerOverride;

const GENESIS_SEED: &[u8; 7] = b"genesis";
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
    genesis_key_pair: peer::ExposedKeyPair,
    network: std::collections::BTreeMap<u16, peer::PeerInfo>,
    topology: std::collections::BTreeSet<iroha_data_model::peer::Peer>,
    consensus_mode: Option<String>,
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
                    let key_pair = peer::generate_key_pair(seed, &extra_seed);
                    (
                        nth,
                        (
                            override_.name,
                            [override_.p2p_port, override_.api_port],
                            key_pair,
                        ),
                    )
                })
                .collect()
        } else {
            peer::network(count.get(), seed)
        };
        let topology = peer::topology(network.values());
        Ok(Self {
            healthcheck,
            config_dir: path::AbsolutePath::new(config_dir)?.relative_to(target_dir)?,
            chain: peer::chain(),
            genesis_key_pair: peer::generate_key_pair(seed, GENESIS_SEED),
            network,
            topology,
            consensus_mode,
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
                mode_activation_height: None,
            },
        )
    }

    #[test]
    fn single_build_banner() {
        expect_test::expect!([r##"
            # Single-line banner

            services:
              irohad0:
                build: ..
                command: |-
                  /bin/sh -c "
                      EXECUTOR_RELATIVE_PATH=$(jq -r '.executor // empty' /config/genesis.json) && \\
                      if [ -n \"$$EXECUTOR_RELATIVE_PATH\" ]; then EXECUTOR_ABSOLUTE_PATH=$(realpath \"/config/$$EXECUTOR_RELATIVE_PATH\"); else EXECUTOR_ABSOLUTE_PATH=; fi && \\
                      IVM_DIR_RELATIVE_PATH=$(jq -r '.ivm_dir // empty' /config/genesis.json) && \\
                      if [ -n \"$$IVM_DIR_RELATIVE_PATH\" ]; then IVM_DIR_ABSOLUTE_PATH=$(realpath \"/config/$$IVM_DIR_RELATIVE_PATH\"); else IVM_DIR_ABSOLUTE_PATH=; fi && \\
                      jq \\
                          --arg executor \"$$EXECUTOR_ABSOLUTE_PATH\" \\
                          --arg ivm_dir \"$$IVM_DIR_ABSOLUTE_PATH\" \\
                          --argjson topology \"$$TOPOLOGY\" \\
                          'if ($executor|length)>0 then .executor = $$executor else del(.executor) end | if ($ivm_dir|length)>0 then .ivm_dir = $$ivm_dir else del(.ivm_dir) end | .topology = $$topology' /config/genesis.json \\
                          >/tmp/genesis.json && \\
                      kagami genesis sign /tmp/genesis.json \\
                          --public-key $$GENESIS_PUBLIC_KEY \\
                          --private-key $$GENESIS_PRIVATE_KEY \\
                          ${GENESIS_CONSENSUS_MODE:+--consensus-mode $$GENESIS_CONSENSUS_MODE} \\
                          ${GENESIS_MODE_ACTIVATION_HEIGHT:+--mode-activation-height $$GENESIS_MODE_ACTIVATION_HEIGHT} \\
                          --out-file $$GENESIS \\
                      && \\
                      exec irohad
                  "
                environment:
                  API_ADDRESS: 0.0.0.0:8080
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS: /tmp/genesis.signed.nrt
                  GENESIS_PRIVATE_KEY: 802620FB8B867188E4952F1E83534B9B2E0A12D5122BD6F417CBC79D50D8A8C9C917B0
                  GENESIS_PUBLIC_KEY: ed0120F9F92758E815121F637C9704DFDA54842BA937AA721C0603018E208D6E25787E
                  P2P_ADDRESS: 0.0.0.0:1337
                  P2P_PUBLIC_ADDRESS: irohad0:1337
                  PRIVATE_KEY: 802620F173D8C4913E2244715B9BF810AC0A4DBE1C9E08F595C8D9510E3E335EF964BB
                  PUBLIC_KEY: ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA
                  TOPOLOGY: '["ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA"]'
                image: hyperledger/iroha:dev
                init: true
                ports:
                  - 1337:1337
                  - 8080:8080
                pull_policy: never
                volumes:
                  - ./genesis.json:/config/genesis.json:ro
                  - ./client.toml:/config/client.toml:ro
        "##]).assert_eq(&build_as_string(
            nonzero_ext::nonzero!(1u16),
            false,
            Some("."),
            false,
            Some(&["Single-line banner"]),
        ));
    }

    #[test]
    fn single_build_banner_nocache() {
        expect_test::expect!([r##"
            # Multi-line banner 1
            # Multi-line banner 2

            services:
              irohad0:
                build: ..
                command: |-
                  /bin/sh -c "
                      EXECUTOR_RELATIVE_PATH=$(jq -r '.executor // empty' /config/genesis.json) && \\
                      if [ -n \"$$EXECUTOR_RELATIVE_PATH\" ]; then EXECUTOR_ABSOLUTE_PATH=$(realpath \"/config/$$EXECUTOR_RELATIVE_PATH\"); else EXECUTOR_ABSOLUTE_PATH=; fi && \\
                      IVM_DIR_RELATIVE_PATH=$(jq -r '.ivm_dir // empty' /config/genesis.json) && \\
                      if [ -n \"$$IVM_DIR_RELATIVE_PATH\" ]; then IVM_DIR_ABSOLUTE_PATH=$(realpath \"/config/$$IVM_DIR_RELATIVE_PATH\"); else IVM_DIR_ABSOLUTE_PATH=; fi && \\
                      jq \\
                          --arg executor \"$$EXECUTOR_ABSOLUTE_PATH\" \\
                          --arg ivm_dir \"$$IVM_DIR_ABSOLUTE_PATH\" \\
                          --argjson topology \"$$TOPOLOGY\" \\
                          'if ($executor|length)>0 then .executor = $$executor else del(.executor) end | if ($ivm_dir|length)>0 then .ivm_dir = $$ivm_dir else del(.ivm_dir) end | .topology = $$topology' /config/genesis.json \\
                          >/tmp/genesis.json && \\
                      kagami genesis sign /tmp/genesis.json \\
                          --public-key $$GENESIS_PUBLIC_KEY \\
                          --private-key $$GENESIS_PRIVATE_KEY \\
                          ${GENESIS_CONSENSUS_MODE:+--consensus-mode $$GENESIS_CONSENSUS_MODE} \\
                          ${GENESIS_MODE_ACTIVATION_HEIGHT:+--mode-activation-height $$GENESIS_MODE_ACTIVATION_HEIGHT} \\
                          --out-file $$GENESIS \\
                      && \\
                      exec irohad
                  "
                environment:
                  API_ADDRESS: 0.0.0.0:8080
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS: /tmp/genesis.signed.nrt
                  GENESIS_PRIVATE_KEY: 802620FB8B867188E4952F1E83534B9B2E0A12D5122BD6F417CBC79D50D8A8C9C917B0
                  GENESIS_PUBLIC_KEY: ed0120F9F92758E815121F637C9704DFDA54842BA937AA721C0603018E208D6E25787E
                  P2P_ADDRESS: 0.0.0.0:1337
                  P2P_PUBLIC_ADDRESS: irohad0:1337
                  PRIVATE_KEY: 802620F173D8C4913E2244715B9BF810AC0A4DBE1C9E08F595C8D9510E3E335EF964BB
                  PUBLIC_KEY: ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA
                  TOPOLOGY: '["ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA"]'
                image: hyperledger/iroha:dev
                init: true
                ports:
                  - 1337:1337
                  - 8080:8080
                pull_policy: build
                volumes:
                  - ./genesis.json:/config/genesis.json:ro
                  - ./client.toml:/config/client.toml:ro
        "##]).assert_eq(&build_as_string(
            nonzero_ext::nonzero!(1u16),
            false,
            Some("."),
            true,
            Some(&["Multi-line banner 1", "Multi-line banner 2"]),
        ));
    }

    #[test]
    fn multiple_build_banner_nocache() {
        expect_test::expect!([r##"
            # Single-line banner

            services:
              irohad0:
                build: ..
                command: |-
                  /bin/sh -c "
                      EXECUTOR_RELATIVE_PATH=$(jq -r '.executor // empty' /config/genesis.json) && \\
                      if [ -n \"$$EXECUTOR_RELATIVE_PATH\" ]; then EXECUTOR_ABSOLUTE_PATH=$(realpath \"/config/$$EXECUTOR_RELATIVE_PATH\"); else EXECUTOR_ABSOLUTE_PATH=; fi && \\
                      IVM_DIR_RELATIVE_PATH=$(jq -r '.ivm_dir // empty' /config/genesis.json) && \\
                      if [ -n \"$$IVM_DIR_RELATIVE_PATH\" ]; then IVM_DIR_ABSOLUTE_PATH=$(realpath \"/config/$$IVM_DIR_RELATIVE_PATH\"); else IVM_DIR_ABSOLUTE_PATH=; fi && \\
                      jq \\
                          --arg executor \"$$EXECUTOR_ABSOLUTE_PATH\" \\
                          --arg ivm_dir \"$$IVM_DIR_ABSOLUTE_PATH\" \\
                          --argjson topology \"$$TOPOLOGY\" \\
                          'if ($executor|length)>0 then .executor = $$executor else del(.executor) end | if ($ivm_dir|length)>0 then .ivm_dir = $$ivm_dir else del(.ivm_dir) end | .topology = $$topology' /config/genesis.json \\
                          >/tmp/genesis.json && \\
                      kagami genesis sign /tmp/genesis.json \\
                          --public-key $$GENESIS_PUBLIC_KEY \\
                          --private-key $$GENESIS_PRIVATE_KEY \\
                          ${GENESIS_CONSENSUS_MODE:+--consensus-mode $$GENESIS_CONSENSUS_MODE} \\
                          ${GENESIS_MODE_ACTIVATION_HEIGHT:+--mode-activation-height $$GENESIS_MODE_ACTIVATION_HEIGHT} \\
                          --out-file $$GENESIS \\
                      && \\
                      exec irohad
                  "
                environment:
                  API_ADDRESS: 0.0.0.0:8080
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS: /tmp/genesis.signed.nrt
                  GENESIS_PRIVATE_KEY: 802620FB8B867188E4952F1E83534B9B2E0A12D5122BD6F417CBC79D50D8A8C9C917B0
                  GENESIS_PUBLIC_KEY: ed0120F9F92758E815121F637C9704DFDA54842BA937AA721C0603018E208D6E25787E
                  P2P_ADDRESS: 0.0.0.0:1337
                  P2P_PUBLIC_ADDRESS: irohad0:1337
                  PRIVATE_KEY: 802620F173D8C4913E2244715B9BF810AC0A4DBE1C9E08F595C8D9510E3E335EF964BB
                  PUBLIC_KEY: ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA
                  TOPOLOGY: '["ed012063ED3DFEDEBD8A86B4941CC4379D2EF0B74BDFE61F033FC0C89867D57C882A26","ed012064BD9B25BF8477144D03B26FC8CF5D8A354B2F780DA310EE69933DC1E86FBCE2","ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA","ed01208EA177921AF051CD12FC07E3416419320908883A1104B31401B650EEB820A300"]'
                  TRUSTED_PEERS: '["ed012063ED3DFEDEBD8A86B4941CC4379D2EF0B74BDFE61F033FC0C89867D57C882A26","ed012064BD9B25BF8477144D03B26FC8CF5D8A354B2F780DA310EE69933DC1E86FBCE2","ed01208EA177921AF051CD12FC07E3416419320908883A1104B31401B650EEB820A300"]'
                image: hyperledger/iroha:dev
                init: true
                ports:
                  - 1337:1337
                  - 8080:8080
                pull_policy: build
                volumes:
                  - ./genesis.json:/config/genesis.json:ro
                  - ./client.toml:/config/client.toml:ro
              irohad1:
                depends_on:
                  - irohad0
                environment:
                  API_ADDRESS: 0.0.0.0:8081
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS_PUBLIC_KEY: ed0120F9F92758E815121F637C9704DFDA54842BA937AA721C0603018E208D6E25787E
                  P2P_ADDRESS: 0.0.0.0:1338
                  P2P_PUBLIC_ADDRESS: irohad1:1338
                  PRIVATE_KEY: 802620FD8E2F03755AA130464ABF57A75E207BE870636B57F614D7A7B94E42318F9CA9
                  PUBLIC_KEY: ed012064BD9B25BF8477144D03B26FC8CF5D8A354B2F780DA310EE69933DC1E86FBCE2
                  TRUSTED_PEERS: '["ed012063ED3DFEDEBD8A86B4941CC4379D2EF0B74BDFE61F033FC0C89867D57C882A26","ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA","ed01208EA177921AF051CD12FC07E3416419320908883A1104B31401B650EEB820A300"]'
                image: hyperledger/iroha:dev
                init: true
                ports:
                  - 1338:1338
                  - 8081:8081
                pull_policy: never
                volumes:
                  - ./genesis.json:/config/genesis.json:ro
                  - ./client.toml:/config/client.toml:ro
              irohad2:
                depends_on:
                  - irohad0
                environment:
                  API_ADDRESS: 0.0.0.0:8082
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS_PUBLIC_KEY: ed0120F9F92758E815121F637C9704DFDA54842BA937AA721C0603018E208D6E25787E
                  P2P_ADDRESS: 0.0.0.0:1339
                  P2P_PUBLIC_ADDRESS: irohad2:1339
                  PRIVATE_KEY: 8026203A18FAC2654F1C8A331A84F4B142396EEC900022B38842D88D55E0DE144C8DF2
                  PUBLIC_KEY: ed01208EA177921AF051CD12FC07E3416419320908883A1104B31401B650EEB820A300
                  TRUSTED_PEERS: '["ed012063ED3DFEDEBD8A86B4941CC4379D2EF0B74BDFE61F033FC0C89867D57C882A26","ed012064BD9B25BF8477144D03B26FC8CF5D8A354B2F780DA310EE69933DC1E86FBCE2","ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA"]'
                image: hyperledger/iroha:dev
                init: true
                ports:
                  - 1339:1339
                  - 8082:8082
                pull_policy: never
                volumes:
                  - ./genesis.json:/config/genesis.json:ro
                  - ./client.toml:/config/client.toml:ro
              irohad3:
                depends_on:
                  - irohad0
                environment:
                  API_ADDRESS: 0.0.0.0:8083
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS_PUBLIC_KEY: ed0120F9F92758E815121F637C9704DFDA54842BA937AA721C0603018E208D6E25787E
                  P2P_ADDRESS: 0.0.0.0:1340
                  P2P_PUBLIC_ADDRESS: irohad3:1340
                  PRIVATE_KEY: 8026209464445DBA9030D6AC4F83161D3219144F886068027F6708AF9686F85DF6C4F0
                  PUBLIC_KEY: ed012063ED3DFEDEBD8A86B4941CC4379D2EF0B74BDFE61F033FC0C89867D57C882A26
                  TRUSTED_PEERS: '["ed012064BD9B25BF8477144D03B26FC8CF5D8A354B2F780DA310EE69933DC1E86FBCE2","ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA","ed01208EA177921AF051CD12FC07E3416419320908883A1104B31401B650EEB820A300"]'
                image: hyperledger/iroha:dev
                init: true
                ports:
                  - 1340:1340
                  - 8083:8083
                pull_policy: never
                volumes:
                  - ./genesis.json:/config/genesis.json:ro
                  - ./client.toml:/config/client.toml:ro
        "##]).assert_eq(&build_as_string(
            nonzero_ext::nonzero!(4u16),
            false,
            Some("."),
            true,
            Some(&["Single-line banner"]),
        ));
    }

    #[test]
    fn single_pull_healthcheck() {
        expect_test::expect!([r#"
            services:
              irohad0:
                command: |-
                  /bin/sh -c "
                      EXECUTOR_RELATIVE_PATH=$(jq -r '.executor // empty' /config/genesis.json) && \\
                      if [ -n \"$$EXECUTOR_RELATIVE_PATH\" ]; then EXECUTOR_ABSOLUTE_PATH=$(realpath \"/config/$$EXECUTOR_RELATIVE_PATH\"); else EXECUTOR_ABSOLUTE_PATH=; fi && \\
                      IVM_DIR_RELATIVE_PATH=$(jq -r '.ivm_dir // empty' /config/genesis.json) && \\
                      if [ -n \"$$IVM_DIR_RELATIVE_PATH\" ]; then IVM_DIR_ABSOLUTE_PATH=$(realpath \"/config/$$IVM_DIR_RELATIVE_PATH\"); else IVM_DIR_ABSOLUTE_PATH=; fi && \\
                      jq \\
                          --arg executor \"$$EXECUTOR_ABSOLUTE_PATH\" \\
                          --arg ivm_dir \"$$IVM_DIR_ABSOLUTE_PATH\" \\
                          --argjson topology \"$$TOPOLOGY\" \\
                          'if ($executor|length)>0 then .executor = $$executor else del(.executor) end | if ($ivm_dir|length)>0 then .ivm_dir = $$ivm_dir else del(.ivm_dir) end | .topology = $$topology' /config/genesis.json \\
                          >/tmp/genesis.json && \\
                      kagami genesis sign /tmp/genesis.json \\
                          --public-key $$GENESIS_PUBLIC_KEY \\
                          --private-key $$GENESIS_PRIVATE_KEY \\
                          ${GENESIS_CONSENSUS_MODE:+--consensus-mode $$GENESIS_CONSENSUS_MODE} \\
                          ${GENESIS_MODE_ACTIVATION_HEIGHT:+--mode-activation-height $$GENESIS_MODE_ACTIVATION_HEIGHT} \\
                          --out-file $$GENESIS \\
                      && \\
                      exec irohad
                  "
                environment:
                  API_ADDRESS: 0.0.0.0:8080
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS: /tmp/genesis.signed.nrt
                  GENESIS_PRIVATE_KEY: 802620FB8B867188E4952F1E83534B9B2E0A12D5122BD6F417CBC79D50D8A8C9C917B0
                  GENESIS_PUBLIC_KEY: ed0120F9F92758E815121F637C9704DFDA54842BA937AA721C0603018E208D6E25787E
                  P2P_ADDRESS: 0.0.0.0:1337
                  P2P_PUBLIC_ADDRESS: irohad0:1337
                  PRIVATE_KEY: 802620F173D8C4913E2244715B9BF810AC0A4DBE1C9E08F595C8D9510E3E335EF964BB
                  PUBLIC_KEY: ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA
                  TOPOLOGY: '["ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA"]'
                healthcheck:
                  interval: 2s
                  retries: 30
                  start_period: 4s
                  test: 'test $(curl -s http://127.0.0.1:8080/status/blocks) -gt 0'
                  timeout: 1s
                image: hyperledger/iroha:dev
                init: true
                ports:
                  - 1337:1337
                  - 8080:8080
                pull_policy: missing
                volumes:
                  - ./genesis.json:/config/genesis.json:ro
                  - ./client.toml:/config/client.toml:ro
        "#]).assert_eq(&build_as_string(
            nonzero_ext::nonzero!(1u16),
            true,
            None,
            false,
            None,
        ));
    }

    #[test]
    fn multiple_pull_healthcheck_nocache() {
        expect_test::expect!([r#"
            services:
              irohad0:
                command: |-
                  /bin/sh -c "
                      EXECUTOR_RELATIVE_PATH=$(jq -r '.executor // empty' /config/genesis.json) && \\
                      if [ -n \"$$EXECUTOR_RELATIVE_PATH\" ]; then EXECUTOR_ABSOLUTE_PATH=$(realpath \"/config/$$EXECUTOR_RELATIVE_PATH\"); else EXECUTOR_ABSOLUTE_PATH=; fi && \\
                      IVM_DIR_RELATIVE_PATH=$(jq -r '.ivm_dir // empty' /config/genesis.json) && \\
                      if [ -n \"$$IVM_DIR_RELATIVE_PATH\" ]; then IVM_DIR_ABSOLUTE_PATH=$(realpath \"/config/$$IVM_DIR_RELATIVE_PATH\"); else IVM_DIR_ABSOLUTE_PATH=; fi && \\
                      jq \\
                          --arg executor \"$$EXECUTOR_ABSOLUTE_PATH\" \\
                          --arg ivm_dir \"$$IVM_DIR_ABSOLUTE_PATH\" \\
                          --argjson topology \"$$TOPOLOGY\" \\
                          'if ($executor|length)>0 then .executor = $$executor else del(.executor) end | if ($ivm_dir|length)>0 then .ivm_dir = $$ivm_dir else del(.ivm_dir) end | .topology = $$topology' /config/genesis.json \\
                          >/tmp/genesis.json && \\
                      kagami genesis sign /tmp/genesis.json \\
                          --public-key $$GENESIS_PUBLIC_KEY \\
                          --private-key $$GENESIS_PRIVATE_KEY \\
                          ${GENESIS_CONSENSUS_MODE:+--consensus-mode $$GENESIS_CONSENSUS_MODE} \\
                          ${GENESIS_MODE_ACTIVATION_HEIGHT:+--mode-activation-height $$GENESIS_MODE_ACTIVATION_HEIGHT} \\
                          --out-file $$GENESIS \\
                      && \\
                      exec irohad
                  "
                environment:
                  API_ADDRESS: 0.0.0.0:8080
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS: /tmp/genesis.signed.nrt
                  GENESIS_PRIVATE_KEY: 802620FB8B867188E4952F1E83534B9B2E0A12D5122BD6F417CBC79D50D8A8C9C917B0
                  GENESIS_PUBLIC_KEY: ed0120F9F92758E815121F637C9704DFDA54842BA937AA721C0603018E208D6E25787E
                  P2P_ADDRESS: 0.0.0.0:1337
                  P2P_PUBLIC_ADDRESS: irohad0:1337
                  PRIVATE_KEY: 802620F173D8C4913E2244715B9BF810AC0A4DBE1C9E08F595C8D9510E3E335EF964BB
                  PUBLIC_KEY: ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA
                  TOPOLOGY: '["ed012063ED3DFEDEBD8A86B4941CC4379D2EF0B74BDFE61F033FC0C89867D57C882A26","ed012064BD9B25BF8477144D03B26FC8CF5D8A354B2F780DA310EE69933DC1E86FBCE2","ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA","ed01208EA177921AF051CD12FC07E3416419320908883A1104B31401B650EEB820A300"]'
                  TRUSTED_PEERS: '["ed012063ED3DFEDEBD8A86B4941CC4379D2EF0B74BDFE61F033FC0C89867D57C882A26","ed012064BD9B25BF8477144D03B26FC8CF5D8A354B2F780DA310EE69933DC1E86FBCE2","ed01208EA177921AF051CD12FC07E3416419320908883A1104B31401B650EEB820A300"]'
                healthcheck:
                  interval: 2s
                  retries: 30
                  start_period: 4s
                  test: 'test $(curl -s http://127.0.0.1:8080/status/blocks) -gt 0'
                  timeout: 1s
                image: hyperledger/iroha:dev
                init: true
                ports:
                  - 1337:1337
                  - 8080:8080
                pull_policy: always
                volumes:
                  - ./genesis.json:/config/genesis.json:ro
                  - ./client.toml:/config/client.toml:ro
              irohad1:
                environment:
                  API_ADDRESS: 0.0.0.0:8081
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS_PUBLIC_KEY: ed0120F9F92758E815121F637C9704DFDA54842BA937AA721C0603018E208D6E25787E
                  P2P_ADDRESS: 0.0.0.0:1338
                  P2P_PUBLIC_ADDRESS: irohad1:1338
                  PRIVATE_KEY: 802620FD8E2F03755AA130464ABF57A75E207BE870636B57F614D7A7B94E42318F9CA9
                  PUBLIC_KEY: ed012064BD9B25BF8477144D03B26FC8CF5D8A354B2F780DA310EE69933DC1E86FBCE2
                  TRUSTED_PEERS: '["ed012063ED3DFEDEBD8A86B4941CC4379D2EF0B74BDFE61F033FC0C89867D57C882A26","ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA","ed01208EA177921AF051CD12FC07E3416419320908883A1104B31401B650EEB820A300"]'
                healthcheck:
                  interval: 2s
                  retries: 30
                  start_period: 4s
                  test: 'test $(curl -s http://127.0.0.1:8081/status/blocks) -gt 0'
                  timeout: 1s
                image: hyperledger/iroha:dev
                init: true
                ports:
                  - 1338:1338
                  - 8081:8081
                pull_policy: always
                volumes:
                  - ./genesis.json:/config/genesis.json:ro
                  - ./client.toml:/config/client.toml:ro
              irohad2:
                environment:
                  API_ADDRESS: 0.0.0.0:8082
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS_PUBLIC_KEY: ed0120F9F92758E815121F637C9704DFDA54842BA937AA721C0603018E208D6E25787E
                  P2P_ADDRESS: 0.0.0.0:1339
                  P2P_PUBLIC_ADDRESS: irohad2:1339
                  PRIVATE_KEY: 8026203A18FAC2654F1C8A331A84F4B142396EEC900022B38842D88D55E0DE144C8DF2
                  PUBLIC_KEY: ed01208EA177921AF051CD12FC07E3416419320908883A1104B31401B650EEB820A300
                  TRUSTED_PEERS: '["ed012063ED3DFEDEBD8A86B4941CC4379D2EF0B74BDFE61F033FC0C89867D57C882A26","ed012064BD9B25BF8477144D03B26FC8CF5D8A354B2F780DA310EE69933DC1E86FBCE2","ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA"]'
                healthcheck:
                  interval: 2s
                  retries: 30
                  start_period: 4s
                  test: 'test $(curl -s http://127.0.0.1:8082/status/blocks) -gt 0'
                  timeout: 1s
                image: hyperledger/iroha:dev
                init: true
                ports:
                  - 1339:1339
                  - 8082:8082
                pull_policy: always
                volumes:
                  - ./genesis.json:/config/genesis.json:ro
                  - ./client.toml:/config/client.toml:ro
              irohad3:
                environment:
                  API_ADDRESS: 0.0.0.0:8083
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS_PUBLIC_KEY: ed0120F9F92758E815121F637C9704DFDA54842BA937AA721C0603018E208D6E25787E
                  P2P_ADDRESS: 0.0.0.0:1340
                  P2P_PUBLIC_ADDRESS: irohad3:1340
                  PRIVATE_KEY: 8026209464445DBA9030D6AC4F83161D3219144F886068027F6708AF9686F85DF6C4F0
                  PUBLIC_KEY: ed012063ED3DFEDEBD8A86B4941CC4379D2EF0B74BDFE61F033FC0C89867D57C882A26
                  TRUSTED_PEERS: '["ed012064BD9B25BF8477144D03B26FC8CF5D8A354B2F780DA310EE69933DC1E86FBCE2","ed012087FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA","ed01208EA177921AF051CD12FC07E3416419320908883A1104B31401B650EEB820A300"]'
                healthcheck:
                  interval: 2s
                  retries: 30
                  start_period: 4s
                  test: 'test $(curl -s http://127.0.0.1:8083/status/blocks) -gt 0'
                  timeout: 1s
                image: hyperledger/iroha:dev
                init: true
                ports:
                  - 1340:1340
                  - 8083:8083
                pull_policy: always
                volumes:
                  - ./genesis.json:/config/genesis.json:ro
                  - ./client.toml:/config/client.toml:ro
        "#]).assert_eq(&build_as_string(
            nonzero_ext::nonzero!(4u16),
            true,
            None,
            true,
            None,
        ));
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
                mode_activation_height: Some(7),
            },
        );

        assert!(
            output.contains("GENESIS_CONSENSUS_MODE: npos"),
            "genesis signing environment should carry consensus mode"
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
