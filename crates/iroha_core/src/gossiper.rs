//! Gossiper actor responsible for transaction gossiping.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    io::Write,
    num::{NonZeroU32, NonZeroUsize},
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use iroha_config::parameters::actual::{
    DataspaceGossip, DataspaceGossipFallback, LaneConfig as LaneGeometry, Network as NetworkConfig,
    RestrictedPublicPayload, TransactionGossiper as Config,
};
use iroha_crypto::{HashOf, KeyPair};
use iroha_data_model::{
    ChainId, DataSpaceId,
    account::AccountId,
    domain::DomainId,
    isi::InstructionBox,
    nexus::{LaneCatalog, LaneId, LaneVisibility},
    peer::PeerId,
    transaction::SignedTransaction,
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_p2p::{Broadcast, Post, Priority};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
    core as ncore,
};
use tokio::sync::mpsc;

use crate::{
    IrohaNetwork, NetworkMessage,
    queue::{GossipBatchEntry, Queue, RoutingDecision},
    state::State,
    tx::AcceptedTransaction,
};

/// Grouped gossip entries and the lanes they originated from.
#[derive(Default)]
struct DataspaceBatch {
    entries: Vec<GossipBatchEntry>,
    lanes: BTreeSet<LaneId>,
}

#[derive(Debug, PartialEq, Eq)]
enum RestrictedTargetPlan {
    Send {
        targets: Vec<PeerId>,
        fallback_used: bool,
        fallback_surface: Option<&'static str>,
        reason: Option<&'static str>,
    },
    Drop {
        reason: &'static str,
        fallback_used: bool,
        fallback_surface: Option<&'static str>,
        targets: Vec<PeerId>,
    },
}

const DROP_REASON_NO_RESTRICTED_TARGETS: &str = "no_restricted_targets";
const DROP_REASON_PUBLIC_OVERLAY_REFUSED: &str = "restricted_public_overlay_refused";
const DROP_REASON_ROUTE_MISMATCH: &str = "route_mismatch";
const DROP_REASON_PEER_RECENT_SUPPRESSION: &str = "peer_recent_suppression";
const OUTCOME_PUBLIC_OVERLAY_FORWARD: &str = "restricted_public_overlay_forward";
const SURFACE_PUBLIC_OVERLAY: &str = "public_overlay";
const GOSSIP_SEED_PUBLIC_DOMAIN: u64 = 0x5055_424C_4943_5F00;
const GOSSIP_SEED_RESTRICTED_DOMAIN: u64 = 0x5245_5354_5249_4354;
const GOSSIP_PEER_RECENT_SUPPRESSION_TTL_TICKS: usize = 8;

#[derive(Debug, Clone)]
struct PeerRecentSuppressionEntry {
    peer_id: PeerId,
    tx_hash: HashOf<SignedTransaction>,
    expires_tick: u64,
}

fn tx_gossip_frame_payload_cap(
    network_cfg: &NetworkConfig,
    chain_id: &ChainId,
    self_peer_id: &PeerId,
    max_peer_id: &PeerId,
) -> usize {
    let plaintext_cap = network_cfg
        .max_frame_bytes_tx_gossip
        .min(iroha_p2p::frame_plaintext_cap(network_cfg.max_frame_bytes));
    if plaintext_cap == 0 {
        return 0;
    }
    let dummy_keypair = KeyPair::random();
    let dummy_domain = DomainId::from_str("dummy").expect("static domain id should parse");
    let dummy_authority = AccountId::new(dummy_domain, dummy_keypair.public_key().clone());
    let dummy_signed =
        iroha_data_model::transaction::TransactionBuilder::new(chain_id.clone(), dummy_authority)
            .with_instructions(std::iter::empty::<InstructionBox>())
            .sign(dummy_keypair.private_key());
    let probe_payload_len = plaintext_cap;
    let payload = Arc::new(vec![0u8; probe_payload_len]);
    let probe_gossip = TransactionGossip {
        txs: vec![GossipTransaction::with_encoded(dummy_signed, payload)],
        routes: vec![GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
        }],
        plane: GossipPlane::Public,
    };
    let gossip_len = probe_gossip
        .encoded_len_exact()
        .or_else(|| probe_gossip.encoded_len_hint())
        .unwrap_or(0);
    let payload = NetworkMessage::TransactionGossiper(Arc::new(probe_gossip));
    let direct_len = iroha_p2p::network::data_frame_wire_len(
        self_peer_id,
        Some(max_peer_id),
        network_cfg.relay_ttl,
        Priority::Low,
        &payload,
    );
    let broadcast_len = iroha_p2p::network::data_frame_wire_len(
        self_peer_id,
        None,
        network_cfg.relay_ttl,
        Priority::Low,
        &payload,
    );
    let envelope_len = direct_len.max(broadcast_len).saturating_sub(gossip_len);
    plaintext_cap.saturating_sub(envelope_len)
}

fn splitmix64(mut state: u64) -> u64 {
    state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[derive(Debug)]
struct GossipTargetSeed {
    seed: u64,
    last_reshuffle: Instant,
    reshuffle_period: Duration,
}

impl GossipTargetSeed {
    fn new(seed: u64, reshuffle_period: Duration, now: Instant) -> Self {
        Self {
            seed,
            last_reshuffle: now,
            reshuffle_period,
        }
    }

    fn current(&mut self, now: Instant) -> u64 {
        if now.saturating_duration_since(self.last_reshuffle) >= self.reshuffle_period {
            self.seed = splitmix64(self.seed);
            self.last_reshuffle = now;
        }
        self.seed
    }
}

/// [`TransactionGossiper`] actor handle.
#[derive(Clone)]
pub struct TransactionGossiperHandle {
    message_sender: mpsc::Sender<Arc<TransactionGossip>>,
}

impl TransactionGossiperHandle {
    /// Send [`TransactionGossip`] to actor.
    ///
    /// Messages are best-effort: if the queue is full, the gossip is dropped
    /// to avoid blocking consensus traffic.
    pub fn gossip(&self, gossip: Arc<TransactionGossip>) {
        let txs = gossip.txs.len();
        let plane = gossip_plane_label(gossip.plane);
        match self.message_sender.try_send(gossip) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                iroha_logger::debug!(
                    plane,
                    txs,
                    "transaction gossiper queue full; dropping gossip"
                );
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                iroha_logger::warn!(
                    plane,
                    txs,
                    "transaction gossiper channel closed; dropping gossip"
                );
            }
        }
    }
}

#[cfg(test)]
mod handle_tests {
    use tokio::sync::mpsc;

    use super::*;

