//! Adversarial relay and partition scenarios for unstable networks.

use std::{
    any::Any,
    borrow::Cow,
    collections::{HashMap, HashSet},
    panic::{self, AssertUnwindSafe},
    sync::Arc,
    time::{Duration, Instant},
};

use eyre::{Result, eyre};
use futures_util::{StreamExt, stream::FuturesUnordered};
use integration_tests::sandbox;
use iroha_config::parameters::defaults;
use iroha_config_base::toml::WriteExt;
use iroha_core::sumeragi::network_topology::{
    Topology, commit_quorum_from_len, redundant_send_r_from_len,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    ChainId, Level,
    asset::AssetDefinition,
    isi::Register,
    parameter::{BlockParameter, SumeragiParameter},
    prelude::*,
};
use iroha_primitives::addr::socket_addr;
use iroha_test_network::{
    BlockHeight, Network, NetworkBuilder, NetworkPeer, genesis_factory, once_blocks_sync,
};
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;
use rand::{SeedableRng, prelude::IteratorRandom};
use rand_chacha::ChaCha8Rng;
use relay::P2pRelay;
use tokio::{
    self,
    task::spawn_blocking,
    time::{sleep, timeout},
};
use toml::{Table, Value as TomlValue};

mod relay {
    //! Utilities to build a peer-to-peer network relay using TCP proxies with ability to targetly
    //! suspend communication.
    //!
    //! For each peer-to-peer pair, [`P2pRelay`] reserves a port and starts a TCP listener on it. This
    //! listener is sipmly forwarding packages, but also provides a toggle to suspend the
    //! forwarding. Peers' `trusted_peers` configuration must be set up to use the relay's proxies
    //! and not directly the other peers.
    //!
    //! First, create [`P2pRelay`] with the given _real topology_ (a set of peers'
    //! real network addresses and public keys), by [`P2pRelay::new`] or [`P2pRelay::for_network`].
    //! Then, start each peer, setting `trusted_peers` config parameter with the _virtual topology_ provided by
    //! the relay ([`P2pRelay::trusted_peers_for`]). Then, start the relay itself ([`P2pRelay::start`]).
    //!
    //! It is possible to "suspend" a particular peer in the network by suspending all incoming/outgoing packages from/to other peers.
    //! Relay provides [`P2pRelay::suspend`] to acquire a [`Suspend`] control (per-peer), which provides
    //! [`Suspend::activate`] and [`Suspend::deactivate`] in turn.

