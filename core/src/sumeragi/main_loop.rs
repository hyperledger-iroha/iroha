//! The main event loop that powers sumeragi.
use std::{collections::BTreeSet, sync::mpsc};

use iroha_crypto::{HashOf, KeyPair};
use iroha_data_model::{block::*, events::pipeline::PipelineEventBox, peer::PeerId};
use iroha_p2p::UpdateTopology;
use tracing::{span, Level};

use super::{view_change::ProofBuilder, *};
use crate::{block::*, sumeragi::tracing::instrument};

/// `Sumeragi` is the implementation of the consensus.
pub struct Sumeragi {
    /// Unique id of the blockchain. Used for simple replay attack protection.
    pub chain_id: ChainId,
    /// The pair of keys used for communication given this Sumeragi instance.
    pub key_pair: KeyPair,
    /// Address of queue
    pub queue: Arc<Queue>,
    /// The peer id of myself.
    pub peer_id: PeerId,
    /// An actor that sends events
    pub events_sender: EventsSender,
    /// Time by which a newly created block should be committed. Prevents malicious nodes
    /// from stalling the network by not participating in consensus
    pub commit_time: Duration,
    /// Time by which a new block should be created regardless if there were enough transactions or not.
    /// Used to force block commits when there is a small influx of new transactions.
    pub block_time: Duration,
    /// The maximum number of transactions in the block
    pub max_txs_in_block: usize,
    /// Kura instance used for IO
    pub kura: Arc<Kura>,
    /// [`iroha_p2p::Network`] actor address
    pub network: IrohaNetwork,
    /// Receiver channel, for control flow messages.
    pub control_message_receiver: mpsc::Receiver<ControlFlowMessage>,
    /// Receiver channel.
    pub message_receiver: mpsc::Receiver<BlockMessage>,
    /// Only used in testing. Causes the genesis peer to withhold blocks when it
    /// is the proxy tail.
    pub debug_force_soft_fork: bool,
    /// The current network topology.
    pub topology: Topology,
    /// In order to *be fast*, we must minimize communication with
    /// other subsystems where we can. This way the performance of
    /// sumeragi is more dependent on the code that is internal to the
    /// subsystem.
    pub transaction_cache: Vec<AcceptedTransaction>,
    /// Metrics for reporting number of view changes in current round
    pub view_changes_metric: iroha_telemetry::metrics::ViewChangesGauge,

    /// Was there a commit in previous round?
    pub was_commit: bool,
    /// Instant when the current round started
    // NOTE: Round is only restarted on a block commit, so that in the case of
    // a view change a new block is immediately created by the leader
    pub round_start_time: Instant,
}

#[allow(clippy::missing_fields_in_debug)]
impl Debug for Sumeragi {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sumeragi")
            .field("public_key", &self.key_pair.public_key())
            .field("peer_id", &self.peer_id)
            .finish()
    }
}

impl Sumeragi {
    fn role(&self) -> Role {
        self.topology.role(&self.peer_id)
    }