    #[test]
    fn gossip_drops_when_queue_full() {
        let (message_sender, mut message_receiver) = mpsc::channel(1);
        let handle = TransactionGossiperHandle { message_sender };

        let msg1 = TransactionGossip {
            txs: Vec::new(),
            routes: Vec::new(),
            plane: GossipPlane::Public,
        };
        let msg2 = TransactionGossip {
            txs: Vec::new(),
            routes: Vec::new(),
            plane: GossipPlane::Restricted,
        };

        handle
            .message_sender
            .try_send(Arc::new(msg1))
            .expect("queue has space");
        handle.gossip(Arc::new(msg2));

        let received = message_receiver
            .try_recv()
            .expect("expected queued message");
        assert_eq!(received.plane, GossipPlane::Public);
        assert!(matches!(
            message_receiver.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }
}

/// Actor which gossips transactions and receives transaction gossips
pub struct TransactionGossiper {
    /// Unique id of the blockchain. Used for simple replay attack protection.
    chain_id: ChainId,
    /// The time between gossip messages. More frequent gossiping shortens
    /// the time to sync, but can overload the network.
    gossip_period: Duration,
    /// Maximum size of a batch that is being gossiped. Smaller size leads
    /// to longer time to synchronise, useful if you have high packet loss.
    gossip_size: NonZeroU32,
    /// Number of gossip periods to wait before re-sending the same transactions.
    gossip_resend_ticks: NonZeroU32,
    /// Monotonic tick counter for gossip resend pacing.
    gossip_tick: u64,
    /// Deferred gossip hashes bucketed by resend tick.
    gossip_deferred: Vec<Vec<HashOf<SignedTransaction>>>,
    /// Recently-sent transaction hashes tracked per peer to suppress duplicate fanout.
    peer_recently_sent: BTreeMap<PeerId, HashMap<HashOf<SignedTransaction>, u64>>,
    /// Expiry ring for per-peer suppression entries (tick-based TTL).
    peer_recent_ring: Vec<Vec<PeerRecentSuppressionEntry>>,
    /// Subscriber-queue drop counter at the last backpressure observation.
    last_drop_count: u64,
    /// Timestamp of the last observed subscriber-queue drop.
    last_drop_at: Option<Instant>,
    network: IrohaNetwork,
    queue: Arc<Queue>,
    state: Arc<State>,
    tx_frame_cap: usize,
    dataspace_cfg: DataspaceGossip,
    public_seed: GossipTargetSeed,
    restricted_seed: GossipTargetSeed,
}

impl TransactionGossiper {
    /// Start [`Self`] actor.
    pub fn start(self, shutdown_signal: ShutdownSignal) -> (TransactionGossiperHandle, Child) {
        let (message_sender, message_receiver) = mpsc::channel(1);
        (
            TransactionGossiperHandle { message_sender },
            Child::new(
                tokio::task::spawn(self.run(message_receiver, shutdown_signal)),
                OnShutdown::Abort,
            ),
        )
    }

    /// Construct [`Self`] from configuration
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn from_config(
        chain_id: ChainId,
        Config {
            gossip_period,
            gossip_size,
            gossip_resend_ticks,
            dataspace,
        }: Config,
        network_cfg: &NetworkConfig,
        self_peer_id: PeerId,
        max_peer_id: PeerId,
        network: IrohaNetwork,
        queue: Arc<Queue>,
        state: Arc<State>,
    ) -> Self {
        let now = Instant::now();
        let dataspace_cfg = dataspace;
        let public_seed =
            GossipTargetSeed::new(rand::random(), dataspace_cfg.public_target_reshuffle, now);
        let restricted_seed = GossipTargetSeed::new(
            rand::random(),
            dataspace_cfg.restricted_target_reshuffle,
            now,
        );
        // Keep gossip batches below the plaintext per-topic cap while respecting the encrypted
        // frame ceiling and the P2P message envelope overhead.
        let tx_frame_cap =
            tx_gossip_frame_payload_cap(network_cfg, &chain_id, &self_peer_id, &max_peer_id);
        let gossip_deferred = vec![Vec::new(); gossip_resend_ticks.get() as usize];
        let peer_recent_ring = vec![Vec::new(); GOSSIP_PEER_RECENT_SUPPRESSION_TTL_TICKS];
        Self {
            chain_id,
            gossip_period,
            gossip_size,
            gossip_resend_ticks,
            gossip_tick: 0,
            gossip_deferred,
            peer_recently_sent: BTreeMap::new(),
            peer_recent_ring,
            last_drop_count: 0,
            last_drop_at: None,
            network,
            queue,
            state,
            tx_frame_cap,
            dataspace_cfg,
            public_seed,
            restricted_seed,
        }
    }

    async fn run(
        mut self,
        mut message_receiver: mpsc::Receiver<Arc<TransactionGossip>>,
        shutdown_signal: ShutdownSignal,
    ) {
        let mut gossip_period = tokio::time::interval(self.gossip_period);
        loop {
            tokio::select! {
                _ = gossip_period.tick() => self.gossip_transactions(),
                Some(transaction_gossip) = message_receiver.recv() => {
                    self.handle_transaction_gossip(transaction_gossip);
                }
                () = shutdown_signal.receive() => {
                    iroha_logger::debug!("Shutting down transactions gossiper");
                    break;
                },
            }
            tokio::task::yield_now().await;
        }
    }

    fn deferred_index(&self) -> usize {
        let len = u64::from(self.gossip_resend_ticks.get());
        let len_usize = usize::try_from(len).expect("gossip_resend_ticks fits usize");
        debug_assert!(len > 0, "gossip_resend_ticks must be non-zero");
        debug_assert_eq!(
            self.gossip_deferred.len(),
            len_usize,
            "gossip_deferred must match gossip_resend_ticks"
        );
        usize::try_from(self.gossip_tick % len).expect("gossip_resend_ticks fits usize")
    }

    fn backpressure_cooldown(&self) -> Duration {
        self.gossip_period
            .checked_mul(self.gossip_resend_ticks.get())
            .unwrap_or(self.gossip_period)
    }

    fn gossip_backpressure_active(&mut self, now: Instant) -> bool {
        let current = iroha_p2p::network::subscriber_queue_full_count();
        if current > self.last_drop_count {
            self.last_drop_count = current;
            self.last_drop_at = Some(now);
        }
        let cooldown = self.backpressure_cooldown();
        self.last_drop_at
            .is_some_and(|last| now.saturating_duration_since(last) < cooldown)
    }

    fn release_deferred_gossip(&mut self) {
        if self.gossip_deferred.is_empty() {
            return;
        }
        let index = self.deferred_index();
        let hashes = std::mem::take(&mut self.gossip_deferred[index]);
        if hashes.is_empty() {
            return;
        }
        self.queue.requeue_gossip_hashes(hashes);
    }

    fn defer_gossip_hashes(&mut self, hashes: impl IntoIterator<Item = HashOf<SignedTransaction>>) {
        if self.gossip_deferred.is_empty() {
            return;
        }
        let index = self.deferred_index();
        self.gossip_deferred[index].extend(hashes);
    }

    fn advance_gossip_tick(&mut self) {
        self.gossip_tick = self.gossip_tick.wrapping_add(1);
    }

    fn peer_recent_slot_for_tick(&self, tick: u64) -> usize {
        let slots = self.peer_recent_ring.len();
        if slots == 0 {
            return 0;
        }
        let slots_u64 = u64::try_from(slots).expect("peer_recent_ring length fits u64");
        usize::try_from(tick % slots_u64).expect("slot index fits usize")
    }

    fn expire_peer_recent_suppression(&mut self) {
        if self.peer_recent_ring.is_empty() {
            return;
        }
        let slot_idx = self.peer_recent_slot_for_tick(self.gossip_tick);
        let expiring = std::mem::take(&mut self.peer_recent_ring[slot_idx]);
        if expiring.is_empty() {
            return;
        }
        let mut empty_peers = BTreeSet::new();
        let mut deferred = Vec::new();
        for entry in expiring {
            let peer_id = entry.peer_id.clone();
            if let Some(peer_map) = self.peer_recently_sent.get_mut(&peer_id) {
                let should_remove = peer_map.get(&entry.tx_hash).copied().is_some_and(|expiry| {
                    expiry == entry.expires_tick && expiry <= self.gossip_tick
                });
                if should_remove {
                    peer_map.remove(&entry.tx_hash);
                } else {
                    deferred.push(entry);
                }
                if peer_map.is_empty() {
                    empty_peers.insert(peer_id);
                }
            } else {
                deferred.push(entry);
            }
        }
        for peer in empty_peers {
            self.peer_recently_sent.remove(&peer);
        }
        for entry in deferred {
            let deferred_slot = self.peer_recent_slot_for_tick(entry.expires_tick);
            self.peer_recent_ring[deferred_slot].push(entry);
        }
    }

    fn peer_recently_seen_all_hashes(
        &self,
        peer_id: &PeerId,
        tx_hashes: &[HashOf<SignedTransaction>],
    ) -> bool {
        self.peer_recently_sent
            .get(peer_id)
            .is_some_and(|seen| tx_hashes.iter().all(|hash| seen.contains_key(hash)))
    }

    fn filter_targets_by_peer_recent_suppression(
        &self,
        targets: Vec<PeerId>,
        tx_hashes: &[HashOf<SignedTransaction>],
    ) -> (Vec<PeerId>, usize) {
        if tx_hashes.is_empty() || targets.is_empty() {
            return (targets, 0);
        }
        let mut filtered = Vec::with_capacity(targets.len());
        let mut suppressed = 0usize;
        for peer in targets {
            if self.peer_recently_seen_all_hashes(&peer, tx_hashes) {
                suppressed = suppressed.saturating_add(1);
            } else {
                filtered.push(peer);
            }
        }
        (filtered, suppressed)
    }

    fn remember_peer_recent_sends(
        &mut self,
        targets: &[PeerId],
        tx_hashes: &[HashOf<SignedTransaction>],
    ) {
        if targets.is_empty() || tx_hashes.is_empty() || self.peer_recent_ring.is_empty() {
            return;
        }
        let ttl_ticks =
            u64::try_from(self.peer_recent_ring.len()).expect("peer_recent_ring length fits u64");
        let expires_tick = self.gossip_tick.saturating_add(ttl_ticks);
        let slot_idx = self.peer_recent_slot_for_tick(expires_tick);
        let slot = &mut self.peer_recent_ring[slot_idx];
        for peer_id in targets {
            let peer_map = self.peer_recently_sent.entry(peer_id.clone()).or_default();
            for tx_hash in tx_hashes {
                peer_map.insert(tx_hash.clone(), expires_tick);
                slot.push(PeerRecentSuppressionEntry {
                    peer_id: peer_id.clone(),
                    tx_hash: tx_hash.clone(),
                    expires_tick,
                });
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn gossip_transactions(&mut self) {
        let now = Instant::now();
        if self.gossip_backpressure_active(now) {
            iroha_logger::trace!(
                drops = self.last_drop_count,
                cooldown_ms = self.backpressure_cooldown().as_millis(),
                "transaction gossiper skipping gossip due to relay backpressure"
            );
            return;
        }
        self.expire_peer_recent_suppression();
        self.release_deferred_gossip();
        let (entries, lane_config, lane_catalog, commit_topology) = {
            let nexus = self.state.nexus_snapshot();
            let lane_config = nexus.lane_config.clone();
            let lane_catalog = nexus.lane_catalog.clone();
            let entries = self
                .queue
                .gossip_batch_with_state(self.gossip_size.get(), &self.state);
            let commit_topology = self.state.commit_topology_snapshot();
            (entries, lane_config, lane_catalog, commit_topology)
        };

        if entries.is_empty() {
            self.advance_gossip_tick();
            return;
        }
        let public_seed = self.public_seed.current(now);
        let restricted_seed = self.restricted_seed.current(now);

        #[cfg(feature = "telemetry")]
        {
            self.record_gossip_caps();
        }

        let mut grouped: BTreeMap<DataSpaceId, DataspaceBatch> = BTreeMap::new();
        for entry in entries {
            let route = GossipRoute {
                lane_id: entry.routing.lane_id,
                dataspace_id: entry.routing.dataspace_id,
            };
            if let Err(reason) = validate_route(&lane_catalog, route) {
                iroha_logger::warn!(
                    lane_id = %route.lane_id,
                    dataspace_id = %route.dataspace_id,
                    reason,
                    "dropping transaction gossip entry before broadcast"
                );
                let plane = dataspace_plane(&lane_config, route.dataspace_id)
                    .unwrap_or(GossipPlane::Restricted);
                self.record_drop_metric(
                    plane,
                    route.dataspace_id,
                    &[route.lane_id],
                    reason,
                    false,
                    None,
                    &[],
                    self.target_cap_for_plane(plane),
                    0,
                    0,
                );
                continue;
            }

            let entry_slot = grouped.entry(entry.routing.dataspace_id).or_default();
            entry_slot.lanes.insert(route.lane_id);
            entry_slot.entries.push(entry);
        }

        for (dataspace_id, batch) in grouped {
            let mut lane_ids = Vec::with_capacity(batch.lanes.len());
            lane_ids.extend(batch.lanes.iter().copied());
            let entries = batch.entries;
            let plane = dataspace_plane(&lane_config, dataspace_id).or({
                if self.dataspace_cfg.drop_unknown_dataspace {
                    None
                } else {
                    Some(GossipPlane::Restricted)
                }
            });

            let Some(plane) = plane else {
                iroha_logger::warn!(
                    dataspace = %dataspace_id,
                    "dataspace missing from lane catalog; requeueing gossip batch"
                );
                self.defer_gossip_hashes(entries.iter().map(|entry| entry.tx.as_ref().hash()));
                self.record_drop_metric(
                    GossipPlane::Restricted,
                    dataspace_id,
                    &lane_ids,
                    "unknown_dataspace",
                    false,
                    None,
                    &[],
                    self.target_cap_for_plane(GossipPlane::Restricted),
                    entries.len(),
                    0,
                );
                continue;
            };

            match plane {
                GossipPlane::Public => {
                    self.gossip_public(dataspace_id, &lane_ids, entries, public_seed);
                }
                GossipPlane::Restricted => self.gossip_restricted(
                    dataspace_id,
                    &lane_ids,
                    entries,
                    &commit_topology,
                    restricted_seed,
                ),
            }
        }
        self.advance_gossip_tick();
    }

    #[allow(clippy::too_many_lines)]
    fn gossip_public(
        &mut self,
        dataspace_id: DataSpaceId,
        lane_ids: &[LaneId],
        entries: Vec<GossipBatchEntry>,
        gossip_seed: u64,
    ) {
        if entries.is_empty() {
            return;
        }
        let PartitionedGossipBatch {
            message,
            requeue,
            encoded_len,
        } = partition_gossip_batch(
            self.gossip_size.get() as usize,
            self.tx_frame_cap,
            GossipPlane::Public,
            entries,
        );

        if !requeue.is_empty() {
            self.defer_gossip_hashes(requeue);
        }

        if message.txs.is_empty() {
            iroha_logger::debug!(
                frame_cap = self.tx_frame_cap,
                "Skipping transaction gossip broadcast because current frame cap cannot fit any transaction"
            );
            self.record_drop_metric(
                GossipPlane::Public,
                dataspace_id,
                lane_ids,
                "frame_cap_too_small",
                false,
                None,
                &[],
                self.target_cap_for_plane(GossipPlane::Public),
                0,
                encoded_len,
            );
            return;
        }

        let mut sent_hashes = Vec::with_capacity(message.txs.len());
        for tx in &message.txs {
            sent_hashes.push(tx.as_signed().hash());
        }
        let batch_txs = message.txs.len();
        let frame_bytes = encoded_len;

        let targets: Vec<PeerId> = self
            .network
            .online_peers(|online| online.iter().map(|peer| peer.id().clone()).collect());
        let seed = Self::seed_for_plane(gossip_seed, dataspace_id, GOSSIP_SEED_PUBLIC_DOMAIN);
        let (targets, total_online) =
            Self::select_targets_with_seed(targets, self.dataspace_cfg.public_target_cap, seed);
        let (targets, suppressed_targets) =
            self.filter_targets_by_peer_recent_suppression(targets, &sent_hashes);

        if targets.is_empty() {
            iroha_logger::debug!(
                tx_count = batch_txs,
                dataspace = %dataspace_id,
                suppressed_targets,
                "skipping public gossip batch after per-peer suppression"
            );
            self.defer_gossip_hashes(sent_hashes);
            self.record_drop_metric(
                GossipPlane::Public,
                dataspace_id,
                lane_ids,
                DROP_REASON_PEER_RECENT_SUPPRESSION,
                false,
                None,
                &[],
                self.target_cap_for_plane(GossipPlane::Public),
                batch_txs,
                frame_bytes,
            );
            return;
        }

        let message = Arc::new(message);
        if self.dataspace_cfg.public_target_cap.is_some() {
            iroha_logger::debug!(
                tx_count = batch_txs,
                size_bytes = encoded_len,
                targets = targets.len(),
                suppressed_targets,
                online_peers = total_online,
                dataspace = %dataspace_id,
                "gossiping public transaction batch to capped target set"
            );
            let payload = NetworkMessage::TransactionGossiper(Arc::clone(&message));
            for peer_id in &targets {
                self.network.post(Post {
                    data: payload.clone(),
                    peer_id: peer_id.clone(),
                    priority: Priority::Low,
                });
            }
            self.record_sent_metric(
                GossipPlane::Public,
                dataspace_id,
                lane_ids,
                &targets,
                self.dataspace_cfg.public_target_cap,
                batch_txs,
                frame_bytes,
                false,
                None,
                None,
            );
            self.remember_peer_recent_sends(&targets, &sent_hashes);
            // Re-enqueue sent hashes when we gossip to a capped target set so the batch can
            // continue spreading to other peers in subsequent rounds.
            self.defer_gossip_hashes(sent_hashes);
        } else {
            if suppressed_targets == 0 {
                iroha_logger::debug!(
                    tx_count = batch_txs,
                    size_bytes = encoded_len,
                    online_peers = total_online,
                    dataspace = %dataspace_id,
                    "broadcasting transaction gossip batch"
                );
                self.network.broadcast(Broadcast {
                    data: NetworkMessage::TransactionGossiper(Arc::clone(&message)),
                    priority: Priority::Low,
                });
            } else {
                iroha_logger::debug!(
                    tx_count = batch_txs,
                    size_bytes = encoded_len,
                    targets = targets.len(),
                    suppressed_targets,
                    online_peers = total_online,
                    dataspace = %dataspace_id,
                    "gossiping public transaction batch to unsuppressed peer subset"
                );
                let payload = NetworkMessage::TransactionGossiper(Arc::clone(&message));
                for peer_id in &targets {
                    self.network.post(Post {
                        data: payload.clone(),
                        peer_id: peer_id.clone(),
                        priority: Priority::Low,
                    });
                }
            }
            self.record_sent_metric(
                GossipPlane::Public,
                dataspace_id,
                lane_ids,
                &targets,
                self.dataspace_cfg.public_target_cap,
                batch_txs,
                frame_bytes,
                false,
                None,
                None,
            );
            self.remember_peer_recent_sends(&targets, &sent_hashes);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn gossip_restricted(
        &mut self,
        dataspace_id: DataSpaceId,
        lane_ids: &[LaneId],
        entries: Vec<GossipBatchEntry>,
        commit_topology: &[PeerId],
        gossip_seed: u64,
    ) {
        if entries.is_empty() {
            return;
        }

        let PartitionedGossipBatch {
            message,
            requeue,
            encoded_len,
        } = partition_gossip_batch(
            self.gossip_size.get() as usize,
            self.tx_frame_cap,
            GossipPlane::Restricted,
            entries,
        );

        if !requeue.is_empty() {
            self.defer_gossip_hashes(requeue);
        }

        if message.txs.is_empty() {
            iroha_logger::debug!(
                frame_cap = self.tx_frame_cap,
                %dataspace_id,
                "Skipping restricted transaction gossip because current frame cap cannot fit any transaction"
            );
            self.record_drop_metric(
                GossipPlane::Restricted,
                dataspace_id,
                lane_ids,
                "frame_cap_too_small",
                false,
                None,
                &[],
                self.target_cap_for_plane(GossipPlane::Restricted),
                0,
                encoded_len,
            );
            return;
        }

        let batch_txs = message.txs.len();
        let frame_bytes = encoded_len;
        let mut sent_hashes = Vec::with_capacity(message.txs.len());
        for tx in &message.txs {
            sent_hashes.push(tx.as_signed().hash());
        }

        let seed = Self::seed_for_plane(gossip_seed, dataspace_id, GOSSIP_SEED_RESTRICTED_DOMAIN);
        let plan = self.restricted_target_plan(commit_topology, batch_txs, seed);
        let (targets, fallback_used, fallback_surface, reason) = match plan {
            RestrictedTargetPlan::Send {
                targets,
                fallback_used,
                fallback_surface,
                reason,
            } => (targets, fallback_used, fallback_surface, reason),
            RestrictedTargetPlan::Drop {
                reason,
                fallback_used,
                fallback_surface,
                targets,
            } => {
                self.defer_gossip_hashes(sent_hashes);
                self.record_drop_metric(
                    GossipPlane::Restricted,
                    dataspace_id,
                    lane_ids,
                    reason,
                    fallback_used,
                    fallback_surface,
                    &targets,
                    self.target_cap_for_plane(GossipPlane::Restricted),
                    batch_txs,
                    encoded_len,
                );
                return;
            }
        };
        let (targets, suppressed_targets) =
            self.filter_targets_by_peer_recent_suppression(targets, &sent_hashes);
        if targets.is_empty() {
            self.defer_gossip_hashes(sent_hashes);
            self.record_drop_metric(
                GossipPlane::Restricted,
                dataspace_id,
                lane_ids,
                DROP_REASON_PEER_RECENT_SUPPRESSION,
                fallback_used,
                fallback_surface,
                &[],
                self.target_cap_for_plane(GossipPlane::Restricted),
                batch_txs,
                encoded_len,
            );
            return;
        }

        let message = Arc::new(message);
        let payload = NetworkMessage::TransactionGossiper(Arc::clone(&message));
        for peer_id in &targets {
            self.network.post(Post {
                data: payload.clone(),
                peer_id: peer_id.clone(),
                priority: Priority::Low,
            });
        }

        iroha_logger::debug!(
            tx_count = message.txs.len(),
            size_bytes = encoded_len,
            targets = targets.len(),
            suppressed_targets,
            %dataspace_id,
            "gossiping restricted transactions to online commit topology"
        );
        self.record_sent_metric(
            GossipPlane::Restricted,
            dataspace_id,
            lane_ids,
            &targets,
            self.dataspace_cfg.restricted_target_cap,
            batch_txs,
            frame_bytes,
            fallback_used,
            fallback_surface,
            reason,
        );
        self.remember_peer_recent_sends(&targets, &sent_hashes);
        self.defer_gossip_hashes(sent_hashes);
    }

    #[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
    #[allow(clippy::unused_self)]
    #[allow(clippy::too_many_arguments)]
    fn record_drop_metric(
        &self,
        plane: GossipPlane,
        dataspace: DataSpaceId,
        lane_ids: &[LaneId],
        reason: &str,
        fallback_used: bool,
        fallback_surface: Option<&str>,
        targets: &[PeerId],
        target_cap: Option<NonZeroUsize>,
        batch_txs: usize,
        frame_bytes: usize,
    ) {
        iroha_logger::debug!(
            %dataspace,
            reason,
            plane = gossip_plane_label(plane),
            fallback_used,
            lanes = ?lane_ids,
            "transaction gossip drop recorded for dataspace"
        );
        #[cfg(not(feature = "telemetry"))]
        {
            let _ = (
                fallback_surface,
                targets,
                target_cap,
                batch_txs,
                frame_bytes,
            );
        }
        #[cfg(feature = "telemetry")]
        {
            self.state.telemetry.record_tx_gossip_attempt(
                plane,
                dataspace,
                lane_ids,
                targets,
                target_cap,
                false,
                Some(reason),
                fallback_used,
                fallback_surface,
                batch_txs,
                frame_bytes,
            );
        }
    }

    #[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
    #[allow(clippy::unused_self)]
    #[allow(clippy::too_many_arguments)]
    fn record_sent_metric(
        &self,
        plane: GossipPlane,
        dataspace: DataSpaceId,
        lane_ids: &[LaneId],
        targets: &[PeerId],
        target_cap: Option<NonZeroUsize>,
        batch_txs: usize,
        frame_bytes: usize,
        fallback_used: bool,
        fallback_surface: Option<&str>,
        reason: Option<&str>,
    ) {
        iroha_logger::debug!(
            %dataspace,
            targets = targets.len(),
            plane = gossip_plane_label(plane),
            fallback_used,
            lanes = ?lane_ids,
            "transaction gossip sent metric recorded"
        );
        #[cfg(not(feature = "telemetry"))]
        {
            let _ = (target_cap, batch_txs, frame_bytes, fallback_surface, reason);
        }
        #[cfg(feature = "telemetry")]
        {
            self.state.telemetry.record_tx_gossip_attempt(
                plane,
                dataspace,
                lane_ids,
                targets,
                target_cap,
                true,
                reason,
                fallback_used,
                fallback_surface,
                batch_txs,
                frame_bytes,
            );
        }
    }

    #[cfg(feature = "telemetry")]
    fn record_gossip_caps(&self) {
        self.state.telemetry.record_tx_gossip_caps(
            self.tx_frame_cap,
            self.dataspace_cfg.public_target_cap.map(NonZeroUsize::get),
            self.dataspace_cfg
                .restricted_target_cap
                .map(NonZeroUsize::get),
            self.dataspace_cfg.drop_unknown_dataspace,
            self.dataspace_cfg.restricted_fallback,
            self.dataspace_cfg.restricted_public_payload,
            self.dataspace_cfg.public_target_reshuffle,
            self.dataspace_cfg.restricted_target_reshuffle,
        );
    }

    fn target_cap_for_plane(&self, plane: GossipPlane) -> Option<NonZeroUsize> {
        match plane {
            GossipPlane::Public => self.dataspace_cfg.public_target_cap,
            GossipPlane::Restricted => self.dataspace_cfg.restricted_target_cap,
        }
    }

    fn seed_for_plane(seed: u64, dataspace_id: DataSpaceId, domain: u64) -> u64 {
        splitmix64(seed ^ dataspace_id.as_u64() ^ domain)
    }

    /// Deterministically shuffle targets by seed and return a capped subset.
    fn select_targets_with_seed(
        mut targets: Vec<PeerId>,
        cap: Option<NonZeroUsize>,
        seed: u64,
    ) -> (Vec<PeerId>, usize) {
        targets.sort();
        targets.dedup();
        let total = targets.len();
        let Some(cap) = cap else {
            return (targets, total);
        };
        let cap = cap.get();
        if total <= cap {
            return (targets, total);
        }
        let mut scored: Vec<(u64, PeerId)> = Vec::with_capacity(total);
        for peer in targets {
            scored.push((Self::peer_target_score(&peer, seed), peer));
        }
        scored.sort_by_key(|(score, _)| *score);
        scored.truncate(cap);
        let mut targets = Vec::with_capacity(scored.len());
        for (_, peer) in scored {
            targets.push(peer);
        }
        (targets, total)
    }

    /// Stable score for a peer keyed by the gossip seed.
    fn peer_target_score(peer_id: &PeerId, seed: u64) -> u64 {
        let (algorithm, payload) = peer_id.public_key().to_bytes();
        let mut state = seed ^ u64::from(algorithm as u8);
        for chunk in payload.chunks(8) {
            let mut buf = [0u8; 8];
            buf[..chunk.len()].copy_from_slice(chunk);
            state = splitmix64(state ^ u64::from_le_bytes(buf));
        }
        splitmix64(state)
    }

    fn restricted_target_plan_with_targets(
        commit_topology: Vec<PeerId>,
        fallback_targets: Vec<PeerId>,
        target_cap: Option<NonZeroUsize>,
        fallback_policy: DataspaceGossipFallback,
        payload_policy: RestrictedPublicPayload,
        tx_count: usize,
        seed: u64,
    ) -> RestrictedTargetPlan {
        let online: BTreeSet<_> = fallback_targets.iter().cloned().collect();
        let mut filtered_commit = Vec::with_capacity(commit_topology.len());
        for peer in commit_topology {
            if online.contains(&peer) {
                filtered_commit.push(peer);
            }
        }
        let commit_topology = filtered_commit;
        let (capped_commit, _) = Self::select_targets_with_seed(commit_topology, target_cap, seed);
        if !capped_commit.is_empty() {
            return RestrictedTargetPlan::Send {
                targets: capped_commit,
                fallback_used: false,
                fallback_surface: None,
                reason: None,
            };
        }

        let (capped_fallback, _) =
            Self::select_targets_with_seed(fallback_targets, target_cap, seed);
        decide_restricted_target_plan(capped_fallback, fallback_policy, payload_policy, tx_count)
    }

    fn restricted_target_plan(
        &self,
        commit_topology: &[PeerId],
        tx_count: usize,
        seed: u64,
    ) -> RestrictedTargetPlan {
        let fallback_targets: Vec<PeerId> = self.network.online_peers(|online| {
            let mut peers = Vec::with_capacity(online.len());
            for peer in online {
                peers.push(peer.id().clone());
            }
            peers
        });
        Self::restricted_target_plan_with_targets(
            commit_topology.to_vec(),
            fallback_targets,
            self.dataspace_cfg.restricted_target_cap,
            self.dataspace_cfg.restricted_fallback,
            self.dataspace_cfg.restricted_public_payload,
            tx_count,
            seed,
        )
    }

    fn is_transaction_known_locally(&self, tx_hash: HashOf<SignedTransaction>) -> bool {
        if self.queue.contains_transaction_hash(tx_hash) {
            return true;
        }
        self.state.has_committed_transaction(tx_hash)
    }

    fn is_transaction_known_locally_cached(&self, tx_hash: HashOf<SignedTransaction>) -> bool {
        if GOSSIP_KNOWN_TX_HASH_CACHE.with(|cache| cache.borrow().contains(tx_hash)) {
            return true;
        }
        if self.is_transaction_known_locally(tx_hash) {
            GOSSIP_KNOWN_TX_HASH_CACHE.with(|cache| cache.borrow_mut().remember(tx_hash));
            return true;
        }
        false
    }

    fn handle_transaction_gossip(&self, gossip: Arc<TransactionGossip>) {
        match Arc::try_unwrap(gossip) {
            Ok(owned) => self.handle_transaction_gossip_owned(owned),
            Err(shared) => self.handle_transaction_gossip_shared(shared.as_ref()),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn handle_transaction_gossip_owned(
        &self,
        TransactionGossip { txs, routes, plane }: TransactionGossip,
    ) {
        iroha_logger::debug!(size = txs.len(), "received transaction gossip batch");
        let batch_txs = txs.len();

        if routes.is_empty() {
            iroha_logger::warn!("dropping transaction gossip without routing metadata");
            self.record_drop_metric(
                plane,
                DataSpaceId::GLOBAL,
                &[],
                "missing_routes",
                false,
                None,
                &[],
                self.target_cap_for_plane(plane),
                batch_txs,
                0,
            );
            return;
        }

        if routes.len() != txs.len() {
            let dataspace = routes
                .first()
                .map_or(DataSpaceId::GLOBAL, |route| route.dataspace_id);
            iroha_logger::warn!(
                routes = routes.len(),
                txs = txs.len(),
                "dropping transaction gossip batch due to route/tx length mismatch"
            );
            self.record_drop_metric(
                plane,
                dataspace,
                routes
                    .first()
                    .map(|route| vec![route.lane_id])
                    .unwrap_or_default()
                    .as_slice(),
                "route_tx_len_mismatch",
                false,
                None,
                &[],
                self.target_cap_for_plane(plane),
                batch_txs,
                0,
            );
            return;
        }

        let nexus = self.state.nexus_snapshot();
        let lane_catalog = nexus.lane_catalog.clone();
        let lane_config = nexus.lane_config.clone();
        let (max_clock_drift, tx_limits) = {
            let world_view = self.state.world_view();
            let params = &world_view.parameters;
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let crypto_cfg = self.state.crypto();
        let mut batch_seen_hashes = HashSet::with_capacity(batch_txs);
        let state = self.state.as_ref();

        for (idx, tx) in txs.into_iter().enumerate() {
            let Some(route) = routes.get(idx).copied() else {
                iroha_logger::warn!("route metadata missing for transaction gossip entry");
                self.record_drop_metric(
                    plane,
                    DataSpaceId::GLOBAL,
                    &[],
                    "missing_route_entry",
                    false,
                    None,
                    &[],
                    self.target_cap_for_plane(plane),
                    1,
                    0,
                );
                continue;
            };
            let tx_hash = tx.hash();
            if !batch_seen_hashes.insert(tx_hash.clone()) {
                crate::sumeragi::status::inc_gossip_duplicate_known_skipped();
                continue;
            }
            if self.is_transaction_known_locally_cached(tx_hash) {
                crate::sumeragi::status::inc_gossip_duplicate_known_skipped();
                continue;
            }
            if let Err(reason) = validate_route(&lane_catalog, route) {
                iroha_logger::warn!(
                    lane_id = %route.lane_id,
                    dataspace_id = %route.dataspace_id,
                    reason,
                    "dropping transaction gossip entry due to invalid route"
                );
                self.record_drop_metric(
                    plane,
                    route.dataspace_id,
                    &[route.lane_id],
                    reason,
                    false,
                    None,
                    &[],
                    self.target_cap_for_plane(plane),
                    1,
                    0,
                );
                continue;
            }
            let expected_plane = dataspace_plane(&lane_config, route.dataspace_id).or({
                if self.dataspace_cfg.drop_unknown_dataspace {
                    None
                } else {
                    Some(GossipPlane::Restricted)
                }
            });
            let Some(expected_plane) = expected_plane else {
                iroha_logger::warn!(
                    lane_id = %route.lane_id,
                    dataspace_id = %route.dataspace_id,
                    "dropping transaction gossip entry due to unknown dataspace"
                );
                self.record_drop_metric(
                    plane,
                    route.dataspace_id,
                    &[route.lane_id],
                    "unknown_dataspace",
                    false,
                    None,
                    &[],
                    self.target_cap_for_plane(plane),
                    1,
                    0,
                );
                continue;
            };
            if plane == GossipPlane::Restricted && route.dataspace_id == DataSpaceId::GLOBAL {
                iroha_logger::warn!(
                    lane_id = %route.lane_id,
                    "restricted plane reported global dataspace; dropping entry"
                );
                self.record_drop_metric(
                    plane,
                    route.dataspace_id,
                    &[route.lane_id],
                    "restricted_global_dataspace",
                    false,
                    None,
                    &[],
                    self.target_cap_for_plane(plane),
                    1,
                    0,
                );
                continue;
            }
            if expected_plane != plane {
                iroha_logger::warn!(
                    lane_id = %route.lane_id,
                    dataspace_id = %route.dataspace_id,
                    plane = ?plane,
                    expected_plane = ?expected_plane,
                    "dropping transaction gossip entry due to plane mismatch"
                );
                self.record_drop_metric(
                    plane,
                    route.dataspace_id,
                    &[route.lane_id],
                    "plane_mismatch",
                    false,
                    None,
                    &[],
                    self.target_cap_for_plane(plane),
                    1,
                    0,
                );
                continue;
            }

            let (signed, payload) = tx.into_signed_with_payload();
            match AcceptedTransaction::accept(
                signed,
                &self.chain_id,
                max_clock_drift,
                tx_limits,
                crypto_cfg.as_ref(),
            ) {
                Ok(tx) => {
                    let advertised_route = RoutingDecision::new(route.lane_id, route.dataspace_id);
                    let local_route = self.queue.route_for_gossip_with_state(&tx, state);
                    if local_route != advertised_route {
                        iroha_logger::warn!(
                                %tx_hash,
                                advertised_lane_id = %route.lane_id,
                                advertised_dataspace_id = %route.dataspace_id,
                            expected_lane_id = %local_route.lane_id,
                            expected_dataspace_id = %local_route.dataspace_id,
                            "dropping transaction gossip entry due to routing mismatch"
                        );
                        self.record_drop_metric(
                            plane,
                            local_route.dataspace_id,
                            &[local_route.lane_id],
                            DROP_REASON_ROUTE_MISMATCH,
                            false,
                            None,
                            &[],
                            self.target_cap_for_plane(plane),
                            1,
                            0,
                        );
                        continue;
                    }
                    match self
                        .queue
                        .push_with_gossip_payload_with_state(tx, state, payload)
                    {
                        Ok(()) => {
                            iroha_logger::debug!(%tx_hash, "transaction enqueued from gossip");
                        }
                        Err(crate::queue::Failure {
                            tx,
                            err: crate::queue::Error::InBlockchain,
                        }) => {
                            iroha_logger::debug!(
                                tx = %tx.as_ref().as_ref().hash(),
                                "Transaction already in blockchain, ignoring..."
                            )
                        }
                        Err(crate::queue::Failure {
                            tx,
                            err: crate::queue::Error::IsInQueue,
                        }) => {
                            iroha_logger::trace!(
                                tx = %tx.as_ref().as_ref().hash(),
                                "Transaction already in the queue, ignoring..."
                            )
                        }
                        Err(crate::queue::Failure { tx, err }) => {
                            iroha_logger::error!(
                                ?err,
                                tx = %tx.as_ref().as_ref().hash(),
                                "Failed to enqueue transaction."
                            )
                        }
                    }
                }
                Err(err) => iroha_logger::error!(%err, "Transaction rejected"),
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn handle_transaction_gossip_shared(&self, gossip: &TransactionGossip) {
        let txs = &gossip.txs;
        let routes = &gossip.routes;
        let plane = gossip.plane;

        iroha_logger::debug!(size = txs.len(), "received transaction gossip batch");
        let batch_txs = txs.len();

        if routes.is_empty() {
            iroha_logger::warn!("dropping transaction gossip without routing metadata");
            self.record_drop_metric(
                plane,
                DataSpaceId::GLOBAL,
                &[],
                "missing_routes",
                false,
                None,
                &[],
                self.target_cap_for_plane(plane),
                batch_txs,
                0,
            );
            return;
        }

        if routes.len() != txs.len() {
            let dataspace = routes
                .first()
                .map_or(DataSpaceId::GLOBAL, |route| route.dataspace_id);
            iroha_logger::warn!(
                routes = routes.len(),
                txs = txs.len(),
                "dropping transaction gossip batch due to route/tx length mismatch"
            );
            self.record_drop_metric(
                plane,
                dataspace,
                routes
                    .first()
                    .map(|route| vec![route.lane_id])
                    .unwrap_or_default()
                    .as_slice(),
                "route_tx_len_mismatch",
                false,
                None,
                &[],
                self.target_cap_for_plane(plane),
                batch_txs,
                0,
            );
            return;
        }

        let nexus = self.state.nexus_snapshot();
        let lane_catalog = nexus.lane_catalog.clone();
        let lane_config = nexus.lane_config.clone();
        let (max_clock_drift, tx_limits) = {
            let world_view = self.state.world_view();
            let params = &world_view.parameters;
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let crypto_cfg = self.state.crypto();
        let mut batch_seen_hashes = HashSet::with_capacity(batch_txs);
        let state = self.state.as_ref();

        for (idx, tx) in txs.iter().enumerate() {
            let Some(route) = routes.get(idx).copied() else {
                iroha_logger::warn!("route metadata missing for transaction gossip entry");
                self.record_drop_metric(
                    plane,
                    DataSpaceId::GLOBAL,
                    &[],
                    "missing_route_entry",
                    false,
                    None,
                    &[],
                    self.target_cap_for_plane(plane),
                    1,
                    0,
                );
                continue;
            };
            let tx_hash = tx.hash();
            if !batch_seen_hashes.insert(tx_hash.clone()) {
                crate::sumeragi::status::inc_gossip_duplicate_known_skipped();
                continue;
            }
            if self.is_transaction_known_locally_cached(tx_hash) {
                crate::sumeragi::status::inc_gossip_duplicate_known_skipped();
                continue;
            }
            if let Err(reason) = validate_route(&lane_catalog, route) {
                iroha_logger::warn!(
                    lane_id = %route.lane_id,
                    dataspace_id = %route.dataspace_id,
                    reason,
                    "dropping transaction gossip entry due to invalid route"
                );
                self.record_drop_metric(
                    plane,
                    route.dataspace_id,
                    &[route.lane_id],
                    reason,
                    false,
                    None,
                    &[],
                    self.target_cap_for_plane(plane),
                    1,
                    0,
                );
                continue;
            }
            let expected_plane = dataspace_plane(&lane_config, route.dataspace_id).or({
                if self.dataspace_cfg.drop_unknown_dataspace {
                    None
                } else {
                    Some(GossipPlane::Restricted)
                }
            });
            let Some(expected_plane) = expected_plane else {
                iroha_logger::warn!(
                    lane_id = %route.lane_id,
                    dataspace_id = %route.dataspace_id,
                    "dropping transaction gossip entry due to unknown dataspace"
                );
                self.record_drop_metric(
                    plane,
                    route.dataspace_id,
                    &[route.lane_id],
                    "unknown_dataspace",
                    false,
                    None,
                    &[],
                    self.target_cap_for_plane(plane),
                    1,
                    0,
                );
                continue;
            };
            if plane == GossipPlane::Restricted && route.dataspace_id == DataSpaceId::GLOBAL {
                iroha_logger::warn!(
                    lane_id = %route.lane_id,
                    "restricted plane reported global dataspace; dropping entry"
                );
                self.record_drop_metric(
                    plane,
                    route.dataspace_id,
                    &[route.lane_id],
                    "restricted_global_dataspace",
                    false,
                    None,
                    &[],
                    self.target_cap_for_plane(plane),
                    1,
                    0,
                );
                continue;
            }
            if expected_plane != plane {
                iroha_logger::warn!(
                    lane_id = %route.lane_id,
                    dataspace_id = %route.dataspace_id,
                    plane = ?plane,
                    expected_plane = ?expected_plane,
                    "dropping transaction gossip entry due to plane mismatch"
                );
                self.record_drop_metric(
                    plane,
                    route.dataspace_id,
                    &[route.lane_id],
                    "plane_mismatch",
                    false,
                    None,
                    &[],
                    self.target_cap_for_plane(plane),
                    1,
                    0,
                );
                continue;
            }

            let signed = (*tx.signed).clone();
            let payload = tx.encoded.as_ref().map(Arc::clone);
            match AcceptedTransaction::accept(
                signed,
                &self.chain_id,
                max_clock_drift,
                tx_limits,
                crypto_cfg.as_ref(),
            ) {
                Ok(tx) => {
                    let advertised_route = RoutingDecision::new(route.lane_id, route.dataspace_id);
                    let local_route = self.queue.route_for_gossip_with_state(&tx, state);
                    if local_route != advertised_route {
                        iroha_logger::warn!(
                                %tx_hash,
                                advertised_lane_id = %route.lane_id,
                                advertised_dataspace_id = %route.dataspace_id,
                            expected_lane_id = %local_route.lane_id,
                            expected_dataspace_id = %local_route.dataspace_id,
                            "dropping transaction gossip entry due to routing mismatch"
                        );
                        self.record_drop_metric(
                            plane,
                            local_route.dataspace_id,
                            &[local_route.lane_id],
                            DROP_REASON_ROUTE_MISMATCH,
                            false,
                            None,
                            &[],
                            self.target_cap_for_plane(plane),
                            1,
                            0,
                        );
                        continue;
                    }
                    match self
                        .queue
                        .push_with_gossip_payload_with_state(tx, state, payload)
                    {
                        Ok(()) => {
                            iroha_logger::debug!(%tx_hash, "transaction enqueued from gossip");
                        }
                        Err(crate::queue::Failure {
                            tx,
                            err: crate::queue::Error::InBlockchain,
                        }) => {
                            iroha_logger::debug!(
                                tx = %tx.as_ref().as_ref().hash(),
                                "Transaction already in blockchain, ignoring..."
                            )
                        }
                        Err(crate::queue::Failure {
                            tx,
                            err: crate::queue::Error::IsInQueue,
                        }) => {
                            iroha_logger::trace!(
                                tx = %tx.as_ref().as_ref().hash(),
                                "Transaction already in the queue, ignoring..."
                            )
                        }
                        Err(crate::queue::Failure { tx, err }) => {
                            iroha_logger::error!(
                                ?err,
                                tx = %tx.as_ref().as_ref().hash(),
                                "Failed to enqueue transaction."
                            )
                        }
                    }
                }
                Err(err) => iroha_logger::error!(%err, "Transaction rejected"),
            }
        }
    }
}

fn decide_restricted_target_plan(
    fallback_targets: Vec<PeerId>,
    fallback_policy: DataspaceGossipFallback,
    payload_policy: RestrictedPublicPayload,
    tx_count: usize,
) -> RestrictedTargetPlan {
    match fallback_policy {
        DataspaceGossipFallback::Drop => RestrictedTargetPlan::Drop {
            reason: DROP_REASON_NO_RESTRICTED_TARGETS,
            fallback_used: false,
            fallback_surface: None,
            targets: fallback_targets,
        },
        DataspaceGossipFallback::UsePublicOverlay => {
            if fallback_targets.is_empty() {
                iroha_logger::warn!(tx_count, "restricted gossip fallback found no online peers");
                return RestrictedTargetPlan::Drop {
                    reason: DROP_REASON_NO_RESTRICTED_TARGETS,
                    fallback_used: true,
                    fallback_surface: Some(SURFACE_PUBLIC_OVERLAY),
                    targets: fallback_targets,
                };
            }
            match payload_policy {
                RestrictedPublicPayload::Forward => {
                    iroha_logger::warn!(
                        tx_count,
                        targets = fallback_targets.len(),
                        "restricted gossip forwarded to public overlay per configuration"
                    );
                    RestrictedTargetPlan::Send {
                        targets: fallback_targets,
                        fallback_used: true,
                        fallback_surface: Some(SURFACE_PUBLIC_OVERLAY),
                        reason: Some(OUTCOME_PUBLIC_OVERLAY_FORWARD),
                    }
                }
                RestrictedPublicPayload::Refuse => {
                    iroha_logger::warn!(
                        tx_count,
                        targets = fallback_targets.len(),
                        "restricted gossip fallback refused due to overlay policy"
                    );
                    RestrictedTargetPlan::Drop {
                        reason: DROP_REASON_PUBLIC_OVERLAY_REFUSED,
                        fallback_used: true,
                        fallback_surface: Some(SURFACE_PUBLIC_OVERLAY),
                        targets: fallback_targets,
                    }
                }
            }
        }
    }
}

fn validate_route(lane_catalog: &LaneCatalog, route: GossipRoute) -> Result<(), &'static str> {
    let Some(lane) = lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == route.lane_id)
    else {
        return Err("lane missing from catalog");
    };
    if lane.dataspace_id != route.dataspace_id {
        return Err("route dataspace does not match lane catalog");
    }
    Ok(())
}

fn dataspace_plane(lane_config: &LaneGeometry, dataspace_id: DataSpaceId) -> Option<GossipPlane> {
    let mut plane: Option<GossipPlane> = None;
    for entry in lane_config.entries() {
        if entry.dataspace_id != dataspace_id {
            continue;
        }
        let entry_plane = match entry.visibility {
            LaneVisibility::Public => GossipPlane::Public,
            LaneVisibility::Restricted => GossipPlane::Restricted,
        };
        plane = match plane {
            Some(GossipPlane::Restricted) => Some(GossipPlane::Restricted),
            Some(GossipPlane::Public) if entry_plane == GossipPlane::Restricted => {
                Some(GossipPlane::Restricted)
            }
            Some(existing) => Some(existing),
            None => Some(entry_plane),
        };
        if entry_plane == GossipPlane::Restricted {
            break;
        }
    }
    plane
}

pub(crate) fn gossip_plane_label(plane: GossipPlane) -> &'static str {
    match plane {
        GossipPlane::Public => "public",
        GossipPlane::Restricted => "restricted",
    }
}

#[cfg(test)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn dataspace_label(dataspace: DataSpaceId) -> String {
    dataspace.as_u64().to_string()
}

/// Message for gossiping batches of transactions.
#[derive(Decode, Debug, Clone)]
pub struct TransactionGossip {
    /// Batch of transactions.
    pub txs: Vec<GossipTransaction>,
    /// Routing metadata aligned with `txs`.
    pub routes: Vec<GossipRoute>,
    /// Visibility plane this batch targets.
    pub plane: GossipPlane,
}

impl TransactionGossip {
    /// Constructor.
    pub fn new(txs: Vec<AcceptedTransaction<'static>>) -> Self {
        let mut gossip_txs = Vec::with_capacity(txs.len());
        gossip_txs.extend(txs.into_iter().map(GossipTransaction::new));
        Self {
            // Converting into non-accepted transaction because it's not possible
            // to guarantee that the sending peer checked transaction limits
            txs: gossip_txs,
            routes: Vec::new(),
            plane: GossipPlane::Public,
        }
    }
}

impl NoritoSerialize for TransactionGossip {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), ncore::Error> {
        let mut tmp = ncore::DeriveSmallBuf::new();
        ncore::write_len_prefixed(&mut writer, &self.txs, &mut tmp)?;
        ncore::write_len_prefixed(&mut writer, &self.routes, &mut tmp)?;
        ncore::write_len_prefixed(&mut writer, &self.plane, &mut tmp)?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.encoded_len_exact()
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        let txs_payload_len = gossip_vec_payload_len_cached(self.txs.iter())
            .or_else(|| gossip_vec_payload_len_exact(self.txs.iter()))?;
        let routes_payload_len = gossip_routes_payload_len(self.routes.len())?;
        gossip_message_encoded_len(txs_payload_len, routes_payload_len)
    }
}

/// Gossip payload wrapper for signed transactions.
#[derive(Debug, Clone)]
pub struct GossipTransaction {
    signed: Arc<SignedTransaction>,
    encoded: Option<Arc<Vec<u8>>>,
    tx_hash: HashOf<SignedTransaction>,
}

const GOSSIP_TX_DECODE_CACHE_LIMIT: usize = 2048;
const GOSSIP_KNOWN_TX_HASH_CACHE_LIMIT: usize = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct GossipTxDecodeCacheKey {
    len: u32,
    prefix: u128,
    suffix: u128,
}

impl GossipTxDecodeCacheKey {
    fn from_bytes(bytes: &[u8]) -> Self {
        let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        let mut prefix_bytes = [0u8; 16];
        let mut suffix_bytes = [0u8; 16];

        let prefix_len = bytes.len().min(prefix_bytes.len());
        prefix_bytes[..prefix_len].copy_from_slice(&bytes[..prefix_len]);

        let suffix_len = bytes.len().min(suffix_bytes.len());
        if suffix_len > 0 {
            suffix_bytes[..suffix_len].copy_from_slice(&bytes[bytes.len() - suffix_len..]);
        }

        Self {
            len,
            prefix: u128::from_le_bytes(prefix_bytes),
            suffix: u128::from_le_bytes(suffix_bytes),
        }
    }
}

struct GossipTxDecodeCacheEntry {
    signed: Arc<SignedTransaction>,
    encoded: Arc<Vec<u8>>,
    tx_hash: HashOf<SignedTransaction>,
    consumed: usize,
}

struct GossipTxDecodeCache {
    map: HashMap<GossipTxDecodeCacheKey, GossipTxDecodeCacheEntry>,
}

impl GossipTxDecodeCache {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    fn get(&self, key: &GossipTxDecodeCacheKey) -> Option<&GossipTxDecodeCacheEntry> {
        self.map.get(key)
    }

    fn insert(&mut self, key: GossipTxDecodeCacheKey, entry: GossipTxDecodeCacheEntry) {
        if self.map.len() >= GOSSIP_TX_DECODE_CACHE_LIMIT {
            // Simple bounded cache: clear rather than paying LRU bookkeeping cost.
            self.map.clear();
        }
        self.map.insert(key, entry);
    }
}

thread_local! {
    static GOSSIP_TX_DECODE_CACHE: RefCell<GossipTxDecodeCache> =
        RefCell::new(GossipTxDecodeCache::new());
}

#[derive(Default)]
struct GossipKnownTxHashCache {
    known: HashMap<HashOf<SignedTransaction>, ()>,
}

impl GossipKnownTxHashCache {
    fn contains(&self, tx_hash: HashOf<SignedTransaction>) -> bool {
        self.known.contains_key(&tx_hash)
    }

    fn remember(&mut self, tx_hash: HashOf<SignedTransaction>) {
        if self.known.len() >= GOSSIP_KNOWN_TX_HASH_CACHE_LIMIT {
            // Simple bounded cache: clear when full to keep overhead predictable.
            self.known.clear();
        }
        self.known.insert(tx_hash, ());
    }
}

thread_local! {
    static GOSSIP_KNOWN_TX_HASH_CACHE: RefCell<GossipKnownTxHashCache> =
        RefCell::new(GossipKnownTxHashCache::default());
}

fn decode_gossip_transaction_payload(
    bytes: &[u8],
) -> Result<
    (
        Arc<SignedTransaction>,
        Arc<Vec<u8>>,
        HashOf<SignedTransaction>,
        usize,
    ),
    ncore::Error,
> {
    if let Some(hit) = GOSSIP_TX_DECODE_CACHE.with(|cache| {
        let cache = cache.borrow();
        let key = GossipTxDecodeCacheKey::from_bytes(bytes);
        cache.get(&key).and_then(|entry| {
            // Key collisions must not produce incorrect transactions. Confirm the actual bytes
            // match the cached encoded payload before reusing it.
            if entry.consumed <= bytes.len() && entry.encoded.as_slice() == &bytes[..entry.consumed]
            {
                Some((
                    Arc::clone(&entry.signed),
                    Arc::clone(&entry.encoded),
                    entry.tx_hash.clone(),
                    entry.consumed,
                ))
            } else {
                None
            }
        })
    }) {
        return Ok(hit);
    }

    let (signed, consumed) = ncore::decode_field_canonical_slice::<SignedTransaction>(bytes)?;
    let signed = Arc::new(signed);
    let tx_hash = signed.hash();
    let encoded = Arc::new(bytes[..consumed].to_vec());
    let entry = GossipTxDecodeCacheEntry {
        signed: signed.clone(),
        encoded: encoded.clone(),
        tx_hash,
        consumed,
    };
    let key = GossipTxDecodeCacheKey::from_bytes(bytes);
    GOSSIP_TX_DECODE_CACHE.with(|cache| cache.borrow_mut().insert(key, entry));
    Ok((signed, encoded, tx_hash.clone(), consumed))
}

impl GossipTransaction {
    /// Wrap an accepted transaction, dropping acceptance metadata for gossip.
    pub fn new(tx: AcceptedTransaction<'static>) -> Self {
        let signed: SignedTransaction = tx.into();
        let tx_hash = signed.hash();
        Self {
            signed: Arc::new(signed),
            encoded: None,
            tx_hash,
        }
    }

    /// Wrap an already-signed transaction with cached encoded bytes.
    pub fn with_encoded(signed: SignedTransaction, encoded: Arc<Vec<u8>>) -> Self {
        let tx_hash = signed.hash();
        Self {
            signed: Arc::new(signed),
            encoded: Some(encoded),
            tx_hash,
        }
    }

    /// Borrow the signed transaction payload.
    pub fn as_signed(&self) -> &SignedTransaction {
        self.signed.as_ref()
    }

    /// Return the transaction hash without rehashing.
    pub fn hash(&self) -> HashOf<SignedTransaction> {
        self.tx_hash.clone()
    }

    /// Consume the wrapper and return the signed transaction.
    pub fn into_signed(self) -> SignedTransaction {
        self.into_signed_with_payload().0
    }

    /// Consume the wrapper and return the signed transaction and cached payload.
    pub fn into_signed_with_payload(self) -> (SignedTransaction, Option<Arc<Vec<u8>>>) {
        let signed = Arc::try_unwrap(self.signed).unwrap_or_else(|arc| (*arc).clone());
        (signed, self.encoded)
    }
}

impl From<SignedTransaction> for GossipTransaction {
    fn from(signed: SignedTransaction) -> Self {
        let tx_hash = signed.hash();
        Self {
            signed: Arc::new(signed),
            encoded: None,
            tx_hash,
        }
    }
}

impl NoritoSerialize for GossipTransaction {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), ncore::Error> {
        if let Some(encoded) = self.encoded.as_ref() {
            writer.write_all(encoded)?;
            return Ok(());
        }
        self.signed.serialize(writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.encoded
            .as_ref()
            .map(|bytes| bytes.len())
            .or_else(|| self.signed.encoded_len_hint())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded
            .as_ref()
            .map(|bytes| bytes.len())
            .or_else(|| self.signed.encoded_len_exact())
    }
}

impl<'a> NoritoDeserialize<'a> for GossipTransaction {
    fn deserialize(archived: &'a ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("decode gossip transaction")
    }

    fn try_deserialize(archived: &'a ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let bytes = ncore::payload_slice_from_ptr(ptr)?;
        let (signed, encoded, tx_hash, _) = decode_gossip_transaction_payload(bytes)?;
        Ok(Self {
            signed,
            encoded: Some(encoded),
            tx_hash,
        })
    }
}

impl<'a> ncore::DecodeFromSlice<'a> for GossipTransaction {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let (signed, encoded, tx_hash, consumed) = decode_gossip_transaction_payload(bytes)?;
        Ok((
            Self {
                signed,
                encoded: Some(encoded),
                tx_hash,
            },
            consumed,
        ))
    }
}

/// Visibility plane for transaction gossip frames.
#[derive(Decode, Encode, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GossipPlane {
    /// Public lanes/dataspaces; broadcast is permitted.
    Public,
    /// Restricted lanes/dataspaces; targets must be explicitly selected.
    Restricted,
}

const GOSSIP_PLANE_BYTES: usize = core::mem::size_of::<u32>();
const GOSSIP_SEQ_LEN_BYTES: usize = core::mem::size_of::<u64>();

fn gossip_route_encoded_len() -> Option<usize> {
    let route = GossipRoute {
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::GLOBAL,
    };
    route
        .encoded_len_exact()
        .or_else(|| route.encoded_len_hint())
}

fn gossip_message_empty_len() -> Option<usize> {
    let txs_payload_len = GOSSIP_SEQ_LEN_BYTES;
    let routes_payload_len = GOSSIP_SEQ_LEN_BYTES;
    gossip_message_encoded_len(txs_payload_len, routes_payload_len)
}

fn gossip_message_encoded_len(txs_payload_len: usize, routes_payload_len: usize) -> Option<usize> {
    let mut total = ncore::len_prefix_len(txs_payload_len).checked_add(txs_payload_len)?;
    total = total
        .checked_add(ncore::len_prefix_len(routes_payload_len))?
        .checked_add(routes_payload_len)?;
    total = total
        .checked_add(ncore::len_prefix_len(GOSSIP_PLANE_BYTES))?
        .checked_add(GOSSIP_PLANE_BYTES)?;
    Some(total)
}

#[allow(single_use_lifetimes)]
fn gossip_vec_payload_len_exact<'a>(
    items: impl Iterator<Item = &'a GossipTransaction>,
) -> Option<usize> {
    let mut total = GOSSIP_SEQ_LEN_BYTES;
    for item in items {
        let item_len = item.encoded_len_exact()?;
        total = total.checked_add(ncore::len_prefix_len(item_len))?;
        total = total.checked_add(item_len)?;
    }
    Some(total)
}

#[allow(single_use_lifetimes)]
fn gossip_vec_payload_len_cached<'a>(
    items: impl Iterator<Item = &'a GossipTransaction>,
) -> Option<usize> {
    let mut total = GOSSIP_SEQ_LEN_BYTES;
    for item in items {
        let item_len = item.encoded.as_ref().map(|bytes| bytes.len())?;
        total = total.checked_add(ncore::len_prefix_len(item_len))?;
        total = total.checked_add(item_len)?;
    }
    Some(total)
}

fn gossip_routes_payload_len(len: usize) -> Option<usize> {
    let route_len = gossip_route_encoded_len()?;
    let per_elem = ncore::len_prefix_len(route_len).checked_add(route_len)?;
    let elems = per_elem.checked_mul(len)?;
    GOSSIP_SEQ_LEN_BYTES.checked_add(elems)
}

/// Lane/dataspace tags carried alongside gossiped transactions for visibility gating.
#[derive(Debug, Clone, Copy, Decode, Encode)]
pub struct GossipRoute {
    /// Lane assigned to the transaction at the sender.
    pub lane_id: LaneId,
    /// Dataspace assigned to the transaction at the sender.
    pub dataspace_id: DataSpaceId,
}

struct PartitionedGossipBatch {
    message: TransactionGossip,
    requeue: Vec<HashOf<SignedTransaction>>,
    encoded_len: usize,
}

fn partition_gossip_batch(
    max_count: usize,
    frame_cap_bytes: usize,
    plane: GossipPlane,
    txs: Vec<GossipBatchEntry>,
) -> PartitionedGossipBatch {
    let mut message = TransactionGossip {
        txs: Vec::new(),
        routes: Vec::new(),
        plane,
    };
    let reserved = max_count.min(txs.len());
    message.txs.reserve(reserved);
    message.routes.reserve(reserved);
    let mut requeue = Vec::with_capacity(txs.len());
    let mut encoded_len = gossip_message_empty_len().unwrap_or(0);
    let Some(route_len) = gossip_route_encoded_len() else {
        requeue.extend(txs.into_iter().map(|entry| entry.tx.as_ref().hash()));
        return PartitionedGossipBatch {
            message,
            requeue,
            encoded_len,
        };
    };
    let Some(route_entry_len) = ncore::len_prefix_len(route_len).checked_add(route_len) else {
        requeue.extend(txs.into_iter().map(|entry| entry.tx.as_ref().hash()));
        return PartitionedGossipBatch {
            message,
            requeue,
            encoded_len,
        };
    };

    if frame_cap_bytes == 0 {
        requeue.extend(txs.into_iter().map(|entry| entry.tx.as_ref().hash()));
        return PartitionedGossipBatch {
            message,
            requeue,
            encoded_len,
        };
    }

    for entry in txs {
        let hash = entry.tx.as_ref().hash();

        if message.txs.len() >= max_count {
            requeue.push(hash);
            continue;
        }

        let routing = entry.routing;
        let tx_payload_len = entry.payload.len();
        let Some(tx_entry_len) = ncore::len_prefix_len(tx_payload_len).checked_add(tx_payload_len)
        else {
            requeue.push(hash);
            continue;
        };
        let Some(next_encoded_len) = encoded_len
            .checked_add(tx_entry_len)
            .and_then(|total| total.checked_add(route_entry_len))
        else {
            requeue.push(hash);
            continue;
        };
        if next_encoded_len > frame_cap_bytes {
            requeue.push(hash);
            continue;
        }

        message.txs.push(GossipTransaction::with_encoded(
            SignedTransaction::from(entry.tx),
            entry.payload,
        ));
        message.routes.push(GossipRoute {
            lane_id: routing.lane_id,
            dataspace_id: routing.dataspace_id,
        });
        encoded_len = next_encoded_len;
    }

    let mut exact_len = message.encoded_len_exact().unwrap_or(encoded_len);
    while exact_len > frame_cap_bytes && !message.txs.is_empty() {
        if let Some(removed) = message.txs.pop() {
            requeue.push(removed.as_signed().hash());
        }
        message.routes.pop();
        exact_len = message.encoded_len_exact().unwrap_or(exact_len);
    }

    PartitionedGossipBatch {
        message,
        requeue,
        encoded_len: exact_len,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        borrow::Cow,
        collections::BTreeSet,
        num::{NonZeroU32, NonZeroUsize},
        sync::Arc,
        time::Duration,
    };

    use iroha_config::{
        kura::{FsyncMode, InitMode},
        parameters::{
            actual::{
                DataspaceGossipFallback, Kura as KuraConfig, LaneConfig as LaneGeometry,
                LaneProfile, Queue as QueueConfig, RelayMode, RestrictedPublicPayload,
                SoranetHandshake, SoranetPrivacy, SoranetVpn,
            },
            defaults,
        },
    };
    use iroha_config_base::WithOrigin;
    use iroha_data_model::{
        ChainId, DataSpaceId, Level,
        isi::Log,
        nexus::{DataSpaceCatalog, DataSpaceMetadata, LaneCatalog, LaneId, LaneVisibility},
        transaction::TransactionBuilder,
    };
    use iroha_primitives::{addr::socket_addr, time::TimeSource};
    use iroha_test_samples::{
        ALICE_ID, ALICE_KEYPAIR, BOB_KEYPAIR, CARPENTER_KEYPAIR, PEER_KEYPAIR,
    };
    use norito::{codec::Decode, core as ncore};
    use tempfile::tempdir;

    use crate::NetworkMessage;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        queue::{LaneRouter, RoutingDecision},
        state::{State, World},
    };

    fn build_transaction(message: &str) -> (SignedTransaction, AcceptedTransaction<'static>) {
        let chain_id: ChainId = "test-chain".parse().expect("valid chain id");
        let authority = (*ALICE_ID).clone();
        let signed = TransactionBuilder::new(chain_id, authority)
            .with_instructions([Log::new(Level::INFO, message.to_string())])
            .sign(ALICE_KEYPAIR.private_key());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(signed.clone()));
        (signed, accepted)
    }

    fn payload_for(tx: &SignedTransaction) -> Arc<Vec<u8>> {
        Arc::new(norito::codec::Encode::encode(tx))
    }

    #[test]
    fn gossip_transaction_decode_cache_reuses_arcs() {
        let (signed, _accepted) = build_transaction("gossip-decode-cache-test");
        let payload = payload_for(&signed);
        let bytes = payload.as_ref().as_slice();

        let (first, used1) =
            <GossipTransaction as ncore::DecodeFromSlice>::decode_from_slice(bytes)
                .expect("decode first gossip transaction");
        let (second, used2) =
            <GossipTransaction as ncore::DecodeFromSlice>::decode_from_slice(bytes)
                .expect("decode second gossip transaction");

        assert_eq!(used1, bytes.len());
        assert_eq!(used2, bytes.len());
        assert!(
            Arc::ptr_eq(&first.signed, &second.signed),
            "signed transaction must be reused from cache"
        );
        let first_encoded = first
            .encoded
            .as_ref()
            .expect("encoded bytes must be cached");
        let second_encoded = second
            .encoded
            .as_ref()
            .expect("encoded bytes must be cached");
        assert!(
            Arc::ptr_eq(first_encoded, second_encoded),
            "encoded bytes must be reused from cache"
        );
    }

    fn test_network_config(addr: iroha_primitives::addr::SocketAddr) -> NetworkConfig {
        let public_addr = addr.clone();
        NetworkConfig {
            address: WithOrigin::inline(addr),
            public_address: WithOrigin::inline(public_addr),
            relay_mode: RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_ttl: defaults::network::RELAY_TTL,
            soranet_handshake: SoranetHandshake::default(),
            soranet_privacy: SoranetPrivacy::default(),
            soranet_vpn: SoranetVpn::default(),
            lane_profile: LaneProfile::Core,
            require_sm_handshake_match: defaults::network::REQUIRE_SM_HANDSHAKE_MATCH,
            require_sm_openssl_preview_match: defaults::network::REQUIRE_SM_OPENSSL_PREVIEW_MATCH,
            idle_timeout: defaults::network::IDLE_TIMEOUT,
            connect_startup_delay: defaults::network::CONNECT_STARTUP_DELAY,
            dial_timeout: defaults::network::DIAL_TIMEOUT,
            peer_gossip_period: defaults::network::PEER_GOSSIP_PERIOD,
            peer_gossip_max_period: defaults::network::PEER_GOSSIP_PERIOD,
            trust_gossip: defaults::network::TRUST_GOSSIP,
            trust_decay_half_life: defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip: defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer: defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: defaults::network::TRUST_MIN_SCORE,
            dns_refresh_interval: None,
            dns_refresh_ttl: None,
            p2p_proxy: None,
            p2p_proxy_required: false,
            p2p_no_proxy: Vec::new(),
            p2p_proxy_tls_verify: true,
            p2p_proxy_tls_pinned_cert_der_base64: None,
            scion: iroha_config::parameters::actual::ScionConfig::default(),
            quic_enabled: false,
            quic_datagrams_enabled: defaults::network::QUIC_DATAGRAMS_ENABLED,
            quic_datagram_max_payload_bytes: defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
            quic_datagram_receive_buffer_bytes: defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
            quic_datagram_send_buffer_bytes: defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
            tls_enabled: false,
            tls_fallback_to_plain: true,
            tls_listen_address: None,
            tls_inbound_only: false,
            prefer_ws_fallback: false,
            p2p_queue_cap_high: defaults::network::P2P_QUEUE_CAP_HIGH,
            p2p_queue_cap_low: defaults::network::P2P_QUEUE_CAP_LOW,
            p2p_post_queue_cap: defaults::network::P2P_POST_QUEUE_CAP,
            p2p_subscriber_queue_cap: defaults::network::P2P_SUBSCRIBER_QUEUE_CAP,
            consensus_ingress_rate_per_sec: defaults::network::CONSENSUS_INGRESS_RATE_PER_SEC,
            consensus_ingress_burst: defaults::network::CONSENSUS_INGRESS_BURST,
            consensus_ingress_bytes_per_sec: defaults::network::CONSENSUS_INGRESS_BYTES_PER_SEC,
            consensus_ingress_bytes_burst: defaults::network::CONSENSUS_INGRESS_BYTES_BURST,
            consensus_ingress_critical_rate_per_sec:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_RATE_PER_SEC,
            consensus_ingress_critical_burst:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BURST,
            consensus_ingress_critical_bytes_per_sec:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BYTES_PER_SEC,
            consensus_ingress_critical_bytes_burst:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BYTES_BURST,
            consensus_ingress_rbc_session_limit:
                defaults::network::CONSENSUS_INGRESS_RBC_SESSION_LIMIT,
            consensus_ingress_penalty_threshold:
                defaults::network::CONSENSUS_INGRESS_PENALTY_THRESHOLD,
            consensus_ingress_penalty_window: Duration::from_millis(
                defaults::network::CONSENSUS_INGRESS_PENALTY_WINDOW_MS,
            ),
            consensus_ingress_penalty_cooldown: Duration::from_millis(
                defaults::network::CONSENSUS_INGRESS_PENALTY_COOLDOWN_MS,
            ),
            happy_eyeballs_stagger: defaults::network::HAPPY_EYEBALLS_STAGGER,
            addr_ipv6_first: false,
            max_incoming: None,
            max_total_connections: None,
            accept_rate_per_ip_per_sec: None,
            accept_burst_per_ip: None,
            max_accept_buckets: defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits: defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits: defaults::network::ACCEPT_PREFIX_V6_BITS,
            accept_rate_per_prefix_per_sec: None,
            accept_burst_per_prefix: None,
            low_priority_rate_per_sec: None,
            low_priority_burst: None,
            low_priority_bytes_per_sec: None,
            low_priority_bytes_burst: None,
            allowlist_only: false,
            allow_keys: Vec::new(),
            deny_keys: Vec::new(),
            allow_cidrs: Vec::new(),
            deny_cidrs: Vec::new(),
            disconnect_on_post_overflow: defaults::network::DISCONNECT_ON_POST_OVERFLOW,
            max_frame_bytes: defaults::network::MAX_FRAME_BYTES.get(),
            tcp_nodelay: defaults::network::TCP_NODELAY,
            tcp_keepalive: Some(defaults::network::TCP_KEEPALIVE),
            max_frame_bytes_consensus: defaults::network::MAX_FRAME_BYTES_CONSENSUS.get(),
            max_frame_bytes_control: defaults::network::MAX_FRAME_BYTES_CONTROL.get(),
            max_frame_bytes_block_sync: defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC.get(),
            max_frame_bytes_tx_gossip: defaults::network::MAX_FRAME_BYTES_TX_GOSSIP.get(),
            max_frame_bytes_peer_gossip: defaults::network::MAX_FRAME_BYTES_PEER_GOSSIP.get(),
            max_frame_bytes_health: defaults::network::MAX_FRAME_BYTES_HEALTH.get(),
            max_frame_bytes_other: defaults::network::MAX_FRAME_BYTES_OTHER.get(),
            tls_only_v1_3: true,
            quic_max_idle_timeout: None,
        }
    }

    fn closed_test_gossiper(resend_ticks: NonZeroU32) -> TransactionGossiper {
        let temp_dir = tempdir().expect("temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: defaults::kura::BLOCKS_IN_MEMORY,
            block_sync_roster_retention: defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: defaults::kura::ROSTER_SIDECAR_RETENTION,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: defaults::kura::FSYNC_INTERVAL,
        };
        let (kura, _) = Kura::new(&kura_cfg, &LaneGeometry::default()).expect("init kura");
        let live_query = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(World::new(), kura, live_query));
        let queue = Arc::new(Queue::test(
            QueueConfig::default(),
            &TimeSource::new_system(),
        ));
        let now = Instant::now();
        TransactionGossiper {
            chain_id: "test-chain".parse().expect("chain id"),
            gossip_period: Duration::from_millis(50),
            gossip_size: NonZeroU32::new(1).expect("nonzero size"),
            gossip_resend_ticks: resend_ticks,
            gossip_tick: 0,
            gossip_deferred: vec![Vec::new(); resend_ticks.get() as usize],
            peer_recently_sent: BTreeMap::new(),
            peer_recent_ring: vec![Vec::new(); GOSSIP_PEER_RECENT_SUPPRESSION_TTL_TICKS],
            last_drop_count: iroha_p2p::network::subscriber_queue_full_count(),
            last_drop_at: None,
            network: IrohaNetwork::closed_for_tests(),
            queue,
            state,
            tx_frame_cap: 1024,
            dataspace_cfg: DataspaceGossip::default(),
            public_seed: GossipTargetSeed::new(0xBEEF_0001, Duration::from_secs(1), now),
            restricted_seed: GossipTargetSeed::new(0xBEEF_0002, Duration::from_secs(1), now),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gossiper_tx_frame_cap_respects_encrypted_frame_limit() {
        let temp_dir = tempdir().expect("temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: defaults::kura::BLOCKS_IN_MEMORY,
            block_sync_roster_retention: defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: defaults::kura::ROSTER_SIDECAR_RETENTION,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: defaults::kura::FSYNC_INTERVAL,
        };
        let (kura, _) = Kura::new(&kura_cfg, &LaneGeometry::default()).expect("init kura");
        let live_query = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(World::new(), kura, live_query));
        let queue = Arc::new(Queue::test(
            QueueConfig::default(),
            &TimeSource::new_system(),
        ));

        let mut network_cfg = test_network_config(socket_addr!(127.0.0.1:0));
        network_cfg.max_frame_bytes = 512;
        network_cfg.max_frame_bytes_tx_gossip = 1024;
        let self_peer_id = PeerId::new(PEER_KEYPAIR.public_key().clone());
        let max_peer_id = self_peer_id.clone();
        let chain_id: ChainId = "test-chain".parse().expect("chain id");
        let expected =
            tx_gossip_frame_payload_cap(&network_cfg, &chain_id, &self_peer_id, &max_peer_id);

        let network = IrohaNetwork::closed_for_tests();

        let gossiper = TransactionGossiper::from_config(
            chain_id,
            Config {
                gossip_period: Duration::from_millis(1000),
                gossip_size: NonZeroU32::new(1).expect("nonzero size"),
                gossip_resend_ticks: defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS,
                dataspace: DataspaceGossip::default(),
            },
            &network_cfg,
            self_peer_id,
            max_peer_id,
            network,
            queue,
            Arc::clone(&state),
        );

        assert_eq!(gossiper.tx_frame_cap, expected);
    }

    #[test]
    fn gossip_defers_requeue_until_resend_tick() {
        let temp_dir = tempdir().expect("temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: defaults::kura::BLOCKS_IN_MEMORY,
            block_sync_roster_retention: defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: defaults::kura::ROSTER_SIDECAR_RETENTION,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: defaults::kura::FSYNC_INTERVAL,
        };
        let (kura, _) = Kura::new(&kura_cfg, &LaneGeometry::default()).expect("init kura");
        let live_query = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(World::new(), kura, live_query));
        let queue = Arc::new(Queue::test(
            QueueConfig::default(),
            &TimeSource::new_system(),
        ));

        let (_signed, accepted) = build_transaction("defer");
        queue
            .push(accepted, state.view())
            .expect("queue accepts tx");
        let batch = queue.gossip_batch(1, &state.view());
        assert_eq!(batch.len(), 1);
        let hash = batch[0].tx.as_ref().hash();

        let resend_ticks = NonZeroU32::new(2).expect("nonzero resend ticks");
        let now = Instant::now();
        let mut gossiper = TransactionGossiper {
            chain_id: "test-chain".parse().expect("chain id"),
            gossip_period: Duration::from_millis(50),
            gossip_size: NonZeroU32::new(1).expect("nonzero size"),
            gossip_resend_ticks: resend_ticks,
            gossip_tick: 0,
            gossip_deferred: vec![Vec::new(); resend_ticks.get() as usize],
            peer_recently_sent: BTreeMap::new(),
            peer_recent_ring: vec![Vec::new(); GOSSIP_PEER_RECENT_SUPPRESSION_TTL_TICKS],
            last_drop_count: iroha_p2p::network::subscriber_queue_full_count(),
            last_drop_at: None,
            network: IrohaNetwork::closed_for_tests(),
            queue: Arc::clone(&queue),
            state: Arc::clone(&state),
            tx_frame_cap: 1024,
            dataspace_cfg: DataspaceGossip::default(),
            public_seed: GossipTargetSeed::new(0xBEEF_0001, Duration::from_secs(1), now),
            restricted_seed: GossipTargetSeed::new(0xBEEF_0002, Duration::from_secs(1), now),
        };

        gossiper.release_deferred_gossip();
        gossiper.defer_gossip_hashes(vec![hash]);
        gossiper.advance_gossip_tick();
        assert!(queue.gossip_batch(1, &state.view()).is_empty());

        gossiper.release_deferred_gossip();
        gossiper.advance_gossip_tick();
        assert!(queue.gossip_batch(1, &state.view()).is_empty());

        gossiper.release_deferred_gossip();
        let batch = queue.gossip_batch(1, &state.view());
        assert_eq!(batch.len(), 1);
    }

    #[test]
    fn gossip_backpressure_cooldown_respects_last_drop() {
        let temp_dir = tempdir().expect("temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: defaults::kura::BLOCKS_IN_MEMORY,
            block_sync_roster_retention: defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: defaults::kura::ROSTER_SIDECAR_RETENTION,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: defaults::kura::FSYNC_INTERVAL,
        };
        let (kura, _) = Kura::new(&kura_cfg, &LaneGeometry::default()).expect("init kura");
        let live_query = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(World::new(), kura, live_query));
        let queue = Arc::new(Queue::test(
            QueueConfig::default(),
            &TimeSource::new_system(),
        ));

        let resend_ticks = NonZeroU32::new(2).expect("nonzero resend ticks");
        let now = Instant::now();
        let mut gossiper = TransactionGossiper {
            chain_id: "test-chain".parse().expect("chain id"),
            gossip_period: Duration::from_millis(50),
            gossip_size: NonZeroU32::new(1).expect("nonzero size"),
            gossip_resend_ticks: resend_ticks,
            gossip_tick: 0,
            gossip_deferred: vec![Vec::new(); resend_ticks.get() as usize],
            peer_recently_sent: BTreeMap::new(),
            peer_recent_ring: vec![Vec::new(); GOSSIP_PEER_RECENT_SUPPRESSION_TTL_TICKS],
            last_drop_count: u64::MAX,
            last_drop_at: Some(now),
            network: IrohaNetwork::closed_for_tests(),
            queue,
            state,
            tx_frame_cap: 1024,
            dataspace_cfg: DataspaceGossip::default(),
            public_seed: GossipTargetSeed::new(0xBEEF_0001, Duration::from_secs(1), now),
            restricted_seed: GossipTargetSeed::new(0xBEEF_0002, Duration::from_secs(1), now),
        };

        assert!(gossiper.gossip_backpressure_active(now));

        let cooldown = gossiper.backpressure_cooldown();
        let past = now
            .checked_sub(cooldown.saturating_add(Duration::from_millis(1)))
            .unwrap_or(now);
        gossiper.last_drop_at = Some(past);
        assert!(!gossiper.gossip_backpressure_active(now));
    }

    #[test]
    fn peer_recent_suppression_expires_after_ttl_ticks() {
        let resend_ticks = NonZeroU32::new(2).expect("nonzero resend ticks");
        let mut gossiper = closed_test_gossiper(resend_ticks);
        let peer: PeerId = (*PEER_KEYPAIR).public_key().clone().into();
        let (signed, _) = build_transaction("suppression-expiry");
        let tx_hash = signed.hash();

        gossiper.remember_peer_recent_sends(
            std::slice::from_ref(&peer),
            std::slice::from_ref(&tx_hash),
        );
        let (targets, suppressed) = gossiper.filter_targets_by_peer_recent_suppression(
            vec![peer.clone()],
            std::slice::from_ref(&tx_hash),
        );
        assert!(targets.is_empty());
        assert_eq!(suppressed, 1);

        for _ in 0..GOSSIP_PEER_RECENT_SUPPRESSION_TTL_TICKS {
            gossiper.expire_peer_recent_suppression();
            gossiper.advance_gossip_tick();
        }
        gossiper.expire_peer_recent_suppression();

        let (targets, suppressed) = gossiper.filter_targets_by_peer_recent_suppression(
            vec![peer.clone()],
            std::slice::from_ref(&tx_hash),
        );
        assert_eq!(targets, vec![peer]);
        assert_eq!(suppressed, 0);
    }

    #[test]
    fn peer_recent_suppression_keeps_peers_missing_any_hash() {
        let resend_ticks = NonZeroU32::new(2).expect("nonzero resend ticks");
        let mut gossiper = closed_test_gossiper(resend_ticks);
        let peer_a: PeerId = (*ALICE_KEYPAIR).public_key().clone().into();
        let peer_b: PeerId = (*BOB_KEYPAIR).public_key().clone().into();
        let (signed_a, _) = build_transaction("peer-a");
        let (signed_b, _) = build_transaction("peer-b");
        let tx_hash_a = signed_a.hash();
        let tx_hash_b = signed_b.hash();
        let all_hashes = vec![tx_hash_a.clone(), tx_hash_b.clone()];

        gossiper.remember_peer_recent_sends(std::slice::from_ref(&peer_a), &[tx_hash_a]);
        gossiper.remember_peer_recent_sends(std::slice::from_ref(&peer_b), &all_hashes);

        let (targets, suppressed) = gossiper.filter_targets_by_peer_recent_suppression(
            vec![peer_a.clone(), peer_b.clone()],
            &all_hashes,
        );
        assert_eq!(targets, vec![peer_a]);
        assert_eq!(suppressed, 1);
    }

    #[test]
    fn partition_respects_frame_cap() {
        let (small_signed, small_accepted) = build_transaction("small");
        let (large_signed, large_accepted) = build_transaction(&"x".repeat(512));

        let small_route = GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
        };
        let large_route = GossipRoute {
            lane_id: LaneId::new(2),
            dataspace_id: DataSpaceId::new(7),
        };
        let small_len = norito::codec::Encode::encode(&TransactionGossip {
            txs: vec![small_signed.clone().into()],
            routes: vec![small_route],
            plane: GossipPlane::Public,
        })
        .len();
        let both_len = norito::codec::Encode::encode(&TransactionGossip {
            txs: vec![small_signed.clone().into(), large_signed.clone().into()],
            routes: vec![small_route, large_route],
            plane: GossipPlane::Public,
        })
        .len();
        let frame_cap = small_len + 8;
        assert!(frame_cap < both_len, "cap should exclude both transactions");

        let partitioned = partition_gossip_batch(
            usize::MAX,
            frame_cap,
            GossipPlane::Public,
            vec![
                GossipBatchEntry {
                    tx: small_accepted,
                    routing: RoutingDecision::default(),
                    payload: payload_for(&small_signed),
                },
                GossipBatchEntry {
                    tx: large_accepted,
                    routing: RoutingDecision::default(),
                    payload: payload_for(&large_signed),
                },
            ],
        );

        let partitioned_hashes: Vec<_> = partitioned
            .message
            .txs
            .iter()
            .map(|tx| tx.as_signed().hash())
            .collect();
        assert_eq!(partitioned_hashes, vec![small_signed.hash()]);
        assert_eq!(
            partitioned.encoded_len,
            norito::codec::Encode::encode(&partitioned.message).len()
        );
        assert_eq!(partitioned.requeue, vec![large_signed.hash()]);
    }

