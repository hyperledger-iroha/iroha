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
                    let (key_pair, pop) = peer::generate_bls_key_pair(seed, &extra_seed);
                    (
                        nth,
                        (
                            override_.name,
                            [override_.p2p_port, override_.api_port],
                            key_pair,
                            pop,
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
                  "
                environment:
                  API_ADDRESS: 0.0.0.0:8080
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS: /tmp/genesis.signed.nrt
                  GENESIS_PEER_POPS: '--peer-pop ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39=0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5'
                  GENESIS_PRIVATE_KEY: 802620AEEBAD4A796FCC2E15DC4C6061B45ED9B373F26ADFC798CA7D2D8CC58182718E
                  GENESIS_PUBLIC_KEY: ed0120CB6D05AFB7B05D488ACD6B5558FA285ECCCA40DC59D188338AB364494A629B82
                  P2P_ADDRESS: 0.0.0.0:1337
                  P2P_PUBLIC_ADDRESS: irohad0:1337
                  PRIVATE_KEY: 8926207AAB38417AB8226BEA595EB47FE1138165550E3BB5667F8AE4FFECBF6B2AC13D
                  PUBLIC_KEY: ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39
                  TOPOLOGY: '["ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39"]'
                  TRUSTED_PEERS_POP: '[{"pop_hex":"0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5","public_key":"ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39"}]'
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
                  "
                environment:
                  API_ADDRESS: 0.0.0.0:8080
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS: /tmp/genesis.signed.nrt
                  GENESIS_PEER_POPS: '--peer-pop ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39=0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5'
                  GENESIS_PRIVATE_KEY: 802620AEEBAD4A796FCC2E15DC4C6061B45ED9B373F26ADFC798CA7D2D8CC58182718E
                  GENESIS_PUBLIC_KEY: ed0120CB6D05AFB7B05D488ACD6B5558FA285ECCCA40DC59D188338AB364494A629B82
                  P2P_ADDRESS: 0.0.0.0:1337
                  P2P_PUBLIC_ADDRESS: irohad0:1337
                  PRIVATE_KEY: 8926207AAB38417AB8226BEA595EB47FE1138165550E3BB5667F8AE4FFECBF6B2AC13D
                  PUBLIC_KEY: ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39
                  TOPOLOGY: '["ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39"]'
                  TRUSTED_PEERS_POP: '[{"pop_hex":"0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5","public_key":"ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39"}]'
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
                  "
                environment:
                  API_ADDRESS: 0.0.0.0:8080
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS: /tmp/genesis.signed.nrt
                  GENESIS_PEER_POPS: '--peer-pop ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39=0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5 --peer-pop ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0=0x932e9c24c0cee5b3c79174f89bffca9068ab96a5a183e29d2b0ca64ce688ca3caba4e63bd67fb2532b8ee1cdee19f479014a8df44540fda6853b161efcc8afc1561dbbc6422830d713087de8cb29f176f678fc8793c589f9bedbee53d84e1c73 --peer-pop ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A=0x92116175995869a1e32d7964fcea60289c50c9fd1bb7baa35453b4cea19e20e2cd75db3456f969217f2332662678632b074b46d6d23b91d2049023a8acd40d1de304fcd4bdd09b5e1e62bc0ef2e7f3c1e4a0d0f587c98f36b7be9d1744d9e165 --peer-pop ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9=0xb9e4f1ef4aae8b525ceccf97cdbdd598d9a70bd72821796c41311f5c0a475e3c7506f1717680038a9836e8a79516a85a0a7a1c4677e9a26d80dd567d42e81683eb35f7257a286469fdfa9e4ea8a0e399dc8e9749b488bf5180b0eb048c880457'
                  GENESIS_PRIVATE_KEY: 802620AEEBAD4A796FCC2E15DC4C6061B45ED9B373F26ADFC798CA7D2D8CC58182718E
                  GENESIS_PUBLIC_KEY: ed0120CB6D05AFB7B05D488ACD6B5558FA285ECCCA40DC59D188338AB364494A629B82
                  P2P_ADDRESS: 0.0.0.0:1337
                  P2P_PUBLIC_ADDRESS: irohad0:1337
                  PRIVATE_KEY: 8926207AAB38417AB8226BEA595EB47FE1138165550E3BB5667F8AE4FFECBF6B2AC13D
                  PUBLIC_KEY: ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39
                  TOPOLOGY: '["ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9","ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39","ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A","ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"]'
                  TRUSTED_PEERS: '["ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9","ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A","ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"]'
                  TRUSTED_PEERS_POP: '[{"pop_hex":"0xb9e4f1ef4aae8b525ceccf97cdbdd598d9a70bd72821796c41311f5c0a475e3c7506f1717680038a9836e8a79516a85a0a7a1c4677e9a26d80dd567d42e81683eb35f7257a286469fdfa9e4ea8a0e399dc8e9749b488bf5180b0eb048c880457","public_key":"ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9"},{"pop_hex":"0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5","public_key":"ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39"},{"pop_hex":"0x92116175995869a1e32d7964fcea60289c50c9fd1bb7baa35453b4cea19e20e2cd75db3456f969217f2332662678632b074b46d6d23b91d2049023a8acd40d1de304fcd4bdd09b5e1e62bc0ef2e7f3c1e4a0d0f587c98f36b7be9d1744d9e165","public_key":"ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A"},{"pop_hex":"0x932e9c24c0cee5b3c79174f89bffca9068ab96a5a183e29d2b0ca64ce688ca3caba4e63bd67fb2532b8ee1cdee19f479014a8df44540fda6853b161efcc8afc1561dbbc6422830d713087de8cb29f176f678fc8793c589f9bedbee53d84e1c73","public_key":"ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"}]'
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
                  GENESIS_PUBLIC_KEY: ed0120CB6D05AFB7B05D488ACD6B5558FA285ECCCA40DC59D188338AB364494A629B82
                  P2P_ADDRESS: 0.0.0.0:1338
                  P2P_PUBLIC_ADDRESS: irohad1:1338
                  PRIVATE_KEY: 892620C0E8730DDB91DF6EA2049A6C92B8CB76C5FAEEB61AD87D969B07585650E2D94C
                  PUBLIC_KEY: ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0
                  TRUSTED_PEERS: '["ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9","ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39","ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A"]'
                  TRUSTED_PEERS_POP: '[{"pop_hex":"0xb9e4f1ef4aae8b525ceccf97cdbdd598d9a70bd72821796c41311f5c0a475e3c7506f1717680038a9836e8a79516a85a0a7a1c4677e9a26d80dd567d42e81683eb35f7257a286469fdfa9e4ea8a0e399dc8e9749b488bf5180b0eb048c880457","public_key":"ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9"},{"pop_hex":"0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5","public_key":"ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39"},{"pop_hex":"0x92116175995869a1e32d7964fcea60289c50c9fd1bb7baa35453b4cea19e20e2cd75db3456f969217f2332662678632b074b46d6d23b91d2049023a8acd40d1de304fcd4bdd09b5e1e62bc0ef2e7f3c1e4a0d0f587c98f36b7be9d1744d9e165","public_key":"ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A"},{"pop_hex":"0x932e9c24c0cee5b3c79174f89bffca9068ab96a5a183e29d2b0ca64ce688ca3caba4e63bd67fb2532b8ee1cdee19f479014a8df44540fda6853b161efcc8afc1561dbbc6422830d713087de8cb29f176f678fc8793c589f9bedbee53d84e1c73","public_key":"ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"}]'
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
                  GENESIS_PUBLIC_KEY: ed0120CB6D05AFB7B05D488ACD6B5558FA285ECCCA40DC59D188338AB364494A629B82
                  P2P_ADDRESS: 0.0.0.0:1339
                  P2P_PUBLIC_ADDRESS: irohad2:1339
                  PRIVATE_KEY: 892620D48701E56C55E12E34FFE323E9073A99FB020D24EA5136415C1724C78270790F
                  PUBLIC_KEY: ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A
                  TRUSTED_PEERS: '["ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9","ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39","ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"]'
                  TRUSTED_PEERS_POP: '[{"pop_hex":"0xb9e4f1ef4aae8b525ceccf97cdbdd598d9a70bd72821796c41311f5c0a475e3c7506f1717680038a9836e8a79516a85a0a7a1c4677e9a26d80dd567d42e81683eb35f7257a286469fdfa9e4ea8a0e399dc8e9749b488bf5180b0eb048c880457","public_key":"ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9"},{"pop_hex":"0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5","public_key":"ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39"},{"pop_hex":"0x92116175995869a1e32d7964fcea60289c50c9fd1bb7baa35453b4cea19e20e2cd75db3456f969217f2332662678632b074b46d6d23b91d2049023a8acd40d1de304fcd4bdd09b5e1e62bc0ef2e7f3c1e4a0d0f587c98f36b7be9d1744d9e165","public_key":"ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A"},{"pop_hex":"0x932e9c24c0cee5b3c79174f89bffca9068ab96a5a183e29d2b0ca64ce688ca3caba4e63bd67fb2532b8ee1cdee19f479014a8df44540fda6853b161efcc8afc1561dbbc6422830d713087de8cb29f176f678fc8793c589f9bedbee53d84e1c73","public_key":"ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"}]'
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
                  GENESIS_PUBLIC_KEY: ed0120CB6D05AFB7B05D488ACD6B5558FA285ECCCA40DC59D188338AB364494A629B82
                  P2P_ADDRESS: 0.0.0.0:1340
                  P2P_PUBLIC_ADDRESS: irohad3:1340
                  PRIVATE_KEY: 892620A58A3381FD2BB62E71F093EED8EDF9B7963A0BD2E6FF7F8E04994E0D70E7CF50
                  PUBLIC_KEY: ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9
                  TRUSTED_PEERS: '["ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39","ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A","ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"]'
                  TRUSTED_PEERS_POP: '[{"pop_hex":"0xb9e4f1ef4aae8b525ceccf97cdbdd598d9a70bd72821796c41311f5c0a475e3c7506f1717680038a9836e8a79516a85a0a7a1c4677e9a26d80dd567d42e81683eb35f7257a286469fdfa9e4ea8a0e399dc8e9749b488bf5180b0eb048c880457","public_key":"ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9"},{"pop_hex":"0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5","public_key":"ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39"},{"pop_hex":"0x92116175995869a1e32d7964fcea60289c50c9fd1bb7baa35453b4cea19e20e2cd75db3456f969217f2332662678632b074b46d6d23b91d2049023a8acd40d1de304fcd4bdd09b5e1e62bc0ef2e7f3c1e4a0d0f587c98f36b7be9d1744d9e165","public_key":"ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A"},{"pop_hex":"0x932e9c24c0cee5b3c79174f89bffca9068ab96a5a183e29d2b0ca64ce688ca3caba4e63bd67fb2532b8ee1cdee19f479014a8df44540fda6853b161efcc8afc1561dbbc6422830d713087de8cb29f176f678fc8793c589f9bedbee53d84e1c73","public_key":"ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"}]'
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
                  "
                environment:
                  API_ADDRESS: 0.0.0.0:8080
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS: /tmp/genesis.signed.nrt
                  GENESIS_PEER_POPS: '--peer-pop ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39=0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5'
                  GENESIS_PRIVATE_KEY: 802620AEEBAD4A796FCC2E15DC4C6061B45ED9B373F26ADFC798CA7D2D8CC58182718E
                  GENESIS_PUBLIC_KEY: ed0120CB6D05AFB7B05D488ACD6B5558FA285ECCCA40DC59D188338AB364494A629B82
                  P2P_ADDRESS: 0.0.0.0:1337
                  P2P_PUBLIC_ADDRESS: irohad0:1337
                  PRIVATE_KEY: 8926207AAB38417AB8226BEA595EB47FE1138165550E3BB5667F8AE4FFECBF6B2AC13D
                  PUBLIC_KEY: ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39
                  TOPOLOGY: '["ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39"]'
                  TRUSTED_PEERS_POP: '[{"pop_hex":"0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5","public_key":"ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39"}]'
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
                  "
                environment:
                  API_ADDRESS: 0.0.0.0:8080
                  CHAIN: 00000000-0000-0000-0000-000000000000
                  GENESIS: /tmp/genesis.signed.nrt
                  GENESIS_PEER_POPS: '--peer-pop ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39=0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5 --peer-pop ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0=0x932e9c24c0cee5b3c79174f89bffca9068ab96a5a183e29d2b0ca64ce688ca3caba4e63bd67fb2532b8ee1cdee19f479014a8df44540fda6853b161efcc8afc1561dbbc6422830d713087de8cb29f176f678fc8793c589f9bedbee53d84e1c73 --peer-pop ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A=0x92116175995869a1e32d7964fcea60289c50c9fd1bb7baa35453b4cea19e20e2cd75db3456f969217f2332662678632b074b46d6d23b91d2049023a8acd40d1de304fcd4bdd09b5e1e62bc0ef2e7f3c1e4a0d0f587c98f36b7be9d1744d9e165 --peer-pop ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9=0xb9e4f1ef4aae8b525ceccf97cdbdd598d9a70bd72821796c41311f5c0a475e3c7506f1717680038a9836e8a79516a85a0a7a1c4677e9a26d80dd567d42e81683eb35f7257a286469fdfa9e4ea8a0e399dc8e9749b488bf5180b0eb048c880457'
                  GENESIS_PRIVATE_KEY: 802620AEEBAD4A796FCC2E15DC4C6061B45ED9B373F26ADFC798CA7D2D8CC58182718E
                  GENESIS_PUBLIC_KEY: ed0120CB6D05AFB7B05D488ACD6B5558FA285ECCCA40DC59D188338AB364494A629B82
                  P2P_ADDRESS: 0.0.0.0:1337
                  P2P_PUBLIC_ADDRESS: irohad0:1337
                  PRIVATE_KEY: 8926207AAB38417AB8226BEA595EB47FE1138165550E3BB5667F8AE4FFECBF6B2AC13D
                  PUBLIC_KEY: ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39
                  TOPOLOGY: '["ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9","ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39","ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A","ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"]'
                  TRUSTED_PEERS: '["ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9","ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A","ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"]'
                  TRUSTED_PEERS_POP: '[{"pop_hex":"0xb9e4f1ef4aae8b525ceccf97cdbdd598d9a70bd72821796c41311f5c0a475e3c7506f1717680038a9836e8a79516a85a0a7a1c4677e9a26d80dd567d42e81683eb35f7257a286469fdfa9e4ea8a0e399dc8e9749b488bf5180b0eb048c880457","public_key":"ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9"},{"pop_hex":"0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5","public_key":"ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39"},{"pop_hex":"0x92116175995869a1e32d7964fcea60289c50c9fd1bb7baa35453b4cea19e20e2cd75db3456f969217f2332662678632b074b46d6d23b91d2049023a8acd40d1de304fcd4bdd09b5e1e62bc0ef2e7f3c1e4a0d0f587c98f36b7be9d1744d9e165","public_key":"ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A"},{"pop_hex":"0x932e9c24c0cee5b3c79174f89bffca9068ab96a5a183e29d2b0ca64ce688ca3caba4e63bd67fb2532b8ee1cdee19f479014a8df44540fda6853b161efcc8afc1561dbbc6422830d713087de8cb29f176f678fc8793c589f9bedbee53d84e1c73","public_key":"ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"}]'
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
                  GENESIS_PUBLIC_KEY: ed0120CB6D05AFB7B05D488ACD6B5558FA285ECCCA40DC59D188338AB364494A629B82
                  P2P_ADDRESS: 0.0.0.0:1338
                  P2P_PUBLIC_ADDRESS: irohad1:1338
                  PRIVATE_KEY: 892620C0E8730DDB91DF6EA2049A6C92B8CB76C5FAEEB61AD87D969B07585650E2D94C
                  PUBLIC_KEY: ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0
                  TRUSTED_PEERS: '["ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9","ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39","ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A"]'
                  TRUSTED_PEERS_POP: '[{"pop_hex":"0xb9e4f1ef4aae8b525ceccf97cdbdd598d9a70bd72821796c41311f5c0a475e3c7506f1717680038a9836e8a79516a85a0a7a1c4677e9a26d80dd567d42e81683eb35f7257a286469fdfa9e4ea8a0e399dc8e9749b488bf5180b0eb048c880457","public_key":"ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9"},{"pop_hex":"0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5","public_key":"ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39"},{"pop_hex":"0x92116175995869a1e32d7964fcea60289c50c9fd1bb7baa35453b4cea19e20e2cd75db3456f969217f2332662678632b074b46d6d23b91d2049023a8acd40d1de304fcd4bdd09b5e1e62bc0ef2e7f3c1e4a0d0f587c98f36b7be9d1744d9e165","public_key":"ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A"},{"pop_hex":"0x932e9c24c0cee5b3c79174f89bffca9068ab96a5a183e29d2b0ca64ce688ca3caba4e63bd67fb2532b8ee1cdee19f479014a8df44540fda6853b161efcc8afc1561dbbc6422830d713087de8cb29f176f678fc8793c589f9bedbee53d84e1c73","public_key":"ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"}]'
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
                  GENESIS_PUBLIC_KEY: ed0120CB6D05AFB7B05D488ACD6B5558FA285ECCCA40DC59D188338AB364494A629B82
                  P2P_ADDRESS: 0.0.0.0:1339
                  P2P_PUBLIC_ADDRESS: irohad2:1339
                  PRIVATE_KEY: 892620D48701E56C55E12E34FFE323E9073A99FB020D24EA5136415C1724C78270790F
                  PUBLIC_KEY: ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A
                  TRUSTED_PEERS: '["ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9","ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39","ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"]'
                  TRUSTED_PEERS_POP: '[{"pop_hex":"0xb9e4f1ef4aae8b525ceccf97cdbdd598d9a70bd72821796c41311f5c0a475e3c7506f1717680038a9836e8a79516a85a0a7a1c4677e9a26d80dd567d42e81683eb35f7257a286469fdfa9e4ea8a0e399dc8e9749b488bf5180b0eb048c880457","public_key":"ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9"},{"pop_hex":"0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5","public_key":"ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39"},{"pop_hex":"0x92116175995869a1e32d7964fcea60289c50c9fd1bb7baa35453b4cea19e20e2cd75db3456f969217f2332662678632b074b46d6d23b91d2049023a8acd40d1de304fcd4bdd09b5e1e62bc0ef2e7f3c1e4a0d0f587c98f36b7be9d1744d9e165","public_key":"ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A"},{"pop_hex":"0x932e9c24c0cee5b3c79174f89bffca9068ab96a5a183e29d2b0ca64ce688ca3caba4e63bd67fb2532b8ee1cdee19f479014a8df44540fda6853b161efcc8afc1561dbbc6422830d713087de8cb29f176f678fc8793c589f9bedbee53d84e1c73","public_key":"ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"}]'
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
                  GENESIS_PUBLIC_KEY: ed0120CB6D05AFB7B05D488ACD6B5558FA285ECCCA40DC59D188338AB364494A629B82
                  P2P_ADDRESS: 0.0.0.0:1340
                  P2P_PUBLIC_ADDRESS: irohad3:1340
                  PRIVATE_KEY: 892620A58A3381FD2BB62E71F093EED8EDF9B7963A0BD2E6FF7F8E04994E0D70E7CF50
                  PUBLIC_KEY: ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9
                  TRUSTED_PEERS: '["ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39","ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A","ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"]'
                  TRUSTED_PEERS_POP: '[{"pop_hex":"0xb9e4f1ef4aae8b525ceccf97cdbdd598d9a70bd72821796c41311f5c0a475e3c7506f1717680038a9836e8a79516a85a0a7a1c4677e9a26d80dd567d42e81683eb35f7257a286469fdfa9e4ea8a0e399dc8e9749b488bf5180b0eb048c880457","public_key":"ea013081877D45ECCFCDB08F4CE36B9E948F0C6367E0A6066712D989E4E40BD8577417AD633132C935DF04D5AF546FDEC356E9"},{"pop_hex":"0xb9cd3633f028c577f15527ce9ea028e5b80223d380465f50cbc1cb0fbae7b608e63e28a0b10078e7a7f6e8954b35bb5712ad7d22e23469a11943052c035959444cde9cf2fc7524f4a17200c4e9f08620985f456f75c010c9d9682f8caa6c3cd5","public_key":"ea01308F5DE301D031B70D0E3B5410135371DB3F9639030F3FAC945925C94F7FF94E5D4FEEEA8965412A9E3348FF237DE0BF39"},{"pop_hex":"0x92116175995869a1e32d7964fcea60289c50c9fd1bb7baa35453b4cea19e20e2cd75db3456f969217f2332662678632b074b46d6d23b91d2049023a8acd40d1de304fcd4bdd09b5e1e62bc0ef2e7f3c1e4a0d0f587c98f36b7be9d1744d9e165","public_key":"ea0130A3C2A34B6C65EEC10C3B044F4F29401B43E8A8A4E104BA6B2ED9D00C0706FC196811D4BC96A88989F7FA6904FEF5F90A"},{"pop_hex":"0x932e9c24c0cee5b3c79174f89bffca9068ab96a5a183e29d2b0ca64ce688ca3caba4e63bd67fb2532b8ee1cdee19f479014a8df44540fda6853b161efcc8afc1561dbbc6422830d713087de8cb29f176f678fc8793c589f9bedbee53d84e1c73","public_key":"ea0130AF63B2147C0EF0F20FD16CEDB429F427BCEA17EEC18224324B996077D9BBD53EF7842E96B670ECA000EA82B3A88CD8F0"}]'
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
