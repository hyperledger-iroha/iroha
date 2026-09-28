//! The Sumeragi instance of a node (`specs/sumeragi.md` §12): startup, the production
//! backends and the driver.
//!
//! [`start`] applies (fresh chain) or re-executes (restart) genesis, replays the blocks Kura
//! holds through the executor, installs the safety records of the node's keys, assembles the
//! core's `Init` and spawns the driver over:
//! the transport `N` (P2P in the node, in-memory in tests), the file record and body stores,
//! Kura, the system clock and the State executor.

use std::{path::PathBuf, sync::Arc, time::Duration};

use iroha_config::parameters::actual::SumeragiLocalOverrides;

use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    block::SignedBlock,
    parameter::system::ConsensusMode,
    sumeragi::{SumeragiFootprint, SumeragiHaltReason, SumeragiStatus},
};
use iroha_model_base::peer::PeerId;
/// The Sumeragi wire protocol version peers bind in the handshake.
pub use iroha_sumeragi::message::PROTOCOL_VERSION;
use iroha_sumeragi::{
    api::{CoreStatus, HaltReason, LocalParams},
    crypto::NoAttestation,
    preimage::{InstanceKind, instance_id},
    types::{Hash32, PublicKey},
};

use super::{
    block_store::{KuraBlockStore, Staging},
    bodies::{BodyLimits, FileBodyStore},
    crypto::{BlsCrypto, KeyPairSigner, core_key, iroha_key},
    driver::{
        Driver, DriverConfig, DriverHandle, DriverStart, RunningDriver, SharedCrypto,
        assemble_init,
        traits::{BlockStore, Net, Observer, SystemClock},
    },
    executor::{ExecutorContext, StateExecutor},
    net::{FrameCaps, P2pNet, SumeragiIngress, spawn_ingress, subscribe},
    records::{FileRecordStore, FreshKeyAssertion, install},
    schedule,
    startup::{self, GENESIS_HEIGHT, GenesisTip, StartupError},
};
use crate::{
    EventsSender, IrohaNetwork,
    kura::Kura,
    queue::Queue,
    state::{State, WorldReadOnly},
};

/// Where the instance keeps its files, and the operator's startup choices.
#[derive(Clone, Debug)]
pub struct NodeConfig {
    /// Safety records (one file per instance and key; never backed up or restored).
    pub records_dir: PathBuf,
    /// The installation log, outside `records_dir`.
    pub installation_log: PathBuf,
    /// Root of the body store (bodies of accepted, unapplied blocks).
    pub bodies_dir: PathBuf,
    /// Overrides of the core's local parameters (the rest are the defaults for the committee
    /// size).
    pub local: SumeragiLocalOverrides,
    /// The operator asserts, at first boot, that the node's keys never signed for this chain
    /// (the one-shot `--sumeragi-assert-fresh-key` flag, §7.4).
    pub assert_fresh_key: bool,
    /// Retired consensus keys (restored, never signing).
    pub retired_keys: Vec<iroha_crypto::PublicKey>,
}

/// What [`prepare`] needs: the state to rebuild and where its blocks are.
pub struct PrepareInputs {
    /// The node's state (empty: it is rebuilt from genesis and Kura).
    pub state: Arc<State>,
    /// Kura.
    pub kura: Arc<Kura>,
    /// Pipeline and state events.
    pub events: EventsSender,
    /// The chain id.
    pub chain_id: String,
    /// The signed genesis block of a fresh chain (ignored when Kura already holds genesis).
    pub genesis: Option<SignedBlock>,
    /// The genesis account.
    pub genesis_account: AccountId,
    /// The consensus mode.
    pub consensus_mode: ConsensusMode,
}

/// What [`Prepared::start`] adds: the queue, the transport, the node's key and files.
pub struct StartInputs<N> {
    /// The transport.
    pub net: Arc<N>,
    /// The transaction queue.
    pub queue: Arc<Queue>,
    /// The node's consensus key pair (BLS normal).
    pub key_pair: KeyPair,
    /// Files and operator choices.
    pub config: NodeConfig,
    /// Reports of the instance.
    pub observer: Arc<dyn Observer>,
    /// Driver limits.
    pub driver: DriverConfig,
    /// Runtime-only threshold share custody installed by the node's signer broker.
    pub beacon_signer: Option<Arc<dyn crate::beacon::GlobalThresholdBeaconPartialSignerV1>>,
}

/// Everything [`start`] needs from the node.
pub struct NodeInputs<N> {
    /// The node's state (empty before genesis).
    pub state: Arc<State>,
    /// Kura.
    pub kura: Arc<Kura>,
    /// The transaction queue.
    pub queue: Arc<Queue>,
    /// Pipeline and state events.
    pub events: EventsSender,
    /// The transport.
    pub net: Arc<N>,
    /// The node's consensus key pair (BLS normal).
    pub key_pair: KeyPair,
    /// The chain id.
    pub chain_id: String,
    /// The signed genesis block of a fresh chain (ignored when Kura already holds genesis).
    pub genesis: Option<SignedBlock>,
    /// The genesis account.
    pub genesis_account: AccountId,
    /// The consensus mode.
    pub consensus_mode: ConsensusMode,
    /// Files and operator choices.
    pub config: NodeConfig,
    /// Reports of the instance.
    pub observer: Arc<dyn Observer>,
    /// Driver limits.
    pub driver: DriverConfig,
}

/// A running Sumeragi instance.
pub struct RunningNode {
    /// The driver.
    pub driver: RunningDriver,
    /// The instance id (`I`).
    pub instance: Hash32,
    /// The instance's cryptography.
    pub crypto: Arc<BlsCrypto>,
    identity: NodeIdentity,
    beacon: Arc<super::beacon::BeaconService>,
}

impl RunningNode {
    /// The handle Torii, the transport and the node's services use.
    pub fn handle(&self) -> NodeHandle {
        NodeHandle {
            driver: self.driver.handle(),
            instance: self.instance,
            identity: self.identity.clone(),
        }
    }
}