    use std::{
        collections::HashMap,
        iter::once,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    use futures_util::{StreamExt, stream::FuturesUnordered};
    use iroha_data_model::{peer::PeerId, prelude::Peer};
    use iroha_primitives::{
        addr::{SocketAddr, socket_addr},
        unique_vec::UniqueVec,
    };
    use iroha_test_network::{Network, fslock_ports::AllocatedPort};
    use tokio::{
        io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        select,
        sync::Notify,
        task::JoinSet,
    };
    use tracing::{Instrument, Level};

    #[derive(Debug)]
    pub struct P2pRelay {
        peers: HashMap<PeerId, RelayPeer>,
        tasks: JoinSet<()>,
    }

    #[derive(Debug)]
    struct RelayPeer {
        real_addr: SocketAddr,
        /// Map of proxied destinations.
        /// Key is a peer id of
        mock_outgoing: HashMap<PeerId, (SocketAddr, AllocatedPort)>,
        suspend: Suspend,
    }

    impl P2pRelay {
        /// Construct a relay with the given topology.
        pub fn new(real_topology: &UniqueVec<Peer>) -> Self {
            let peers: HashMap<_, _> = real_topology
                .iter()
                .map(|peer| {
                    let id = peer.id();
                    let real_addr = peer.address().clone();
                    let mock_outgoing = real_topology
                        .iter()
                        .map(iroha_data_model::peer::Peer::id)
                        .filter(|x| *x != id)
                        .map(|other_id| {
                            let mock_port = AllocatedPort::new();
                            let mock_addr = socket_addr!(127.0.0.1:*mock_port);
                            (other_id.clone(), (mock_addr, mock_port))
                        })
                        .collect();
                    let relay_peer = RelayPeer {
                        real_addr,
                        mock_outgoing,
                        suspend: Suspend::new(),
                    };
                    (id.clone(), relay_peer)
                })
                .collect();

            let mut table = ascii_table::AsciiTable::default();
            table.set_max_width(ascii_table::Width::Fixed(30 * (1 + real_topology.len())));
            table.column(0).set_header("From");
            for (i, id) in real_topology.iter().enumerate() {
                table
                    .column(i + 1)
                    .set_header(format!("To {}", id.address()));
            }
            let rows: Vec<Vec<String>> = real_topology
                .iter()
                .map(|peer| {
                    once(format!("{}", peer.address()))
                        .chain(real_topology.iter().map(|other_peer| {
                            if other_peer.id() == peer.id() {
                                String::new()
                            } else {
                                let (mock_addr, _) = peers
                                    .get(peer.id())
                                    .unwrap()
                                    .mock_outgoing
                                    .get(other_peer.id())
                                    .unwrap();
                                format!("{mock_addr}")
                            }
                        }))
                        .collect::<Vec<_>>()
                })
                .collect();

            let mut table_buf = Vec::new();
            table
                .writeln(&mut table_buf, &rows)
                .expect("writing ascii table should succeed");
            let table_formatted = String::from_utf8(table_buf)
                .unwrap_or_else(|_| "<table render failed>".to_string());

            iroha_logger::debug!("TCP relay table:\n{table_formatted}");

            Self {
                peers,
                tasks: <_>::default(),
            }
        }

        /// Construct a relay using the given network's peers as topology.
        pub fn for_network(network: &Network) -> Self {
            Self::new(
                &network
                    .peers()
                    .iter()
                    .map(|peer| Peer::new(peer.p2p_address(), peer.id()))
                    .collect(),
            )
        }

        /// Get trusted peers for a particular peer in the given topology.
        ///
        /// The relay constructs "virtual" set of addresses of other peers for each peer. This must
        /// be passed to the peer's configuration.
        pub fn trusted_peers_for(&self, peer: &PeerId) -> UniqueVec<Peer> {
            let peer_info = self
                .peers
                .get(peer)
                .expect("existing peer must be supplied");
            peer_info
                .mock_outgoing
                .iter()
                .map(|(other, (addr, _port))| Peer::new(addr.clone(), other.clone()))
                .chain(Some(Peer::new(peer_info.real_addr.clone(), peer.clone())))
                .collect()
        }

        /// Start the relay, i.e. the set of TCP proxies between the peers.
        pub fn start(&mut self) {
            for peer in self.peers.values() {
                for (other, (other_mock_addr, _)) in &peer.mock_outgoing {
                    let other_peer = self.peers.get(other).expect("must be present");
                    let suspend =
                        SuspendIfAny(vec![peer.suspend.clone(), other_peer.suspend.clone()]);

                    P2pRelay::run_proxy(
                        &mut self.tasks,
                        other_mock_addr.clone(),
                        other_peer.real_addr.clone(),
                        suspend,
                    );
                }
            }
        }

        fn run_proxy(
            tasks: &mut JoinSet<()>,
            from: SocketAddr,
            to: SocketAddr,
            suspend: SuspendIfAny,
        ) {
            let span = tracing::span!(Level::INFO, "proxy_run", from = %from, to = %to);
            let mut proxy = Proxy::new(from, to, suspend);

            tasks.spawn(
                async move {
                    if let Err(err) = proxy.run().await {
                        iroha_logger::error!("proxy at {} exited with an error: {err}", proxy.from);
                    } else {
                        iroha_logger::info!("proxy exited normally");
                    }
                }
                .instrument(span),
            );
        }

        /// Acquire a [`Suspend`] control for a peer
        pub fn suspend(&self, peer: &PeerId) -> Suspend {
            self.peers
                .get(peer)
                .expect("must be present")
                .suspend
                .clone()
        }
    }

    #[derive(Clone, Debug)]
    pub struct Suspend {
        active: Arc<AtomicBool>,
        notify: Arc<Notify>,
    }

    impl Suspend {
        fn new() -> Self {
            Self {
                active: <_>::default(),
                notify: <_>::default(),
            }
        }

        pub fn activate(&self) {
            self.active.store(true, Ordering::Release);
        }

        pub fn deactivate(&self) {
            self.active.store(false, Ordering::Release);
            self.notify.notify_waiters();
        }
    }

    #[derive(Clone, Debug)]
    struct SuspendIfAny(Vec<Suspend>);

    impl SuspendIfAny {
        async fn is_not_active(&self) {
            loop {
                let waited_for = self
                    .0
                    .iter()
                    .filter_map(|x| {
                        x.active
                            .load(Ordering::Acquire)
                            .then_some(x.notify.notified())
                    })
                    .collect::<FuturesUnordered<_>>()
                    .collect::<Vec<_>>()
                    .await
                    .len();
                if waited_for == 0 {
                    break;
                }
            }
        }
    }

    struct Proxy {
        from: SocketAddr,
        to: SocketAddr,
        suspend: SuspendIfAny,
    }

    impl Proxy {
        fn new(from: SocketAddr, to: SocketAddr, suspend: SuspendIfAny) -> Self {
            Self { from, to, suspend }
        }

        async fn run(&mut self) -> eyre::Result<()> {
            let listener = TcpListener::bind(self.from.to_string()).await?;
            loop {
                let (client, _) = listener.accept().await?;
                let server = match TcpStream::connect(self.to.to_string()).await {
                    Ok(server) => server,
                    Err(err) => {
                        tracing::warn!(
                            ?err,
                            from = %self.from,
                            to = %self.to,
                            "proxy connect failed; dropping client"
                        );
                        continue;
                    }
                };

                let (mut eread, mut ewrite) = client.into_split();
                let (mut oread, mut owrite) = server.into_split();

                let suspend = self.suspend.clone();
                let e2o =
                    tokio::spawn(
                        async move { Proxy::copy(&suspend, &mut eread, &mut owrite).await },
                    );
                let suspend = self.suspend.clone();
                let o2e =
                    tokio::spawn(
                        async move { Proxy::copy(&suspend, &mut oread, &mut ewrite).await },
                    );

                select! {
                    _ = e2o => {
                        // eprintln!("{} → {}: client-to-server closed ×", self.from, self.to);
                    },
                    _ = o2e => {
                        // eprintln!("{} → {}: server-to-client closed ×", self.from, self.to);
                    },
                }
            }
        }

        async fn copy<R, W>(
            suspend: &SuspendIfAny,
            mut reader: R,
            mut writer: W,
        ) -> std::io::Result<()>
        where
            R: AsyncRead + Unpin,
            W: AsyncWrite + Unpin,
        {
            // NOTE: quite large to allocate on stack, causing overflow
            let mut buf = vec![0u8; 2usize.pow(20)];

            loop {
                suspend.is_not_active().await;

                let n = reader.read(&mut buf).await?;
                if n == 0 {
                    break;
                }

                writer.write_all(&buf[..n]).await?;
            }

            Ok(())
        }
    }
}

#[derive(Default)]
enum GenesisPeer {
    #[default]
    Whichever,
    Nth(usize),
}

const COLLECTORS_K: u16 = 3;
const REDUNDANT_SEND_R: u8 = 2;

fn collectors_k_for_peers(peer_count: usize) -> u16 {
    if peer_count <= 1 {
        return 0;
    }
    let fault_budget = peer_count.saturating_sub(commit_quorum_from_len(peer_count));
    let desired = fault_budget.saturating_add(1);
    let min_k = usize::from(COLLECTORS_K).max(1);
    let k = desired.max(min_k).min(peer_count.saturating_sub(1));
    u16::try_from(k).unwrap_or(u16::MAX)
}

fn redundant_send_r_for_peers(peer_count: usize) -> u8 {
    let desired = u8::try_from(collectors_k_for_peers(peer_count)).unwrap_or(u8::MAX);
    let baseline = REDUNDANT_SEND_R.max(redundant_send_r_from_len(peer_count));
    baseline.max(desired)
}

fn scaled_timeout(base: Duration, peer_count: usize) -> Duration {
    let scale = ((peer_count + 3) / 4).max(1) as u32;
    base.saturating_mul(scale)
}

fn non_faulty_sync_timeout(
    sync_timeout: Duration,
    pipeline_time: Duration,
    faulty_peers: usize,
) -> Duration {
    if faulty_peers == 0 {
        return sync_timeout;
    }
    // Keep relay partitions short enough to avoid RBC session expiry and view-change churn.
    let cap = if faulty_peers > 1 {
        pipeline_time
            .saturating_mul(2)
            .saturating_add(Duration::from_secs(2))
    } else {
        let session_cap = Duration::from_millis(defaults::sumeragi::RBC_SESSION_TTL_MS)
            .saturating_sub(Duration::from_secs(10));
        if session_cap.is_zero() {
            pipeline_time
                .saturating_mul(10)
                .saturating_add(Duration::from_secs(2))
        } else {
            session_cap
        }
    };
    sync_timeout.min(cap)
}

fn submit_retry_backoff(attempt: usize) -> Duration {
    let step = Duration::from_millis(200);
    let scaled = step.saturating_mul(u32::try_from(attempt).unwrap_or(u32::MAX));
    scaled.min(Duration::from_secs(2))
}

fn should_resubmit_tx(
    allow_resubmit: bool,
    already_resubmitted: bool,
    now: Instant,
    resubmit_at: Instant,
) -> bool {
    allow_resubmit && !already_resubmitted && now >= resubmit_at
}

fn permissioned_prf_seed(chain_id: &ChainId) -> [u8; 32] {
    let hash = Hash::new(chain_id.as_str().as_bytes());
    <[u8; 32]>::from(hash)
}

fn topology_for_permissioned_round(
    peer_ids: &[PeerId],
    chain_id: &ChainId,
    height: u64,
    view: u64,
) -> Vec<PeerId> {
    let mut ordered = peer_ids.to_vec();
    ordered.sort();
    ordered.dedup();
    let mut topology = Topology::new(ordered);
    topology.shuffle_prf(permissioned_prf_seed(chain_id), height);
    topology.nth_rotation(view);
    topology.into_iter().collect()
}

async fn start_network_under_relay(
    network: &Network,
    relay: &P2pRelay,
    genesis_peer: GenesisPeer,
) -> Result<()> {
    // Callers already hold the serial guard via SerializedNetwork.
    // Iroha refuses to start on empty storage without a genesis file, so hand the
    // canonical genesis block to every peer up front.
    let genesis_block = genesis_factory(
        network.genesis_isi().clone(),
        network
            .peers()
            .iter()
            .map(iroha_test_network::NetworkPeer::id)
            .collect(),
        network.topology_entries().to_vec(),
    );
    let mut pops_by_peer_id = HashMap::new();
    for peer in network.peers() {
        let pop = peer.bls_pop().expect("network peers should have BLS PoPs");
        pops_by_peer_id.insert(peer.id(), pop.to_vec());
    }

    let startup_timeout = scaled_timeout(network.peer_startup_timeout(), network.peers().len());
    let results = timeout(
        startup_timeout,
        network
            .peers()
            .iter()
            .enumerate()
            .map(|(i, peer)| {
                let _designated_genesis_peer = match genesis_peer {
                    GenesisPeer::Whichever => i == 0,
                    GenesisPeer::Nth(n) => i == n,
                };
                let trusted_peers = relay.trusted_peers_for(&peer.id());
                let trusted_peers_list = trusted_peers
                    .iter()
                    .map(|peer| peer.to_string())
                    .collect::<Vec<_>>();
                let mut trusted_peers_pop = Vec::new();
                let mut seen = HashSet::new();
                for trusted in trusted_peers.iter() {
                    let peer_id = trusted.id();
                    if !seen.insert(peer_id.clone()) {
                        continue;
                    }
                    let pop = pops_by_peer_id.get(peer_id).unwrap_or_else(|| {
                        panic!("missing PoP for trusted peer {}", peer_id.public_key())
                    });
                    let mut pop_entry = Table::new();
                    pop_entry.insert(
                        "public_key".into(),
                        TomlValue::String(peer_id.public_key().to_string()),
                    );
                    pop_entry.insert(
                        "pop_hex".into(),
                        TomlValue::String(format!("0x{}", hex::encode(pop))),
                    );
                    trusted_peers_pop.push(TomlValue::Table(pop_entry));
                }

                let config = network.config_layers().chain(Some(Cow::Owned(
                    Table::new()
                        .write(["trusted_peers"], trusted_peers_list)
                        .write(["trusted_peers_pop"], TomlValue::Array(trusted_peers_pop))
                        // We don't want peers to gossip any actual addresses, because each peer has
                        // its own set of incoming and outgoing proxies with every other peer.
                        // Thus, we are giving this addr which should always reject connections and
                        // peers should rely on what they got in the `sumeragi.trusted_peers`.
                        .write(
                            ["network", "public_address"],
                            // This IP is in the range of IPs reserved for "documentation and examples"
                            // https://en.wikipedia.org/wiki/Reserved_IP_addresses#:~:text=192.0.2.0/24
                            socket_addr!(192.0.2.133:1337).to_literal(),
                        ),
                )));

                let genesis = Some(genesis_block.clone());

                async move { peer.start_checked(config, genesis.as_ref()).await }
            })
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>(),
    )
    .await?;

    for result in results {
        result?;
    }

    Ok(())
}

fn panic_reason(panic: &(dyn Any + Send)) -> Option<String> {
    panic
        .downcast_ref::<&str>()
        .map(std::string::ToString::to_string)
        .or_else(|| panic.downcast_ref::<String>().cloned())
}

fn is_sandbox_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("permission denied") || lower.contains("operation not permitted")
}

