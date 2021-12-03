//! This module contains structures and messages for synchronization of blocks between peers.

use std::{fmt::Debug, sync::Arc, time::Duration};

use iroha_actor::{broker::*, prelude::*, Context};
use iroha_crypto::SignatureOf;
use iroha_data_model::prelude::*;
use iroha_logger::prelude::*;
use rand::{prelude::SliceRandom, SeedableRng};

use self::{
    config::BlockSyncConfiguration,
    message::{Message, *},
};
use crate::{
    prelude::*,
    sumeragi::{network_topology::Role, CommitBlock, GetNetworkTopology, SumeragiTrait},
    wsv::WorldTrait,
    VersionedCommittedBlock,
};

/// The state of `BlockSynchronizer`.
#[derive(Clone, Debug)]
enum State {
    /// Not synchronizing now.
    Idle,
    /// Synchronization is in progress: validating and committing blocks.
    /// Contains a vector of blocks left to commit and an id of the peer from which the blocks were requested.
    InProgress(Vec<VersionedCommittedBlock>, PeerId),
}

/// Structure responsible for block synchronization between peers.
#[derive(Debug)]
pub struct BlockSynchronizer<S: SumeragiTrait, W: WorldTrait> {
    wsv: Arc<WorldStateView<W>>,
    sumeragi: AlwaysAddr<S>,
    peer_id: PeerId,
    state: State,
    gossip_period: Duration,
    batch_size: u32,
    broker: Broker,
    mailbox: usize,
}

/// Block synchronizer
pub trait BlockSynchronizerTrait: Actor + Handler<ContinueSync> + Handler<Message> {
    /// Requires sumeragi for sending direct messages to it
    type Sumeragi: SumeragiTrait;
    /// Requires world to read latest blocks commited
    type World: WorldTrait;

    /// Constructs `BlockSync`
    fn from_configuration(
        config: &BlockSyncConfiguration,
        wsv: Arc<WorldStateView<Self::World>>,
        sumeragi: AlwaysAddr<Self::Sumeragi>,
        peer_id: PeerId,
        broker: Broker,
    ) -> Self;
}

impl<S: SumeragiTrait, W: WorldTrait> BlockSynchronizerTrait for BlockSynchronizer<S, W> {
    type Sumeragi = S;
    type World = W;

    fn from_configuration(
        config: &BlockSyncConfiguration,
        wsv: Arc<WorldStateView<W>>,
        sumeragi: AlwaysAddr<S>,
        peer_id: PeerId,
        broker: Broker,
    ) -> Self {
        Self {
            wsv,
            peer_id,
            sumeragi,
            state: State::Idle,
            gossip_period: Duration::from_millis(config.gossip_period_ms),
            batch_size: config.batch_size,
            broker,
            mailbox: config.mailbox,
        }
    }
}

/// Message to send to block synchronizer. It will call `continue_sync` method on it
#[derive(Debug, Clone, Copy, iroha_actor::Message)]
pub struct ContinueSync;

/// Message to initiate receiving of latest blocks from other peers
///
/// Every `gossip_period` peer will poll one randomly selected peer for latest blocks
#[derive(Debug, Clone, Copy, Default, iroha_actor::Message)]
pub struct ReceiveUpdates;

#[async_trait::async_trait]
impl<S: SumeragiTrait, W: WorldTrait> Actor for BlockSynchronizer<S, W> {
    fn mailbox_capacity(&self) -> usize {
        self.mailbox
    }

    async fn on_start(&mut self, ctx: &mut Context<Self>) {
        self.broker.subscribe::<Message, _>(ctx);
        self.broker.subscribe::<ContinueSync, _>(ctx);
        ctx.notify_every::<ReceiveUpdates>(self.gossip_period);
    }
}

#[async_trait::async_trait]
impl<S: SumeragiTrait, W: WorldTrait> Handler<ReceiveUpdates> for BlockSynchronizer<S, W> {
    type Result = ();
    async fn handle(&mut self, ReceiveUpdates: ReceiveUpdates) {
        let rng = &mut rand::rngs::StdRng::from_entropy();

        if let Some(random_peer) = self.wsv.peers().choose(rng) {
            self.request_latest_blocks_from_peer(random_peer.id.clone())
                .await;
        }
    }
}

#[async_trait::async_trait]
impl<S: SumeragiTrait, W: WorldTrait> Handler<ContinueSync> for BlockSynchronizer<S, W> {
    type Result = ();
    async fn handle(&mut self, ContinueSync: ContinueSync) {
        self.continue_sync().await;
    }
}