    #[test]
    fn gossip_roundtrip_preserves_cached_payload() {
        let (signed, _accepted) = build_transaction("cached");
        let payload = payload_for(&signed);
        let message = TransactionGossip {
            txs: vec![GossipTransaction::with_encoded(
                signed.clone(),
                Arc::clone(&payload),
            )],
            routes: vec![GossipRoute {
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::GLOBAL,
            }],
            plane: GossipPlane::Public,
        };

        let encoded = message.encode();
        let decoded: TransactionGossip =
            Decode::decode(&mut encoded.as_slice()).expect("decode gossip");

        assert_eq!(decoded.txs.len(), 1);
        assert_eq!(decoded.routes.len(), 1);
        assert_eq!(decoded.txs[0].as_signed().hash(), signed.hash());
        let decoded_payload = decoded.txs[0].encoded.as_ref().expect("cached payload");
        assert_eq!(decoded_payload.as_slice(), payload.as_slice());
        assert_eq!(decoded.routes[0].lane_id, LaneId::SINGLE);
        assert_eq!(decoded.routes[0].dataspace_id, DataSpaceId::GLOBAL);
        assert_eq!(decoded.plane, GossipPlane::Public);
        assert_eq!(decoded.txs[0].encode().as_slice(), payload.as_slice());
    }