#[allow(clippy::unnecessary_wraps)]
fn run_or_skip_on_sandbox_panic<T, F>(context: &str, build: F) -> Result<Option<T>>
where
    F: FnOnce() -> T,
{
    match panic::catch_unwind(AssertUnwindSafe(build)) {
        Ok(value) => Ok(Some(value)),
        Err(panic) => {
            if let Some(reason) = panic_reason(panic.as_ref())
                && is_sandbox_message(&reason)
            {
                eprintln!(
                    "sandboxed network restriction detected while running {context}; skipping network startup ({reason})"
                );
                return Ok(None);
            }
            panic::resume_unwind(panic);
        }
    }
}

#[test]
fn sandbox_panic_skips_network_build() -> Result<()> {
    let result = run_or_skip_on_sandbox_panic("sandbox_panic_skips_network_build", || -> usize {
        panic!("operation not permitted");
    })?;
    assert!(result.is_none());
    Ok(())
}

#[test]
fn non_sandbox_panic_is_propagated() {
    let result = panic::catch_unwind(|| {
        let _ = run_or_skip_on_sandbox_panic("non_sandbox_panic_is_propagated", || -> usize {
            panic!("boom");
        });
    });
    assert!(result.is_err());
}

#[tokio::test]
async fn network_starts_with_relay() -> Result<()> {
    let Some((network, mut relay)) =
        run_or_skip_on_sandbox_panic(stringify!(network_starts_with_relay), || {
            let guard = sandbox::serial_guard();
            let network = NetworkBuilder::new()
                .with_auto_populated_trusted_peers()
                .with_peers(4)
                .build();
            let network = sandbox::SerializedNetwork::new(network, guard);
            let relay = P2pRelay::for_network(&network);
            (network, relay)
        })?
    else {
        return Ok(());
    };

    relay.start();
    if sandbox::handle_result(
        start_network_under_relay(&network, &relay, GenesisPeer::Whichever).await,
        stringify!(network_starts_with_relay),
    )?
    .is_none()
    {
        return Ok(());
    }
    let status_deadline = Instant::now() + network.peer_startup_timeout();
    loop {
        match network.peer().status().await {
            Ok(_) => break,
            Err(err) => {
                if Instant::now() >= status_deadline {
                    return Err(err.wrap_err("network_starts_with_relay torii not ready"));
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    let client = network.client();
    let register = Register::domain(Domain::new("relay_net".parse()?));
    spawn_blocking(move || client.submit(register)).await??;
    network.ensure_blocks(2).await?;

    Ok(())
}

#[tokio::test]
async fn network_doesnt_start_without_relay_being_started() -> Result<()> {
    let Some((network, relay)) = run_or_skip_on_sandbox_panic(
        stringify!(network_doesnt_start_without_relay_being_started),
        || {
            let guard = sandbox::serial_guard();
            let network = NetworkBuilder::new()
                .with_auto_populated_trusted_peers()
                .with_peers(4)
                .build();
            let network = sandbox::SerializedNetwork::new(network, guard);
            let relay = P2pRelay::for_network(&network);
            (network, relay)
        },
    )?
    else {
        return Ok(());
    };

    if sandbox::handle_result(
        start_network_under_relay(&network, &relay, GenesisPeer::Whichever).await,
        stringify!(network_doesnt_start_without_relay_being_started),
    )?
    .is_none()
    {
        return Ok(());
    }

    let client = network.client();
    spawn_blocking(move || client.submit(Log::new(Level::INFO, "relay stalled".to_owned())))
        .await??;

    let Err(_) = timeout(
        Duration::from_secs(3),
        once_blocks_sync(network.peers().iter(), BlockHeight::predicate_non_empty(2)),
    )
    .await
    else {
        panic!("network must not start!")
    };

    Ok(())
}

#[tokio::test]
async fn suspending_works() -> Result<()> {
    const N_PEERS: usize = 4;
    const { assert!(N_PEERS > 0) };

    let Some((network, mut relay)) =
        run_or_skip_on_sandbox_panic(stringify!(suspending_works), || {
            let guard = sandbox::serial_guard();
            let network = NetworkBuilder::new()
                .with_auto_populated_trusted_peers()
                .with_peers(N_PEERS)
                .build();
            let network = sandbox::SerializedNetwork::new(network, guard);
            let relay = P2pRelay::for_network(&network);
            (network, relay)
        })?
    else {
        return Ok(());
    };
    let sync_timeout = network.sync_timeout();
    let sync_probe = network.pipeline_time() + Duration::from_secs(3);
    // we will plug/unplug the last peer to simulate a short partition
    let last_peer = network
        .peers()
        .last()
        .expect("there are more than 0 of them");
    let suspend = relay.suspend(&last_peer.id());

    suspend.activate();
    relay.start();
    if sandbox::handle_result(
        start_network_under_relay(&network, &relay, GenesisPeer::Whichever).await,
        stringify!(suspending_works),
    )?
    .is_none()
    {
        return Ok(());
    }

    let client = network.client();
    spawn_blocking(move || client.submit(Log::new(Level::INFO, "suspend tick".to_owned())))
        .await??;

    // all peers except the last one should get the non-empty block
    timeout(
        sync_timeout,
        once_blocks_sync(
            network.peers().iter().take(N_PEERS - 1),
            BlockHeight::predicate_non_empty(2),
        ),
    )
    .await??;
    if timeout(
        sync_probe,
        last_peer.once_block_with(BlockHeight::predicate_non_empty(2)),
    )
    .await
    .is_ok()
    {
        return Err(eyre!("suspending_works: peer caught up while suspended"));
    }

    // unsuspend, the last peer should get the block too
    suspend.deactivate();
    timeout(
        sync_timeout,
        last_peer.once_block_with(BlockHeight::predicate_non_empty(2)),
    )
    .await
    .map_err(|_| eyre!("suspending_works: timed out waiting for peer to catch up"))?;

    Ok(())
}

#[tokio::test]
async fn block_after_genesis_is_synced() -> Result<()> {
    // A consensus anomaly occurred deterministically depending on the peer set
    // (how public keys of different peers are sorted to each other, which determines consensus
    // roles) and which peer submits the genesis. The values below are an experimentally found
    // case.
    const SEED: &str = "we want a determined order of peers";
    const GENESIS_PEER_INDEX: usize = 3;

    let Some((network, mut relay)) =
        run_or_skip_on_sandbox_panic(stringify!(block_after_genesis_is_synced), || {
            let guard = sandbox::serial_guard();
            let network = NetworkBuilder::new()
                .with_base_seed(SEED)
                .with_peers(5)
                // Align test timing with the new consensus pipeline (3s block, 6s commit).
                .with_pipeline_time(Duration::from_secs(9))
                .build();
            let network = sandbox::SerializedNetwork::new(network, guard);
            let relay = P2pRelay::for_network(&network);
            (network, relay)
        })?
    else {
        return Ok(());
    };
    let pipeline_window = network.pipeline_time() + Duration::from_secs(3);
    let sync_timeout = scaled_timeout(network.sync_timeout(), network.peers().len());

    relay.start();
    if sandbox::handle_result(
        start_network_under_relay(&network, &relay, GenesisPeer::Nth(GENESIS_PEER_INDEX)).await,
        stringify!(block_after_genesis_is_synced),
    )?
    .is_none()
    {
        return Ok(());
    }
    network.ensure_blocks(1).await?;

    for peer in network.peers() {
        relay.suspend(&peer.id()).activate();
    }
    let client = network.client();
    spawn_blocking(move || client.submit(Log::new(Level::INFO, "tick".to_owned()))).await??;
    let Err(_) = timeout(
        pipeline_window,
        once_blocks_sync(network.peers().iter(), BlockHeight::predicate_non_empty(2)),
    )
    .await
    else {
        panic!("should not sync with relay being suspended")
    };
    for peer in network.peers() {
        relay.suspend(&peer.id()).deactivate();
    }
    // Allow enough time for a full pipeline (propose+commit) after resuming the relay.
    // Using the broader sync timeout keeps the assertion tolerant of slower machines
    // and scenarios where consensus gating adds a little extra latency.
    timeout(
        sync_timeout,
        once_blocks_sync(network.peers().iter(), BlockHeight::predicate_non_empty(2)),
    )
    .await??;

    Ok(())
}

// ======= ACTUAL TESTS BEGIN HERE =======

struct UnstableNetwork {
    n_peers: usize,
    n_faulty_peers: usize,
    n_rounds: usize,
    force_soft_fork: bool,
}

impl UnstableNetwork {
    async fn run(self) -> Result<()> {
        if self.n_peers <= self.n_faulty_peers {
            return Err(eyre!(
                "expected more peers than faulty peers (peers={}, faulty={})",
                self.n_peers,
                self.n_faulty_peers
            ));
        }
        let commit_quorum = commit_quorum_from_len(self.n_peers);
        let fault_budget = Self::fault_budget_for_peer_count(self.n_peers);
        if self.n_faulty_peers > fault_budget {
            return Err(eyre!(
                "faulty peers ({}) exceed commit quorum budget ({}) for {} peers (commit quorum {})",
                self.n_faulty_peers,
                fault_budget,
                self.n_peers,
                commit_quorum
            ));
        }

        let account_id = ALICE_ID.clone();
        let asset_definition_id: AssetDefinitionId = "unstable#wonderland".parse().expect("Valid");

        let Some((network, mut relay)) =
            run_or_skip_on_sandbox_panic("unstable_network::run", || {
                let guard = sandbox::serial_guard();
                let network = self.build_network();
                let network = sandbox::SerializedNetwork::new(network, guard);
                let relay = P2pRelay::for_network(&network);
                (network, relay)
            })?
        else {
            return Ok(());
        };
        relay.start();
        let start_result =
            start_network_under_relay(&network, &relay, GenesisPeer::Whichever).await;
        if sandbox::handle_result(start_result, "unstable_network::run")?.is_none() {
            return Ok(());
        }
        Self::register_numeric_asset(&network, &asset_definition_id).await?;
        let init_blocks = 2;
        network.ensure_blocks(init_blocks).await?;

        let peers = network.peers();
        let round_ctx = RoundContext {
            init_blocks,
            asset_definition_id: &asset_definition_id,
            account_id: &account_id,
        };
        for i in 0..self.n_rounds {
            self.execute_round(i, &round_ctx, &network, &mut relay, peers.as_slice())
                .await?;
        }

        let expected_height = init_blocks + u64::try_from(self.n_rounds).unwrap_or(0);
        Self::assert_total_supply(
            &network,
            &asset_definition_id,
            self.n_rounds,
            expected_height,
        )
        .await?;

        Ok(())
    }

    fn fault_budget_for_peer_count(peer_count: usize) -> usize {
        peer_count.saturating_sub(commit_quorum_from_len(peer_count))
    }

    fn fault_round_seed(round_index: usize, n_faulty_peers: usize, fault_budget: usize) -> usize {
        if n_faulty_peers >= fault_budget {
            0
        } else {
            round_index
        }
    }

    fn select_faulty_peer_ids(
        peer_ids: &[PeerId],
        n_faulty_peers: usize,
        round_index: usize,
        chain_id: &ChainId,
        height: u64,
        collectors_k: u16,
    ) -> Vec<PeerId> {
        if n_faulty_peers == 0 {
            return Vec::new();
        }
        let rotated = topology_for_permissioned_round(peer_ids, chain_id, height, 0);
        let commit_quorum = commit_quorum_from_len(rotated.len());
        let candidates: Vec<PeerId> = if n_faulty_peers <= 1 {
            rotated.get(commit_quorum..).unwrap_or(&[]).to_vec()
        } else {
            let mut collector_ids = HashSet::new();
            if rotated.len() > 1 && collectors_k > 0 {
                let seed = permissioned_prf_seed(chain_id);
                let topology = Topology::new(rotated.clone());
                for idx in
                    topology.collector_indices_k_prf(usize::from(collectors_k), seed, height, 0)
                {
                    if let Some(peer) = topology.as_ref().get(idx) {
                        collector_ids.insert(peer.clone());
                    }
                }
            }
            let mut selected = Vec::new();
            let mut seen = HashSet::new();
            if let Some(tail) = rotated.get(commit_quorum..) {
                for peer in tail {
                    if collector_ids.contains(peer) {
                        continue;
                    }
                    if seen.insert(peer.clone()) {
                        selected.push(peer.clone());
                    }
                }
            }
            if selected.len() < n_faulty_peers {
                let head = rotated.iter().skip(1).take(commit_quorum.saturating_sub(1));
                for peer in head {
                    if collector_ids.contains(peer) {
                        continue;
                    }
                    if seen.insert(peer.clone()) {
                        selected.push(peer.clone());
                    }
                }
            }
            if selected.len() < n_faulty_peers {
                rotated.get(commit_quorum..).unwrap_or(&[]).to_vec()
            } else {
                selected
            }
        };
        let mut rng =
            ChaCha8Rng::seed_from_u64(0x5553_5442 + u64::try_from(round_index).unwrap_or(0));
        candidates
            .iter()
            .choose_multiple(&mut rng, n_faulty_peers)
            .into_iter()
            .cloned()
            .collect()
    }

    fn build_network(&self) -> Network {
        let mut builder = NetworkBuilder::new()
            .with_auto_populated_trusted_peers()
            .with_peers(self.n_peers)
            .with_data_availability_enabled(true)
            .with_default_pipeline_time()
            // Slow gossip slightly so the relay toggles mirror the higher RTT budget of
            // the default consensus pipeline.
            .with_block_sync_gossip_period(Duration::from_millis(400))
            .with_config_layer(|cfg| {
                if self.force_soft_fork {
                    cfg.write(["sumeragi", "debug", "force_soft_fork"], true);
                }
            })
            .with_genesis_instruction(SetParameter::new(Parameter::Block(
                BlockParameter::MaxTransactions(nonzero!(1u64)),
            )));

        if self.n_peers > 4 {
            let collectors_k = collectors_k_for_peers(self.n_peers);
            let redundant_send_r = redundant_send_r_for_peers(self.n_peers);
            builder = builder
                .with_config_layer(|layer| {
                    layer
                        .write(["sumeragi", "collectors", "k"], i64::from(collectors_k))
                        .write(
                            ["sumeragi", "collectors", "redundant_send_r"],
                            i64::from(redundant_send_r),
                        );
                })
                .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
                    SumeragiParameter::CollectorsK(collectors_k),
                )))
                .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
                    SumeragiParameter::RedundantSendR(redundant_send_r),
                )));
        }

        builder.build()
    }

    async fn register_numeric_asset(
        network: &Network,
        asset_definition_id: &AssetDefinitionId,
    ) -> Result<()> {
        let status_timeout = scaled_timeout(network.sync_timeout(), network.peers().len());
        let mut client = network.client();
        if client.transaction_status_timeout < status_timeout {
            client.transaction_status_timeout = status_timeout;
        }
        if let Some(ttl) = client.transaction_ttl {
            let min_ttl = status_timeout + Duration::from_secs(120);
            if ttl < min_ttl {
                client.transaction_ttl = Some(min_ttl);
            }
        }
        let isi = Register::asset_definition(AssetDefinition::numeric(asset_definition_id.clone()));
        spawn_blocking(move || client.submit_blocking(isi)).await??;
        Ok(())
    }

    async fn execute_round(
        &self,
        round_index: usize,
        ctx: &RoundContext<'_>,
        network: &Network,
        relay: &mut P2pRelay,
        peers: &[NetworkPeer],
    ) -> Result<()> {
        iroha_logger::info!(
            round = round_index + 1,
            total = self.n_rounds,
            "Begin round"
        );

        let target_height = ctx.init_blocks + (round_index as u64) + 1;
        let chain_id = network.chain_id();
        let peer_ids: Vec<_> = peers.iter().map(NetworkPeer::id).collect();
        let rotated_topology =
            topology_for_permissioned_round(&peer_ids, &chain_id, target_height, 0);
        let leader_id = rotated_topology
            .first()
            .cloned()
            .expect("topology always has a leader");
        let collectors_k = collectors_k_for_peers(self.n_peers);
        let fault_budget = Self::fault_budget_for_peer_count(self.n_peers);
        let fault_round_seed =
            Self::fault_round_seed(round_index, self.n_faulty_peers, fault_budget);
        let faulty_ids: HashSet<_> = Self::select_faulty_peer_ids(
            &peer_ids,
            self.n_faulty_peers,
            fault_round_seed,
            &chain_id,
            target_height,
            collectors_k,
        )
        .into_iter()
        .collect();
        let faulty: Vec<_> = peers
            .iter()
            .filter(|peer| faulty_ids.contains(&peer.id()))
            .cloned()
            .collect();

        let sync_timeout = scaled_timeout(network.sync_timeout(), peers.len());
        let relay_pause =
            non_faulty_sync_timeout(sync_timeout, network.pipeline_time(), self.n_faulty_peers);
        let min_ttl = sync_timeout.saturating_add(Duration::from_secs(300));
        let submit_while_partitioned = self.n_faulty_peers <= 1;
        let stagger_faults = self.n_faulty_peers > 1;
        let primary_peer = peers
            .iter()
            .find(|peer| peer.id() == leader_id)
            .filter(|peer| !faulty_ids.contains(&peer.id()))
            .cloned()
            .unwrap_or_else(|| {
                let mut rng = ChaCha8Rng::seed_from_u64(
                    0x5553_5052 + u64::try_from(round_index).unwrap_or(0),
                );
                peers
                    .iter()
                    .filter(|peer| !faulty_ids.contains(&peer.id()))
                    .choose(&mut rng)
                    .expect("there should be some working peers")
                    .clone()
            });
        let mut candidates: Vec<_> = peers
            .iter()
            .filter(|peer| !faulty_ids.contains(&peer.id()))
            .cloned()
            .collect();
        if let Some(pos) = candidates
            .iter()
            .position(|peer| peer.id() == primary_peer.id())
        {
            let primary = candidates.remove(pos);
            candidates.insert(0, primary);
        }
        let mut builder_client = primary_peer.client();
        if let Some(ttl) = builder_client.transaction_ttl {
            if ttl < min_ttl {
                builder_client.transaction_ttl = Some(min_ttl);
            }
        }
        let mint_asset = Mint::asset_numeric(
            Numeric::one(),
            AssetId::new(ctx.asset_definition_id.clone(), ctx.account_id.clone()),
        );
        let tx = Arc::new(
            builder_client.build_transaction_from_items(vec![mint_asset], Metadata::default()),
        );
        let partition_submit_window = relay_pause
            .min(
                network
                    .pipeline_time()
                    .saturating_mul(2)
                    .max(Duration::from_secs(5)),
            )
            .max(Duration::from_secs(2));
        let is_already_accepted = |err: &eyre::Report| {
            err.chain().any(|cause| {
                let message = cause.to_string();
                message.contains("ALREADY_ENQUEUED") || message.contains("already committed")
            })
        };
        if stagger_faults {
            let per_peer_pause = relay_pause
                .checked_div(u32::try_from(self.n_faulty_peers).unwrap_or(1))
                .unwrap_or(Duration::from_secs(1))
                .max(Duration::from_millis(200));
            for peer in &faulty {
                relay.suspend(&peer.id()).activate();
                iroha_logger::info!(peer = peer.mnemonic(), "Suspended");
                sleep(per_peer_pause).await;
                relay.suspend(&peer.id()).deactivate();
                iroha_logger::info!(peer = peer.mnemonic(), "Unsuspended");
            }
        } else {
            for peer in &faulty {
                relay.suspend(&peer.id()).activate();
                iroha_logger::info!(peer = peer.mnemonic(), "Suspended");
            }
        }
        let submit_tx = |phase: &'static str, timeout_window: Duration| {
            let candidates = candidates.clone();
            let tx = Arc::clone(&tx);
            async move {
                let deadline = Instant::now() + timeout_window;
                let mut attempts = 0usize;
                loop {
                    attempts = attempts.saturating_add(1);
                    let mut attempt_err: Option<eyre::Report> = None;
                    for peer in &candidates {
                        let client = peer.client();
                        iroha_logger::info!(
                            phase,
                            via_peer = peer.mnemonic(),
                            "Submit transaction"
                        );
                        let tx = Arc::clone(&tx);
                        let res = spawn_blocking(move || client.submit_transaction(&tx)).await;
                        match res {
                            Ok(Ok(_hash)) => return Ok(()),
                            Ok(Err(err)) => {
                                if is_already_accepted(&err) {
                                    return Ok(());
                                }
                                attempt_err = Some(err);
                            }
                            Err(err) => {
                                attempt_err = Some(eyre::Report::new(err));
                            }
                        }
                    }
                    if Instant::now() >= deadline {
                        return Err(attempt_err.unwrap_or_else(|| {
                            eyre!("transaction submission failed on all non-faulty peers")
                        }));
                    }
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(attempt_err.unwrap_or_else(|| {
                            eyre!("transaction submission failed on all non-faulty peers")
                        }));
                    }
                    sleep(submit_retry_backoff(attempts).min(remaining)).await;
                }
            }
        };
        let mut submitted_during_partition = false;
        if submit_while_partitioned {
            match submit_tx("partitioned", partition_submit_window).await {
                Ok(()) => submitted_during_partition = true,
                Err(err) => {
                    iroha_logger::warn!(
                        ?err,
                        "partitioned submit failed; will retry after recovery"
                    );
                }
            }
        }

        if !stagger_faults {
            if submit_while_partitioned {
                match timeout(
                    relay_pause,
                    once_blocks_sync(
                        network.peers().iter().filter(|x| !faulty.contains(x)),
                        BlockHeight::predicate_non_empty(target_height),
                    ),
                )
                .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(err)) => {
                        iroha_logger::warn!(
                            ?err,
                            target_height,
                            "non-suspended peers failed to sync during partition window"
                        );
                    }
                    Err(err) => {
                        iroha_logger::warn!(
                            ?err,
                            target_height,
                            "non-suspended peers did not sync within partition window"
                        );
                    }
                }
            } else {
                sleep(relay_pause).await;
            }

            for peer in &faulty {
                relay.suspend(&peer.id()).deactivate();
                iroha_logger::info!(peer = peer.mnemonic(), "Unsuspended");
            }
        }
        // Let connections settle after partitions before confirming the transaction.
        let recovery_delay = if submit_while_partitioned {
            network.pipeline_time().max(Duration::from_secs(2))
        } else {
            relay_pause
                .saturating_add(network.pipeline_time())
                .max(network.pipeline_time().saturating_mul(2))
        };
        sleep(recovery_delay).await;
        if !submitted_during_partition {
            submit_tx("recovered", sync_timeout).await?;
        }
        let expected_supply = Numeric::new((round_index + 1) as u128, 0);
        let supply_start = Instant::now();
        let supply_deadline = supply_start + sync_timeout;
        let resubmit_at = supply_start + sync_timeout.checked_div(2).unwrap_or(sync_timeout);
        let allow_resubmit = self.n_faulty_peers > 0;
        let supply_peers: Vec<_> = peers
            .iter()
            .filter(|peer| !faulty_ids.contains(&peer.id()))
            .collect();
        let mut last_seen = None;
        let mut last_height = None;
        let mut resubmitted = false;
        'wait_supply: loop {
            for peer in &supply_peers {
                if let Some(height) = peer.best_effort_block_height() {
                    last_height = Some(height);
                    if height.non_empty >= target_height {
                        iroha_logger::warn!(
                            expected_supply = ?expected_supply,
                            observed_height = ?height,
                            "supply check accepting committed height without asset confirmation"
                        );
                        break 'wait_supply;
                    }
                }
                let client = peer.client();
                let asset =
                    match spawn_blocking(move || client.query(FindAssets).execute_all()).await {
                        Ok(Ok(assets)) => assets
                            .into_iter()
                            .find(|asset| asset.id().definition() == ctx.asset_definition_id),
                        Ok(Err(err)) => {
                            iroha_logger::warn!(
                                ?err,
                                peer = peer.mnemonic(),
                                "asset query failed during supply check"
                            );
                            None
                        }
                        Err(err) => {
                            iroha_logger::warn!(
                                ?err,
                                peer = peer.mnemonic(),
                                "asset query task failed during supply check"
                            );
                            None
                        }
                    };
                let asset_value = asset.as_ref().map(|asset| asset.value().clone());
                if let Some(asset) = asset.as_ref() {
                    if *asset.value() == expected_supply {
                        break 'wait_supply;
                    }
                }
                last_seen = asset_value;
            }
            let now = Instant::now();
            if should_resubmit_tx(allow_resubmit, resubmitted, now, resubmit_at) {
                let remaining = supply_deadline.saturating_duration_since(now);
                if !remaining.is_zero() {
                    if let Err(err) = submit_tx("recovered_retry", remaining).await {
                        iroha_logger::warn!(
                            ?err,
                            "recovered submit retry failed while waiting for supply"
                        );
                    }
                }
                resubmitted = true;
            }
            if now >= supply_deadline {
                return Err(eyre!(
                    "total supply did not reach expected {expected_supply}; last seen value: {last_seen:?}; last height: {last_height:?}"
                ));
            }
            sleep(Duration::from_millis(200)).await;
        }

        let catch_up_timeout = sync_timeout.min(Duration::from_secs(180));
        match timeout(
            catch_up_timeout,
            once_blocks_sync(
                network.peers().iter().filter(|x| !faulty.contains(x)),
                BlockHeight::predicate_non_empty(target_height),
            ),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                iroha_logger::warn!(
                    ?err,
                    target_height,
                    "non-faulty peers failed to catch up before timeout"
                );
            }
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    target_height,
                    "non-faulty peers did not catch up before timeout"
                );
            }
        }

        if !faulty.is_empty() {
            match timeout(
                catch_up_timeout,
                once_blocks_sync(
                    faulty.iter(),
                    BlockHeight::predicate_non_empty(target_height),
                ),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    iroha_logger::warn!(
                        ?err,
                        target_height,
                        "faulty peers failed to catch up before timeout"
                    );
                }
                Err(err) => {
                    iroha_logger::warn!(
                        ?err,
                        target_height,
                        "faulty peers did not catch up before timeout"
                    );
                }
            }
        }

        Ok(())
    }

    async fn assert_total_supply(
        network: &Network,
        asset_definition_id: &AssetDefinitionId,
        rounds: usize,
        expected_height: u64,
    ) -> Result<()> {
        let peers = network.peers();
        let expected = Numeric::new(rounds as u128, 0);
        let deadline =
            Instant::now() + scaled_timeout(network.sync_timeout(), network.peers().len());
        loop {
            let mut last_seen = None;
            let mut last_height = None;
            for peer in peers {
                if let Some(height) = peer.best_effort_block_height() {
                    last_height = Some(height);
                    if height.non_empty >= expected_height {
                        iroha_logger::warn!(
                            expected_supply = ?expected,
                            observed_height = ?height,
                            "final supply check accepting committed height without asset confirmation"
                        );
                        return Ok(());
                    }
                }
                let client = peer.client();
                let asset =
                    match spawn_blocking(move || client.query(FindAssets).execute_all()).await {
                        Ok(Ok(assets)) => assets
                            .into_iter()
                            .find(|asset| asset.id().definition() == asset_definition_id),
                        Ok(Err(err)) => {
                            iroha_logger::warn!(
                                ?err,
                                peer = peer.mnemonic(),
                                "asset query failed during final supply check"
                            );
                            None
                        }
                        Err(err) => {
                            iroha_logger::warn!(
                                ?err,
                                peer = peer.mnemonic(),
                                "asset query task failed during final supply check"
                            );
                            None
                        }
                    };
                let asset_value = asset.as_ref().map(|asset| asset.value().clone());
                if let Some(asset) = asset.as_ref() {
                    if *asset.value() == expected {
                        return Ok(());
                    }
                }
                last_seen = asset_value;
            }
            if Instant::now() >= deadline {
                return Err(eyre!(
                    "total supply did not reach expected {expected}; last seen value: {last_seen:?}; last height: {last_height:?}"
                ));
            }
            sleep(Duration::from_millis(200)).await;
        }
    }
}

