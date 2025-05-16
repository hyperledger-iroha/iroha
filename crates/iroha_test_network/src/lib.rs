//! Puppeteer for `irohad`, to create test networks

mod config;
mod fslock_ports;

use core::{fmt::Debug, time::Duration};
use std::{
    num::NonZero,
    ops::Deref,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, OnceLock,
    },
};

use backoff::ExponentialBackoffBuilder;
use color_eyre::eyre::{eyre, Context, Result};
use fslock_ports::AllocatedPort;
use futures::{prelude::*, stream::FuturesUnordered};
use iroha::{client::Client, data_model::prelude::*};
use iroha_config::base::{
    read::ConfigReader,
    toml::{TomlSource, WriteExt as _, Writer as TomlWriter},
};
use iroha_crypto::{ExposedPrivateKey, KeyPair, PrivateKey};
use iroha_data_model::{
    isi::InstructionBox,
    parameter::{SumeragiParameter, SumeragiParameters},
    ChainId,
};
use iroha_genesis::GenesisBlock;
use iroha_primitives::{addr::socket_addr, unique_vec::UniqueVec};
use iroha_telemetry::metrics::Status;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, PEER_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
use parity_scale_codec::Encode;
use rand::{prelude::IteratorRandom, thread_rng};
use tempfile::TempDir;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Child,
    runtime::{self, Runtime},
    sync::{broadcast, oneshot, watch, Mutex},
    task::{spawn_blocking, JoinSet},
    time::timeout,
};
use toml::Table;
use tracing::{error, info, info_span, warn, Instrument};

const INSTANT_PIPELINE_TIME: Duration = Duration::from_millis(500);
const DEFAULT_BLOCK_SYNC: Duration = Duration::from_millis(150);
const PEER_START_TIMEOUT: Duration = Duration::from_secs(30);
const PEER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const SYNC_TIMEOUT: Duration = Duration::from_secs(30);

const IROHAD_BIN_ENV: &str = "TEST_NETWORK_IROHAD";
const IROHAD_DEFAULT: &str = "target/release/irohad";
const IROHAD_DEFAULT_BUILD: &str = "cargo build --release --bin irohad";

fn iroha_bin() -> impl AsRef<Path> {
    static PATH: OnceLock<PathBuf> = OnceLock::new();

    PATH.get_or_init(|| {
        let path = std::env::var(IROHAD_BIN_ENV)
            .map(PathBuf::from)
            .map(|path| {
                if path.is_relative() {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../")
                        .join(path)
                } else { path }
            })
            .unwrap_or_else(|_err| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../").join(IROHAD_DEFAULT)
            });

        match path.canonicalize() {
            Ok(path) => path,
            Err(err) => {
                eprintln!(
                    "FATAL ERROR: `irohad` path does not exist: {err}\n  \
                        Path: {}\n  \
                        It is necessary in order to run `iroha_test_network`. Solutions:\n  \
                        1. Run `{IROHAD_DEFAULT_BUILD}`, and `{IROHAD_DEFAULT}` will be used by default\n  \
                        2. Override path via {IROHAD_BIN_ENV} env (relative paths are resolved relative to the project root)",
                    path.display()
                );
                panic!("could not proceed without `irohad`, see the message above");
            }
        }
    })
}

const TEMPDIR_PREFIX: &str = "irohad_test_network_";
const TEMPDIR_IN_ENV: &str = "TEST_NETWORK_TMP_DIR";

fn tempdir_in() -> Option<impl AsRef<Path>> {
    static ENV: OnceLock<Option<PathBuf>> = OnceLock::new();

    ENV.get_or_init(|| std::env::var(TEMPDIR_IN_ENV).map(PathBuf::from).ok())
        .as_ref()
}

fn init_logger() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    static ONCE: OnceLock<()> = OnceLock::new();

    ONCE.get_or_init(|| {
        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::from("debug"))
            .with(
                tracing_subscriber::fmt::layer()
                    .pretty()
                    .with_timer(tracing_subscriber::fmt::time::uptime()),
            )
            .init();
    });
}

/// Network of peers
pub struct Network {
    peers: Vec<NetworkPeer>,

    genesis: GenesisBlock,
    block_time: Duration,
    commit_time: Duration,

    config: Table,
}

impl Network {
    /// Add a peer to the network.
    pub fn add_peer(&mut self, peer: &NetworkPeer) {
        self.peers.push(peer.clone());
    }