    /// Send a sumeragi packet over the network to the specified `peer`.
    /// # Errors
    /// Fails if network sending fails
    #[instrument(skip(self, packet))]
    fn post_packet_to(&self, packet: BlockMessage, peer: &PeerId) {
        if peer == &self.peer_id {
            return;
        }

        let post = iroha_p2p::Post {
            data: NetworkMessage::SumeragiBlock(Box::new(packet)),
            peer_id: peer.clone(),
        };
        self.network.post(post);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn broadcast_packet_to<'peer_id, I: IntoIterator<Item = &'peer_id PeerId> + Send>(
        &self,
        msg: impl Into<BlockMessage>,
        ids: I,
    ) {
        let msg = msg.into();

        for peer_id in ids {
            self.post_packet_to(msg.clone(), peer_id);
        }
    }

    fn broadcast_packet(&self, msg: impl Into<BlockMessage>) {
        let broadcast = iroha_p2p::Broadcast {
            data: NetworkMessage::SumeragiBlock(Box::new(msg.into())),
        };
        self.network.broadcast(broadcast);
    }

    fn broadcast_control_flow_packet(&self, msg: ControlFlowMessage) {
        let broadcast = iroha_p2p::Broadcast {
            data: NetworkMessage::SumeragiControlFlow(Box::new(msg)),
        };
        self.network.broadcast(broadcast);
    }

    /// Connect or disconnect peers according to the current network topology.
    fn connect_peers(&self, topology: &Topology) {
        let peers = topology.iter().cloned().collect();
        self.network.update_topology(UpdateTopology(peers));
    }

    /// The maximum time a sumeragi round can take to produce a block when
    /// there are no faulty peers in the a set.
    fn pipeline_time(&self) -> Duration {
        self.block_time + self.commit_time
    }

    fn send_event(&self, event: impl Into<EventBox>) {
        let _ = self.events_sender.send(event.into());
    }

    fn receive_network_packet(
        &self,
        latest_block: HashOf<SignedBlock>,
        view_change_proof_chain: &mut ProofChain,
    ) -> (Option<BlockMessage>, bool) {
        const MAX_CONTROL_MSG_IN_A_ROW: usize = 25;

        let mut should_sleep = true;
        for _ in 0..MAX_CONTROL_MSG_IN_A_ROW {
            if let Ok(msg) = self.control_message_receiver
                .try_recv()
                .map_err(|recv_error| {
                    assert!(
                        recv_error != mpsc::TryRecvError::Disconnected,
                        "Sumeragi control message pump disconnected. This is not a recoverable error."
                    )
                }) {
                should_sleep = false;
                if let Err(error) = view_change_proof_chain.merge(
                    msg.view_change_proofs,
                    &self.topology,
                    latest_block
                ) {
                    trace!(%error, "Failed to add proofs into view change proof chain")
                }
            } else {
                break;
            }
        }

        let block_msg =
            self.receive_block_message_network_packet(latest_block, view_change_proof_chain);

        should_sleep &= block_msg.is_none();
        (block_msg, should_sleep)
    }

    fn receive_block_message_network_packet(
        &self,
        latest_block: HashOf<SignedBlock>,
        view_change_proof_chain: &ProofChain,
    ) -> Option<BlockMessage> {
        let current_view_change_index =
            view_change_proof_chain.verify_with_state(&self.topology, latest_block);

        loop {
            let block_msg = self
                .message_receiver
                .try_recv()
                .map_err(|recv_error| {
                    assert!(
                        recv_error != mpsc::TryRecvError::Disconnected,
                        "Sumeragi message pump disconnected. This is not a recoverable error."
                    )
                })
                .ok()?;

            let block_vc_index = match &block_msg {
                BlockMessage::BlockCreated(bc) => {
                    Some(bc.block.header().view_change_index as usize)
                }
                // Signed and Committed contain no block.
                // Block sync updates are exempt from early pruning.
                BlockMessage::BlockSigned(_)
                | BlockMessage::BlockCommitted(_)
                | BlockMessage::BlockSyncUpdate(_) => None,
            };
            if let Some(block_vc_index) = block_vc_index {
                if block_vc_index < current_view_change_index {
                    // ignore block_message
                    continue;
                }
            }
            return Some(block_msg);
        }
    }

    fn init_listen_for_genesis(
        &mut self,
        genesis_account: &AccountId,
        state: &State,
        shutdown_receiver: &mut tokio::sync::oneshot::Receiver<()>,
    ) -> Result<(), EarlyReturn> {
        info!(
            peer_id=%self.peer_id,
            role=%self.role(),
            "Listening for genesis..."
        );

        loop {
            std::thread::sleep(Duration::from_millis(50));
            early_return(shutdown_receiver).map_err(|e| {
                debug!(?e, "Early return.");
                e
            })?;

            match self.message_receiver.try_recv() {
                Ok(message) => {
                    let block = match message {
                        BlockMessage::BlockCreated(BlockCreated { block })
                        | BlockMessage::BlockSyncUpdate(BlockSyncUpdate { block }) => block,
                        msg => {
                            trace!(?msg, "Not handling the message, waiting for genesis...");
                            continue;
                        }
                    };

                    let mut state_block = state.block();
                    let block = match ValidBlock::validate(
                        block,
                        &self.topology,
                        &self.chain_id,
                        genesis_account,
                        &mut state_block,
                    )
                    .unpack(|e| self.send_event(e))
                    .and_then(|block| {
                        block
                            .commit(&self.topology)
                            .unpack(|e| self.send_event(e))
                            .map_err(|(block, error)| (block.into(), error))
                    }) {
                        Ok(block) => block,
                        Err(error) => {
                            error!(
                                peer_id=%self.peer_id,
                                ?error,
                                "Received invalid genesis block"
                            );

                            continue;
                        }
                    };

                    if block.as_ref().transactions().any(|tx| tx.error.is_some()) {
                        error!(
                            peer_id=%self.peer_id,
                            role=%self.role(),
                            "Genesis contains invalid transactions"
                        );

                        continue;
                    }

                    *state_block.world.trusted_peers_ids =
                        block.as_ref().commit_topology().cloned().collect();
                    self.topology = Topology::new(block.as_ref().commit_topology().cloned());
                    self.commit_block(block, state_block);
                    return Ok(());
                }
                Err(mpsc::TryRecvError::Disconnected) => return Err(EarlyReturn::Disconnected),
                _ => (),
            }
        }
    }

    fn init_commit_genesis(
        &mut self,
        genesis: GenesisBlock,
        genesis_account: &AccountId,
        state: &State,
    ) {
        std::thread::sleep(Duration::from_millis(250)); // TODO: Why this sleep?

        {
            let state_view = state.view();
            assert_eq!(state_view.height(), 0);
            assert_eq!(state_view.latest_block_hash(), None);
        }

        let mut state_block = state.block();
        let genesis = ValidBlock::validate(
            genesis.0,
            &self.topology,
            &self.chain_id,
            genesis_account,
            &mut state_block,
        )
        .unpack(|e| self.send_event(e))
        .expect("Genesis invalid");

        assert!(
            !genesis.as_ref().transactions().any(|tx| tx.error.is_some()),
            "Genesis contains invalid transactions"
        );

        let msg = BlockCreated::from(&genesis);
        let genesis = genesis
            .commit(&self.topology)
            .unpack(|e| self.send_event(e))
            .expect("Genesis invalid");

        self.broadcast_packet(msg);
        self.commit_block(genesis, state_block);
    }

    fn commit_block(&mut self, block: CommittedBlock, state_block: StateBlock<'_>) {
        self.update_state::<NewBlockStrategy>(block, state_block);
    }

    fn replace_top_block(&mut self, block: CommittedBlock, state_block: StateBlock<'_>) {
        self.update_state::<ReplaceTopBlockStrategy>(block, state_block);
    }

    fn update_state<Strategy: ApplyBlockStrategy>(
        &mut self,
        block: CommittedBlock,
        mut state_block: StateBlock<'_>,
    ) {
        let prev_role = self.role();

        let state_events = state_block.apply_without_execution(&block);

        // Parameters are updated before updating public copy of sumeragi
        self.update_params(&state_block);
        self.cache_transaction(&state_block);

        self.topology
            .block_committed(block.as_ref(), state_block.world.peers().cloned());
        self.connect_peers(&self.topology);

        let block_hash = block.as_ref().hash();
        let block_height = block.as_ref().header().height();
        Strategy::kura_store_block(&self.kura, block);

        // Commit new block making it's effect visible for the rest of application
        state_block.commit();
        info!(
            peer_id=%self.peer_id,
            %prev_role,
            next_role=%self.role(),
            block_hash=%block_hash,
            new_height=%block_height,
            "{}", Strategy::LOG_MESSAGE,
        );
        #[cfg(debug_assertions)]
        iroha_logger::info!(
            peer_id=%self.peer_id,
            role=%self.role(),
            topology=?self.topology,
            "Topology after commit"
        );

        // NOTE: This sends `BlockStatus::Applied` event,
        // so it should be done AFTER public facing state update
        state_events.into_iter().for_each(|e| self.send_event(e));

        self.round_start_time = Instant::now();
        self.was_commit = true;
    }

    fn update_params(&mut self, state_block: &StateBlock<'_>) {
        self.block_time = state_block.config.block_time;
        self.commit_time = state_block.config.commit_time;
        self.max_txs_in_block = state_block.config.max_transactions_in_block.get() as usize;
    }

    fn cache_transaction(&mut self, state_block: &StateBlock<'_>) {
        self.transaction_cache.retain(|tx| {
            !state_block.has_transaction(tx.as_ref().hash()) && !self.queue.is_expired(tx)
        });
    }

    fn validate_block<'state>(
        &self,
        state: &'state State,
        topology: &Topology,
        genesis_account: &AccountId,
        BlockCreated { block }: BlockCreated,
    ) -> Option<VotingBlock<'state>> {
        let mut state_block = state.block();

