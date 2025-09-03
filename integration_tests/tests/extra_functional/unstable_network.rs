use std::{borrow::Cow, time::Duration};

use eyre::{Context, Result};
use futures_util::{stream::FuturesUnordered, StreamExt};
use iroha_config_base::toml::WriteExt;
use iroha_data_model::{
    asset::AssetDefinition, isi::Register, parameter::BlockParameter, prelude::*, Level,
};
use iroha_primitives::addr::socket_addr;
use iroha_test_network::{once_blocks_sync, BlockHeight, Network, NetworkBuilder};
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;
use rand::{
    prelude::{IteratorRandom, SliceRandom},
    thread_rng,
};
use relay::P2pRelay;
use tokio::{self, task::spawn_blocking, time::timeout};
use toml::Table;

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
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };

    use futures_util::{stream::FuturesUnordered, StreamExt};
    use iroha_data_model::{peer::PeerId, prelude::Peer};
    use iroha_primitives::{
        addr::{socket_addr, SocketAddr},
        unique_vec::UniqueVec,
    };
    use iroha_test_network::{fslock_ports::AllocatedPort, Network};
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
            table.set_max_width(30 * (1 + real_topology.len()));
            table.column(0).set_header("From");
            for (i, id) in real_topology.iter().enumerate() {
                table
                    .column(i + 1)
                    .set_header(format!("To {}", id.address()));
            }
            let table_formatted = table.format(real_topology.iter().map(|peer| {
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
            }));

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
                let server = TcpStream::connect(self.to.to_string()).await?;

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