    /// Remove a peer from the network.
    pub fn remove_peer(&mut self, peer: &NetworkPeer) {
        self.peers.retain(|x| x != peer);
    }

    /// Access network peers
    pub fn peers(&self) -> &Vec<NetworkPeer> {
        &self.peers
    }

    /// Get a random peer in the network
    pub fn peer(&self) -> &NetworkPeer {
        self.peers
            .iter()
            .choose(&mut thread_rng())
            .expect("there is at least one peer")
    }

    /// Start all peers, waiting until they are up and have committed genesis (submitted by one of them).
    ///
    /// # Panics
    /// - If some peer was already started
    /// - If some peer exists early
    pub async fn start_all(&self) -> &Self {
        timeout(
            PEER_START_TIMEOUT,
            self.peers
                .iter()
                .enumerate()
                .map(|(i, peer)| async move {
                    let failure = async move {
                        peer.once(|e| matches!(e, PeerLifecycleEvent::Terminated { .. }))
                            .await;
                        panic!("a peer exited unexpectedly");
                    };

                    let start = async move {
                        peer.start(self.config(), (i == 0).then_some(&self.genesis))
                            .await;
                        peer.once_block(1).await;
                    };

                    tokio::select! {
                        _ = failure => {},
                        _ = start => {},
                    }
                })
                .collect::<FuturesUnordered<_>>()
                .collect::<Vec<_>>(),
        )
        .await
        .expect("expected peers to start within timeout");
        self
    }

    /// Pipeline time of the network.
    ///
    /// Is relevant only if users haven't submitted [`SumeragiParameter`] changing it.
    /// Users should do it through a network method (which hasn't been necessary yet).
    pub fn pipeline_time(&self) -> Duration {
        self.block_time + self.commit_time
    }

    pub fn sync_timeout(&self) -> Duration {
        SYNC_TIMEOUT
    }

    pub fn peer_startup_timeout(&self) -> Duration {
        PEER_START_TIMEOUT
    }

    /// Get a client for a random peer in the network
    pub fn client(&self) -> Client {
        self.peer().client()
    }

    /// Chain ID of the network
    pub fn chain_id(&self) -> ChainId {
        config::chain_id()
    }

    /// Base configuration of all peers.
    ///
    /// Includes `trusted_peers` parameter, containing all currently present peers.
    pub fn config(&self) -> Table {
        self.config
            .clone()
            .write(["trusted_peers"], self.topology())
    }

    /// Network genesis block.
    pub fn genesis(&self) -> &GenesisBlock {
        &self.genesis
    }

    /// Shutdown running peers
    pub async fn shutdown(&self) -> &Self {
        self.peers
            .iter()
            .filter(|peer| peer.is_running())
            .map(|peer| peer.shutdown())
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>()
            .await;
        self
    }

    fn topology(&self) -> UniqueVec<Peer> {
        self.peers.iter().map(|x| x.id.clone()).collect()
    }

    /// Resolves when all _running_ peers have at least N non-empty blocks
    /// # Errors
    /// If this doesn't happen within a timeout.
    pub async fn ensure_blocks(&self, height: u64) -> Result<&Self> {
        self.ensure_blocks_with(|block_height| block_height.non_empty >= height)
            .await
            .wrap_err_with(|| eyre!("expected to reach height={height}"))?;

        info!(%height, "network sync height");

        Ok(self)
    }

    pub async fn ensure_blocks_with<F: Fn(BlockHeight) -> bool>(&self, f: F) -> Result<&Self> {
        timeout(
            self.sync_timeout(),
            self.peers
                .iter()
                .filter(|x| x.is_running())
                .map(|x| x.once_block_with(&f))
                .collect::<FuturesUnordered<_>>()
                .collect::<Vec<_>>(),
        )
        .await
        .wrap_err("Network overall height did not pass given predicate within timeout")?;

        Ok(self)
    }
}

/// Builder of [`Network`]
pub struct NetworkBuilder {
    n_peers: usize,
    config: Table,
    pipeline_time: Option<Duration>,
    extra_isi: Vec<InstructionBox>,
}

impl Default for NetworkBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Test network builder
impl NetworkBuilder {
    /// Constructor
    pub fn new() -> Self {
        Self {
            n_peers: 1,
            config: config::base_iroha_config(),
            pipeline_time: Some(INSTANT_PIPELINE_TIME),
            extra_isi: vec![],
        }
    }