/// A cheap, cloneable handle of the node's running instance.
#[derive(Clone)]
pub struct NodeHandle {
    driver: DriverHandle,
    instance: Hash32,
    identity: NodeIdentity,
}

/// Immutable identity and resolved configuration of the running consensus instance.
#[derive(Clone, Debug)]
pub struct NodeIdentity {
    /// Consensus peer identity installed at startup.
    pub node_id: PeerId,
    /// Canonical fingerprint of the effective local and driver settings.
    pub config_fingerprint: Hash,
}

impl core::fmt::Debug for NodeHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NodeHandle")
            .field("instance", &self.instance)
            .finish_non_exhaustive()
    }
}

impl NodeHandle {
    /// Identity captured from the actual startup inputs, never from HTTP parameters.
    pub fn identity(&self) -> &NodeIdentity {
        &self.identity
    }

    /// The instance id (`I`).
    pub fn instance(&self) -> Hash32 {
        self.instance
    }

    /// The core's latest diagnostics (`None` before the core started).
    pub fn status(&self) -> Option<CoreStatus> {
        self.driver.status()
    }

    /// Why the instance halted, if it did (a stopped worker stops it too).
    pub fn halted(&self) -> Option<HaltReason> {
        self.driver.halted()
    }

    /// The status the node serves (`/v1/sumeragi/status`; `None` before the core started).
    pub fn status_dto(&self) -> Option<SumeragiStatus> {
        let status = self.status()?;
        let key = |key: &PublicKey| iroha_key(key).ok();
        let widen = |count: usize| u64::try_from(count).unwrap_or(u64::MAX);
        let footprint = &status.footprint;
        Some(SumeragiStatus {
            instance: status.instance.0,
            height: status.height,
            view: status.view,
            stage: status.stage,
            leader: status.leader.as_ref().and_then(key),
            proxy_tail: status.proxy_tail.as_ref().and_then(key),
            high_qc_view: status.high_qc_view,
            level: status.level,
            start_level: status.start_level,
            t_retx_ms: status.t_retx,
            committed_height: status.committed_height,
            applied_height: status.applied_height,
            awaiting: status.awaiting,
            signer: status.signer.as_ref().and_then(key),
            unanchored: status.unanchored,
            abstaining: status.abstaining,
            halted: self.halted().map(|reason| match reason {
                HaltReason::SafetyRecordCorrupt => SumeragiHaltReason::SafetyRecordCorrupt,
                HaltReason::SafetyRecordInconsistent => {
                    SumeragiHaltReason::SafetyRecordInconsistent
                }
                HaltReason::SafetyViolation { height } => {
                    SumeragiHaltReason::SafetyViolation(height)
                }
                HaltReason::ApplyDiverged { height } => SumeragiHaltReason::ApplyDiverged(height),
                HaltReason::DriverAnomaly => SumeragiHaltReason::DriverAnomaly,
            }),
            footprint: SumeragiFootprint {
                votes: widen(footprint.votes),
                timeouts: widen(footprint.timeouts),
                blocks: widen(footprint.blocks),
                exec_entries: widen(footprint.exec_entries),
                wants: widen(footprint.wants),
                pending_apply: widen(footprint.pending_apply),
                sync_entries: widen(footprint.sync_entries),
                sync_bytes: widen(footprint.sync_bytes),
                peers: widen(footprint.peers),
                recent_headers: widen(footprint.recent_headers),
                configs: widen(footprint.configs),
                cert_cache: widen(footprint.cert_cache),
                evidence_keys: widen(footprint.evidence_keys),
                probe: widen(footprint.probe),
            },
        })
    }

    /// The instance halted or stopped: only a restart recovers the node's consensus.
    pub fn restart_required(&self) -> bool {
        self.halted().is_some()
    }

    /// The core started, has not halted, and the instance runs.
    pub fn ready(&self) -> bool {
        self.driver.ready()
    }

    /// An includable transaction entered the queue (the leader's `PayloadReady`).
    pub fn transactions_available(&self) {
        self.driver.transactions_available();
    }

    /// The driver's handle (the transport's frame sink).
    pub fn driver(&self) -> &DriverHandle {
        &self.driver
    }
}

/// The global instance id (`I`, §3.5) of the chain with this genesis block and chain id. Peers
/// bind it in the handshake as the consensus fingerprint.
pub fn global_instance(genesis: &SignedBlock, chain_id: &str) -> Hash32 {
    instance_id(
        &BlsCrypto::new(),
        &startup::core_hash_of(genesis),
        chain_id.as_bytes(),
        InstanceKind::Global,
        0,
    )
}

/// Why the instance could not start.
#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    /// Genesis or replay failed.
    #[error(transparent)]
    Startup(#[from] StartupError),
    /// No genesis: Kura holds none and none was supplied.
    #[error("no genesis block: Kura holds none and none was supplied")]
    NoGenesis,
    /// The node's key is not BLS normal.
    #[error("the consensus key is not BLS normal: {0}")]
    Key(String),
    /// A block Kura holds does not replay.
    #[error("replay of height {height} failed: {reason}")]
    Replay {
        /// Height.
        height: u64,
        /// Reason.
        reason: String,
    },
    /// Records, bodies or the schedule could not be loaded.
    #[error("startup input: {0}")]
    Input(String),
    /// The driver did not start.
    #[error("driver: {0}")]
    Driver(String),
}

/// Start the node's Sumeragi instance: [`prepare`], then [`Prepared::start`].
///
/// # Errors
/// See [`NodeError`].
pub fn start<N: Net + 'static>(inputs: NodeInputs<N>) -> Result<RunningNode, NodeError> {
    let NodeInputs {
        state,
        kura,
        queue,
        events,
        net,
        key_pair,
        chain_id,
        genesis,
        genesis_account,
        consensus_mode,
        config,
        observer,
        driver,
    } = inputs;
    prepare(PrepareInputs {
        state,
        kura,
        events,
        chain_id,
        genesis,
        genesis_account,
        consensus_mode,
    })?
    .start(StartInputs {
        beacon_signer: None,
        net,
        queue,
        key_pair,
        config,
        observer,
        driver,
    })
}