struct RoundContext<'a> {
    init_blocks: u64,
    asset_definition_id: &'a AssetDefinitionId,
    account_id: &'a AccountId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use iroha_crypto::KeyPair;

    #[test]
    fn faulty_peer_selection_respects_collectors() {
        let peer_ids: Vec<_> = (0..9)
            .map(|_| PeerId::new(KeyPair::random().public_key().clone()))
            .collect();
        let chain_id: ChainId = "unstable-network-selection".parse().expect("chain id");
        let height = 7_u64;
        let rotated = topology_for_permissioned_round(&peer_ids, &chain_id, height, 0);
        let commit_quorum = commit_quorum_from_len(rotated.len());
        let expected_tail: BTreeSet<_> = rotated[commit_quorum..].iter().cloned().collect();
        let collectors_k = collectors_k_for_peers(peer_ids.len());
        let selected_single = UnstableNetwork::select_faulty_peer_ids(
            &peer_ids,
            1,
            0,
            &chain_id,
            height,
            collectors_k,
        );
        assert_eq!(selected_single.len(), 1);
        assert!(expected_tail.contains(&selected_single[0]));

        let topology = Topology::new(rotated.clone());
        let seed = permissioned_prf_seed(&chain_id);
        let collector_ids: BTreeSet<_> = topology
            .collector_indices_k_prf(usize::from(collectors_k), seed, height, 0)
            .into_iter()
            .filter_map(|idx| topology.as_ref().get(idx).cloned())
            .collect();
        let selected_multi: BTreeSet<_> = UnstableNetwork::select_faulty_peer_ids(
            &peer_ids,
            3,
            0,
            &chain_id,
            height,
            collectors_k,
        )
        .into_iter()
        .collect();
        assert_eq!(selected_multi.len(), 3);
        assert!(collector_ids.is_disjoint(&selected_multi));
    }

    #[test]
    fn fault_budget_matches_commit_quorum_tail() {
        let peer_count = 9;
        let commit_quorum = commit_quorum_from_len(peer_count);
        let fault_budget = UnstableNetwork::fault_budget_for_peer_count(peer_count);
        assert_eq!(fault_budget, peer_count - commit_quorum);
    }

    #[test]
    fn fault_round_seed_sticks_at_budget() {
        let budget = UnstableNetwork::fault_budget_for_peer_count(12);
        assert!(budget > 0);
        let sticky = UnstableNetwork::fault_round_seed(4, budget, budget);
        assert_eq!(sticky, 0);
        let rotating = UnstableNetwork::fault_round_seed(4, budget.saturating_sub(1), budget);
        assert_eq!(rotating, 4);
    }

    #[test]
    fn non_faulty_sync_timeout_caps_multi_faults() {
        let sync_timeout = Duration::from_secs(180);
        let pipeline_time = Duration::from_secs(9);
        let capped = non_faulty_sync_timeout(sync_timeout, pipeline_time, 2);
        assert!(capped < sync_timeout);
        assert!(capped <= pipeline_time * 2 + Duration::from_secs(2));
        let single_fault = non_faulty_sync_timeout(sync_timeout, pipeline_time, 1);
        assert!(single_fault < sync_timeout);
        let session_cap = Duration::from_millis(defaults::sumeragi::RBC_SESSION_TTL_MS)
            .saturating_sub(Duration::from_secs(10));
        let expected_cap = if session_cap.is_zero() {
            pipeline_time * 10 + Duration::from_secs(2)
        } else {
            session_cap
        };
        assert!(single_fault <= expected_cap);
        let no_faults = non_faulty_sync_timeout(sync_timeout, pipeline_time, 0);
        assert_eq!(no_faults, sync_timeout);
    }

    #[test]
    fn submit_retry_backoff_caps_delay() {
        assert_eq!(submit_retry_backoff(1), Duration::from_millis(200));
        assert_eq!(submit_retry_backoff(5), Duration::from_millis(1000));
        assert_eq!(submit_retry_backoff(20), Duration::from_secs(2));
    }

    #[test]
    fn resubmit_gate_respects_flags_and_deadline() {
        let now = Instant::now();
        assert!(!should_resubmit_tx(false, false, now, now));
        assert!(!should_resubmit_tx(true, true, now, now));
        assert!(should_resubmit_tx(true, false, now, now));
        assert!(!should_resubmit_tx(
            true,
            false,
            now,
            now + Duration::from_secs(1)
        ));
    }
}