    /// Set the number of peers in the network.
    ///
    /// One by default.
    pub fn with_peers(mut self, n_peers: usize) -> Self {
        assert_ne!(n_peers, 0);
        self.n_peers = n_peers;
        self
    }

    /// Set the pipeline time.
    ///
    /// Translates into setting of the [`SumeragiParameter::BlockTimeMs`] (1/3) and
    /// [`SumeragiParameter::CommitTimeMs`] (2/3) in the genesis block.
    ///
    /// Reflected in [`Network::pipeline_time`].
    pub fn with_pipeline_time(mut self, duration: Duration) -> Self {
        self.pipeline_time = Some(duration);
        self
    }

    /// Do not overwrite default pipeline time ([`SumeragiParameters::default`]) in genesis.
    pub fn with_default_pipeline_time(mut self) -> Self {
        self.pipeline_time = None;
        self
    }

    /// Add a layer of TOML configuration via [`TomlWriter`].
    ///
    /// # Example
    ///
    /// ```
    /// use iroha_test_network::NetworkBuilder;
    ///
    /// NetworkBuilder::new().with_config(|t| {
    ///     t.write(["logger", "level"], "DEBUG");
    /// });
    /// ```
    pub fn with_config<F>(mut self, f: F) -> Self
    where
        for<'a> F: FnOnce(&'a mut TomlWriter<'a>),
    {
        let mut writer = TomlWriter::new(&mut self.config);
        f(&mut writer);
        self
    }

    /// Append an instruction to genesis.
    pub fn with_genesis_instruction(mut self, isi: impl Into<InstructionBox>) -> Self {
        self.extra_isi.push(isi.into());
        self
    }

    /// Build the [`Network`]. Doesn't start it.
    pub fn build(self) -> Network {
        let peers: Vec<_> = (0..self.n_peers).map(|_| NetworkPeer::generate()).collect();

        let topology: UniqueVec<_> = peers.iter().map(|peer| peer.peer_id()).collect();

        let block_sync_gossip_period = DEFAULT_BLOCK_SYNC;

        let mut extra_isi = vec![];
        let block_time;
        let commit_time;
        if let Some(duration) = self.pipeline_time {
            block_time = duration / 3;
            commit_time = duration / 2;
            extra_isi.extend([
                InstructionBox::SetParameter(SetParameter::new(Parameter::Sumeragi(
                    SumeragiParameter::BlockTimeMs(block_time.as_millis() as u64),
                ))),
                InstructionBox::SetParameter(SetParameter::new(Parameter::Sumeragi(
                    SumeragiParameter::CommitTimeMs(commit_time.as_millis() as u64),
                ))),
            ]);
        } else {
            block_time = SumeragiParameters::default().block_time();
            commit_time = SumeragiParameters::default().commit_time();
        }

        let genesis = config::genesis(
            [
                InstructionBox::SetParameter(SetParameter::new(Parameter::Sumeragi(
                    SumeragiParameter::BlockTimeMs(block_time.as_millis() as u64),
                ))),
                InstructionBox::SetParameter(SetParameter::new(Parameter::Sumeragi(
                    SumeragiParameter::CommitTimeMs(commit_time.as_millis() as u64),
                ))),
            ]
            .into_iter()
            .chain(self.extra_isi)
            .chain(extra_isi),
            topology,
        );

        Network {
            peers,
            genesis,
            block_time,
            commit_time,
            config: self.config.write(
                ["network", "block_gossip_period_ms"],
                block_sync_gossip_period.as_millis() as u64,
            ),
        }
    }

    /// Same as [`Self::build`], but also creates a [`Runtime`].
    ///
    /// This method exists for convenience and to preserve compatibility with non-async tests.
    pub fn build_blocking(self) -> (Network, Runtime) {
        let rt = runtime::Builder::new_multi_thread()
            .thread_stack_size(32 * 1024 * 1024)
            .enable_all()
            .build()
            .unwrap();
        let network = self.build();
        (network, rt)
    }

    /// Build and start the network.
    ///
    /// Resolves when all peers are running and have committed genesis block.
    /// See [`Network::start_all`].
    pub async fn start(self) -> Result<Network> {
        init_logger();

        let network = self.build();
        network.start_all().await;
        Ok(network)
    }