    #[test]
    fn gossip_transaction_len_hints_use_cached_payload() {
        let (signed, _accepted) = build_transaction("hint");
        let payload = payload_for(&signed);
        let tx = GossipTransaction::with_encoded(signed, Arc::clone(&payload));

        assert_eq!(
            ncore::NoritoSerialize::encoded_len_hint(&tx),
            Some(payload.len())
        );
        assert_eq!(
            ncore::NoritoSerialize::encoded_len_exact(&tx),
            Some(payload.len())
        );
    }

    #[test]
    fn gossip_route_encoded_len_matches_wire() {
        let route = GossipRoute {
            lane_id: LaneId::new(3),
            dataspace_id: DataSpaceId::new(7),
        };

        let encoded = route.encode();
        let expected = gossip_route_encoded_len().expect("gossip route len");
        assert_eq!(encoded.len(), expected);

        let decoded: GossipRoute =
            Decode::decode(&mut encoded.as_slice()).expect("decode gossip route");
        assert_eq!(decoded.lane_id, route.lane_id);
        assert_eq!(decoded.dataspace_id, route.dataspace_id);
    }

    #[test]
    fn transaction_gossip_encoded_len_exact_matches_encode() {
        let (signed, _accepted) = build_transaction("len");
        let payload = payload_for(&signed);
        let message = TransactionGossip {
            txs: vec![GossipTransaction::with_encoded(
                signed,
                Arc::clone(&payload),
            )],
            routes: vec![GossipRoute {
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::GLOBAL,
            }],
            plane: GossipPlane::Public,
        };

        let encoded = message.encode();
        assert_eq!(
            ncore::NoritoSerialize::encoded_len_exact(&message),
            Some(encoded.len())
        );
    }