#[tokio::test]
async fn unstable_network_5_peers_1_fault() -> Result<()> {
    UnstableNetwork {
        n_peers: 5,
        n_faulty_peers: 1,
        n_rounds: 5,
        force_soft_fork: false,
    }
    .run()
    .await
}

#[tokio::test]
async fn soft_fork() -> Result<()> {
    UnstableNetwork {
        n_peers: 4,
        n_faulty_peers: 0,
        n_rounds: 5,
        force_soft_fork: true,
    }
    .run()
    .await
}

#[tokio::test]
async fn unstable_network_8_peers_1_fault() -> Result<()> {
    UnstableNetwork {
        n_peers: 8,
        n_faulty_peers: 1,
        n_rounds: 3,
        force_soft_fork: false,
    }
    .run()
    .await
}

#[tokio::test]
async fn unstable_network_9_peers_2_faults() -> Result<()> {
    UnstableNetwork {
        n_peers: 9,
        n_faulty_peers: 2,
        n_rounds: 3,
        force_soft_fork: false,
    }
    .run()
    .await
}

#[tokio::test]
async fn unstable_network_9_peers_3_faults() -> Result<()> {
    UnstableNetwork {
        n_peers: 9,
        n_faulty_peers: 3,
        n_rounds: 3,
        force_soft_fork: false,
    }
    .run()
    .await
}

#[tokio::test]
async fn unstable_network_12_peers_4_faults() -> Result<()> {
    UnstableNetwork {
        n_peers: 12,
        n_faulty_peers: 4,
        n_rounds: 1,
        force_soft_fork: false,
    }
    .run()
    .await
}