    /// Combination of [`Self::build_blocking`] and [`Self::start`].
    pub fn start_blocking(self) -> Result<(Network, Runtime)> {
        let (network, rt) = self.build_blocking();
        rt.block_on(async { network.start_all().await });
        Ok((network, rt))
    }
}

/// A common signatory in the test network.
///
/// # Example
///
/// ```
/// use iroha_test_network::Signatory;
///
/// let _alice_kp = Signatory::Alice.key_pair();
/// ```
pub enum Signatory {
    Peer,
    Genesis,
    Alice,
}

impl Signatory {
    /// Get the associated key pair
    pub fn key_pair(&self) -> &KeyPair {
        match self {
            Signatory::Peer => &PEER_KEYPAIR,
            Signatory::Genesis => &SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
            Signatory::Alice => &ALICE_KEYPAIR,
        }
        .deref()
    }
}

/// Running Iroha peer.
///
/// Aborts peer forcefully when dropped
#[derive(Debug)]
struct PeerRun {
    tasks: JoinSet<()>,
    shutdown: oneshot::Sender<()>,
}

/// Lifecycle events of a peer
#[derive(Copy, Clone, Debug)]
pub enum PeerLifecycleEvent {
    /// Process spawned
    Spawned,
    /// Server started to respond
    ServerStarted,
    /// Process terminated
    Terminated { status: ExitStatus },
    /// Process was killed
    Killed,
    /// Caught a related pipeline event
    BlockApplied { height: u64 },
}

/// Controls execution of `irohad` child process.
///
/// While exists, allocates socket ports and a temporary directory (not cleared automatically).
///
/// It can be started and shut down repeatedly.
/// It stores configuration and logs for each run separately.
///
/// When dropped, aborts the child process (if it is running).
#[derive(Clone, Debug)]
pub struct NetworkPeer {
    span: tracing::Span,
    id: Peer,
    key_pair: KeyPair,
    dir: Arc<TempDir>,
    run: Arc<Mutex<Option<PeerRun>>>,
    runs_count: Arc<AtomicUsize>,
    is_running: Arc<AtomicBool>,
    events: broadcast::Sender<PeerLifecycleEvent>,
    block_height: watch::Sender<Option<BlockHeight>>,
    // dropping these the last
    port_p2p: Arc<AllocatedPort>,
    port_api: Arc<AllocatedPort>,
}

impl NetworkPeer {
    /// Generate a random peer
    pub fn generate() -> Self {
        init_logger();

        let mnemonic = petname::petname(2, "_").unwrap();
        let key_pair = KeyPair::random();
        let port_p2p = AllocatedPort::new();
        let port_api = AllocatedPort::new();
        let id = Peer::new(
            socket_addr!(127.0.0.1:*port_p2p),
            key_pair.public_key().clone(),
        );
        let temp_dir = Arc::new({
            let mut builder = tempfile::Builder::new();
            builder.keep(true).prefix(TEMPDIR_PREFIX);
            match tempdir_in() {
                Some(path) => builder.tempdir_in(path),
                None => builder.tempdir(),
            }
            .expect("temp dirs must be available in the system")
        });

        let (events, _rx) = broadcast::channel(32);
        let (block_height, _rx) = watch::channel(None);

        let span = info_span!("peer", mnemonic);
        span.in_scope(|| {
            info!(
                dir=%temp_dir.path().display(),
                port_p2p=%port_p2p,
                port_api=%port_api,
                "Generated peer",
            )
        });

        Self {
            span,
            id,
            key_pair,
            dir: temp_dir,
            run: Default::default(),
            runs_count: Default::default(),
            is_running: Default::default(),
            events,
            block_height,
            port_p2p: Arc::new(port_p2p),
            port_api: Arc::new(port_api),
        }
    }