#[async_trait::async_trait]
impl<S: SumeragiTrait, W: WorldTrait> Handler<Message> for BlockSynchronizer<S, W> {
    type Result = ();
    async fn handle(&mut self, message: Message) {
        message.handle(self).await;
    }
}

impl<S: SumeragiTrait + Debug, W: WorldTrait> BlockSynchronizer<S, W> {
    /// Sends request for latest blocks to a chosen peer
    async fn request_latest_blocks_from_peer(&mut self, peer_id: PeerId) {
        Message::GetBlocksAfter(GetBlocksAfter::new(
            self.wsv.latest_block_hash(),
            self.peer_id.clone(),
        ))
        .send_to(self.broker.clone(), peer_id)
        .await;
    }

    /// Continues the synchronization if it was ongoing. Should be called after `WSV` update.
    #[iroha_futures::telemetry_future]
    pub async fn continue_sync(&mut self) {
        let (blocks, peer_id) = if let State::InProgress(blocks, peer_id) = self.state.clone() {
            (blocks, peer_id)
        } else {
            return;
        };

        info!(blocks_left = blocks.len(), "Synchronizing blocks");

        let (this_block, remaining_blocks) = if let Some((blck, blcks)) = blocks.split_first() {
            (blck, blcks)
        } else {
            self.state = State::Idle;
            self.request_latest_blocks_from_peer(peer_id).await;
            return;
        };

        let mut network_topology = self
            .sumeragi
            .send(GetNetworkTopology(this_block.header().clone()))
            .await;
        // If it is genesis topology we cannot apply view changes as peers have custom order!
        #[allow(clippy::expect_used)]
        if !this_block.header().is_genesis() {
            network_topology = network_topology
                .into_builder()
                .with_view_changes(this_block.header().view_change_proofs.clone())
                .build()
                .expect(
                    "Unreachable as doing view changes on valid topology will not raise an error.",
                );
        }
        if self.wsv.as_ref().latest_block_hash() == this_block.header().previous_block_hash
            && network_topology
                .filter_signatures_by_roles(
                    &[Role::ValidatingPeer, Role::Leader, Role::ProxyTail],
                    this_block
                        .verified_signatures()
                        .map(SignatureOf::transmute_ref),
                )
                .len()
                >= network_topology.min_votes_for_commit() as usize
        {
            self.state = State::InProgress(remaining_blocks.to_vec(), peer_id);
            self.sumeragi
                .do_send(CommitBlock(this_block.clone().into()))
                .await;
        } else {
            warn!(block_hash = %this_block.hash(), "Failed to commit a block received via synchronization request - validation failed");
            self.state = State::Idle;
        }
    }
}

/// The module for block synchronization related peer to peer messages.
pub mod message {
    use iroha_actor::broker::Broker;
    use iroha_crypto::*;
    use iroha_data_model::prelude::*;
    use iroha_logger::prelude::*;
    use iroha_macro::*;
    use iroha_p2p::Post;
    use iroha_version::prelude::*;
    use parity_scale_codec::{Decode, Encode};

    use super::{BlockSynchronizer, State};
    use crate::{
        block::VersionedCommittedBlock, sumeragi::SumeragiTrait, wsv::WorldTrait, NetworkMessage,
    };

    declare_versioned_with_scale!(VersionedMessage 1..2, Debug, Clone, iroha_macro::FromVariant, iroha_actor::Message);

    impl VersionedMessage {
        /// Same as [`as_v1`](`VersionedMessage::as_v1()`) but also does conversion
        pub const fn as_inner_v1(&self) -> &Message {
            match self {
                Self::V1(v1) => &v1.0,
            }
        }

        /// Same as [`as_inner_v1`](`VersionedMessage::as_inner_v1()`) but returns mutable reference
        pub fn as_mut_inner_v1(&mut self) -> &mut Message {
            match self {
                Self::V1(v1) => &mut v1.0,
            }
        }

        /// Same as [`into_v1`](`VersionedMessage::into_v1()`) but also does conversion
        pub fn into_inner_v1(self) -> Message {
            match self {
                Self::V1(v1) => v1.0,
            }
        }
    }

    /// Get blocks after some block
    #[derive(Io, Decode, Encode, Debug, Clone)]
    pub struct GetBlocksAfter {
        /// Block hash
        pub hash: HashOf<VersionedCommittedBlock>,
        /// Peer id
        pub peer_id: PeerId,
    }

