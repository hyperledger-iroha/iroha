//! A node subscribes to the same original P2P actor that owns its outbound transport.

use super::*;
use iroha_config::{
    base::{
        WithOrigin,
        file_source::{ConfigFileAccess, ConfigFileRequest, ConfigFileSource},
        read::ConfigReader,
        toml::TomlSource,
    },
    parameters::user,
};
use iroha_futures::supervisor::Supervisor;
use iroha_p2p::{
    P2pIdentityKeys,
    network::message::{UpdatePeers, UpdateTopology},
};
use iroha_sumeragi::{
    crypto::{Crypto, Signer},
    message::{Vote, VoteKind, WireMessage},
    types::Signature,
};
use std::collections::HashSet;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("real P2P test runtime")
}

// This source satisfies configuration parsing only. Its keyring is deliberately not
// canonical and cannot admit a publisher service; no operational credentials are produced.
const PARSER_KEYRING_PATH: &str = "/__iroha_core_p2p_test__/unadmitted-publisher-keyring";
const PARSER_SUBMITTER_PATH: &str = "/__iroha_core_p2p_test__/publisher-submitter";
const UNADMITTED_KEYRING: &[u8] = b"configuration-parser-only; not an authenticated keyring";
struct ParserOnlyPublisherFiles;
impl ConfigFileSource for ParserOnlyPublisherFiles {
    fn read(
        &self,
        path: &std::path::Path,
        request: ConfigFileRequest,
    ) -> std::io::Result<zeroize::Zeroizing<Vec<u8>>> {
        if request.access != ConfigFileAccess::Private {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        let bytes = if path == std::path::Path::new(PARSER_KEYRING_PATH) {
            UNADMITTED_KEYRING.to_vec()
        } else if path == std::path::Path::new(PARSER_SUBMITTER_PATH) {
            let fixture = TomlSource::inline(
                include_str!("../../../../../iroha_config/iroha_test_config.toml")
                    .parse()
                    .unwrap(),
            );
            fixture.table()["private_key"]
                .as_str()
                .unwrap()
                .as_bytes()
                .to_vec()
        } else {
            return Err(std::io::ErrorKind::NotFound.into());
        };
        if bytes.len() > request.maximum {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        Ok(zeroize::Zeroizing::new(bytes))
    }
}
fn parser_only_network_reader(path: &std::path::Path) -> ConfigReader {
    let refs = TomlSource::inline(
        format!(
            "[kagemusha_load_authorizer]\nkeyring_file = {PARSER_KEYRING_PATH:?}\nsubmitter_key_file = {PARSER_SUBMITTER_PATH:?}\n",
        ).parse().unwrap(),
    );
    ConfigReader::new()
        .without_env()
        .read_toml_with_extends(path)
        .unwrap()
        .with_toml_source(refs)
}
#[test]
fn parser_only_publisher_source_preserves_required_private_bounded_custody_refusal() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../iroha_config/iroha_test_config.toml");
    let missing = ConfigReader::new()
        .without_env()
        .read_toml_with_extends(&path)
        .unwrap()
        .read_and_complete::<user::Root>()
        .unwrap()
        .parse_with_file_source(&ParserOnlyPublisherFiles)
        .unwrap_err();
    assert!(
        format!("{missing:?}").contains(
            "kagemusha_load_authorizer requires both keyring_file and submitter_key_file"
        )
    );
    let actual = parser_only_network_reader(&path)
        .read_and_complete::<user::Root>()
        .unwrap()
        .parse_with_file_source(&ParserOnlyPublisherFiles)
        .unwrap();
    assert_eq!(
        actual.kagemusha_load_authorizer.custody.keyring.as_slice(),
        UNADMITTED_KEYRING
    );
    assert!(
        crate::kagemusha_wallet_v1::PublicationWorker::from_canonical_keyring(
            &actual.kagemusha_load_authorizer.custody.keyring
        )
        .is_err()
    );
    assert_eq!(
        actual.network.address.value(),
        actual.network.public_address.value()
    );
    let private = ConfigFileRequest {
        access: ConfigFileAccess::Private,
        maximum: 65_536,
    };
    let files = ParserOnlyPublisherFiles;
    assert_eq!(
        files
            .read(std::path::Path::new(PARSER_KEYRING_PATH), private)
            .unwrap()
            .as_slice(),
        UNADMITTED_KEYRING
    );
    let submitter = files
        .read(std::path::Path::new(PARSER_SUBMITTER_PATH), private)
        .unwrap();
    let canonical: iroha_crypto::PrivateKey =
        std::str::from_utf8(&submitter).unwrap().parse().unwrap();
    assert_eq!(
        actual
            .kagemusha_load_authorizer
            .custody
            .submitter
            .private_key(),
        &canonical
    );
    for (path, request, kind) in [
        ("another-keyring", private, std::io::ErrorKind::NotFound),
        (
            PARSER_KEYRING_PATH,
            ConfigFileRequest {
                access: ConfigFileAccess::Public,
                ..private
            },
            std::io::ErrorKind::PermissionDenied,
        ),
        (
            PARSER_KEYRING_PATH,
            ConfigFileRequest {
                maximum: UNADMITTED_KEYRING.len() - 1,
                ..private
            },
            std::io::ErrorKind::InvalidData,
        ),
    ] {
        assert_eq!(
            files
                .read(std::path::Path::new(path), request)
                .unwrap_err()
                .kind(),
            kind
        );
    }
}

fn network_config(replay_root: &std::path::Path) -> iroha_config::parameters::actual::Network {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../iroha_config/iroha_test_config.toml");
    let user = parser_only_network_reader(&path)
        .read_and_complete::<user::Root>()
        .unwrap();
    let mut config = user
        .parse_with_file_source(&ParserOnlyPublisherFiles)
        .unwrap()
        .network;
    // Bind a fresh loopback address; the production parser supplies all protocol and
    // byte-bound defaults. This test changes only local listening/dial timing settings.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().into();
    drop(listener);
    config.address = WithOrigin::inline(address);
    config.public_address = config.address.clone();
    config.connect_startup_delay = Duration::ZERO;
    config.soranet_handshake.pow.revocation_store_path = replay_root
        .join("revocations.norito")
        .to_str()
        .unwrap()
        .to_owned()
        .into();
    config
}

async fn live_network(
    key: &KeyPair,
    network_id: NetworkId,
    targets: HashSet<PeerId>,
    supervisor: &mut Supervisor,
) -> (
    IrohaNetwork,
    iroha_primitives::addr::SocketAddr,
    tempfile::TempDir,
) {
    let replay_root = tempfile::tempdir().unwrap();
    let config = network_config(replay_root.path());
    let address = config.public_address.value().clone();
    let transport = KeyPair::from_seed(vec![0xEA; 32], Algorithm::Ed25519);
    let (network, child) = IrohaNetwork::start_with_crypto_and_initial_authorities(
        P2pIdentityKeys::new(key.clone(), transport).unwrap(),
        config,
        network_id,
        None,
        None,
        None,
        targets.clone(),
        targets,
        supervisor.shutdown_signal(),
    )
    .await
    .expect("real network actor startup");
    supervisor.monitor(child);
    (network, address, replay_root)
}

fn prepared_root(chain: &Chain) -> (Arc<State>, Arc<Kura>, Prepared) {
    let kura = Kura::blank_kura_for_testing();
    let state = empty_state(&chain.chain_id, &chain.genesis, &kura);
    let prepared = prepare(PrepareInputs {
        state: Arc::clone(&state),
        events: tokio::sync::broadcast::channel(16).0,
        genesis: Some(chain.genesis.clone()),
        genesis_account: SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        consensus_mode: ConsensusMode::Permissioned,
    })
    .expect("authenticated original root");
    (state, kura, prepared)
}

fn inputs(
    chain: &Chain,
    dir: &std::path::Path,
    network: IrohaNetwork,
) -> StartInputs<P2pNet<IrohaNetwork>> {
    let (_, time) = TimeSource::new_mock(Duration::ZERO);
    StartInputs {
        net: Arc::new(P2pNet::new(network)),
        queue: Arc::new(Queue::test(
            iroha_config::parameters::actual::Queue::default(),
            &time,
        )),
        key_pair: chain.keys[0].clone(),
        beacon_signer: None,
        config: NodeConfig {
            records_dir: dir.join("records"),
            installation_log: dir.join("keys/installation.log"),
            bodies_dir: dir.join("bodies"),
            local: SumeragiLocalOverrides::default(),
            assert_fresh_key: true,
            retired_keys: Vec::new(),
        },
        observer: Arc::new(LogObserver),
        driver: DriverConfig::default(),
    }
}

#[test]
fn network_start_rejects_closed_retained_actor_before_driver_files() {
    let chain = chain(4, 200);
    let (state, kura, prepared) = prepared_root(&chain);
    let dir = tempfile::tempdir().unwrap();
    let runtime = runtime();
    let mut supervisor = Supervisor::new();
    let (unrelated, _, _replay_root) = runtime.block_on(live_network(
        &chain.keys[1],
        *state.network_id_ref(),
        HashSet::from([PeerId::new(chain.keys[0].public_key().clone())]),
        &mut supervisor,
    ));
    assert!(
        unrelated.online_peers_receiver().has_changed().is_ok(),
        "the independent actor remains live but grants no startup authority"
    );
    let result = prepared.start_on_network(
        inputs(&chain, dir.path(), IrohaNetwork::closed_for_tests()),
        16,
    );
    let files_created = ["records", "keys", "bodies"].map(|path| dir.path().join(path).exists());
    // Clean up any wrongly started original driver before the failing-before assertion.
    let error = match result {
        Ok(node) => {
            node.shutdown();
            None
        }
        Err(error) => Some(error),
    };
    supervisor.shutdown_signal().send();
    runtime
        .block_on(supervisor.start())
        .expect("normal network shutdown");
    assert!(
        matches!(error, Some(NodeError::Subscription(_))),
        "a separate live subscription actor cannot authorize a closed retained sender: {error:?}; files={files_created:?}"
    );
    assert!(!dir.path().join("records").exists());
    assert!(!dir.path().join("keys").exists());
    assert!(!dir.path().join("bodies").exists());
    assert_eq!(state.view().height(), 1);
    assert_eq!(kura.blocks_count(), 1);
}

#[test]
fn retained_live_actor_delivers_signed_native_ingress() {
    let chain = chain(4, 200);
    let (state, kura, prepared) = prepared_root(&chain);
    let instance = prepared.instance();
    let config = state
        .view()
        .world()
        .consensus_schedule()
        .ready(2)
        .unwrap()
        .height_config()
        .unwrap();
    let remote = KeyPairSigner::new(&chain.keys[1]).unwrap();
    let mut vote = Vote {
        kind: VoteKind::Prepare,
        instance,
        epoch: config.epoch.id,
        height: 2,
        view: 0,
        block_hash: Hash32([0xAB; 32]),
        result: Hash32([0xBC; 32]),
        signer: config.committee.index_of(remote.public_key()).unwrap(),
        sig: Signature([0; 96]),
    };
    vote.sig = remote.sign(&vote.preimage());
    assert!(
        prepared
            .crypto
            .verify(remote.public_key(), &vote.preimage(), &vote.sig)
    );
    let frame = Frame {
        instance,
        class: iroha_sumeragi::message::TrafficClass::Control,
        bytes: Arc::from(WireMessage::Vote(vote).encode().unwrap()),
    };
    let dir = tempfile::tempdir().unwrap();
    let runtime = runtime();
    let mut supervisor = Supervisor::new();
    let local_peer = PeerId::new(chain.keys[0].public_key().clone());
    let remote_peer = PeerId::new(chain.keys[1].public_key().clone());
    let (local, _, _local_replay_root) = runtime.block_on(live_network(
        &chain.keys[0],
        *state.network_id_ref(),
        HashSet::from([remote_peer.clone()]),
        &mut supervisor,
    ));
    let (remote_actor, remote_address, _remote_replay_root) = runtime.block_on(live_network(
        &chain.keys[1],
        *state.network_id_ref(),
        HashSet::from([local_peer.clone()]),
        &mut supervisor,
    ));
    let node = prepared
        .start_on_network(inputs(&chain, dir.path(), local.clone()), 16)
        .unwrap();
    let ingress = Arc::clone(&node.ingress);
    let sender = P2pNet::new(remote_actor.clone());
    local.update_topology(UpdateTopology(HashSet::from([remote_peer.clone()])));
    remote_actor.update_topology(UpdateTopology(HashSet::from([local_peer])));
    local.update_peers_addresses(UpdatePeers(vec![(remote_peer, remote_address)]));
    let outcome = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(20), async {
            for network in [&local, &remote_actor] {
                let mut peers = network.online_peers_receiver();
                while peers.borrow().is_empty() {
                    peers
                        .changed()
                        .await
                        .map_err(|_| "original peer watch closed")?;
                }
            }
            let sent = sender.send(&core_key(chain.keys[0].public_key()).unwrap(), &frame);
            if !matches!(sent, SendOutcome::Admitted) {
                return Err("live original actor refused the signed message");
            }
            while ingress.stats().delivered == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Ok(())
        })
        .await
    });
    node.shutdown();
    supervisor.shutdown_signal().send();
    runtime
        .block_on(supervisor.start())
        .expect("normal peer network shutdown");
    outcome
        .expect("signed original frame crosses the retained actor subscription")
        .expect("live original peer admission");
    assert!(ingress.stats().delivered > 0);
    assert_eq!(
        state.view().height(),
        1,
        "one vote grants no publication authority"
    );
    assert_eq!(kura.blocks_count(), 1);
}