    /// Spawn the child process.
    ///
    /// Passed configuration must contain network topology in the `trusted_peers` parameter.
    ///
    /// This function waits for peer server to start working,
    /// in particular it waits for `/status` response and connects to event stream.
    /// However it doesn't wait for genesis block to be commited.
    /// See [`Self::events`]/[`Self::once`]/[`Self::once_block`] to monitor peer's lifecycle.
    ///
    /// # Panics
    /// If peer was not started.
    pub async fn start(&self, config: Table, genesis: Option<&GenesisBlock>) {
        init_logger();

        let mut run_guard = self.run.lock().await;
        assert!(run_guard.is_none(), "already running");

        let run_num = self.runs_count.fetch_add(1, Ordering::Relaxed) + 1;
        let span = info_span!(parent: &self.span, "peer_run", run_num);
        span.in_scope(|| info!("Starting"));

        let mut config = config
            .clone()
            .write("public_key", self.key_pair.public_key())
            .write(
                "private_key",
                ExposedPrivateKey(self.key_pair.private_key().clone()),
            )
            .write(
                ["network", "address"],
                format!("127.0.0.1:{}", self.port_p2p),
            )
            .write(
                ["network", "public_address"],
                format!("127.0.0.1:{}", self.port_p2p),
            )
            .write(["torii", "address"], format!("127.0.0.1:{}", self.port_api))
            .write(["logger", "format"], "pretty");

        let config_path = self.dir.path().join(format!("run-{run_num}-config.toml"));
        let genesis_path = self.dir.path().join(format!("run-{run_num}-genesis.scale"));

        if genesis.is_some() {
            config = config.write(["genesis", "file"], &genesis_path);
        }

        tokio::fs::write(
            &config_path,
            toml::to_string(&config).expect("TOML config is valid"),
        )
        .await
        .expect("temp directory exists and there was no config file before");

        if let Some(genesis) = genesis {
            tokio::fs::write(genesis_path, genesis.0.encode())
                .await
                .expect("tmp dir is available and genesis was not written before");
        }

        let mut cmd = tokio::process::Command::new(iroha_bin().as_ref());
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .arg("--config")
            .arg(config_path)
            .arg("--terminal-colors=true");
        cmd.current_dir(self.dir.path());
        let mut child = cmd.spawn().expect("spawn failure is abnormal");
        self.is_running.store(true, Ordering::Relaxed);
        let _ = self.events.send(PeerLifecycleEvent::Spawned);

        let mut tasks = JoinSet::<()>::new();

        {
            let output = child.stdout.take().unwrap();
            let mut file = File::create(self.dir.path().join(format!("run-{run_num}-stdout.log")))
                .await
                .unwrap();
            tasks.spawn(async move {
                let mut lines = BufReader::new(output).lines();
                while let Ok(Some(mut line)) = lines.next_line().await {
                    line.push('\n');
                    file.write_all(line.as_bytes())
                        .await
                        .expect("writing logs to file shouldn't fail");
                    file.flush()
                        .await
                        .expect("writing logs to file shouldn't fail");
                }
            });
        }
        {
            let span = span.clone();
            let output = child.stderr.take().unwrap();
            let path = self.dir.path().join(format!("run-{run_num}-stderr.log"));
            tasks.spawn(async move {
                let mut in_memory = PeerStderrBuffer {
                    span,
                    buffer: String::new(),
                };
                let mut lines = BufReader::new(output).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    in_memory.buffer.push_str(&line);
                    in_memory.buffer.push('\n');
                }

                let mut file = File::create(path).await.expect("should create");
                file.write_all(in_memory.buffer.as_bytes())
                    .await
                    .expect("should write");
            });
        }

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let is_normal_shutdown_started = Arc::new(AtomicBool::new(false));
        let peer_exit = PeerExit {
            child,
            span: span.clone(),
            is_running: self.is_running.clone(),
            is_normal_shutdown_started: is_normal_shutdown_started.clone(),
            events: self.events.clone(),
            block_height: self.block_height.clone(),
        };
        tasks.spawn(
            async move {
                if let Err(err) = peer_exit.monitor(shutdown_rx).await {
                    error!("something went very bad during peer exit monitoring: {err}");
                    panic!()
                }
            }
            .instrument(span.clone()),
        );