    #[test]
    fn gossip_transaction_decode_rejects_trailing_bytes() {
        let (signed, _accepted) = build_transaction("trailing");
        let mut encoded = norito::codec::Encode::encode(&signed);
        encoded.extend_from_slice(&[0xAA, 0xBB]);

        let err =
            ncore::decode_field_canonical::<GossipTransaction>(&encoded).expect_err("bad bytes");
        assert!(matches!(err, ncore::Error::LengthMismatch));
    }

    #[test]
    fn gossip_network_message_roundtrip_preserves_cached_payload() {
        let (signed, _accepted) = build_transaction("cached-network");
        let payload = payload_for(&signed);
        let message = TransactionGossip {
            txs: vec![GossipTransaction::with_encoded(
                signed.clone(),
                Arc::clone(&payload),
            )],
            routes: vec![GossipRoute {
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::GLOBAL,
            }],
            plane: GossipPlane::Public,
        };

        let network = NetworkMessage::TransactionGossiper(Arc::new(message));
        let encoded = network.encode();
        let decoded: NetworkMessage =
            Decode::decode(&mut encoded.as_slice()).expect("decode network gossip");

        match decoded {
            NetworkMessage::TransactionGossiper(message) => {
                assert_eq!(message.txs.len(), 1);
                assert_eq!(message.routes.len(), 1);
                assert_eq!(message.txs[0].as_signed().hash(), signed.hash());
                let decoded_payload = message.txs[0].encoded.as_ref().expect("cached payload");
                assert_eq!(decoded_payload.as_slice(), payload.as_slice());
                assert_eq!(message.routes[0].lane_id, LaneId::SINGLE);
                assert_eq!(message.routes[0].dataspace_id, DataSpaceId::GLOBAL);
                assert_eq!(message.plane, GossipPlane::Public);
            }
            other => panic!("unexpected network message: {other:?}"),
        }
    }