        ValidBlock::validate(
            block,
            topology,
            &self.chain_id,
            genesis_account,
            &mut state_block,
        )
        .unpack(|e| self.send_event(e))
        .map(|block| VotingBlock::new(block, state_block))
        .map_err(|(block, error)| {
            warn!(
                peer_id=%self.peer_id,
                role=%self.role(),
                block=%block.hash(),
                ?error,
                "Block validation failed"
            );
        })
        .ok()
    }

    fn prune_view_change_proofs_and_calculate_current_index(
        &self,
        latest_block: HashOf<SignedBlock>,
        view_change_proof_chain: &mut ProofChain,
    ) -> usize {
        view_change_proof_chain.prune(latest_block);
        view_change_proof_chain.verify_with_state(&self.topology, latest_block)
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::too_many_arguments)]
    fn handle_message<'state>(
        &mut self,
        message: BlockMessage,
        state: &'state State,
        voting_block: &mut Option<VotingBlock<'state>>,
        view_change_index: usize,
        genesis_account: &AccountId,
        voting_signatures: &mut BTreeSet<BlockSignature>,
        #[cfg_attr(not(debug_assertions), allow(unused_variables))] is_genesis_peer: bool,
    ) {
        #[allow(clippy::suspicious_operation_groupings)]
        match (message, self.role()) {
            (BlockMessage::BlockSyncUpdate(BlockSyncUpdate { block }), _) => {
                info!(
                    peer_id=%self.peer_id,
                    role=%self.role(),
                    block=%block.hash(),
                    "Block sync update received"
                );

                // Release block writer before creating new one
                // FIX: Restore `voting_block` if `handle_block_sync` returns Err
                // Currently it's not possible because block writer needs to be released
                // Look at https://github.com/hyperledger/iroha/issues/4643
                let _ = voting_block.take();

                match handle_block_sync(&self.chain_id, block, state, genesis_account, &|e| {
                    self.send_event(e)
                }) {
                    Ok(BlockSyncOk::CommitBlock(block, state_block, topology)) => {
                        self.topology = topology;
                        self.commit_block(block, state_block);
                    }
                    Ok(BlockSyncOk::ReplaceTopBlock(block, state_block, topology)) => {
                        let latest_block = state_block
                            .latest_block()
                            .expect("INTERNAL BUG: No latest block");

                        warn!(
                            peer_id=%self.peer_id,
                            role=%self.role(),
                            peer_latest_block_hash=?state_block.latest_block_hash(),
                            peer_latest_block_view_change_index=%latest_block.header().view_change_index,
                            consensus_latest_block=%block.as_ref().hash(),
                            consensus_latest_block_view_change_index=%block.as_ref().header().view_change_index,
                            "Soft fork occurred: peer in inconsistent state. Rolling back and replacing top block."
                        );
                        self.topology = topology;
                        self.replace_top_block(block, state_block);
                    }
                    Err((block, BlockSyncError::BlockNotValid(error))) => {
                        error!(
                            peer_id=%self.peer_id,
                            role=%self.role(),
                            block=%block.hash(),
                            ?error,
                            "Block not valid."
                        );
                    }
                    Err((block, BlockSyncError::SoftForkBlockNotValid(error))) => {
                        error!(
                            peer_id=%self.peer_id,
                            role=%self.role(),
                            block=%block.hash(),
                            ?error,
                            "Soft-fork block not valid."
                        );
                    }
                    Err((
                        block,
                        BlockSyncError::SoftForkBlockSmallViewChangeIndex {
                            peer_view_change_index,
                            block_view_change_index,
                        },
                    )) => {
                        debug!(
                            peer_id=%self.peer_id,
                            role=%self.role(),
                            peer_latest_block_hash=?state.view().latest_block_hash(),
                            peer_latest_block_view_change_index=?peer_view_change_index,
                            consensus_latest_block=%block.hash(),
                            consensus_latest_block_view_change_index=%block_view_change_index,
                            "Soft fork didn't occur: block has the same or smaller view change index"
                        );
                    }
                    Err((
                        block,
                        BlockSyncError::BlockNotProperHeight {
                            peer_height,
                            block_height,
                        },
                    )) => {
                        warn!(
                            peer_id=%self.peer_id,
                            role=%self.role(),
                            block=%block.hash(),
                            %block_height,
                            %peer_height,
                            "Received irrelevant or outdated block (neither `peer_height` nor `peer_height + 1`)."
                        );
                    }
                }
            }
            (BlockMessage::BlockCreated(block_created), Role::ValidatingPeer) => {
                let topology = &self
                    .topology
                    .is_consensus_required()
                    .expect("INTERNAL BUG: Consensus required for validating peer");

                // Release block writer before creating new one
                let _ = voting_block.take();

                if let Some(mut v_block) =
                    self.validate_block(state, topology, genesis_account, block_created)
                {
                    v_block.block.sign(&self.key_pair, topology);

                    let msg = BlockSigned::from(&v_block.block);
                    self.broadcast_packet_to(msg, [topology.proxy_tail()]);

                    *voting_block = Some(v_block);
                } else {
                    // FIX: Restore `voting_block`
                    // Currently it's not possible because block writer needs to be released
                    // Look at https://github.com/hyperledger/iroha/issues/4643
                }
            }
            (BlockMessage::BlockCreated(block_created), Role::ObservingPeer) => {
                let topology = &self
                    .topology
                    .is_consensus_required()
                    .expect("INTERNAL BUG: Consensus required for observing peer");

                // Release block writer before creating new one
                let _ = voting_block.take();

                if let Some(mut v_block) =
                    self.validate_block(state, topology, genesis_account, block_created)
                {
                    if view_change_index >= 1 {
                        v_block.block.sign(&self.key_pair, topology);

                        let msg = BlockSigned::from(&v_block.block);
                        self.broadcast_packet_to(msg, [topology.proxy_tail()]);

                        info!(
                            peer_id=%self.peer_id,
                            role=%self.role(),
                            block=%v_block.block.as_ref().hash(),
                            "Block signed and forwarded"
                        );
                    }

                    *voting_block = Some(v_block);
                } else {
                    // FIX: Restore `voting_block`
                    // Currently it's not possible because block writer needs to be released
                    // Look at https://github.com/hyperledger/iroha/issues/4643
                }
            }
            (BlockMessage::BlockCreated(block_created), Role::ProxyTail) => {
                info!(
                    peer_id=%self.peer_id,
                    role=%self.role(),
                    block=%block_created.block.hash(),
                    "Block received"
                );

                // Release block writer before creating new one
                let _ = voting_block.take();

                if let Some(mut valid_block) =
                    self.validate_block(state, &self.topology, genesis_account, block_created)
                {
                    // NOTE: Up until this point it was unknown which block is expected to be received,
                    // therefore all the signatures (of any hash) were collected and will now be pruned

                    for signature in core::mem::take(voting_signatures) {
                        if let Err(error) =
                            valid_block.block.add_signature(signature, &self.topology)
                        {
                            debug!(?error, "Signature not valid");
                        }
                    }

                    *voting_block = self.try_commit_block(valid_block, is_genesis_peer);
                } else {
                    // FIX: Restore `voting_block`
                    // Currently it's not possible because block writer needs to be released
                    // Look at https://github.com/hyperledger/iroha/issues/4643
                }
            }
            (BlockMessage::BlockSigned(BlockSigned { hash, signature }), Role::ProxyTail) => {
                info!(
                    peer_id=%self.peer_id,
                    role=%self.role(),
                    "Received block signatures"
                );

                if let Ok(signatory_idx) = usize::try_from(signature.0) {
                    let signatory = &self.topology.as_ref()[signatory_idx];

                    match self.topology.role(signatory) {
                        Role::Leader => error!(
                            peer_id=%self.peer_id,
                            role=%self.role(),
                            "Signatory is leader"
                        ),
                        Role::Undefined => error!(
                            peer_id=%self.peer_id,
                            role=%self.role(),
                            "Unknown signatory"
                        ),
                        Role::ObservingPeer if view_change_index == 0 => error!(
                            peer_id=%self.peer_id,
                            role=%self.role(),
                            "Signatory is observing peer"
                        ),
                        Role::ProxyTail => error!(
                            peer_id=%self.peer_id,
                            role=%self.role(),
                            "Signatory is proxy tail"
                        ),
                        _ => {
                            if let Some(mut voted_block) = voting_block.take() {
                                let actual_hash = voted_block.block.as_ref().hash_of_payload();

                                if hash != actual_hash {
                                    error!(
                                        peer_id=%self.peer_id,
                                        role=%self.role(),
                                        expected_hash=?hash,
                                        ?actual_hash,
                                        "Block hash mismatch"
                                    );
                                } else if let Err(err) =
                                    voted_block.block.add_signature(signature, &self.topology)
                                {
                                    error!(
                                        peer_id=%self.peer_id,
                                        role=%self.role(),
                                        ?err,
                                        "Signature not valid"
                                    );
                                } else {
                                    *voting_block =
                                        self.try_commit_block(voted_block, is_genesis_peer);
                                }
                            } else {
                                // NOTE: Due to the nature of distributed systems, signatures can sometimes be received before
                                // the block (sent by the leader). Collect the signatures and wait for the block to be received
                                if !voting_signatures.insert(signature) {
                                    error!(
                                        peer_id=%self.peer_id,
                                        role=%self.role(),
                                        "Duplicate signature"
                                    );
                                }
                            }
                        }
                    }
                } else {
                    error!(
                        peer_id=%self.peer_id,
                        role=%self.role(),
                        "Signatory index exceeds usize::MAX"
                    );
                }
            }
            (BlockMessage::BlockCommitted(BlockCommitted { .. }), Role::Leader)
                if self.topology.is_consensus_required().is_none() => {}
            (
                BlockMessage::BlockCommitted(BlockCommitted { hash, signatures }),
                Role::Leader | Role::ValidatingPeer | Role::ObservingPeer,
            ) => {
                if let Some(mut voted_block) = voting_block.take() {
                    let actual_hash = voted_block.block.as_ref().hash_of_payload();

                    if actual_hash == hash {
                        match voted_block
                            .block
                            // NOTE: The manipulation of the topology relies upon all peers seeing the same signature set.
                            // Therefore we must clear the signatures and accept what the proxy tail has giveth.
                            .replace_signatures(signatures, &self.topology)
                            .unpack(|e| self.send_event(e))
                        {
                            Ok(prev_signatures) => {
                                match voted_block
                                    .block
                                    .commit(&self.topology)
                                    .unpack(|e| self.send_event(e))
                                {
                                    Ok(committed_block) => {
                                        self.commit_block(committed_block, voted_block.state_block)
                                    }
                                    Err((mut block, error)) => {
                                        error!(
                                            peer_id=%self.peer_id,
                                            role=%self.role(),
                                            ?error,
                                            "Block failed to be committed"
                                        );

                                        block
                                            .replace_signatures(prev_signatures, &self.topology)
                                            .unpack(|e| self.send_event(e))
                                            .expect("INTERNAL BUG: Failed to replace signatures");
                                        voted_block.block = block;
                                        *voting_block = Some(voted_block);
                                    }
                                }
                            }
                            Err(error) => {
                                error!(
                                    peer_id=%self.peer_id,
                                    role=%self.role(),
                                    ?error,
                                    "Received incorrect signatures"
                                );

                                *voting_block = Some(voted_block);
                            }
                        }
                    } else {
                        error!(
                            peer_id=%self.peer_id,
                            role=%self.role(),
                            expected_hash=?hash,
                            ?actual_hash,
                            "Block hash mismatch"
                        );
                    }
                } else {
                    error!(
                        peer_id=%self.peer_id,
                        role=%self.role(),
                        "Peer missing voting block"
                    );
                }
            }
            (msg, _) => {
                trace!(
                    role=%self.role(),
                    peer_id=%self.peer_id,
                    ?msg,
                    "message not handled"
                );
            }
        }
    }

    /// Commits block if there are enough votes
    fn try_commit_block<'state>(
        &mut self,
        mut voting_block: VotingBlock<'state>,
        #[cfg_attr(not(debug_assertions), allow(unused_variables))] is_genesis_peer: bool,
    ) -> Option<VotingBlock<'state>> {
        assert_eq!(self.role(), Role::ProxyTail);

        let votes_count = voting_block.block.as_ref().signatures().len();
        if votes_count + 1 >= self.topology.min_votes_for_commit() {
            voting_block.block.sign(&self.key_pair, &self.topology);

            let committed_block = voting_block
                .block
                .commit(&self.topology)
                .unpack(|e| self.send_event(e))
                .expect("INTERNAL BUG: Proxy tail failed to commit block");

            #[cfg(debug_assertions)]
            if is_genesis_peer && self.debug_force_soft_fork {
                std::thread::sleep(self.pipeline_time() * 2);
            } else {
                let msg = BlockCommitted::from(&committed_block);
                self.broadcast_packet(msg);
            }

            #[cfg(not(debug_assertions))]
            {
                let msg = BlockCommitted::from(&committed_block);
                self.broadcast_packet(msg);
            }

            self.commit_block(committed_block, voting_block.state_block);

            return None;
        }

        Some(voting_block)
    }

    #[allow(clippy::too_many_lines)]
    fn try_create_block<'state>(
        &mut self,
        state: &'state State,
        voting_block: &mut Option<VotingBlock<'state>>,
    ) {
        assert_eq!(self.role(), Role::Leader);

        let tx_cache_full = self.transaction_cache.len() >= self.max_txs_in_block;
        let deadline_reached = self.round_start_time.elapsed() > self.block_time;
        let tx_cache_non_empty = !self.transaction_cache.is_empty();

        if tx_cache_full || (deadline_reached && tx_cache_non_empty) {
            let transactions = self.transaction_cache.clone();

            // TODO: properly process triggers!
            let mut state_block = state.block();
            let event_recommendations = Vec::new();
            let create_block_start_time = Instant::now();
            let new_block =
                BlockBuilder::new(transactions, self.topology.clone(), event_recommendations)
                    .chain(self.topology.view_change_index(), &mut state_block)
                    .sign(self.key_pair.private_key())
                    .unpack(|e| self.send_event(e));

            let created_in = create_block_start_time.elapsed();
            if created_in > self.pipeline_time() / 2 {
                warn!(
                    role=%self.role(),
                    peer_id=%self.peer_id,
                    "Creating block takes too much time. \
                    This might prevent consensus from operating. \
                    Consider increasing `commit_time` or decreasing `max_transactions_in_block`"
                );
            }

            if self.topology.is_consensus_required().is_some() {
                info!(
                    peer_id=%self.peer_id,
                    block=%new_block.as_ref().hash(),
                    view_change_index=%self.topology.view_change_index(),
                    txns=%new_block.as_ref().transactions().len(),
                    created_in_ms=%created_in.as_millis(),
                    "Block created"
                );

                let msg = BlockCreated::from(&new_block);
                *voting_block = Some(VotingBlock::new(new_block, state_block));
                self.broadcast_packet(msg);
            } else {
                let committed_block = new_block
                    .commit(&self.topology)
                    .unpack(|e| self.send_event(e))
                    .expect("INTERNAL BUG: Leader failed to commit created block");

                let msg = BlockCommitted::from(&committed_block);
                self.broadcast_packet(msg);
                self.commit_block(committed_block, state_block);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn reset_state(
    peer_id: &PeerId,
    pipeline_time: Duration,
    view_change_index: usize,
    was_commit: &mut bool,
    topology: &mut Topology,
    voting_block: &mut Option<VotingBlock>,
    voting_signatures: &mut BTreeSet<BlockSignature>,
    last_view_change_time: &mut Instant,
    view_change_time: &mut Duration,
) {
    let mut was_commit_or_view_change = *was_commit;

    let prev_role = topology.role(peer_id);
    if topology.view_change_index() < view_change_index {
        let new_rotations = topology.nth_rotation(view_change_index);

        error!(
            %peer_id,
            %prev_role,
            next_role=%topology.role(peer_id),
            n=%new_rotations,
            %view_change_index,
            "Topology rotated n times"
        );
        #[cfg(debug_assertions)]
        iroha_logger::info!(
            %peer_id,
            role=%topology.role(peer_id),
            topology=?topology,
            "Topology after rotation"
        );

        was_commit_or_view_change = true;
    }

    // Reset state for the next round.
    if was_commit_or_view_change {
        *voting_block = None;
        voting_signatures.clear();
        *last_view_change_time = Instant::now();
        *view_change_time = pipeline_time;

        *was_commit = false;
    }
}

fn should_terminate(shutdown_receiver: &mut tokio::sync::oneshot::Receiver<()>) -> bool {
    use tokio::sync::oneshot::error::TryRecvError;

    match shutdown_receiver.try_recv() {
        Err(TryRecvError::Empty) => false,
        reason => {
            info!(?reason, "Sumeragi Thread is being shut down.");
            true
        }
    }
}

#[iroha_logger::log(name = "consensus", skip_all)]
/// Execute the main loop of [`Sumeragi`]
pub(crate) fn run(
    genesis_network: GenesisWithPubKey,
    mut sumeragi: Sumeragi,
    mut shutdown_receiver: tokio::sync::oneshot::Receiver<()>,
    state: Arc<State>,
) {
    // Connect peers with initial topology
    sumeragi.connect_peers(&sumeragi.topology);

    let genesis_account = AccountId::new(
        iroha_genesis::GENESIS_DOMAIN_ID.clone(),
        genesis_network.public_key.clone(),
    );

    let span = span!(tracing::Level::TRACE, "genesis").entered();
    let is_genesis_peer = if state.view().height() == 0
        || state.view().latest_block_hash().is_none()
    {
        if let Some(genesis) = genesis_network.genesis {
            sumeragi.init_commit_genesis(genesis, &genesis_account, &state);
            true
        } else {
            if let Err(err) =
                sumeragi.init_listen_for_genesis(&genesis_account, &state, &mut shutdown_receiver)
            {
                info!(?err, "Sumeragi Thread is being shut down.");
                return;
            }
            false
        }
    } else {
        false
    };
    span.exit();

    info!(
        peer_id=%sumeragi.peer_id,
        role=%sumeragi.role(),
        "Sumeragi initialized",
    );

    let mut voting_block = None;
    // Proxy tail collection of voting block signatures
    let mut voting_signatures = BTreeSet::new();
    let mut should_sleep = false;
    let mut view_change_proof_chain = ProofChain::default();
    // Duration after which a view change is suggested
    let mut view_change_time = sumeragi.pipeline_time();
    // Instant when the previous view change or round happened.
    let mut last_view_change_time = Instant::now();

    sumeragi.was_commit = false;
    sumeragi.round_start_time = Instant::now();
    while !should_terminate(&mut shutdown_receiver) {
        if should_sleep {
            let span = span!(Level::TRACE, "main_thread_sleep");
            let _enter = span.enter();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let span_for_sumeragi_cycle = span!(Level::TRACE, "main_thread_cycle");
        let _enter_for_sumeragi_cycle = span_for_sumeragi_cycle.enter();

        let state_view = state.view();

        sumeragi
            .transaction_cache
            // Checking if transactions are in the blockchain is costly
            .retain(|tx| {
                let expired = sumeragi.queue.is_expired(tx);
                if expired {
                    debug!(?tx, "Transaction expired")
                }
                expired
            });

        sumeragi.queue.get_transactions_for_block(
            &state_view,
            sumeragi.max_txs_in_block,
            &mut sumeragi.transaction_cache,
        );

        let view_change_index = sumeragi.prune_view_change_proofs_and_calculate_current_index(
            state_view
                .latest_block_hash()
                .expect("INTERNAL BUG: No latest block"),
            &mut view_change_proof_chain,
        );

        reset_state(
            &sumeragi.peer_id,
            sumeragi.pipeline_time(),
            view_change_index,
            &mut sumeragi.was_commit,
            &mut sumeragi.topology,
            &mut voting_block,
            &mut voting_signatures,
            &mut last_view_change_time,
            &mut view_change_time,
        );
        sumeragi
            .view_changes_metric
            .set(sumeragi.topology.view_change_index() as u64);

        if let Some(message) = {
            let (msg, sleep) = sumeragi.receive_network_packet(
                state_view
                    .latest_block_hash()
                    .expect("INTERNAL BUG: No latest block"),
                &mut view_change_proof_chain,
            );
            should_sleep = sleep;
            msg
        } {
            sumeragi.handle_message(
                message,
                &state,
                &mut voting_block,
                view_change_index,
                &genesis_account,
                &mut voting_signatures,
                is_genesis_peer,
            );
        }

        // State could be changed after handling message so it is necessary to reset state before handling message independent step
        let state_view = state.view();
        let view_change_index = sumeragi.prune_view_change_proofs_and_calculate_current_index(
            state_view
                .latest_block_hash()
                .expect("INTERNAL BUG: No latest block"),
            &mut view_change_proof_chain,
        );

        // We broadcast our view change suggestion after having processed the latest from others inside `receive_network_packet`
        let block_expected = !sumeragi.transaction_cache.is_empty();
        if (block_expected || view_change_index > 0)
            && last_view_change_time.elapsed() > view_change_time
        {
            if block_expected {
                if let Some(VotingBlock { block, .. }) = voting_block.as_ref() {
                    // NOTE: Suspecting the tail node because it hasn't committed the block yet

                    warn!(
                        peer_id=%sumeragi.peer_id,
                        role=%sumeragi.role(),
                        block=%block.as_ref().hash(),
                        "Block not committed in due time, requesting view change..."
                    );
                } else {
                    // NOTE: Suspecting the leader node because it hasn't produced a block
                    // If the current node has a transaction, leader should have as well

                    warn!(
                        peer_id=%sumeragi.peer_id,
                        role=%sumeragi.role(),
                        "No block produced in due time, requesting view change..."
                    );
                }

                let latest_block = state_view
                    .latest_block_hash()
                    .expect("INTERNAL BUG: No latest block");
                let suspect_proof =
                    ProofBuilder::new(latest_block, view_change_index).sign(&sumeragi.key_pair);

                view_change_proof_chain
                    .insert_proof(suspect_proof, &sumeragi.topology, latest_block)
                    .unwrap_or_else(|err| error!("{err}"));
            }

            let msg = ControlFlowMessage::new(view_change_proof_chain.clone());
            sumeragi.broadcast_control_flow_packet(msg);

            // NOTE: View change must be periodically suggested until it is accepted.
            // Must be initialized to pipeline time but can increase by chosen amount
            view_change_time += sumeragi.pipeline_time();
        }

        reset_state(
            &sumeragi.peer_id,
            sumeragi.pipeline_time(),
            view_change_index,
            &mut sumeragi.was_commit,
            &mut sumeragi.topology,
            &mut voting_block,
            &mut voting_signatures,
            &mut last_view_change_time,
            &mut view_change_time,
        );
        sumeragi
            .view_changes_metric
            .set(sumeragi.topology.view_change_index() as u64);

        if sumeragi.role() == Role::Leader && voting_block.is_none() {
            sumeragi.try_create_block(&state, &mut voting_block);
        }
    }
}

/// Type enumerating early return types to reduce cyclomatic
/// complexity of the main loop items and allow direct short
/// circuiting with the `?` operator. Candidate for `impl
/// FromResidual`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EarlyReturn {
    /// Shutdown message received.
    ShutdownMessageReceived,
    /// Disconnected
    Disconnected,
}

fn early_return(
    shutdown_receiver: &mut tokio::sync::oneshot::Receiver<()>,
) -> Result<(), EarlyReturn> {
    use tokio::sync::oneshot::error::TryRecvError;

    match shutdown_receiver.try_recv() {
        Ok(()) | Err(TryRecvError::Closed) => {
            info!("Sumeragi Thread is being shut down.");
            Err(EarlyReturn::ShutdownMessageReceived)
        }
        Err(TryRecvError::Empty) => Ok(()),
    }
}

/// Strategy to apply block to sumeragi.
trait ApplyBlockStrategy {
    const LOG_MESSAGE: &'static str;

    /// Operation to invoke in kura to store block.
    fn kura_store_block(kura: &Kura, block: CommittedBlock);
}

/// Commit new block strategy. Used during normal consensus rounds.
struct NewBlockStrategy;

impl ApplyBlockStrategy for NewBlockStrategy {
    const LOG_MESSAGE: &'static str = "Block committed";

    #[inline]
    fn kura_store_block(kura: &Kura, block: CommittedBlock) {
        kura.store_block(block)
    }
}

/// Replace top block strategy. Used in case of soft-fork.
struct ReplaceTopBlockStrategy;

impl ApplyBlockStrategy for ReplaceTopBlockStrategy {
    const LOG_MESSAGE: &'static str = "Top block replaced";

    #[inline]
    fn kura_store_block(kura: &Kura, block: CommittedBlock) {
        kura.replace_top_block(block)
    }
}

enum BlockSyncOk<'state> {
    CommitBlock(CommittedBlock, StateBlock<'state>, Topology),
    ReplaceTopBlock(CommittedBlock, StateBlock<'state>, Topology),
}

#[derive(Debug)]
enum BlockSyncError {
    BlockNotValid(BlockValidationError),
    SoftForkBlockNotValid(BlockValidationError),
    SoftForkBlockSmallViewChangeIndex {
        peer_view_change_index: usize,
        block_view_change_index: usize,
    },
    BlockNotProperHeight {
        peer_height: usize,
        block_height: usize,
    },
}

fn handle_block_sync<'state, F: Fn(PipelineEventBox)>(
    chain_id: &ChainId,
    block: SignedBlock,
    state: &'state State,
    genesis_account: &AccountId,
    handle_events: &F,
) -> Result<BlockSyncOk<'state>, (SignedBlock, BlockSyncError)> {
    let block_height = block
        .header()
        .height
        .try_into()
        .expect("INTERNAL BUG: Block height exceeds usize::MAX");

    let state_height = state.view().height();
    let (mut state_block, soft_fork) = if state_height + 1 == block_height {
        // NOTE: Normal branch for adding new block on top of current

        (state.block(), false)
    } else if state_height == block_height && block_height > 1 {
        // NOTE: Soft fork branch for replacing current block with valid one

        let latest_block = state
            .view()
            .latest_block()
            .expect("INTERNAL BUG: No latest block");
        let peer_view_change_index = latest_block.header().view_change_index as usize;
        let block_view_change_index = block.header().view_change_index as usize;
        if peer_view_change_index >= block_view_change_index {
            return Err((
                block,
                BlockSyncError::SoftForkBlockSmallViewChangeIndex {
                    peer_view_change_index,
                    block_view_change_index,
                },
            ));
        }

        (state.block_and_revert(), true)
    } else {
        // Error branch other peer send irrelevant block
        return Err((
            block,
            BlockSyncError::BlockNotProperHeight {
                peer_height: state_height,
                block_height,
            },
        ));
    };
    let latest_block = state_block
        .latest_block()
        .expect("INTERNAL BUG: No latest block");
    let mut topology = Topology::new(latest_block.commit_topology().cloned());
    topology.block_committed(&latest_block, state_block.world.peers().cloned());
    topology.nth_rotation(block.header().view_change_index as usize);

    ValidBlock::validate(
        block,
        &topology,
        chain_id,
        genesis_account,
        &mut state_block,
    )
    .unpack(handle_events)
    .and_then(|block| {
        block
            .commit(&topology)
            .unpack(handle_events)
            .map_err(|(block, err)| (block.into(), err))
    })
    .map_err(|(block, error)| {
        (
            block,
            if soft_fork {
                BlockSyncError::SoftForkBlockNotValid(error)
            } else {
                BlockSyncError::BlockNotValid(error)
            },
        )
    })
    .map(|block| {
        if soft_fork {
            BlockSyncOk::ReplaceTopBlock(block, state_block, topology)
        } else {
            BlockSyncOk::CommitBlock(block, state_block, topology)
        }
    })
}

#[cfg(test)]
mod tests {
    use iroha_genesis::GENESIS_DOMAIN_ID;
    use test_samples::gen_account_in;
    use tokio::test;

    use super::*;
    use crate::{query::store::LiveQueryStore, smartcontracts::Registrable};

    /// Used to inject faulty payload for testing
    fn clone_and_modify_payload(
        block: &SignedBlock,
        private_key: &PrivateKey,
        f: impl FnOnce(&mut BlockPayload),
    ) -> SignedBlock {
        let mut payload = block.payload().clone();
        f(&mut payload);

        payload.sign(private_key)
    }

    fn create_data_for_test(
        chain_id: &ChainId,
        topology: &Topology,
        leader_private_key: &PrivateKey,
    ) -> (State, Arc<Kura>, SignedBlock, AccountId) {
        // Predefined world state
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let genesis_account = AccountId::new(
            GENESIS_DOMAIN_ID.clone(),
            alice_keypair.public_key().clone(),
        );
        let account = Account::new(alice_id.clone()).build(&alice_id);
        let domain_id = "wonderland".parse().expect("Valid");
        let domain = Domain::new(domain_id).build(&alice_id);
        let world = World::with([domain], [account], topology.iter().cloned().collect());
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::test().start();
        let state = State::new(world, Arc::clone(&kura), query_handle);

        // Create "genesis" block
        // Creating an instruction
        let fail_box = Fail::new("Dummy isi".to_owned());

        let mut state_block = state.block();
        // Making two transactions that have the same instruction
        let tx = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([fail_box])
            .sign(alice_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            chain_id,
            state_block.transaction_executor().transaction_limits,
        )
        .expect("Valid");

        // Creating a block of two identical transactions and validating it
        let block = BlockBuilder::new(vec![tx.clone(), tx], topology.clone(), Vec::new())
            .chain(0, &mut state_block)
            .sign(leader_private_key)
            .unpack(|_| {});

        let genesis = block
            .commit(topology)
            .unpack(|_| {})
            .expect("Block is valid");
        let _events = state_block.apply(&genesis).expect("Failed to apply block");
        state_block.commit();
        kura.store_block(genesis);

        let block = {
            let mut state_block = state.block();
            // Making two transactions that have the same instruction
            let create_asset_definition1 = Register::asset_definition(AssetDefinition::numeric(
                "xor1#wonderland".parse().expect("Valid"),
            ));
            let create_asset_definition2 = Register::asset_definition(AssetDefinition::numeric(
                "xor2#wonderland".parse().expect("Valid"),
            ));

            let tx1 = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
                .with_instructions([create_asset_definition1])
                .sign(alice_keypair.private_key());
            let tx1 = AcceptedTransaction::accept(
                tx1,
                chain_id,
                state_block.transaction_executor().transaction_limits,
            )
            .map(Into::into)
            .expect("Valid");
            let tx2 = TransactionBuilder::new(chain_id.clone(), alice_id)
                .with_instructions([create_asset_definition2])
                .sign(alice_keypair.private_key());
            let tx2 = AcceptedTransaction::accept(
                tx2,
                chain_id,
                state_block.transaction_executor().transaction_limits,
            )
            .map(Into::into)
            .expect("Valid");

            // Creating a block of two identical transactions and validating it
            BlockBuilder::new(vec![tx1, tx2], topology.clone(), Vec::new())
                .chain(0, &mut state_block)
                .sign(leader_private_key)
                .unpack(|_| {})
        };

        (state, kura, block.into(), genesis_account)
    }

    #[test]
    #[allow(clippy::redundant_clone)]
    async fn block_sync_invalid_block() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        let (leader_public_key, leader_private_key) = KeyPair::random().into_parts();
        let peer_id = PeerId::new("127.0.0.1:8080".parse().unwrap(), leader_public_key);
        let topology = Topology::new(vec![peer_id]);
        let (state, _, block, genesis_public_key) =
            create_data_for_test(&chain_id, &topology, &leader_private_key);

        // Malform block to make it invalid
        let block = clone_and_modify_payload(&block, &leader_private_key, |payload| {
            payload.commit_topology.clear();
        });

        let result = handle_block_sync(&chain_id, block, &state, &genesis_public_key, &|_| {});
        assert!(matches!(result, Err((_, BlockSyncError::BlockNotValid(_)))))
    }

    #[test]
    async fn block_sync_invalid_soft_fork_block() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        let (leader_public_key, leader_private_key) = KeyPair::random().into_parts();
        let peer_id = PeerId::new("127.0.0.1:8080".parse().unwrap(), leader_public_key);
        let topology = Topology::new(vec![peer_id]);
        let (state, kura, block, genesis_public_key) =
            create_data_for_test(&chain_id, &topology, &leader_private_key);

        let mut state_block = state.block();
        let committed_block = ValidBlock::validate(
            block.clone(),
            &topology,
            &chain_id,
            &genesis_public_key,
            &mut state_block,
        )
        .unpack(|_| {})
        .unwrap()
        .commit(&topology)
        .unpack(|_| {})
        .expect("Block is valid");
        let _events = state_block.apply_without_execution(&committed_block);
        state_block.commit();
        kura.store_block(committed_block);

        // Malform block to make it invalid
        let block = clone_and_modify_payload(&block, &leader_private_key, |payload| {
            payload.commit_topology.clear();
            payload.header.view_change_index = 1;
        });

        let result = handle_block_sync(&chain_id, block, &state, &genesis_public_key, &|_| {});
        assert!(matches!(
            result,
            Err((_, BlockSyncError::SoftForkBlockNotValid(_)))
        ))
    }

    #[test]
    #[allow(clippy::redundant_clone)]
    async fn block_sync_not_proper_height() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        let (leader_public_key, leader_private_key) = KeyPair::random().into_parts();
        let peer_id = PeerId::new("127.0.0.1:8080".parse().unwrap(), leader_public_key);
        let topology = Topology::new(vec![peer_id]);
        let (state, _, block, genesis_public_key) =
            create_data_for_test(&chain_id, &topology, &leader_private_key);

        // Change block height
        let block = clone_and_modify_payload(&block, &leader_private_key, |payload| {
            payload.header.height = 42;
        });

        let result = handle_block_sync(&chain_id, block, &state, &genesis_public_key, &|_| {});
        assert!(matches!(
            result,
            Err((
                _,
                BlockSyncError::BlockNotProperHeight {
                    peer_height: 1,
                    block_height: 42
                }
            ))
        ))
    }

    #[test]
    #[allow(clippy::redundant_clone)]
    async fn block_sync_commit_block() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        let (leader_public_key, leader_private_key) = KeyPair::random().into_parts();
        let peer_id = PeerId::new("127.0.0.1:8080".parse().unwrap(), leader_public_key);
        let topology = Topology::new(vec![peer_id]);
        let (state, _, block, genesis_public_key) =
            create_data_for_test(&chain_id, &topology, &leader_private_key);
        let result = handle_block_sync(&chain_id, block, &state, &genesis_public_key, &|_| {});
        assert!(matches!(result, Ok(BlockSyncOk::CommitBlock(_, _, _))))
    }

    #[test]
    async fn block_sync_replace_top_block() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        let (leader_public_key, leader_private_key) = KeyPair::random().into_parts();
        let peer_id = PeerId::new("127.0.0.1:8080".parse().unwrap(), leader_public_key);
        let topology = Topology::new(vec![peer_id]);
        let (state, kura, block, genesis_public_key) =
            create_data_for_test(&chain_id, &topology, &leader_private_key);

        let mut state_block = state.block();
        let committed_block = ValidBlock::validate(
            block.clone(),
            &topology,
            &chain_id,
            &genesis_public_key,
            &mut state_block,
        )
        .unpack(|_| {})
        .unwrap()
        .commit(&topology)
        .unpack(|_| {})
        .expect("Block is valid");
        let _events = state_block.apply_without_execution(&committed_block);
        state_block.commit();

        kura.store_block(committed_block);
        let latest_block = state
            .view()
            .latest_block()
            .expect("INTERNAL BUG: No latest block");
        let latest_block_view_change_index = latest_block.header().view_change_index;
        assert_eq!(latest_block_view_change_index, 0);

        // Increase block view change index
        let block = clone_and_modify_payload(&block, &leader_private_key, |payload| {
            payload.header.view_change_index = 42;
        });

        let result = handle_block_sync(&chain_id, block, &state, &genesis_public_key, &|_| {});
        assert!(matches!(result, Ok(BlockSyncOk::ReplaceTopBlock(_, _, _))))
    }

    #[test]
    async fn block_sync_small_view_change_index() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        let (leader_public_key, leader_private_key) = KeyPair::random().into_parts();
        let peer_id = PeerId::new("127.0.0.1:8080".parse().unwrap(), leader_public_key);
        let topology = Topology::new(vec![peer_id]);
        let (state, kura, block, genesis_public_key) =
            create_data_for_test(&chain_id, &topology, &leader_private_key);

        // Increase block view change index
        let block = clone_and_modify_payload(&block, &leader_private_key, |payload| {
            payload.header.view_change_index = 42;
        });

        let mut state_block = state.block();
        let committed_block = ValidBlock::validate(
            block.clone(),
            &topology,
            &chain_id,
            &genesis_public_key,
            &mut state_block,
        )
        .unpack(|_| {})
        .unwrap()
        .commit(&topology)
        .unpack(|_| {})
        .expect("Block is valid");
        let _events = state_block.apply_without_execution(&committed_block);
        state_block.commit();
        kura.store_block(committed_block);
        let latest_block = state
            .view()
            .latest_block()
            .expect("INTERNAL BUG: No latest block");
        let latest_block_view_change_index = latest_block.header().view_change_index;
        assert_eq!(latest_block_view_change_index, 42);

        // Decrease block view change index back
        let block = clone_and_modify_payload(&block, &leader_private_key, |payload| {
            payload.header.view_change_index = 0;
        });

        let result = handle_block_sync(&chain_id, block, &state, &genesis_public_key, &|_| {});
        assert!(matches!(
            result,
            Err((
                _,
                BlockSyncError::SoftForkBlockSmallViewChangeIndex {
                    peer_view_change_index: 42,
                    block_view_change_index: 0
                }
            ))
        ))
    }

    #[test]
    #[allow(clippy::redundant_clone)]
    async fn block_sync_genesis_block_do_not_replace() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        let (leader_public_key, leader_private_key) = KeyPair::random().into_parts();
        let peer_id = PeerId::new("127.0.0.1:8080".parse().unwrap(), leader_public_key);
        let topology = Topology::new(vec![peer_id]);
        let (state, _, block, genesis_public_key) =
            create_data_for_test(&chain_id, &topology, &leader_private_key);

        // Change block height and view change index
        // Soft-fork on genesis block is not possible
        let block = clone_and_modify_payload(&block, &leader_private_key, |payload| {
            payload.header.view_change_index = 42;
            payload.header.height = 1;
        });

        let result = handle_block_sync(&chain_id, block, &state, &genesis_public_key, &|_| {});
        assert!(matches!(
            result,
            Err((
                _,
                BlockSyncError::BlockNotProperHeight {
                    peer_height: 1,
                    block_height: 1,
                }
            ))
        ))
    }
}