/// The instance with its state rebuilt, ready to start.
pub struct Prepared {
    state: Arc<State>,
    crypto: Arc<BlsCrypto>,
    instance: Hash32,
    tip: GenesisTip,
    blocks: Arc<KuraBlockStore>,
    executor: StateExecutor,
    consensus_mode: ConsensusMode,
    applied_watch: Arc<crate::sumeragi::lanes::global::AppliedWatch>,
}

impl core::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Prepared")
            .field("instance", &self.instance)
            .field("tip", &self.tip)
            .finish_non_exhaustive()
    }
}

/// Rebuild the state (§12.1): apply the supplied genesis to the empty state (fresh chain) or
/// re-execute the stored one, then replay every block Kura holds, each checked against its
/// certified result. Nothing is sent or signed.
///
/// # Errors
/// See [`NodeError`].
pub fn prepare(inputs: PrepareInputs) -> Result<Prepared, NodeError> {
    let PrepareInputs {
        state,
        kura,
        events,
        chain_id,
        genesis,
        genesis_account,
        consensus_mode,
    } = inputs;
    // TODO(S7): start from a snapshot (a consensus anchor: tip, CommitQC, `W + 2` headers).
    if startup::applied_height(&state) != 0 {
        return Err(NodeError::Input(
            "the state must be empty: Sumeragi rebuilds it from genesis and Kura".into(),
        ));
    }
    // Genesis: re-execute the stored one, or apply the supplied one.
    let tip: GenesisTip = match startup::stored_genesis(&state) {
        Some((block, certificate, stored)) => {
            if let Some(supplied) = &genesis
                && startup::core_hash_of(supplied) != stored.block_hash
            {
                return Err(NodeError::Input(
                    "the supplied genesis differs from the one Kura holds".into(),
                ));
            }
            startup::apply_genesis(
                &state,
                block,
                &genesis_account,
                consensus_mode,
                Some(&certificate),
            )?
        }
        None => startup::apply_genesis(
            &state,
            genesis.ok_or(NodeError::NoGenesis)?,
            &genesis_account,
            consensus_mode,
            None,
        )?,
    };
    let crypto = Arc::new(BlsCrypto::new());
    let shared: SharedCrypto = crypto.clone();
    let instance = instance_id(
        &*crypto,
        &tip.block_hash,
        chain_id.as_bytes(),
        InstanceKind::Global,
        0,
    );
    let staging = Staging::new();
    let blocks = Arc::new(KuraBlockStore::new(
        Arc::clone(&kura),
        shared,
        GENESIS_HEIGHT,
        staging.clone(),
    ));
    let applied_watch = Arc::new(crate::sumeragi::lanes::global::AppliedWatch::new(
        GENESIS_HEIGHT,
        state.view().latest_block_hash(),
    ));
    let mut executor = StateExecutor::spawn(ExecutorContext {
        state: Arc::clone(&state),
        queue: None,
        staging,
        events,
        genesis_account,
        consensus_mode,
        applied: (GENESIS_HEIGHT, tip.block_hash),
        crypto: Some(Arc::clone(&crypto)),
        applied_watch: Arc::clone(&applied_watch),
    })
    .map_err(|error| NodeError::Driver(error.to_string()))?;
    admit_window(&state, &crypto, GENESIS_HEIGHT);
    // Replay what Kura holds above genesis.
    let stored = blocks.height();
    for height in GENESIS_HEIGHT.saturating_add(1)..=stored {
        let entry = blocks.entry(height).ok_or_else(|| NodeError::Replay {
            height,
            reason: "entry missing".into(),
        })?;
        executor
            .replay(&entry.block, &entry.commit_qc)
            .map_err(|reason| NodeError::Replay { height, reason })?;
    }
    admit_window(&state, &crypto, stored.max(GENESIS_HEIGHT));
    Ok(Prepared {
        state,
        crypto,
        instance,
        tip,
        blocks,
        executor,
        consensus_mode,
        applied_watch,
    })
}

impl Prepared {
    /// Bind SoraFS capture after startup reconciliation and before consensus can advance.
    ///
    /// # Errors
    /// The exact committed tip cannot be captured or this executor is already bound.
    pub fn attach_finalized_archives(
        &self,
        archives: super::executor::FinalizedArchives,
    ) -> Result<(), NodeError> {
        self.executor
            .attach_finalized_archives(archives)
            .map_err(NodeError::Input)
    }

    /// The instance id (`I`): peers bind it in the handshake.
    pub fn instance(&self) -> Hash32 {
        self.instance
    }