    #[test]
    fn partition_yields_empty_when_cap_too_small() {
        let (signed, accepted) = build_transaction("tiny");
        let cap = 1;
        let partitioned = partition_gossip_batch(
            usize::MAX,
            cap,
            GossipPlane::Public,
            vec![GossipBatchEntry {
                tx: accepted,
                routing: RoutingDecision::default(),
                payload: payload_for(&signed),
            }],
        );
        assert!(partitioned.message.txs.is_empty());
        assert!(partitioned.message.routes.is_empty());
        assert_eq!(partitioned.requeue, vec![signed.hash()]);
    }

    #[test]
    fn partition_respects_max_count() {
        let (tx_a_signed, tx_a_accepted) = build_transaction("a");
        let (tx_b_signed, tx_b_accepted) = build_transaction("b");

        let cap = norito::codec::Encode::encode(&TransactionGossip {
            txs: vec![tx_a_signed.clone().into(), tx_b_signed.clone().into()],
            routes: vec![
                GossipRoute {
                    lane_id: LaneId::SINGLE,
                    dataspace_id: DataSpaceId::GLOBAL,
                },
                GossipRoute {
                    lane_id: LaneId::new(2),
                    dataspace_id: DataSpaceId::new(5),
                },
            ],
            plane: GossipPlane::Public,
        })
        .len()
            + 16;

        let partitioned = partition_gossip_batch(
            1,
            cap,
            GossipPlane::Public,
            vec![
                GossipBatchEntry {
                    tx: tx_a_accepted,
                    routing: RoutingDecision::default(),
                    payload: payload_for(&tx_a_signed),
                },
                GossipBatchEntry {
                    tx: tx_b_accepted,
                    routing: RoutingDecision::default(),
                    payload: payload_for(&tx_b_signed),
                },
            ],
        );
        let hashes: Vec<_> = partitioned
            .message
            .txs
            .iter()
            .map(|tx| tx.as_signed().hash())
            .collect();
        assert_eq!(hashes, vec![tx_a_signed.hash()]);
        assert_eq!(partitioned.requeue, vec![tx_b_signed.hash()]);
    }