    impl GetBlocksAfter {
        /// Default constructor
        pub const fn new(hash: HashOf<VersionedCommittedBlock>, peer_id: PeerId) -> Self {
            Self { hash, peer_id }
        }
    }

    /// Message variant to share blocks to peer
    #[derive(Io, Decode, Encode, Debug, Clone)]
    pub struct ShareBlocks {
        /// Blocks
        pub blocks: Vec<VersionedCommittedBlock>,
        /// Peer id
        pub peer_id: PeerId,
    }

    impl ShareBlocks {
        /// Default constructor
        pub const fn new(blocks: Vec<VersionedCommittedBlock>, peer_id: PeerId) -> Self {
            Self { blocks, peer_id }
        }
    }

    /// Message's variants that are used by peers to communicate in the process of consensus.
    #[version_with_scale(n = 1, versioned = "VersionedMessage", derive = "Debug, Clone")]
    #[derive(Io, Decode, Encode, Debug, Clone, FromVariant, iroha_actor::Message)]
    pub enum Message {
        /// Request for blocks after the block with `Hash` for the peer with `PeerId`.
        GetBlocksAfter(GetBlocksAfter),
        /// The response to `GetBlocksAfter`. Contains the requested blocks and the id of the peer who shared them.
        ShareBlocks(ShareBlocks),
    }

    impl Message {
        /// Handles the incoming message.
        #[iroha_futures::telemetry_future]
        pub async fn handle<S: SumeragiTrait, W: WorldTrait>(
            &self,
            block_sync: &mut BlockSynchronizer<S, W>,
        ) {
            match self {
                Message::GetBlocksAfter(GetBlocksAfter { hash, peer_id }) => {
                    if block_sync.batch_size == 0 {
                        warn!("Error: not sending any blocks as batch_size is equal to zero.");
                        return;
                    }
                    if *hash == block_sync.wsv.latest_block_hash() {
                        return;
                    }

                    let blocks = block_sync.wsv.blocks_after(*hash, block_sync.batch_size);
                    if blocks.is_empty() {
                        warn!(%hash, "Block hash not found");
                    } else {
                        Message::ShareBlocks(ShareBlocks::new(blocks, block_sync.peer_id.clone()))
                            .send_to(block_sync.broker.clone(), peer_id.clone())
                            .await;
                    }
                }
                Message::ShareBlocks(ShareBlocks { blocks, peer_id }) => {
                    if let State::Idle = block_sync.state.clone() {
                        block_sync.state = State::InProgress(blocks.clone(), peer_id.clone());
                        block_sync.continue_sync().await;
                    }
                }
            }
        }

        /// Send this message over the network to the specified `peer`.
        #[iroha_futures::telemetry_future]
        #[log("TRACE")]
        pub async fn send_to(self, broker: Broker, peer: PeerId) {
            let data = NetworkMessage::BlockSync(Box::new(VersionedMessage::from(self)));
            let message = Post {
                data,
                peer: peer.clone(),
            };
            broker.issue_send(message).await;
        }
    }
}

/// This module contains all configuration related logic.
pub mod config {
    use iroha_config::derive::Configurable;
    use serde::{Deserialize, Serialize};

    const DEFAULT_BATCH_SIZE: u32 = 4;
    const DEFAULT_GOSSIP_PERIOD_MS: u64 = 10000;
    const DEFAULT_MAILBOX_SIZE: usize = 100;

    /// Configuration for `BlockSynchronizer`.
    #[derive(Copy, Clone, Deserialize, Serialize, Debug, Configurable, PartialEq, Eq)]
    #[serde(rename_all = "UPPERCASE")]
    #[serde(default)]
    #[config(env_prefix = "BLOCK_SYNC_")]
    pub struct BlockSyncConfiguration {
        /// The time between sending request for latest block.
        pub gossip_period_ms: u64,
        /// The number of blocks, which can be sent in one message.
        /// Underlying network (`iroha_network`) should support transferring messages this large.
        pub batch_size: u32,
        /// Mailbox size
        pub mailbox: usize,
    }

    impl Default for BlockSyncConfiguration {
        fn default() -> Self {
            Self {
                gossip_period_ms: DEFAULT_GOSSIP_PERIOD_MS,
                batch_size: DEFAULT_BATCH_SIZE,
                mailbox: DEFAULT_MAILBOX_SIZE,
            }
        }
    }
}
