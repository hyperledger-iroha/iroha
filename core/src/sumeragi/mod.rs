//! Translates to Emperor. Consensus-related logic of Iroha.
//!
//! `Consensus` trait is now implemented only by `Sumeragi` for now.
#![allow(
    clippy::arithmetic,
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc
)]
use std::{
    collections::{hash_map::Entry, BTreeMap, HashMap, HashSet},
    fmt::{self, Debug, Formatter},
    marker::PhantomData,
    sync::Arc,
    time::{Duration, Instant},
};

use eyre::{eyre, Result};
use iroha_config::sumeragi::Configuration;
use iroha_actor::broker::Broker;
use iroha_crypto::{HashOf, KeyPair};
use iroha_data_model::prelude::*;
use iroha_logger::prelude::*;
use iroha_p2p::{ConnectPeer, DisconnectPeer};
use network_topology::{Role, Topology};
use rand::prelude::SliceRandom;

use crate::{genesis::GenesisNetwork};
use iroha_config::sumeragi::Configuration as SumeragiConfiguration;

pub mod fault;
pub mod message;
pub mod network_topology;
pub mod view_change;

use self::{
    fault::{NoFault, SumeragiWithFault},
    message::{Message, *},
    view_change::{Proof, ProofChain as ViewChangeProofs},
};
use crate::{
    block::{BlockHeader, ChainedBlock, EmptyChainHash, VersionedPendingBlock},
    genesis::GenesisNetworkTrait,
    kura::Kura,
    prelude::*,
    queue::Queue,
    send_event,
    tx::TransactionValidator,
    EventsSender, IrohaNetwork, NetworkMessage, VersionedValidBlock,
};

trait Consensus {
    fn round(
        &mut self,
        transactions: Vec<VersionedAcceptedTransaction>,
    ) -> Option<VersionedPendingBlock>;
}

/// `Sumeragi` is the implementation of the consensus.
#[derive(Debug)]
pub struct Sumeragi {
    internal: SumeragiWithFault<NoFault>,
}

impl Sumeragi {
    /// Construct [`Sumeragi`].
    ///
    /// # Errors
    /// Can fail during initing network topology
    #[allow(clippy::too_many_arguments)]
    pub fn from_configuration(
        configuration: &Configuration,
        events_sender: EventsSender,
        wsv: WorldStateView,
        transaction_validator: TransactionValidator,
        telemetry_started: bool,
        genesis_network: Option<GenesisNetwork>,
        queue: Arc<Queue>,
        broker: Broker,
        kura: Arc<Kura>,
        // network: Addr<IrohaNetwork>,
    ) -> Result<Self> {
        let network_topology = Topology::builder()
            .at_block(EmptyChainHash::default().into())
            .with_peers(configuration.trusted_peers.peers.clone())
            .build()?;

        Ok(Self {
            internal: SumeragiWithFault::<NoFault> {
                key_pair: configuration.key_pair.clone(),
                topology: network_topology,
                peer_id: configuration.peer_id.clone(),
                voting_block: None,
                votes_for_blocks: BTreeMap::new(),
                events_sender,
                wsv: std::sync::Mutex::new(wsv),
                txs_awaiting_receipts: HashMap::new(),
                txs_awaiting_created_block: HashSet::new(),
                votes_for_view_change: HashMap::new(),
                commit_time: Duration::from_millis(configuration.commit_time_limit_ms),
                tx_receipt_time: Duration::from_millis(configuration.tx_receipt_time_limit_ms),
                block_time: Duration::from_millis(configuration.block_time_ms),
                block_height: 0,
                invalidated_blocks_hashes: Vec::new(),
                telemetry_started,
                transaction_limits: configuration.transaction_limits,
                transaction_validator,
                genesis_network,
                queue,
                broker,
                kura,
                // network,
                actor_channel_capacity: configuration.actor_channel_capacity,
                fault_injection: PhantomData,
                gossip_batch_size: configuration.gossip_batch_size,
                gossip_period: Duration::from_millis(configuration.gossip_period_ms),
            },
        })
    }

    pub fn latest_block_hash(&self) -> HashOf<VersionedCommittedBlock> {
        self.internal.wsv.lock().unwrap().latest_block_hash()
    }

    pub fn get_network_topology(&self, header: &BlockHeader) -> Topology {
        self.internal.get_network_topology(header)
    }

    pub fn blocks_after_hash(
        &self,
        block_hash: HashOf<VersionedCommittedBlock>,
    ) -> Vec<VersionedCommittedBlock> {
        self.internal
            .wsv
            .lock()
            .unwrap()
            .blocks_after_hash(block_hash)
            .collect()
    }

    pub fn get_random_peer_for_block_sync(&self) -> Option<Peer> {
        use rand::{prelude::SliceRandom, SeedableRng};

        let rng = &mut rand::rngs::StdRng::from_entropy();
        self.internal
            .wsv
            .lock()
            .unwrap()
            .peers()
            .choose(rng)
            .cloned()
    }

    pub fn get_clone_of_world_state_view(&self) -> WorldStateView {
        self.internal.wsv.lock().unwrap().clone()
    }

    pub fn initialize_and_start_thread(
        sumeragi: Arc<Self>,
        latest_block_hash: HashOf<VersionedCommittedBlock>,
        latest_block_height: u64,
    ) {
        std::thread::spawn(move || {
            fault::run_sumeragi_main_loop(
                &sumeragi.internal,
                latest_block_hash,
                latest_block_height,
            )
        });
    }
}

/// The interval at which sumeragi checks if there are tx in the
/// `queue`.  And will create a block if is leader and the voting is
/// not already in progress.
pub const TX_RETRIEVAL_INTERVAL: Duration = Duration::from_millis(200);
/// The interval of peers (re/dis)connection.
pub const PEERS_CONNECT_INTERVAL: Duration = Duration::from_secs(1);
/// The interval of telemetry updates.
pub const TELEMETRY_INTERVAL: Duration = Duration::from_secs(5);

/// Structure represents a block that is currently in discussion.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct VotingBlock {
    /// At what time has this peer voted for this block
    pub voted_at: Duration,
    /// Valid Block
    pub block: VersionedValidBlock,
}

impl VotingBlock {
    /// Constructs new `VotingBlock.`
    #[allow(clippy::expect_used)]
    pub fn new(block: VersionedValidBlock) -> VotingBlock {
        VotingBlock {
            voted_at: current_time(),
            block,
        }
    }
}