    #[test]
    fn validate_route_rejects_missing_lane() {
        let catalog = LaneCatalog::default();
        let route = GossipRoute {
            lane_id: LaneId::new(5),
            dataspace_id: DataSpaceId::new(7),
        };
        assert!(validate_route(&catalog, route).is_err());
    }

    #[test]
    fn validate_route_rejects_dataspace_mismatch() {
        let lane = iroha_data_model::nexus::LaneConfig {
            id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(2),
            alias: "alpha".to_string(),
            visibility: LaneVisibility::Public,
            ..iroha_data_model::nexus::LaneConfig::default()
        };
        let catalog = LaneCatalog::new(
            core::num::NonZeroU32::new(4).expect("nonzero lanes"),
            vec![lane],
        )
        .expect("lane catalog");
        let route = GossipRoute {
            lane_id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(3),
        };
        assert!(validate_route(&catalog, route).is_err());
    }

    #[test]
    fn validate_route_accepts_matching_lane() {
        let lane = iroha_data_model::nexus::LaneConfig {
            id: LaneId::new(2),
            dataspace_id: DataSpaceId::new(9),
            alias: "beta".to_string(),
            visibility: LaneVisibility::Restricted,
            ..iroha_data_model::nexus::LaneConfig::default()
        };
        let catalog = LaneCatalog::new(
            core::num::NonZeroU32::new(3).expect("nonzero lanes"),
            vec![lane],
        )
        .expect("lane catalog");
        let route = GossipRoute {
            lane_id: LaneId::new(2),
            dataspace_id: DataSpaceId::new(9),
        };
        assert!(validate_route(&catalog, route).is_ok());
    }

    #[test]
    fn dataspace_plane_favors_restricted_when_mixed() {
        let lanes = vec![
            iroha_data_model::nexus::LaneConfig {
                id: LaneId::new(0),
                dataspace_id: DataSpaceId::new(42),
                alias: "public-lane".to_string(),
                visibility: LaneVisibility::Public,
                ..iroha_data_model::nexus::LaneConfig::default()
            },
            iroha_data_model::nexus::LaneConfig {
                id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(42),
                alias: "restricted-lane".to_string(),
                visibility: LaneVisibility::Restricted,
                ..iroha_data_model::nexus::LaneConfig::default()
            },
        ];
        let catalog =
            LaneCatalog::new(core::num::NonZeroU32::new(2).expect("nonzero lanes"), lanes)
                .expect("lane catalog");
        let lane_config = LaneGeometry::from_catalog(&catalog);
        assert_eq!(
            dataspace_plane(&lane_config, DataSpaceId::new(42)),
            Some(GossipPlane::Restricted)
        );
    }

    #[test]
    fn dataspace_plane_handles_unknown_dataspace() {
        let catalog = LaneCatalog::default();
        let lane_config = LaneGeometry::from_catalog(&catalog);
        assert_eq!(dataspace_plane(&lane_config, DataSpaceId::new(999)), None);
    }

    #[test]
    fn gossip_plane_labels_are_stable() {
        assert_eq!(gossip_plane_label(GossipPlane::Public), "public");
        assert_eq!(gossip_plane_label(GossipPlane::Restricted), "restricted");
    }

    #[test]
    fn dataspace_label_renders_numeric_id() {
        let dataspace = DataSpaceId::new(42);
        assert_eq!(dataspace_label(dataspace), "42");
    }

    #[test]
    fn restricted_plan_refuses_public_overlay_policy() {
        let peer: PeerId = (*PEER_KEYPAIR).public_key().clone().into();
        let plan = decide_restricted_target_plan(
            vec![peer.clone()],
            DataspaceGossipFallback::UsePublicOverlay,
            RestrictedPublicPayload::Refuse,
            2,
        );
        assert_eq!(
            plan,
            RestrictedTargetPlan::Drop {
                reason: DROP_REASON_PUBLIC_OVERLAY_REFUSED,
                fallback_used: true,
                fallback_surface: Some(SURFACE_PUBLIC_OVERLAY),
                targets: vec![peer.clone()],
            }
        );
    }

    #[test]
    fn restricted_plan_drops_when_fallback_policy_is_drop() {
        let peer: PeerId = (*PEER_KEYPAIR).public_key().clone().into();
        let plan = decide_restricted_target_plan(
            vec![peer.clone()],
            DataspaceGossipFallback::Drop,
            RestrictedPublicPayload::Forward,
            3,
        );
        assert_eq!(
            plan,
            RestrictedTargetPlan::Drop {
                reason: DROP_REASON_NO_RESTRICTED_TARGETS,
                fallback_used: false,
                fallback_surface: None,
                targets: vec![peer],
            }
        );
    }

    #[test]
    fn restricted_plan_drops_when_no_fallback_targets() {
        let plan = decide_restricted_target_plan(
            Vec::new(),
            DataspaceGossipFallback::UsePublicOverlay,
            RestrictedPublicPayload::Forward,
            1,
        );
        assert_eq!(
            plan,
            RestrictedTargetPlan::Drop {
                reason: DROP_REASON_NO_RESTRICTED_TARGETS,
                fallback_used: true,
                fallback_surface: Some(SURFACE_PUBLIC_OVERLAY),
                targets: Vec::new(),
            }
        );
    }

    #[test]
    fn restricted_plan_forwards_public_overlay_when_allowed() {
        let peer: PeerId = (*PEER_KEYPAIR).public_key().clone().into();
        let plan = decide_restricted_target_plan(
            vec![peer.clone()],
            DataspaceGossipFallback::UsePublicOverlay,
            RestrictedPublicPayload::Forward,
            1,
        );
        assert_eq!(
            plan,
            RestrictedTargetPlan::Send {
                targets: vec![peer],
                fallback_used: true,
                reason: Some(OUTCOME_PUBLIC_OVERLAY_FORWARD),
                fallback_surface: Some(SURFACE_PUBLIC_OVERLAY),
            }
        );
    }

    #[test]
    fn select_targets_dedups_and_caps_with_seed() {
        let targets = vec![
            (*ALICE_KEYPAIR).public_key().clone().into(),
            (*BOB_KEYPAIR).public_key().clone().into(),
            (*ALICE_KEYPAIR).public_key().clone().into(),
            (*PEER_KEYPAIR).public_key().clone().into(),
        ];
        let cap = NonZeroUsize::new(2).expect("non-zero cap");
        let seed = 0xA5A5_1234;

        let (selected, total) =
            TransactionGossiper::select_targets_with_seed(targets.clone(), Some(cap), seed);

        let unique: BTreeSet<_> = targets.into_iter().collect();
        assert_eq!(total, unique.len(), "total should count unique targets");
        assert_eq!(selected.len(), cap.get(), "selection should respect cap");
        assert!(
            selected.iter().all(|peer| unique.contains(peer)),
            "selection must be a subset of inputs"
        );
    }

    #[test]
    fn select_targets_is_deterministic_for_seed() {
        let targets = vec![
            (*ALICE_KEYPAIR).public_key().clone().into(),
            (*BOB_KEYPAIR).public_key().clone().into(),
            (*PEER_KEYPAIR).public_key().clone().into(),
            (*CARPENTER_KEYPAIR).public_key().clone().into(),
        ];
        let cap = NonZeroUsize::new(3).expect("non-zero cap");
        let seed = 0xDEAD_BEEF;

        let (first, _) =
            TransactionGossiper::select_targets_with_seed(targets.clone(), Some(cap), seed);
        let (second, _) = TransactionGossiper::select_targets_with_seed(targets, Some(cap), seed);

        assert_eq!(
            first, second,
            "selection should be stable for the same seed"
        );
    }

    #[test]
    fn seed_for_plane_changes_with_dataspace() {
        let base = 0xCAFE_BABE;
        let first = TransactionGossiper::seed_for_plane(
            base,
            DataSpaceId::new(1),
            GOSSIP_SEED_PUBLIC_DOMAIN,
        );
        let second = TransactionGossiper::seed_for_plane(
            base,
            DataSpaceId::new(2),
            GOSSIP_SEED_PUBLIC_DOMAIN,
        );
        assert_ne!(first, second, "dataspace should perturb the gossip seed");
    }

    #[test]
    fn gossip_target_seed_holds_until_reshuffle_period() {
        let now = Instant::now();
        let mut seed = GossipTargetSeed::new(0xA5A5_1234, Duration::from_secs(5), now);
        let first = seed.current(now);
        let still = seed.current(now + Duration::from_secs(4));
        assert_eq!(first, still, "seed should remain stable before reshuffle");
    }

    #[test]
    fn gossip_target_seed_advances_after_reshuffle_period() {
        let now = Instant::now();
        let mut seed = GossipTargetSeed::new(0xBEEF_0001, Duration::from_secs(5), now);
        let first = seed.current(now);
        let updated = seed.current(now + Duration::from_secs(5));
        assert_eq!(
            updated,
            splitmix64(first),
            "seed should advance on reshuffle"
        );
    }