    /// Install the safety records of the node's keys, assemble the core's `Init` and spawn the
    /// driver over `inputs.net`.
    ///
    /// # Errors
    /// See [`NodeError`].
    pub fn start<N: Net + 'static>(self, inputs: StartInputs<N>) -> Result<RunningNode, NodeError> {
        let Self {
            state,
            crypto,
            instance,
            tip,
            blocks,
            executor,
            consensus_mode,
            applied_watch: _,
        } = self;
        let StartInputs {
            net,
            queue,
            key_pair,
            config,
            observer,
            driver,
            beacon_signer,
        } = inputs;
        executor.attach_queue(queue);
        let beacon = super::beacon::BeaconService::spawn(
            Arc::clone(&state),
            instance,
            PeerId::new(key_pair.public_key().clone()),
            beacon_signer,
            net.clone(),
            consensus_mode,
        )
        .map_err(|error| NodeError::Driver(error.to_string()))?;
        executor.attach_beacon(Arc::clone(&beacon));
        let shared: SharedCrypto = crypto.clone();
        // Records of the node's keys.
        let key =
            core_key(key_pair.public_key()).map_err(|error| NodeError::Key(error.to_string()))?;
        let mut keys: Vec<(PublicKey, bool)> = vec![(key, false)];
        for retired in &config.retired_keys {
            keys.push((
                core_key(retired).map_err(|error| NodeError::Key(error.to_string()))?,
                true,
            ));
        }
        let records = Arc::new(
            FileRecordStore::open(&config.records_dir, &config.installation_log)
                .map_err(|error| NodeError::Input(error.to_string()))?,
        );
        let assertion = FreshKeyAssertion::from_operator_flag(config.assert_fresh_key);
        let found = install(
            &*records,
            &*crypto,
            &instance,
            &keys,
            GENESIS_HEIGHT,
            assertion.as_ref(),
        )
        .map_err(|error| NodeError::Input(error.to_string()))?;
        // Height configurations of the window (t, t + 1, t + 2).
        let (configs, demotion_window) = {
            let view = state.view();
            let configs = view
                .world()
                .consensus_schedule()
                .init_configs(GENESIS_HEIGHT)
                .map_err(|error| NodeError::Input(error.to_string()))?;
            (
                configs,
                view.world().parameters().sumeragi().demotion_window.get(),
            )
        };
        let n = configs
            .first()
            .map_or(1, |(_, config)| config.committee.n());
        let identity = NodeIdentity {
            node_id: PeerId::new(key_pair.public_key().clone()),
            config_fingerprint: configuration_fingerprint(
                n,
                &config.local,
                &driver,
                &config.retired_keys,
            ),
        };
        let init = assemble_init(
            &*blocks,
            instance,
            GENESIS_HEIGHT,
            (tip.block_hash, tip.result),
            demotion_window,
            found,
            configs,
            startup_nonce(),
        )
        .map_err(|error| NodeError::Input(error.to_string()))?;
        let bodies = Arc::new(
            FileBodyStore::open(
                &config.bodies_dir,
                &instance,
                Arc::clone(&shared),
                BodyLimits::default(),
            )
            .map_err(|error| NodeError::Input(error.to_string()))?,
        );
        let signer =
            KeyPairSigner::new(&key_pair).map_err(|error| NodeError::Key(error.to_string()))?;
        let running = Driver::new(
            net,
            records,
            bodies,
            blocks,
            Arc::new(SystemClock::new()),
            executor,
            observer,
        )
        .spawn(
            driver,
            DriverStart {
                local: local_params(n, &config.local),
                init,
                signers: vec![Box::new(signer)],
                crypto: shared,
                // TODO(WP5c-kagemusha): the KAGEMUSHA Pasta attestor and verifier; until then no
                // block is flagged (top-ups are not proposable before WP8a).
                attestor: Box::new(NoAttestation),
                verifier: Box::new(NoAttestation),
            },
        )
        .map_err(|error| NodeError::Driver(error.to_string()))?;
        beacon.set_wakeup(running.handle());
        Ok(RunningNode {
            beacon,
            driver: running,
            instance,
            crypto,
            identity,
        })
    }

    /// [`Prepared::start`] over the node's P2P `network`: the transport is [`P2pNet`], and
    /// inbound frames reach the driver through a [`SumeragiIngress`] fed by the driver's own
    /// FIFOs, each holding up to `fifo_capacity` messages.
    ///
    /// # Errors
    /// See [`NodeError`]; the subscription or the ingress thread failing is
    /// [`NodeError::Driver`].
    pub fn start_on_network(
        self,
        inputs: StartInputs<P2pNet<IrohaNetwork>>,
        network: &IrohaNetwork,
        fifo_capacity: usize,
    ) -> Result<NetworkedNode, NodeError> {
        let subscription = subscribe(network, fifo_capacity)
            .map_err(|error| NodeError::Driver(error.to_string()))?;
        let node = self.start(inputs)?;
        let ingress = Arc::new(SumeragiIngress::new(FrameCaps::TRANSPORT));
        ingress.register(
            node.instance,
            Arc::new(super::beacon::BeaconFrameSink::new(
                node.driver.handle(),
                Arc::clone(&node.beacon),
            )),
        );
        let ingress_thread = match spawn_ingress(subscription, Arc::clone(&ingress)) {
            Ok(thread) => thread,
            Err(error) => {
                node.beacon.shutdown();
                node.driver.shutdown();
                return Err(NodeError::Driver(format!(
                    "sumeragi ingress thread: {error}"
                )));
            }
        };
        Ok(NetworkedNode {
            node,
            ingress,
            ingress_thread,
        })
    }
}

/// The node's instance on the P2P network: the running instance and the thread routing its
/// three FIFOs (§12.3 O8) to the driver.
pub struct NetworkedNode {
    /// The running instance.
    pub node: RunningNode,
    ingress: Arc<SumeragiIngress>,
    ingress_thread: std::thread::JoinHandle<()>,
}

impl NetworkedNode {
    /// The instance's handle.
    pub fn handle(&self) -> NodeHandle {
        self.node.handle()
    }

    /// Stop the instance and wait for its threads. The ingress thread ends with the network.
    pub fn shutdown(self) {
        self.ingress.unregister(&self.node.instance);
        self.node.beacon.shutdown();
        self.node.driver.shutdown();
        drop(self.ingress_thread);
    }
}

/// Reports of the instance in the node's log. Evidence is logged for the operator; the
/// status endpoint reads [`super::driver::DriverHandle::status`].
#[derive(Clone, Copy, Debug, Default)]
pub struct LogObserver;

impl Observer for LogObserver {
    fn evidence(&self, evidence: &iroha_sumeragi::message::Evidence) {
        iroha_logger::warn!(?evidence, "sumeragi: evidence of misbehaviour");
    }

    fn fault(&self, fault: &iroha_sumeragi::api::LocalFault) {
        iroha_logger::warn!(?fault, "sumeragi: local fault");
    }

