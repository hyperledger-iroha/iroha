//! The main event loop that powers sumeragi.
#![allow(clippy::cognitive_complexity)]
use iroha_crypto::HashOf;
use iroha_data_model::{block::*, transaction::error::TransactionExpired};
use iroha_p2p::UpdateTopology;
use parking_lot::Mutex;
use tracing::{span, Level};

use super::*;
use crate::{block::*, sumeragi::tracing::instrument};

/// `Sumeragi` is the implementation of the consensus.
///
/// TODO: paraphrase
///
/// `sumeragi_state_data` is a [`Mutex`] instead of a `RWLock`
/// because it communicates more clearly the correct use of the
/// lock. The most frequent action on this lock is the main loop
/// writing to it. This means that if anyone holds this lock they are
/// blocking the sumeragi thread. A `RWLock` will tempt someone to
/// hold a read lock because they think they are being smart, whilst a
/// [`Mutex`] screams *DO NOT HOLD ME*. That is why the [`State`] is
/// wrapped in a mutex, it's more self-documenting.
pub struct Sumeragi {
    /// The pair of keys used for communication given this Sumeragi instance.
    pub key_pair: KeyPair,
    /// Address of queue
    pub queue: Arc<Queue>,
    /// The peer id of myself.
    pub peer_id: PeerId,
    /// An actor that sends events
    pub events_sender: EventsSender,
    /// The world state view instance that is used in public contexts
    pub wsv: Mutex<WorldStateView>,
    /// Time by which a newly created block should be committed. Prevents malicious nodes
    /// from stalling the network by not participating in consensus
    pub commit_time: Duration,
    /// Time by which a new block should be created regardless if there were enough transactions or not.
    /// Used to force block commits when there is a small influx of new transactions.
    pub block_time: Duration,
    /// Limits that all transactions need to obey, in terms of size
    /// of WASM blob and number of instructions.
    pub transaction_limits: TransactionLimits,
    /// [`TransactionValidator`] instance that we use
    pub transaction_validator: TransactionValidator,
    /// Kura instance used for IO
    pub kura: Arc<Kura>,
    /// [`iroha_p2p::Network`] actor address
    pub network: IrohaNetwork,
    /// The size of batch that is being gossiped. Smaller size leads
    /// to longer time to synchronise, useful if you have high packet loss.
    pub gossip_batch_size: u32,
    /// The time between gossiping. More frequent gossiping shortens
    /// the time to sync, but can overload the network.
    pub gossip_period: Duration,
    /// Receiver channel.
    // TODO: Mutex shouldn't be required and must be removed
    pub message_receiver: Mutex<mpsc::Receiver<MessagePacket>>,
    /// Only used in testing. Causes the genesis peer to withhold blocks when it
    /// is the proxy tail.
    pub debug_force_soft_fork: bool,
}

impl Debug for Sumeragi {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sumeragi")
            .field("public_key", &self.key_pair.public_key())
            .field("peer_id", &self.peer_id)
            .finish()
    }
}

/// Internal structure that retains the state.
pub struct State {
    /// The view change index of latest [`VersionedCommittedBlock`]
    pub latest_block_view_change_index: u64,
    /// The hash of the latest [`VersionedCommittedBlock`]
    pub latest_block_hash: Option<HashOf<VersionedCommittedBlock>>,
    /// Hash of the previous [`VersionedCommittedBlock`]
    pub previous_block_hash: Option<HashOf<VersionedCommittedBlock>>,
    /// Current block height
    pub latest_block_height: u64,
    /// The current network topology.
    pub current_topology: Topology,
    /// The sumeragi internal `WorldStateView`. This will probably
    /// morph into a wsv + various patches as we attempt to
    /// multithread isi execution. In the future we might also once
    /// again merge the internal wsv with the public facing one. But
    /// as of now we keep them seperate for greater flexibility when
    /// optimizing.
    pub wsv: WorldStateView,
    /// A copy of wsv that is kept one block behind at all times. Because
    /// we currently don't support rolling back wsv block application we
    /// reset to a copy of the finalized_wsv instead. This is expensive but
    /// enables us to handle soft-forks.
    pub finalized_wsv: WorldStateView,
    /// In order to *be fast*, we must minimize communication with
    /// other subsystems where we can. This way the performance of
    /// sumeragi is more dependent on the code that is internal to the
    /// subsystem.
    pub transaction_cache: Vec<VersionedAcceptedTransaction>,
}