        {
            let client = self.client();
            let events_tx = self.events.clone();
            let block_height_tx = self.block_height.clone();
            tasks.spawn(
                async move {
                    let status_client = client.clone();
                    let status = backoff::future::retry(
                        ExponentialBackoffBuilder::new()
                            .with_initial_interval(Duration::from_millis(50))
                            .with_max_interval(Duration::from_secs(1))
                            .with_max_elapsed_time(None)
                            .build(),
                        move || {
                            let client = status_client.clone();
                            // let log_prefix_status = log_prefix_status.clone();
                            async move {
                                let status = spawn_blocking(move || client.get_status())
                                    .await
                                    .expect("should not panic");
                                if let Err(err) = &status {
                                    warn!("get status failed: {err}")
                                };
                                Ok(status?)
                            }
                        },
                    )
                    .await
                    .expect("there is no max elapsed time");
                    let mut block_height = BlockHeight::from(status);
                    let _ = events_tx.send(PeerLifecycleEvent::ServerStarted);
                    let _ = block_height_tx.send_replace(Some(block_height));
                    info!(?status, "server started");

                    loop {
                        let mut blocks = match client
                            .listen_for_blocks_async(NonZero::new(block_height.total + 1).unwrap())
                            .await
                        {
                            Ok(stream) => stream,
                            Err(err) => {
                                const RETRY: Duration = Duration::from_secs(1);
                                error!(%err, "failed to subscribe to blocks, will retry again in {RETRY:?}");
                                tokio::time::sleep(RETRY).await;
                                continue
                            }
                        };

                        while let Some(Ok(block)) = blocks.next().await {
                            let height = block.header().height().get();
                            let is_empty = block.header().transactions_hash().is_none();
                            assert_eq!(height, block_height.total + 1);
                            block_height.total += 1;
                            if !is_empty {
                                block_height.non_empty += 1;
                            }

                            info!(?block_height, "received block");
                            block_height_tx.send_modify(|x| {
                                if x.is_some() {
                                    *x = Some(block_height);
                                }
                                // if none - peer terminated
                            });
                        }
                        if is_normal_shutdown_started.load(Ordering::Relaxed) {
                            info!("block stream closed normally after shutdown");
                            break
                        }
                        else {
                            warn!("blocks stream closed while there is no shutdown signal yet; reconnecting");
                        }
                    }
                }
                .instrument(span),
            );
        }

        *run_guard = Some(PeerRun {
            tasks,
            shutdown: shutdown_tx,
        });
    }

    /// Forcefully kills the running peer
    ///
    /// # Panics
    /// If peer was not started.
    pub async fn shutdown(&self) {
        let mut guard = self.run.lock().await;
        let Some(run) = (*guard).take() else {
            panic!("peer is not running, nothing to shut down");
        };
        if self.is_running() {
            let _ = run.shutdown.send(());
            timeout(PEER_SHUTDOWN_TIMEOUT, run.tasks.join_all())
                .await
                .expect("run-related tasks should exit within timeout");
            assert!(!self.is_running());
        }
    }

    /// Subscribe on peer lifecycle events.
    pub fn events(&self) -> broadcast::Receiver<PeerLifecycleEvent> {
        self.events.subscribe()
    }

    /// Wait _once_ an event matches a predicate.
    ///
    /// ```ignore
    /// use iroha_test_network::{Network, NetworkBuilder, PeerLifecycleEvent};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let network = NetworkBuilder::new().build();
    ///     let peer = network.peer();
    ///
    ///     tokio::join!(
    ///         peer.start(network.config(), None),
    ///         peer.once(|event| matches!(event, PeerLifecycleEvent::ServerStarted))
    ///     );
    /// }
    /// ```
    ///
    /// It is a narrowed version of [`Self::events`].
    pub async fn once<F>(&self, f: F)
    where
        F: Fn(PeerLifecycleEvent) -> bool,
    {
        let mut rx = self.events();
        loop {
            tokio::select! {
                Ok(event) = rx.recv() => {
                    if f(event) { break }
                }
            }
        }
    }

    /// Wait until peer's non-empty block height reaches N.
    ///
    /// Resolves immediately if peer is already running _and_ has at least N non-empty blocks committed.
    pub async fn once_block(&self, n: u64) {
        self.once_block_with(|height| height.non_empty >= n).await
    }

    /// Wait until peer's block height passes the given predicate.
    ///
    /// Resolves immediately if peer is running _and_ the predicate passes.
    pub async fn once_block_with<F: Fn(BlockHeight) -> bool>(&self, f: F) {
        let mut recv = self.block_height.subscribe();

        if recv.borrow().map(&f).unwrap_or(false) {
            return;
        }

        loop {
            recv.changed()
                .await
                .expect("could fail only if the peer is dropped");

            if recv.borrow_and_update().map(&f).unwrap_or(false) {
                break;
            }
        }
    }

    /// Generated [`Peer`]
    pub fn peer(&self) -> Peer {
        self.id.clone()
    }

    /// Generated [`PeerId`]
    pub fn peer_id(&self) -> PeerId {
        self.id.id().clone()
    }

    /// Check whether the peer is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Create a client to interact with this peer
    pub fn client_for(&self, account_id: &AccountId, account_private_key: PrivateKey) -> Client {
        let config = ConfigReader::new()
            .with_toml_source(TomlSource::inline(
                Table::new()
                    .write("chain", config::chain_id())
                    .write(["account", "domain"], account_id.domain())
                    .write(["account", "public_key"], account_id.signatory())
                    .write(
                        ["account", "private_key"],
                        ExposedPrivateKey(account_private_key.clone()),
                    )
                    .write("torii_url", format!("http://127.0.0.1:{}", self.port_api)),
            ))
            .read_and_complete::<iroha::config::UserConfig>()
            .expect("peer client config should be valid")
            .parse()
            .expect("peer client config should be valid");

        Client::new(config)
    }

    /// Client for Alice. ([`Self::client_for`] + [`Signatory::Alice`])
    pub fn client(&self) -> Client {
        self.client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone())
    }

    pub async fn status(&self) -> Result<Status> {
        let client = self.client();
        spawn_blocking(move || client.get_status())
            .await
            .expect("should not panic")
    }
}