    #[test]
    fn restricted_plan_caps_commit_topology() {
        let commit = vec![
            (*ALICE_KEYPAIR).public_key().clone().into(),
            (*BOB_KEYPAIR).public_key().clone().into(),
            (*PEER_KEYPAIR).public_key().clone().into(),
        ];
        let cap = NonZeroUsize::new(2);
        let seed = 0x5A5A_0F0F;

        let plan = TransactionGossiper::restricted_target_plan_with_targets(
            commit.clone(),
            commit.clone(),
            cap,
            DataspaceGossipFallback::UsePublicOverlay,
            RestrictedPublicPayload::Forward,
            3,
            seed,
        );

        match plan {
            RestrictedTargetPlan::Send {
                targets,
                fallback_used,
                fallback_surface,
                reason,
            } => {
                let unique: BTreeSet<_> = commit.into_iter().collect();
                assert_eq!(targets.len(), cap.unwrap().get());
                assert!(
                    targets.iter().all(|peer| unique.contains(peer)),
                    "targets must be drawn from the commit topology"
                );
                assert!(!fallback_used);
                assert!(fallback_surface.is_none());
                assert!(reason.is_none());
            }
            other => panic!("expected capped commit plan, got {other:?}"),
        }
    }

    #[test]
    fn restricted_plan_dedups_commit_topology() {
        let duplicated = vec![
            (*ALICE_KEYPAIR).public_key().clone().into(),
            (*BOB_KEYPAIR).public_key().clone().into(),
            (*ALICE_KEYPAIR).public_key().clone().into(),
        ];
        let seed = 0x0102_0304;

        let plan = TransactionGossiper::restricted_target_plan_with_targets(
            duplicated.clone(),
            duplicated.clone(),
            None,
            DataspaceGossipFallback::UsePublicOverlay,
            RestrictedPublicPayload::Forward,
            1,
            seed,
        );

        match plan {
            RestrictedTargetPlan::Send { targets, .. } => {
                let mut expected = duplicated;
                expected.sort();
                expected.dedup();
                assert_eq!(targets, expected, "duplicates should be removed");
            }
            other => panic!("expected deduped commit plan, got {other:?}"),
        }
    }

    #[test]
    fn restricted_plan_filters_commit_topology_to_online_peers() {
        let online_peer: PeerId = (*ALICE_KEYPAIR).public_key().clone().into();
        let offline_peer: PeerId = (*BOB_KEYPAIR).public_key().clone().into();
        let seed = 0xDEC0_1DED;

        let plan = TransactionGossiper::restricted_target_plan_with_targets(
            vec![online_peer.clone(), offline_peer],
            vec![online_peer.clone()],
            None,
            DataspaceGossipFallback::UsePublicOverlay,
            RestrictedPublicPayload::Forward,
            1,
            seed,
        );

        match plan {
            RestrictedTargetPlan::Send { targets, .. } => {
                assert_eq!(targets, vec![online_peer]);
            }
            other => panic!("expected filtered commit plan, got {other:?}"),
        }
    }

    #[test]
    fn restricted_plan_caps_fallback_targets() {
        let fallback = vec![
            (*PEER_KEYPAIR).public_key().clone().into(),
            (*BOB_KEYPAIR).public_key().clone().into(),
            (*ALICE_KEYPAIR).public_key().clone().into(),
        ];
        let cap = NonZeroUsize::new(2);
        let seed = 0x0BAD_F00D;

        let plan = TransactionGossiper::restricted_target_plan_with_targets(
            Vec::new(),
            fallback.clone(),
            cap,
            DataspaceGossipFallback::UsePublicOverlay,
            RestrictedPublicPayload::Forward,
            2,
            seed,
        );

        match plan {
            RestrictedTargetPlan::Send {
                targets,
                fallback_used,
                fallback_surface,
                reason,
            } => {
                let unique: BTreeSet<_> = fallback.into_iter().collect();
                assert_eq!(targets.len(), cap.unwrap().get());
                assert!(
                    targets.iter().all(|peer| unique.contains(peer)),
                    "fallback targets must be drawn from available peers"
                );
                assert!(fallback_used);
                assert_eq!(fallback_surface, Some(SURFACE_PUBLIC_OVERLAY));
                assert_eq!(reason, Some(OUTCOME_PUBLIC_OVERLAY_FORWARD));
            }
            other => panic!("expected capped fallback plan, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gossip_accepts_valid_entries_with_invalid_routes_present() {
        let temp_dir = tempdir().expect("temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: defaults::kura::BLOCKS_IN_MEMORY,
            block_sync_roster_retention: defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: defaults::kura::ROSTER_SIDECAR_RETENTION,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: defaults::kura::FSYNC_INTERVAL,
        };
        let (kura, _) = Kura::new(&kura_cfg, &LaneGeometry::default()).expect("init kura");
        let live_query = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(World::new(), kura, live_query));
        let queue = Arc::new(Queue::test(
            QueueConfig::default(),
            &TimeSource::new_system(),
        ));

        let now = Instant::now();
        let gossiper = TransactionGossiper {
            chain_id: "test-chain".parse().expect("chain id"),
            gossip_period: Duration::from_millis(50),
            gossip_size: NonZeroU32::new(1).expect("nonzero size"),
            gossip_resend_ticks: defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS,
            gossip_tick: 0,
            gossip_deferred: vec![
                Vec::new();
                defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS.get() as usize
            ],
            peer_recently_sent: BTreeMap::new(),
            peer_recent_ring: vec![Vec::new(); GOSSIP_PEER_RECENT_SUPPRESSION_TTL_TICKS],
            last_drop_count: iroha_p2p::network::subscriber_queue_full_count(),
            last_drop_at: None,
            network: IrohaNetwork::closed_for_tests(),
            queue: Arc::clone(&queue),
            state,
            tx_frame_cap: 1024,
            dataspace_cfg: DataspaceGossip::default(),
            public_seed: GossipTargetSeed::new(0xBEEF_0001, Duration::from_secs(1), now),
            restricted_seed: GossipTargetSeed::new(0xBEEF_0002, Duration::from_secs(1), now),
        };

        let (invalid_signed, _) = build_transaction("invalid");
        let (valid_signed, _) = build_transaction("valid");
        let invalid_route = GossipRoute {
            lane_id: LaneId::new(9),
            dataspace_id: DataSpaceId::new(9),
        };
        let valid_route = GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
        };
        gossiper.handle_transaction_gossip(Arc::new(TransactionGossip {
            txs: vec![invalid_signed.into(), valid_signed.into()],
            routes: vec![invalid_route, valid_route],
            plane: GossipPlane::Public,
        }));

        assert_eq!(queue.queued_len(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gossip_skips_already_known_transaction_hashes() {
        let temp_dir = tempdir().expect("temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: defaults::kura::BLOCKS_IN_MEMORY,
            block_sync_roster_retention: defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: defaults::kura::ROSTER_SIDECAR_RETENTION,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: defaults::kura::FSYNC_INTERVAL,
        };
        let (kura, _) = Kura::new(&kura_cfg, &LaneGeometry::default()).expect("init kura");
        let live_query = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(World::new(), kura, live_query));
        let queue = Arc::new(Queue::test(
            QueueConfig::default(),
            &TimeSource::new_system(),
        ));

        let shutdown = ShutdownSignal::new();
        let network_cfg = test_network_config(socket_addr!(127.0.0.1:0));
        let (network, _child) = IrohaNetwork::start(
            KeyPair::random(),
            network_cfg,
            None,
            None,
            None,
            shutdown.clone(),
        )
        .await
        .expect("network starts");

        let (known_signed, _) = build_transaction("known");
        queue
            .push(
                AcceptedTransaction::new_unchecked(Cow::Owned(known_signed.clone())),
                state.view(),
            )
            .expect("seed queue with known tx");
        assert_eq!(queue.queued_len(), 1, "queue should contain the seeded tx");

        let now = Instant::now();
        let gossiper = TransactionGossiper {
            chain_id: "test-chain".parse().expect("chain id"),
            gossip_period: Duration::from_millis(50),
            gossip_size: NonZeroU32::new(1).expect("nonzero size"),
            gossip_resend_ticks: defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS,
            gossip_tick: 0,
            gossip_deferred: vec![
                Vec::new();
                defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS.get() as usize
            ],
            peer_recently_sent: BTreeMap::new(),
            peer_recent_ring: vec![Vec::new(); GOSSIP_PEER_RECENT_SUPPRESSION_TTL_TICKS],
            last_drop_count: iroha_p2p::network::subscriber_queue_full_count(),
            last_drop_at: None,
            network,
            queue: Arc::clone(&queue),
            state,
            tx_frame_cap: 1024,
            dataspace_cfg: DataspaceGossip::default(),
            public_seed: GossipTargetSeed::new(0xBEEF_0001, Duration::from_secs(1), now),
            restricted_seed: GossipTargetSeed::new(0xBEEF_0002, Duration::from_secs(1), now),
        };

        gossiper.handle_transaction_gossip(Arc::new(TransactionGossip {
            txs: vec![known_signed.into()],
            routes: vec![GossipRoute {
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::GLOBAL,
            }],
            plane: GossipPlane::Public,
        }));

        assert_eq!(
            queue.queued_len(),
            1,
            "already-known gossip should be ignored without queue churn"
        );

        shutdown.send();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gossip_drops_route_mismatch() {
        struct FixedRouter {
            lane: LaneId,
            dataspace: DataSpaceId,
        }

        impl LaneRouter for FixedRouter {
            fn route(&self, _tx: &crate::tx::AcceptedTransaction<'_>) -> RoutingDecision {
                RoutingDecision::new(self.lane, self.dataspace)
            }
        }

        let temp_dir = tempdir().expect("temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: defaults::kura::BLOCKS_IN_MEMORY,
            block_sync_roster_retention: defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: defaults::kura::ROSTER_SIDECAR_RETENTION,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: defaults::kura::FSYNC_INTERVAL,
        };
        let (kura, _) = Kura::new(&kura_cfg, &LaneGeometry::default()).expect("init kura");
        let live_query = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(World::new(), kura, live_query));
        let queue = Arc::new(Queue::test_with_router(
            QueueConfig::default(),
            &TimeSource::new_system(),
            Arc::new(FixedRouter {
                lane: LaneId::new(1),
                dataspace: DataSpaceId::new(7),
            }),
        ));

        let now = Instant::now();
        let gossiper = TransactionGossiper {
            chain_id: "test-chain".parse().expect("chain id"),
            gossip_period: Duration::from_millis(50),
            gossip_size: NonZeroU32::new(1).expect("nonzero size"),
            gossip_resend_ticks: defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS,
            gossip_tick: 0,
            gossip_deferred: vec![
                Vec::new();
                defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS.get() as usize
            ],
            peer_recently_sent: BTreeMap::new(),
            peer_recent_ring: vec![Vec::new(); GOSSIP_PEER_RECENT_SUPPRESSION_TTL_TICKS],
            last_drop_count: iroha_p2p::network::subscriber_queue_full_count(),
            last_drop_at: None,
            network: IrohaNetwork::closed_for_tests(),
            queue: Arc::clone(&queue),
            state,
            tx_frame_cap: 1024,
            dataspace_cfg: DataspaceGossip::default(),
            public_seed: GossipTargetSeed::new(0xBEEF_0001, Duration::from_secs(1), now),
            restricted_seed: GossipTargetSeed::new(0xBEEF_0002, Duration::from_secs(1), now),
        };

        let (signed, _) = build_transaction("route-mismatch");
        let route = GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
        };
        gossiper.handle_transaction_gossip(Arc::new(TransactionGossip {
            txs: vec![signed.into()],
            routes: vec![route],
            plane: GossipPlane::Public,
        }));

        assert_eq!(queue.queued_len(), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    #[allow(clippy::too_many_lines)]
    async fn gossip_accepts_restricted_route_match() {
        struct FixedRouter {
            lane: LaneId,
            dataspace: DataSpaceId,
        }

        impl LaneRouter for FixedRouter {
            fn route(&self, _tx: &crate::tx::AcceptedTransaction<'_>) -> RoutingDecision {
                RoutingDecision::new(self.lane, self.dataspace)
            }
        }

        let temp_dir = tempdir().expect("temp dir");
        let kura_cfg = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
            max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: defaults::kura::BLOCKS_IN_MEMORY,
            block_sync_roster_retention: defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: defaults::kura::ROSTER_SIDECAR_RETENTION,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: defaults::kura::FSYNC_INTERVAL,
        };
        let (kura, _) = Kura::new(&kura_cfg, &LaneGeometry::default()).expect("init kura");
        let live_query = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(World::new(), kura, live_query));

        let restricted_dataspace = DataSpaceId::new(7);
        let restricted_lane = LaneId::new(1);
        let lane_catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("nonzero lanes"),
            vec![
                iroha_data_model::nexus::LaneConfig {
                    id: LaneId::SINGLE,
                    alias: "public".to_string(),
                    ..iroha_data_model::nexus::LaneConfig::default()
                },
                iroha_data_model::nexus::LaneConfig {
                    id: restricted_lane,
                    dataspace_id: restricted_dataspace,
                    alias: "restricted".to_string(),
                    visibility: LaneVisibility::Restricted,
                    ..iroha_data_model::nexus::LaneConfig::default()
                },
            ],
        )
        .expect("lane catalog");
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: restricted_dataspace,
                alias: "restricted".to_string(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        {
            let mut nexus = state.nexus.write();
            nexus.enabled = true;
            nexus.lane_catalog = lane_catalog.clone();
            nexus.lane_config = LaneGeometry::from_catalog(&lane_catalog);
            nexus.dataspace_catalog = dataspace_catalog;
        }

        let queue = Arc::new(Queue::test_with_router(
            QueueConfig::default(),
            &TimeSource::new_system(),
            Arc::new(FixedRouter {
                lane: restricted_lane,
                dataspace: restricted_dataspace,
            }),
        ));

        let now = Instant::now();
        let gossiper = TransactionGossiper {
            chain_id: "test-chain".parse().expect("chain id"),
            gossip_period: Duration::from_millis(50),
            gossip_size: NonZeroU32::new(1).expect("nonzero size"),
            gossip_resend_ticks: defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS,
            gossip_tick: 0,
            gossip_deferred: vec![
                Vec::new();
                defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS.get() as usize
            ],
            peer_recently_sent: BTreeMap::new(),
            peer_recent_ring: vec![Vec::new(); GOSSIP_PEER_RECENT_SUPPRESSION_TTL_TICKS],
            last_drop_count: iroha_p2p::network::subscriber_queue_full_count(),
            last_drop_at: None,
            network: IrohaNetwork::closed_for_tests(),
            queue: Arc::clone(&queue),
            state,
            tx_frame_cap: 1024,
            dataspace_cfg: DataspaceGossip::default(),
            public_seed: GossipTargetSeed::new(0xBEEF_0001, Duration::from_secs(1), now),
            restricted_seed: GossipTargetSeed::new(0xBEEF_0002, Duration::from_secs(1), now),
        };

        let (signed, _) = build_transaction("restricted-route");
        let route = GossipRoute {
            lane_id: restricted_lane,
            dataspace_id: restricted_dataspace,
        };
        gossiper.handle_transaction_gossip(Arc::new(TransactionGossip {
            txs: vec![signed.into()],
            routes: vec![route],
            plane: GossipPlane::Restricted,
        }));

        assert_eq!(queue.queued_len(), 1);
    }
}