impl Sumeragi {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        configuration: &Configuration,
        queue: Arc<Queue>,
        events_sender: EventsSender,
        wsv: WorldStateView,
        transaction_validator: TransactionValidator,
        kura: Arc<Kura>,
        network: IrohaNetwork,
        message_receiver: mpsc::Receiver<MessagePacket>,
    ) -> Self {
        #[cfg(debug_assertions)]
        let soft_fork = configuration.debug_force_soft_fork;
        #[cfg(not(debug_assertions))]
        let soft_fork = false;

        Self {
            key_pair: configuration.key_pair.clone(),
            queue,
            peer_id: configuration.peer_id.clone(),
            events_sender,
            wsv: Mutex::new(wsv),
            commit_time: Duration::from_millis(configuration.commit_time_limit_ms),
            block_time: Duration::from_millis(configuration.block_time_ms),
            transaction_limits: configuration.transaction_limits,
            transaction_validator,
            kura,
            network,
            gossip_batch_size: configuration.gossip_batch_size,
            gossip_period: Duration::from_millis(configuration.gossip_period_ms),
            message_receiver: Mutex::new(message_receiver),
            debug_force_soft_fork: soft_fork,
        }
    }

    /// Send a sumeragi packet over the network to the specified `peer`.
    /// # Errors
    /// Fails if network sending fails
    #[instrument(skip(self, packet))]
    #[allow(clippy::needless_pass_by_value)] // TODO: Fix.
    fn post_packet_to(&self, packet: MessagePacket, peer: &PeerId) {
        let post = iroha_p2p::Post {
            data: NetworkMessage::SumeragiPacket(Box::new(packet.into())),
            peer_id: peer.clone(),
        };
        self.network.post(post);
    }

    #[allow(clippy::needless_pass_by_value, single_use_lifetimes)] // TODO: uncomment when anonymous lifetimes are stable
    fn broadcast_packet_to<'peer_id>(
        &self,
        msg: MessagePacket,
        ids: impl IntoIterator<Item = &'peer_id PeerId> + Send,
    ) {
        for peer_id in ids {
            self.post_packet_to(msg.clone(), peer_id);
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn broadcast_packet(&self, msg: MessagePacket, topology: &Topology) {
        self.broadcast_packet_to(msg, &topology.sorted_peers);
    }

    fn gossip_transactions(&self, state: &State, view_change_proof_chain: &ProofChain) {
        let current_topology = &state.current_topology;
        let role = current_topology.role(&self.peer_id);

        // Transactions are intentionally taken from the queue instead of the cache
        // to gossip multisignature transactions too
        let txs = self
            .queue
            .n_random_transactions(self.gossip_batch_size, &state.wsv);

        if !txs.is_empty() {
            debug!(%role, tx_count = txs.len(), "Gossiping transactions");

            let msg =
                MessagePacket::new(view_change_proof_chain.clone(), TransactionGossip::new(txs));

            self.broadcast_packet(msg, current_topology);
        }
    }

    /// Connect or disconnect peers according to the current network topology.
    fn connect_peers(&self, topology: &Topology) {
        let peers = topology.sorted_peers.clone().into_iter().collect();
        self.network.update_topology(UpdateTopology(peers));
    }

    /// The maximum time a sumeragi round can take to produce a block when
    /// there are no faulty peers in the a set.
    fn pipeline_time(&self) -> Duration {
        self.block_time + self.commit_time
    }

    fn send_events(&self, events: impl Into<Vec<Event>>) {
        let addr = &self.peer_id.address;

        if self.events_sender.receiver_count() > 0 {
            for event in events.into() {
                self.events_sender
                    .send(event)
                    .map_err(|err| warn!(%addr, ?err, "Event not sent"))
                    .unwrap_or(0);
            }
        }
    }

    #[allow(clippy::panic)]
    fn receive_network_packet(
        &self,
        state: &State,
        view_change_proof_chain: &mut ProofChain,
    ) -> Option<Message> {
        let current_topology = &state.current_topology;
        match self.message_receiver.lock().try_recv() {
            Ok(packet) => {
                if let Err(error) = view_change_proof_chain.merge(
                    packet.view_change_proofs,
                    &current_topology.sorted_peers,
                    current_topology.max_faults(),
                    state.latest_block_hash,
                ) {
                    trace!(%error, "Failed to add proofs into view change proof chain")
                }
                Some(packet.message)
            }
            Err(recv_error) => match recv_error {
                mpsc::TryRecvError::Empty => None,
                mpsc::TryRecvError::Disconnected => {
                    panic!("Sumeragi message pump disconnected. This is not a recoverable error.")
                    // TODO: Use early return.
                }
            },
        }
    }

    #[allow(clippy::panic, clippy::panic_in_result_fn)]
    fn init_listen_for_genesis(
        &self,
        state: &mut State,
        shutdown_receiver: &mut tokio::sync::oneshot::Receiver<()>,
    ) -> Result<(), EarlyReturn> {
        trace!("Listen for genesis");
        assert!(
            state.current_topology.is_consensus_required(),
            "Only peer in network, yet required to receive genesis topology. This is a configuration error."
        );
        loop {
            std::thread::sleep(Duration::from_millis(50));
            early_return(shutdown_receiver).map_err(|e| {
                debug!(?e, "Early return.");
                e
            })?;
            // we must connect to peers so that our block_sync can find us
            // the genesis block.
            match self.message_receiver.lock().try_recv() {
                Ok(packet) => {
                    let block = match packet.message {
                        Message::BlockCreated(block_created) => {
                            // If we receive a committed genesis block that is
                            // valid, use it without question.  During the
                            // genesis round we blindly take on the network
                            // topology described in the provided genesis
                            // block.
                            let block = {
                                let span = span!(
                                    Level::TRACE,
                                    "Genesis Round Peer is revalidating the block."
                                );
                                let _enter = span.enter();
                                match block_created.validate_and_extract_block::<true>(
                                    &self.transaction_validator,
                                    state.wsv.clone(),
                                    state.latest_block_hash,
                                    state.latest_block_height,
                                ) {
                                    Ok(block) => block,
                                    Err(error) => {
                                        error!(?error);
                                        continue;
                                    }
                                }
                            };
                            // Omit signature verification during genesis round
                            block.commit_unchecked().into()
                        }
                        Message::BlockSyncUpdate(block_sync_update) => {
                            // Omit signature verification during genesis round
                            match block_sync_update.validate_and_extract_block::<true>(
                                &self.transaction_validator,
                                state.wsv.clone(),
                                state.latest_block_hash,
                                state.latest_block_height,
                            ) {
                                Ok(block) => block,
                                Err(error) => {
                                    error!(?error);
                                    continue;
                                }
                            }
                        }
                        msg => {
                            trace!(?msg, "Not handling the message, waiting for genesis...");
                            continue;
                        }
                    };

                    if block.header().is_genesis() {
                        commit_block(self, state, block);
                        return Err(EarlyReturn::GenesisBlockReceivedAndCommitted);
                    }
                    debug!("Received a block that was not genesis.");
                }
                Err(mpsc::TryRecvError::Disconnected) => return Err(EarlyReturn::Disconnected),
                _ => (),
            }
        }
    }
}

fn commit_block(sumeragi: &Sumeragi, state: &mut State, block: impl Into<VersionedCommittedBlock>) {
    let committed_block = block.into();

    state.finalized_wsv = state.wsv.clone();
    update_state(state, sumeragi, &committed_block);
    state.previous_block_hash = state.latest_block_hash;

    info!(
        addr=%sumeragi.peer_id.address,
        role=%state.current_topology.role(&sumeragi.peer_id),
        block_height=%state.latest_block_height,
        block_hash=%committed_block.hash(),
        "Committing block"
    );

    update_topology(state, sumeragi, &committed_block);

    sumeragi.kura.store_block(committed_block);

    cache_transaction(state, sumeragi);
}

fn replace_top_block(
    sumeragi: &Sumeragi,
    state: &mut State,
    block: impl Into<VersionedCommittedBlock>,
) {
    let committed_block = block.into();

    state.wsv = state.finalized_wsv.clone();
    update_state(state, sumeragi, &committed_block);
    // state.previous_block_hash stays the same.

    info!(
        addr=%sumeragi.peer_id.address,
        role=%state.current_topology.role(&sumeragi.peer_id),
        block_height=%state.latest_block_height,
        block_hash=%committed_block.hash(),
        "Replacing top block"
    );

    update_topology(state, sumeragi, &committed_block);

    sumeragi.kura.replace_top_block(committed_block);

    cache_transaction(state, sumeragi)
}

fn update_topology(
    state: &mut State,
    sumeragi: &Sumeragi,
    committed_block: &VersionedCommittedBlock,
) {
    let mut topology = Topology {
        sorted_peers: committed_block.header().committed_with_topology.clone(),
    };
    topology.lift_up_peers(
        &committed_block
            .signatures()
            .into_iter()
            .map(|s| s.public_key().clone())
            .collect::<Vec<PublicKey>>(),
    );
    topology.rotate_set_a();
    topology.update_peer_list(
        &state
            .wsv
            .peers_ids()
            .iter()
            .map(|id| id.clone())
            .collect::<Vec<PeerId>>(),
    );
    state.current_topology = topology;
    sumeragi.connect_peers(&state.current_topology);
}

fn update_state(state: &mut State, sumeragi: &Sumeragi, committed_block: &VersionedCommittedBlock) {
    state
        .wsv
        .apply(committed_block)
        .expect("Failed to apply block on WSV. Bailing.");

    sumeragi.send_events(state.wsv.events_buffer.replace(Vec::new()));

    // Update WSV copy that is public facing
    *sumeragi.wsv.lock() = state.wsv.clone();

    // This sends "Block committed" event, so it should be done
    // AFTER public facing WSV update
    sumeragi.send_events(committed_block);

    state.latest_block_height = committed_block.header().height;
    state.latest_block_hash = Some(committed_block.hash());
    state.latest_block_view_change_index = committed_block.header().view_change_index;
}

fn cache_transaction(state: &mut State, sumeragi: &Sumeragi) {
    let transaction_cache = &mut state.transaction_cache;
    transaction_cache.retain(|tx| {
        !tx.is_in_blockchain(&state.wsv) && !tx.is_expired(sumeragi.queue.tx_time_to_live)
    });
}

fn suggest_view_change(
    sumeragi: &Sumeragi,
    state: &State,
    view_change_proof_chain: &mut ProofChain,
    current_view_change_index: u64,
) {
    let suspect_proof = {
        let mut proof = Proof {
            latest_block_hash: state.latest_block_hash,
            view_change_index: current_view_change_index,
            signatures: Vec::new(),
        };
        proof
            .sign(sumeragi.key_pair.clone())
            .expect("Proof signing failed");
        proof
    };

    view_change_proof_chain
        .insert_proof(
            &state.current_topology.sorted_peers,
            state.current_topology.max_faults(),
            state.latest_block_hash,
            suspect_proof,
        )
        .unwrap_or_else(|err| error!("{err}"));

    let msg = MessagePacket::new(
        view_change_proof_chain.clone(),
        Message::ViewChangeSuggested,
    );
    sumeragi.broadcast_packet(msg, &state.current_topology);
}

fn prune_view_change_proofs_and_calculate_current_index(
    state: &State,
    view_change_proof_chain: &mut ProofChain,
) -> u64 {
    view_change_proof_chain.prune(state.latest_block_hash);
    view_change_proof_chain.verify_with_state(
        &state.current_topology.sorted_peers,
        state.current_topology.max_faults(),
        state.latest_block_hash,
    ) as u64
}

fn enqueue_transaction(sumeragi: &Sumeragi, wsv: &WorldStateView, tx: VersionedSignedTransaction) {
    let tx = tx.into_v1();

    let addr = &sumeragi.peer_id.address;
    match AcceptedTransaction::accept::<false>(tx, &sumeragi.transaction_limits) {
        Ok(tx) => match sumeragi.queue.push(tx.into(), wsv) {
            Ok(_) => {}
            Err(crate::queue::Failure {
                tx,
                err: crate::queue::Error::InBlockchain,
            }) => {
                debug!(tx_hash = %tx.hash(), "Transaction already in blockchain, ignoring...")
            }
            Err(crate::queue::Failure { tx, err }) => {
                error!(%addr, ?err, tx_hash = %tx.hash(), "Failed to enqueue transaction.")
            }
        },
        Err(err) => error!(%addr, %err, "Transaction rejected"),
    }
}

#[allow(clippy::too_many_lines)]
fn handle_message(
    message: Message,
    sumeragi: &Sumeragi,
    state: &mut State,
    voting_block: &mut Option<VotingBlock>,
    current_view_change_index: u64,
    view_change_proof_chain: &mut ProofChain,
    voting_signatures: &mut Vec<SignatureOf<PendingBlock>>,
) {
    let current_topology = &state.current_topology;
    let role = current_topology.role(&sumeragi.peer_id);
    let addr = &sumeragi.peer_id.address;

    match (message, role) {
        (Message::TransactionGossip(tx_gossip), _) => {
            for transaction in tx_gossip.txs {
                enqueue_transaction(sumeragi, &state.wsv, transaction);
            }
        }
        (Message::ViewChangeSuggested, _) => {
            trace!("Received view change suggestion.");
        }
        (Message::BlockSyncUpdate(block_sync_update), _) => {
            let block_hash = block_sync_update.hash();
            info!(%addr, %role, hash=%block_hash, "Block sync update received");

            let block = match block_sync_update
                .clone()
                .validate_and_extract_block::<false>(
                    &sumeragi.transaction_validator,
                    state.wsv.clone(),
                    state.latest_block_hash,
                    state.latest_block_height,
                )
                .or_else(|_|
                /* If the block fails validation we must check again using the finaziled wsv.
                When a soft-fork occurs the consensus-block may be valid on the previous
                wsv but not the current one. */
                block_sync_update.validate_and_extract_block::<false>(
                    &sumeragi.transaction_validator,
                    state.finalized_wsv.clone(),
                    state.previous_block_hash,
                    state.latest_block_height.saturating_sub(1),
                )) {
                Ok(block) => block,
                Err(error) => {
                    error!(%addr, %role, %block_hash, ?error, "Block not valid.");
                    return;
                }
            };

            if state.previous_block_hash == block.header().previous_block_hash
                && state.latest_block_height == block.header().height
                && state.latest_block_hash != Some(block.hash())
                && state.latest_block_view_change_index < block.header().view_change_index
            {
                error!(
                    %addr, %role,
                    peer_latest_block_hash=?state.latest_block_hash,
                    peer_latest_block_view_change_index=?state.latest_block_view_change_index,
                    consensus_latest_block_hash=%block.hash(),
                    consensus_latest_block_view_change_index=%block.header().view_change_index,
                    "Soft fork occurred: peer in inconsistent state. Rolling back and replacing top block."
                );
                replace_top_block(sumeragi, state, block);
                return;
            }
            if state.latest_block_hash != block.header().previous_block_hash {
                error!(
                    %addr, %role,
                    actual = ?block.header().previous_block_hash,
                    expected = ?state.latest_block_hash,
                    "Mismatch between the actual and expected hashes of the latest block."
                );
                return;
            }
            if state.latest_block_height + 1 != block.header().height {
                error!(
                    %addr, %role,
                    actual = block.header().height,
                    expected = state.latest_block_height + 1,
                    "Mismatch between the actual and expected height of the block."
                );
                return;
            }

            commit_block(sumeragi, state, block);
        }
        (Message::BlockCommitted(BlockCommitted { hash, signatures }), _) => {
            if role == Role::ProxyTail && current_topology.is_consensus_required()
                || role == Role::Leader && !current_topology.is_consensus_required()
            {
                error!(%addr, %role, "Received BlockCommitted message, but shouldn't");
            } else if let Some(mut voted_block) = voting_block.take() {
                let voting_block_hash = voted_block.block.hash();

                if hash == voting_block_hash.transmute() {
                    // The manipulation of the topology relies upon all peers seeing the same signature set.
                    // Therefore we must clear the signatures and accept what the proxy tail giveth.
                    voted_block.block.signatures.clear();
                    add_signatures::<true>(&mut voted_block, signatures.transmute());

                    match voted_block.block.commit(current_topology) {
                        Ok(committed_block) => commit_block(sumeragi, state, committed_block),
                        Err((_, err)) => {
                            error!(%addr, %role, %hash, ?err, "Block failed to be committed")
                        }
                    };
                } else {
                    error!(
                        %addr, %role, committed_block_hash=%hash, %voting_block_hash,
                        "The hash of the committed block does not match the hash of the block stored by the peer."
                    );

                    *voting_block = Some(voted_block);
                };
            } else {
                error!(%addr, %role, %hash, "Peer missing voting block")
            }
        }
        (Message::BlockCreated(block_created), Role::ValidatingPeer) => {
            if let Some(block) = vote_for_block(sumeragi, state, block_created) {
                let block_hash = block.block.hash();

                let msg = MessagePacket::new(
                    view_change_proof_chain.clone(),
                    BlockSigned::from(block.block.clone()),
                );

                sumeragi.broadcast_packet_to(msg, [current_topology.proxy_tail()]);
                info!(%addr, %block_hash, "Block validated, signed and forwarded");

                *voting_block = Some(block);
            }
        }
        (Message::BlockCreated(block_created), Role::ObservingPeer) => {
            if let Some(block) = vote_for_block(sumeragi, state, block_created) {
                if current_view_change_index >= 1 {
                    let block_hash = block.block.hash();

                    let msg = MessagePacket::new(
                        view_change_proof_chain.clone(),
                        BlockSigned::from(block.block.clone()),
                    );

                    sumeragi.broadcast_packet_to(msg, [current_topology.proxy_tail()]);
                    info!(%addr, %block_hash, "Block validated, signed and forwarded");
                }
                *voting_block = Some(block);
            }
        }
        (Message::BlockCreated(block_created), Role::ProxyTail) => {
            // NOTE: False positive from nursery
            #[allow(clippy::iter_with_drain)]
            if let Some(mut new_block) = vote_for_block(sumeragi, state, block_created) {
                // NOTE: Up until this point it was unknown which block is expected to be received,
                // therefore all the signatures (of any hash) were collected and will now be pruned
                add_signatures::<false>(&mut new_block, voting_signatures.drain(..));
                *voting_block = Some(new_block);
            }
        }
        (Message::BlockSigned(BlockSigned { hash, signatures }), Role::ProxyTail) => {
            trace!(block_hash=%hash, "Received block signatures");

            let roles: &[Role] = if current_view_change_index >= 1 {
                &[Role::ValidatingPeer, Role::ObservingPeer]
            } else {
                &[Role::ValidatingPeer]
            };
            let valid_signatures = current_topology.filter_signatures_by_roles(roles, &signatures);

            if let Some(voted_block) = voting_block.as_mut() {
                let voting_block_hash = voted_block.block.hash();

                if hash == voting_block_hash {
                    add_signatures::<true>(voted_block, valid_signatures);
                } else {
                    debug!(%voting_block_hash, "Received signatures are not for the current block");
                }
            } else {
                // NOTE: Due to the nature of distributed systems, signatures can sometimes be received before
                // the block (sent by the leader). Collect the signatures and wait for the block to be received
                voting_signatures.extend(valid_signatures);
            }
        }
        (msg, role) => {
            trace!(%addr, %role, ?msg, "message not handled")
        }
    }
}

fn process_message_independent(
    sumeragi: &Sumeragi,
    state: &mut State,
    voting_block: &mut Option<VotingBlock>,
    current_view_change_index: u64,
    view_change_proof_chain: &mut ProofChain,
    round_start_time: &Instant,
    is_genesis_peer: bool,
) {
    let current_topology = &state.current_topology;
    let role = current_topology.role(&sumeragi.peer_id);
    let addr = &sumeragi.peer_id.address;

    match role {
        Role::Leader => {
            if voting_block.is_none() {
                let cache_full = state.transaction_cache.len() >= sumeragi.queue.txs_in_block;
                let deadline_reached = round_start_time.elapsed() > sumeragi.block_time;

                if cache_full || (deadline_reached && !state.transaction_cache.is_empty()) {
                    let transactions = state.transaction_cache.clone();
                    info!(txns=%transactions.len(), "Creating block...");

                    // TODO: properly process triggers!
                    let event_recommendations = Vec::new();
                    let new_block = BlockBuilder {
                        transactions,
                        event_recommendations,
                        height: state.latest_block_height + 1,
                        previous_block_hash: state.latest_block_hash,
                        view_change_index: current_view_change_index,
                        committed_with_topology: state.current_topology.clone(),
                        key_pair: sumeragi.key_pair.clone(),
                        transaction_validator: &sumeragi.transaction_validator,
                        wsv: state.wsv.clone(),
                    }
                    .build();

                    sumeragi.send_events(&new_block);
                    if current_topology.is_consensus_required() {
                        info!(%addr, hash=%new_block.hash(), "Block created");
                        *voting_block = Some(VotingBlock::new(new_block.clone()));

                        let msg = MessagePacket::new(
                            view_change_proof_chain.clone(),
                            BlockCreated::from(new_block),
                        );
                        sumeragi.broadcast_packet(msg, current_topology);
                    } else {
                        match new_block.commit(current_topology) {
                            Ok(committed_block) => {
                                let msg = MessagePacket::new(
                                    view_change_proof_chain.clone(),
                                    BlockCommitted::from(Into::<VersionedCommittedBlock>::into(
                                        committed_block.clone(),
                                    )),
                                );

                                sumeragi.broadcast_packet(msg, current_topology);
                                commit_block(sumeragi, state, committed_block);
                            }
                            Err(err) => error!(%addr, role=%Role::Leader, ?err),
                        }
                    }
                }
            }
        }
        Role::ProxyTail => {
            if let Some(voted_block) = voting_block.take() {
                let voted_at = voted_block.voted_at;

                match voted_block.block.commit(current_topology) {
                    Ok(committed_block) => {
                        info!(voting_block_hash = %committed_block.hash(), "Block reached required number of votes");

                        let msg = MessagePacket::new(
                            view_change_proof_chain.clone(),
                            BlockCommitted::from(Into::<VersionedCommittedBlock>::into(
                                committed_block.clone(),
                            )),
                        );

                        #[cfg(debug_assertions)]
                        if is_genesis_peer && sumeragi.debug_force_soft_fork {
                            std::thread::sleep(sumeragi.pipeline_time() * 2);
                        } else {
                            sumeragi.broadcast_packet(msg, current_topology);
                        }

                        #[cfg(not(debug_assertions))]
                        {
                            sumeragi.broadcast_packet(msg, current_topology);
                        }
                        commit_block(sumeragi, state, committed_block);
                    }
                    Err((block, err)) => {
                        // Restore the current voting block and continue the round
                        *voting_block = Some(VotingBlock::voted_at(block, voted_at));
                        trace!(?err, "Not enough signatures, waiting for more...");
                    }
                }
            }
        }
        _ => {}
    }
}

// NOTE: False positive useless_let_if_seq from nursery
#[allow(clippy::too_many_arguments, clippy::useless_let_if_seq)]
fn reset_state(
    peer_id: &PeerId,
    pipeline_time: Duration,
    current_view_change_index: u64,
    old_view_change_index: &mut u64,
    current_latest_block_height: u64,
    old_latest_block_height: &mut u64,
    // below is the state that gets reset.
    current_topology: &mut Topology,
    voting_block: &mut Option<VotingBlock>,
    voting_signatures: &mut Vec<SignatureOf<PendingBlock>>,
    round_start_time: &mut Instant,
    last_view_change_time: &mut Instant,
    view_change_time: &mut Duration,
) {
    let mut was_commit_or_view_change = false;
    if current_latest_block_height != *old_latest_block_height {
        // Round is only restarted on a block commit, so that in the case of
        // a view change a new block is immediately created by the leader
        *round_start_time = Instant::now();
        was_commit_or_view_change = true;
        *old_view_change_index = 0;
    }

    while *old_view_change_index < current_view_change_index {
        *old_view_change_index += 1;
        error!(addr=%peer_id.address, "Rotating the entire topology.");
        current_topology.rotate_all();
        was_commit_or_view_change = true;
    }

    // Reset state for the next round.
    if was_commit_or_view_change {
        *old_latest_block_height = current_latest_block_height;

        *voting_block = None;
        voting_signatures.clear();
        *last_view_change_time = Instant::now();
        *view_change_time = pipeline_time;
        info!(addr=%peer_id.address, role=%current_topology.role(peer_id), %current_view_change_index, "View change updated");
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
    genesis_network: Option<GenesisNetwork>,
    sumeragi: &Sumeragi,
    mut state: State,
    mut shutdown_receiver: tokio::sync::oneshot::Receiver<()>,
) {
    // Connect peers with initial topology
    sumeragi.connect_peers(&state.current_topology);

    let span = span!(tracing::Level::TRACE, "genesis").entered();
    let is_genesis_peer = if state.latest_block_height == 0 || state.latest_block_hash.is_none() {
        if let Some(genesis_network) = genesis_network {
            sumeragi_init_commit_genesis(sumeragi, &mut state, genesis_network);
            true
        } else {
            sumeragi
                .init_listen_for_genesis(&mut state, &mut shutdown_receiver)
                .unwrap_or_else(|err| assert_ne!(EarlyReturn::Disconnected, err, "Disconnected"));
            false
        }
    } else {
        false
    };
    span.exit();

    // Assert initialization was done properly.
    assert_eq!(state.latest_block_hash, state.wsv.latest_block_hash());
    trace!(
        me=%sumeragi.peer_id.public_key,
        role_in_next_round=%state.current_topology.role(&sumeragi.peer_id),
        "Finished sumeragi init.",
    );

    let mut voting_block = None;
    // Proxy tail collection of voting block signatures
    let mut voting_signatures = Vec::new();
    let mut should_sleep = false;
    let mut last_sent_transaction_gossip_time = Instant::now();
    let mut view_change_proof_chain = ProofChain::default();
    let mut old_view_change_index = 0;
    let mut old_latest_block_height = 0;
    // Duration after which a view change is suggested
    let mut view_change_time = sumeragi.pipeline_time();
    // Instant when the current round started
    let mut round_start_time = Instant::now();
    // Instant when the previous view change or round happened.
    let mut last_view_change_time = Instant::now();

    while !should_terminate(&mut shutdown_receiver) {
        if should_sleep {
            let span = span!(Level::TRACE, "main_thread_sleep");
            let _enter = span.enter();
            std::thread::sleep(std::time::Duration::from_millis(5));
            should_sleep = false;
        }
        let span_for_sumeragi_cycle = span!(Level::TRACE, "main_thread_cycle");
        let _enter_for_sumeragi_cycle = span_for_sumeragi_cycle.enter();

        state
            .transaction_cache
            // Checking if transactions are in the blockchain is costly
            .retain(|tx| !tx.is_expired(sumeragi.queue.tx_time_to_live));

        let mut expired_transactions = Vec::new();
        sumeragi.queue.get_transactions_for_block(
            &state.wsv,
            &mut state.transaction_cache,
            &mut expired_transactions,
        );
        sumeragi.send_events(
            expired_transactions
                .iter()
                .map(expired_event)
                .collect::<Vec<_>>(),
        );

        if last_sent_transaction_gossip_time.elapsed() > sumeragi.gossip_period {
            sumeragi.gossip_transactions(&state, &view_change_proof_chain);
            last_sent_transaction_gossip_time = Instant::now();
        }

        let current_view_change_index = prune_view_change_proofs_and_calculate_current_index(
            &state,
            &mut view_change_proof_chain,
        );

        reset_state(
            &sumeragi.peer_id,
            sumeragi.pipeline_time(),
            current_view_change_index,
            &mut old_view_change_index,
            state.latest_block_height,
            &mut old_latest_block_height,
            &mut state.current_topology,
            &mut voting_block,
            &mut voting_signatures,
            &mut round_start_time,
            &mut last_view_change_time,
            &mut view_change_time,
        );

        let node_expects_block = !state.transaction_cache.is_empty();
        if node_expects_block && last_view_change_time.elapsed() > view_change_time {
            let role = state.current_topology.role(&sumeragi.peer_id);

            if let Some(VotingBlock { block, .. }) = voting_block.as_ref() {
                // NOTE: Suspecting the tail node because it hasn't yet committed a block produced by leader
                warn!(peer_public_key=%sumeragi.peer_id.public_key, %role, block=%block.hash(), "Block not committed in due time, requesting view change...");
            } else {
                // NOTE: Suspecting the leader node because it hasn't produced a block
                // If the current node has a transaction, the leader should have as well
                warn!(peer_public_key=%sumeragi.peer_id.public_key, %role, "No block produced in due time, requesting view change...");
            }

            suggest_view_change(
                sumeragi,
                &state,
                &mut view_change_proof_chain,
                current_view_change_index,
            );

            // NOTE: View change must be periodically suggested until it is accepted.
            // Must be initialized to pipeline time but can increase by chosen amount
            view_change_time += sumeragi.pipeline_time();
        }

        sumeragi
            .receive_network_packet(&state, &mut view_change_proof_chain)
            .map_or_else(
                || {
                    should_sleep = true;
                },
                |message| {
                    handle_message(
                        message,
                        sumeragi,
                        &mut state,
                        &mut voting_block,
                        current_view_change_index,
                        &mut view_change_proof_chain,
                        &mut voting_signatures,
                    );
                },
            );

        process_message_independent(
            sumeragi,
            &mut state,
            &mut voting_block,
            current_view_change_index,
            &mut view_change_proof_chain,
            &round_start_time,
            is_genesis_peer,
        );
    }
}

fn add_signatures<const EXPECT_VALID: bool>(
    block: &mut VotingBlock,
    signatures: impl IntoIterator<Item = SignatureOf<PendingBlock>>,
) {
    for signature in signatures {
        if let Err(err) = block.block.add_signature(signature) {
            let err_msg = "Signature not valid";

            if EXPECT_VALID {
                error!(?err, err_msg);
            } else {
                debug!(?err, err_msg);
            }
        }
    }
}

/// Create expired pipeline event for the given transaction.
fn expired_event(txn: &impl Transaction) -> Event {
    PipelineEvent {
        entity_kind: PipelineEntityKind::Transaction,
        status: PipelineStatus::Rejected(PipelineRejectionReason::Transaction(
            TransactionRejectionReason::Expired(TransactionExpired {
                time_to_live_ms: txn.payload().time_to_live_ms,
            }),
        )),
        hash: txn.hash().into(),
    }
    .into()
}

fn vote_for_block(
    sumeragi: &Sumeragi,
    state: &State,
    block_created: BlockCreated,
) -> Option<VotingBlock> {
    let block_hash = block_created.hash();
    let addr = &sumeragi.peer_id.address;
    let role = state.current_topology.role(&sumeragi.peer_id);
    trace!(%addr, %role, block_hash=%block_hash, "Block received, voting...");

    let mut block = {
        let span = span!(Level::TRACE, "block revalidation");
        let _enter = span.enter();

        match block_created.validate_and_extract_block::<false>(
            &sumeragi.transaction_validator,
            state.wsv.clone(),
            state.latest_block_hash,
            state.latest_block_height,
        ) {
            Ok(block) => block,
            Err(err) => {
                warn!(%addr, %role, ?err);
                return None;
            }
        }
    };

    if state
        .current_topology
        .filter_signatures_by_roles(&[Role::Leader], block.retain_verified_signatures())
        .is_empty()
    {
        error!(
            %addr, %role, leader=%state.current_topology.leader().address, hash=%block.hash(),
            "The block is rejected as it is not signed by the leader."
        );

        return None;
    }

    if block.header.committed_with_topology != state.current_topology.sorted_peers {
        error!(
            %addr, %role, block_topology=?block.header.committed_with_topology, my_topology=?state.current_topology, hash=%block.hash(),
            "The block is rejected as because the topology field is incorrect."
        );

        return None;
    }

    let signed_block = block
        .sign(sumeragi.key_pair.clone())
        .expect("Block signing failed");

    sumeragi.send_events(&signed_block);
    Some(VotingBlock::new(signed_block))
}

fn sumeragi_init_commit_genesis(
    sumeragi: &Sumeragi,
    state: &mut State,
    genesis_network: GenesisNetwork,
) {
    std::thread::sleep(Duration::from_millis(250));

    info!("Initializing iroha using the genesis block.");

    assert_eq!(state.latest_block_height, 0);
    assert_eq!(state.latest_block_hash, None);

    let transactions = genesis_network.transactions;
    // Don't start genesis round. Instead just commit the genesis block.
    assert!(
        !transactions.is_empty(),
        "Genesis transaction set contains no valid transactions"
    );

    let block = BlockBuilder {
        transactions,
        event_recommendations: Vec::new(),
        height: 1,
        previous_block_hash: None,
        view_change_index: 0,
        committed_with_topology: state.current_topology.clone(),
        key_pair: sumeragi.key_pair.clone(),
        transaction_validator: &sumeragi.transaction_validator,
        wsv: state.wsv.clone(),
    }
    .build();

    {
        info!(block_hash = %block.hash(), "Publishing genesis block.");

        info!(
            role = ?state.current_topology.role(&sumeragi.peer_id),
            block_hash = %block.hash(),
            "Created a block to commit.",
        );

        sumeragi.send_events(&block);
        let msg = MessagePacket::new(ProofChain::default(), BlockCreated::from(block.clone()));
        sumeragi.broadcast_packet(msg, &state.current_topology);
        // Omit signature verification during genesis round
        commit_block(sumeragi, state, block.commit_unchecked());
    }
}

/// Type enumerating early return types to reduce cyclomatic
/// complexity of the main loop items and allow direct short
/// circuiting with the `?` operator. Candidate for `impl
/// FromResidual`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EarlyReturn {
    /// Genesis block received and committed
    GenesisBlockReceivedAndCommitted,
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