/// Compare by ID
impl PartialEq for NetworkPeer {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

/// Prints collected STDERR on drop.
///
/// Used to avoid loss of useful data in case of task abortion before it is printed directly.
struct PeerStderrBuffer {
    span: tracing::Span,
    buffer: String,
}

impl Drop for PeerStderrBuffer {
    fn drop(&mut self) {
        if !self.buffer.is_empty() {
            self.span.in_scope(|| {
                info!("STDERR:\n=======\n{}======= END OF STDERR", self.buffer);
            });
        }
    }
}

struct PeerExit {
    child: Child,
    span: tracing::Span,
    is_running: Arc<AtomicBool>,
    is_normal_shutdown_started: Arc<AtomicBool>,
    events: broadcast::Sender<PeerLifecycleEvent>,
    block_height: watch::Sender<Option<BlockHeight>>,
}

impl PeerExit {
    async fn monitor(mut self, shutdown: oneshot::Receiver<()>) -> Result<()> {
        let status = tokio::select! {
            status = self.child.wait() => status?,
            _ = shutdown => self.shutdown_or_kill().await?,
        };

        self.span.in_scope(|| info!(%status, "Peer terminated"));
        let _ = self.events.send(PeerLifecycleEvent::Terminated { status });
        self.is_running.store(false, Ordering::Relaxed);
        self.block_height.send_modify(|x| *x = None);

        Ok(())
    }

    async fn shutdown_or_kill(&mut self) -> Result<ExitStatus> {
        use nix::{sys::signal, unistd::Pid};
        const TIMEOUT: Duration = Duration::from_secs(5);

        self.is_normal_shutdown_started
            .store(true, Ordering::Relaxed);

        self.span.in_scope(|| info!("sending SIGTERM"));
        signal::kill(
            Pid::from_raw(self.child.id().ok_or(eyre!("race condition"))? as i32),
            signal::Signal::SIGTERM,
        )
        .wrap_err("failed to send SIGTERM")?;

        if let Ok(status) = timeout(TIMEOUT, self.child.wait()).await {
            self.span.in_scope(|| info!("exited gracefully"));
            return status.wrap_err("wait failure");
        };
        self.span
            .in_scope(|| warn!("process didn't terminate after {TIMEOUT:?}, killing"));
        timeout(TIMEOUT, async move {
            self.child.kill().await.expect("not a recoverable failure");
            self.child.wait().await
        })
        .await
        .wrap_err("didn't terminate after SIGKILL")?
        .wrap_err("wait failure")
    }
}

/// Composite block height representation
#[derive(Debug, Copy, Clone)]
pub struct BlockHeight {
    /// Total blocks
    pub total: u64,
    /// Non-empty blocks
    pub non_empty: u64,
}

impl From<Status> for BlockHeight {
    fn from(value: Status) -> Self {
        Self {
            total: value.blocks,
            non_empty: value.blocks_non_empty,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn can_start_networks() {
        NetworkBuilder::new().with_peers(4).start().await.unwrap();
        NetworkBuilder::new().start().await.unwrap();
    }
}