async fn start_network_under_relay(network: &Network, relay: &P2pRelay) -> Result<()> {
    let genesis = network.genesis();

    timeout(
        network.peer_startup_timeout(),
        network
            .peers()
            .iter()
            .map(|peer| {
                let config = network.config_layers().chain(Some(Cow::Owned(
                    Table::new()
                        .write(["trusted_peers"], relay.trusted_peers_for(&peer.id()))
                        // We don't want peers to gossip any actual addresses, because each peer has
                        // its own set of incoming and outgoing proxies with every other peer.
                        // Thus, we are giving this addr which should always reject connections and
                        // peers should rely on what they got in the `sumeragi.trusted_peers`.
                        .write(
                            ["network", "public_address"],
                            // This IP is in the range of IPs reserved for "documentation and examples"
                            // https://en.wikipedia.org/wiki/Reserved_IP_addresses#:~:text=192.0.2.0/24
                            socket_addr!(192.0.2.133:1337),
                        ),
                )));

                async move {
                    peer.start_checked(config, genesis)
                        .await
                        .expect("must start");
                }
            })
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>(),
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn network_starts_with_relay() -> Result<()> {
    let network = NetworkBuilder::new().with_peers(4).build();
    let mut relay = P2pRelay::for_network(&network);

    relay.start();
    start_network_under_relay(&network, &relay).await?;
    network.ensure_blocks(1).await?;

    Ok(())
}

#[tokio::test]
#[ignore = "Since #5481, the genesis block has been committed without communication. This test should target blocks after the genesis block."]
async fn network_doesnt_start_without_relay_being_started() -> Result<()> {
    let network = NetworkBuilder::new().with_peers(4).build();
    let relay = P2pRelay::for_network(&network);

    start_network_under_relay(&network, &relay).await?;

    let Err(_) = timeout(
        Duration::from_secs(3),
        once_blocks_sync(network.peers().iter(), BlockHeight::predicate_non_empty(1)),
    )
    .await
    else {
        panic!("network must not start!")
    };

    Ok(())
}

#[tokio::test]
#[ignore = "Since #5481, the genesis block has been committed without communication. This test should target blocks after the genesis block."]
async fn suspending_works() -> Result<()> {
    const SYNC: Duration = Duration::from_secs(3);
    const N_PEERS: usize = 4;
    const { assert!(N_PEERS > 0) };

    let network = NetworkBuilder::new().with_peers(N_PEERS).build();
    let mut relay = P2pRelay::for_network(&network);
    start_network_under_relay(&network, &relay).await?;
    // we will plug/unplug the last peer who doesn't have the genesis
    let last_peer = network
        .peers()
        .last()
        .expect("there are more than 0 of them");
    let suspend = relay.suspend(&last_peer.id());

    suspend.activate();
    relay.start();

    // all peers except the last one should get the genesis
    timeout(
        SYNC,
        once_blocks_sync(
            network.peers().iter().take(N_PEERS - 1),
            BlockHeight::predicate_non_empty(1),
        ),
    )
    .await??;
    let Err(_) = timeout(SYNC, last_peer.once_block(1)).await else {
        panic!("should not get block within timeout!")
    };

    // unsuspend, the last peer should get the block too
    suspend.deactivate();
    timeout(SYNC, last_peer.once_block(1)).await?;

    Ok(())
}

#[tokio::test]
#[ignore = "Since #5481, the genesis block has been committed without communication. This test should target blocks after the genesis block."]
async fn block_after_genesis_is_synced() -> Result<()> {
    // A consensus anomaly occurred deterministically depending on the peer set
    // (how public keys of different peers are sorted to each other, which determines consensus
    // roles) and which peer submits the genesis. The values below are an experimentally found
    // case.

    const SEED: &str = "we want a determined order of peers";

    let network = NetworkBuilder::new()
        .with_base_seed(SEED)
        .with_peers(5)
        .with_pipeline_time(Duration::from_secs(6))
        .build();
    let mut relay = P2pRelay::for_network(&network);

    relay.start();
    start_network_under_relay(&network, &relay).await?;
    network.ensure_blocks(1).await?;

    for peer in network.peers() {
        relay.suspend(&peer.id()).activate();
    }
    let client = network.client();
    spawn_blocking(move || client.submit(Log::new(Level::INFO, "tick".to_owned()))).await??;
    let Err(_) = timeout(
        Duration::from_secs(3),
        once_blocks_sync(network.peers().iter(), BlockHeight::predicate_non_empty(2)),
    )
    .await
    else {
        panic!("should not sync with relay being suspended")
    };
    for peer in network.peers() {
        relay.suspend(&peer.id()).deactivate();
    }
    timeout(
        Duration::from_secs(2),
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
        assert!(self.n_peers > self.n_faulty_peers);

        let account_id = ALICE_ID.clone();
        let asset_definition_id: AssetDefinitionId = "camomile#wonderland".parse().expect("Valid");

        let network = NetworkBuilder::new()
            .with_peers(self.n_peers)
            .with_config_layer(|cfg| {
                if self.force_soft_fork {
                    cfg.write(["sumeragi", "debug", "force_soft_fork"], true);
                }
            })
            .with_genesis_instruction(SetParameter::new(Parameter::Block(
                BlockParameter::MaxTransactions(nonzero!(1u64)),
            )))
            .build();
        let mut relay = P2pRelay::for_network(&network);
        relay.start();

        start_network_under_relay(&network, &relay).await?;

        {
            let client = network.client();
            let isi =
                Register::asset_definition(AssetDefinition::numeric(asset_definition_id.clone()));
            spawn_blocking(move || client.submit_blocking(isi)).await??;
        }
        let init_blocks = 2;
        network.ensure_blocks(init_blocks).await?;

        for i in 0..self.n_rounds {
            iroha_logger::info!(round = i + 1, total = self.n_rounds, "Begin round");
            // Make random peers faulty.
            let faulty: Vec<_> = network
                .peers()
                .choose_multiple(&mut thread_rng(), self.n_faulty_peers)
                .collect();
            for peer in &faulty {
                relay.suspend(&peer.id()).activate();
                iroha_logger::info!(peer = peer.mnemonic(), "Suspended");
            }

            // When minted
            let quantity = Numeric::ONE;
            let mint_asset = Mint::asset_numeric(
                quantity,
                AssetId::new(asset_definition_id.clone(), account_id.clone()),
            );
            let some_peer = network
                .peers()
                .iter()
                .filter(|x| !faulty.contains(x))
                .choose(&mut thread_rng())
                .expect("there should be some working peers");
            iroha_logger::info!(via_peer = some_peer.mnemonic(), "Submit transaction");
            let client = some_peer.client();
            spawn_blocking(move || client.submit(mint_asset)).await??;

            // Then all non-faulty peers get the new block
            timeout(
                network.sync_timeout(),
                once_blocks_sync(
                    network.peers().iter().filter(|x| !faulty.contains(x)),
                    BlockHeight::predicate_non_empty(init_blocks + (i as u64) + 1),
                ),
            )
            .await
            .wrap_err("Non-suspended peers must sync within timeout")??;

            // Return all peers to normal function.
            for peer in &faulty {
                relay.suspend(&peer.id()).deactivate();
                iroha_logger::info!(peer = peer.mnemonic(), "Unsuspended");
            }

            // await for sync so that we can start the next round
            network
                .ensure_blocks_with(BlockHeight::predicate_non_empty(
                    init_blocks + (i as u64) + 1,
                ))
                .await
                .wrap_err("Faulty peers musth catch up")?;
        }

        // Then there are N assets minted
        let client = network.client();
        let asset = spawn_blocking(move || {
            client
                .query(FindAssets)
                .filter_with(|asset| asset.id.definition.eq(asset_definition_id))
                .execute_all()
        })
        .await??
        .into_iter()
        .next()
        .expect("there should be 1 result");
        assert_eq!(*asset.value(), Numeric::new(self.n_rounds as u128, 0));

        Ok(())
    }
}

// FIXME: The tests below don't pass! Potentially, Iroha has networking/consensus issues. Needs
// deeper investigation.
// Possible issue: leader becomes faulty, larger number of peers hides the problem?
// For this moment, increased number of retries will do.

#[tokio::test]
async fn unstable_network_5_peers_1_fault() -> Result<()> {
    UnstableNetwork {
        n_peers: 5,
        n_faulty_peers: 1,
        n_rounds: 10,
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
        n_rounds: 20,
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
        n_rounds: 20,
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
        n_rounds: 5,
        force_soft_fork: false,
    }
    .run()
    .await
}

#[tokio::test]
#[ignore]
async fn unstable_network_9_peers_4_faults() -> Result<()> {
    UnstableNetwork {
        n_peers: 9,
        n_faulty_peers: 4,
        n_rounds: 5,
        force_soft_fork: false,
    }
    .run()
    .await
}