    fn halt(&self, reason: &iroha_sumeragi::api::HaltReason) {
        iroha_logger::error!(?reason, "sumeragi: the instance halted");
    }

    fn stopped(&self, worker: super::driver::Worker) {
        iroha_logger::error!(
            ?worker,
            "sumeragi: a worker stopped and the instance with it; a restart recovers"
        );
    }

    fn frame_limit(&self, exceeded: &super::driver::FrameLimitExceeded) {
        iroha_logger::error!(
            height = exceeded.height,
            needed = exceeded.needed,
            limit = exceeded.limit,
            "sumeragi: a committed configuration outgrows the transport frame limit"
        );
    }
}

/// Canonical fingerprint shared by release inventory and the actual running driver.
///
/// Binds every resolved local/driver limit, the body-store cap and retired keys. Chain
/// parameters are authenticated by the committed execution-result preimage. File locations
/// and the one-shot fresh-key assertion do not change the running protocol configuration.
#[must_use]
pub fn configuration_fingerprint(
    committee_size: usize,
    overrides: &SumeragiLocalOverrides,
    driver: &DriverConfig,
    retired_keys: &[iroha_crypto::PublicKey],
) -> Hash {
    use norito::codec::Encode as _;
    let local = local_params(committee_size, overrides);
    let mut bytes = b"iroha/sumeragi/node-configuration/v1\0".to_vec();
    for value in [
        u64::from(PROTOCOL_VERSION),
        local.t_base,
        local.t_max,
        u64::from(local.start_cap),
        u64::from(local.decay_after),
        local.rebroadcast_interval,
        local.status_keepalive,
        local.build_timeout,
        local.fetch_retry,
        u64::from(local.sync_batch),
        local.sync_retry,
        u64::from(local.sync_max_bytes),
        u64::from(local.max_observers),
        driver.ingress.per_peer[0] as u64,
        driver.ingress.per_peer[1] as u64,
        driver.ingress.per_peer[2] as u64,
        u64::from(driver.ingress.bulk_every),
        driver.backoff.initial,
        driver.backoff.max,
        driver.held.effects as u64,
        driver.held.payload_bytes,
        driver.serve.bytes_per_sec,
        driver.serve.burst_bytes,
        driver.serve.max_peers as u64,
        driver.frame_limit,
        BodyLimits::default().max_bytes,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let mut retired = retired_keys.to_vec();
    retired.sort();
    bytes.extend_from_slice(&retired.encode());
    Hash::new(bytes)
}

/// The core's local parameters for a committee of `n`, with the node's overrides.
fn local_params(n: usize, overrides: &SumeragiLocalOverrides) -> LocalParams {
    let millis = |duration: Duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
    let defaults = LocalParams::for_committee_size(n);
    LocalParams {
        t_base: overrides.t_base.map_or(defaults.t_base, millis),
        t_max: overrides.t_max.map_or(defaults.t_max, millis),
        start_cap: overrides.start_cap.unwrap_or(defaults.start_cap),
        decay_after: overrides.decay_after.unwrap_or(defaults.decay_after),
        rebroadcast_interval: overrides
            .rebroadcast_interval
            .map_or(defaults.rebroadcast_interval, millis),
        status_keepalive: overrides
            .status_keepalive
            .map_or(defaults.status_keepalive, millis),
        build_timeout: overrides
            .build_timeout
            .map_or(defaults.build_timeout, millis),
        fetch_retry: overrides.fetch_retry.map_or(defaults.fetch_retry, millis),
        sync_batch: overrides.sync_batch.unwrap_or(defaults.sync_batch),
        sync_retry: overrides.sync_retry.map_or(defaults.sync_retry, millis),
        sync_max_bytes: overrides.sync_max_bytes.unwrap_or(defaults.sync_max_bytes),
        max_observers: overrides.max_observers.unwrap_or(defaults.max_observers),
    }
}

/// Admit the committee keys scheduled for `t`, `t + 1` and `t + 2`.
fn admit_window(state: &State, crypto: &BlsCrypto, t: u64) {
    let view = state.view();
    let world = view.world();
    for height in t..=t.saturating_add(2) {
        let Some(config) = world.consensus_schedule().get(height) else {
            continue;
        };
        for (peer, pop) in schedule::committee_pops(world, config) {
            if let Err(error) = crypto.admit(peer.public_key(), &pop) {
                iroha_logger::warn!(%peer, ?error, "sumeragi: committee key not admitted");
            }
        }
    }
}

/// A fresh nonce for the record-loss probe (§7.4 R2): distinct at every start.
fn startup_nonce() -> u64 {
    rand::random()
}

#[cfg(test)]
mod tests {
    #[test]
    fn configuration_fingerprint_binds_effective_runtime_settings() {
        use super::{DriverConfig, SumeragiLocalOverrides, configuration_fingerprint};
        let local = SumeragiLocalOverrides::default();
        let driver = DriverConfig::default();
        let original = configuration_fingerprint(4, &local, &driver, &[]);
        let explicit_default = SumeragiLocalOverrides {
            t_base: Some(std::time::Duration::from_millis(2_000)),
            ..local
        };
        assert_eq!(
            original,
            configuration_fingerprint(4, &explicit_default, &driver, &[])
        );
        let changed = SumeragiLocalOverrides {
            build_timeout: Some(std::time::Duration::from_millis(201)),
            ..local
        };
        assert_ne!(
            original,
            configuration_fingerprint(4, &changed, &driver, &[])
        );
        let mut changed_driver = driver;
        changed_driver.ingress.per_peer[0] += 1;
        assert_ne!(
            original,
            configuration_fingerprint(4, &local, &changed_driver, &[])
        );
        changed_driver = driver;
        changed_driver.held.payload_bytes += 1;
        assert_ne!(
            original,
            configuration_fingerprint(4, &local, &changed_driver, &[])
        );
        let first =
            iroha_crypto::KeyPair::from_seed(vec![1; 32], iroha_crypto::Algorithm::BlsNormal)
                .public_key()
                .clone();
        let second =
            iroha_crypto::KeyPair::from_seed(vec![2; 32], iroha_crypto::Algorithm::BlsNormal)
                .public_key()
                .clone();
        assert_ne!(
            original,
            configuration_fingerprint(4, &local, &driver, &[first.clone()])
        );
        assert_eq!(
            configuration_fingerprint(4, &local, &driver, &[first.clone(), second.clone()]),
            configuration_fingerprint(4, &local, &driver, &[second, first])
        );
    }

    use std::{collections::HashMap, num::NonZeroU64, time::Duration};

    use iroha_crypto::HashOf;
    use iroha_crypto::{Algorithm, bls_normal_pop_prove};
    use iroha_data_model::{
        NetworkId,
        block::consensus_v2::{SumeragiV2GenesisContextParameters, ValidatorPower},
        parameter::{Parameter, system::SumeragiParameter},
        prelude::*,
    };
    use iroha_data_model::{
        isi::Log,
        transaction::{FeePaymentIntent, TransactionEntrypoint},
    };
    use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry};
    use iroha_model_base::{chain::ChainId, peer::PeerId};
    use iroha_primitives::time::TimeSource;
    use iroha_test_samples::{
        ALICE_ID, ALICE_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
    };

    use super::*;
    use crate::{
        governance::manifest::LaneManifestRegistry, query::store::LiveQueryStore, state::World,
        sumeragi::driver::traits::Frame, tx::AcceptedTransaction,
    };
    use iroha_sumeragi::types::PublicKey as CoreKey;

    /// The in-memory transport: every node's handle, filled once the nodes started (frames
    /// sent before are dropped, and the core rebroadcasts).
    #[derive(Default)]
    struct Registry(parking_lot::Mutex<HashMap<CoreKey, DriverHandle>>);

    struct MemNet {
        from: CoreKey,
        registry: Arc<Registry>,
    }

    impl Net for MemNet {
        fn send(&self, to: &CoreKey, frame: &Frame) {
            let handle = self.registry.0.lock().get(to).cloned();
            if let Some(handle) = handle {
                handle.deliver(&self.from, &frame.bytes);
            }
        }
    }

    /// Prints what the instance reports (shown on failure).
    struct PrintObserver(usize);

    impl Observer for PrintObserver {
        fn evidence(&self, evidence: &iroha_sumeragi::message::Evidence) {
            eprintln!("node {}: evidence {evidence:?}", self.0);
        }
        fn fault(&self, fault: &iroha_sumeragi::api::LocalFault) {
            eprintln!("node {}: fault {fault:?}", self.0);
        }
        fn halt(&self, reason: &iroha_sumeragi::api::HaltReason) {
            eprintln!("node {}: halt {reason:?}", self.0);
        }
        fn stopped(&self, worker: crate::sumeragi::driver::Worker) {
            eprintln!("node {}: stopped {worker:?}", self.0);
        }
    }

    struct Chain {
        genesis: SignedBlock,
        keys: Vec<KeyPair>,
        chain_id: ChainId,
    }

    /// A chain of `validators` with a 100 ms block time and the given idle interval.
    fn chain(validators: u8, payload_retry_interval_ms: u64) -> Chain {
        iroha_genesis::init_instruction_registry();
        let chain_id = ChainId::from("sumeragi-node-test");
        let mut keys = (0..validators)
            .map(|index| {
                KeyPair::try_from_seed(vec![0xC0 + index; 32], Algorithm::BlsNormal).expect("key")
            })
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
        let entries = keys
            .iter()
            .map(|key| {
                GenesisTopologyEntry::new(
                    PeerId::new(key.public_key().clone()),
                    bls_normal_pop_prove(key.private_key()).expect("pop"),
                )
            })
            .collect::<Vec<_>>();
        let roster = entries
            .iter()
            .map(|entry| ValidatorPower {
                validator: entry.peer.clone(),
                power: 1,
            })
            .collect::<Vec<_>>();
        // TODO(WP9): the genesis builder drops its v2 context requirement.
        let genesis = GenesisBuilder::new_without_executor(chain_id.clone(), ".")
            .append_parameter(Parameter::Sumeragi(
                SumeragiParameter::PayloadRetryIntervalMs(
                    NonZeroU64::new(payload_retry_interval_ms).expect("non-zero"),
                ),
            ))
            .with_block_cadence_ms(NonZeroU64::new(100).expect("non-zero"))
            .set_topology(entries)
            .with_sumeragi_v2_context_parameters(SumeragiV2GenesisContextParameters::recommended())
            .with_kagemusha_mint_finality_genesis_parameters(
                crate::kagemusha_v1_test_fixtures::mint_finality_genesis_parameters(&roster),
            )
            .build_and_sign(&SAMPLE_GENESIS_ACCOUNT_KEYPAIR)
            .expect("genesis")
            .0;
        Chain {
            genesis,
            keys,
            chain_id,
        }
    }

    fn empty_state(chain_id: &ChainId, genesis: &SignedBlock, kura: &Arc<Kura>) -> Arc<State> {
        let account = SAMPLE_GENESIS_ACCOUNT_ID.clone();
        let world = World::with(
            [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&account)],
            [
                Account::new(account.clone()).build(&account),
                Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            ],
            [],
        );
        let state = Arc::new(State::new_with_chain_and_network_id_for_testing(
            world,
            Arc::clone(kura),
            LiveQueryStore::start_test(),
            chain_id.clone(),
            NetworkId::from_genesis_hash(genesis.hash()),
        ));
        let nexus = state.nexus_snapshot();
        state.install_lane_manifests(&Arc::new(
            LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
        ));
        state
    }

    /// One validator's durable parts: Kura and its directory (records, bodies).
    struct Disk {
        kura: Arc<Kura>,
        dir: tempfile::TempDir,
    }

    fn disks(chain: &Chain) -> Vec<Disk> {
        (0..chain.keys.len())
            .map(|_| Disk {
                kura: Kura::blank_kura_for_testing(),
                dir: tempfile::tempdir().expect("tempdir"),
            })
            .collect()
    }

    /// A running validator and the parts the test drives.
    struct Validator {
        node: RunningNode,
        state: Arc<State>,
        queue: Arc<Queue>,
    }

    fn start_all(chain: &Chain, disks: &[Disk], fresh: bool) -> Vec<Validator> {
        let registry = Arc::new(Registry::default());
        let validators = disks
            .iter()
            .enumerate()
            .map(|(index, disk)| {
                let key_pair = chain.keys[index].clone();
                let (_, time_source) = TimeSource::new_mock(Duration::ZERO);
                let state = empty_state(&chain.chain_id, &chain.genesis, &disk.kura);
                let queue = Arc::new(Queue::test(
                    iroha_config::parameters::actual::Queue::default(),
                    &time_source,
                ));
                let node = start(NodeInputs {
                    state: Arc::clone(&state),
                    kura: Arc::clone(&disk.kura),
                    queue: Arc::clone(&queue),
                    events: tokio::sync::broadcast::channel(1024).0,
                    net: Arc::new(MemNet {
                        from: core_key(key_pair.public_key()).expect("BLS key"),
                        registry: Arc::clone(&registry),
                    }),
                    key_pair,
                    chain_id: chain.chain_id.to_string(),
                    genesis: Some(chain.genesis.clone()),
                    genesis_account: SAMPLE_GENESIS_ACCOUNT_ID.clone(),
                    consensus_mode: ConsensusMode::Permissioned,
                    config: NodeConfig {
                        records_dir: disk.dir.path().join("records"),
                        installation_log: disk.dir.path().join("keys").join("installation.log"),
                        bodies_dir: disk.dir.path().join("bodies"),
                        local: SumeragiLocalOverrides::default(),
                        assert_fresh_key: fresh,
                        retired_keys: Vec::new(),
                    },
                    observer: Arc::new(PrintObserver(index)),
                    driver: DriverConfig::default(),
                })
                .expect("start");
                Validator { node, state, queue }
            })
            .collect::<Vec<_>>();
        let mut peers = registry.0.lock();
        for (key, validator) in chain.keys.iter().zip(&validators) {
            peers.insert(
                core_key(key.public_key()).expect("BLS key"),
                validator.node.driver.handle(),
            );
        }
        drop(peers);
        validators
    }

    fn shutdown(validators: Vec<Validator>) {
        for validator in validators {
            validator.node.driver.shutdown();
        }
    }

    fn committed_heights(validators: &[Validator]) -> Vec<u64> {
        validators
            .iter()
            .map(|validator| {
                validator
                    .node
                    .driver
                    .handle()
                    .status()
                    .map_or(0, |status| status.committed_height)
            })
            .collect()
    }

    /// Wait until `done` holds, failing on a halt or after `limit`.
    fn wait_until(
        validators: &[Validator],
        limit: Duration,
        what: &str,
        mut done: impl FnMut() -> bool,
    ) {
        let start = std::time::Instant::now();
        while !done() {
            for validator in validators {
                let halted = validator.node.driver.handle().halted();
                assert!(halted.is_none(), "halted: {halted:?}");
            }
            if start.elapsed() >= limit {
                for validator in validators {
                    eprintln!("{:#?}", validator.node.driver.handle().status());
                    let view = validator.state.view();
                    for height in 2..=view.height() {
                        let block = validator
                            .state
                            .kura()
                            .get_block(core::num::NonZeroUsize::new(height).expect("non-zero"))
                            .expect("stored");
                        eprintln!(
                            "  block {height}: {} entrypoints, queued {}",
                            block.network_entrypoint_count(),
                            validator.queue.queued_len()
                        );
                    }
                }
                panic!(
                    "{what}: not reached in {limit:?} (heights {:?})",
                    committed_heights(validators)
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn assert_idle_height_unchanged(validators: &[Validator], duration: Duration) {
        let baseline = committed_heights(validators);
        let started = std::time::Instant::now();
        while started.elapsed() < duration {
            assert_eq!(
                committed_heights(validators),
                baseline,
                "idle chain advanced"
            );
            for validator in validators {
                assert!(validator.node.driver.handle().halted().is_none());
                assert_eq!(validator.queue.queued_len(), 0);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Submit a transaction to every validator's queue (the transaction gossip's job in the
    /// node) and tell the drivers.
    fn submit(
        chain: &Chain,
        validators: &[Validator],
        message: &str,
    ) -> HashOf<TransactionEntrypoint> {
        let network_id = NetworkId::from_genesis_hash(chain.genesis.hash());
        let signed = TransactionBuilder::new(
            network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, message.to_owned())])
        .sign(ALICE_KEYPAIR.private_key());
        let accepted = AcceptedTransaction::accept(
            signed,
            &network_id,
            Duration::from_secs(10),
            TransactionParameters::default(),
            &iroha_config::parameters::actual::Crypto::default(),
        )
        .expect("accepted");
        let hash = accepted.hash_as_entrypoint();
        for validator in validators {
            validator
                .queue
                .push(accepted.clone(), validator.state.view())
                .expect("queued");
            validator.node.driver.handle().transactions_available();
        }
        hash
    }

    fn committed_everywhere(validators: &[Validator], hash: HashOf<TransactionEntrypoint>) -> bool {
        validators.iter().all(|validator| {
            validator.state.has_committed_entrypoint(hash)
                && validator
                    .node
                    .driver
                    .handle()
                    .status()
                    .is_some_and(|status| {
                        status.applied_height
                            == u64::try_from(validator.state.view().height()).unwrap()
                    })
        })
    }

    /// Every validator stored the same blocks up to `height`, each with its commit certificate
    /// (genesis with its result-only one).
    fn assert_same_certified_blocks(disks: &[Disk], height: usize) {
        for height in 1..=height {
            let height = core::num::NonZeroUsize::new(height).expect("non-zero");
            let blocks = disks
                .iter()
                .map(|disk| disk.kura.get_block(height).expect("stored"))
                .collect::<Vec<_>>();
            for block in &blocks {
                assert!(block.commit_certificate().is_some(), "height {height}");
                assert_eq!(block.hash(), blocks[0].hash(), "height {height}");
                assert!(
                    block.network_entrypoint_count() > 0,
                    "empty block at {height}"
                );
            }
        }
    }

    #[test]
    fn idle_chain_never_advances_and_real_work_survives_restart() {
        let chain = chain(4, 200);
        let disks = disks(&chain);
        let validators = start_all(&chain, &disks, true);
        assert_eq!(committed_heights(&validators), vec![GENESIS_HEIGHT; 4]);
        assert_idle_height_unchanged(&validators, Duration::from_millis(750));
        let hash = submit(&chain, &validators, "after idle");
        wait_until(
            &validators,
            Duration::from_secs(30),
            "real work after idle",
            || committed_everywhere(&validators, hash),
        );
        assert_idle_height_unchanged(&validators, Duration::from_millis(750));
        shutdown(validators);
        let committed = disks
            .iter()
            .map(|disk| disk.kura.blocks_count())
            .min()
            .expect("validators");
        assert_eq!(committed, 2, "only genesis and the submitted transaction");
        assert_same_certified_blocks(&disks, committed);
        // Replay the exact retained history, remain idle, then accept new work.
        let validators = start_all(&chain, &disks, false);
        assert_idle_height_unchanged(&validators, Duration::from_millis(750));
        let hash = submit(&chain, &validators, "after idle restart");
        wait_until(
            &validators,
            Duration::from_secs(30),
            "real work after restart",
            || committed_everywhere(&validators, hash),
        );
        assert_eq!(committed_heights(&validators), vec![3; 4]);
        shutdown(validators);
        assert_same_certified_blocks(&disks, 3);
    }

    /// Replay checks every stored block against its certified result (§12.1): a certificate
    /// whose result differs from re-execution stops the replay.
    #[test]
    fn replay_rejects_a_block_whose_certified_result_differs() {
        let chain = chain(4, 200);
        let disks = disks(&chain);
        let validators = start_all(&chain, &disks, true);
        let hash = submit(&chain, &validators, "certified replay input");
        wait_until(
            &validators,
            Duration::from_secs(30),
            "replay input committed",
            || committed_everywhere(&validators, hash),
        );
        shutdown(validators);
        let kura = Arc::clone(&disks[0].kura);
        let state = empty_state(&chain.chain_id, &chain.genesis, &kura);
        let (genesis, certificate, _) = startup::stored_genesis(&state).expect("stored genesis");
        let tip = startup::apply_genesis(
            &state,
            genesis,
            &SAMPLE_GENESIS_ACCOUNT_ID,
            ConsensusMode::Permissioned,
            Some(&certificate),
        )
        .expect("genesis re-executes");
        let crypto = Arc::new(BlsCrypto::new());
        let shared: SharedCrypto = crypto.clone();
        let staging = Staging::new();
        let blocks = KuraBlockStore::new(kura, shared, GENESIS_HEIGHT, staging.clone());
        let mut executor = StateExecutor::spawn(ExecutorContext {
            state: Arc::clone(&state),
            queue: None,
            staging,
            events: tokio::sync::broadcast::channel(16).0,
            genesis_account: SAMPLE_GENESIS_ACCOUNT_ID.clone(),
            consensus_mode: ConsensusMode::Permissioned,
            applied: (GENESIS_HEIGHT, tip.block_hash),
            crypto: Some(Arc::clone(&crypto)),
            applied_watch: Arc::new(crate::sumeragi::lanes::global::AppliedWatch::new(
                GENESIS_HEIGHT,
                state.view().latest_block_hash(),
            )),
        })
        .expect("executor");
        admit_window(&state, &crypto, GENESIS_HEIGHT);
        let entry = blocks.entry(2).expect("height 2 stored");
        let mut forged = entry.commit_qc.clone();
        forged.result = Hash32([0xAB; 32]);
        let error = executor
            .replay(&entry.block, &forged)
            .expect_err("a differing certified result");
        assert!(error.contains("diverges"), "{error}");
        assert_eq!(startup::applied_height(&state), GENESIS_HEIGHT);
        // The stored certificate replays.
        executor
            .replay(&entry.block, &entry.commit_qc)
            .expect("the certified result reproduces");
        assert_eq!(startup::applied_height(&state), 2);
    }

    #[test]
    fn every_committed_block_contains_work_before_and_after_restart() {
        // Explicit queue notifications must bypass even a long rebuild retry interval.
        let chain = chain(4, 600_000);
        let disks = disks(&chain);
        let validators = start_all(&chain, &disks, true);
        for index in 0..3 {
            let hash = submit(&chain, &validators, &format!("before restart {index}"));
            wait_until(
                &validators,
                Duration::from_secs(30),
                "transaction committed",
                || committed_everywhere(&validators, hash),
            );
        }
        let heights = committed_heights(&validators);
        assert_eq!(heights, vec![4; 4]);
        // Applied transactions leave every queue.
        wait_until(
            &validators,
            Duration::from_secs(10),
            "queues drained",
            || {
                validators
                    .iter()
                    .all(|validator| validator.queue.queued_len() == 0)
            },
        );
        shutdown(validators);
        let committed = disks
            .iter()
            .map(|disk| disk.kura.blocks_count())
            .min()
            .expect("validators");
        assert_same_certified_blocks(&disks, committed);
        let validators = start_all(&chain, &disks, false);
        let hash = submit(&chain, &validators, "after restart");
        wait_until(
            &validators,
            Duration::from_secs(30),
            "transaction committed",
            || committed_everywhere(&validators, hash),
        );
        shutdown(validators);
        assert_same_certified_blocks(&disks, 5);
    }
}
